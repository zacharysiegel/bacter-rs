mod helpers;

use std::collections::BTreeSet;

use shared::ability::{AbilityPhase, AbilityPressSet, AimVector, Loadout, OrganismAbilities, ShotPhase, SporePhase};
use shared::error::AppError;
use shared::game;
use shared::game::{
    GameModeKind, GameSettings, GameState, GameSummary, InputBundle, MemberEvent, PlayerInput, PlayerTickInput, Tick,
};
use shared::geometry::{SubpixelPoint, WorldPoint};
use shared::member::{Joiner, MemberId, MemberRoleKind};
use shared::organism::Organism;
use shared::protocol;
use shared::protocol::protocol_limits;
use shared::protocol::{
    AimVectorSerial, AppearanceSerial, FirstAbilityKindSerial, GameModeKindSerial, GameSettingsSerialIn,
    GameSnapshotSerialOut, GameStateSerialOut, GameSummarySerialOut, InputBundleSerialOut, JoinerSerialIn,
    LoadoutSerial, MessageSerialIn, MessageSerialOut, OrganismColorKindSerial, PlayerInputSerialIn, RejectionKind,
    RejectionKindSerialOut, RequestKindSerialOut, SecondAbilityKindSerial, SkinKindSerial, SubpixelPointSerial,
    TeamChoiceKindSerialIn, TeamKindSerial, ThirdAbilityKindSerial, WorldShapeKindSerial,
};
use shared::random::Pcg32;
use shared::replay;
use shared::replay::{ReplayLog, ReplayReadError};
use shared::round::RoundPhase;
use shared::world::WorldShapeKind;

use crate::helpers::replay_fixture;

const EXPECTED_PHASE_NAMES: [&str; 18] = [
    "ability active",
    "ability cooling",
    "ability ready",
    "compressed",
    "field center",
    "frozen",
    "round playing",
    "round post-round",
    "round pre-round",
    "round waiting",
    "shot cooling",
    "shot flying",
    "shot ready",
    "shot secreting",
    "spore cooling",
    "spore flying",
    "spore ready",
    "spore secreting",
];

const MALFORMED_INPUT_SEED_COUNT: u64 = 400;
const MALFORMED_FRAME_MUTATION_COUNT: usize = 4;
const MALFORMED_SNAPSHOT_TICK: Tick = Tick(2500);
const MALFORMED_REPLAY_BUNDLE_COUNT: usize = 20;
const RANDOM_FRAME_BYTE_LIMIT: u32 = 64;
const LOADOUT_SERIAL: LoadoutSerial = LoadoutSerial {
    appearance: AppearanceSerial {
        color: OrganismColorKindSerial::Lake,
        skin: SkinKindSerial::Grid,
    },
    first: FirstAbilityKindSerial::Compress,
    second: SecondAbilityKindSerial::Freeze,
    third: ThirdAbilityKindSerial::Toxin,
};
const SIZE_BUDGET_TICK: Tick = Tick(1_000_000);
const GROWTH_TICK_COUNT: u32 = 300;
const SNAPSHOT_SIZE_BUDGET_BYTES: usize = 32 * 1024;
const EIGHT_PLAYER_COUNT: u32 = 8;
const EIGHT_PLAYER_INPUT_BUNDLE_SIZE_BUDGET_BYTES: usize = 128;
const SIXTEEN_PLAYER_COUNT: u32 = 16;
const SIXTEEN_PLAYER_INPUT_BUNDLE_SIZE_BUDGET_BYTES: usize = 256;

#[derive(Debug)]
struct MalformedInputOutcomeCounts {
    decode_rejected: u32,
    conversion_rejected: u32,
    accepted: u32,
}

impl MalformedInputOutcomeCounts {
    fn new() -> MalformedInputOutcomeCounts {
        MalformedInputOutcomeCounts {
            decode_rejected: 0,
            conversion_rejected: 0,
            accepted: 0,
        }
    }

    fn record<T>(&mut self, decoded: Result<T, AppError>, convert: impl FnOnce(T) -> Result<(), AppError>) {
        let Ok(value): Result<T, AppError> = decoded else {
            self.decode_rejected += 1;
            return;
        };

        let conversion: Result<(), AppError> = convert(value);

        match conversion {
            Ok(()) => self.accepted += 1,
            Err(_) => self.conversion_rejected += 1,
        }
    }

    fn total(&self) -> u32 {
        self.decode_rejected + self.conversion_rejected + self.accepted
    }
}

fn get_ability_phase_name(ability_phase: &AbilityPhase) -> &'static str {
    match ability_phase {
        AbilityPhase::Ready => "ability ready",
        AbilityPhase::Active { .. } => "ability active",
        AbilityPhase::Cooling { .. } => "ability cooling",
    }
}

fn get_spore_phase_name(spore_phase: &SporePhase) -> &'static str {
    match spore_phase {
        SporePhase::Ready => "spore ready",
        SporePhase::Flying { .. } => "spore flying",
        SporePhase::Secreting { .. } => "spore secreting",
        SporePhase::Cooling { .. } => "spore cooling",
    }
}

fn get_shot_phase_name(shot_phase: &ShotPhase) -> &'static str {
    match shot_phase {
        ShotPhase::Ready => "shot ready",
        ShotPhase::Flying { .. } => "shot flying",
        ShotPhase::Secreting { .. } => "shot secreting",
        ShotPhase::Cooling { .. } => "shot cooling",
    }
}

fn get_round_phase_name(round_phase: &RoundPhase) -> &'static str {
    match round_phase {
        RoundPhase::Waiting => "round waiting",
        RoundPhase::PreRound => "round pre-round",
        RoundPhase::Playing => "round playing",
        RoundPhase::PostRound => "round post-round",
    }
}

fn get_organism_phase_names(abilities: &OrganismAbilities) -> Vec<&'static str> {
    let mut phase_names: Vec<&'static str> = vec![
        get_ability_phase_name(&abilities.first),
        get_ability_phase_name(&abilities.second),
        get_ability_phase_name(&abilities.third),
        get_spore_phase_name(&abilities.spore),
        get_shot_phase_name(&abilities.shots[0]),
        get_shot_phase_name(&abilities.shots[1]),
    ];

    if abilities.third_center.is_some() {
        phase_names.push("field center");
    }

    if abilities.compressed_until.is_some() {
        phase_names.push("compressed");
    }

    if abilities.frozen_until.is_some() {
        phase_names.push("frozen");
    }

    phase_names
}

fn get_phase_names(state: &GameState) -> Vec<&'static str> {
    let mut phase_names: Vec<&'static str> =
        state.round.iter().map(|round| get_round_phase_name(&round.phase)).collect();

    for member in state.members.values() {
        let Some(organism): Option<&Organism> = member.organism.as_ref() else {
            continue;
        };

        phase_names.extend(get_organism_phase_names(&organism.abilities));
    }

    phase_names
}

#[test]
fn try_from_restores_every_scripted_state_and_bundle() {
    let replay_log: ReplayLog = replay_fixture::read_scripted_game_log();
    let mut state: GameState = GameState::new(replay_log.header.settings.clone(), replay_log.header.seed);
    let mut observed_phase_names: BTreeSet<&'static str> = BTreeSet::new();

    for bundle in &replay_log.input_bundles {
        assert_eq!(
            &InputBundle::try_from(InputBundleSerialOut::from(bundle)).unwrap(),
            bundle,
        );

        game::step(&mut state, bundle).unwrap();

        assert_eq!(GameState::try_from(GameStateSerialOut::from(&state)).unwrap(), state);

        observed_phase_names.extend(get_phase_names(&state));
    }

    assert_eq!(observed_phase_names, BTreeSet::from(EXPECTED_PHASE_NAMES));
}

/// The state after `tick`, and the bundle of the tick after it.
fn get_state_and_next_bundle(replay_log: &ReplayLog, tick: Tick) -> (GameState, InputBundle) {
    let mut state: GameState = GameState::new(replay_log.header.settings.clone(), replay_log.header.seed);
    let bundle_index: usize = replay_log.input_bundles.iter().position(|bundle| bundle.tick == tick).unwrap();

    for bundle in &replay_log.input_bundles[..=bundle_index] {
        game::step(&mut state, bundle).unwrap();
    }

    (state, replay_log.input_bundles[bundle_index + 1].clone())
}

fn create_server_frames(replay_log: &ReplayLog) -> Vec<Vec<u8>> {
    let (state, next_bundle): (GameState, InputBundle) = get_state_and_next_bundle(replay_log, MALFORMED_SNAPSHOT_TICK);
    let server_messages: Vec<MessageSerialOut> = vec![
        MessageSerialOut::Snapshot(GameSnapshotSerialOut {
            game_id: 1,
            state: GameStateSerialOut::from(&state),
        }),
        MessageSerialOut::InputBundle(InputBundleSerialOut::from(&next_bundle)),
        MessageSerialOut::GameList {
            game_summaries: vec![GameSummarySerialOut {
                game_id: 1,
                title: String::from("Arena"),
                mode: GameModeKindSerial::Skirmish,
                player_count: 3,
                spectator_count: 1,
                player_cap: 8,
                secured: true,
                team_sizes: vec![2, 1],
            }],
            online_client_count: 9,
        },
        MessageSerialOut::RequestRejected {
            request: RequestKindSerialOut::JoinGame,
            reason: RejectionKindSerialOut::TeamUnbalanced {
                requested: TeamKindSerial::Red,
                smaller: TeamKindSerial::Blue,
            },
        },
        MessageSerialOut::CursorCorrected {
            client_tick: 40,
            cursor: SubpixelPointSerial { x: 409_600, y: 3 },
        },
    ];

    server_messages.iter().map(protocol::encode_message_out).collect()
}

fn create_client_frames() -> Vec<Vec<u8>> {
    let joiner_serial_in: JoinerSerialIn = JoinerSerialIn {
        screen_name: String::from("Blob"),
        loadout: LOADOUT_SERIAL,
        team: TeamChoiceKindSerialIn::Team(TeamKindSerial::Green),
    };
    let client_messages: Vec<MessageSerialIn> = vec![
        MessageSerialIn::CreateGame {
            settings: GameSettingsSerialIn {
                title: String::from("Arena"),
                mode: GameModeKindSerial::Skirmish,
                world_shape: WorldShapeKindSerial::Ellipse,
                world_size_pixels: 800,
                player_minimum: None,
                player_cap: 16,
                team_count: Some(4),
                leaderboard_length: 10,
            },
            password: Some(String::from("secret")),
            joiner: joiner_serial_in.clone(),
        },
        MessageSerialIn::JoinGame {
            game_id: 1,
            password: None,
            joiner: joiner_serial_in,
        },
        MessageSerialIn::SpectateGame {
            game_id: 1,
            password: None,
            screen_name: String::from("Watcher"),
        },
        MessageSerialIn::Respawn {
            loadout: LOADOUT_SERIAL,
            team: TeamChoiceKindSerialIn::Auto,
        },
        MessageSerialIn::Input(PlayerInputSerialIn {
            client_tick: 12,
            cursor: SubpixelPointSerial { x: 300_000, y: 200_000 },
            ability_presses: 0b0001,
            aim: Some(AimVectorSerial { x: 50, y: -50 }),
        }),
    ];

    client_messages.iter().map(protocol::encode_message_in).collect()
}

fn create_malformed_frames(frame: &[u8], rng: &mut Pcg32) -> [Vec<u8>; MALFORMED_FRAME_MUTATION_COUNT] {
    let frame_length: u32 = u32::try_from(frame.len()).unwrap();
    let truncated_length: usize = usize::try_from(rng.below(frame_length)).unwrap();

    let flipped_bit_index: usize = usize::try_from(rng.below(frame_length * 8)).unwrap();
    let mut flipped_frame: Vec<u8> = frame.to_vec();
    flipped_frame[flipped_bit_index / 8] ^= 1 << (flipped_bit_index % 8);

    let mut appended_frame: Vec<u8> = frame.to_vec();
    appended_frame.push(get_random_byte(rng));

    let random_frame_length: u32 = rng.below(RANDOM_FRAME_BYTE_LIMIT);
    let random_frame: Vec<u8> = (0..random_frame_length).map(|_| get_random_byte(rng)).collect();

    [
        frame[..truncated_length].to_vec(),
        flipped_frame,
        appended_frame,
        random_frame,
    ]
}

fn get_random_byte(rng: &mut Pcg32) -> u8 {
    u8::try_from(rng.below(256)).unwrap()
}

fn convert_server_message(message_serial_out: MessageSerialOut) -> Result<(), AppError> {
    match message_serial_out {
        MessageSerialOut::GameList { game_summaries, .. } => {
            for game_summary_serial_out in game_summaries {
                GameSummary::try_from(game_summary_serial_out)?;
            }
        }
        MessageSerialOut::Snapshot(snapshot_serial_out) => {
            GameState::try_from(snapshot_serial_out.state)?;
        }
        MessageSerialOut::InputBundle(bundle_serial_out) => {
            InputBundle::try_from(bundle_serial_out)?;
        }
        MessageSerialOut::CursorCorrected { cursor, .. } => {
            SubpixelPoint::try_from(cursor)?;
        }
        MessageSerialOut::JoinAccepted { .. }
        | MessageSerialOut::RequestRejected { .. }
        | MessageSerialOut::StateChecksum { .. }
        | MessageSerialOut::LeftGame
        | MessageSerialOut::GameEnded => {}
    }

    Ok(())
}

fn convert_client_message(message_serial_in: MessageSerialIn) -> Result<(), AppError> {
    match message_serial_in {
        MessageSerialIn::CreateGame {
            settings,
            password,
            joiner,
        } => {
            GameSettings::try_from(settings).map_err(RejectionKind::to_app_error)?;
            protocol_limits::normalize_password(password).map_err(RejectionKind::to_app_error)?;
            Joiner::try_from(joiner).map_err(RejectionKind::to_app_error)?;
        }
        MessageSerialIn::JoinGame { password, joiner, .. } => {
            protocol_limits::normalize_password(password).map_err(RejectionKind::to_app_error)?;
            Joiner::try_from(joiner).map_err(RejectionKind::to_app_error)?;
        }
        MessageSerialIn::SpectateGame {
            password, screen_name, ..
        } => {
            protocol_limits::normalize_password(password).map_err(RejectionKind::to_app_error)?;
            protocol_limits::normalize_screen_name(&screen_name).map_err(RejectionKind::to_app_error)?;
        }
        MessageSerialIn::Input(player_input_serial_in) => {
            PlayerInput::try_from(player_input_serial_in)?;
        }
        MessageSerialIn::SubscribeGameList
        | MessageSerialIn::UnsubscribeGameList
        | MessageSerialIn::Respawn { .. }
        | MessageSerialIn::UpdateAppearance { .. }
        | MessageSerialIn::RequestSnapshot { .. }
        | MessageSerialIn::LeaveGame => {}
    }

    Ok(())
}

#[test]
fn decode_and_try_from_never_panic_on_malformed_frames() {
    let replay_log: ReplayLog = replay_fixture::read_scripted_game_log();
    let server_frames: Vec<Vec<u8>> = create_server_frames(&replay_log);
    let client_frames: Vec<Vec<u8>> = create_client_frames();
    let mut outcome_counts: MalformedInputOutcomeCounts = MalformedInputOutcomeCounts::new();

    for seed in 0..MALFORMED_INPUT_SEED_COUNT {
        let mut rng: Pcg32 = Pcg32::from_seed(seed);

        for server_frame in &server_frames {
            for malformed_frame in create_malformed_frames(server_frame, &mut rng) {
                outcome_counts.record(protocol::decode_message_out(&malformed_frame), convert_server_message);
            }
        }

        for client_frame in &client_frames {
            for malformed_frame in create_malformed_frames(client_frame, &mut rng) {
                outcome_counts.record(protocol::decode_message_in(&malformed_frame), convert_client_message);
            }
        }
    }

    let frame_count: u64 = u64::try_from(server_frames.len() + client_frames.len()).unwrap();
    let mutation_count: u64 = u64::try_from(MALFORMED_FRAME_MUTATION_COUNT).unwrap();
    let expected_total: u64 = MALFORMED_INPUT_SEED_COUNT * frame_count * mutation_count;

    assert_eq!(u64::from(outcome_counts.total()), expected_total);
    assert!(outcome_counts.decode_rejected > 0);
    assert!(outcome_counts.conversion_rejected > 0);
    assert!(outcome_counts.accepted > 0);
}

#[test]
fn read_replay_never_panics_on_a_malformed_replay() {
    let mut replay_log: ReplayLog = replay_fixture::read_scripted_game_log();
    replay_log.input_bundles.truncate(MALFORMED_REPLAY_BUNDLE_COUNT);
    let replay_bytes: Vec<u8> = replay::write_replay(&replay_log);
    let mut rejected_count: u32 = 0;

    for seed in 0..MALFORMED_INPUT_SEED_COUNT {
        let mut rng: Pcg32 = Pcg32::from_seed(seed);

        for malformed_replay_bytes in create_malformed_frames(&replay_bytes, &mut rng) {
            let read_result: Result<ReplayLog, ReplayReadError> = replay::read_replay(&malformed_replay_bytes);

            if read_result.is_err() {
                rejected_count += 1;
            }
        }
    }

    assert!(rejected_count > 0);
}

fn create_pressing_bundle(player_count: u32) -> InputBundle {
    let every_press: AbilityPressSet = AbilityPressSet::FIRST
        .with(AbilityPressSet::SECOND)
        .with(AbilityPressSet::THIRD)
        .with(AbilityPressSet::FOURTH);
    let player_inputs: Vec<PlayerTickInput> = (0..player_count)
        .map(|player_index| {
            let offset: i32 = i32::try_from(player_index).unwrap();

            PlayerTickInput {
                member_id: MemberId(player_index),
                cursor: WorldPoint {
                    x: 99_000 - 3 * offset,
                    y: 98_000 + 3 * offset,
                },
                ability_presses: every_press,
                aim: Some(AimVector {
                    x: -1000 + i16::try_from(offset).unwrap(),
                    y: 1000,
                }),
            }
        })
        .collect();

    InputBundle {
        tick: SIZE_BUDGET_TICK,
        member_events: Vec::new(),
        player_inputs,
    }
}

fn get_bundle_frame_length(bundle: &InputBundle) -> usize {
    protocol::encode_message_out(&MessageSerialOut::InputBundle(InputBundleSerialOut::from(bundle))).len()
}

fn create_empty_bundle(tick: Tick) -> InputBundle {
    InputBundle {
        tick,
        member_events: Vec::new(),
        player_inputs: Vec::new(),
    }
}

fn create_full_game_settings() -> GameSettings {
    GameSettings {
        title: String::from("Full game"),
        mode: GameModeKind::FreeForAll,
        world_shape: WorldShapeKind::Rectangle,
        world_width_pixels: 3000,
        world_height_pixels: 3000,
        player_minimum: None,
        player_cap: protocol_limits::PLAYER_CAP_HIGHEST,
        team_count: None,
        leaderboard_length: 10,
    }
}

fn create_joined_players_bundle(player_count: u32) -> InputBundle {
    let loadout: Loadout = Loadout::from(LOADOUT_SERIAL);
    let member_events: Vec<MemberEvent> = (0..player_count)
        .flat_map(|player_index| {
            [
                MemberEvent::Joined {
                    member_id: MemberId(player_index),
                    screen_name: format!("player {player_index}"),
                    role: MemberRoleKind::Participant,
                    loadout: Some(loadout),
                    team: None,
                },
                MemberEvent::SpawnRequested {
                    member_id: MemberId(player_index),
                    loadout,
                    team: None,
                },
            ]
        })
        .collect();

    InputBundle {
        tick: Tick(1),
        member_events,
        player_inputs: Vec::new(),
    }
}

fn create_spore_press_bundle(state: &GameState) -> InputBundle {
    let player_inputs: Vec<PlayerTickInput> = state
        .members
        .values()
        .filter_map(|member| member.organism.as_ref().map(|organism| (member.member_id, organism.cursor)))
        .map(|(member_id, cursor)| PlayerTickInput {
            member_id,
            cursor,
            ability_presses: AbilityPressSet::FOURTH,
            aim: None,
        })
        .collect();

    InputBundle {
        tick: state.tick.next(),
        member_events: Vec::new(),
        player_inputs,
    }
}

fn create_full_game_with_spores_in_flight() -> GameState {
    let mut state: GameState = GameState::new(create_full_game_settings(), 7);
    let player_cap: u32 = u32::from(protocol_limits::PLAYER_CAP_HIGHEST);

    game::step(&mut state, &create_joined_players_bundle(player_cap)).unwrap();

    for tick in 2..=GROWTH_TICK_COUNT {
        game::step(&mut state, &create_empty_bundle(Tick(tick))).unwrap();
    }

    let spore_press_bundle: InputBundle = create_spore_press_bundle(&state);
    game::step(&mut state, &spore_press_bundle).unwrap();

    state
}

fn is_launching_spores(state: &GameState) -> bool {
    state.members.values().all(|member| {
        member.organism.as_ref().is_some_and(
            |organism| matches!(&organism.abilities.spore, SporePhase::Flying { spores, .. } if !spores.is_empty()),
        )
    })
}

#[test]
fn input_bundle_frame_fits_the_size_budgets() {
    assert!(
        get_bundle_frame_length(&create_pressing_bundle(EIGHT_PLAYER_COUNT))
            <= EIGHT_PLAYER_INPUT_BUNDLE_SIZE_BUDGET_BYTES
    );
    assert!(
        get_bundle_frame_length(&create_pressing_bundle(SIXTEEN_PLAYER_COUNT))
            <= SIXTEEN_PLAYER_INPUT_BUNDLE_SIZE_BUDGET_BYTES
    );
}

#[test]
fn snapshot_frame_at_the_player_cap_with_spores_in_flight_fits_the_size_budget() {
    let state: GameState = create_full_game_with_spores_in_flight();
    let snapshot_frame: Vec<u8> = protocol::encode_message_out(&MessageSerialOut::Snapshot(GameSnapshotSerialOut {
        game_id: 1,
        state: GameStateSerialOut::from(&state),
    }));

    assert_eq!(
        state.alive_organism_count(),
        u32::from(protocol_limits::PLAYER_CAP_HIGHEST)
    );
    assert!(is_launching_spores(&state));
    assert!(snapshot_frame.len() <= SNAPSHOT_SIZE_BUDGET_BYTES);
}
