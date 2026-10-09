use crate::ability;
use crate::ability::ability_constants;
use crate::ability::projectile;
use crate::ability::{
    AbilityActivation, AbilityPhase, AbilityPressSet, FirstAbilityKind, Loadout, OrganismAbilities, Projectile,
    SecondAbilityKind, ShotEffectKind, ShotPhase, SporePhase,
};
use crate::game::{GameState, PlayerTickInput, SimulationEvent, Tick};
use crate::geometry::{SubpixelPoint, WorldPoint};
use crate::member;
use crate::member::{Member, MemberId, TeamKind};
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

    match loadout.first {
        FirstAbilityKind::Extend => {
            organism.abilities.first.activate(tick, loadout.first.active_ticks());

            Vec::new()
        }
        FirstAbilityKind::Compress if organism.abilities.first.is_ready() => {
            press_shot_slot(state, player_input, ShotEffectKind::Compress, tick)
        }
        FirstAbilityKind::Compress => Vec::new(),
    }
}

fn press_second(state: &mut GameState, player_input: &PlayerTickInput, tick: Tick) -> Vec<SimulationEvent> {
    let Some((loadout, organism)) = get_loadout_and_organism(state, player_input.member_id) else {
        return Vec::new();
    };

    match loadout.second {
        SecondAbilityKind::Immortality => {
            organism.abilities.second.activate(tick, loadout.second.active_ticks());

            Vec::new()
        }
        SecondAbilityKind::Freeze if organism.abilities.second.is_ready() => {
            press_shot_slot(state, player_input, ShotEffectKind::Freeze, tick)
        }
        SecondAbilityKind::Freeze => Vec::new(),
    }
}

fn press_third(state: &mut GameState, member_id: MemberId, tick: Tick) {
    let Some((loadout, organism)) = get_loadout_and_organism(state, member_id) else {
        return;
    };

    let activation: AbilityActivation = organism.abilities.third.activate(tick, loadout.third.active_ticks());

    match activation {
        AbilityActivation::Started => organism.abilities.third_center = Some(organism.cursor),
        AbilityActivation::NotReady => {}
    }
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

fn press_shot_slot(
    state: &mut GameState,
    player_input: &PlayerTickInput,
    effect: ShotEffectKind,
    tick: Tick,
) -> Vec<SimulationEvent> {
    let slot_index: usize = effect.slot_index();

    let Some((_, organism)) = get_loadout_and_organism(state, player_input.member_id) else {
        return Vec::new();
    };

    match organism.abilities.shots[slot_index] {
        ShotPhase::Ready => {
            let shot: Option<Projectile> = player_input.aim.and_then(|aim| projectile::launch_shot(organism, aim));

            if let Some(shot) = shot {
                organism.abilities.shots[slot_index] = ShotPhase::Flying {
                    ends_at: tick.plus(ability_constants::SHOT_FLIGHT_TICKS),
                    shot,
                };
            }

            Vec::new()
        }
        ShotPhase::Flying { shot, .. } => {
            organism.abilities.shots[slot_index] = ShotPhase::Secreting {
                ends_at: tick.plus(ability_constants::SHOT_SECRETION_TICKS),
                center: shot.position,
            };

            apply_shot_effect(state, player_input.member_id, effect, shot.position, tick)
        }
        ShotPhase::Secreting { .. } | ShotPhase::Cooling { .. } => Vec::new(),
    }
}

/// Only a hit starts the caster's carried ability timer.
fn apply_shot_effect(
    state: &mut GameState,
    caster_id: MemberId,
    effect: ShotEffectKind,
    center: SubpixelPoint,
    tick: Tick,
) -> Vec<SimulationEvent> {
    let caster_team: Option<TeamKind> = state.members.get(&caster_id).and_then(|caster| caster.team);
    let target_ids: Vec<MemberId> = state
        .members
        .values()
        .filter(|member| is_shot_target(member, caster_id, caster_team, center))
        .map(|member| member.member_id)
        .collect();
    let effect_until: Option<Tick> = Some(tick.plus(effect.effect_ticks()));
    let mut simulation_events: Vec<SimulationEvent> = Vec::new();

    for target_id in &target_ids {
        let Some((_, target_organism)) = get_loadout_and_organism(state, *target_id) else {
            continue;
        };

        match effect {
            ShotEffectKind::Compress => target_organism.abilities.compressed_until = effect_until,
            ShotEffectKind::Freeze => target_organism.abilities.frozen_until = effect_until,
        }

        simulation_events.push(SimulationEvent::EffectApplied {
            target: *target_id,
            caster: caster_id,
            kind: effect,
        });
    }

    if !target_ids.is_empty() {
        start_carried_ability_timer(state, caster_id, effect, tick);
    }

    simulation_events
}

/// Only the target's own neutralize field protects it from a shot.
fn is_shot_target(member: &Member, caster_id: MemberId, caster_team: Option<TeamKind>, center: SubpixelPoint) -> bool {
    if member.member_id == caster_id || member::is_same_team(member.team, caster_team) {
        return false;
    }

    let (Some(loadout), Some(organism)) = (member.loadout.as_ref(), member.organism.as_ref()) else {
        return false;
    };

    let neutralize_field_center: Option<WorldPoint> = organism.abilities.get_neutralize_field_center(loadout);

    organism.cells.iter().any(|lattice_coordinate| {
        let cell_center: WorldPoint = organism.cell_center(lattice_coordinate);
        let is_neutralized: bool =
            neutralize_field_center.is_some_and(|field_center| ability::is_inside_field(field_center, cell_center));

        !is_neutralized && ability::is_inside_shot_secretion(center, cell_center.to_subpixel_point())
    })
}

fn start_carried_ability_timer(state: &mut GameState, caster_id: MemberId, effect: ShotEffectKind, tick: Tick) {
    let Some((loadout, caster_organism)) = get_loadout_and_organism(state, caster_id) else {
        return;
    };

    match effect {
        ShotEffectKind::Compress => {
            caster_organism.abilities.first = AbilityPhase::Active {
                ends_at: tick.plus(loadout.first.active_ticks()),
            };
        }
        ShotEffectKind::Freeze => {
            caster_organism.abilities.second = AbilityPhase::Active {
                ends_at: tick.plus(loadout.second.active_ticks()),
            };
        }
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
    use crate::ability::{AimVector, ThirdAbilityKind};
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::{LatticeCoordinate, SubpixelVector};
    use crate::world::WorldShapeKind;

    const CASTER_ID: MemberId = MemberId(0);
    const TARGET_ID: MemberId = MemberId(1);
    const OTHER_TARGET_ID: MemberId = MemberId(2);
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

    fn set_flying_shot(state: &mut GameState, member_id: MemberId, effect: ShotEffectKind, position: WorldPoint) {
        test_fixture::get_organism_mut(state, member_id).abilities.shots[effect.slot_index()] = ShotPhase::Flying {
            ends_at: Tick(30),
            shot: Projectile {
                position: position.to_subpixel_point(),
                velocity: SubpixelVector { x: 0, y: 0 },
            },
        };
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
    fn press_third_keeps_the_active_field_centre() {
        let mut state: GameState =
            test_fixture::create_state_with_organisms(GameModeKind::FreeForAll, 800, &[WorldPoint { x: 100, y: 100 }]);
        get_loadout_mut(&mut state, CASTER_ID).third = ThirdAbilityKind::Toxin;
        test_fixture::get_organism_mut(&mut state, CASTER_ID).cursor = WorldPoint { x: 120, y: 90 };
        press(&mut state, CASTER_ID, AbilityPressSet::THIRD, None, PRESS_TICK);
        test_fixture::get_organism_mut(&mut state, CASTER_ID).cursor = WorldPoint { x: 80, y: 110 };

        press(&mut state, CASTER_ID, AbilityPressSet::THIRD, None, Tick(11));

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
            expected_organism.cells,
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
    fn press_fourth_ignores_a_press_while_secreting_or_cooling() {
        let projectile: Projectile = Projectile {
            position: SubpixelPoint { x: 0, y: 0 },
            velocity: SubpixelVector { x: 1, y: 0 },
        };
        let ignored_spore_phases: [SporePhase; 2] = [
            SporePhase::Secreting {
                ends_at: Tick(20),
                spores: vec![projectile],
            },
            SporePhase::Cooling { ready_at: Tick(20) },
        ];

        for ignored_spore_phase in ignored_spore_phases {
            let mut state: GameState = test_fixture::create_state_with_organisms(
                GameModeKind::FreeForAll,
                800,
                &[WorldPoint { x: 100, y: 100 }],
            );
            insert_cells(&mut state, CASTER_ID, &[(1, 0)]);
            test_fixture::get_organism_mut(&mut state, CASTER_ID).abilities.spore = ignored_spore_phase;
            let organism_before: Organism = test_fixture::get_organism(&state, CASTER_ID).clone();

            press(&mut state, CASTER_ID, AbilityPressSet::FOURTH, None, PRESS_TICK);

            assert_eq!(test_fixture::get_organism(&state, CASTER_ID), &organism_before);
        }
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

    #[test]
    fn run_ability_press_phase_applies_inputs_in_ascending_member_id() {
        let mut state: GameState = test_fixture::create_state_with_organisms(
            GameModeKind::FreeForAll,
            800,
            &[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 130, y: 100 }],
        );
        get_loadout_mut(&mut state, CASTER_ID).first = FirstAbilityKind::Compress;
        get_loadout_mut(&mut state, TARGET_ID).first = FirstAbilityKind::Compress;
        set_flying_shot(
            &mut state,
            CASTER_ID,
            ShotEffectKind::Compress,
            WorldPoint { x: 130, y: 100 },
        );
        set_flying_shot(
            &mut state,
            TARGET_ID,
            ShotEffectKind::Compress,
            WorldPoint { x: 100, y: 100 },
        );

        let simulation_events: Vec<SimulationEvent> = run_ability_press_phase(
            &mut state,
            &[
                create_input(TARGET_ID, AbilityPressSet::FIRST, None),
                create_input(CASTER_ID, AbilityPressSet::FIRST, None),
            ],
            PRESS_TICK,
        );

        assert_eq!(
            simulation_events,
            vec![
                SimulationEvent::EffectApplied {
                    target: TARGET_ID,
                    caster: CASTER_ID,
                    kind: ShotEffectKind::Compress,
                },
                SimulationEvent::EffectApplied {
                    target: CASTER_ID,
                    caster: TARGET_ID,
                    kind: ShotEffectKind::Compress,
                },
            ],
        );
    }

    #[test]
    fn press_shot_slot_launches_along_a_non_zero_aim() {
        let mut state: GameState =
            test_fixture::create_state_with_organisms(GameModeKind::FreeForAll, 800, &[WorldPoint { x: 100, y: 100 }]);
        get_loadout_mut(&mut state, CASTER_ID).first = FirstAbilityKind::Compress;
        insert_cells(&mut state, CASTER_ID, &[(1, 0)]);

        press(
            &mut state,
            CASTER_ID,
            AbilityPressSet::FIRST,
            Some(AimVector { x: 0, y: 0 }),
            PRESS_TICK,
        );
        press(&mut state, CASTER_ID, AbilityPressSet::FIRST, None, PRESS_TICK);

        assert_eq!(get_abilities(&state, CASTER_ID).shots[0], ShotPhase::Ready);

        press(
            &mut state,
            CASTER_ID,
            AbilityPressSet::FIRST,
            Some(AimVector { x: 5, y: 0 }),
            PRESS_TICK,
        );

        let abilities: &OrganismAbilities = get_abilities(&state, CASTER_ID);
        assert_eq!(
            abilities.shots[0],
            ShotPhase::Flying {
                ends_at: Tick(31),
                shot: Projectile {
                    position: WorldPoint { x: 106, y: 100 }.to_subpixel_point(),
                    velocity: SubpixelVector { x: 8960, y: 0 },
                },
            },
        );
        assert_eq!(abilities.first, AbilityPhase::Ready);
    }

    #[test]
    fn press_shot_slot_pops_and_compresses_each_target_once() {
        let mut state: GameState = test_fixture::create_state_with_organisms(
            GameModeKind::FreeForAll,
            800,
            &[
                WorldPoint { x: 100, y: 100 },
                WorldPoint { x: 300, y: 300 },
                WorldPoint { x: 308, y: 300 },
            ],
        );
        get_loadout_mut(&mut state, CASTER_ID).first = FirstAbilityKind::Compress;
        insert_cells(&mut state, TARGET_ID, &[(1, 0)]);
        set_flying_shot(
            &mut state,
            CASTER_ID,
            ShotEffectKind::Compress,
            WorldPoint { x: 304, y: 300 },
        );

        let simulation_events: Vec<SimulationEvent> =
            press(&mut state, CASTER_ID, AbilityPressSet::FIRST, None, PRESS_TICK);

        assert_eq!(
            simulation_events,
            vec![
                SimulationEvent::EffectApplied {
                    target: TARGET_ID,
                    caster: CASTER_ID,
                    kind: ShotEffectKind::Compress,
                },
                SimulationEvent::EffectApplied {
                    target: OTHER_TARGET_ID,
                    caster: CASTER_ID,
                    kind: ShotEffectKind::Compress,
                },
            ],
        );
        assert_eq!(get_abilities(&state, TARGET_ID).compressed_until, Some(Tick(60)));
        assert_eq!(get_abilities(&state, OTHER_TARGET_ID).compressed_until, Some(Tick(60)));
        assert_eq!(
            get_abilities(&state, CASTER_ID).first,
            AbilityPhase::Active { ends_at: Tick(60) },
        );
        assert_eq!(
            get_abilities(&state, CASTER_ID).shots[0],
            ShotPhase::Secreting {
                ends_at: Tick(21),
                center: WorldPoint { x: 304, y: 300 }.to_subpixel_point(),
            },
        );
    }

    #[test]
    fn press_shot_slot_freezes_from_slot_one() {
        let mut state: GameState = test_fixture::create_state_with_organisms(
            GameModeKind::FreeForAll,
            800,
            &[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 300, y: 300 }],
        );
        get_loadout_mut(&mut state, CASTER_ID).second = SecondAbilityKind::Freeze;
        set_flying_shot(
            &mut state,
            CASTER_ID,
            ShotEffectKind::Freeze,
            WorldPoint { x: 300, y: 312 },
        );

        press(&mut state, CASTER_ID, AbilityPressSet::SECOND, None, PRESS_TICK);

        assert_eq!(get_abilities(&state, TARGET_ID).frozen_until, Some(Tick(67)));
        assert_eq!(
            get_abilities(&state, CASTER_ID).second,
            AbilityPhase::Active { ends_at: Tick(67) },
        );
    }

    #[test]
    fn press_shot_slot_restarts_the_compression_of_a_target() {
        let mut state: GameState = test_fixture::create_state_with_organisms(
            GameModeKind::FreeForAll,
            800,
            &[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 300, y: 300 }],
        );
        get_loadout_mut(&mut state, CASTER_ID).first = FirstAbilityKind::Compress;
        test_fixture::get_organism_mut(&mut state, TARGET_ID).abilities.compressed_until = Some(Tick(12));
        set_flying_shot(
            &mut state,
            CASTER_ID,
            ShotEffectKind::Compress,
            WorldPoint { x: 300, y: 300 },
        );

        press(&mut state, CASTER_ID, AbilityPressSet::FIRST, None, PRESS_TICK);

        assert_eq!(get_abilities(&state, TARGET_ID).compressed_until, Some(Tick(60)));
    }

    #[test]
    fn press_shot_slot_miss_keeps_the_carried_ability_ready() {
        let mut state: GameState = test_fixture::create_state_with_organisms(
            GameModeKind::FreeForAll,
            800,
            &[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 300, y: 300 }],
        );
        get_loadout_mut(&mut state, CASTER_ID).first = FirstAbilityKind::Compress;
        set_flying_shot(
            &mut state,
            CASTER_ID,
            ShotEffectKind::Compress,
            WorldPoint { x: 300, y: 313 },
        );

        let simulation_events: Vec<SimulationEvent> =
            press(&mut state, CASTER_ID, AbilityPressSet::FIRST, None, PRESS_TICK);

        assert_eq!(simulation_events, Vec::new());
        assert_eq!(get_abilities(&state, TARGET_ID).compressed_until, None);
        assert_eq!(get_abilities(&state, CASTER_ID).first, AbilityPhase::Ready);
        assert!(matches!(
            get_abilities(&state, CASTER_ID).shots[0],
            ShotPhase::Secreting { .. },
        ));
    }

    #[test]
    fn press_shot_slot_spares_teammates_and_the_caster() {
        let mut state: GameState = test_fixture::create_state_with_organisms(
            GameModeKind::Skirmish,
            800,
            &[WorldPoint { x: 300, y: 294 }, WorldPoint { x: 300, y: 300 }],
        );

        for member in state.members.values_mut() {
            member.team = Some(TeamKind::Blue);
        }

        get_loadout_mut(&mut state, CASTER_ID).first = FirstAbilityKind::Compress;
        set_flying_shot(
            &mut state,
            CASTER_ID,
            ShotEffectKind::Compress,
            WorldPoint { x: 300, y: 297 },
        );

        let simulation_events: Vec<SimulationEvent> =
            press(&mut state, CASTER_ID, AbilityPressSet::FIRST, None, PRESS_TICK);

        assert_eq!(simulation_events, Vec::new());
        assert_eq!(get_abilities(&state, CASTER_ID).compressed_until, None);
        assert_eq!(get_abilities(&state, TARGET_ID).compressed_until, None);
    }

    #[test]
    fn press_shot_slot_spares_cells_inside_the_targets_own_neutralize_field() {
        let mut state: GameState = test_fixture::create_state_with_organisms(
            GameModeKind::FreeForAll,
            800,
            &[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 300, y: 300 }],
        );
        get_loadout_mut(&mut state, CASTER_ID).first = FirstAbilityKind::Compress;
        let target_abilities: &mut OrganismAbilities =
            &mut test_fixture::get_organism_mut(&mut state, TARGET_ID).abilities;
        target_abilities.third = AbilityPhase::Active { ends_at: Tick(40) };
        target_abilities.third_center = Some(WorldPoint { x: 300, y: 340 });
        set_flying_shot(
            &mut state,
            CASTER_ID,
            ShotEffectKind::Compress,
            WorldPoint { x: 300, y: 300 },
        );

        let simulation_events: Vec<SimulationEvent> =
            press(&mut state, CASTER_ID, AbilityPressSet::FIRST, None, PRESS_TICK);

        assert_eq!(simulation_events, Vec::new());
    }

    #[test]
    fn press_first_does_not_reach_the_shot_while_a_compress_casters_timer_runs() {
        let mut state: GameState = test_fixture::create_state_with_organisms(
            GameModeKind::FreeForAll,
            800,
            &[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 300, y: 300 }],
        );
        get_loadout_mut(&mut state, CASTER_ID).first = FirstAbilityKind::Compress;
        test_fixture::get_organism_mut(&mut state, CASTER_ID).abilities.first =
            AbilityPhase::Active { ends_at: Tick(40) };
        set_flying_shot(
            &mut state,
            CASTER_ID,
            ShotEffectKind::Compress,
            WorldPoint { x: 300, y: 300 },
        );

        press(&mut state, CASTER_ID, AbilityPressSet::FIRST, None, PRESS_TICK);

        assert!(matches!(
            get_abilities(&state, CASTER_ID).shots[0],
            ShotPhase::Flying { .. }
        ));
        assert_eq!(get_abilities(&state, TARGET_ID).compressed_until, None);
    }

    #[test]
    fn press_shot_slot_ignores_a_secreting_slot() {
        let mut state: GameState =
            test_fixture::create_state_with_organisms(GameModeKind::FreeForAll, 800, &[WorldPoint { x: 100, y: 100 }]);
        get_loadout_mut(&mut state, CASTER_ID).first = FirstAbilityKind::Compress;
        let secreting_phase: ShotPhase = ShotPhase::Secreting {
            ends_at: Tick(15),
            center: SubpixelPoint { x: 0, y: 0 },
        };
        test_fixture::get_organism_mut(&mut state, CASTER_ID).abilities.shots[0] = secreting_phase;

        press(
            &mut state,
            CASTER_ID,
            AbilityPressSet::FIRST,
            Some(AimVector { x: 1, y: 1 }),
            PRESS_TICK,
        );

        assert_eq!(get_abilities(&state, CASTER_ID).shots[0], secreting_phase);
    }
}
