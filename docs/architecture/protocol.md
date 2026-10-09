# Protocol

## Transport, codec, version
- One binary WebSocket per client at `/ws?protocol_version=<PROTOCOL_VERSION>`; text frames are a protocol error.
- Version check outside bitcode: the server reads the query parameter before decoding any frame. Missing or different: the upgrade is accepted and immediately closed with code 4001, reason text `server protocol version <N>` (plain WebSocket close, no bitcode on either side), so an old tab after a redeploy can always read the reason.
- Every message after the upgrade is one frame: `bitcode::encode(&MessageSerialIn)` (client to server) or `bitcode::encode(&MessageSerialOut)` (server to client). There is no `Hello` message.
- `protocol::encode_message_in`, `decode_message_in`, `encode_message_out`, `decode_message_out`; decode returns `Result<_, AppError>`; then each payload converts to its model through `TryFrom`, which carries the semantic validation ([wire types](#wire-types)).
- Naming: `SerialIn` is client to server, `SerialOut` server to client, `Serial` when both directions are byte-identical (`docs/conventions/types.md`); every type on the wire or in a replay file is a dedicated wire type (owner decision, [wire types](#wire-types)).
- `PROTOCOL_VERSION: u16 = 1`; bumped on any change to a wire type (bitcode format stability across versions is a non-goal of bitcode).
- No `usize`, no zero-sized types in vectors.

## Client to server
- Type: `MessageSerialIn`.
- `SubscribeGameList`, `UnsubscribeGameList`: `Lobby` phase only ([lobby](#lobby)).
- `CreateGame { settings: GameSettingsSerialIn, password: Option<String>, joiner: JoinerSerialIn }`: creates the game and joins the creator in one message.
- `JoinGame { game_id: u32, password: Option<String>, joiner: JoinerSerialIn }`.
- `SpectateGame { game_id: u32, password: Option<String>, screen_name: String }`.
- `Respawn { loadout: LoadoutSerial, team: TeamChoiceKindSerialIn }`.
- `UpdateAppearance { appearance: AppearanceSerial }` (pause menu Apply; colour ignored in skm).
- `Input(PlayerInputSerialIn)`.
- `RequestSnapshot { client_tick: u32 }` after a checksum mismatch, a step error or a backlog ([checksums and resync](#checksums-and-resync)); never rejected.
- `LeaveGame`.
- Payloads:
  - `GameSettingsSerialIn { title: String, mode: GameModeKindSerial, world_shape: WorldShapeKindSerial, world_size_pixels: u32, player_minimum: Option<u8>, player_cap: u8, team_count: Option<u8>, leaderboard_length: u8 }`; `player_minimum` only for srv, `team_count` only for skm (others are a validation error).
  - `JoinerSerialIn { screen_name: String, loadout: LoadoutSerial, team: TeamChoiceKindSerialIn }`, converted to `Joiner` (`member/joiner.rs`).
  - `TeamChoiceKindSerialIn { Auto, Team(TeamKindSerial) }`, converted to `TeamChoiceKind`; ignored outside skm.
  - `PlayerInputSerialIn { client_tick: u32, cursor: SubpixelPointSerial, ability_presses: u8, aim: Option<AimVectorSerial> }`, converted to `PlayerInput` (`game/player_input.rs`).
  - `AimVectorSerial { x: i16, y: i16 }` (model `AimVector`): mouse offset from the on-screen crosshair in CSS px at the shot press; present only with a shot-slot press. The crosshair is at the screen centre whenever the camera follows it; in the tutorial's fixed camera it is wherever the crosshair is drawn (faithful, `Ability.js:284-294`).
  - `client_tick`: tick of the last bundle the client applied when it sampled.
- Input sampling (client, `play/game_key.rs`):
  - One `Input` right after applying each bundle, never after a snapshot alone, only while the own member has an organism.
  - Ability presses are keydown edges; `KeyboardEvent.repeat` is ignored; edges accumulate since the last sample and are cleared when sent.
  - Edges made while the own member has no organism are discarded.

## Server to client
- Type: `MessageSerialOut`.
- `GameList { game_summaries: Vec<GameSummarySerialOut>, online_client_count: u32 }`.
- `JoinAccepted { game_id: u32, member_id: u32 }`.
- `Snapshot(GameSnapshotSerialOut)`.
- `InputBundle(InputBundleSerialOut)`.
- `StateChecksum { tick: u32, checksum: u64 }`.
- `CursorCorrected { client_tick: u32, cursor: SubpixelPointSerial }`: only to the clamped player, only when clamped.
- `RequestRejected { request: RequestKindSerialOut, reason: RejectionKindSerialOut }`.
- `LeftGame`: sent by the connection task itself after dropping its membership receiver ([tasks and channels](./server.md#tasks-and-channels)), so no game frame can follow it.
- `GameEnded`: the game's control channel closed with no close reason set (the game task ended abnormally); written by the writer task after detaching the membership ([failure isolation](./server.md#failure-isolation)); the connection is back in `Lobby`.
- `GameSummarySerialOut { game_id: u32, title: String, mode: GameModeKindSerial, player_count: u8, spectator_count: u8, player_cap: u8, secured: bool, team_sizes: Vec<u8> }`, the wire form of the model `GameSummary` (`game/game_summary.rs`: the same fields with `game_id: GameId`, `mode: GameModeKind`).
  - `player_count`: alive organisms (the original's player list).
  - `spectator_count`: members without an organism, pure Spectators and dead or waiting Participants (the original moved dead players to its spectator list).
- `GameSnapshotSerialOut { game_id: u32, state: GameStateSerialOut }`.
- `RequestKind { SubscribeGameList, CreateGame, JoinGame, SpectateGame, Respawn, UpdateAppearance, LeaveGame }`.
- `SettingFieldKind { WorldSize, PlayerMinimum, PlayerCap, TeamCount, LeaderboardLength }`; `RangeBoundKind { Below, Above }`.
- These and `RejectionKind` are models (`protocol/rejection.rs`), each with a `...SerialOut` mirror of the same variants (`TeamKindSerial` in place of `TeamKind`).
- `RejectionKind`, with the form field it attaches to and its text ("original" marks the original's string; "general" is the bottom error row; `<min>`/`<max>` from `protocol_limits.rs`):
  - `ScreenNameEmpty`: screen name; "Screen name cannot be left empty" (original).
  - `ScreenNameTooLong`: screen name; "Screen name can be at most `<max>` characters".
  - `ScreenNameInvalidCharacter`: screen name; "Screen name cannot contain control characters".
  - `ScreenNameTaken`: screen name; "Name matches that of another player" (original).
  - `TitleEmpty`: game title; "Title cannot be left blank" (original).
  - `TitleTooLong`: game title; "Title can be at most `<max>` characters".
  - `TitleInvalidCharacter`: game title; "Title cannot contain control characters".
  - `TitleTaken`: game title; "Title matches that of another game" (original).
  - `SettingOutOfRange { field: WorldSize, bound }`: world width; "Dimensions must be at least `<min>` x `<min>` px" or "Dimensions can be at most `<max>` x `<max>` px" (original).
  - `SettingOutOfRange { field: PlayerMinimum, bound: Below }`: player minimum; "Player minimum must be at least `<min>`" (original).
  - `SettingOutOfRange { field: PlayerCap, bound }`: player cap; "Player cap must be at least `<min>`" (original) or "Player cap can be at most `<max>`".
  - `SettingOutOfRange { field: TeamCount, bound }`: team count; "Team count must be at least `<min>`" or "Team count can be at most `<max>`" (original).
  - `SettingOutOfRange { field: LeaderboardLength, bound }`: leaderboard length; "Leaderboard length must be at least `<min>`" or "Leaderboard length can be at most `<max>`" (original).
  - `SettingNotApplicable { field }`: general; "Setting does not apply to this mode" (never produced by the web form).
  - `PlayerCapBelowMinimum`: player cap; "Player cap cannot be less than player minimum" (original; srv only).
  - `PlayerCapBelowTeamCount`: team count; "Player cap cannot be less than the number of teams" (original).
  - `PasswordRequired`: password; "A password is required for this game" (original).
  - `PasswordIncorrect`: password; "Password is invalid" (original).
  - `PasswordTooLong`: password; "Password can be at most 72 bytes".
  - `GameNotFound`: general; "The game has closed" (original).
  - `GameFull`: player cap (join) or general (respawn); "Game is at maximum player capacity" (original).
  - `SpectatorLimitReached`: general; "Game is at maximum spectator capacity".
  - `TeamUnbalanced { requested: TeamKind, smaller: TeamKind }`: auto assign; "Cannot join `<requested>` team because it already has more players than `<smaller>`" (original).
  - `RoundInProgress`: general; "Wait for the round to complete" (original message text).
  - `AlreadyInGame`, `NotInGame`, `NotDead`: general; "Request is not valid right now" (unreachable from the web UI; logged as a client fault).
  - `ServerFull`: general; "The server is at maximum game capacity".
  - `ServerBusy`: general; "The server is busy; try again".
  - `RateLimited`: general; "Too many requests; try again".
  - Whole-number errors of the original never arise: Number fields floor their input ([screens](./client-web.md#screens)).
- `CloseReasonKind` (`shared/src/protocol/close_reason.rs`, so the client maps codes to notices), close code, reason text, client notice ([shell](./client-web.md#shell)):
  - `ServerShutdown`: 1001; "server shutting down"; "Disconnected from the game server".
  - `ProtocolViolation`: 1008; short cause (decode, conversion, text frame, oversize, validation); "Disconnected: protocol error".
  - `InternalError`: 1011; none; "Disconnected from the game server".
  - `ProtocolVersionMismatch`: 4001; "server protocol version N"; "Bacter has been updated. Reload the page."
  - `SlowConsumer`: 4002; "slow consumer"; "Disconnected: the connection could not keep up".
  - `PasswordFailureLimit`: 4003; "password failure limit"; "Too many incorrect passwords".
  - `RateLimitAbuse`: 4004; "rate limit"; "Too many requests".
  - `InboundTimeout`: 4005; "inbound timeout"; "Disconnected from the game server".
  - `ServerFull`: 4006; "server full" (connection limits, [registries](./server.md#registries)); "The server is full".
  - Abnormal close (1006) or a failed open: "Cannot reach the game server".

## Input bundle
Simulation form (`game/input_bundle.rs`), what `step` consumes:
```
InputBundle { tick: Tick, member_events: Vec<MemberEvent>, player_inputs: Vec<PlayerTickInput> }
PlayerTickInput { member_id: MemberId, cursor: WorldPoint, ability_presses: AbilityPressSet, aim: Option<AimVector> }
MemberEvent {
  Joined { member_id, screen_name, role: MemberRoleKind, loadout: Option<Loadout>, team: Option<TeamKind> },
  Left { member_id },
  SpawnRequested { member_id, loadout: Loadout, team: Option<TeamKind> },
  AppearanceChanged { member_id, appearance: Appearance },
}
```
Wire form (`protocol/input_bundle_serial.rs`), what is broadcast and written to replay files:
```
InputBundleSerialOut { tick: u32, member_events: Vec<MemberEventSerialOut>, player_inputs: Vec<PlayerTickInputSerialOut> }
PlayerTickInputSerialOut { member_id: u32, cursor: WorldPointSerialOut, ability_presses: u8, aim: Option<AimVectorSerial> }
MemberEventSerialOut {
  Joined { member_id: u32, screen_name: String, role: MemberRoleKindSerialOut, loadout: Option<LoadoutSerial>, team: Option<TeamKindSerial> },
  Left { member_id: u32 },
  SpawnRequested { member_id: u32, loadout: LoadoutSerial, team: Option<TeamKindSerial> },
  AppearanceChanged { member_id: u32, appearance: AppearanceSerial },
}
```
- The server builds an `InputBundle`, steps it, then encodes `InputBundleSerialOut::from(&bundle)`; the client decodes, converts with `InputBundle::try_from` and steps the result.
- One bundle per tick, broadcast identically to every member connection of the game.
- `Joined` never spawns. When the joiner may spawn, the server puts `SpawnRequested` for it directly after its `Joined` in the same bundle; events keep server arrival order.
- Member ids: the server allocates them as `state.next_member_id + pending joins` ([lifecycle](./server.md#lifecycle)), so `JoinAccepted` can carry the id before the event is stepped. `step` requires `member_id >= state.next_member_id` for every `Joined` (else `StepError::MemberIdOutOfOrder`, state untouched) and sets `next_member_id = member_id + 1`.
- `cursor` is the clamped server cursor rounded to whole px (what the simulation consumes).
- A member appears in `player_inputs` only when its cursor changed or it pressed something; otherwise the previous cursor persists.
- Presses received during the tick are OR-ed; `aim` is taken from the input that carried the shot press.
- Measured with bitcode on a struct equivalent to `InputBundleSerialOut`: 61 B for 8 players, 106 B for 16 players. At 14.29 bundles/s that is about 0.87 KB/s and 1.51 KB/s per client, plus 2 B WebSocket header per frame.
- Original for comparison: 3.68 MB/s per client at 8 players, 7.3 MB/s at 16.

## Checksums and resync
- `state_checksum::get_state_checksum(&GameState) -> u64` (`protocol/state_checksum.rs`): FNV-1a 64 over `protocol::encode_game_state(&GameStateSerialOut::from(&state))`, so it covers exactly the wire encoding of the state.
- Server sends `StateChecksum` for the state after tick t when `t % CHECKSUM_INTERVAL_TICKS == 0`; `CHECKSUM_INTERVAL_TICKS = 14` (about 1 s), right after bundle t.
- Client triggers a resync on a checksum mismatch after applying bundle t, on a `StepError`, on a game frame that fails to decode or convert (treated like a decode error; outside a game such a frame is logged and dropped), or on a backlog: applied tick behind the expected tick (snapshot tick plus wall time since the snapshot arrived, in ticks) by more than `BACKLOG_LIMIT_TICKS = 43` (about 3 s).
- Resync: send `RequestSnapshot`, discard bundles until the `Snapshot` arrives, then apply later bundles normally. Message box shows "Catching up" meanwhile.
- Server never rejects `RequestSnapshot`: the game task records `snapshot_pending` for the member (repeats coalesce) and sends the snapshot at the first tick boundary at least `SNAPSHOT_MINIMUM_INTERVAL_TICKS = 29` (about 2 s) after the last snapshot sent to that member, the join snapshot included. Each resync is logged (`resync requested; [game_id=.. member_id=.. tick=..]`).
- Client re-sends `RequestSnapshot` if none arrives within `SNAPSHOT_RESPONSE_TIMEOUT_MILLISECONDS = 5000`.
- More than `RESYNC_LIMIT = 5` resyncs within 60 s: the client leaves the game and returns to the title with the notice "Lost synchronisation with the game".

## Snapshot
- `GameSnapshotSerialOut` carries `GameStateSerialOut::from(&state)` at tick T; the next frames are bundles T+1, T+2, ... on the same ordered membership queue.
- The client converts it with `GameState::try_from` and so restores the exact simulation, including RNG; a failed conversion is handled like a decode error ([checksums and resync](#checksums-and-resync)).
- Wire form (`protocol/game_state_serial.rs`, `member_serial.rs`, `organism_serial.rs`), field for field the [data model](./simulation.md#data-model) with newtypes as primitives:

```
GameStateSerialOut { tick: u32, settings: GameSettingsSerialOut, rng: Pcg32SerialOut, world: WorldSerialOut,
                     round: Option<RoundStateSerialOut>, members: Vec<MemberSerialOut>, next_member_id: u32 }
  members in ascending member_id, in place of the BTreeMap
Pcg32SerialOut { state: u64, increment: u64 }
GameSettingsSerialOut { title: String, mode: GameModeKindSerial, world_shape: WorldShapeKindSerial, world_width_pixels: u32, world_height_pixels: u32,
                        player_minimum: Option<u8>, player_cap: u8, team_count: Option<u8>, leaderboard_length: u8 }
WorldSerialOut { shape: WorldShapeKindSerial, bounds: WorldBoundsSerialOut, initial_bounds: WorldBoundsSerialOut }
WorldBoundsSerialOut { left: i32, top: i32, width: i32, height: i32 }
RoundStateSerialOut { phase: RoundPhaseSerialOut, phase_started_at: u32 }
MemberSerialOut { member_id: u32, screen_name: String, role: MemberRoleKindSerialOut, loadout: Option<LoadoutSerial>, team: Option<TeamKindSerial>,
                  score: ScoreSerialOut, organism: Option<OrganismSerialOut> }
LoadoutSerial { appearance: AppearanceSerial, first: FirstAbilityKindSerial, second: SecondAbilityKindSerial, third: ThirdAbilityKindSerial }
AppearanceSerial { color: OrganismColorKindSerial, skin: SkinKindSerial }
OrganismSerialOut { anchor: WorldPointSerialOut, cells: CellOccupancySerialOut, cursor: WorldPointSerialOut, last_hitter: Option<u32>,
                    abilities: OrganismAbilitiesSerialOut }
CellOccupancySerialOut { origin: LatticeCoordinateSerialOut, width: u16, height: u16, row_bitmap_bytes: Vec<u8> }
OrganismAbilitiesSerialOut { first, second, third: AbilityPhaseSerialOut, third_center: Option<WorldPointSerialOut>, spore: SporePhaseSerialOut,
                             shots: [ShotPhaseSerialOut; 2], compressed_until: Option<u32>, frozen_until: Option<u32> }
AbilityPhaseSerialOut, SporePhaseSerialOut, ShotPhaseSerialOut, ProjectileSerialOut: variants and fields of simulation.md#data-model, ticks as u32, points and vectors as their geometry wire types
```

- Cells use `CellOccupancySerialOut` (tight bounding-box bitmap).
- Size: the original's growth rules, measured with a throwaway script (single organism, 3000 ticks), give 7 to 25 B of bitmap plus 6 B of box header (13 to 31 B per organism), against 82 B measured for a 69-cell coordinate list.
- Estimate: about 1 KB for 16 players and about 2 KB at the 32-player cap, with no projectiles. Size test: at most 32 KB at the player cap with every organism's spores in flight (about 25 KB estimated from 45 exposed cells per organism at 16 B per projectile).
- Sent on: join and spectate (`JoinAccepted` then `Snapshot` of the state before the member's `Joined`), create (the empty tick-0 state, [lifecycle](./server.md#lifecycle)), resync ([checksums and resync](#checksums-and-resync)).

## Lobby
- `SubscribeGameList` (`Lobby` phase only; `AlreadyInGame` otherwise) adds the connection's lobby sender to `LOBBY_SUBSCRIBERS` and sends the current list immediately.
- `CreateGame`, `JoinGame` and `SpectateGame` remove the subscription when accepted; the client re-subscribes on returning to the browser.
- The list revision increments when any summary or the online client count changes; the lobby task sends `GameList` at most once per second, only when the revision changed.
- Summaries only; never cells or member lists.
- Games are listed from creation until removal, including during the empty-game grace period.

## HTTP endpoints
- None for gameplay; the game list goes over the WebSocket.
- Static site files and `GET /health` (plain `ok`, 200) only.
- Rationale: one codec and one connection; a later account service ([adding an account service later](./server.md#adding-an-account-service-later)) may add JSON HTTP endpoints of its own.

## Limits
- Module: `protocol/protocol_limits.rs`, shared by client validation and server.

| Limit                                      | Value                                  |
| ------------------------------------------ | -------------------------------------- |
| Inbound frame                              | 4096 B                                 |
| Screen name                                | 1 to 64 chars, trimmed                 |
| Title                                      | 1 to 64 chars, trimmed                 |
| Password                                   | 1 to 72 bytes (bcrypt)                 |
| World size                                 | 300 to 100000 px                       |
| Player minimum (srv)                       | 2 to player cap                        |
| Player cap (alive organisms)               | 2 to 32                                |
| Team count (skm)                           | 2 to 4, at most cap                    |
| Leaderboard length                         | 1 to 20                                |
| Members per game                           | player cap + 64                        |
| Pure Spectators per game                   | 64                                     |
| Games per server                           | 256                                    |
| Connections per server                     | 4096                                   |
| Connections per IP                         | 16                                     |
| Failed password attempts per IP            | 20 per 10 min                          |
| Cursor coordinate                          | ±2^28 subpixels (262144 px)            |

- `player_cap` limits alive organisms (faithful: the original's player list held only alive players). The member limit bounds Participants of any status plus Spectators, and so bundle size, snapshot size and per-tick cost; admission checks both against state plus pending admissions ([lifecycle](./server.md#lifecycle)).
- Names and titles: no control characters. Screen names unique among the game's members and pending joins (exact match after trimming); titles unique among live games.
- Hidden fields: ffa and skm have no player minimum; only skm has a team count; world 800, srv minimum 4, cap 16, skm team count 2 and leaderboard 10 are the form defaults. Cross-field rules apply only to fields the mode has (`PlayerCapBelowMinimum` srv only, `PlayerCapBelowTeamCount` skm only), so an ffa or skm game with cap 2 or 3 is valid.

## Wire types
- Module: `shared/src/protocol/`.
- Owner decision: every type that crosses the WebSocket or is written to a replay file has its own dedicated wire type; only wire types derive `bitcode::Encode`/`Decode`. The wire types mark the shapes that cannot change without a `PROTOCOL_VERSION` bump.
- Not wire types, no bitcode derive: every simulation type (`GameState`, `GameSettings`, `World`, `RoundState`, `Member`, `Organism`, `CellOccupancy`, `OrganismAbilities`, ability phases, `Projectile`, `Loadout`, `Appearance`, every `...Kind`, `Pcg32`, `InputBundle`, `PlayerTickInput`, `MemberEvent`, `Tick`, `MemberId`, `GameId`, geometry types), the request models (`Joiner`, `TeamChoiceKind`, `PlayerInput`, `RequestKind`, `RejectionKind`), the lobby model `GameSummary`, the replay models (`ReplayHeader`, `ReplayLog`).
- Naming per `docs/conventions/types.md`:
  - `<Model>SerialOut`: server to client.
  - `<Model>SerialIn`: client to server.
  - `<Model>Serial`: both directions byte-identical (`LoadoutSerial`, `AppearanceSerial`, `TeamKindSerial`, `GameModeKindSerial`, `WorldShapeKindSerial`, the ability, colour and skin kind mirrors, `SubpixelPointSerial`, `AimVectorSerial`).
  - Replay files are server output: their records use the server-to-client wire types, the header included (`ReplayHeaderSerialOut`).
  - Wire-only types without a model, so without conversions: `MessageSerialIn`, `MessageSerialOut`, `GameSnapshotSerialOut`.
- Newtypes and bit sets are primitives on the wire: `Tick`, `MemberId`, `GameId` as `u32`; `Subpixels` as `i32`; `AbilityPressSet` as `u8`.
- Conversions, immediately after the wire type in the same file:
  - `impl From<&Model> for <Model>SerialOut` (or `<Model>Serial`): sending side, infallible.
  - `impl TryFrom<<Model>SerialIn> for Model`; and `TryFrom<<Model>SerialOut>` or `TryFrom<<Model>Serial>` for the receiving side (client decoding, replay reading). `From` only for plain enum mirrors, where nothing can fail.
  - Error: `RejectionKind` for request payloads (`GameSettingsSerialIn`, `JoinerSerialIn`, `TeamChoiceKindSerialIn`), answered as `RequestRejected`; `AppError` for every other conversion, handled like a decode error (server: `ProtocolViolation`, [failure isolation](./server.md#failure-isolation); client: resync, [checksums and resync](#checksums-and-resync)).
- Receiving: `bitcode::decode` into the wire type, then `TryFrom` into the model. Sending: `From` into the wire type, then `bitcode::encode`. Only `protocol/` names `bitcode` ([determinism tests](./testing.md#determinism-tests)).
- `TryFrom` checks what decoding cannot:
  - `CellOccupancySerialOut`: bitmap length `ceil(width / 8) * height`, tight bounding box (through `CellOccupancy::from_tight_bitmap`, below);
  - `GameStateSerialOut`: members strictly ascending by `member_id`, every `member_id` below `next_member_id`, loadout exactly for Participants, team exactly in skm, round exactly in srv, organisms only for Participants;
  - `Pcg32SerialOut`: odd increment (through `Pcg32::from_parts`, below);
  - press bits: no unknown bits; inbound `aim` only with a shot-slot press;
  - collection lengths and coordinates within `protocol_limits.rs`.
- A model whose invariants sit in private fields owns them in its own module: raw-part accessors for `From`, and a validating constructor returning `Option` which the `TryFrom` in `protocol/` calls (`None` becomes `AppError`):
  - `Pcg32::from_parts(state: u64, increment: u64) -> Option<Pcg32>`, `Pcg32::state()`, `Pcg32::increment()`;
  - `CellOccupancy::from_tight_bitmap(origin: LatticeCoordinate, width: u16, height: u16, row_bitmap_bytes: Vec<u8>) -> Option<CellOccupancy>`, plus accessors for the four parts.
- Replay files ([replay tests](./testing.md#replay-tests)):
  - `PROTOCOL_VERSION` as a little-endian `u16` first, outside bitcode (as [transport, codec, version](#transport-codec-version));
  - then length-prefixed records: one `ReplayHeaderSerialOut { settings: GameSettingsSerialOut, seed: u64 }`, then one `InputBundleSerialOut` per tick, encoded with `protocol::encode_replay_header` and `protocol::encode_replay_bundle`;
  - read back through `TryFrom` into `ReplayHeader { settings: GameSettings, seed: u64 }` and `InputBundle`;
  - a version other than `PROTOCOL_VERSION` is rejected before any decode with `ReplayReadError::ProtocolVersionMismatch { file_version: u16, current_version: u16 }`.
