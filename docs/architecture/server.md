# Server

## Process
- `#[actix_web::main]`; `HttpServer::new(|| App::new().configure(socket_api::configurer).configure(site_api::configurer))`.
- `ServerConfig` from environment (dotenvy loads `.env` when present):
  - `BACTER_BIND_ADDRESS` (default `127.0.0.1`), `BACTER_PORT` (default `3100`).
  - `BACTER_SITE_ROOT` (unset in dev: no static serving).
  - `BACTER_ALLOWED_ORIGINS` (comma list; dev default `http://127.0.0.1:3011,http://localhost:3011`, matching the web `site-addr`).
  - `BACTER_REPLAY_CAPTURE_DIRECTORY` (optional), `RUST_LOG`.
  - `BACTER_TLS_CERTIFICATE_PATH`, `BACTER_TLS_PRIVATE_KEY_PATH` (optional, PEM): both set binds with TLS; neither set binds plain HTTP; only one set is a startup error.
- TLS (`config/tls.rs`, owner decision): rustls inside actix-web, `HttpServer::bind_rustls_0_23` with a `rustls::ServerConfig` built by `rustls::ServerConfig::builder().with_no_client_auth().with_single_cert(chain, key)`; chain and key read with `CertificateDer::pem_file_iter` and `PrivateKeyDer::from_pem_file` (`rustls::pki_types::pem::PemObject`). Otherwise `HttpServer::bind`.
- Cloudflare may proxy in front: its Full (strict) mode connects to the origin over TLS (needs the TLS variables, with a Cloudflare origin certificate or any trusted one); Flexible connects over plain HTTP. The mode is chosen at deploy time.
- `/ws` rejects a handshake whose `Origin` is not allowed (actix does not check it).
- Logging: `log` + `env_logger`, format `<message>; [key=value ...]`, for example `game created; [game_id=7 mode=srv]`.

## Tasks and channels

```
connection read task   --GameInput (mpsc, bounded 1024, try_send)-->                 game task
connection read task   --GameRequest (mpsc, bounded 64, send().await)-->             game task
connection writer task <--Bytes (mpsc, bounded 256, new per membership)--            game task
connection writer task <--Bytes (mpsc, bounded 16)--                                 lobby task
connection writer task <--WriterCommand (mpsc, bounded 32, send().await)--           connection read task
connection read task   <--ConnectionControl (watch, sender owned by the game task)-- game task
lobby task (1 per server instance) reads GAMES summaries
game supervisor (1 per game) awaits the game task's JoinHandle
```

- Two tasks per socket, both `actix_web::rt::spawn` on the accepting actix worker:
  - Read task (`socket/connection.rs`): owns `ConnectionPhase`; `tokio::select!` over `stream.recv()`, `control.changed()` (while `InGame`), the writer task's exit and the ping interval; never writes to the socket itself.
  - Writer task (`socket/socket_writer.rs`): the only owner of `actix_ws::Session`; `select!` over the `WriterCommand` receiver, the current membership receiver (when attached) and the lobby receiver; each socket write wrapped in `tokio::time::timeout(SOCKET_WRITE_TIMEOUT)`, `SOCKET_WRITE_TIMEOUT = 5 s`; expiry drops the session without a close frame (the peer is not reading), which ends the response stream and the TCP connection, and ends the task.
  - A stalled peer therefore blocks only the writer, never the read task's control and timeout handling.
- `WriterCommand { Frame(Bytes), AttachMembership(mpsc::Receiver<Bytes>), DetachMembership { then_send: Option<Bytes> }, Close(CloseReasonKind) }`.
  - `DetachMembership` drops the membership receiver unread, then writes `then_send`, so no frame of the old game can follow `LeftGame` or `GameEnded`.
- Read task: answers `Ping` with `pong` (through the writer); sends `Ping` every 10 s; closes after 30 s without any inbound frame; `max_frame_size(4096)`; continuation aggregation off.
- Read task end (close, error, timeout, writer gone): sends `GameRequest::ConnectionClosed` with `send().await` when `InGame`, removes its lobby subscription, releases its connection-limit slots.
- Loop control is an enum (`ConnectionFlow::Continue | Close(CloseReasonKind)`), not a bool.
- Game task owns `GameState`, never awaits a send; each frame is encoded once into `Bytes` and cloned per recipient.
- Channel policies:
  - `GameInput` (inputs only): `try_send`; `Full` drops the input and counts it.
  - `GameRequest` (join, spectate, respawn, appearance, leave, connection closed, snapshot request): never dropped. `Leave`, `ConnectionClosed` and `RequestSnapshot` use `send().await` without a timeout; join, spectate, respawn and appearance use `REQUEST_SEND_TIMEOUT = 2 s`, after which the request is answered `ServerBusy`.
  - Membership `Bytes` (game frames): `try_send`. `Full`: the game task stops queueing bundles for that recipient and marks `snapshot_pending`; when the queue has drained below half capacity it sends a fresh `Snapshot` and resumes bundles after it (the member stays in the game). Still full after `SLOW_CONSUMER_LIMIT = 10 s`: `ConnectionControl::Close(SlowConsumer)`, recipient dropped, `MemberEvent::Left` queued. `Closed`: recipient dropped and `Left` queued. Capacity 256 bundles is about 18 s.
  - Each tick the game task also checks `outbound.is_closed()` for every recipient and queues `MemberEvent::Left` for any closed one, so membership never depends on `ConnectionClosed` arriving.
  - Lobby `Bytes` (`GameList`): `try_send`; `Full` drops that list only (lists are latest-state).
- Leave: read task sends `GameRequest::Leave { member_id, reply }`; the game task removes the recipient, queues `MemberEvent::Left`, replies; the read task then sends `DetachMembership { then_send: LeftGame }` and moves to `Lobby`. A failed send or a dropped reply means the game is gone and is handled the same way.
- Join: a fresh membership channel and control watch per membership; the read task attaches the receiver to the writer only after the join reply is `Ok`, so frames queued by the game meanwhile wait in the receiver.

## Registries
- Modules: `registries/registries.rs`, `game/game_registry.rs`.
- In-memory only; revisited when persistence is added ([deferred work](./overview.md#deferred-work)).
- Statics, each in its feature module:
  - `GAMES: LazyLock<DashMap<GameId, GameHandle>>`.
  - `GAME_TITLES: LazyLock<DashMap<String, GameId>>`; reservation through `entry()` is atomic.
  - `NEXT_GAME_ID: AtomicU32`, `GAME_COUNT: AtomicU32`.
  - `CONNECTION_COUNT: AtomicU32`, `CONNECTIONS_PER_IP: LazyLock<DashMap<IpAddr, u32>>` (`socket/connection_limit.rs`).
  - `LOBBY_SUBSCRIBERS` ([`server` crate](./overview.md#server-crate)), `PASSWORD_FAILURES_PER_IP` (`password/password_failure_limit.rs`).
- `ServerRegistries` (`Clone, Copy`): one `&'static` reference per static. `ServerRegistries::production()` points at the statics; `ServerRegistries::leaked()` (tests) leaks fresh empty ones with `Box::leak`, so parallel in-process servers never share titles, games or subscribers.
- `create_server(config, registries)` passes the struct to every connection task, game task, supervisor and the lobby task; no other code names the statics.
- Counted limits (games, connections, connections per IP) are reserved atomically: `fetch_add`, and `fetch_sub` with a rejection when the previous value was already at the limit; per IP through `entry()` under the shard lock.
  - Over the game limit: `CreateGame` gets `ServerFull`.
  - Over a connection limit: the upgrade is accepted and closed at once with `ServerFull` (4006), so the client can show the notice.
- `GameHandle` (`Clone`) `{ requests: mpsc::Sender<GameRequest>, inputs: mpsc::Sender<GameInput>, summary: watch::Receiver<GameSummary>, password_hash: Option<String>, settings: GameSettings }`.
  - Callers clone the handle out of the map (`games.get(&id).map(|entry| entry.value().clone())`) and drop the map reference before any await.
- `GameRequest { Join { endpoint: ConnectionEndpoint, joiner: Joiner, reply: oneshot::Sender<Result<MemberId, RejectionKind>> }, Spectate {..}, Respawn {..}, UpdateAppearance {..}, Leave { member_id, reply: oneshot::Sender<()> }, ConnectionClosed { connection_id }, RequestSnapshot { member_id } }`.
- `GameInput { member_id, input: PlayerInput }`: the read task converts `PlayerInputSerialIn` before sending ([input validation and speed clamp](#input-validation-and-speed-clamp)).
- `ConnectionEndpoint { connection_id: ConnectionId, outbound: mpsc::Sender<Bytes>, control: watch::Sender<ConnectionControl> }`; the connection keeps only the matching receivers.
- A send to a closed request channel (`SendError`, `TrySendError::Closed`) and a dropped reply (`oneshot::error::RecvError`) both mean `GameNotFound`.

## Game task
- Module: `game/game_task.rs`.
- Game tasks and supervisors run on `static GAME_RUNTIME: LazyLock<tokio::runtime::Runtime>` (`game/game_runtime.rs`, multi-threaded, worker count from `std::thread::available_parallelism`), spawned through `GAME_RUNTIME.spawn`, never on an actix worker.
- `tokio::time::interval_at(Instant::now() + 70 ms, 70 ms)` with `MissedTickBehavior::Burst`, so the tick count tracks elapsed time.
  - More than `TICK_LAG_RESET_THRESHOLD = 43` consecutive late ticks (about 3 s behind): `interval.reset()` and a log line; the tick count, not wall time, is the game clock, and each client's backlog check then triggers one resync ([checksums and resync](./protocol.md#checksums-and-resync)).
- `select!` over `interval.tick()`, `requests.recv()` and `inputs.recv()`; after each tick, up to `COMMAND_DRAIN_LIMIT_PER_TICK = 4096` queued requests and inputs are drained with `try_recv`, so a tick burst cannot starve them.
- Per tick:
  1. `bundle_builder` turns admitted membership events (arrival order) and the latest input per member into an `InputBundle`, applying the cursor clamp; `PendingAdmissions` is cleared.
  2. `shared::game::step(&mut state, &bundle)`.
  3. Convert the bundle to `InputBundleSerialOut`; encode it once as a `MessageSerialOut::InputBundle` frame and fan out; hand the `InputBundleSerialOut` itself to replay capture, whose writer task encodes it separately with `protocol::encode_replay_bundle` (a replay record is not a message frame, [wire types](./protocol.md#wire-types)); checksum message on cadence; pending snapshots whose interval has passed ([checksums and resync](./protocol.md#checksums-and-resync)).
  4. Recipient liveness check ([tasks and channels](#tasks-and-channels)).
  5. Handle `SimulationEvent`s: clamp reset on spawn, spawn tick recorded, summary update, logs.
  6. Empty-game check ([empty-game removal](#empty-game-removal)).
- On an admitted join or spectate (between ticks): enqueue `JoinAccepted` then `Snapshot` (`GameStateSerialOut` of the state after the last completed tick T) on the joiner's membership channel, add the recipient, queue `MemberEvent::Joined` (plus `SpawnRequested` when it spawns) for bundle T+1, reply `Ok(member_id)`.
- Inputs arriving after the bundle for tick t is built apply to t+1; the server never waits for a client and never predicts.
- Replay capture when `BACTER_REPLAY_CAPTURE_DIRECTORY` is set (`game/replay_capture.rs`):
  - A writer task per game on `GAME_RUNTIME` appends to `<directory>/<game_id>.replay` as records are produced: the version prefix, one `ReplayHeaderSerialOut { settings: GameSettingsSerialOut, seed: u64 }`, then one `InputBundleSerialOut` record per bundle ([wire types](./protocol.md#wire-types)); flushed after every record.
  - The file is a valid prefix at every moment, so a panic loses nothing already written and memory use stays constant.
  - Bounded channel with `try_send`; `Full` abandons the capture for that game with a log line, never stalls the tick.
  - The game starts with no members ([lifecycle](#lifecycle)), so header plus bundles reproduce it exactly.

## Input validation and speed clamp
- Module: `game/cursor_clamp.rs`, server only.
- Decode, then `TryFrom<PlayerInputSerialIn> for PlayerInput` (failure closes the connection, [failure isolation](#failure-isolation)):
  - `aim` only with a shot-slot press;
  - no unknown press bits;
  - cursor within `±2^28` subpixels per axis ([limits](./protocol.md#limits)).
- Then, same failure: `client_tick` not greater than the server tick.
- In the game task, not a protocol error:
  - `client_tick` not strictly greater than the member's last accepted `client_tick`: dropped and counted.
  - Inputs from members without an organism: ignored.
  - `client_tick` before the member's latest spawn tick: ignored (sampled before the client saw its spawn).
- Speed: 42.5 px/s, so `MOVE_SUBPIXELS_PER_TICK = 3046` (2.975 px is 3046.4 subpixels).
- Budget per member: `+3046` per tick, capped at `MOVE_BUDGET_CAP_TICKS = 15` ticks (about 1 s), plus `MOVE_TOLERANCE_SUBPIXELS_PER_TICK = 128` for quantisation.
- Each tick, latest input target versus last applied cursor, differences in i64: within budget applies it and spends the distance; beyond budget moves along the same direction by the budget, empties it, and sends `CursorCorrected`.
- Reset to the spawn cursor on `OrganismSpawned`.
- Not part of the simulation; floats are acceptable here.

## Lifecycle
- Create (read task):
  1. Shared validation: `GameSettings::try_from(GameSettingsSerialIn)` ([wire types](./protocol.md#wire-types)).
  2. Reserve the title (`entry()`), then a game slot (`GAME_COUNT`); any later failure releases both.
  3. Hash the password ([passwords](#passwords)).
  4. Draw a `u64` seed from `getrandom`; `GameState::new(settings, seed)` ([determinism rules](./determinism.md#determinism-rules)): tick 0, no members.
  5. Insert the `GameHandle` into `GAMES`, then spawn the game task and its supervisor.
  6. Send the creator's `GameRequest::Join` through the normal join path: `JoinAccepted`, `Snapshot` of the empty tick-0 state, and `Joined` plus `SpawnRequested` in bundle 1, as for any joiner.
- Join (read task): clone the handle, verify the password ([passwords](#passwords)), send `GameRequest::Join`; the game task admits or rejects it (below) and replies once.
- Admission (`game/membership.rs`, `game/pending_admissions.rs`): every check reads `GameState` plus `PendingAdmissions` (screen names, spawn count, team sizes, members and spectators admitted since the last bundle, next member id), so two requests in one tick cannot both take the last slot, the same name or the same team place.
  - Name unique among members and pending joins.
  - Member limit: members plus pending below `player_cap + 64`, else `GameFull`.
  - Spawning: alive organisms plus pending spawns below `player_cap`, else `GameFull`.
  - Team balance against current plus pending team sizes.
  - srv while Playing or PostRound: joins as a Participant without an organism (no `SpawnRequested`); the message box shows "Wait for the round to complete".
  - Member id `state.next_member_id + pending joins` ([input bundle](./protocol.md#input-bundle)).
  - With these checks `step` never declines a spawn the server admitted; `SpawnRejected { PositionNotFound }` (no free position in 64 candidates) is the only rejection a server game can produce, and the client shows "No room to spawn; press 'R' to try again" ([client session, frame pacing and driving](./client-web.md#client-session-frame-pacing-and-driving)).
- Spectate: password, name uniqueness, spectators plus pending below 64 (`SpectatorLimitReached`), member limit; role Spectator, no loadout.
- Die: simulation event; the member stays.
  - ffa and skm: may `Respawn` at once.
  - srv: may `Respawn` while Waiting or PreRound ([world, spawn, rounds](./simulation.md#world-spawn-rounds)); while Playing or PostRound waits for the next force spawn.
- Respawn: `NotDead` (organism or pending spawn), `GameFull`, `RoundInProgress`, `TeamUnbalanced`; emits `SpawnRequested`.
- Leave and disconnect share one path: `MemberEvent::Left` in the next bundle ([tasks and channels](#tasks-and-channels)).
- Empty-game grace ([empty-game removal](#empty-game-removal)).
- One membership per connection; requests outside the right `ConnectionPhase` get `AlreadyInGame` or `NotInGame`.
- `ConnectionPhase { Lobby, InGame { game_id, member_id } }`; a connection starts in `Lobby` once the version check ([transport, codec, version](./protocol.md#transport-codec-version)) has passed.

## Empty-game removal
- `EMPTY_GAME_GRACE_PERIOD: Duration = Duration::from_secs(10)`; default of `ServerConfig.empty_game_grace_period` (tests set it short).
- When the member count reaches zero the task records `empty_since: Instant`; a join before the period ends clears it.
- After the period the task closes its request and input receivers, answers queued requests with `GameNotFound`, and returns.
- The supervisor is the only code that removes registry entries, whichever way the task ended (normal return or panic, logged): `games.remove(&game_id)`, `game_titles.remove_if(&title, |_, reserved_id| *reserved_id == game_id)`, `game_count.fetch_sub(1)`, so a newer game reusing the title keeps its reservation.

## Passwords
- Module: `password/password.rs`.
- bcrypt, cost 10 (Singularity default), on `web::block`; the closure returns `Result<_, AppErrorStatic>` ([`shared` crate](./overview.md#shared-crate)), since `web::block` needs a `Send` result.
- Hash stored only in `GameHandle`; plaintext never stored or logged.
- `static PASSWORD_CHECK_SEMAPHORE: LazyLock<tokio::sync::Semaphore>` with one permit per available CPU bounds concurrent hashing and verification; `try_acquire` failing answers `ServerBusy`.
- Verified once per join or spectate request.
- Limits:
  - `PASSWORD_FAILURE_LIMIT = 5` failures per connection closes it (`PasswordFailureLimit`, 4003).
  - 20 failures per IP per 10 minutes ([limits](./protocol.md#limits)): further attempts from that IP get `RateLimited` without hashing.

## Rate limits
- Module: `socket/rate_limit.rs`.
- Token buckets per connection:
  - `Input`: 30/s, burst 45 (nominal 14.29/s plus catch-up).
  - Requests (create, join, spectate, respawn, appearance, leave): 4/s, burst 8.
- `RequestSnapshot` has no bucket: the game task coalesces repeats and enforces the minimum interval itself ([checksums and resync](./protocol.md#checksums-and-resync)).
- Exceeding: inputs dropped; requests answered `RateLimited`; sustained abuse (bucket empty for 10 s) closes the connection (`RateLimitAbuse`, 4004).
- Connection limits per server and per IP ([limits](./protocol.md#limits)) are checked at upgrade ([registries](#registries)).

## Failure isolation
- Decode, conversion or validation errors affect only that connection (`ProtocolViolation`, 1008); request payload conversions are the exception and answer `RequestRejected` ([wire types](./protocol.md#wire-types)).
- Handlers return `Result`; nothing indexes by a client-supplied position.
- A panic inside a game task ends that game only: the supervisor cleans the registries ([empty-game removal](#empty-game-removal)), and dropping the task's state drops every member's `watch::Sender<ConnectionControl>`. Each member's read task sees `control.changed()` return `Err` while `InGame`, sends `DetachMembership { then_send: GameEnded }` and moves to `Lobby`; the client returns to the browser with the notice "The game has closed".

## Production static serving
- Module: `site/site_api.rs`.
- `actix_files::Files::new("/", site_root).index_file("index.html")` when `BACTER_SITE_ROOT` is set.
- `/pkg/*`: `Cache-Control: public, max-age=31536000, immutable`; `index.html` and `game-server.json`: `no-cache`.

## Adding an account service later
- New `server/src/account/` feature module (or a separate service) plus `account_db.rs` when a database arrives.
- Identity attaches at the connection layer: `ConnectionIdentity { Anonymous, Account { account_id } }` on the connection; members stay keyed by `MemberId`, so `shared` simulation and the game task do not change.
- Protocol version bump adds a first message `MessageSerialIn::Authenticate { session_ticket: String }` after the upgrade (a browser WebSocket cannot set headers); the version check stays outside bitcode ([transport, codec, version](./protocol.md#transport-codec-version)).
- Results: the game task emits `SimulationEvent`s already; an account task consumes `OrganismDied`/`RoundWon` through a bounded channel.
- Secrets arrive with it: eafora's `secr` pattern (`static` store, workspace-relative path), `secrets.yaml`, `MASTER_SECRET`.
