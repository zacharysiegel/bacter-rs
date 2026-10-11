use crate::error::AppError;
use crate::game;
use crate::game::{GameId, GameState, GameSummary, InputBundle, SimulationEvent, StepError, Tick};
use crate::geometry::{SubpixelPoint, WorldPoint};
use crate::member::MemberId;
use crate::play::own_member_state;
use crate::play::{AbilityPressSample, FramePacer, OwnMemberStateKind, PlayerControls};
use crate::protocol;
use crate::protocol::{
    AimVectorSerial, CloseReasonKind, GameSnapshotSerialOut, GameSummarySerialOut, InputBundleSerialOut,
    MessageSerialIn, MessageSerialOut, PlayerInputSerialIn, RejectionKind, RequestKind, SubpixelPointSerial,
};
use crate::world::WorldBounds;

#[derive(Clone, Debug, PartialEq)]
pub enum ClientEffect {
    SendFrame(Vec<u8>),
    GameListReceived {
        game_summaries: Vec<GameSummary>,
        online_client_count: u32,
    },
    JoinAccepted {
        game_id: GameId,
        member_id: MemberId,
    },
    RequestRejected {
        request: RequestKind,
        reason: RejectionKind,
    },
    OwnMemberStateChanged(OwnMemberStateKind),
    OwnOrganismSpawned {
        cursor: WorldPoint,
    },
    OwnSpawnRejected,
    TickApplied {
        tick: Tick,
    },
    ResyncStarted,
    ResyncFinished,
    LeftGame,
    GameEnded,
    SynchronisationLost,
    /// `None` for an abnormal close, a failed open, or a close code no close reason uses.
    ConnectionLost(Option<CloseReasonKind>),
}

/// One per WebSocket connection.
#[derive(Clone, Debug, PartialEq)]
pub struct ClientSession {
    phase: SessionPhase,
    pacer: FramePacer,
}

impl ClientSession {
    pub fn new() -> ClientSession {
        ClientSession {
            phase: SessionPhase::Lobby,
            pacer: FramePacer::new(),
        }
    }

    pub fn receive(&mut self, frame: &[u8], controls: &mut PlayerControls, now_milliseconds: f64) -> Vec<ClientEffect> {
        let decoded: Result<MessageSerialOut, AppError> = protocol::decode_message_out(frame);

        match decoded {
            Ok(message_serial_out) => self.receive_message(message_serial_out, controls, now_milliseconds),
            Err(error) => {
                log::warn!("server frame dropped; [error={error}]");

                Vec::new()
            }
        }
    }

    pub fn send_request(&mut self, request: MessageSerialIn) -> Vec<ClientEffect> {
        vec![ClientEffect::SendFrame(protocol::encode_message_in(&request))]
    }

    pub fn connection_closed(&mut self, close_code: u16) -> Vec<ClientEffect> {
        self.phase = SessionPhase::Lobby;

        vec![ClientEffect::ConnectionLost(CloseReasonKind::from_close_code(
            close_code,
        ))]
    }

    /// `None` outside a game and until the first snapshot.
    pub fn render_state(&self) -> Option<&GameState> {
        self.get_game_session()?.confirmed_state.as_ref()
    }

    /// `None` outside a game and until the own member's `Joined` has been applied.
    pub fn own_member_state(&self) -> Option<OwnMemberStateKind> {
        self.get_game_session()?.own_member_state
    }

    pub fn game_id(&self) -> Option<GameId> {
        self.get_game_session().map(|game_session| game_session.game_id)
    }

    pub fn own_member_id(&self) -> Option<MemberId> {
        self.get_game_session().map(|game_session| game_session.member_id)
    }

    fn get_game_session(&self) -> Option<&GameSession> {
        match &self.phase {
            SessionPhase::InGame(game_session) => Some(game_session),
            SessionPhase::Lobby => None,
        }
    }

    fn receive_message(
        &mut self,
        message_serial_out: MessageSerialOut,
        controls: &mut PlayerControls,
        now_milliseconds: f64,
    ) -> Vec<ClientEffect> {
        match message_serial_out {
            MessageSerialOut::GameList {
                game_summaries,
                online_client_count,
            } => receive_game_list(game_summaries, online_client_count),
            MessageSerialOut::JoinAccepted { game_id, member_id } => {
                self.enter_game(GameId(game_id), MemberId(member_id))
            }
            MessageSerialOut::Snapshot(snapshot_serial_out) => {
                self.receive_snapshot(snapshot_serial_out, controls, now_milliseconds)
            }
            MessageSerialOut::InputBundle(bundle_serial_out) => {
                self.receive_bundle(bundle_serial_out, controls, now_milliseconds)
            }
            MessageSerialOut::StateChecksum { .. } => Vec::new(),
            MessageSerialOut::CursorCorrected { client_tick, cursor } => {
                self.receive_cursor_correction(Tick(client_tick), cursor, controls)
            }
            MessageSerialOut::RequestRejected { request, reason } => vec![ClientEffect::RequestRejected {
                request: RequestKind::from(request),
                reason: RejectionKind::from(reason),
            }],
            MessageSerialOut::LeftGame => self.leave_game(ClientEffect::LeftGame),
            MessageSerialOut::GameEnded => self.leave_game(ClientEffect::GameEnded),
        }
    }

    fn enter_game(&mut self, game_id: GameId, member_id: MemberId) -> Vec<ClientEffect> {
        self.phase = SessionPhase::InGame(GameSession::new(game_id, member_id));

        vec![ClientEffect::JoinAccepted { game_id, member_id }]
    }

    fn receive_snapshot(
        &mut self,
        snapshot_serial_out: GameSnapshotSerialOut,
        controls: &mut PlayerControls,
        now_milliseconds: f64,
    ) -> Vec<ClientEffect> {
        let SessionPhase::InGame(game_session) = &mut self.phase else {
            return Vec::new();
        };

        if GameId(snapshot_serial_out.game_id) != game_session.game_id {
            log::warn!(
                "snapshot of another game dropped; [game_id={}]",
                snapshot_serial_out.game_id
            );

            return Vec::new();
        }

        let converted: Result<GameState, AppError> = GameState::try_from(snapshot_serial_out.state);

        let state: GameState = match converted {
            Ok(state) => state,
            Err(error) => {
                log::warn!("snapshot dropped; [error={error}]");

                return Vec::new();
            }
        };

        self.pacer.record_tick_applied(now_milliseconds);

        game_session.apply_snapshot(state, controls)
    }

    fn receive_bundle(
        &mut self,
        bundle_serial_out: InputBundleSerialOut,
        controls: &mut PlayerControls,
        now_milliseconds: f64,
    ) -> Vec<ClientEffect> {
        let SessionPhase::InGame(game_session) = &mut self.phase else {
            return Vec::new();
        };

        let converted: Result<InputBundle, AppError> = InputBundle::try_from(bundle_serial_out);

        let bundle: InputBundle = match converted {
            Ok(bundle) => bundle,
            Err(error) => {
                log::warn!("bundle dropped; [error={error}]");

                return Vec::new();
            }
        };

        let applied: Result<Vec<ClientEffect>, StepError> = game_session.apply_bundle(&bundle, controls);

        match applied {
            Ok(effects) => {
                self.pacer.record_tick_applied(now_milliseconds);

                effects
            }
            Err(step_error) => {
                log::warn!(
                    "bundle could not be stepped; [tick={} error={step_error:?}]",
                    bundle.tick.0
                );

                Vec::new()
            }
        }
    }

    fn receive_cursor_correction(
        &mut self,
        client_tick: Tick,
        cursor_serial: SubpixelPointSerial,
        controls: &mut PlayerControls,
    ) -> Vec<ClientEffect> {
        if self.get_game_session().is_none() {
            return Vec::new();
        }

        let converted: Result<SubpixelPoint, AppError> = SubpixelPoint::try_from(cursor_serial);

        match converted {
            Ok(cursor) => controls.crosshair.apply_correction(client_tick, cursor),
            Err(error) => log::warn!("cursor correction dropped; [error={error}]"),
        }

        Vec::new()
    }

    /// `leave_effect` only when a game was left.
    fn leave_game(&mut self, leave_effect: ClientEffect) -> Vec<ClientEffect> {
        let previous_phase: SessionPhase = std::mem::replace(&mut self.phase, SessionPhase::Lobby);

        match previous_phase {
            SessionPhase::InGame(_) => vec![leave_effect],
            SessionPhase::Lobby => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum SessionPhase {
    Lobby,
    InGame(GameSession),
}

#[derive(Clone, Debug, PartialEq)]
struct GameSession {
    game_id: GameId,
    member_id: MemberId,
    /// `None` until the first snapshot.
    confirmed_state: Option<GameState>,
    own_member_state: Option<OwnMemberStateKind>,
}

impl GameSession {
    fn new(game_id: GameId, member_id: MemberId) -> GameSession {
        GameSession {
            game_id,
            member_id,
            confirmed_state: None,
            own_member_state: None,
        }
    }

    fn apply_snapshot(&mut self, state: GameState, controls: &mut PlayerControls) -> Vec<ClientEffect> {
        let is_first_snapshot: bool = self.confirmed_state.is_none();

        if is_first_snapshot {
            controls.crosshair.snap_to(get_world_center(&state));
        }

        self.confirmed_state = Some(state);

        self.report_own_member_state().into_iter().collect()
    }

    /// Bundles before the first snapshot are discarded.
    fn apply_bundle(
        &mut self,
        bundle: &InputBundle,
        controls: &mut PlayerControls,
    ) -> Result<Vec<ClientEffect>, StepError> {
        let Some(state) = self.confirmed_state.as_mut() else {
            return Ok(Vec::new());
        };

        let simulation_events: Vec<SimulationEvent> = game::step(state, bundle)?;
        let mut effects: Vec<ClientEffect> = vec![ClientEffect::TickApplied { tick: bundle.tick }];

        for simulation_event in &simulation_events {
            let own_event_effect: Option<ClientEffect> = self.handle_own_simulation_event(simulation_event, controls);
            effects.extend(own_event_effect);
        }

        effects.extend(self.report_own_member_state());
        effects.extend(self.sample_input(controls));

        Ok(effects)
    }

    fn handle_own_simulation_event(
        &self,
        simulation_event: &SimulationEvent,
        controls: &mut PlayerControls,
    ) -> Option<ClientEffect> {
        match simulation_event {
            SimulationEvent::OrganismSpawned { member_id, cursor } if *member_id == self.member_id => {
                controls.crosshair.snap_to(cursor.to_subpixel_point());

                Some(ClientEffect::OwnOrganismSpawned { cursor: *cursor })
            }
            SimulationEvent::SpawnRejected { member_id, .. } if *member_id == self.member_id => {
                Some(ClientEffect::OwnSpawnRejected)
            }
            SimulationEvent::MemberJoined { .. }
            | SimulationEvent::MemberLeft { .. }
            | SimulationEvent::OrganismSpawned { .. }
            | SimulationEvent::SpawnRejected { .. }
            | SimulationEvent::OrganismDied { .. }
            | SimulationEvent::RoundPhaseChanged { .. }
            | SimulationEvent::RoundWon { .. }
            | SimulationEvent::EffectApplied { .. } => None,
        }
    }

    fn report_own_member_state(&mut self) -> Option<ClientEffect> {
        let own_member_state: Option<OwnMemberStateKind> = self
            .confirmed_state
            .as_ref()
            .and_then(|state| own_member_state::get_own_member_state(state, self.member_id));

        if own_member_state == self.own_member_state {
            return None;
        }

        self.own_member_state = own_member_state;

        own_member_state.map(ClientEffect::OwnMemberStateChanged)
    }

    /// Only while the own member has an organism; presses made without one are discarded.
    fn sample_input(&self, controls: &mut PlayerControls) -> Option<ClientEffect> {
        let state: &GameState = self.confirmed_state.as_ref()?;
        let has_organism: bool = state.members.get(&self.member_id).is_some_and(|member| member.organism.is_some());

        if !has_organism {
            controls.keyboard.discard_ability_presses();

            return None;
        }

        let sample: AbilityPressSample = controls.keyboard.take_ability_presses();
        let cursor: SubpixelPoint = controls.crosshair.position();
        controls.crosshair.record_sent(state.tick, cursor);

        let input: MessageSerialIn = MessageSerialIn::Input(PlayerInputSerialIn {
            client_tick: state.tick.0,
            cursor: SubpixelPointSerial::from(&cursor),
            ability_presses: sample.ability_presses.bits(),
            aim: sample.aim.as_ref().map(AimVectorSerial::from),
        });

        Some(ClientEffect::SendFrame(protocol::encode_message_in(&input)))
    }
}

/// An unconvertible summary drops the whole list.
fn receive_game_list(
    game_summary_serial_outs: Vec<GameSummarySerialOut>,
    online_client_count: u32,
) -> Vec<ClientEffect> {
    let converted: Result<Vec<GameSummary>, AppError> =
        game_summary_serial_outs.into_iter().map(GameSummary::try_from).collect();

    match converted {
        Ok(game_summaries) => vec![ClientEffect::GameListReceived {
            game_summaries,
            online_client_count,
        }],
        Err(error) => {
            log::warn!("game list dropped; [error={error}]");

            Vec::new()
        }
    }
}

fn get_world_center(state: &GameState) -> SubpixelPoint {
    let bounds: &WorldBounds = &state.world.bounds;

    SubpixelPoint {
        x: bounds.left.0 + bounds.width.0 / 2,
        y: bounds.top.0 + bounds.height.0 / 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::AbilityPressSet;
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry;
    use crate::member::Member;
    use crate::play::{GameKeyKind, KeyRepeatKind, KeyTransitionKind};
    use crate::protocol::{
        GameModeKindSerial, GameStateSerialOut, LoadoutSerial, MemberEventSerialOut, MemberRoleKindSerialOut,
        RejectionKindSerialOut, RequestKindSerialOut, SettingFieldKind, SettingFieldKindSerialOut,
    };
    use crate::world::WorldShapeKind;

    const GAME_ID: GameId = GameId(7);
    const OWN_MEMBER_ID: MemberId = MemberId(2);
    const NOW_MILLISECONDS: f64 = 1000.0;

    const OTHER_MEMBER_ID: MemberId = MemberId(1);

    fn receive_message(session: &mut ClientSession, message_serial_out: &MessageSerialOut) -> Vec<ClientEffect> {
        let mut controls: PlayerControls = PlayerControls::new();

        receive_with_controls(session, message_serial_out, &mut controls)
    }

    fn receive_with_controls(
        session: &mut ClientSession,
        message_serial_out: &MessageSerialOut,
        controls: &mut PlayerControls,
    ) -> Vec<ClientEffect> {
        let frame: Vec<u8> = protocol::encode_message_out(message_serial_out);

        session.receive(&frame, controls, NOW_MILLISECONDS)
    }

    /// The tick-0 state of a new game, as the server sends it to a creator.
    fn create_server_state() -> GameState {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        state.next_member_id = OWN_MEMBER_ID;

        state
    }

    fn create_server_state_with_own_member(own_member: Member) -> GameState {
        let mut state: GameState = create_server_state();
        state.members.insert(OWN_MEMBER_ID, own_member);
        state.next_member_id = OWN_MEMBER_ID.next();

        state
    }

    fn create_alive_own_member() -> Member {
        test_fixture::create_participant_with_organism(OWN_MEMBER_ID, WorldPoint { x: 300, y: 200 })
    }

    fn create_snapshot_message(state: &GameState) -> MessageSerialOut {
        MessageSerialOut::Snapshot(GameSnapshotSerialOut {
            game_id: GAME_ID.0,
            state: GameStateSerialOut::from(state),
        })
    }

    fn create_bundle_message(tick: Tick, member_event_serial_outs: Vec<MemberEventSerialOut>) -> MessageSerialOut {
        MessageSerialOut::InputBundle(InputBundleSerialOut {
            tick: tick.0,
            member_events: member_event_serial_outs,
            player_inputs: Vec::new(),
        })
    }

    fn create_own_join_events(role: MemberRoleKindSerialOut) -> Vec<MemberEventSerialOut> {
        let loadout_serial: LoadoutSerial = LoadoutSerial::from(&test_fixture::create_loadout());
        let joined_event_serial_out: MemberEventSerialOut = MemberEventSerialOut::Joined {
            member_id: OWN_MEMBER_ID.0,
            screen_name: String::from("Blob"),
            role,
            loadout: (role == MemberRoleKindSerialOut::Participant).then_some(loadout_serial),
            team: None,
        };

        match role {
            MemberRoleKindSerialOut::Participant => vec![
                joined_event_serial_out,
                MemberEventSerialOut::SpawnRequested {
                    member_id: OWN_MEMBER_ID.0,
                    loadout: loadout_serial,
                    team: None,
                },
            ],
            MemberRoleKindSerialOut::Spectator => vec![joined_event_serial_out],
        }
    }

    /// Joined, with the snapshot applied.
    fn create_session_with_snapshot(state: &GameState, controls: &mut PlayerControls) -> ClientSession {
        let mut session: ClientSession = create_joined_session();
        receive_with_controls(&mut session, &create_snapshot_message(state), controls);

        session
    }

    fn get_sent_messages(effects: &[ClientEffect]) -> Vec<MessageSerialIn> {
        effects
            .iter()
            .filter_map(|effect| match effect {
                ClientEffect::SendFrame(frame) => Some(protocol::decode_message_in(frame).unwrap()),
                _ => None,
            })
            .collect()
    }

    fn press(controls: &mut PlayerControls, key: GameKeyKind) {
        controls.keyboard.key_changed(key, KeyTransitionKind::Pressed, KeyRepeatKind::Initial);
    }

    fn create_joined_session() -> ClientSession {
        let mut session: ClientSession = ClientSession::new();
        receive_message(
            &mut session,
            &MessageSerialOut::JoinAccepted {
                game_id: GAME_ID.0,
                member_id: OWN_MEMBER_ID.0,
            },
        );

        session
    }

    fn create_game_summary_serial_out(title: &str) -> GameSummarySerialOut {
        GameSummarySerialOut {
            game_id: GAME_ID.0,
            title: String::from(title),
            mode: GameModeKindSerial::FreeForAll,
            player_count: 3,
            spectator_count: 1,
            player_cap: 8,
            secured: true,
            team_sizes: Vec::new(),
        }
    }

    #[test]
    fn receive_converts_a_game_list() {
        let mut session: ClientSession = ClientSession::new();

        let effects: Vec<ClientEffect> = receive_message(
            &mut session,
            &MessageSerialOut::GameList {
                game_summaries: vec![create_game_summary_serial_out("Arena")],
                online_client_count: 12,
            },
        );

        assert_eq!(
            effects,
            vec![ClientEffect::GameListReceived {
                game_summaries: vec![GameSummary {
                    game_id: GAME_ID,
                    title: String::from("Arena"),
                    mode: GameModeKind::FreeForAll,
                    player_count: 3,
                    spectator_count: 1,
                    player_cap: 8,
                    secured: true,
                    team_sizes: Vec::new(),
                }],
                online_client_count: 12,
            }],
        );
    }

    #[test]
    fn receive_drops_a_game_list_with_an_unconvertible_summary() {
        let mut session: ClientSession = ClientSession::new();

        let effects: Vec<ClientEffect> = receive_message(
            &mut session,
            &MessageSerialOut::GameList {
                game_summaries: vec![
                    create_game_summary_serial_out("Arena"),
                    create_game_summary_serial_out(""),
                ],
                online_client_count: 12,
            },
        );

        assert_eq!(effects, Vec::<ClientEffect>::new());
    }

    #[test]
    fn receive_enters_the_game_on_join_accepted() {
        let mut session: ClientSession = ClientSession::new();

        let effects: Vec<ClientEffect> = receive_message(
            &mut session,
            &MessageSerialOut::JoinAccepted {
                game_id: GAME_ID.0,
                member_id: OWN_MEMBER_ID.0,
            },
        );

        assert_eq!(
            effects,
            vec![ClientEffect::JoinAccepted {
                game_id: GAME_ID,
                member_id: OWN_MEMBER_ID,
            }],
        );
        assert_eq!(session.game_id(), Some(GAME_ID));
        assert_eq!(session.own_member_id(), Some(OWN_MEMBER_ID));
    }

    #[test]
    fn receive_passes_on_a_rejected_request() {
        let mut session: ClientSession = ClientSession::new();

        let effects: Vec<ClientEffect> = receive_message(
            &mut session,
            &MessageSerialOut::RequestRejected {
                request: RequestKindSerialOut::CreateGame,
                reason: RejectionKindSerialOut::SettingNotApplicable {
                    field: SettingFieldKindSerialOut::TeamCount,
                },
            },
        );

        assert_eq!(
            effects,
            vec![ClientEffect::RequestRejected {
                request: RequestKind::CreateGame,
                reason: RejectionKind::SettingNotApplicable {
                    field: SettingFieldKind::TeamCount,
                },
            }],
        );
    }

    #[test]
    fn receive_returns_to_the_lobby_when_the_game_is_left_or_ended() {
        let mut left_session: ClientSession = create_joined_session();
        let mut ended_session: ClientSession = create_joined_session();

        let left_effects: Vec<ClientEffect> = receive_message(&mut left_session, &MessageSerialOut::LeftGame);
        let ended_effects: Vec<ClientEffect> = receive_message(&mut ended_session, &MessageSerialOut::GameEnded);

        assert_eq!(left_effects, vec![ClientEffect::LeftGame]);
        assert_eq!(ended_effects, vec![ClientEffect::GameEnded]);
        assert_eq!(left_session.own_member_id(), None);
        assert_eq!(ended_session.game_id(), None);
    }

    #[test]
    fn receive_ignores_left_game_in_the_lobby() {
        let mut session: ClientSession = ClientSession::new();

        assert_eq!(
            receive_message(&mut session, &MessageSerialOut::LeftGame),
            Vec::<ClientEffect>::new(),
        );
    }

    #[test]
    fn receive_drops_an_undecodable_frame_in_the_lobby() {
        let mut session: ClientSession = ClientSession::new();
        let mut controls: PlayerControls = PlayerControls::new();

        let effects: Vec<ClientEffect> = session.receive(&[0xff, 0xff, 0xff], &mut controls, NOW_MILLISECONDS);

        assert_eq!(effects, Vec::<ClientEffect>::new());
    }

    #[test]
    fn send_request_encodes_the_request() {
        let mut session: ClientSession = ClientSession::new();

        let effects: Vec<ClientEffect> = session.send_request(MessageSerialIn::SubscribeGameList);

        assert_eq!(
            effects,
            vec![ClientEffect::SendFrame(protocol::encode_message_in(
                &MessageSerialIn::SubscribeGameList
            ))],
        );
    }

    #[test]
    fn connection_closed_reports_the_close_reason_and_leaves_the_game() {
        let mut session: ClientSession = create_joined_session();

        assert_eq!(
            session.connection_closed(CloseReasonKind::SlowConsumer.close_code()),
            vec![ClientEffect::ConnectionLost(Some(CloseReasonKind::SlowConsumer))],
        );
        assert_eq!(session.own_member_id(), None);
        assert_eq!(
            session.connection_closed(1006),
            vec![ClientEffect::ConnectionLost(None)],
        );
    }

    #[test]
    fn receive_applies_the_snapshot_then_the_bundles_after_it() {
        let mut controls: PlayerControls = PlayerControls::new();
        let mut server_state: GameState = create_server_state();
        let mut session: ClientSession = create_session_with_snapshot(&server_state, &mut controls);
        let bundle_messages: Vec<MessageSerialOut> = vec![
            create_bundle_message(Tick(1), create_own_join_events(MemberRoleKindSerialOut::Participant)),
            create_bundle_message(Tick(2), Vec::new()),
        ];
        let mut applied_ticks: Vec<Tick> = Vec::new();

        for bundle_message in &bundle_messages {
            let MessageSerialOut::InputBundle(bundle_serial_out) = bundle_message else {
                unreachable!();
            };

            game::step(
                &mut server_state,
                &InputBundle::try_from(bundle_serial_out.clone()).unwrap(),
            )
            .unwrap();

            let effects: Vec<ClientEffect> = receive_with_controls(&mut session, bundle_message, &mut controls);
            applied_ticks.extend(effects.iter().filter_map(|effect| match effect {
                ClientEffect::TickApplied { tick } => Some(*tick),
                _ => None,
            }));
        }

        assert_eq!(applied_ticks, vec![Tick(1), Tick(2)]);
        assert_eq!(session.render_state(), Some(&server_state));
    }

    #[test]
    fn receive_places_the_crosshair_at_the_world_center_on_the_first_snapshot() {
        let mut controls: PlayerControls = PlayerControls::new();

        create_session_with_snapshot(&create_server_state(), &mut controls);

        let center_subpixels: i32 = 400 * geometry::SUBPIXELS_PER_PIXEL;

        assert_eq!(
            controls.crosshair.position(),
            SubpixelPoint {
                x: center_subpixels,
                y: center_subpixels,
            },
        );
    }

    #[test]
    fn receive_sends_no_input_after_a_snapshot_alone() {
        let mut session: ClientSession = create_joined_session();
        let state: GameState = create_server_state_with_own_member(create_alive_own_member());

        let effects: Vec<ClientEffect> = receive_message(&mut session, &create_snapshot_message(&state));

        assert_eq!(
            effects,
            vec![ClientEffect::OwnMemberStateChanged(
                OwnMemberStateKind::AliveParticipant
            )],
        );
    }

    #[test]
    fn receive_ignores_a_snapshot_of_another_game() {
        let mut session: ClientSession = create_joined_session();

        receive_message(
            &mut session,
            &MessageSerialOut::Snapshot(GameSnapshotSerialOut {
                game_id: GAME_ID.0 + 1,
                state: GameStateSerialOut::from(&create_server_state()),
            }),
        );

        assert_eq!(session.render_state(), None);
    }

    #[test]
    fn receive_samples_the_controls_into_an_input_after_each_applied_bundle() {
        let mut controls: PlayerControls = PlayerControls::new();
        let state: GameState = create_server_state_with_own_member(create_alive_own_member());
        let mut session: ClientSession = create_session_with_snapshot(&state, &mut controls);
        let cursor: SubpixelPoint = SubpixelPoint { x: 5000, y: 6000 };
        controls.crosshair.snap_to(cursor);
        press(&mut controls, GameKeyKind::ThirdAbility);

        let effects: Vec<ClientEffect> =
            receive_with_controls(&mut session, &create_bundle_message(Tick(1), Vec::new()), &mut controls);

        assert_eq!(
            get_sent_messages(&effects),
            vec![MessageSerialIn::Input(PlayerInputSerialIn {
                client_tick: 1,
                cursor: SubpixelPointSerial::from(&cursor),
                ability_presses: AbilityPressSet::THIRD.bits(),
                aim: None,
            })],
        );
        assert_eq!(controls.keyboard.pending_ability_presses(), AbilityPressSet::NONE);
    }

    #[test]
    fn receive_discards_the_presses_made_without_an_organism() {
        let mut controls: PlayerControls = PlayerControls::new();
        let state: GameState = create_server_state_with_own_member(test_fixture::create_participant(OWN_MEMBER_ID));
        let mut session: ClientSession = create_session_with_snapshot(&state, &mut controls);
        press(&mut controls, GameKeyKind::FirstAbility);

        let effects: Vec<ClientEffect> =
            receive_with_controls(&mut session, &create_bundle_message(Tick(1), Vec::new()), &mut controls);

        assert_eq!(get_sent_messages(&effects), Vec::<MessageSerialIn>::new());
        assert_eq!(controls.keyboard.pending_ability_presses(), AbilityPressSet::NONE);
    }

    #[test]
    fn receive_snaps_the_crosshair_to_the_own_spawn() {
        let mut controls: PlayerControls = PlayerControls::new();
        let mut session: ClientSession = create_session_with_snapshot(&create_server_state(), &mut controls);

        let effects: Vec<ClientEffect> = receive_with_controls(
            &mut session,
            &create_bundle_message(Tick(1), create_own_join_events(MemberRoleKindSerialOut::Participant)),
            &mut controls,
        );

        let own_cursor: WorldPoint =
            session.render_state().unwrap().members[&OWN_MEMBER_ID].organism.as_ref().unwrap().cursor;

        assert!(effects.contains(&ClientEffect::OwnOrganismSpawned { cursor: own_cursor }));
        assert!(effects.contains(&ClientEffect::OwnMemberStateChanged(
            OwnMemberStateKind::AliveParticipant
        )));
        assert_eq!(controls.crosshair.position(), own_cursor.to_subpixel_point());
        assert!(matches!(
            get_sent_messages(&effects)[..],
            [MessageSerialIn::Input(PlayerInputSerialIn { cursor, .. })]
                if cursor == SubpixelPointSerial::from(&own_cursor.to_subpixel_point())
        ));
    }

    #[test]
    fn receive_reports_an_own_spawn_rejected() {
        let mut controls: PlayerControls = PlayerControls::new();
        let mut state: GameState = create_server_state();
        state.settings.player_cap = 2;

        for member_id in [MemberId(0), OTHER_MEMBER_ID] {
            let position: WorldPoint = WorldPoint {
                x: 100 + 200 * i32::try_from(member_id.0).unwrap(),
                y: 100,
            };
            state.members.insert(
                member_id,
                test_fixture::create_participant_with_organism(member_id, position),
            );
        }

        let mut session: ClientSession = create_session_with_snapshot(&state, &mut controls);

        let effects: Vec<ClientEffect> = receive_with_controls(
            &mut session,
            &create_bundle_message(Tick(1), create_own_join_events(MemberRoleKindSerialOut::Participant)),
            &mut controls,
        );

        assert!(effects.contains(&ClientEffect::OwnSpawnRejected));
        assert_eq!(session.own_member_state(), Some(OwnMemberStateKind::DeadParticipant));
    }

    #[test]
    fn receive_reports_the_own_member_state_only_when_it_changes() {
        let mut controls: PlayerControls = PlayerControls::new();
        let mut session: ClientSession = create_session_with_snapshot(&create_server_state(), &mut controls);

        let joined_effects: Vec<ClientEffect> = receive_with_controls(
            &mut session,
            &create_bundle_message(Tick(1), create_own_join_events(MemberRoleKindSerialOut::Spectator)),
            &mut controls,
        );
        let later_effects: Vec<ClientEffect> =
            receive_with_controls(&mut session, &create_bundle_message(Tick(2), Vec::new()), &mut controls);

        assert_eq!(
            joined_effects,
            vec![
                ClientEffect::TickApplied { tick: Tick(1) },
                ClientEffect::OwnMemberStateChanged(OwnMemberStateKind::Spectator),
            ],
        );
        assert_eq!(later_effects, vec![ClientEffect::TickApplied { tick: Tick(2) }]);
    }

    #[test]
    fn receive_moves_the_crosshair_by_a_cursor_correction() {
        let mut controls: PlayerControls = PlayerControls::new();
        let state: GameState = create_server_state_with_own_member(create_alive_own_member());
        let mut session: ClientSession = create_session_with_snapshot(&state, &mut controls);
        controls.crosshair.snap_to(SubpixelPoint { x: 9000, y: 0 });
        receive_with_controls(&mut session, &create_bundle_message(Tick(1), Vec::new()), &mut controls);

        receive_with_controls(
            &mut session,
            &MessageSerialOut::CursorCorrected {
                client_tick: 1,
                cursor: SubpixelPointSerial { x: 8000, y: 0 },
            },
            &mut controls,
        );

        assert_eq!(controls.crosshair.position(), SubpixelPoint { x: 8000, y: 0 });
    }

    #[test]
    fn receive_discards_a_bundle_before_the_first_snapshot() {
        let mut session: ClientSession = create_joined_session();

        let effects: Vec<ClientEffect> = receive_message(&mut session, &create_bundle_message(Tick(1), Vec::new()));

        assert_eq!(effects, Vec::<ClientEffect>::new());
    }
}
