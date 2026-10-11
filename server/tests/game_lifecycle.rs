mod helpers;

use std::path::PathBuf;
use std::time::Duration;

use helpers::test_server;
use helpers::test_server::{TestClient, TestPlayer, TestServer};
use server::game::{cursor_clamp, game_task};
use shared::ability::AbilityPressSet;
use shared::game::{GameModeKind, InputBundle, MemberEvent, Tick};
use shared::geometry;
use shared::geometry::{SubpixelPoint, WorldPoint};
use shared::member::{MemberId, MemberRoleKind};
use shared::protocol::{
    MemberEventSerialOut, MessageSerialIn, MessageSerialOut, PlayerInputSerialIn, RejectionKindSerialOut,
    RequestKindSerialOut, SubpixelPointSerial, TeamChoiceKindSerialIn,
};
use shared::replay;
use shared::replay::{ReplayLog, TickChecksum};

fn has_member_event(bundle: &InputBundle, is_wanted: impl Fn(&MemberEvent) -> bool) -> bool {
    bundle.member_events.iter().any(is_wanted)
}

async fn play_until_member_event(player: &mut TestPlayer, is_wanted: impl Fn(&MemberEvent) -> bool) {
    let playing = async {
        loop {
            let bundle: InputBundle = player.receive_bundle().await;

            if has_member_event(&bundle, &is_wanted) {
                return;
            }
        }
    };

    tokio::time::timeout(test_server::PLAY_TIMEOUT, playing)
        .await
        .expect("the member event arrives within the play timeout");
}

fn create_input(client_tick: Tick, cursor: SubpixelPoint) -> MessageSerialIn {
    MessageSerialIn::Input(PlayerInputSerialIn {
        client_tick: client_tick.0,
        cursor: SubpixelPointSerial::from(&cursor),
        ability_presses: AbilityPressSet::NONE.bits(),
        aim: None,
    })
}

#[actix_web::test]
async fn create_game_joins_the_creator_in_one_message() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut client: TestClient = TestClient::connect(&test_server).await;

    client.send(&test_server::create_game_message("Arena", None, "Blob")).await;

    assert_eq!(
        client.receive().await,
        MessageSerialOut::JoinAccepted {
            game_id: 1,
            member_id: 0
        }
    );
    assert!(matches!(
        client.receive().await,
        MessageSerialOut::Snapshot(snapshot_serial_out) if snapshot_serial_out.state.tick == 0 && snapshot_serial_out.state.members.is_empty()
    ));
    assert!(matches!(
        client.receive().await,
        MessageSerialOut::InputBundle(bundle_serial_out)
            if bundle_serial_out.tick == 1
                && matches!(
                    bundle_serial_out.member_events[..],
                    [MemberEventSerialOut::Joined { member_id: 0, .. }, MemberEventSerialOut::SpawnRequested { member_id: 0, .. }]
                )
    ));
}

#[actix_web::test]
async fn join_game_sends_a_snapshot_then_bundles_and_every_checksum_matches() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut creator: TestPlayer = test_server::create_game(&test_server, "Arena", "Creator").await;
    creator.play_until(|player| player.applied_tick().0 >= 3).await;

    let mut joiner: TestPlayer = test_server::join_game(&test_server, creator.game_id, "Joiner").await;
    let first_bundle: InputBundle = joiner.receive_bundle().await;

    assert_eq!(joiner.member_id, MemberId(1));
    assert!(has_member_event(&first_bundle, |member_event| matches!(
        member_event,
        MemberEvent::Joined {
            member_id: MemberId(1),
            ..
        }
    )));

    creator.play_until(|player| player.verified_checksum_count >= 2).await;
    joiner.play_until(|player| player.verified_checksum_count >= 1).await;
}

#[actix_web::test]
async fn spectate_game_adds_a_spectator_which_follows_the_game() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let creator: TestPlayer = test_server::create_game(&test_server, "Arena", "Creator").await;
    let mut client: TestClient = TestClient::connect(&test_server).await;

    client
        .send(&MessageSerialIn::SpectateGame {
            game_id: creator.game_id,
            password: None,
            screen_name: String::from("Watcher"),
        })
        .await;
    let mut spectator: TestPlayer = TestPlayer::enter(client).await;
    spectator.play_until(|player| player.state.members.contains_key(&player.member_id)).await;

    assert_eq!(
        spectator.state.members[&spectator.member_id].role,
        MemberRoleKind::Spectator
    );
    spectator.play_until(|player| player.verified_checksum_count >= 1).await;
}

#[actix_web::test]
async fn respawn_brings_back_a_dead_participant() {
    let test_server: TestServer =
        test_server::start_test_server(|config| config.tick_period = Duration::from_millis(10));
    let mut client: TestClient = TestClient::connect(&test_server).await;
    client
        .send(&MessageSerialIn::CreateGame {
            settings: test_server::create_settings_serial_in("Small", GameModeKind::FreeForAll, 300, 8),
            password: None,
            joiner: test_server::create_joiner_serial_in("Mortal", TeamChoiceKindSerialIn::Auto),
        })
        .await;
    let mut player: TestPlayer = TestPlayer::enter(client).await;
    player.play_until(|player| player.get_own_organism().is_some()).await;

    let target: SubpixelPoint = WorldPoint { x: -2000, y: 150 }.to_subpixel_point();
    let dying = async {
        while player.get_own_organism().is_some() {
            player.client.send(&create_input(player.applied_tick(), target)).await;
            player.receive_bundle().await;
        }
    };

    tokio::time::timeout(test_server::PLAY_TIMEOUT, dying)
        .await
        .expect("the organism dies within the play timeout");

    player
        .client
        .send(&MessageSerialIn::Respawn {
            loadout: test_server::create_loadout_serial(),
            team: TeamChoiceKindSerialIn::Auto,
        })
        .await;
    player.play_until(|player| player.get_own_organism().is_some()).await;
}

#[actix_web::test]
async fn leave_game_answers_left_game_and_the_others_see_the_member_leave() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut creator: TestPlayer = test_server::create_game(&test_server, "Arena", "Creator").await;
    let mut leaver: TestPlayer = test_server::join_game(&test_server, creator.game_id, "Leaver").await;

    leaver.client.send(&MessageSerialIn::LeaveGame).await;

    leaver
        .client
        .receive_until(|message_serial_out| *message_serial_out == MessageSerialOut::LeftGame)
        .await;
    play_until_member_event(&mut creator, |member_event| {
        *member_event == MemberEvent::Left { member_id: MemberId(1) }
    })
    .await;
}

#[actix_web::test]
async fn leave_game_then_join_another_game_receives_no_frame_of_the_first_game_after_left_game() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let first_creator: TestPlayer = test_server::create_game(&test_server, "First", "First creator").await;
    let second_creator: TestPlayer = test_server::create_game(&test_server, "Second", "Second creator").await;
    let mut traveller: TestPlayer = test_server::join_game(&test_server, first_creator.game_id, "Traveller").await;
    traveller.receive_bundle().await;

    traveller.client.send(&MessageSerialIn::LeaveGame).await;
    traveller
        .client
        .send(&test_server::join_game_message(
            second_creator.game_id,
            None,
            "Traveller",
        ))
        .await;
    traveller
        .client
        .receive_until(|message_serial_out| *message_serial_out == MessageSerialOut::LeftGame)
        .await;

    let mut second_game_player: TestPlayer = TestPlayer::enter(traveller.client).await;

    assert_eq!(second_game_player.game_id, second_creator.game_id);

    for _ in 0..5 {
        let tick_before: Tick = second_game_player.applied_tick();
        let bundle: InputBundle = second_game_player.receive_bundle().await;

        assert_eq!(bundle.tick, tick_before.next());
    }
}

#[actix_web::test]
async fn finish_connection_removes_the_member_of_a_closed_connection() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut creator: TestPlayer = test_server::create_game(&test_server, "Arena", "Creator").await;
    let leaver: TestPlayer = test_server::join_game(&test_server, creator.game_id, "Leaver").await;

    drop(leaver);

    play_until_member_event(&mut creator, |member_event| {
        *member_event == MemberEvent::Left { member_id: MemberId(1) }
    })
    .await;
}

#[actix_web::test]
async fn join_game_admits_only_one_of_two_joins_with_one_name() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let creator: TestPlayer = test_server::create_game(&test_server, "Arena", "Creator").await;
    let mut first_client: TestClient = TestClient::connect(&test_server).await;
    let mut second_client: TestClient = TestClient::connect(&test_server).await;

    first_client.send(&test_server::join_game_message(creator.game_id, None, "Twin")).await;
    second_client.send(&test_server::join_game_message(creator.game_id, None, "Twin")).await;
    let replies: [MessageSerialOut; 2] = [first_client.receive().await, second_client.receive().await];

    let rejection: MessageSerialOut = MessageSerialOut::RequestRejected {
        request: RequestKindSerialOut::JoinGame,
        reason: RejectionKindSerialOut::ScreenNameTaken,
    };
    let accepted_count: usize =
        replies.iter().filter(|reply| matches!(reply, MessageSerialOut::JoinAccepted { .. })).count();

    assert_eq!(accepted_count, 1);
    assert!(replies.contains(&rejection));
}

#[actix_web::test]
async fn join_game_admits_only_one_of_two_joins_for_the_last_slot() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut creator_client: TestClient = TestClient::connect(&test_server).await;
    creator_client
        .send(&MessageSerialIn::CreateGame {
            settings: test_server::create_settings_serial_in("Duel", GameModeKind::FreeForAll, 800, 2),
            password: None,
            joiner: test_server::create_joiner_serial_in("Creator", TeamChoiceKindSerialIn::Auto),
        })
        .await;
    let creator: TestPlayer = TestPlayer::enter(creator_client).await;
    let mut first_client: TestClient = TestClient::connect(&test_server).await;
    let mut second_client: TestClient = TestClient::connect(&test_server).await;

    first_client.send(&test_server::join_game_message(creator.game_id, None, "First")).await;
    second_client.send(&test_server::join_game_message(creator.game_id, None, "Second")).await;
    let replies: [MessageSerialOut; 2] = [first_client.receive().await, second_client.receive().await];

    let rejection: MessageSerialOut = MessageSerialOut::RequestRejected {
        request: RequestKindSerialOut::JoinGame,
        reason: RejectionKindSerialOut::GameFull,
    };
    let accepted_count: usize =
        replies.iter().filter(|reply| matches!(reply, MessageSerialOut::JoinAccepted { .. })).count();

    assert_eq!(accepted_count, 1);
    assert!(replies.contains(&rejection));
}

#[actix_web::test]
async fn run_game_task_removes_an_empty_game_after_the_grace_period() {
    let test_server: TestServer =
        test_server::start_test_server(|config| config.empty_game_grace_period = Duration::from_millis(200));
    let mut creator: TestPlayer = test_server::create_game(&test_server, "Arena", "Creator").await;
    creator.client.send(&MessageSerialIn::LeaveGame).await;
    creator
        .client
        .receive_until(|message_serial_out| *message_serial_out == MessageSerialOut::LeftGame)
        .await;

    tokio::time::sleep(Duration::from_millis(600)).await;
    creator.client.send(&test_server::join_game_message(creator.game_id, None, "Late")).await;

    assert_eq!(
        creator.client.receive().await,
        MessageSerialOut::RequestRejected {
            request: RequestKindSerialOut::JoinGame,
            reason: RejectionKindSerialOut::GameNotFound,
        },
    );

    creator.client.send(&test_server::create_game_message("Arena", None, "Creator")).await;

    assert!(matches!(
        creator.client.receive().await,
        MessageSerialOut::JoinAccepted { .. }
    ));
}

#[actix_web::test]
async fn join_game_during_the_grace_period_keeps_the_game() {
    let test_server: TestServer =
        test_server::start_test_server(|config| config.empty_game_grace_period = Duration::from_millis(200));
    let mut creator: TestPlayer = test_server::create_game(&test_server, "Arena", "Creator").await;
    creator.client.send(&MessageSerialIn::LeaveGame).await;
    creator
        .client
        .receive_until(|message_serial_out| *message_serial_out == MessageSerialOut::LeftGame)
        .await;

    let mut rejoiner: TestPlayer = test_server::join_game(&test_server, creator.game_id, "Rejoiner").await;
    tokio::time::sleep(Duration::from_millis(600)).await;

    rejoiner.play_until(|player| player.verified_checksum_count >= 1).await;
}

#[actix_web::test]
async fn input_beyond_the_move_budget_is_clamped_and_corrected() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut player: TestPlayer = test_server::create_game(&test_server, "Arena", "Fast").await;
    player.play_until(|player| player.get_own_organism().is_some()).await;
    let spawn_cursor: SubpixelPoint = player.get_own_organism().unwrap().cursor.to_subpixel_point();
    let client_tick: Tick = player.applied_tick();

    let target: SubpixelPoint = SubpixelPoint {
        x: spawn_cursor.x + 500 * geometry::SUBPIXELS_PER_PIXEL,
        y: spawn_cursor.y,
    };
    player.client.send(&create_input(client_tick, target)).await;

    let corrected_cursor: SubpixelPoint = loop {
        let message_serial_out: MessageSerialOut = player.client.receive().await;
        player.apply(&message_serial_out);

        if let MessageSerialOut::CursorCorrected {
            client_tick: corrected_tick,
            cursor,
        } = message_serial_out
        {
            assert_eq!(corrected_tick, client_tick.0);

            break SubpixelPoint::try_from(cursor).unwrap();
        }
    };

    let own_cursor: WorldPoint = player.get_own_organism().unwrap().cursor;
    let move_budget_cap_subpixels: i64 = cursor_clamp::MOVE_BUDGET_CAP_TICKS
        * (cursor_clamp::MOVE_SUBPIXELS_PER_TICK + cursor_clamp::MOVE_TOLERANCE_SUBPIXELS_PER_TICK);
    let rounded_corrected_cursor_x: i32 =
        (corrected_cursor.x + geometry::SUBPIXELS_PER_PIXEL / 2).div_euclid(geometry::SUBPIXELS_PER_PIXEL);

    assert_eq!(corrected_cursor.y, spawn_cursor.y);
    assert!(corrected_cursor.x > spawn_cursor.x);
    assert!(i64::from(corrected_cursor.x - spawn_cursor.x) <= move_budget_cap_subpixels);
    assert_eq!(own_cursor.x, rounded_corrected_cursor_x);
    assert!(own_cursor.x < target.x / geometry::SUBPIXELS_PER_PIXEL);
}

#[actix_web::test]
async fn replay_capture_records_a_replay_with_the_server_checksums() {
    let replay_directory: PathBuf = std::env::temp_dir().join(format!("bacter-replay-capture-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&replay_directory);
    let configured_directory: PathBuf = replay_directory.clone();
    let test_server: TestServer =
        test_server::start_test_server(move |config| config.replay_capture_directory = Some(configured_directory));
    let mut player: TestPlayer = test_server::create_game(&test_server, "Recorded", "Blob").await;

    let wanted_checksum_count: u32 = 2;
    let mut received_checksums: Vec<(u32, u64)> = Vec::new();

    while received_checksums.len() < usize::try_from(wanted_checksum_count).unwrap() {
        let message_serial_out: MessageSerialOut = player.client.receive().await;
        player.apply(&message_serial_out);

        if let MessageSerialOut::StateChecksum { tick, checksum } = message_serial_out {
            received_checksums.push((tick, checksum));
        }
    }

    let replay_path: PathBuf = replay_directory.join(format!("{}.replay", player.game_id));
    let bundle_count_through_last_checksum: usize =
        usize::try_from(wanted_checksum_count * game_task::CHECKSUM_INTERVAL_TICKS).unwrap();
    let replay_written = async {
        loop {
            let replay_bytes: Vec<u8> = std::fs::read(&replay_path).unwrap_or_default();
            let replay_log: Option<ReplayLog> = replay::read_replay(&replay_bytes).ok();
            let complete_replay_log: Option<ReplayLog> =
                replay_log.filter(|replay_log| replay_log.input_bundles.len() >= bundle_count_through_last_checksum);

            if let Some(replay_log) = complete_replay_log {
                return replay_log;
            }

            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    let replay_log: ReplayLog = tokio::time::timeout(test_server::PLAY_TIMEOUT, replay_written)
        .await
        .expect("the replay is written within the play timeout");

    let tick_checksums: Vec<TickChecksum> = replay::run_replay(&replay_log).unwrap();

    for (tick, checksum) in received_checksums {
        let replayed_checksum: TickChecksum = tick_checksums[usize::try_from(tick - 1).unwrap()];

        assert_eq!(
            replayed_checksum,
            TickChecksum {
                tick: Tick(tick),
                checksum
            }
        );
    }
}
