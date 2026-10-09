use crate::game::{GameState, InputBundle, MemberEvent, SimulationEvent, Tick};
use crate::member::{Appearance, Member, MemberId, Score};
use crate::organism::spawn;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::WorldPoint;
    use crate::member::{MemberRoleKind, OrganismColorKind, SkinKind, TeamKind};
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
}
