mod helpers;

use std::net::{IpAddr, Ipv4Addr};

use tokio_tungstenite::tungstenite::handshake::client::Request;
use tokio_tungstenite::tungstenite::http::{HeaderName, HeaderValue};
use tokio_tungstenite::tungstenite::{Error as WebSocketError, Message};

use helpers::test_server;
use helpers::test_server::{TestClient, TestPlayer, TestServer};
use server::password;
use server::shutdown::ServerRunState;
use server::socket;
use shared::game::GameModeKind;
use shared::member::MemberId;
use shared::protocol;
use shared::protocol::protocol_limits;
use shared::protocol::{
    MessageSerialIn, MessageSerialOut, RejectionKindSerialOut, RequestKindSerialOut, TeamChoiceKindSerialIn,
    TeamKindSerial,
};

const FORWARDED_CLIENT_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 7));
const OTHER_FORWARDED_CLIENT_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 8));

fn rejection(request: RequestKindSerialOut, reason: RejectionKindSerialOut) -> MessageSerialOut {
    MessageSerialOut::RequestRejected { request, reason }
}

async fn connect_with_client_address_header(test_server: &TestServer, client_ip: IpAddr) -> TestClient {
    let mut request: Request = test_server::create_socket_request(
        test_server,
        Some(test_server::TEST_ORIGIN),
        Some(protocol::PROTOCOL_VERSION),
    )
    .unwrap();
    let header_name: HeaderName = HeaderName::from_bytes(socket::CLIENT_ADDRESS_HEADER.as_str().as_bytes()).unwrap();

    request.headers_mut().insert(header_name, HeaderValue::from_str(&client_ip.to_string()).unwrap());

    TestClient::try_connect_request(request).await.unwrap()
}

/// Each client has received a game list, so its connection is counted.
async fn fill_per_ip_limit_with_client_address_header(test_server: &TestServer, client_ip: IpAddr) -> Vec<TestClient> {
    let mut clients: Vec<TestClient> = Vec::new();

    for _ in 0..protocol_limits::CONNECTION_LIMIT_PER_IP {
        let mut client: TestClient = connect_with_client_address_header(test_server, client_ip).await;
        client.send(&MessageSerialIn::SubscribeGameList).await;

        client.receive().await;
        clients.push(client);
    }

    clients
}

#[actix_web::test]
async fn open_socket_closes_a_connection_with_a_wrong_or_missing_protocol_version() {
    let test_server: TestServer = test_server::start_test_server(|_| {});

    for protocol_version in [Some(0), None] {
        let mut client: TestClient =
            TestClient::try_connect(&test_server, Some(test_server::TEST_ORIGIN), protocol_version)
                .await
                .unwrap();

        assert_eq!(
            client.receive_close().await,
            (4001, format!("server protocol version {}", protocol::PROTOCOL_VERSION))
        );
    }
}

#[actix_web::test]
async fn open_socket_refuses_a_handshake_from_a_disallowed_or_missing_origin() {
    let test_server: TestServer = test_server::start_test_server(|_| {});

    for origin in [Some("http://evil.example"), None] {
        let connection_result: Result<TestClient, WebSocketError> =
            TestClient::try_connect(&test_server, origin, Some(protocol::PROTOCOL_VERSION)).await;

        assert!(matches!(connection_result, Err(WebSocketError::Http(response)) if response.status() == 403));
    }
}

#[actix_web::test]
async fn run_connection_closes_on_a_text_frame() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut client: TestClient = TestClient::connect(&test_server).await;

    client.send_raw(Message::text("hello")).await;

    assert_eq!(client.receive_close().await, (1008, String::from("text frame")));
}

#[actix_web::test]
async fn run_connection_closes_on_an_oversize_frame() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut client: TestClient = TestClient::connect(&test_server).await;

    client.send_raw(Message::binary(vec![0; protocol_limits::INBOUND_FRAME_BYTE_LIMIT + 1])).await;

    assert_eq!(client.receive_close().await, (1008, String::from("oversize")));
}

#[actix_web::test]
async fn run_connection_closes_only_the_connection_which_sent_garbage() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut garbage_client: TestClient = TestClient::connect(&test_server).await;
    let mut other_client: TestClient = TestClient::connect(&test_server).await;

    garbage_client.send_raw(Message::binary(vec![0xff; 32])).await;

    assert_eq!(garbage_client.receive_close().await, (1008, String::from("decode")));

    other_client.send(&MessageSerialIn::SubscribeGameList).await;

    assert!(matches!(
        other_client.receive().await,
        MessageSerialOut::GameList { .. }
    ));
}

#[actix_web::test]
async fn dispatch_answers_requests_beyond_the_rate_limit_with_rate_limited() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut client: TestClient = TestClient::connect(&test_server).await;
    let request_burst: usize = socket::REQUEST_BURST as usize;
    let request_count: usize = request_burst + 1;

    for _ in 0..request_count {
        client.send(&MessageSerialIn::SubscribeGameList).await;
    }

    let mut message_serial_outs: Vec<MessageSerialOut> = Vec::new();

    for _ in 0..request_count {
        message_serial_outs.push(client.receive().await);
    }

    let game_list_count: usize = message_serial_outs
        .iter()
        .filter(|message_serial_out| matches!(message_serial_out, MessageSerialOut::GameList { .. }))
        .count();

    assert_eq!(game_list_count, request_burst);
    assert!(message_serial_outs.contains(&rejection(
        RequestKindSerialOut::SubscribeGameList,
        RejectionKindSerialOut::RateLimited,
    )));
}

#[actix_web::test]
async fn open_socket_closes_connections_beyond_the_per_ip_limit_with_server_full() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut clients: Vec<TestClient> = Vec::new();

    for _ in 0..protocol_limits::CONNECTION_LIMIT_PER_IP {
        let mut client: TestClient = TestClient::connect(&test_server).await;
        client.send(&MessageSerialIn::SubscribeGameList).await;

        client.receive().await;
        clients.push(client);
    }

    let mut extra_client: TestClient = TestClient::connect(&test_server).await;

    assert_eq!(extra_client.receive_close().await, (4006, String::from("server full")));
}

#[actix_web::test]
async fn open_socket_counts_a_trusted_proxy_connection_against_its_client_address_header() {
    let test_server: TestServer = test_server::start_test_server(|config| {
        config.trusted_proxy_addresses = vec![IpAddr::V4(Ipv4Addr::LOCALHOST)];
    });
    let _clients: Vec<TestClient> =
        fill_per_ip_limit_with_client_address_header(&test_server, FORWARDED_CLIENT_IP).await;

    let mut same_ip_client: TestClient = connect_with_client_address_header(&test_server, FORWARDED_CLIENT_IP).await;

    assert_eq!(
        same_ip_client.receive_close().await,
        (4006, String::from("server full"))
    );

    let mut other_ip_client: TestClient =
        connect_with_client_address_header(&test_server, OTHER_FORWARDED_CLIENT_IP).await;
    other_ip_client.send(&MessageSerialIn::SubscribeGameList).await;

    assert!(matches!(
        other_ip_client.receive().await,
        MessageSerialOut::GameList { .. }
    ));
    assert_eq!(
        *test_server.registries.connections_per_ip.get(&FORWARDED_CLIENT_IP).unwrap(),
        protocol_limits::CONNECTION_LIMIT_PER_IP,
    );
    assert_eq!(
        *test_server.registries.connections_per_ip.get(&OTHER_FORWARDED_CLIENT_IP).unwrap(),
        1
    );
    assert!(!test_server.registries.connections_per_ip.contains_key(&IpAddr::V4(Ipv4Addr::LOCALHOST)));
}

#[actix_web::test]
async fn open_socket_ignores_the_client_address_header_without_a_trusted_proxy() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let _clients: Vec<TestClient> =
        fill_per_ip_limit_with_client_address_header(&test_server, FORWARDED_CLIENT_IP).await;

    let mut other_ip_client: TestClient =
        connect_with_client_address_header(&test_server, OTHER_FORWARDED_CLIENT_IP).await;

    assert_eq!(
        other_ip_client.receive_close().await,
        (4006, String::from("server full"))
    );
    assert!(!test_server.registries.connections_per_ip.contains_key(&FORWARDED_CLIENT_IP));
}

#[actix_web::test]
async fn run_connection_closes_every_connection_when_the_server_shuts_down() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut idle_client: TestClient = TestClient::connect(&test_server).await;
    let mut lobby_client: TestClient = TestClient::connect(&test_server).await;
    lobby_client.send(&MessageSerialIn::SubscribeGameList).await;

    lobby_client.receive().await;

    test_server.run_state.send(ServerRunState::ShuttingDown).unwrap();

    assert_eq!(
        idle_client.receive_close().await,
        (1001, String::from("server shutting down"))
    );
    assert_eq!(
        lobby_client.receive_close().await,
        (1001, String::from("server shutting down"))
    );
}

#[actix_web::test]
async fn join_game_checks_the_password() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut creator: TestClient = TestClient::connect(&test_server).await;
    creator.send(&test_server::create_game_message("Private", Some("secret"), "Owner")).await;
    let creator_player: TestPlayer = TestPlayer::enter(creator).await;
    let mut joiner: TestClient = TestClient::connect(&test_server).await;

    joiner.send(&test_server::join_game_message(creator_player.game_id, None, "Guest")).await;

    assert_eq!(
        joiner.receive().await,
        rejection(RequestKindSerialOut::JoinGame, RejectionKindSerialOut::PasswordRequired),
    );

    joiner
        .send(&test_server::join_game_message(
            creator_player.game_id,
            Some("wrong"),
            "Guest",
        ))
        .await;

    assert_eq!(
        joiner.receive().await,
        rejection(
            RequestKindSerialOut::JoinGame,
            RejectionKindSerialOut::PasswordIncorrect
        ),
    );

    joiner
        .send(&test_server::join_game_message(
            creator_player.game_id,
            Some("secret"),
            "Guest",
        ))
        .await;

    assert_eq!(TestPlayer::enter(joiner).await.member_id, MemberId(1));
}

#[actix_web::test]
async fn join_game_closes_a_connection_at_the_password_failure_limit() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut creator: TestClient = TestClient::connect(&test_server).await;
    creator.send(&test_server::create_game_message("Private", Some("secret"), "Owner")).await;
    let creator_player: TestPlayer = TestPlayer::enter(creator).await;
    let mut joiner: TestClient = TestClient::connect(&test_server).await;

    for _ in 0..password::PASSWORD_FAILURE_LIMIT - 1 {
        joiner
            .send(&test_server::join_game_message(
                creator_player.game_id,
                Some("wrong"),
                "Guest",
            ))
            .await;

        assert_eq!(
            joiner.receive().await,
            rejection(
                RequestKindSerialOut::JoinGame,
                RejectionKindSerialOut::PasswordIncorrect
            ),
        );
    }

    joiner
        .send(&test_server::join_game_message(
            creator_player.game_id,
            Some("wrong"),
            "Guest",
        ))
        .await;

    assert_eq!(
        joiner.receive_close().await,
        (4003, String::from("password failure limit"))
    );
}

#[actix_web::test]
async fn join_game_rejects_a_taken_screen_name() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let creator: TestPlayer = test_server::create_game(&test_server, "Arena", "Blob").await;
    let mut joiner: TestClient = TestClient::connect(&test_server).await;

    joiner.send(&test_server::join_game_message(creator.game_id, None, " Blob ")).await;

    assert_eq!(
        joiner.receive().await,
        rejection(RequestKindSerialOut::JoinGame, RejectionKindSerialOut::ScreenNameTaken),
    );
}

#[actix_web::test]
async fn join_game_rejects_a_full_game() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut creator: TestClient = TestClient::connect(&test_server).await;
    creator
        .send(&MessageSerialIn::CreateGame {
            settings: test_server::create_settings_serial_in("Duel", GameModeKind::FreeForAll, 800, 2),
            password: None,
            joiner: test_server::create_joiner_serial_in("First", TeamChoiceKindSerialIn::Auto),
        })
        .await;
    let creator_player: TestPlayer = TestPlayer::enter(creator).await;
    let _second_player: TestPlayer = test_server::join_game(&test_server, creator_player.game_id, "Second").await;
    let mut third_client: TestClient = TestClient::connect(&test_server).await;

    third_client.send(&test_server::join_game_message(creator_player.game_id, None, "Third")).await;

    assert_eq!(
        third_client.receive().await,
        rejection(RequestKindSerialOut::JoinGame, RejectionKindSerialOut::GameFull),
    );
}

#[actix_web::test]
async fn join_game_rejects_an_unbalancing_team() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut creator: TestClient = TestClient::connect(&test_server).await;
    creator
        .send(&MessageSerialIn::CreateGame {
            settings: test_server::create_settings_serial_in("Teams", GameModeKind::Skirmish, 800, 8),
            password: None,
            joiner: test_server::create_joiner_serial_in("Red", TeamChoiceKindSerialIn::Team(TeamKindSerial::Red)),
        })
        .await;
    let creator_player: TestPlayer = TestPlayer::enter(creator).await;
    let mut joiner: TestClient = TestClient::connect(&test_server).await;

    joiner
        .send(&MessageSerialIn::JoinGame {
            game_id: creator_player.game_id,
            password: None,
            joiner: test_server::create_joiner_serial_in("Also red", TeamChoiceKindSerialIn::Team(TeamKindSerial::Red)),
        })
        .await;

    assert_eq!(
        joiner.receive().await,
        rejection(
            RequestKindSerialOut::JoinGame,
            RejectionKindSerialOut::TeamUnbalanced {
                requested: TeamKindSerial::Red,
                smaller: TeamKindSerial::Blue,
            },
        ),
    );
}

#[actix_web::test]
async fn dispatch_rejects_requests_outside_their_phase() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut player: TestPlayer = test_server::create_game(&test_server, "Arena", "Blob").await;
    let mut lobby_client: TestClient = TestClient::connect(&test_server).await;

    lobby_client.send(&MessageSerialIn::LeaveGame).await;
    player.client.send(&test_server::create_game_message("Second", None, "Blob")).await;

    assert_eq!(
        lobby_client.receive().await,
        rejection(RequestKindSerialOut::LeaveGame, RejectionKindSerialOut::NotInGame),
    );
    assert_eq!(
        player
            .client
            .receive_until(|message_serial_out| matches!(message_serial_out, MessageSerialOut::RequestRejected { .. }))
            .await,
        rejection(RequestKindSerialOut::CreateGame, RejectionKindSerialOut::AlreadyInGame),
    );
}
