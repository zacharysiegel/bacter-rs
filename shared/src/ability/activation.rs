use crate::ability::ability_constants;
use crate::ability::projectile;
use crate::ability::{
    AbilityPhase, AbilityPressSet, FirstAbilityKind, Loadout, OrganismAbilities, Projectile, SecondAbilityKind,
    SporePhase,
};
use crate::game::{GameState, PlayerTickInput, SimulationEvent, Tick};
use crate::member::{Member, MemberId};
use crate::organism::Organism;

pub fn run_timer_expiry_phase(state: &mut GameState, tick: Tick) {
    for member in state.members.values_mut() {
        let Some(loadout) = member.loadout else {
            continue;
        };

        let Some(organism) = member.organism.as_mut() else {
            continue;
        };

        expire_timers(&mut organism.abilities, &loadout, tick);
    }
}

pub fn expire_timers(abilities: &mut OrganismAbilities, loadout: &Loadout, tick: Tick) {
    abilities.first.expire(tick, loadout.first.cooldown_ticks());
    abilities.second.expire(tick, loadout.second.cooldown_ticks());
    abilities.third.expire(tick, loadout.third.cooldown_ticks());

    if !abilities.third.is_active() {
        abilities.third_center = None;
    }

    abilities.spore.expire(tick);

    for shot_phase in &mut abilities.shots {
        shot_phase.expire(tick);
    }

    abilities.compressed_until = abilities.compressed_until.filter(|compressed_until| *compressed_until > tick);
    abilities.frozen_until = abilities.frozen_until.filter(|frozen_until| *frozen_until > tick);
}

/// Inputs apply in ascending member id; members without an organism press nothing.
pub fn run_ability_press_phase(
    state: &mut GameState,
    player_inputs: &[PlayerTickInput],
    tick: Tick,
) -> Vec<SimulationEvent> {
    let mut ordered_player_inputs: Vec<&PlayerTickInput> = player_inputs.iter().collect();
    ordered_player_inputs.sort_by_key(|player_input| player_input.member_id);

    let mut simulation_events: Vec<SimulationEvent> = Vec::new();

    for player_input in ordered_player_inputs {
        let press_events: Vec<SimulationEvent> = apply_presses(state, player_input, tick);
        simulation_events.extend(press_events);
    }

    simulation_events
}

fn apply_presses(state: &mut GameState, player_input: &PlayerTickInput, tick: Tick) -> Vec<SimulationEvent> {
    let presses: AbilityPressSet = player_input.ability_presses;
    let mut simulation_events: Vec<SimulationEvent> = Vec::new();

    if presses.contains(AbilityPressSet::FIRST) {
        let first_press_events: Vec<SimulationEvent> = press_first(state, player_input, tick);
        simulation_events.extend(first_press_events);
    }

    if presses.contains(AbilityPressSet::SECOND) {
        let second_press_events: Vec<SimulationEvent> = press_second(state, player_input, tick);
        simulation_events.extend(second_press_events);
    }

    if presses.contains(AbilityPressSet::THIRD) {
        press_third(state, player_input.member_id, tick);
    }

    if presses.contains(AbilityPressSet::FOURTH) {
        press_fourth(state, player_input.member_id, tick);
    }

    simulation_events
}

fn press_first(state: &mut GameState, player_input: &PlayerTickInput, tick: Tick) -> Vec<SimulationEvent> {
    let Some((loadout, organism)) = get_loadout_and_organism(state, player_input.member_id) else {
        return Vec::new();
    };

    if !organism.abilities.first.is_ready() {
        return Vec::new();
    }

    match loadout.first {
        FirstAbilityKind::Extend => {
            organism.abilities.first = AbilityPhase::Active {
                ends_at: tick.plus(loadout.first.active_ticks()),
            };

            Vec::new()
        }
        FirstAbilityKind::Compress => Vec::new(),
    }
}

fn press_second(state: &mut GameState, player_input: &PlayerTickInput, tick: Tick) -> Vec<SimulationEvent> {
    let Some((loadout, organism)) = get_loadout_and_organism(state, player_input.member_id) else {
        return Vec::new();
    };

    if !organism.abilities.second.is_ready() {
        return Vec::new();
    }

    match loadout.second {
        SecondAbilityKind::Immortality => {
            organism.abilities.second = AbilityPhase::Active {
                ends_at: tick.plus(loadout.second.active_ticks()),
            };

            Vec::new()
        }
        SecondAbilityKind::Freeze => Vec::new(),
    }
}

fn press_third(state: &mut GameState, member_id: MemberId, tick: Tick) {
    let Some((loadout, organism)) = get_loadout_and_organism(state, member_id) else {
        return;
    };

    if !organism.abilities.third.is_ready() {
        return;
    }

    organism.abilities.third = AbilityPhase::Active {
        ends_at: tick.plus(loadout.third.active_ticks()),
    };
    organism.abilities.third_center = Some(organism.cursor);
}

fn press_fourth(state: &mut GameState, member_id: MemberId, tick: Tick) {
    let Some((_, organism)) = get_loadout_and_organism(state, member_id) else {
        return;
    };

    match organism.abilities.spore {
        SporePhase::Ready => {
            let spores: Vec<Projectile> = projectile::launch_spores(organism);

            organism.abilities.spore = SporePhase::Flying {
                ends_at: tick.plus(ability_constants::SPORE_FLIGHT_TICKS),
                spores,
            };
        }
        SporePhase::Flying { ref mut spores, .. } => {
            let secreting_spores: Vec<Projectile> = std::mem::take(spores);

            organism.abilities.spore = SporePhase::Secreting {
                ends_at: tick.plus(ability_constants::SPORE_SECRETION_TICKS),
                spores: secreting_spores,
            };
        }
        SporePhase::Secreting { .. } | SporePhase::Cooling { .. } => {}
    }
}

fn get_loadout_and_organism(state: &mut GameState, member_id: MemberId) -> Option<(Loadout, &mut Organism)> {
    let member: &mut Member = state.members.get_mut(&member_id)?;
    let loadout: Loadout = member.loadout?;
    let organism: &mut Organism = member.organism.as_mut()?;

    Some((loadout, organism))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::{AimVector, ShotPhase, ThirdAbilityKind};
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::{LatticeCoordinate, SubpixelPoint, SubpixelVector, WorldPoint};
    use crate::world::WorldShapeKind;

    const CASTER_ID: MemberId = MemberId(0);
    const TARGET_ID: MemberId = MemberId(1);
    const PRESS_TICK: Tick = Tick(10);

    fn get_abilities(state: &GameState, member_id: MemberId) -> &OrganismAbilities {
        &test_fixture::get_organism(state, member_id).abilities
    }

    fn get_loadout_mut(state: &mut GameState, member_id: MemberId) -> &mut Loadout {
        state.members.get_mut(&member_id).unwrap().loadout.as_mut().unwrap()
    }

    fn create_input(member_id: MemberId, ability_presses: AbilityPressSet, aim: Option<AimVector>) -> PlayerTickInput {
        PlayerTickInput {
            member_id,
            cursor: WorldPoint { x: 0, y: 0 },
            ability_presses,
            aim,
        }
    }

    fn press(
        state: &mut GameState,
        member_id: MemberId,
        ability_presses: AbilityPressSet,
        aim: Option<AimVector>,
        tick: Tick,
    ) -> Vec<SimulationEvent> {
        run_ability_press_phase(state, &[create_input(member_id, ability_presses, aim)], tick)
    }

    fn insert_cells(state: &mut GameState, member_id: MemberId, lattice_coordinates: &[(i32, i32)]) {
        let organism: &mut Organism = test_fixture::get_organism_mut(state, member_id);

        for (i, j) in lattice_coordinates {
            organism.cells.insert(LatticeCoordinate { i: *i, j: *j });
        }
    }

    #[test]
    fn expire_timers_uses_the_cooldown_of_the_chosen_kind() {
        let mut loadout: Loadout = test_fixture::create_loadout();
        loadout.third = ThirdAbilityKind::Toxin;
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();
        abilities.first = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.second = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.third = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.third_center = Some(WorldPoint { x: 4, y: 4 });

        expire_timers(&mut abilities, &loadout, Tick(50));

        assert_eq!(abilities.first, AbilityPhase::Cooling { ready_at: Tick(107) });
        assert_eq!(abilities.second, AbilityPhase::Cooling { ready_at: Tick(136) });
        assert_eq!(abilities.third, AbilityPhase::Cooling { ready_at: Tick(136) });
        assert_eq!(abilities.third_center, None);
    }

    #[test]
    fn expire_timers_cools_an_ended_spore_and_every_ended_shot() {
        let loadout: Loadout = test_fixture::create_loadout();
        let projectile: Projectile = Projectile {
            position: SubpixelPoint { x: 0, y: 0 },
            velocity: SubpixelVector { x: 1, y: 0 },
        };
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();
        abilities.spore = SporePhase::Secreting {
            ends_at: Tick(50),
            spores: vec![projectile],
        };
        abilities.shots = [
            ShotPhase::Flying {
                ends_at: Tick(50),
                shot: projectile,
            },
            ShotPhase::Secreting {
                ends_at: Tick(50),
                center: SubpixelPoint { x: 0, y: 0 },
            },
        ];

        expire_timers(&mut abilities, &loadout, Tick(50));

        assert_eq!(abilities.spore, SporePhase::Cooling { ready_at: Tick(157) });
        assert_eq!(
            abilities.shots,
            [
                ShotPhase::Cooling { ready_at: Tick(79) },
                ShotPhase::Cooling { ready_at: Tick(79) },
            ],
        );
    }

    #[test]
    fn expire_timers_clears_received_effects_on_their_deadline() {
        let loadout: Loadout = test_fixture::create_loadout();
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();
        abilities.compressed_until = Some(Tick(60));
        abilities.frozen_until = Some(Tick(61));

        expire_timers(&mut abilities, &loadout, Tick(60));

        assert_eq!(abilities.compressed_until, None);
        assert_eq!(abilities.frozen_until, Some(Tick(61)));
    }

    #[test]
    fn expire_timers_keeps_an_active_field_centre() {
        let loadout: Loadout = test_fixture::create_loadout();
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();
        abilities.third = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.third_center = Some(WorldPoint { x: 4, y: 4 });

        expire_timers(&mut abilities, &loadout, Tick(49));

        assert_eq!(abilities.third_center, Some(WorldPoint { x: 4, y: 4 }));
    }

    #[test]
    fn run_timer_expiry_phase_advances_every_organism() {
        let mut state: GameState = test_fixture::create_state_with_organisms(
            GameModeKind::FreeForAll,
            800,
            &[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 300, y: 100 }],
        );

        for member_id in [CASTER_ID, TARGET_ID] {
            test_fixture::get_organism_mut(&mut state, member_id).abilities.second =
                AbilityPhase::Active { ends_at: Tick(5) };
        }

        run_timer_expiry_phase(&mut state, Tick(5));

        for member_id in [CASTER_ID, TARGET_ID] {
            assert_eq!(
                get_abilities(&state, member_id).second,
                AbilityPhase::Cooling { ready_at: Tick(91) },
            );
        }
    }

    #[test]
    fn press_first_extends_for_its_duration_once() {
        let mut state: GameState =
            test_fixture::create_state_with_organisms(GameModeKind::FreeForAll, 800, &[WorldPoint { x: 100, y: 100 }]);

        press(&mut state, CASTER_ID, AbilityPressSet::FIRST, None, PRESS_TICK);
        press(&mut state, CASTER_ID, AbilityPressSet::FIRST, None, Tick(11));

        assert_eq!(
            get_abilities(&state, CASTER_ID).first,
            AbilityPhase::Active { ends_at: Tick(74) },
        );
    }

    #[test]
    fn press_second_makes_immortal() {
        let mut state: GameState =
            test_fixture::create_state_with_organisms(GameModeKind::FreeForAll, 800, &[WorldPoint { x: 100, y: 100 }]);

        press(&mut state, CASTER_ID, AbilityPressSet::SECOND, None, PRESS_TICK);

        assert_eq!(
            get_abilities(&state, CASTER_ID).second,
            AbilityPhase::Active { ends_at: Tick(60) },
        );
    }

    #[test]
    fn press_third_centres_the_field_on_the_cursor() {
        let mut state: GameState =
            test_fixture::create_state_with_organisms(GameModeKind::FreeForAll, 800, &[WorldPoint { x: 100, y: 100 }]);
        get_loadout_mut(&mut state, CASTER_ID).third = ThirdAbilityKind::Toxin;
        test_fixture::get_organism_mut(&mut state, CASTER_ID).cursor = WorldPoint { x: 120, y: 90 };

        press(&mut state, CASTER_ID, AbilityPressSet::THIRD, None, PRESS_TICK);

        let abilities: &OrganismAbilities = get_abilities(&state, CASTER_ID);
        assert_eq!(abilities.third, AbilityPhase::Active { ends_at: Tick(67) });
        assert_eq!(abilities.third_center, Some(WorldPoint { x: 120, y: 90 }));
    }

    #[test]
    fn press_fourth_launches_then_secretes_the_spores() {
        let mut state: GameState =
            test_fixture::create_state_with_organisms(GameModeKind::FreeForAll, 800, &[WorldPoint { x: 100, y: 100 }]);
        insert_cells(&mut state, CASTER_ID, &[(1, 0)]);
        let mut expected_organism: Organism = test_fixture::get_organism(&state, CASTER_ID).clone();
        let expected_spores: Vec<Projectile> = projectile::launch_spores(&mut expected_organism);

        press(&mut state, CASTER_ID, AbilityPressSet::FOURTH, None, PRESS_TICK);

        assert_eq!(expected_spores.len(), 1);
        assert_eq!(
            test_fixture::get_organism(&state, CASTER_ID).cells,
            expected_organism.cells
        );
        assert_eq!(
            get_abilities(&state, CASTER_ID).spore,
            SporePhase::Flying {
                ends_at: Tick(34),
                spores: expected_spores.clone(),
            },
        );

        press(&mut state, CASTER_ID, AbilityPressSet::FOURTH, None, Tick(12));

        assert_eq!(
            get_abilities(&state, CASTER_ID).spore,
            SporePhase::Secreting {
                ends_at: Tick(23),
                spores: expected_spores,
            },
        );
    }

    #[test]
    fn run_ability_press_phase_ignores_members_without_an_organism() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        state.members.insert(CASTER_ID, test_fixture::create_participant(CASTER_ID));
        let state_before: GameState = state.clone();

        press(
            &mut state,
            CASTER_ID,
            AbilityPressSet::FIRST.with(AbilityPressSet::FOURTH),
            None,
            PRESS_TICK,
        );

        assert_eq!(state, state_before);
    }
}
