use std::time::{Duration, Instant};

use bytes::Bytes;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::{Interval, MissedTickBehavior};

use shared::ability::Loadout;
use shared::game;
use shared::game::{GameId, GameState, GameSummary, MemberEvent, SimulationEvent, StepError, Tick};
use shared::member::{MemberId, TeamChoiceKind};
use shared::protocol::state_checksum;
use shared::protocol::{
    GameSnapshotSerialOut, GameStateSerialOut, InputBundleSerialOut, MessageSerialOut, RejectionKind,
    SubpixelPointSerial,
};
use shared::random::Pcg32;

use crate::game::membership;
use crate::game::{
    BuiltBundle, BundleBuilder, ConnectionEndpoint, CursorCorrection, GameInput, GameRequest, InputAcceptanceKind,
    JoinAdmission, PendingAdmissions, Recipients, ReplayCapture,
};
use crate::socket::connection_model;
use crate::socket::{CloseCauseKind, ProtocolViolationKind};

pub const CHECKSUM_INTERVAL_TICKS: u32 = 14;
/// About 3 s of ticks.
pub const TICK_LAG_RESET_THRESHOLD: u32 = 43;
pub const COMMAND_DRAIN_LIMIT_PER_TICK: usize = 4096;

/// Owns one game's state and members; never awaits a send.
pub struct GameTask {
    game_id: GameId,
    state: GameState,
    is_secured: bool,
    summary: watch::Sender<GameSummary>,
    recipients: Recipients,
    pending_admissions: PendingAdmissions,
    team_assignment_rng: Pcg32,
    bundle_builder: BundleBuilder,
    replay_capture: Option<ReplayCapture>,
    empty_game_grace_period: Duration,
    empty_since: Option<Instant>,
    stale_input_count: u64,
}

impl GameTask {
    pub fn new(
        game_id: GameId,
        state: GameState,
        is_secured: bool,
        summary: watch::Sender<GameSummary>,
        replay_capture: Option<ReplayCapture>,
        empty_game_grace_period: Duration,
    ) -> GameTask {
        GameTask {
            game_id,
            state,
            is_secured,
            summary,
            recipients: Recipients::new(),
            pending_admissions: PendingAdmissions::new(),
            team_assignment_rng: create_team_assignment_rng(game_id),
            bundle_builder: BundleBuilder::new(),
            replay_capture,
            empty_game_grace_period,
            empty_since: None,
            stale_input_count: 0,
        }
    }

    pub fn handle_request(&mut self, request: GameRequest, now: Instant) {
        match request {
            GameRequest::Join {
                endpoint,
                joiner,
                reply,
            } => {
                let admission: Result<JoinAdmission, RejectionKind> = membership::admit_join(
                    &self.state,
                    &mut self.pending_admissions,
                    &mut self.team_assignment_rng,
                    &joiner,
                );
                self.admit_member(admission, endpoint, reply, now);
            }
            GameRequest::Spectate {
                endpoint,
                screen_name,
                reply,
            } => {
                let admission: Result<JoinAdmission, RejectionKind> =
                    membership::admit_spectate(&self.state, &mut self.pending_admissions, &screen_name);
                self.admit_member(admission, endpoint, reply, now);
            }
            GameRequest::Respawn {
                member_id,
                loadout,
                team,
                reply,
            } => {
                let respawn: Result<(), RejectionKind> = self.respawn_member(member_id, loadout, team);
                let _ = reply.send(respawn);
            }
            GameRequest::UpdateAppearance { member_id, appearance } => {
                if self.recipients.contains(member_id) {
                    self.bundle_builder.queue_member_event(MemberEvent::AppearanceChanged { member_id, appearance });
                }
            }
            GameRequest::Leave { member_id, reply } => {
                self.remove_member(member_id);
                let _ = reply.send(());
            }
            GameRequest::ConnectionClosed { connection_id } => {
                let member_id: Option<MemberId> = self.recipients.remove_connection(connection_id);

                if let Some(member_id) = member_id {
                    self.queue_departure(member_id);
                }
            }
            GameRequest::RequestSnapshot { member_id } => {
                log::info!(
                    "resync requested; [game_id={} member_id={} tick={}]",
                    self.game_id.0,
                    member_id.0,
                    self.state.tick.0,
                );
                self.recipients.request_snapshot(member_id);
            }
        }
    }

    pub fn handle_input(&mut self, game_input: GameInput) {
        if !self.recipients.contains(game_input.member_id) {
            return;
        }

        let acceptance: InputAcceptanceKind =
            self.bundle_builder.queue_input(&self.state, game_input.member_id, game_input.input);

        match acceptance {
            InputAcceptanceKind::AheadOfServer => {
                let cause: CloseCauseKind = CloseCauseKind::ProtocolViolation(ProtocolViolationKind::Validation);
                self.recipients.close_member(game_input.member_id, cause);
                self.queue_departure(game_input.member_id);
            }
            InputAcceptanceKind::Stale => self.stale_input_count += 1,
            InputAcceptanceKind::Accepted | InputAcceptanceKind::WithoutOrganism | InputAcceptanceKind::BeforeSpawn => {
            }
        }
    }

    pub fn run_tick(&mut self, now: Instant) -> GameFlow {
        let built_bundle: BuiltBundle = self.bundle_builder.build(&self.state);
        self.pending_admissions.clear();

        let step_result: Result<Vec<SimulationEvent>, StepError> = game::step(&mut self.state, &built_bundle.bundle);
        let simulation_events: Vec<SimulationEvent> = match step_result {
            Ok(simulation_events) => simulation_events,
            Err(error) => {
                log::error!("game step failed; [game_id={} error={error:?}]", self.game_id.0);

                return GameFlow::End;
            }
        };

        self.send_tick_frames(built_bundle, now);

        let departed_member_ids: Vec<MemberId> = self.recipients.remove_departed(now);

        for member_id in departed_member_ids {
            self.queue_departure(member_id);
        }

        self.handle_simulation_events(&simulation_events);
        self.update_summary();
        self.update_empty_since(now);

        GameFlow::Continue
    }

    /// An empty game ends after its grace period.
    pub fn get_flow(&self, now: Instant) -> GameFlow {
        let is_expired: bool = self
            .empty_since
            .is_some_and(|empty_since| now.saturating_duration_since(empty_since) >= self.empty_game_grace_period);

        match is_expired {
            true => GameFlow::End,
            false => GameFlow::Continue,
        }
    }

    fn admit_member(
        &mut self,
        admission: Result<JoinAdmission, RejectionKind>,
        endpoint: ConnectionEndpoint,
        reply: oneshot::Sender<Result<MemberId, RejectionKind>>,
        now: Instant,
    ) {
        let admission: JoinAdmission = match admission {
            Ok(admission) => admission,
            Err(rejection) => {
                let _ = reply.send(Err(rejection));

                return;
            }
        };

        let member_id: MemberId = admission.member_id;
        let join_accepted_frame: Bytes = connection_model::encode_frame(&MessageSerialOut::JoinAccepted {
            game_id: self.game_id.0,
            member_id: member_id.0,
        });

        self.recipients.add(member_id, endpoint, self.state.tick);
        self.recipients.send_to_member(member_id, join_accepted_frame, now);
        self.recipients.send_to_member(member_id, self.encode_snapshot(), now);

        for member_event in admission.member_events {
            self.bundle_builder.queue_member_event(member_event);
        }

        self.empty_since = None;
        let _ = reply.send(Ok(member_id));

        log::info!(
            "member admitted; [game_id={} member_id={}]",
            self.game_id.0,
            member_id.0
        );
    }

    fn respawn_member(
        &mut self,
        member_id: MemberId,
        loadout: Loadout,
        team: TeamChoiceKind,
    ) -> Result<(), RejectionKind> {
        if !self.recipients.contains(member_id) {
            return Err(RejectionKind::NotInGame);
        }

        let member_event: MemberEvent = membership::admit_respawn(
            &self.state,
            &mut self.pending_admissions,
            &mut self.team_assignment_rng,
            member_id,
            loadout,
            team,
        )?;
        self.bundle_builder.queue_member_event(member_event);

        Ok(())
    }

    fn remove_member(&mut self, member_id: MemberId) {
        let removed_endpoint: Option<ConnectionEndpoint> = self.recipients.remove(member_id);

        if removed_endpoint.is_some() {
            self.queue_departure(member_id);
        }
    }

    /// `Left` in the next bundle.
    fn queue_departure(&mut self, member_id: MemberId) {
        self.bundle_builder.queue_member_event(MemberEvent::Left { member_id });
        self.bundle_builder.forget_member(member_id);
    }

    fn send_tick_frames(&mut self, built_bundle: BuiltBundle, now: Instant) {
        let tick: Tick = self.state.tick;
        let bundle_serial_out: InputBundleSerialOut = InputBundleSerialOut::from(&built_bundle.bundle);

        if let Some(replay_capture) = &mut self.replay_capture {
            replay_capture.record_bundle(bundle_serial_out.clone());
        }

        let bundle_frame: Bytes = connection_model::encode_frame(&MessageSerialOut::InputBundle(bundle_serial_out));
        self.recipients.broadcast(&bundle_frame, now);

        for cursor_correction in built_bundle.cursor_corrections {
            self.send_cursor_correction(cursor_correction, now);
        }

        if tick.0 % CHECKSUM_INTERVAL_TICKS == 0 {
            let checksum_frame: Bytes = connection_model::encode_frame(&MessageSerialOut::StateChecksum {
                tick: tick.0,
                checksum: state_checksum::get_state_checksum(&self.state),
            });
            self.recipients.broadcast(&checksum_frame, now);
        }

        self.send_due_snapshots(tick, now);
    }

    fn send_cursor_correction(&mut self, cursor_correction: CursorCorrection, now: Instant) {
        let correction_frame: Bytes = connection_model::encode_frame(&MessageSerialOut::CursorCorrected {
            client_tick: cursor_correction.client_tick.0,
            cursor: SubpixelPointSerial::from(&cursor_correction.cursor),
        });

        self.recipients.send_to_member(cursor_correction.member_id, correction_frame, now);
    }

    fn send_due_snapshots(&mut self, tick: Tick, now: Instant) {
        let snapshot_member_ids: Vec<MemberId> = self.recipients.get_snapshot_member_ids(tick);

        if snapshot_member_ids.is_empty() {
            return;
        }

        let snapshot_frame: Bytes = self.encode_snapshot();

        for member_id in snapshot_member_ids {
            self.recipients.send_snapshot(member_id, snapshot_frame.clone(), tick, now);
        }
    }

    fn encode_snapshot(&self) -> Bytes {
        connection_model::encode_frame(&MessageSerialOut::Snapshot(GameSnapshotSerialOut {
            game_id: self.game_id.0,
            state: GameStateSerialOut::from(&self.state),
        }))
    }

    fn handle_simulation_events(&mut self, simulation_events: &[SimulationEvent]) {
        for simulation_event in simulation_events {
            match simulation_event {
                SimulationEvent::OrganismSpawned { member_id, cursor } => {
                    self.bundle_builder.record_spawn(*member_id, *cursor, self.state.tick);
                }
                SimulationEvent::MemberLeft { member_id } => {
                    log::info!("member left; [game_id={} member_id={}]", self.game_id.0, member_id.0);
                }
                SimulationEvent::RoundPhaseChanged { phase } => {
                    log::debug!("round phase changed; [game_id={} phase={phase:?}]", self.game_id.0);
                }
                SimulationEvent::MemberJoined { .. }
                | SimulationEvent::SpawnRejected { .. }
                | SimulationEvent::OrganismDied { .. }
                | SimulationEvent::RoundWon { .. }
                | SimulationEvent::EffectApplied { .. } => {}
            }
        }
    }

    fn update_summary(&mut self) {
        let game_summary: GameSummary = GameSummary::from_game_state(self.game_id, &self.state, self.is_secured);

        self.summary.send_if_modified(|current_summary| {
            if *current_summary == game_summary {
                return false;
            }

            *current_summary = game_summary;

            true
        });
    }

    fn update_empty_since(&mut self, now: Instant) {
        if !self.state.members.is_empty() {
            self.empty_since = None;

            return;
        }

        self.empty_since.get_or_insert(now);
    }
}

fn create_team_assignment_rng(game_id: GameId) -> Pcg32 {
    let seed_result: Result<u64, getrandom::Error> = getrandom::u64();

    match seed_result {
        Ok(seed) => Pcg32::from_seed(seed),
        Err(error) => {
            log::warn!(
                "cannot draw a team assignment seed; [game_id={} error={error}]",
                game_id.0
            );

            Pcg32::from_seed(u64::from(game_id.0))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameFlow {
    Continue,
    End,
}

/// Counts consecutive ticks fired at least one period after they were due.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TickLag {
    late_tick_count: u32,
}

impl TickLag {
    pub fn observe(&mut self, due_at: Instant, fired_at: Instant, tick_period: Duration) -> TickLagKind {
        let is_late: bool = fired_at.saturating_duration_since(due_at) >= tick_period;

        if !is_late {
            self.late_tick_count = 0;

            return TickLagKind::OnTime;
        }

        self.late_tick_count += 1;

        if self.late_tick_count > TICK_LAG_RESET_THRESHOLD {
            self.late_tick_count = 0;

            return TickLagKind::ResetDue;
        }

        TickLagKind::Late
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickLagKind {
    OnTime,
    Late,
    /// The tick count, not wall time, is the game clock.
    ResetDue,
}

enum GameWakeKind {
    Tick(Instant),
    Request(Option<GameRequest>),
    Input(Option<GameInput>),
}

/// Runs until the game is empty past its grace period or its step fails; queued requests are then answered
/// `GameNotFound`.
pub async fn run_game_task(
    mut game_task: GameTask,
    mut requests: mpsc::Receiver<GameRequest>,
    mut inputs: mpsc::Receiver<GameInput>,
    tick_period: Duration,
) {
    let mut interval: Interval = tokio::time::interval_at(tokio::time::Instant::now() + tick_period, tick_period);
    interval.set_missed_tick_behavior(MissedTickBehavior::Burst);
    let mut tick_lag: TickLag = TickLag::default();

    loop {
        let wake: GameWakeKind = tokio::select! {
            due_at = interval.tick() => GameWakeKind::Tick(due_at.into_std()),
            request = requests.recv() => GameWakeKind::Request(request),
            input = inputs.recv() => GameWakeKind::Input(input),
        };

        let flow: GameFlow = match wake {
            GameWakeKind::Tick(due_at) => {
                let now: Instant = Instant::now();
                let tick_lag_kind: TickLagKind = tick_lag.observe(due_at, now, tick_period);

                if tick_lag_kind == TickLagKind::ResetDue {
                    log::warn!("game fell behind; [game_id={}]", game_task.game_id.0);
                    interval.reset();
                }

                run_tick_and_drain(&mut game_task, &mut requests, &mut inputs, now)
            }
            GameWakeKind::Request(Some(request)) => {
                game_task.handle_request(request, Instant::now());

                GameFlow::Continue
            }
            GameWakeKind::Input(Some(game_input)) => {
                game_task.handle_input(game_input);

                GameFlow::Continue
            }
            GameWakeKind::Request(None) | GameWakeKind::Input(None) => GameFlow::End,
        };

        if flow == GameFlow::End {
            break;
        }
    }

    close_game(&mut requests, &mut inputs);

    log::info!(
        "game ended; [game_id={} tick={} stale_inputs={}]",
        game_task.game_id.0,
        game_task.state.tick.0,
        game_task.stale_input_count,
    );
}

/// A tick burst cannot starve the queued requests and inputs.
fn run_tick_and_drain(
    game_task: &mut GameTask,
    requests: &mut mpsc::Receiver<GameRequest>,
    inputs: &mut mpsc::Receiver<GameInput>,
    now: Instant,
) -> GameFlow {
    let tick_flow: GameFlow = game_task.run_tick(now);

    if tick_flow == GameFlow::End {
        return GameFlow::End;
    }

    for _ in 0..COMMAND_DRAIN_LIMIT_PER_TICK {
        let Ok(request) = requests.try_recv() else {
            break;
        };

        game_task.handle_request(request, now);
    }

    for _ in 0..COMMAND_DRAIN_LIMIT_PER_TICK {
        let Ok(game_input) = inputs.try_recv() else {
            break;
        };

        game_task.handle_input(game_input);
    }

    game_task.get_flow(now)
}

fn close_game(requests: &mut mpsc::Receiver<GameRequest>, inputs: &mut mpsc::Receiver<GameInput>) {
    requests.close();
    inputs.close();

    while let Ok(request) = requests.try_recv() {
        answer_closed_game(request);
    }
}

fn answer_closed_game(request: GameRequest) {
    match request {
        GameRequest::Join { reply, .. } | GameRequest::Spectate { reply, .. } => {
            let _ = reply.send(Err(RejectionKind::GameNotFound));
        }
        GameRequest::Respawn { reply, .. } => {
            let _ = reply.send(Err(RejectionKind::GameNotFound));
        }
        GameRequest::Leave { reply, .. } => {
            let _ = reply.send(());
        }
        GameRequest::UpdateAppearance { .. }
        | GameRequest::ConnectionClosed { .. }
        | GameRequest::RequestSnapshot { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use crate::socket::{ConnectionControl, ConnectionId, Membership};
    use shared::ability::AbilityPressSet;
    use shared::game::{GameModeKind, PlayerInput};
    use shared::geometry::SubpixelPoint;
    use shared::member::{Joiner, MemberRoleKind};
    use shared::protocol;
    use shared::protocol::{GameSummarySerialOut, MemberEventSerialOut};

    const GAME_ID: GameId = GameId(7);
    const GRACE_PERIOD: Duration = Duration::from_millis(200);

    struct TestGame {
        game_task: GameTask,
        summary: watch::Receiver<GameSummary>,
    }

    fn create_test_game(player_cap: u8) -> TestGame {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, player_cap);
        let (summary_sender, summary_receiver): (watch::Sender<GameSummary>, watch::Receiver<GameSummary>) =
            watch::channel(GameSummary::from_game_state(GAME_ID, &state, false));

        TestGame {
            game_task: GameTask::new(GAME_ID, state, false, summary_sender, None, GRACE_PERIOD),
            summary: summary_receiver,
        }
    }

    fn join(
        game_task: &mut GameTask,
        connection_id: u64,
        screen_name: &str,
    ) -> (Result<MemberId, RejectionKind>, Membership) {
        let (endpoint, membership): (ConnectionEndpoint, Membership) =
            connection_model::create_membership(ConnectionId(connection_id));
        let (reply_sender, mut reply_receiver): (
            oneshot::Sender<Result<MemberId, RejectionKind>>,
            oneshot::Receiver<Result<MemberId, RejectionKind>>,
        ) = oneshot::channel();
        let joiner: Joiner = test_fixture::create_joiner(screen_name, TeamChoiceKind::Auto);

        game_task.handle_request(
            GameRequest::Join {
                endpoint,
                joiner,
                reply: reply_sender,
            },
            Instant::now(),
        );

        (reply_receiver.try_recv().unwrap(), membership)
    }

    fn receive_messages(membership: &mut Membership) -> Vec<MessageSerialOut> {
        let mut message_serial_outs: Vec<MessageSerialOut> = Vec::new();

        while let Ok(frame) = membership.outbound.try_recv() {
            message_serial_outs.push(protocol::decode_message_out(&frame).unwrap());
        }

        message_serial_outs
    }

    fn get_bundle_member_events(message_serial_outs: &[MessageSerialOut]) -> Vec<MemberEventSerialOut> {
        message_serial_outs
            .iter()
            .filter_map(|message_serial_out| match message_serial_out {
                MessageSerialOut::InputBundle(bundle_serial_out) => Some(bundle_serial_out.member_events.clone()),
                _ => None,
            })
            .flatten()
            .collect()
    }

    fn run_ticks(game_task: &mut GameTask, tick_count: u32) {
        for _ in 0..tick_count {
            assert_eq!(game_task.run_tick(Instant::now()), GameFlow::Continue);
        }
    }

    #[test]
    fn handle_request_join_queues_join_accepted_then_the_snapshot_before_the_joined_bundle() {
        let mut test_game: TestGame = create_test_game(8);

        let (reply, mut membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "Blob");
        test_game.game_task.run_tick(Instant::now());
        let message_serial_outs: Vec<MessageSerialOut> = receive_messages(&mut membership);

        assert_eq!(reply, Ok(MemberId(0)));
        assert_eq!(
            message_serial_outs[0],
            MessageSerialOut::JoinAccepted {
                game_id: GAME_ID.0,
                member_id: 0,
            },
        );
        assert!(matches!(
            &message_serial_outs[1],
            MessageSerialOut::Snapshot(GameSnapshotSerialOut { game_id: 7, state }) if state.tick == 0 && state.members.is_empty()
        ));
        assert!(matches!(
            &message_serial_outs[2],
            MessageSerialOut::InputBundle(bundle_serial_out) if bundle_serial_out.tick == 1
        ));
        assert!(matches!(
            get_bundle_member_events(&message_serial_outs)[..],
            [
                MemberEventSerialOut::Joined { member_id: 0, .. },
                MemberEventSerialOut::SpawnRequested { member_id: 0, .. }
            ]
        ));
        assert_eq!(test_game.game_task.state.alive_organism_count(), 1);
    }

    #[test]
    fn handle_request_join_snapshots_the_state_of_the_last_completed_tick() {
        let mut test_game: TestGame = create_test_game(8);
        let (_, _first_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "First");
        run_ticks(&mut test_game.game_task, 3);

        let (reply, mut membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 2, "Second");
        let message_serial_outs: Vec<MessageSerialOut> = receive_messages(&mut membership);

        assert_eq!(reply, Ok(MemberId(1)));
        assert!(matches!(
            &message_serial_outs[1],
            MessageSerialOut::Snapshot(GameSnapshotSerialOut { state, .. }) if state.tick == 3 && state.members.len() == 1
        ));
    }

    #[test]
    fn handle_request_join_answers_a_rejection_without_queueing_frames() {
        let mut test_game: TestGame = create_test_game(8);
        let (_, _first_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "Blob");

        let (reply, mut membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 2, "Blob");

        assert_eq!(reply, Err(RejectionKind::ScreenNameTaken));
        assert_eq!(receive_messages(&mut membership), Vec::new());
    }

    #[test]
    fn run_tick_sends_the_state_checksum_on_its_interval() {
        let mut test_game: TestGame = create_test_game(8);
        let (_, mut membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "Blob");

        run_ticks(&mut test_game.game_task, CHECKSUM_INTERVAL_TICKS);
        let checksum_messages: Vec<MessageSerialOut> = receive_messages(&mut membership)
            .into_iter()
            .filter(|message_serial_out| matches!(message_serial_out, MessageSerialOut::StateChecksum { .. }))
            .collect();

        assert_eq!(
            checksum_messages,
            vec![MessageSerialOut::StateChecksum {
                tick: CHECKSUM_INTERVAL_TICKS,
                checksum: state_checksum::get_state_checksum(&test_game.game_task.state),
            }],
        );
    }

    #[test]
    fn handle_request_leave_queues_left_and_stops_the_frames() {
        let mut test_game: TestGame = create_test_game(8);
        let (_, mut leaving_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "Leaver");
        let (_, mut staying_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 2, "Stayer");
        test_game.game_task.run_tick(Instant::now());
        receive_messages(&mut leaving_membership);
        receive_messages(&mut staying_membership);

        let (reply_sender, mut reply_receiver): (oneshot::Sender<()>, oneshot::Receiver<()>) = oneshot::channel();
        test_game.game_task.handle_request(
            GameRequest::Leave {
                member_id: MemberId(0),
                reply: reply_sender,
            },
            Instant::now(),
        );
        test_game.game_task.run_tick(Instant::now());

        assert_eq!(reply_receiver.try_recv(), Ok(()));
        assert_eq!(receive_messages(&mut leaving_membership), Vec::new());
        assert_eq!(
            get_bundle_member_events(&receive_messages(&mut staying_membership)),
            vec![MemberEventSerialOut::Left { member_id: 0 }],
        );
    }

    #[test]
    fn handle_request_connection_closed_queues_left_for_its_member() {
        let mut test_game: TestGame = create_test_game(8);
        let (_, _leaving_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "Leaver");
        let (_, mut staying_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 2, "Stayer");
        test_game.game_task.run_tick(Instant::now());
        receive_messages(&mut staying_membership);

        test_game.game_task.handle_request(
            GameRequest::ConnectionClosed {
                connection_id: ConnectionId(1),
            },
            Instant::now(),
        );
        test_game.game_task.run_tick(Instant::now());

        assert_eq!(
            get_bundle_member_events(&receive_messages(&mut staying_membership)),
            vec![MemberEventSerialOut::Left { member_id: 0 }],
        );
    }

    #[test]
    fn run_tick_queues_left_for_a_member_whose_connection_is_gone() {
        let mut test_game: TestGame = create_test_game(8);
        let (_, leaving_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "Leaver");
        let (_, mut staying_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 2, "Stayer");
        test_game.game_task.run_tick(Instant::now());
        receive_messages(&mut staying_membership);

        drop(leaving_membership);
        run_ticks(&mut test_game.game_task, 2);

        assert_eq!(
            get_bundle_member_events(&receive_messages(&mut staying_membership)),
            vec![MemberEventSerialOut::Left { member_id: 0 }],
        );
        assert_eq!(test_game.game_task.state.members.len(), 1);
    }

    #[test]
    fn handle_input_closes_a_connection_whose_client_tick_is_ahead_of_the_server() {
        let mut test_game: TestGame = create_test_game(8);
        let (_, membership): (Result<MemberId, RejectionKind>, Membership) = join(&mut test_game.game_task, 1, "Blob");
        test_game.game_task.run_tick(Instant::now());

        test_game.game_task.handle_input(GameInput {
            member_id: MemberId(0),
            input: PlayerInput {
                client_tick: Tick(2),
                cursor: SubpixelPoint { x: 0, y: 0 },
                ability_presses: AbilityPressSet::NONE,
                aim: None,
            },
        });
        test_game.game_task.run_tick(Instant::now());

        assert_eq!(
            *membership.control.borrow(),
            ConnectionControl::Close(CloseCauseKind::ProtocolViolation(ProtocolViolationKind::Validation)),
        );
        assert!(test_game.game_task.state.members.is_empty());
    }

    #[test]
    fn run_tick_sends_a_cursor_correction_only_to_the_clamped_player() {
        let mut test_game: TestGame = create_test_game(8);
        let (_, mut clamped_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "Fast");
        let (_, mut other_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 2, "Other");
        test_game.game_task.run_tick(Instant::now());
        receive_messages(&mut clamped_membership);
        receive_messages(&mut other_membership);

        test_game.game_task.handle_input(GameInput {
            member_id: MemberId(0),
            input: PlayerInput {
                client_tick: Tick(1),
                cursor: SubpixelPoint { x: 0, y: 0 },
                ability_presses: AbilityPressSet::NONE,
                aim: None,
            },
        });
        test_game.game_task.run_tick(Instant::now());

        assert!(
            receive_messages(&mut clamped_membership).iter().any(|message_serial_out| matches!(
                message_serial_out,
                MessageSerialOut::CursorCorrected { client_tick: 1, .. }
            ))
        );
        assert!(
            !receive_messages(&mut other_membership)
                .iter()
                .any(|message_serial_out| matches!(message_serial_out, MessageSerialOut::CursorCorrected { .. }))
        );
    }

    #[test]
    fn handle_request_request_snapshot_sends_one_snapshot_after_the_minimum_interval() {
        let mut test_game: TestGame = create_test_game(8);
        let (_, mut membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "Blob");
        receive_messages(&mut membership);

        for _ in 0..2 {
            test_game
                .game_task
                .handle_request(GameRequest::RequestSnapshot { member_id: MemberId(0) }, Instant::now());
        }

        run_ticks(&mut test_game.game_task, 40);
        let snapshot_ticks: Vec<u32> = receive_messages(&mut membership)
            .iter()
            .filter_map(|message_serial_out| match message_serial_out {
                MessageSerialOut::Snapshot(snapshot_serial_out) => Some(snapshot_serial_out.state.tick),
                _ => None,
            })
            .collect();

        assert_eq!(snapshot_ticks, vec![29]);
    }

    #[test]
    fn handle_request_respawn_rejects_a_member_without_a_connection() {
        let mut test_game: TestGame = create_test_game(8);
        let (reply_sender, mut reply_receiver): (
            oneshot::Sender<Result<(), RejectionKind>>,
            oneshot::Receiver<Result<(), RejectionKind>>,
        ) = oneshot::channel();

        test_game.game_task.handle_request(
            GameRequest::Respawn {
                member_id: MemberId(0),
                loadout: test_fixture::create_loadout(),
                team: TeamChoiceKind::Auto,
                reply: reply_sender,
            },
            Instant::now(),
        );

        assert_eq!(reply_receiver.try_recv(), Ok(Err(RejectionKind::NotInGame)));
    }

    #[test]
    fn handle_request_spectate_adds_a_spectator() {
        let mut test_game: TestGame = create_test_game(8);
        let (endpoint, _membership): (ConnectionEndpoint, Membership) =
            connection_model::create_membership(ConnectionId(1));
        let (reply_sender, mut reply_receiver): (
            oneshot::Sender<Result<MemberId, RejectionKind>>,
            oneshot::Receiver<Result<MemberId, RejectionKind>>,
        ) = oneshot::channel();

        test_game.game_task.handle_request(
            GameRequest::Spectate {
                endpoint,
                screen_name: String::from("Watcher"),
                reply: reply_sender,
            },
            Instant::now(),
        );
        test_game.game_task.run_tick(Instant::now());

        assert_eq!(reply_receiver.try_recv(), Ok(Ok(MemberId(0))));
        assert_eq!(
            test_game.game_task.state.members[&MemberId(0)].role,
            MemberRoleKind::Spectator
        );
    }

    #[test]
    fn run_tick_updates_the_summary_when_it_changes() {
        let mut test_game: TestGame = create_test_game(8);
        let (_, _first_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "Blob");
        test_game.summary.mark_unchanged();

        test_game.game_task.run_tick(Instant::now());

        assert!(test_game.summary.has_changed().unwrap());
        assert_eq!(
            GameSummarySerialOut::from(&*test_game.summary.borrow_and_update()).player_count,
            1,
        );

        test_game.game_task.run_tick(Instant::now());

        assert!(!test_game.summary.has_changed().unwrap());
    }

    #[test]
    fn get_flow_ends_an_empty_game_after_the_grace_period_and_a_join_cancels_it() {
        let mut test_game: TestGame = create_test_game(8);
        let start: Instant = Instant::now();
        test_game.game_task.run_tick(start);

        assert_eq!(
            test_game.game_task.get_flow(start + GRACE_PERIOD - Duration::from_millis(1)),
            GameFlow::Continue
        );
        assert_eq!(test_game.game_task.get_flow(start + GRACE_PERIOD), GameFlow::End);

        let (_, _first_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "Blob");

        assert_eq!(
            test_game.game_task.get_flow(start + GRACE_PERIOD * 2),
            GameFlow::Continue
        );
    }

    #[test]
    fn answer_closed_game_answers_game_not_found() {
        let (endpoint, _membership): (ConnectionEndpoint, Membership) =
            connection_model::create_membership(ConnectionId(1));
        let (reply_sender, mut reply_receiver): (
            oneshot::Sender<Result<MemberId, RejectionKind>>,
            oneshot::Receiver<Result<MemberId, RejectionKind>>,
        ) = oneshot::channel();

        answer_closed_game(GameRequest::Join {
            endpoint,
            joiner: test_fixture::create_joiner("Late", TeamChoiceKind::Auto),
            reply: reply_sender,
        });

        assert_eq!(reply_receiver.try_recv(), Ok(Err(RejectionKind::GameNotFound)));
    }

    #[test]
    fn observe_resets_after_more_than_the_threshold_of_consecutive_late_ticks() {
        let mut tick_lag: TickLag = TickLag::default();
        let tick_period: Duration = Duration::from_millis(70);
        let due_at: Instant = Instant::now();
        let late_fired_at: Instant = due_at + tick_period;

        for _ in 0..TICK_LAG_RESET_THRESHOLD {
            assert_eq!(tick_lag.observe(due_at, late_fired_at, tick_period), TickLagKind::Late);
        }

        assert_eq!(
            tick_lag.observe(due_at, late_fired_at, tick_period),
            TickLagKind::ResetDue
        );
        assert_eq!(tick_lag.observe(due_at, late_fired_at, tick_period), TickLagKind::Late);
        assert_eq!(
            tick_lag.observe(due_at, due_at + Duration::from_millis(69), tick_period),
            TickLagKind::OnTime
        );
    }
}
