use bitcode::DecodeOwned;

use crate::error::AppError;
use crate::protocol::{
    GameStateSerialOut, InputBundleSerialOut, MessageSerialIn, MessageSerialOut, ReplayHeaderSerialOut,
};

/// Bumped on any change to a wire type.
pub const PROTOCOL_VERSION: u16 = 1;

pub fn encode_message_in(message_serial_in: &MessageSerialIn) -> Vec<u8> {
    bitcode::encode(message_serial_in)
}

pub fn decode_message_in(bytes: &[u8]) -> Result<MessageSerialIn, AppError> {
    decode(bytes, "client message")
}

pub fn encode_message_out(message_serial_out: &MessageSerialOut) -> Vec<u8> {
    bitcode::encode(message_serial_out)
}

pub fn decode_message_out(bytes: &[u8]) -> Result<MessageSerialOut, AppError> {
    decode(bytes, "server message")
}

pub fn encode_game_state(state_serial_out: &GameStateSerialOut) -> Vec<u8> {
    bitcode::encode(state_serial_out)
}

pub fn decode_game_state(bytes: &[u8]) -> Result<GameStateSerialOut, AppError> {
    decode(bytes, "game state")
}

pub fn encode_replay_header(header_serial_out: &ReplayHeaderSerialOut) -> Vec<u8> {
    bitcode::encode(header_serial_out)
}

pub fn decode_replay_header(bytes: &[u8]) -> Result<ReplayHeaderSerialOut, AppError> {
    decode(bytes, "replay header")
}

pub fn encode_replay_bundle(bundle_serial_out: &InputBundleSerialOut) -> Vec<u8> {
    bitcode::encode(bundle_serial_out)
}

pub fn decode_replay_bundle(bytes: &[u8]) -> Result<InputBundleSerialOut, AppError> {
    decode(bytes, "replay bundle")
}

fn decode<T: DecodeOwned>(bytes: &[u8], subject: &str) -> Result<T, AppError> {
    bitcode::decode(bytes)
        .map_err(|error| AppError::from_error(&format!("cannot decode the {subject}"), Box::new(error)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameModeKind, GameState, Tick, test_fixture};
    use crate::geometry::{LatticeCoordinate, WorldPoint};
    use crate::member::{Member, MemberId, TeamKind};
    use crate::organism::CellOccupancy;
    use crate::protocol::{
        AimVectorSerial, AppearanceSerial, CellOccupancySerialOut, GameModeKindSerial, GameSettingsSerialIn,
        GameSettingsSerialOut, GameSnapshotSerialOut, GameSummarySerialOut, InputBundleSerialOut, JoinerSerialIn,
        LoadoutSerial, MemberEventSerialOut, MemberRoleKindSerialOut, OrganismColorKindSerial, PlayerInputSerialIn,
        PlayerTickInputSerialOut, RangeBoundKindSerialOut, RejectionKindSerialOut, RequestKindSerialOut,
        SettingFieldKindSerialOut, SkinKindSerial, SubpixelPointSerial, TeamChoiceKindSerialIn, TeamKindSerial,
        WorldPointSerialOut, WorldShapeKindSerial,
    };
    use crate::world::WorldShapeKind;

    fn create_state_serial_out() -> GameStateSerialOut {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Survival, WorldShapeKind::Ellipse, 1200);
        let mut member: Member = test_fixture::create_participant(MemberId(2));
        member.organism = Some(test_fixture::create_organism_with_projectiles());
        state.members.insert(MemberId(2), member);
        state.members.insert(MemberId(4), test_fixture::create_participant(MemberId(4)));
        state.next_member_id = MemberId(5);
        state.tick = Tick(77);

        GameStateSerialOut::from(&state)
    }

    fn create_loadout_serial() -> LoadoutSerial {
        LoadoutSerial::from(&test_fixture::create_loadout())
    }

    fn create_joiner_serial_in() -> JoinerSerialIn {
        JoinerSerialIn {
            screen_name: String::from("Blob"),
            loadout: create_loadout_serial(),
            team: TeamChoiceKindSerialIn::Team(TeamKindSerial::Blue),
        }
    }

    fn create_bundle_serial_out() -> InputBundleSerialOut {
        InputBundleSerialOut {
            tick: 78,
            member_events: vec![
                MemberEventSerialOut::Joined {
                    member_id: 5,
                    screen_name: String::from("Late"),
                    role: MemberRoleKindSerialOut::Participant,
                    loadout: Some(create_loadout_serial()),
                    team: None,
                },
                MemberEventSerialOut::SpawnRequested {
                    member_id: 5,
                    loadout: create_loadout_serial(),
                    team: Some(TeamKindSerial::Red),
                },
                MemberEventSerialOut::Left { member_id: 4 },
                MemberEventSerialOut::AppearanceChanged {
                    member_id: 2,
                    appearance: AppearanceSerial {
                        color: OrganismColorKindSerial::Sun,
                        skin: SkinKindSerial::None,
                    },
                },
            ],
            player_inputs: vec![PlayerTickInputSerialOut {
                member_id: 2,
                cursor: WorldPointSerialOut { x: 41, y: 52 },
                ability_presses: 0b0011,
                aim: Some(AimVectorSerial { x: 9, y: -9 }),
            }],
        }
    }

    #[test]
    fn decode_message_in_restores_every_variant() {
        let message_serial_ins: Vec<MessageSerialIn> = vec![
            MessageSerialIn::SubscribeGameList,
            MessageSerialIn::UnsubscribeGameList,
            MessageSerialIn::CreateGame {
                settings: GameSettingsSerialIn {
                    title: String::from("Arena"),
                    mode: GameModeKindSerial::Skirmish,
                    world_shape: WorldShapeKindSerial::Rectangle,
                    world_size_pixels: 800,
                    player_minimum: None,
                    player_cap: 16,
                    team_count: Some(2),
                    leaderboard_length: 10,
                },
                password: Some(String::from("secret")),
                joiner: create_joiner_serial_in(),
            },
            MessageSerialIn::JoinGame {
                game_id: 3,
                password: None,
                joiner: create_joiner_serial_in(),
            },
            MessageSerialIn::SpectateGame {
                game_id: 3,
                password: Some(String::from("secret")),
                screen_name: String::from("Watcher"),
            },
            MessageSerialIn::Respawn {
                loadout: create_loadout_serial(),
                team: TeamChoiceKindSerialIn::Auto,
            },
            MessageSerialIn::UpdateAppearance {
                appearance: create_loadout_serial().appearance,
            },
            MessageSerialIn::Input(PlayerInputSerialIn {
                client_tick: 90,
                cursor: SubpixelPointSerial { x: -2048, y: 999 },
                ability_presses: 0b0001,
                aim: Some(AimVectorSerial { x: 1, y: 2 }),
            }),
            MessageSerialIn::RequestSnapshot { client_tick: 91 },
            MessageSerialIn::LeaveGame,
        ];

        for message_serial_in in message_serial_ins {
            let message_bytes: Vec<u8> = encode_message_in(&message_serial_in);

            assert_eq!(decode_message_in(&message_bytes).unwrap(), message_serial_in);
        }
    }

    #[test]
    fn decode_message_out_restores_every_variant() {
        let message_serial_outs: Vec<MessageSerialOut> = vec![
            MessageSerialOut::GameList {
                game_summaries: vec![GameSummarySerialOut {
                    game_id: 3,
                    title: String::from("Arena"),
                    mode: GameModeKindSerial::Survival,
                    player_count: 4,
                    spectator_count: 1,
                    player_cap: 16,
                    secured: false,
                    team_sizes: Vec::new(),
                }],
                online_client_count: 12,
            },
            MessageSerialOut::JoinAccepted {
                game_id: 3,
                member_id: 5,
            },
            MessageSerialOut::Snapshot(GameSnapshotSerialOut {
                game_id: 3,
                state: create_state_serial_out(),
            }),
            MessageSerialOut::InputBundle(create_bundle_serial_out()),
            MessageSerialOut::StateChecksum {
                tick: 84,
                checksum: 0xfedc_ba98_7654_3210,
            },
            MessageSerialOut::CursorCorrected {
                client_tick: 80,
                cursor: SubpixelPointSerial { x: 3, y: -3 },
            },
            MessageSerialOut::RequestRejected {
                request: RequestKindSerialOut::JoinGame,
                reason: RejectionKindSerialOut::TeamUnbalanced {
                    requested: TeamKindSerial::Green,
                    smaller: TeamKindSerial::Pink,
                },
            },
            MessageSerialOut::RequestRejected {
                request: RequestKindSerialOut::CreateGame,
                reason: RejectionKindSerialOut::SettingOutOfRange {
                    field: SettingFieldKindSerialOut::LeaderboardLength,
                    bound: RangeBoundKindSerialOut::Above,
                },
            },
            MessageSerialOut::LeftGame,
            MessageSerialOut::GameEnded,
        ];

        for message_serial_out in message_serial_outs {
            let message_bytes: Vec<u8> = encode_message_out(&message_serial_out);

            assert_eq!(decode_message_out(&message_bytes).unwrap(), message_serial_out);
        }
    }

    #[test]
    fn decode_game_state_restores_the_state() {
        let state_serial_out: GameStateSerialOut = create_state_serial_out();
        let state_bytes: Vec<u8> = encode_game_state(&state_serial_out);

        assert_eq!(decode_game_state(&state_bytes).unwrap(), state_serial_out);
    }

    #[test]
    fn decode_message_out_rejects_a_truncated_or_extended_frame() {
        let message_bytes: Vec<u8> = encode_message_out(&MessageSerialOut::InputBundle(create_bundle_serial_out()));
        let mut extended_bytes: Vec<u8> = message_bytes.clone();
        extended_bytes.push(0);

        assert!(decode_message_out(&message_bytes[..message_bytes.len() - 1]).is_err());
        assert!(decode_message_out(&extended_bytes).is_err());
    }

    #[test]
    fn encode_gives_equal_bytes_for_equal_cell_sets_built_in_different_orders() {
        let mut first_cells: CellOccupancy = CellOccupancy::empty();
        let mut second_cells: CellOccupancy = CellOccupancy::empty();

        for (i, j) in [(0, 0), (3, -2), (-1, 4), (2, 2)] {
            first_cells.insert(LatticeCoordinate { i, j });
        }

        for (i, j) in [(2, 2), (-6, 0), (-1, 4), (0, 0), (3, -2)] {
            second_cells.insert(LatticeCoordinate { i, j });
        }

        second_cells.remove(LatticeCoordinate { i: -6, j: 0 });
        second_cells.retighten();

        let first_bytes: Vec<u8> = bitcode::encode(&CellOccupancySerialOut::from(&first_cells));
        let second_bytes: Vec<u8> = bitcode::encode(&CellOccupancySerialOut::from(&second_cells));

        assert_eq!(first_bytes, second_bytes);
    }

    #[test]
    fn decode_game_state_then_try_from_restores_the_model() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Skirmish, WorldShapeKind::Rectangle, 800);
        let mut member: Member = test_fixture::create_participant_with_organism(MemberId(0), WorldPoint { x: 9, y: 9 });
        member.team = Some(TeamKind::Red);
        state.members.insert(MemberId(0), member);
        state.next_member_id = MemberId(1);

        let state_bytes: Vec<u8> = encode_game_state(&GameStateSerialOut::from(&state));
        let decoded_state: GameState = GameState::try_from(decode_game_state(&state_bytes).unwrap()).unwrap();

        assert_eq!(decoded_state, state);
    }

    #[test]
    fn decode_replay_header_restores_the_header() {
        let header_serial_out: ReplayHeaderSerialOut = ReplayHeaderSerialOut {
            settings: GameSettingsSerialOut::from(&test_fixture::create_settings(
                GameModeKind::FreeForAll,
                WorldShapeKind::Rectangle,
                640,
            )),
            seed: 99,
        };
        let header_bytes: Vec<u8> = encode_replay_header(&header_serial_out);

        assert_eq!(decode_replay_header(&header_bytes).unwrap(), header_serial_out);
    }

    #[test]
    fn decode_replay_bundle_restores_the_bundle() {
        let bundle_serial_out: InputBundleSerialOut = create_bundle_serial_out();
        let bundle_bytes: Vec<u8> = encode_replay_bundle(&bundle_serial_out);

        assert_eq!(decode_replay_bundle(&bundle_bytes).unwrap(), bundle_serial_out);
    }
}
