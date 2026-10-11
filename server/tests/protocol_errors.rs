mod helpers;

use tokio_tungstenite::tungstenite::{Error as WebSocketError, Message};

use helpers::test_server;
use helpers::test_server::{TestClient, TestServer};
use server::shutdown::ServerRunState;
use server::socket;
use shared::protocol;
use shared::protocol::protocol_limits;
use shared::protocol::{MessageSerialIn, MessageSerialOut, RejectionKindSerialOut, RequestKindSerialOut};

fn rejection(request: RequestKindSerialOut, reason: RejectionKindSerialOut) -> MessageSerialOut {
    MessageSerialOut::RequestRejected { request, reason }
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
