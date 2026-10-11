// Each test binary compiles this helper and uses only part of it.
#![allow(dead_code)]

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::handshake::client::{Request, Response};
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::http::header;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::{Error as WebSocketError, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use server::CreatedServer;
use server::config::ServerConfig;
use server::registries::ServerRegistries;
use server::shutdown::ServerRunState;
use shared::ability::{FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use shared::game;
use shared::game::{GameModeKind, GameState, InputBundle, Tick};
use shared::member::{Appearance, MemberId, OrganismColorKind, SkinKind};
use shared::organism::Organism;
use shared::protocol;
use shared::protocol::state_checksum;
use shared::protocol::{
    GameModeKindSerial, GameSettingsSerialIn, JoinerSerialIn, LoadoutSerial, MessageSerialIn, MessageSerialOut,
    TeamChoiceKindSerialIn, WorldShapeKindSerial,
};

pub const TEST_ORIGIN: &str = "http://127.0.0.1:3011";
/// For one awaited message.
pub const RECEIVE_TIMEOUT: Duration = Duration::from_secs(10);
/// For a sequence of messages, up to the longest test.
pub const PLAY_TIMEOUT: Duration = Duration::from_secs(120);

pub type TestSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub struct TestServer {
    pub address: SocketAddr,
    pub registries: ServerRegistries,
    pub run_state: watch::Sender<ServerRunState>,
}

pub struct TestClient {
    stream: TestSocket,
}

impl TestClient {
    pub async fn connect(test_server: &TestServer) -> TestClient {
        TestClient::try_connect(test_server, Some(TEST_ORIGIN), Some(protocol::PROTOCOL_VERSION))
            .await
            .expect("the test client connects")
    }

    pub async fn try_connect(
        test_server: &TestServer,
        origin: Option<&str>,
        protocol_version: Option<u16>,
    ) -> Result<TestClient, WebSocketError> {
        let query: String = protocol_version.map_or(String::new(), |version| format!("?protocol_version={version}"));
        let mut request: Request = format!("ws://{}/ws{query}", test_server.address).into_client_request()?;

        if let Some(origin) = origin {
            request.headers_mut().insert(header::ORIGIN, HeaderValue::from_str(origin).unwrap());
        }

        let (stream, _): (TestSocket, Response) = tokio_tungstenite::connect_async(request).await?;

        Ok(TestClient { stream })
    }

    pub fn split(self) -> (SplitSink<TestSocket, Message>, SplitStream<TestSocket>) {
        self.stream.split()
    }

    pub async fn send(&mut self, message_serial_in: &MessageSerialIn) {
        let frame: Vec<u8> = protocol::encode_message_in(message_serial_in);

        self.send_raw(Message::binary(frame)).await;
    }

    pub async fn send_raw(&mut self, message: Message) {
        self.stream.send(message).await.expect("the test client sends");
    }

    /// The next server message; fails on a close or after the receive timeout.
    pub async fn receive(&mut self) -> MessageSerialOut {
        let deadline: Instant = Instant::now() + RECEIVE_TIMEOUT;

        loop {
            let frame: Message = self.receive_frame(deadline).await;

            match frame {
                Message::Binary(frame) => {
                    return protocol::decode_message_out(&frame).expect("a server message decodes");
                }
                Message::Ping(_) | Message::Pong(_) => continue,
                other_message => panic!("expected a server message, received {other_message:?}"),
            }
        }
    }

    pub async fn receive_until(&mut self, mut is_wanted: impl FnMut(&MessageSerialOut) -> bool) -> MessageSerialOut {
        let receiving = async {
            loop {
                let message_serial_out: MessageSerialOut = self.receive().await;

                if is_wanted(&message_serial_out) {
                    return message_serial_out;
                }
            }
        };

        tokio::time::timeout(PLAY_TIMEOUT, receiving)
            .await
            .expect("the wanted message arrives within the play timeout")
    }

    /// The close code and reason; game frames before the close are skipped.
    pub async fn receive_close(&mut self) -> (u16, String) {
        let deadline: Instant = Instant::now() + RECEIVE_TIMEOUT;

        loop {
            let frame: Message = self.receive_frame(deadline).await;

            match frame {
                Message::Close(Some(CloseFrame { code, reason })) => return (u16::from(code), reason.to_string()),
                Message::Close(None) => panic!("the server closed without a close frame"),
                _ => continue,
            }
        }
    }

    /// Pings would restart a per-frame timeout, so the deadline is the caller's.
    async fn receive_frame(&mut self, deadline: Instant) -> Message {
        let frame: Option<Result<Message, WebSocketError>> = tokio::time::timeout_at(deadline, self.stream.next())
            .await
            .expect("a frame arrives within the receive timeout");

        frame.expect("the connection is open").expect("the frame is valid")
    }
}

/// A client which follows its game by stepping every bundle and checking every checksum.
pub struct TestPlayer {
    pub client: TestClient,
    pub game_id: u32,
    pub member_id: MemberId,
    pub state: GameState,
    pub verified_checksum_count: u32,
}

impl TestPlayer {
    /// Reads `JoinAccepted` then the snapshot.
    pub async fn enter(mut client: TestClient) -> TestPlayer {
        let MessageSerialOut::JoinAccepted { game_id, member_id } = client.receive().await else {
            panic!("expected JoinAccepted");
        };
        let MessageSerialOut::Snapshot(snapshot_serial_out) = client.receive().await else {
            panic!("expected the snapshot after JoinAccepted");
        };

        assert_eq!(snapshot_serial_out.game_id, game_id);

        TestPlayer {
            client,
            game_id,
            member_id: MemberId(member_id),
            state: GameState::try_from(snapshot_serial_out.state).expect("the snapshot converts"),
            verified_checksum_count: 0,
        }
    }

    /// Applies one game message; returns the bundle when it was one.
    pub fn apply(&mut self, message_serial_out: &MessageSerialOut) -> Option<InputBundle> {
        match message_serial_out {
            MessageSerialOut::InputBundle(bundle_serial_out) => {
                let bundle: InputBundle =
                    InputBundle::try_from(bundle_serial_out.clone()).expect("the bundle converts");
                game::step(&mut self.state, &bundle).expect("the bundle steps");

                Some(bundle)
            }
            MessageSerialOut::StateChecksum { tick, checksum } => {
                assert_eq!(*tick, self.state.tick.0);
                assert_eq!(*checksum, state_checksum::get_state_checksum(&self.state));
                self.verified_checksum_count += 1;

                None
            }
            MessageSerialOut::Snapshot(snapshot_serial_out) => {
                self.state = GameState::try_from(snapshot_serial_out.state.clone()).expect("the snapshot converts");

                None
            }
            _ => None,
        }
    }

    /// Receives and applies messages until `is_done` holds for the player.
    pub async fn play_until(&mut self, mut is_done: impl FnMut(&TestPlayer) -> bool) {
        let playing = async {
            while !is_done(self) {
                let message_serial_out: MessageSerialOut = self.client.receive().await;
                self.apply(&message_serial_out);
            }
        };

        tokio::time::timeout(PLAY_TIMEOUT, playing)
            .await
            .expect("the player gets there within the play timeout");
    }

    /// Receives and applies messages until the next bundle, which it returns.
    pub async fn receive_bundle(&mut self) -> InputBundle {
        loop {
            let message_serial_out: MessageSerialOut = self.client.receive().await;
            let bundle: Option<InputBundle> = self.apply(&message_serial_out);

            if let Some(bundle) = bundle {
                return bundle;
            }
        }
    }

    pub fn get_own_organism(&self) -> Option<&Organism> {
        self.state.members.get(&self.member_id).and_then(|member| member.organism.as_ref())
    }

    pub fn applied_tick(&self) -> Tick {
        self.state.tick
    }
}

/// Binds a free port with fresh registries; must run inside an actix system.
pub fn start_test_server(customize_config: impl FnOnce(&mut ServerConfig)) -> TestServer {
    let mut config: ServerConfig = ServerConfig {
        bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port: 0,
        site_root: None,
        allowed_origins: vec![String::from(TEST_ORIGIN)],
        replay_capture_directory: None,
        tls_files: None,
        empty_game_grace_period: Duration::from_secs(10),
        tick_period: Duration::from_millis(70),
    };

    customize_config(&mut config);

    let registries: ServerRegistries = ServerRegistries::leaked();
    let created_server: CreatedServer = server::create_server(config, registries).expect("the test server binds");

    actix_web::rt::spawn(created_server.server);

    TestServer {
        address: created_server.local_addresses[0],
        registries,
        run_state: created_server.run_state,
    }
}

pub fn create_loadout_serial() -> LoadoutSerial {
    LoadoutSerial::from(&Loadout {
        appearance: Appearance {
            color: OrganismColorKind::Sky,
            skin: SkinKind::Grid,
        },
        first: FirstAbilityKind::Extend,
        second: SecondAbilityKind::Immortality,
        third: ThirdAbilityKind::Toxin,
    })
}

pub fn create_joiner_serial_in(screen_name: &str, team: TeamChoiceKindSerialIn) -> JoinerSerialIn {
    JoinerSerialIn {
        screen_name: String::from(screen_name),
        loadout: create_loadout_serial(),
        team,
    }
}

/// A square rectangle world; survival needs 2 players and skirmish has 2 teams.
pub fn create_settings_serial_in(
    title: &str,
    mode: GameModeKind,
    world_size_pixels: u32,
    player_cap: u8,
) -> GameSettingsSerialIn {
    GameSettingsSerialIn {
        title: String::from(title),
        mode: GameModeKindSerial::from(&mode),
        world_shape: WorldShapeKindSerial::Rectangle,
        world_size_pixels,
        player_minimum: (mode == GameModeKind::Survival).then_some(2),
        player_cap,
        team_count: (mode == GameModeKind::Skirmish).then_some(2),
        leaderboard_length: 10,
    }
}

pub fn create_game_message(title: &str, password: Option<&str>, screen_name: &str) -> MessageSerialIn {
    MessageSerialIn::CreateGame {
        settings: create_settings_serial_in(title, GameModeKind::FreeForAll, 800, 8),
        password: password.map(String::from),
        joiner: create_joiner_serial_in(screen_name, TeamChoiceKindSerialIn::Auto),
    }
}

pub fn join_game_message(game_id: u32, password: Option<&str>, screen_name: &str) -> MessageSerialIn {
    MessageSerialIn::JoinGame {
        game_id,
        password: password.map(String::from),
        joiner: create_joiner_serial_in(screen_name, TeamChoiceKindSerialIn::Auto),
    }
}

/// Connects, creates a free-for-all game and enters it.
pub async fn create_game(test_server: &TestServer, title: &str, screen_name: &str) -> TestPlayer {
    let mut client: TestClient = TestClient::connect(test_server).await;
    client.send(&create_game_message(title, None, screen_name)).await;

    TestPlayer::enter(client).await
}

/// Connects and joins `game_id`.
pub async fn join_game(test_server: &TestServer, game_id: u32, screen_name: &str) -> TestPlayer {
    let mut client: TestClient = TestClient::connect(test_server).await;
    client.send(&join_game_message(game_id, None, screen_name)).await;

    TestPlayer::enter(client).await
}
