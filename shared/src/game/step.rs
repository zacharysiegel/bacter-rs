use crate::ability::{activation, damage, projectile};
use crate::game::{GameState, InputBundle, MemberEvent, PlayerTickInput, SimulationEvent, Tick};
use crate::member::{Appearance, Member, MemberId, Score};
use crate::organism::Organism;
use crate::organism::{growth, spawn};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepError {
    TickMismatch { expected: Tick, received: Tick },
    MemberIdOutOfOrder { minimum: MemberId, received: MemberId },
}

/// Advances the state by one tick; on `Err` the state is untouched.
pub fn step(state: &mut GameState, bundle: &InputBundle) -> Result<Vec<SimulationEvent>, StepError> {
    check_bundle(state, bundle)?;

    let mut simulation_events: Vec<SimulationEvent> = Vec::new();

    for member_event in &bundle.member_events {
        let simulation_event: Option<SimulationEvent> = apply_member_event(state, member_event);
        simulation_events.extend(simulation_event);
    }

    apply_cursor_updates(state, &bundle.player_inputs);
    activation::run_timer_expiry_phase(state, bundle.tick);

    let effect_events: Vec<SimulationEvent> =
        activation::run_ability_press_phase(state, &bundle.player_inputs, bundle.tick);
    simulation_events.extend(effect_events);

    projectile::run_flight_phase(state);
    growth::run_birth_phase(state, bundle.tick);
    growth::run_natural_death_phase(state);
    damage::run_damage_phase(state);

    let death_events: Vec<SimulationEvent> = record_deaths(state);
    simulation_events.extend(death_events);

    retighten_cell_occupancies(state);
    state.tick = bundle.tick;

    Ok(simulation_events)
}

fn check_bundle(state: &GameState, bundle: &InputBundle) -> Result<(), StepError> {
    let expected_tick: Tick = state.tick.next();

    if bundle.tick != expected_tick {
        return Err(StepError::TickMismatch {
            expected: expected_tick,
            received: bundle.tick,
        });
    }

    let mut next_member_id: MemberId = state.next_member_id;

    for member_event in &bundle.member_events {
        let MemberEvent::Joined { member_id, .. } = member_event else {
            continue;
        };

        if *member_id < next_member_id {
            return Err(StepError::MemberIdOutOfOrder {
                minimum: next_member_id,
                received: *member_id,
            });
        }

        next_member_id = member_id.next();
    }

    Ok(())
}

fn apply_member_event(state: &mut GameState, member_event: &MemberEvent) -> Option<SimulationEvent> {
    match member_event {
        MemberEvent::Joined {
            member_id,
            screen_name,
            role,
            loadout,
            team,
        } => {
            let member: Member = Member {
                member_id: *member_id,
                screen_name: screen_name.clone(),
                role: *role,
                loadout: loadout.map(|loadout| loadout.with_team_color(*team)),
                team: *team,
                score: Score::zero(),
                organism: None,
            };

            Some(add_member(state, member))
        }
        MemberEvent::Left { member_id } => {
            let removed_member: Option<Member> = state.members.remove(member_id);

            removed_member.map(|_| SimulationEvent::MemberLeft { member_id: *member_id })
        }
        MemberEvent::SpawnRequested {
            member_id,
            loadout,
            team,
        } => spawn::spawn_member(state, *member_id, *loadout, *team),
        MemberEvent::AppearanceChanged { member_id, appearance } => {
            change_appearance(state, *member_id, *appearance);

            None
        }
    }
}

fn add_member(state: &mut GameState, member: Member) -> SimulationEvent {
    let member_id: MemberId = member.member_id;
    state.members.insert(member_id, member);
    state.next_member_id = member_id.next();

    SimulationEvent::MemberJoined { member_id }
}

/// A member without a loadout has no appearance to change.
fn change_appearance(state: &mut GameState, member_id: MemberId, appearance: Appearance) {
    let Some(member) = state.members.get_mut(&member_id) else {
        return;
    };

    let Some(loadout) = member.loadout.as_mut() else {
        return;
    };

    loadout.appearance = appearance.with_team_color(member.team);
}

/// Inputs of members without an organism are ignored.
fn apply_cursor_updates(state: &mut GameState, player_inputs: &[PlayerTickInput]) {
    for player_input in player_inputs {
        let organism: Option<&mut Organism> =
            state.members.get_mut(&player_input.member_id).and_then(|member| member.organism.as_mut());
        let Some(organism) = organism else {
            continue;
        };

        organism.cursor = player_input.cursor;
    }
}

/// Kill credit goes to the last hitter unless it is the victim or no longer a member.
fn record_deaths(state: &mut GameState) -> Vec<SimulationEvent> {
    let dead_member_ids: Vec<MemberId> = state
        .members
        .values()
        .filter(|member| member.organism.as_ref().is_some_and(|organism| organism.cells.is_empty()))
        .map(|member| member.member_id)
        .collect();
    let mut death_events: Vec<SimulationEvent> = Vec::new();

    for member_id in dead_member_ids {
        let Some(member) = state.members.get_mut(&member_id) else {
            continue;
        };

        let last_hitter: Option<MemberId> = member.organism.take().and_then(|organism| organism.last_hitter);
        member.score.deaths += 1;

        let credited_to: Option<MemberId> =
            last_hitter.filter(|hitter_id| *hitter_id != member_id && state.members.contains_key(hitter_id));
        let killer: Option<&mut Member> = credited_to.and_then(|killer_id| state.members.get_mut(&killer_id));

        if let Some(killer) = killer {
            killer.score.kills += 1;
        }

        death_events.push(SimulationEvent::OrganismDied { member_id, credited_to });
    }

    death_events
}

fn retighten_cell_occupancies(state: &mut GameState) {
    for member in state.members.values_mut() {
        let Some(organism) = member.organism.as_mut() else {
            continue;
        };

        organism.cells.retighten();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::{AbilityPhase, AbilityPressSet, Projectile, SporePhase};
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::{LatticeCoordinate, SubpixelPoint, SubpixelVector, WorldPoint};
    use crate::member::{MemberRoleKind, OrganismColorKind, SkinKind, TeamKind};
    use crate::organism::CellOccupancy;
    use crate::world::WorldShapeKind;

    fn create_bundle(tick: u32, member_events: Vec<MemberEvent>) -> InputBundle {
        InputBundle {
            tick: Tick(tick),
            member_events,
            player_inputs: Vec::new(),
        }
    }

    fn create_joined_event(member_id: MemberId, team: Option<TeamKind>) -> MemberEvent {
        MemberEvent::Joined {
            member_id,
            screen_name: format!("player {}", member_id.0),
            role: MemberRoleKind::Participant,
            loadout: Some(test_fixture::create_loadout()),
            team,
        }
    }

    fn create_state() -> GameState {
        test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800)
    }

    #[test]
    fn step_rejects_a_tick_mismatch_untouched() {
        let mut state: GameState = create_state();
        let state_before: GameState = state.clone();

        let result: Result<Vec<SimulationEvent>, StepError> = step(
            &mut state,
            &create_bundle(2, vec![create_joined_event(MemberId(0), None)]),
        );

        assert_eq!(
            result,
            Err(StepError::TickMismatch {
                expected: Tick(1),
                received: Tick(2),
            }),
        );
        assert_eq!(state, state_before);
    }

    #[test]
    fn step_rejects_a_member_id_below_the_next_untouched() {
        let mut state: GameState = create_state();
        step(
            &mut state,
            &create_bundle(1, vec![create_joined_event(MemberId(3), None)]),
        )
        .unwrap();

        let state_before: GameState = state.clone();

        let result: Result<Vec<SimulationEvent>, StepError> = step(
            &mut state,
            &create_bundle(
                2,
                vec![
                    create_joined_event(MemberId(5), None),
                    create_joined_event(MemberId(5), None),
                ],
            ),
        );

        assert_eq!(
            result,
            Err(StepError::MemberIdOutOfOrder {
                minimum: MemberId(6),
                received: MemberId(5),
            }),
        );
        assert_eq!(state, state_before);
        assert_eq!(
            step(
                &mut state,
                &create_bundle(2, vec![create_joined_event(MemberId(2), None)]),
            ),
            Err(StepError::MemberIdOutOfOrder {
                minimum: MemberId(4),
                received: MemberId(2),
            }),
        );
    }

    #[test]
    fn step_advances_the_tick() {
        let mut state: GameState = create_state();

        assert_eq!(step(&mut state, &create_bundle(1, Vec::new())), Ok(Vec::new()));
        assert_eq!(state.tick, Tick(1));
    }

    #[test]
    fn step_joined_adds_a_member_without_an_organism() {
        let mut state: GameState = create_state();

        let simulation_events: Vec<SimulationEvent> = step(
            &mut state,
            &create_bundle(1, vec![create_joined_event(MemberId(4), None)]),
        )
        .unwrap();

        assert_eq!(
            simulation_events,
            vec![SimulationEvent::MemberJoined { member_id: MemberId(4) }],
        );
        assert_eq!(state.next_member_id, MemberId(5));
        assert_eq!(state.members[&MemberId(4)].screen_name, "player 4");
        assert_eq!(state.members[&MemberId(4)].organism, None);
        assert_eq!(state.members[&MemberId(4)].score, Score::zero());
    }

    #[test]
    fn step_joined_forces_the_team_color() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Skirmish, WorldShapeKind::Rectangle, 800);

        step(
            &mut state,
            &create_bundle(1, vec![create_joined_event(MemberId(0), Some(TeamKind::Red))]),
        )
        .unwrap();

        assert_eq!(
            state.members[&MemberId(0)].loadout.unwrap().appearance.color,
            OrganismColorKind::Fire,
        );
    }

    #[test]
    fn step_left_removes_the_member() {
        let mut state: GameState = create_state();
        step(
            &mut state,
            &create_bundle(1, vec![create_joined_event(MemberId(0), None)]),
        )
        .unwrap();

        let simulation_events: Vec<SimulationEvent> = step(
            &mut state,
            &create_bundle(
                2,
                vec![
                    MemberEvent::Left { member_id: MemberId(0) },
                    MemberEvent::Left { member_id: MemberId(7) },
                ],
            ),
        )
        .unwrap();

        assert_eq!(
            simulation_events,
            vec![SimulationEvent::MemberLeft { member_id: MemberId(0) }],
        );
        assert!(state.members.is_empty());
        assert_eq!(state.next_member_id, MemberId(1));
    }

    #[test]
    fn step_spawn_requested_follows_its_join_in_bundle_order() {
        let mut state: GameState = create_state();

        let simulation_events: Vec<SimulationEvent> = step(
            &mut state,
            &create_bundle(
                1,
                vec![
                    create_joined_event(MemberId(0), None),
                    MemberEvent::SpawnRequested {
                        member_id: MemberId(0),
                        loadout: test_fixture::create_loadout(),
                        team: None,
                    },
                ],
            ),
        )
        .unwrap();

        let cursor: WorldPoint = test_fixture::get_organism(&state, MemberId(0)).cursor;

        assert_eq!(
            simulation_events,
            vec![
                SimulationEvent::MemberJoined { member_id: MemberId(0) },
                SimulationEvent::OrganismSpawned {
                    member_id: MemberId(0),
                    cursor,
                },
            ],
        );
    }

    #[test]
    fn step_appearance_changed_keeps_the_team_color() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Skirmish, WorldShapeKind::Rectangle, 800);
        step(
            &mut state,
            &create_bundle(1, vec![create_joined_event(MemberId(0), Some(TeamKind::Blue))]),
        )
        .unwrap();

        step(
            &mut state,
            &create_bundle(
                2,
                vec![MemberEvent::AppearanceChanged {
                    member_id: MemberId(0),
                    appearance: Appearance {
                        color: OrganismColorKind::Hot,
                        skin: SkinKind::None,
                    },
                }],
            ),
        )
        .unwrap();

        assert_eq!(
            state.members[&MemberId(0)].loadout.unwrap().appearance,
            Appearance {
                color: OrganismColorKind::Sky,
                skin: SkinKind::None,
            },
        );
    }

    #[test]
    fn step_appearance_changed_updates_a_free_for_all_color() {
        let mut state: GameState = create_state();
        step(
            &mut state,
            &create_bundle(1, vec![create_joined_event(MemberId(0), None)]),
        )
        .unwrap();

        let appearance: Appearance = Appearance {
            color: OrganismColorKind::Royal,
            skin: SkinKind::Ghost,
        };

        step(
            &mut state,
            &create_bundle(
                2,
                vec![MemberEvent::AppearanceChanged {
                    member_id: MemberId(0),
                    appearance,
                }],
            ),
        )
        .unwrap();

        assert_eq!(state.members[&MemberId(0)].loadout.unwrap().appearance, appearance);
    }

    fn create_state_with_organisms(member_ids: &[MemberId]) -> GameState {
        let mut state: GameState = create_state();

        for (index, member_id) in member_ids.iter().enumerate() {
            let x: i32 = 100 + 200 * i32::try_from(index).unwrap();
            state.members.insert(
                *member_id,
                test_fixture::create_participant_with_organism(*member_id, WorldPoint { x, y: 100 }),
            );
            state.next_member_id = member_id.next();
        }

        state
    }

    fn freeze(state: &mut GameState, member_id: MemberId) {
        test_fixture::get_organism_mut(state, member_id).abilities.frozen_until = Some(Tick(u32::MAX));
    }

    fn kill(state: &mut GameState, member_id: MemberId, last_hitter: Option<MemberId>) {
        let organism: &mut Organism = test_fixture::get_organism_mut(state, member_id);
        organism.cells = CellOccupancy::empty();
        organism.last_hitter = last_hitter;
    }

    #[test]
    fn step_moves_cursors_of_alive_organisms() {
        let mut state: GameState = create_state_with_organisms(&[MemberId(0)]);
        state.members.insert(MemberId(1), test_fixture::create_participant(MemberId(1)));
        let mut bundle: InputBundle = create_bundle(1, Vec::new());
        bundle.player_inputs = [MemberId(0), MemberId(1)]
            .iter()
            .map(|member_id| PlayerTickInput {
                member_id: *member_id,
                cursor: WorldPoint { x: 140, y: 90 },
                ability_presses: AbilityPressSet::NONE,
                aim: None,
            })
            .collect();

        step(&mut state, &bundle).unwrap();

        assert_eq!(
            test_fixture::get_organism(&state, MemberId(0)).cursor,
            WorldPoint { x: 140, y: 90 },
        );
        assert_eq!(state.members[&MemberId(1)].organism, None);
    }

    #[test]
    fn step_runs_births_then_natural_deaths() {
        let mut state: GameState = create_state_with_organisms(&[MemberId(0), MemberId(1)]);
        let mut expected_state: GameState = state.clone();
        growth::run_birth_phase(&mut expected_state, Tick(1));
        growth::run_natural_death_phase(&mut expected_state);

        for member_id in [MemberId(0), MemberId(1)] {
            test_fixture::get_organism_mut(&mut expected_state, member_id).cells.retighten();
        }

        step(&mut state, &create_bundle(1, Vec::new())).unwrap();

        assert_eq!(state.rng, expected_state.rng);
        assert_eq!(state.members, expected_state.members);
    }

    #[test]
    fn step_records_a_death_with_kill_credit() {
        let mut state: GameState = create_state_with_organisms(&[MemberId(0), MemberId(1)]);
        kill(&mut state, MemberId(0), Some(MemberId(1)));

        let simulation_events: Vec<SimulationEvent> = step(&mut state, &create_bundle(1, Vec::new())).unwrap();

        assert_eq!(
            simulation_events,
            vec![SimulationEvent::OrganismDied {
                member_id: MemberId(0),
                credited_to: Some(MemberId(1)),
            }],
        );
        assert_eq!(state.members[&MemberId(0)].organism, None);
        assert_eq!(state.members[&MemberId(0)].score.deaths, 1);
        assert_eq!(state.members[&MemberId(1)].score.kills, 1);
    }

    #[test]
    fn step_gives_no_credit_for_a_suicide() {
        let mut state: GameState = create_state_with_organisms(&[MemberId(0)]);
        kill(&mut state, MemberId(0), Some(MemberId(0)));

        let simulation_events: Vec<SimulationEvent> = step(&mut state, &create_bundle(1, Vec::new())).unwrap();

        assert_eq!(
            simulation_events,
            vec![SimulationEvent::OrganismDied {
                member_id: MemberId(0),
                credited_to: None,
            }],
        );
        assert_eq!(state.members[&MemberId(0)].score.kills, 0);
        assert_eq!(state.members[&MemberId(0)].score.deaths, 1);
    }

    #[test]
    fn step_gives_no_credit_to_a_departed_hitter() {
        let mut state: GameState = create_state_with_organisms(&[MemberId(0), MemberId(1)]);
        kill(&mut state, MemberId(0), Some(MemberId(1)));

        let simulation_events: Vec<SimulationEvent> = step(
            &mut state,
            &create_bundle(1, vec![MemberEvent::Left { member_id: MemberId(1) }]),
        )
        .unwrap();

        assert_eq!(
            simulation_events,
            vec![
                SimulationEvent::MemberLeft { member_id: MemberId(1) },
                SimulationEvent::OrganismDied {
                    member_id: MemberId(0),
                    credited_to: None,
                },
            ],
        );
    }

    #[test]
    fn step_retightens_cell_occupancies() {
        let mut state: GameState = create_state_with_organisms(&[MemberId(0)]);
        freeze(&mut state, MemberId(0));
        let organism: &mut Organism = test_fixture::get_organism_mut(&mut state, MemberId(0));
        organism.cells.insert(LatticeCoordinate { i: 5, j: 5 });
        organism.cells.remove(LatticeCoordinate { i: 5, j: 5 });

        step(&mut state, &create_bundle(1, Vec::new())).unwrap();

        assert_eq!(
            test_fixture::get_organism(&state, MemberId(0)).cells,
            CellOccupancy::with_cell(LatticeCoordinate { i: 0, j: 0 }),
        );
    }

    fn create_press_input(member_id: MemberId, ability_presses: AbilityPressSet) -> PlayerTickInput {
        PlayerTickInput {
            member_id,
            cursor: WorldPoint { x: 100, y: 100 },
            ability_presses,
            aim: None,
        }
    }

    #[test]
    fn step_expires_a_cooldown_before_the_press_of_the_same_tick() {
        let mut state: GameState = create_state_with_organisms(&[MemberId(0)]);
        freeze(&mut state, MemberId(0));
        test_fixture::get_organism_mut(&mut state, MemberId(0)).abilities.first =
            AbilityPhase::Cooling { ready_at: Tick(1) };
        let mut bundle: InputBundle = create_bundle(1, Vec::new());
        bundle.player_inputs = vec![create_press_input(MemberId(0), AbilityPressSet::FIRST)];

        step(&mut state, &bundle).unwrap();

        assert_eq!(
            test_fixture::get_organism(&state, MemberId(0)).abilities.first,
            AbilityPhase::Active { ends_at: Tick(65) },
        );
    }

    #[test]
    fn step_moves_spores_in_the_tick_they_launch() {
        let mut state: GameState = create_state_with_organisms(&[MemberId(0)]);
        freeze(&mut state, MemberId(0));
        let organism: &mut Organism = test_fixture::get_organism_mut(&mut state, MemberId(0));

        for j in -1..=1 {
            for i in -1..=1 {
                organism.cells.insert(LatticeCoordinate { i, j });
            }
        }

        let mut bundle: InputBundle = create_bundle(1, Vec::new());
        bundle.player_inputs = vec![create_press_input(MemberId(0), AbilityPressSet::FOURTH)];

        step(&mut state, &bundle).unwrap();

        let expected_first_spore: Projectile = Projectile {
            position: SubpixelPoint {
                x: 96_256 - 7603,
                y: 96_256 - 7603,
            },
            velocity: SubpixelVector { x: -7603, y: -7603 },
        };

        assert!(matches!(
            &test_fixture::get_organism(&state, MemberId(0)).abilities.spore,
            SporePhase::Flying { ends_at: Tick(25), spores } if spores.len() == 8 && spores[0] == expected_first_spore,
        ));
    }

    #[test]
    fn step_damages_before_recording_deaths() {
        let mut state: GameState = create_state_with_organisms(&[MemberId(0), MemberId(1)]);
        freeze(&mut state, MemberId(0));
        freeze(&mut state, MemberId(1));
        test_fixture::get_organism_mut(&mut state, MemberId(0)).abilities.spore = SporePhase::Secreting {
            ends_at: Tick(5),
            spores: vec![Projectile {
                position: WorldPoint { x: 300, y: 100 }.to_subpixel_point(),
                velocity: SubpixelVector { x: 0, y: 0 },
            }],
        };

        let simulation_events: Vec<SimulationEvent> = step(&mut state, &create_bundle(1, Vec::new())).unwrap();

        assert_eq!(
            simulation_events,
            vec![SimulationEvent::OrganismDied {
                member_id: MemberId(1),
                credited_to: Some(MemberId(0)),
            }],
        );
        assert_eq!(state.members[&MemberId(0)].score.kills, 1);
    }
}
