use crate::game::tick;
use crate::game::{GameState, SimulationEvent, Tick};
use crate::member::{Member, MemberId, MemberRoleKind};
use crate::organism::spawn;

pub const FORCE_SPAWN_ELAPSED_TICKS: u32 = tick::get_ticks_from_milliseconds(7000);
/// Length of the PreRound and PostRound phases.
pub const ROUND_DELAY_TICKS: u32 = tick::get_ticks_from_milliseconds(8000);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoundState {
    pub phase: RoundPhase,
    pub phase_started_at: Tick,
}

impl RoundState {
    /// Seconds left of the PreRound or PostRound delay; `None` in the other phases.
    pub fn get_countdown_seconds(&self, tick: Tick) -> Option<u32> {
        match self.phase {
            RoundPhase::PreRound | RoundPhase::PostRound => {
                let elapsed_ticks: u32 = tick.ticks_since(self.phase_started_at);
                let remaining_ticks: u32 = ROUND_DELAY_TICKS.saturating_sub(elapsed_ticks);

                Some(tick::get_seconds_rounded_up(remaining_ticks))
            }
            RoundPhase::Waiting | RoundPhase::Playing => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundPhase {
    Waiting,
    PreRound,
    Playing,
    PostRound,
}

pub fn run_survival_shrink_phase(state: &mut GameState) {
    let is_playing: bool = state.round.is_some_and(|round| round.phase == RoundPhase::Playing);

    if is_playing {
        state.world.shrink();
    }
}

/// At most one transition per tick.
pub fn run_round_transition_phase(state: &mut GameState, tick: Tick) -> Vec<SimulationEvent> {
    let Some(round) = state.round else {
        return Vec::new();
    };

    let elapsed_ticks: u32 = tick.ticks_since(round.phase_started_at);
    let has_player_minimum: bool = count_participants(state) >= u32::from(state.settings.player_minimum.unwrap_or(0));

    match round.phase {
        RoundPhase::Waiting if has_player_minimum => enter_phase(state, RoundPhase::PreRound, tick),
        RoundPhase::PreRound if !has_player_minimum => enter_phase(state, RoundPhase::Waiting, tick),
        RoundPhase::PreRound if elapsed_ticks == FORCE_SPAWN_ELAPSED_TICKS => spawn::force_spawn_participants(state),
        RoundPhase::PreRound if elapsed_ticks >= ROUND_DELAY_TICKS => enter_phase(state, RoundPhase::Playing, tick),
        RoundPhase::Playing if state.alive_organism_count() <= 1 => end_round(state, tick),
        RoundPhase::PostRound if elapsed_ticks >= ROUND_DELAY_TICKS => start_next_round(state, tick),
        RoundPhase::Waiting | RoundPhase::PreRound | RoundPhase::Playing | RoundPhase::PostRound => Vec::new(),
    }
}

/// Members with the Participant role, alive or not.
pub fn count_participants(state: &GameState) -> u32 {
    let participant_count: usize =
        state.members.values().filter(|member| member.role == MemberRoleKind::Participant).count();

    u32::try_from(participant_count).unwrap_or(u32::MAX)
}

fn enter_phase(state: &mut GameState, phase: RoundPhase, tick: Tick) -> Vec<SimulationEvent> {
    state.round = Some(RoundState {
        phase,
        phase_started_at: tick,
    });

    vec![SimulationEvent::RoundPhaseChanged { phase }]
}

fn end_round(state: &mut GameState, tick: Tick) -> Vec<SimulationEvent> {
    let survivor_id: Option<MemberId> =
        state.members.values().find(|member| member.organism.is_some()).map(|member| member.member_id);
    let mut simulation_events: Vec<SimulationEvent> = enter_phase(state, RoundPhase::PostRound, tick);
    let survivor: Option<&mut Member> = survivor_id.and_then(|member_id| state.members.get_mut(&member_id));

    if let Some(survivor) = survivor {
        survivor.score.wins += 1;
        simulation_events.push(SimulationEvent::RoundWon {
            member_id: survivor.member_id,
        });
    }

    simulation_events
}

fn start_next_round(state: &mut GameState, tick: Tick) -> Vec<SimulationEvent> {
    state.world.restore_initial_bounds();

    let mut simulation_events: Vec<SimulationEvent> = enter_phase(state, RoundPhase::Waiting, tick);
    let spawn_events: Vec<SimulationEvent> = spawn::force_spawn_participants(state);
    simulation_events.extend(spawn_events);

    simulation_events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::WorldPoint;
    use crate::world::{WorldBounds, WorldShapeKind};

    const ROUND_START_TICK: Tick = Tick(1000);

    fn create_survival_state(participant_count: u32, phase: RoundPhase) -> GameState {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Survival, WorldShapeKind::Rectangle, 800);
        state.round = Some(RoundState {
            phase,
            phase_started_at: ROUND_START_TICK,
        });

        for index in 0..participant_count {
            let member_id: MemberId = MemberId(index);
            state.members.insert(member_id, test_fixture::create_participant(member_id));
        }

        state
    }

    fn give_organisms(state: &mut GameState, member_ids: &[MemberId]) {
        for (index, member_id) in member_ids.iter().enumerate() {
            let x: i32 = 100 + 100 * i32::try_from(index).unwrap();
            spawn::place_organism(state, *member_id, WorldPoint { x, y: 100 }).unwrap();
        }
    }

    fn get_phase(state: &GameState) -> RoundPhase {
        state.round.unwrap().phase
    }

    fn get_tick_after(elapsed_ticks: u32) -> Tick {
        ROUND_START_TICK.plus(elapsed_ticks)
    }

    #[test]
    fn delays_match_the_designed_tick_counts() {
        assert_eq!(FORCE_SPAWN_ELAPSED_TICKS, 100);
        assert_eq!(ROUND_DELAY_TICKS, 114);
    }

    #[test]
    fn get_countdown_seconds_counts_down_the_delay() {
        let round: RoundState = RoundState {
            phase: RoundPhase::PreRound,
            phase_started_at: ROUND_START_TICK,
        };
        let playing_round: RoundState = RoundState {
            phase: RoundPhase::Playing,
            phase_started_at: ROUND_START_TICK,
        };

        assert_eq!(round.get_countdown_seconds(ROUND_START_TICK), Some(8));
        assert_eq!(round.get_countdown_seconds(get_tick_after(14)), Some(7));
        assert_eq!(round.get_countdown_seconds(get_tick_after(15)), Some(7));
        assert_eq!(round.get_countdown_seconds(get_tick_after(114)), Some(0));
        assert_eq!(playing_round.get_countdown_seconds(ROUND_START_TICK), None);
    }

    #[test]
    fn run_round_transition_phase_starts_the_pre_round_at_the_player_minimum() {
        let mut state: GameState = create_survival_state(1, RoundPhase::Waiting);

        assert_eq!(run_round_transition_phase(&mut state, Tick(1001)), Vec::new());

        state.members.insert(MemberId(1), test_fixture::create_participant(MemberId(1)));

        assert_eq!(
            run_round_transition_phase(&mut state, Tick(1002)),
            vec![SimulationEvent::RoundPhaseChanged {
                phase: RoundPhase::PreRound,
            }],
        );
        assert_eq!(
            state.round,
            Some(RoundState {
                phase: RoundPhase::PreRound,
                phase_started_at: Tick(1002),
            }),
        );
    }

    #[test]
    fn run_round_transition_phase_counts_spectators_out() {
        let mut state: GameState = create_survival_state(1, RoundPhase::Waiting);
        let mut spectator: Member = test_fixture::create_participant(MemberId(1));
        spectator.role = MemberRoleKind::Spectator;
        state.members.insert(MemberId(1), spectator);

        assert_eq!(run_round_transition_phase(&mut state, Tick(1001)), Vec::new());
        assert_eq!(get_phase(&state), RoundPhase::Waiting);
    }

    #[test]
    fn run_round_transition_phase_cancels_the_pre_round_below_the_minimum() {
        let mut state: GameState = create_survival_state(1, RoundPhase::PreRound);

        assert_eq!(
            run_round_transition_phase(&mut state, get_tick_after(50)),
            vec![SimulationEvent::RoundPhaseChanged {
                phase: RoundPhase::Waiting,
            }],
        );
    }

    #[test]
    fn run_round_transition_phase_force_spawns_during_the_pre_round() {
        let mut state: GameState = create_survival_state(2, RoundPhase::PreRound);

        assert_eq!(run_round_transition_phase(&mut state, get_tick_after(99)), Vec::new());

        let simulation_events: Vec<SimulationEvent> = run_round_transition_phase(&mut state, get_tick_after(100));

        assert_eq!(simulation_events.len(), 2);
        assert_eq!(state.alive_organism_count(), 2);
        assert_eq!(get_phase(&state), RoundPhase::PreRound);
    }

    #[test]
    fn run_round_transition_phase_starts_playing_after_the_delay() {
        let mut state: GameState = create_survival_state(2, RoundPhase::PreRound);

        assert_eq!(run_round_transition_phase(&mut state, get_tick_after(113)), Vec::new());
        assert_eq!(
            run_round_transition_phase(&mut state, get_tick_after(114)),
            vec![SimulationEvent::RoundPhaseChanged {
                phase: RoundPhase::Playing,
            }],
        );
    }

    #[test]
    fn run_round_transition_phase_gives_the_sole_survivor_a_win() {
        let mut state: GameState = create_survival_state(3, RoundPhase::Playing);
        give_organisms(&mut state, &[MemberId(0), MemberId(2)]);

        assert_eq!(run_round_transition_phase(&mut state, Tick(1001)), Vec::new());

        state.members.get_mut(&MemberId(0)).unwrap().organism = None;

        assert_eq!(
            run_round_transition_phase(&mut state, Tick(1002)),
            vec![
                SimulationEvent::RoundPhaseChanged {
                    phase: RoundPhase::PostRound,
                },
                SimulationEvent::RoundWon { member_id: MemberId(2) },
            ],
        );
        assert_eq!(state.members[&MemberId(2)].score.wins, 1);
    }

    #[test]
    fn run_round_transition_phase_ends_without_a_winner_when_nobody_survives() {
        let mut state: GameState = create_survival_state(2, RoundPhase::Playing);

        assert_eq!(
            run_round_transition_phase(&mut state, Tick(1001)),
            vec![SimulationEvent::RoundPhaseChanged {
                phase: RoundPhase::PostRound,
            }],
        );
        assert!(state.members.values().all(|member| member.score.wins == 0));
    }

    #[test]
    fn run_round_transition_phase_restores_the_world_and_force_spawns_after_the_post_round() {
        let mut state: GameState = create_survival_state(2, RoundPhase::PostRound);
        state.world.shrink();

        assert_eq!(run_round_transition_phase(&mut state, get_tick_after(113)), Vec::new());

        let simulation_events: Vec<SimulationEvent> = run_round_transition_phase(&mut state, get_tick_after(114));

        assert_eq!(
            simulation_events[0],
            SimulationEvent::RoundPhaseChanged {
                phase: RoundPhase::Waiting,
            },
        );
        assert_eq!(simulation_events.len(), 3);
        assert_eq!(state.world.bounds, WorldBounds::from_pixel_size(800, 800));
        assert_eq!(state.alive_organism_count(), 2);
        assert_eq!(get_phase(&state), RoundPhase::Waiting);
    }

    #[test]
    fn run_round_transition_phase_does_nothing_without_rounds() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);

        assert_eq!(run_round_transition_phase(&mut state, Tick(1)), Vec::new());
    }

    #[test]
    fn run_survival_shrink_phase_shrinks_only_while_playing() {
        let mut waiting_state: GameState = create_survival_state(0, RoundPhase::Waiting);
        let mut playing_state: GameState = create_survival_state(0, RoundPhase::Playing);

        run_survival_shrink_phase(&mut waiting_state);
        run_survival_shrink_phase(&mut playing_state);

        assert_eq!(waiting_state.world.bounds, waiting_state.world.initial_bounds);
        assert_ne!(playing_state.world.bounds, playing_state.world.initial_bounds);
    }
}
