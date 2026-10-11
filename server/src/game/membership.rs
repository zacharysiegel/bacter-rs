use shared::ability::Loadout;
use shared::game::{GameState, MemberEvent};
use shared::member::team_assignment;
use shared::member::{
    Joiner, Member, MemberId, MemberRoleKind, TeamChoiceKind, TeamChoiceRejectionKind, TeamKind, TeamSizes,
};
use shared::protocol::{RejectionKind, protocol_limits};
use shared::random::Pcg32;
use shared::round::RoundPhase;

use crate::game::PendingAdmissions;
use crate::game::pending_admissions;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinAdmission {
    pub member_id: MemberId,
    pub member_events: Vec<MemberEvent>,
}

/// A survival join while a round is in progress admits the joiner without spawning it.
/// `team_assignment_rng` is the server's own, never the game's.
pub fn admit_join(
    state: &GameState,
    pending_admissions: &mut PendingAdmissions,
    team_assignment_rng: &mut Pcg32,
    joiner: &Joiner,
) -> Result<JoinAdmission, RejectionKind> {
    check_screen_name(state, pending_admissions, &joiner.screen_name)?;
    check_member_limit(state, pending_admissions)?;

    let member_id: MemberId = pending_admissions.get_next_member_id(state);
    let spawns: bool = !is_round_in_progress(state);

    if spawns {
        check_spawn_slot(state, pending_admissions)?;
    }

    let team: Option<TeamKind> = resolve_team(state, pending_admissions, team_assignment_rng, joiner.team, None)?;
    let mut member_events: Vec<MemberEvent> = vec![MemberEvent::Joined {
        member_id,
        screen_name: joiner.screen_name.clone(),
        role: MemberRoleKind::Participant,
        loadout: Some(joiner.loadout),
        team,
    }];

    pending_admissions.record_member(&joiner.screen_name, MemberRoleKind::Participant);

    if spawns {
        member_events.push(MemberEvent::SpawnRequested {
            member_id,
            loadout: joiner.loadout,
            team,
        });
        pending_admissions.record_spawn(member_id, team);
    }

    Ok(JoinAdmission {
        member_id,
        member_events,
    })
}

pub fn admit_spectate(
    state: &GameState,
    pending_admissions: &mut PendingAdmissions,
    screen_name: &str,
) -> Result<JoinAdmission, RejectionKind> {
    check_screen_name(state, pending_admissions, screen_name)?;

    if pending_admissions.get_spectator_count(state) >= protocol_limits::SPECTATOR_LIMIT_PER_GAME {
        return Err(RejectionKind::SpectatorLimitReached);
    }

    check_member_limit(state, pending_admissions)?;

    let member_id: MemberId = pending_admissions.get_next_member_id(state);

    pending_admissions.record_member(screen_name, MemberRoleKind::Spectator);

    Ok(JoinAdmission {
        member_id,
        member_events: vec![MemberEvent::Joined {
            member_id,
            screen_name: String::from(screen_name),
            role: MemberRoleKind::Spectator,
            loadout: None,
            team: None,
        }],
    })
}

/// `team_assignment_rng` is the server's own, never the game's.
pub fn admit_respawn(
    state: &GameState,
    pending_admissions: &mut PendingAdmissions,
    team_assignment_rng: &mut Pcg32,
    member_id: MemberId,
    loadout: Loadout,
    team_choice: TeamChoiceKind,
) -> Result<MemberEvent, RejectionKind> {
    let member: &Member = get_respawning_member(state, pending_admissions, member_id)?;

    check_spawn_slot(state, pending_admissions)?;

    if is_round_in_progress(state) {
        return Err(RejectionKind::RoundInProgress);
    }

    let team: Option<TeamKind> = resolve_team(
        state,
        pending_admissions,
        team_assignment_rng,
        team_choice,
        Some(member),
    )?;

    pending_admissions.record_spawn(member_id, team);

    Ok(MemberEvent::SpawnRequested {
        member_id,
        loadout,
        team,
    })
}

fn check_screen_name(
    state: &GameState,
    pending_admissions: &PendingAdmissions,
    screen_name: &str,
) -> Result<(), RejectionKind> {
    if pending_admissions.is_screen_name_taken(state, screen_name) {
        return Err(RejectionKind::ScreenNameTaken);
    }

    Ok(())
}

fn check_member_limit(state: &GameState, pending_admissions: &PendingAdmissions) -> Result<(), RejectionKind> {
    let member_limit: u32 = protocol_limits::get_member_limit(state.settings.player_cap);

    if pending_admissions.get_member_count(state) >= member_limit {
        return Err(RejectionKind::GameFull);
    }

    Ok(())
}

fn check_spawn_slot(state: &GameState, pending_admissions: &PendingAdmissions) -> Result<(), RejectionKind> {
    if pending_admissions.get_spawning_count(state) >= u32::from(state.settings.player_cap) {
        return Err(RejectionKind::GameFull);
    }

    Ok(())
}

/// A member whose join is still pending has not died either.
fn get_respawning_member<'state>(
    state: &'state GameState,
    pending_admissions: &PendingAdmissions,
    member_id: MemberId,
) -> Result<&'state Member, RejectionKind> {
    let Some(member): Option<&Member> = state.members.get(&member_id) else {
        let is_pending_join: bool = member_id >= state.next_member_id;
        let rejection: RejectionKind = if is_pending_join {
            RejectionKind::NotDead
        } else {
            RejectionKind::NotInGame
        };

        return Err(rejection);
    };

    if member.organism.is_some() || pending_admissions.is_spawn_pending(member_id) {
        return Err(RejectionKind::NotDead);
    }

    Ok(member)
}

fn is_round_in_progress(state: &GameState) -> bool {
    state.round.is_some_and(|round| matches!(round.phase, RoundPhase::Playing | RoundPhase::PostRound))
}

/// `None` outside skirmish; `Auto` chooses uniformly among the smallest teams.
fn resolve_team(
    state: &GameState,
    pending_admissions: &PendingAdmissions,
    team_assignment_rng: &mut Pcg32,
    team_choice: TeamChoiceKind,
    respawning_member: Option<&Member>,
) -> Result<Option<TeamKind>, RejectionKind> {
    let Some(team_count): Option<u8> = state.settings.team_count else {
        return Ok(None);
    };

    let current_team: Option<TeamKind> = respawning_member.and_then(|member| member.team);
    let excluded_member_id: Option<MemberId> = respawning_member.map(|member| member.member_id);
    let team_sizes: TeamSizes = pending_admissions.get_team_sizes(state, team_count, excluded_member_id);

    let TeamChoiceKind::Team(requested_team) = team_choice else {
        return Ok(choose_auto_team(&team_sizes, team_assignment_rng));
    };

    let team_choice_check: Result<(), TeamChoiceRejectionKind> =
        team_assignment::check_team_choice(&team_sizes, current_team, requested_team);

    match team_choice_check {
        Ok(()) => Ok(Some(requested_team)),
        Err(TeamChoiceRejectionKind::TeamUnbalanced { smaller_team }) => Err(RejectionKind::TeamUnbalanced {
            requested: requested_team,
            smaller: smaller_team,
        }),
        Err(TeamChoiceRejectionKind::TeamNotInGame) => Err(RejectionKind::TeamUnbalanced {
            requested: requested_team,
            smaller: team_sizes.get_smallest_teams().first().copied().unwrap_or(requested_team),
        }),
    }
}

fn choose_auto_team(team_sizes: &TeamSizes, team_assignment_rng: &mut Pcg32) -> Option<TeamKind> {
    let smallest_teams: Vec<TeamKind> = team_sizes.get_smallest_teams();
    let smallest_team_count: u32 = pending_admissions::saturating_count(smallest_teams.len());
    let chosen_index: u32 = team_assignment_rng.below(smallest_team_count);
    let chosen_team_index: usize = usize::try_from(chosen_index).ok()?;

    smallest_teams.get(chosen_team_index).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use shared::game::{GameModeKind, Tick};
    use shared::round::RoundState;

    fn create_team_assignment_rng() -> Pcg32 {
        Pcg32::from_seed(test_fixture::FIXTURE_SEED)
    }

    fn add_participant(state: &mut GameState, screen_name: &str, team: Option<TeamKind>, is_alive: bool) -> MemberId {
        let member_id: MemberId = state.next_member_id;
        let member: Member =
            test_fixture::create_member(member_id, screen_name, MemberRoleKind::Participant, team, is_alive);

        state.members.insert(member_id, member);
        state.next_member_id = member_id.next();

        member_id
    }

    fn add_spectator(state: &mut GameState, screen_name: &str) -> MemberId {
        let member_id: MemberId = add_participant(state, screen_name, None, false);
        let member: &mut Member = state.members.get_mut(&member_id).unwrap();
        member.role = MemberRoleKind::Spectator;
        member.loadout = None;

        member_id
    }

    fn get_joined_team(admission: &JoinAdmission) -> Option<TeamKind> {
        let MemberEvent::Joined { team, .. } = admission.member_events[0] else {
            panic!("the first member event of an admission is Joined");
        };

        team
    }

    fn set_round_phase(state: &mut GameState, phase: RoundPhase) {
        state.round = Some(RoundState {
            phase,
            phase_started_at: Tick(0),
        });
    }

    #[test]
    fn admit_join_allocates_the_next_member_id_and_spawns_the_joiner() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);
        add_participant(&mut state, "First", None, true);
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();
        let joiner: Joiner = test_fixture::create_joiner("Blob", TeamChoiceKind::Auto);

        let admission: JoinAdmission = admit_join(
            &state,
            &mut pending_admissions,
            &mut create_team_assignment_rng(),
            &joiner,
        )
        .unwrap();

        assert_eq!(
            admission,
            JoinAdmission {
                member_id: MemberId(1),
                member_events: vec![
                    MemberEvent::Joined {
                        member_id: MemberId(1),
                        screen_name: String::from("Blob"),
                        role: MemberRoleKind::Participant,
                        loadout: Some(test_fixture::create_loadout()),
                        team: None,
                    },
                    MemberEvent::SpawnRequested {
                        member_id: MemberId(1),
                        loadout: test_fixture::create_loadout(),
                        team: None,
                    },
                ],
            },
        );
        assert_eq!(pending_admissions.get_next_member_id(&state), MemberId(2));
    }

    #[test]
    fn admit_join_counts_pending_joins_for_names_slots_and_member_ids() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 3);
        add_participant(&mut state, "First", None, true);
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();

        let first_admission: JoinAdmission = admit_join(
            &state,
            &mut pending_admissions,
            &mut create_team_assignment_rng(),
            &test_fixture::create_joiner("Blob", TeamChoiceKind::Auto),
        )
        .unwrap();

        assert_eq!(first_admission.member_id, MemberId(1));
        assert_eq!(
            admit_join(
                &state,
                &mut pending_admissions,
                &mut create_team_assignment_rng(),
                &test_fixture::create_joiner("Blob", TeamChoiceKind::Auto),
            ),
            Err(RejectionKind::ScreenNameTaken),
        );

        let second_admission: JoinAdmission = admit_join(
            &state,
            &mut pending_admissions,
            &mut create_team_assignment_rng(),
            &test_fixture::create_joiner("Second", TeamChoiceKind::Auto),
        )
        .unwrap();

        assert_eq!(second_admission.member_id, MemberId(2));
        assert_eq!(
            admit_join(
                &state,
                &mut pending_admissions,
                &mut create_team_assignment_rng(),
                &test_fixture::create_joiner("Other", TeamChoiceKind::Auto),
            ),
            Err(RejectionKind::GameFull),
        );
    }

    #[test]
    fn admit_join_rejects_a_name_taken_by_a_member() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);
        add_participant(&mut state, "Blob", None, false);

        assert_eq!(
            admit_join(
                &state,
                &mut PendingAdmissions::new(),
                &mut create_team_assignment_rng(),
                &test_fixture::create_joiner("Blob", TeamChoiceKind::Auto),
            ),
            Err(RejectionKind::ScreenNameTaken),
        );
    }

    #[test]
    fn admit_join_rejects_at_the_member_limit() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 2);

        for spectator_index in 0..protocol_limits::get_member_limit(2) {
            add_spectator(&mut state, &format!("Watcher {spectator_index}"));
        }

        assert_eq!(
            admit_join(
                &state,
                &mut PendingAdmissions::new(),
                &mut create_team_assignment_rng(),
                &test_fixture::create_joiner("Blob", TeamChoiceKind::Auto),
            ),
            Err(RejectionKind::GameFull),
        );
    }

    #[test]
    fn admit_join_admits_without_spawning_while_a_survival_round_is_in_progress() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Survival, 2);
        add_participant(&mut state, "First", None, true);
        add_participant(&mut state, "Second", None, true);
        set_round_phase(&mut state, RoundPhase::Playing);
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();

        let admission: JoinAdmission = admit_join(
            &state,
            &mut pending_admissions,
            &mut create_team_assignment_rng(),
            &test_fixture::create_joiner("Late", TeamChoiceKind::Auto),
        )
        .unwrap();

        assert_eq!(
            admission,
            JoinAdmission {
                member_id: MemberId(2),
                member_events: vec![MemberEvent::Joined {
                    member_id: MemberId(2),
                    screen_name: String::from("Late"),
                    role: MemberRoleKind::Participant,
                    loadout: Some(test_fixture::create_loadout()),
                    team: None,
                }],
            },
        );
        assert!(!pending_admissions.is_spawn_pending(admission.member_id));
    }

    #[test]
    fn admit_join_assigns_auto_to_the_smallest_team_counting_pending_joins() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Skirmish, 8);
        add_participant(&mut state, "First", Some(TeamKind::Red), true);
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();
        let mut team_assignment_rng: Pcg32 = create_team_assignment_rng();

        let joined_teams: Vec<Option<TeamKind>> = ["Second", "Third", "Fourth"]
            .into_iter()
            .map(|screen_name| {
                let admission: JoinAdmission = admit_join(
                    &state,
                    &mut pending_admissions,
                    &mut team_assignment_rng,
                    &test_fixture::create_joiner(screen_name, TeamChoiceKind::Auto),
                )
                .unwrap();

                get_joined_team(&admission)
            })
            .collect();

        assert_eq!(joined_teams[0], Some(TeamKind::Blue));
        assert!(matches!(
            (joined_teams[1], joined_teams[2]),
            (Some(TeamKind::Red), Some(TeamKind::Blue)) | (Some(TeamKind::Blue), Some(TeamKind::Red)),
        ));
    }

    #[test]
    fn resolve_team_auto_chooses_among_every_smallest_team() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Skirmish, 8);
        state.settings.team_count = Some(3);
        add_participant(&mut state, "First", Some(TeamKind::Blue), true);
        let pending_admissions: PendingAdmissions = PendingAdmissions::new();
        let mut team_assignment_rng: Pcg32 = create_team_assignment_rng();

        let chosen_teams: Vec<Option<TeamKind>> = (0..32)
            .map(|_| {
                resolve_team(
                    &state,
                    &pending_admissions,
                    &mut team_assignment_rng,
                    TeamChoiceKind::Auto,
                    None,
                )
                .unwrap()
            })
            .collect();

        assert!(chosen_teams.contains(&Some(TeamKind::Red)));
        assert!(chosen_teams.contains(&Some(TeamKind::Green)));
        assert!(chosen_teams.iter().all(|chosen_team| matches!(chosen_team, Some(TeamKind::Red | TeamKind::Green))));
    }

    #[test]
    fn admit_join_rejects_a_team_with_more_players_than_another() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Skirmish, 8);
        add_participant(&mut state, "First", Some(TeamKind::Red), true);

        assert_eq!(
            admit_join(
                &state,
                &mut PendingAdmissions::new(),
                &mut create_team_assignment_rng(),
                &test_fixture::create_joiner("Second", TeamChoiceKind::Team(TeamKind::Red)),
            ),
            Err(RejectionKind::TeamUnbalanced {
                requested: TeamKind::Red,
                smaller: TeamKind::Blue,
            }),
        );
    }

    #[test]
    fn admit_join_rejects_a_team_outside_the_game_naming_the_smallest_team() {
        let state: GameState = test_fixture::create_state(GameModeKind::Skirmish, 8);

        assert_eq!(
            admit_join(
                &state,
                &mut PendingAdmissions::new(),
                &mut create_team_assignment_rng(),
                &test_fixture::create_joiner("First", TeamChoiceKind::Team(TeamKind::Pink)),
            ),
            Err(RejectionKind::TeamUnbalanced {
                requested: TeamKind::Pink,
                smaller: TeamKind::Red,
            }),
        );
    }

    #[test]
    fn admit_join_ignores_the_team_outside_skirmish() {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);

        let admission: JoinAdmission = admit_join(
            &state,
            &mut PendingAdmissions::new(),
            &mut create_team_assignment_rng(),
            &test_fixture::create_joiner("First", TeamChoiceKind::Team(TeamKind::Pink)),
        )
        .unwrap();

        assert!(matches!(
            admission.member_events[0],
            MemberEvent::Joined { team: None, .. }
        ));
    }

    #[test]
    fn admit_spectate_adds_a_spectator_without_loadout_or_team() {
        let state: GameState = test_fixture::create_state(GameModeKind::Skirmish, 8);

        assert_eq!(
            admit_spectate(&state, &mut PendingAdmissions::new(), "Watcher"),
            Ok(JoinAdmission {
                member_id: MemberId(0),
                member_events: vec![MemberEvent::Joined {
                    member_id: MemberId(0),
                    screen_name: String::from("Watcher"),
                    role: MemberRoleKind::Spectator,
                    loadout: None,
                    team: None,
                }],
            }),
        );
    }

    #[test]
    fn admit_spectate_rejects_at_the_spectator_limit_counting_pending_spectators() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();

        for spectator_index in 0..protocol_limits::SPECTATOR_LIMIT_PER_GAME - 1 {
            add_spectator(&mut state, &format!("Watcher {spectator_index}"));
        }

        admit_spectate(&state, &mut pending_admissions, "Last").unwrap();

        assert_eq!(
            admit_spectate(&state, &mut pending_admissions, "Extra"),
            Err(RejectionKind::SpectatorLimitReached),
        );
        assert_eq!(
            admit_spectate(&state, &mut pending_admissions, "Last"),
            Err(RejectionKind::ScreenNameTaken),
        );
    }

    #[test]
    fn admit_respawn_requests_a_spawn_for_a_dead_participant() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);
        let member_id: MemberId = add_participant(&mut state, "First", None, false);
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();

        assert_eq!(
            admit_respawn(
                &state,
                &mut pending_admissions,
                &mut create_team_assignment_rng(),
                member_id,
                test_fixture::create_loadout(),
                TeamChoiceKind::Auto,
            ),
            Ok(MemberEvent::SpawnRequested {
                member_id,
                loadout: test_fixture::create_loadout(),
                team: None,
            }),
        );
        assert_eq!(
            admit_respawn(
                &state,
                &mut pending_admissions,
                &mut create_team_assignment_rng(),
                member_id,
                test_fixture::create_loadout(),
                TeamChoiceKind::Auto,
            ),
            Err(RejectionKind::NotDead),
        );
    }

    #[test]
    fn admit_respawn_rejects_an_alive_or_unknown_member() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);
        let alive_member_id: MemberId = add_participant(&mut state, "First", None, true);
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();
        state.next_member_id = MemberId(5);

        for (member_id, rejection) in [
            (alive_member_id, RejectionKind::NotDead),
            (MemberId(3), RejectionKind::NotInGame),
            (MemberId(5), RejectionKind::NotDead),
        ] {
            assert_eq!(
                admit_respawn(
                    &state,
                    &mut pending_admissions,
                    &mut create_team_assignment_rng(),
                    member_id,
                    test_fixture::create_loadout(),
                    TeamChoiceKind::Auto,
                ),
                Err(rejection),
            );
        }
    }

    #[test]
    fn admit_respawn_rejects_at_the_player_cap_and_during_a_survival_round() {
        let mut full_state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 2);
        add_participant(&mut full_state, "First", None, true);
        add_participant(&mut full_state, "Second", None, true);
        let full_member_id: MemberId = add_participant(&mut full_state, "Third", None, false);
        let mut survival_state: GameState = test_fixture::create_state(GameModeKind::Survival, 8);
        let survival_member_id: MemberId = add_participant(&mut survival_state, "First", None, false);
        set_round_phase(&mut survival_state, RoundPhase::PostRound);

        assert_eq!(
            admit_respawn(
                &full_state,
                &mut PendingAdmissions::new(),
                &mut create_team_assignment_rng(),
                full_member_id,
                test_fixture::create_loadout(),
                TeamChoiceKind::Auto,
            ),
            Err(RejectionKind::GameFull),
        );
        assert_eq!(
            admit_respawn(
                &survival_state,
                &mut PendingAdmissions::new(),
                &mut create_team_assignment_rng(),
                survival_member_id,
                test_fixture::create_loadout(),
                TeamChoiceKind::Auto,
            ),
            Err(RejectionKind::RoundInProgress),
        );
    }

    #[test]
    fn admit_respawn_lets_a_member_stay_on_its_team_and_excludes_it_from_the_count() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Skirmish, 8);
        add_participant(&mut state, "First", Some(TeamKind::Red), true);
        add_participant(&mut state, "Second", Some(TeamKind::Red), true);
        let member_id: MemberId = add_participant(&mut state, "Third", Some(TeamKind::Blue), false);
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();

        assert_eq!(
            admit_respawn(
                &state,
                &mut pending_admissions,
                &mut create_team_assignment_rng(),
                member_id,
                test_fixture::create_loadout(),
                TeamChoiceKind::Team(TeamKind::Red),
            ),
            Err(RejectionKind::TeamUnbalanced {
                requested: TeamKind::Red,
                smaller: TeamKind::Blue,
            }),
        );
        assert_eq!(
            admit_respawn(
                &state,
                &mut pending_admissions,
                &mut create_team_assignment_rng(),
                member_id,
                test_fixture::create_loadout(),
                TeamChoiceKind::Team(TeamKind::Blue),
            ),
            Ok(MemberEvent::SpawnRequested {
                member_id,
                loadout: test_fixture::create_loadout(),
                team: Some(TeamKind::Blue),
            }),
        );
    }

    #[test]
    fn admit_respawn_converts_a_spectator() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);
        let member_id: MemberId = add_spectator(&mut state, "Watcher");

        assert_eq!(
            admit_respawn(
                &state,
                &mut PendingAdmissions::new(),
                &mut create_team_assignment_rng(),
                member_id,
                test_fixture::create_loadout(),
                TeamChoiceKind::Auto,
            ),
            Ok(MemberEvent::SpawnRequested {
                member_id,
                loadout: test_fixture::create_loadout(),
                team: None,
            }),
        );
    }
}
