use crate::ability;
use crate::ability::{Loadout, OrganismAbilities, ShotPhase, SporePhase};
use crate::game::GameState;
use crate::geometry::{LatticeCoordinate, SubpixelPoint, WorldPoint};
use crate::member;
use crate::member::{Member, MemberId, TeamKind};
use crate::organism::Organism;

struct AcidSource {
    owner_id: MemberId,
    owner_team: Option<TeamKind>,
    spore_secretion_positions: Vec<SubpixelPoint>,
    shot_secretion_centers: Vec<SubpixelPoint>,
    toxin_field_center: Option<WorldPoint>,
}

impl AcidSource {
    fn from_member(member: &Member) -> Option<AcidSource> {
        let loadout: &Loadout = member.loadout.as_ref()?;
        let organism: &Organism = member.organism.as_ref()?;

        Some(AcidSource {
            owner_id: member.member_id,
            owner_team: member.team,
            spore_secretion_positions: get_spore_secretion_positions(&organism.abilities),
            shot_secretion_centers: get_shot_secretion_centers(&organism.abilities),
            toxin_field_center: organism.abilities.get_toxin_field_center(loadout),
        })
    }

    /// Spore and shot acid reach the owner's own cells; toxin never does.
    fn removes_cell(&self, victim_id: MemberId, cell_center: WorldPoint) -> bool {
        let cell_center_subpixels: SubpixelPoint = cell_center.to_subpixel_point();
        let is_in_spore_secretion: bool = self
            .spore_secretion_positions
            .iter()
            .any(|spore_position| ability::is_inside_spore_secretion(*spore_position, cell_center_subpixels));
        let is_in_shot_secretion: bool = self
            .shot_secretion_centers
            .iter()
            .any(|shot_center| ability::is_inside_shot_secretion(*shot_center, cell_center_subpixels));
        let is_in_toxin_field: bool = victim_id != self.owner_id
            && self
                .toxin_field_center
                .is_some_and(|field_center| ability::is_inside_field(field_center, cell_center));

        is_in_spore_secretion || is_in_shot_secretion || is_in_toxin_field
    }
}

/// Removes every cell inside an acid source of a non-teammate and outside every neutralize field.
pub fn run_damage_phase(state: &mut GameState) {
    let acid_sources: Vec<AcidSource> = state.members.values().filter_map(AcidSource::from_member).collect();
    let neutralize_field_centers: Vec<WorldPoint> = get_neutralize_field_centers(state);

    for member in state.members.values_mut() {
        damage_member(member, &acid_sources, &neutralize_field_centers);
    }
}

/// The owner of the last removal, in owner order, is the last hitter.
fn damage_member(member: &mut Member, acid_sources: &[AcidSource], neutralize_field_centers: &[WorldPoint]) {
    let victim_id: MemberId = member.member_id;
    let victim_team: Option<TeamKind> = member.team;
    let Some(organism): Option<&mut Organism> = member.organism.as_mut() else {
        return;
    };

    for acid_source in acid_sources {
        let is_teammate: bool =
            victim_id != acid_source.owner_id && member::is_same_team(victim_team, acid_source.owner_team);

        if is_teammate {
            continue;
        }

        let lattice_coordinates: Vec<LatticeCoordinate> = organism.cells.iter().collect();

        for lattice_coordinate in lattice_coordinates {
            let cell_center: WorldPoint = organism.cell_center(lattice_coordinate);
            let is_neutralized: bool = is_protected_by_neutralize(neutralize_field_centers, cell_center);

            if is_neutralized || !acid_source.removes_cell(victim_id, cell_center) {
                continue;
            }

            organism.cells.remove(lattice_coordinate);
            organism.last_hitter = Some(acid_source.owner_id);
        }
    }
}

fn get_spore_secretion_positions(abilities: &OrganismAbilities) -> Vec<SubpixelPoint> {
    match &abilities.spore {
        SporePhase::Secreting { spores, .. } => spores.iter().map(|spore| spore.position).collect(),
        SporePhase::Ready | SporePhase::Flying { .. } | SporePhase::Cooling { .. } => Vec::new(),
    }
}

fn get_shot_secretion_centers(abilities: &OrganismAbilities) -> Vec<SubpixelPoint> {
    abilities
        .shots
        .iter()
        .filter_map(|shot_phase| match shot_phase {
            ShotPhase::Secreting { center, .. } => Some(*center),
            ShotPhase::Ready | ShotPhase::Flying { .. } | ShotPhase::Cooling { .. } => None,
        })
        .collect()
}

fn is_protected_by_neutralize(neutralize_field_centers: &[WorldPoint], cell_center: WorldPoint) -> bool {
    neutralize_field_centers
        .iter()
        .any(|field_center| ability::is_inside_field(*field_center, cell_center))
}

fn get_neutralize_field_centers(state: &GameState) -> Vec<WorldPoint> {
    state
        .members
        .values()
        .filter_map(|member| {
            let loadout: &Loadout = member.loadout.as_ref()?;
            let organism: &Organism = member.organism.as_ref()?;

            organism.abilities.get_neutralize_field_center(loadout)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::{AbilityPhase, Projectile, ThirdAbilityKind};
    use crate::game::{GameModeKind, Tick, test_fixture};
    use crate::geometry::SubpixelVector;
    use crate::world::WorldShapeKind;

    const OWNER_ID: MemberId = MemberId(0);
    const VICTIM_ID: MemberId = MemberId(1);
    const OTHER_OWNER_ID: MemberId = MemberId(2);
    const VICTIM_POSITION: WorldPoint = WorldPoint { x: 300, y: 300 };

    fn create_state(mode: GameModeKind) -> GameState {
        let mut state: GameState = test_fixture::create_state(mode, WorldShapeKind::Rectangle, 800);

        for (member_id, position) in [
            (OWNER_ID, WorldPoint { x: 100, y: 100 }),
            (VICTIM_ID, VICTIM_POSITION),
            (OTHER_OWNER_ID, WorldPoint { x: 600, y: 600 }),
        ] {
            state.members.insert(
                member_id,
                test_fixture::create_participant_with_organism(member_id, position),
            );
        }

        test_fixture::get_organism_mut(&mut state, VICTIM_ID).cells.insert(LatticeCoordinate { i: 1, j: 0 });

        state
    }

    fn get_abilities_mut(state: &mut GameState, member_id: MemberId) -> &mut OrganismAbilities {
        &mut test_fixture::get_organism_mut(state, member_id).abilities
    }

    fn secrete_spore_at(state: &mut GameState, member_id: MemberId, position: WorldPoint) {
        get_abilities_mut(state, member_id).spore = SporePhase::Secreting {
            ends_at: Tick(20),
            spores: vec![Projectile {
                position: position.to_subpixel_point(),
                velocity: SubpixelVector { x: 0, y: 0 },
            }],
        };
    }

    fn secrete_shot_at(state: &mut GameState, member_id: MemberId, slot_index: usize, position: WorldPoint) {
        get_abilities_mut(state, member_id).shots[slot_index] = ShotPhase::Secreting {
            ends_at: Tick(20),
            center: position.to_subpixel_point(),
        };
    }

    fn activate_field(state: &mut GameState, member_id: MemberId, third: ThirdAbilityKind, center: WorldPoint) {
        let loadout: &mut Loadout = state.members.get_mut(&member_id).unwrap().loadout.as_mut().unwrap();
        loadout.third = third;

        let abilities: &mut OrganismAbilities = get_abilities_mut(state, member_id);
        abilities.third = AbilityPhase::Active { ends_at: Tick(20) };
        abilities.third_center = Some(center);
    }

    fn get_cells(state: &GameState, member_id: MemberId) -> Vec<LatticeCoordinate> {
        test_fixture::get_organism(state, member_id).cells.iter().collect()
    }

    #[test]
    fn run_damage_phase_removes_cells_inside_a_spore_secretion() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        secrete_spore_at(&mut state, OWNER_ID, WorldPoint { x: 280, y: 300 });

        run_damage_phase(&mut state);

        assert_eq!(get_cells(&state, VICTIM_ID), vec![LatticeCoordinate { i: 1, j: 0 }]);
        assert_eq!(
            test_fixture::get_organism(&state, VICTIM_ID).last_hitter,
            Some(OWNER_ID)
        );
    }

    #[test]
    fn run_damage_phase_removes_cells_inside_a_shot_secretion() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        secrete_shot_at(&mut state, OWNER_ID, 1, WorldPoint { x: 318, y: 300 });

        run_damage_phase(&mut state);

        assert_eq!(get_cells(&state, VICTIM_ID), vec![LatticeCoordinate { i: 0, j: 0 }]);
    }

    #[test]
    fn run_damage_phase_lets_acid_reach_its_owner() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        secrete_spore_at(&mut state, VICTIM_ID, VICTIM_POSITION);

        run_damage_phase(&mut state);

        let victim_organism: &Organism = test_fixture::get_organism(&state, VICTIM_ID);
        assert!(victim_organism.cells.is_empty());
        assert_eq!(victim_organism.last_hitter, Some(VICTIM_ID));
    }

    #[test]
    fn run_damage_phase_lets_acid_reach_its_owner_in_a_team_mode() {
        let mut state: GameState = create_state(GameModeKind::Skirmish);

        for member in state.members.values_mut() {
            member.team = Some(TeamKind::Red);
        }

        secrete_spore_at(&mut state, VICTIM_ID, VICTIM_POSITION);

        run_damage_phase(&mut state);

        let victim_organism: &Organism = test_fixture::get_organism(&state, VICTIM_ID);
        assert!(victim_organism.cells.is_empty());
        assert_eq!(victim_organism.last_hitter, Some(VICTIM_ID));
    }

    #[test]
    fn run_damage_phase_keeps_toxin_off_its_owner() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        activate_field(&mut state, VICTIM_ID, ThirdAbilityKind::Toxin, VICTIM_POSITION);
        activate_field(
            &mut state,
            OWNER_ID,
            ThirdAbilityKind::Toxin,
            WorldPoint { x: 245, y: 300 },
        );

        run_damage_phase(&mut state);

        assert_eq!(get_cells(&state, VICTIM_ID), vec![LatticeCoordinate { i: 1, j: 0 }]);
    }

    #[test]
    fn run_damage_phase_spares_cells_inside_any_neutralize_field() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        secrete_spore_at(&mut state, OWNER_ID, VICTIM_POSITION);
        activate_field(
            &mut state,
            OTHER_OWNER_ID,
            ThirdAbilityKind::Neutralize,
            WorldPoint { x: 245, y: 300 },
        );

        run_damage_phase(&mut state);

        assert_eq!(get_cells(&state, VICTIM_ID), vec![LatticeCoordinate { i: 0, j: 0 }]);
    }

    #[test]
    fn run_damage_phase_spares_teammates_of_the_owner() {
        let mut state: GameState = create_state(GameModeKind::Skirmish);

        for member in state.members.values_mut() {
            member.team = Some(TeamKind::Red);
        }

        secrete_spore_at(&mut state, OWNER_ID, VICTIM_POSITION);
        activate_field(&mut state, OWNER_ID, ThirdAbilityKind::Toxin, VICTIM_POSITION);

        run_damage_phase(&mut state);

        let victim_organism: &Organism = test_fixture::get_organism(&state, VICTIM_ID);
        assert_eq!(victim_organism.cells.count(), 2);
        assert_eq!(victim_organism.last_hitter, None);
    }

    #[test]
    fn run_damage_phase_credits_the_last_owner_in_id_order() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        secrete_spore_at(&mut state, OTHER_OWNER_ID, WorldPoint { x: 330, y: 300 });
        activate_field(
            &mut state,
            OWNER_ID,
            ThirdAbilityKind::Toxin,
            WorldPoint { x: 245, y: 300 },
        );

        run_damage_phase(&mut state);

        let victim_organism: &Organism = test_fixture::get_organism(&state, VICTIM_ID);
        assert!(victim_organism.cells.is_empty());
        assert_eq!(victim_organism.last_hitter, Some(OTHER_OWNER_ID));
    }

    #[test]
    fn run_damage_phase_records_no_hitter_for_neutralized_acid() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        secrete_spore_at(&mut state, OWNER_ID, VICTIM_POSITION);
        activate_field(&mut state, VICTIM_ID, ThirdAbilityKind::Neutralize, VICTIM_POSITION);

        run_damage_phase(&mut state);

        let victim_organism: &Organism = test_fixture::get_organism(&state, VICTIM_ID);
        assert_eq!(victim_organism.cells.count(), 2);
        assert_eq!(victim_organism.last_hitter, None);
    }
}
