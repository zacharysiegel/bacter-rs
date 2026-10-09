# Web client

## Shell
- One route `/`; the screen is Leptos state, not a URL (prerender guard allows only `/`).
- `Screen { Title, Browser, CreateMenu, JoinMenu { origin: JoinMenuOrigin }, SpectateMenu { game_id }, Playing, Spectating, RespawnMenu, PauseGame, PauseSpectate, Tutorial, PauseTutorial }`.
- `JoinMenuOrigin { Create { settings: GameSettingsSerialIn, password: Option<String> }, Browser { game_id: GameId } }`.
- `app.rs`: `shell()` (ssr) renders head with `HashedStylesheet`, `AutoReload`, `HydrationScripts`; body holds `<App/>`: canvas layer (z -2), shade (white 0.5, z -1) behind menus, UI (z 1).
- `client/hydrate.rs`: panic hook, `console_log` at Debug, `leptos::mount::hydrate_body(App)`.
- Escape: create/browser to title; join menu to browser (or back to create when creating); spectate menu to browser; playing to pause; spectating to pause; tutorial to pause; pause/respawn menus back to play.
- Creating: create menu then join menu (screen name, loadout); its submit sends one `CreateGame`; Back from that join step returns to the create menu with nothing created.
- `alert()` replaced by the message box or inline form errors.
- Connection (`client/game_socket.rs`):
  - One socket at a time; opened after hydration, when `game-server.json` has been read, and again only by the Reconnect button; no automatic reconnect.
  - Title and tutorial never need it: the title scene and tutorial run locally ([tutorial and title](#tutorial-and-title)).
  - Title notice line: "Connecting to the game server" until open; after a close, the client notice of its `CloseReasonKind` ([server to client](./protocol.md#server-to-client)) plus a Reconnect button (accepted by the owner; none for `ProtocolVersionMismatch`, whose notice asks for a reload). Host and Join are disabled while not connected.
  - Close or error on any screen other than Title or Tutorial: `ConnectionLost` returns to Title with that notice; the tutorial continues undisturbed.
- `LeftGame` returns to Browser; `GameEnded` returns to Browser with the notice "The game has closed"; `SynchronisationLost` returns to Title with "Lost synchronisation with the game".

## Screens
- Faithful to the original layout.
- Title: 170×150 box, rgb(10,10,10), 1 px white border; Host, Join, Tutorial in `_bacter` 29 px; title scene animates behind title, browser, create, join and spectate menus.
- Browser: full-width table (Title, Mode, Players, Spectators, Player Cap, Join/Spectate buttons); footer "← Back" and "Online Clients: N"; renders from `GameList`.
- Menu form engine (`menu/menu_form.rs`): header, two-column table, submit button, Back footer, red per-field errors plus a bottom row for general errors, Enter submits; field kinds Text, Number (floored, 0 shown empty, blank takes placeholder), List, Radio group (deselectable, fourth ability fixed), Button.
- Create: Title, Password, World Shape (Square/Circle), World Width and Height (linked), Player Minimum, Player Cap, Team Count, Leaderboard Length, Game Mode (ffa, skm, srv).
  - Visibility: ffa hides minimum and team count; skm hides minimum and leaderboard length; srv hides team count.
  - Validation through the shared `TryFrom<GameSettingsSerialIn> for GameSettings` (in `protocol/message_in.rs`, after the type), resolving defaults first.
- Join: Screen Name, Password (secured only), Color with swatches (hidden in skm), Skin (3 radios; none selected means None), 1st/2nd/3rd ability (required), 4th Spore fixed, Team list "Red: n" preselecting the smallest team (skm), Auto Assign.
- Spectate: Screen Name, Password.
- Respawn: Color, Skin, abilities, Team, Auto Assign prefilled; Leave Game button; opened with R when dead, spawn allowed and alive organisms < cap, otherwise the message "Game is at maximum player capacity".
- Pause game: Color (not skm), Skin, Name Labels, Messages, Leave Game; Apply sends `UpdateAppearance` and stores settings.
- Pause spectate: Name Labels, Messages, Leave Game. Pause tutorial: Leave Tutorial.

## Renderer
- Module: `shared/src/render`, feature `render`.
- `GameRenderer` owns instance, adapter, device, queue, surface, pipelines; `!Send`.
- Surface first, then `request_adapter` with `compatible_surface: Some(&surface)` (required for WebGL2; eafora's order breaks its fallback).
- `Instance::default()` (WebGPU, else WebGL2); `?renderer=webgl2` forces `Backends::GL`; `Limits::downlevel_webgl2_defaults()`, no features, Fifo present mode.
- `surface.rs`: shared `WgpuSurface` at file level; `#[cfg(target_arch = "wasm32")] mod wasm` (`from_canvas`); `#[cfg(not(target_arch = "wasm32"))] mod native` (`from_window_handle`, for iOS later).
- Pipelines:
  - `cell_pipeline.rs` + `cell.wgsl`: one instanced draw for all cells, spores (0.8 scale) and shots; quad corners from `vertex_index`; per-instance vertex buffer (`VertexStepMode::Instance`): `center: [f32; 2]`, `color: unorm8x4`, `params: u32` (skin, scale); 16 B stride.
    - `cell.wgsl` decodes `params` and `color` only through accessor functions defined once, beside the matching Rust instance struct's field order (`fn instance_skin_kind(params: u32) -> u32`, `fn instance_scale(params: u32) -> f32`, `fn instance_color(color: vec4<f32>) -> vec4<f32>`); no call site masks, shifts or indexes channels.
  - `shape_pipeline.rs` + `shape.wgsl`: instanced rectangles and ellipses, filled or stroked: world backdrop, shadow, body, border, toxin and neutralize rings and fills, secretions, crosshair bars.
- Skins in the fragment shader: Grid (fill plus rgb(40,40,40) 0.25 px stroke), Circles (disc radius 3), Ghost (1 px outline, no fill), None (fill plus 1 px stroke in own colour).
- Camera: `world_to_view` centred on the local crosshair, `view_to_clip` orthographic with device pixel ratio; uniform `world_to_clip` (names per `shading.md`).
- Draw order: backdrop (70,70,70), shadow (backdrop minus 20, offset (+7,+6) square or (+5,+4) circle), world body (black), border (210,210,210) 1 px, toxin rings (255,111,92) weight 3, secretions (owner colour), neutralize rings (0,179,12) weight 3 for every player, organisms, spores and shots, crosshair (white plus, ±4 px).
- Cell instance buffer rebuilt only when a tick is applied; per frame only the camera uniform and interpolated projectile instances change.
- `render/scene.rs`: `RenderScene::from_state(&GameState, &InterpolationFrame, camera)`; GPU-free, unit-testable.
- WebGL2 constraints observed: no storage buffers, stride at most 255, uniform blocks at most 16 KiB.

## Client session, frame pacing and driving

Platform-independent part (`shared/src/play/`):
- `PlayerControls { keyboard: KeyboardState, crosshair: CrosshairController }`: owned by the platform driver and lent `&mut` to every session and scene call.
- `ClientSession` (one per WebSocket connection, sans-IO):
  - `ClientSession::new() -> ClientSession`.
  - `fn receive(&mut self, frame: &[u8], controls: &mut PlayerControls, now_milliseconds: f64) -> Vec<ClientEffect>`: decodes the wire type and converts it with `TryFrom` (a decode or conversion failure starts a resync in a game, [checksums and resync](./protocol.md#checksums-and-resync)); applies bundles (steps the simulation); checks checksums and backlog ([checksums and resync](./protocol.md#checksums-and-resync)); applies `CursorCorrected`; after each applied bundle samples `controls` and returns the encoded `Input` ([client to server](./protocol.md#client-to-server)).
  - `fn send_request(&mut self, request: MessageSerialIn) -> Vec<ClientEffect>`: encodes a menu request.
  - `fn connection_closed(&mut self, close_code: u16) -> Vec<ClientEffect>`.
  - `fn render_state(&self) -> Option<&GameState>`; `fn own_member_state(&self) -> Option<OwnMemberStateKind>`.
- `ClientEffect`:
  - `SendFrame(Vec<u8>)`.
  - `GameListReceived { game_summaries: Vec<GameSummary>, online_client_count: u32 }`.
  - `JoinAccepted { game_id, member_id }`, `RequestRejected { request: RequestKind, reason: RejectionKind }`.
  - `OwnMemberStateChanged(OwnMemberStateKind)`, `OwnOrganismSpawned { cursor: WorldPoint }`, `OwnSpawnRejected`.
  - `TickApplied { tick }`, `ResyncStarted`, `ResyncFinished`.
  - `LeftGame`, `GameEnded`, `SynchronisationLost`, `ConnectionLost(CloseReasonKind)`.
- `ActiveScene { Online(ClientSession), Tutorial(Tutorial), Title(TitleScene) }` (`play/active_scene.rs`): `fn advance_frame(&mut self, now_milliseconds: f64, controls: &mut PlayerControls, input_focus: InputFocusKind) -> FrameOutput`; `InputFocusKind { Game, Menu }`.
- `FramePacer` (`play/frame_pacing.rs`), used by every scene:
  - `dt = min(now - last, 250 ms)`.
  - Crosshair: `CrosshairController::advance(&keyboard, dt, speed)` at the own member state's speed (below); diagonals scaled by cos 45°; opposite keys cancel; no movement with `InputFocusKind::Menu`.
  - Tutorial and title: local bundles from a 70 ms accumulator, at most `LOCAL_TICKS_PER_FRAME_LIMIT = 4` per frame (the remainder is dropped, for example after a hidden tab).
  - Interpolation alpha = time since the last applied bundle / 70 ms, clamped to [0, 1]; projectiles drawn between their previous and current tick positions.
  - `FrameOutput { interpolation_alpha: f32, tick_applied: Option<Tick>, effects: Vec<ClientEffect> }`.
- `CrosshairController` with `SentCursorHistory` (ring buffer of the last 64 sent cursors keyed by `client_tick`):
  - `CursorCorrected { client_tick: t, cursor }`: `shift = cursor - sent[t]`; the crosshair moves by `shift`, and every entry `sent[t']` with `t' > t` moves by the same `shift`, so the corrections the server sends for the same overshoot in later ticks come out as zero.
  - A correction for a tick no longer in the history snaps the crosshair to `cursor`.
- Own spawn (`OrganismSpawned` for the own member): crosshair snaps to the spawn cursor.
- `OwnMemberStateKind` (`play/own_member_state.rs`):
  - `AliveParticipant`:
    - Screen `Playing`; crosshair 42.5 px/s; HUD shown.
    - Escape opens `PauseGame`; R does nothing.
  - `DeadParticipant` and `Spectator` (faithful: the original moved dead players to its spectators):
    - Screen `Spectating`; camera 62.5 px/s; HUD hidden.
    - Escape opens `PauseSpectate`.
    - R opens `RespawnMenu` when spawning is allowed (no rounds, or `Waiting` or `PreRound`) and alive organisms are below the cap; at the cap the message box shows "Game is at maximum player capacity" for 3 s; otherwise R does nothing.
    - `RespawnMenu` prefilled from the member's loadout (Participant) or the join defaults (Spectator; respawning converts it, faithful).
  - `OwnSpawnRejected` (no free position): message box "No room to spawn; press 'R' to try again" (accepted by the owner) for 3 s; state unchanged.
  - Message texts per state: [overlays](#overlays).

Browser part (`web/src/canvas/driver.rs`, hydrate):
- `thread_local! { static DRIVER: RefCell<Option<Driver>> }`; `Driver` owns `GameRenderer`, `ActiveScene`, `PlayerControls`, the `GameSocket` and the cached rAF `Closure`.
- WebSocket `onmessage`: `session.receive(bytes, &mut controls, performance.now())`; `SendFrame` goes to the socket, every other effect to Leptos signals. Stepping happens here so a hidden tab keeps simulating.
- `keydown`/`keyup`: `keyboard.key_changed(GameKeyKind, KeyTransitionKind, repeat)` (`canvas/keyboard.rs`).
- rAF callback (continuous while the canvas is mounted): `advance_frame`, render with the alpha, update overlay signals when a tick was applied, reschedule.
- Other players' cursors are not drawn (faithful).

## Overlays
- HTML, Leptos.
- HUD ability bar (`hud/ability_bar.rs`), players only: 4 slots at `center.x - 150 + i * 100`, `y = 0.9 * height`; black disc r 30, grey 215 disc r 29; arc draining while Active, refilling while Cooling (proportion from ticks); icon from `ability_icon.rs` rendered as SVG pixels; key tab (X, C, V, `_`); shot or secretion timer disc r 8 at x - 41; active dots:
  - green (66,244,176) while own Extended, Immortal or Neutralize field active ([data model](./simulation.md#data-model));
  - red (255,141,135) while own `compressed_until` or `frozen_until` is set (an enemy's effect on this player), or own Toxin field active;
  - never for a Compress or Freeze caster's own timer (faithful, `HUD.js:236-245`).
- Name labels (`hud/name_labels.rs`): when enabled, under each organism centroid at `sqrt(36 * count / π) + 20` px; Helvetica 10 in the border colour; names over 30 chars shown as first 20 plus "..." (faithful), centred on the shown text. Positions computed in Rust from the scene camera.
- Leaderboard (`leaderboard/leaderboard.rs`): top right (15 from right, 13 from top), rows 22 px, name column 170, others 46, Helvetica 11, own row or team in Verdana bold, CSS shadow (+4,+3).
- Message box (`message/message_box.rs`): top left, text at (25,30), Helvetica 14 white, shadow (+5,+4); texts from `message_text::get_message_text(&GameState, own_member_id)`, N from ticks ([world, spawn, rounds](./simulation.md#world-spawn-rounds)):
  - srv Waiting, any own state: "Waiting for 1 more player to join" when one is missing, else "Waiting for N more players to join" (N = `player_minimum` minus participants).
  - srv PreRound, any own state: "Round begins in: N".
  - srv Playing: alive, none; without an organism, "Wait for the round to complete".
  - srv PostRound, any own state: "Round ends in: N".
  - No rounds: alive, none; without an organism, "Press 'R' to Spawn".
  - Transient texts ([client session, frame pacing and driving](#client-session-frame-pacing-and-driving)) replace these for 3 s.
  - Tutorial task texts verbatim from `Message.js`.
  - Box width from measured text width (DOM), not character count.
- Font: `@font-face` `_bacter` from `/fonts/bacter.ttf`; Verdana, Georgia, Helvetica, Consolas elsewhere as the original.

## Prediction seam
- The client simulates exactly the server's bundles; only the crosshair is predicted.
- Seam for later rollback: `ClientSession` keeps `confirmed_state` separate from what the renderer reads (`render_state()`), so a predicted copy can be inserted without touching the renderer or protocol.

## Tutorial and title
- Modules: `shared/src/tutorial`, `shared/src/title`.
- Both build local `GameState`s and call the same `step` with local bundles from the `FramePacer` accumulator ([client session, frame pacing and driving](#client-session-frame-pacing-and-driving)); `u64` seed passed in by the platform (`js_sys::Math::random` on the web), through `GameState::new` ([determinism rules](./determinism.md#determinism-rules)).
- Local `GameSettings` built directly: mode `FreeForAll`, rectangle, width and height from the window ([data model](./simulation.md#data-model)), `player_cap` 2 (tutorial) or 10 (title).
- Local membership: `Joined` events in bundle 1; organisms placed with `spawn::place_organism` between steps, never through `SpawnRequested` (fixed positions, no spawn search).
- Title: 10 dummy organisms, random palette colours and skins (including None), at most `floor(10/4) + 1 = 3` per quadrant, fixed cursors, world inset 25 px; resize rescales cursors and resets organisms to one cell.
- Tutorial: world = window minus 25 px margins, camera fixed; player placed at the centre (random palette colour except Sun and Sky, skin None).
- Tasks: move, fullscreen (skipped if fullscreen; F11 or 3.5 s), survive (4.5 s, enables Extend), extend, immortality, neutralize, shoot (enables Compress and Freeze, 10 s), compress (bot placed), freeze, toxin, spore (pauses stepping at half the flight until secrete), done.
- One ability enabled at a time, as the original's per-task `activated` toggles:
  - `Tutorial` keeps `enabled_abilities: AbilityPressSet` per task and masks the sampled presses with it before building each local bundle, so a disabled ability never activates.
  - Loadout changes between tasks (for example Compress and Freeze from the shoot task on, Toxin for the toxin task) are written to the player's `Member.loadout` directly between steps; the local game has no server to agree with.
- Bot: placed at a uniform random point inside the world at least `TUTORIAL_BOT_MINIMUM_DISTANCE_PIXELS = 80` (default range plus 30, faithful) from the player's cursor; random palette colour except Sun and Lime, skin None.
- Every tutorial organism (player and bot) that reaches zero cells is placed again as one cell at its own cursor before the next step (the original's per-organism intent; self acid, spore removal and the survive task rely on it).
- Fixes: bot placed through `place_organism` with explicit arguments; task timers are tick deadlines owned by `Tutorial`, dropped on leave; HUD hidden during move and survive.

## Settings
- `shared::settings` copied from eafora: `SettingKey`, `SettingValue`, `SettingsStore`, `Setting`.
- Keys: `NameLabels` (default true), `Messages` (default true).
- `web/src/client/local_storage_settings.rs`: `LocalStorageSettingsStore`.

## Game server URL
- `web/static/game-server.json`, committed: `{"websocket_url": "ws://127.0.0.1:3100/ws"}`.
- `build-site.sh` rewrites it with `jq` to `{"websocket_url": null}`; null means same origin (`wss:` on https, `ws:` on http, `location.host`, `/ws`).
