# Phase 8: web client implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** the browser client in `web`: the Leptos shell and its prerender, every screen of the original (title, game browser, create, join, spectate, respawn and pause menus), the HTML overlays (ability bar, leaderboard, message box, name labels), the tutorial view, the driver which owns the renderer, the scenes, the keyboard and the socket, localStorage settings, the styles and the font, and the scripts which build and serve the release site; done when the prerender test passes, `cargo leptos watch` serves a playable game against `./scripts/dev/server.sh`, and two browsers play together.

**Architecture:** `docs/architecture/client-web.md`. Pure, host-tested modules decide what the views show (`screen/`, `connection/`, the menu values and requests, the HUD, leaderboard and label views, the per-scene camera and overlays); Leptos components render them from `AppSignals`, a `Copy` bundle of signals shared through context. The hydrate-only driver (`canvas/driver.rs`) lives in a `thread_local`, outside the reactive graph because wgpu resources are `!Send`: it advances the `ActiveScene` every animation frame, steps the online session on every socket frame, draws with `GameRenderer`, and writes the overlay signals; views command it through `DriverCommandKind`. The ssr build renders the document for the dev server and the release prerender.

**Tech Stack:** Rust 1.95 (edition 2024); Leptos 0.8, leptos_meta, leptos_axum and axum (ssr), wasm-bindgen, wasm-bindgen-futures, js-sys, web-sys, console_log and console_error_panic_hook (hydrate), cargo-leptos 0.3 with its Sass build, `jq` and `lsof` in the scripts; all already declared by `web/Cargo.toml`, no new dependencies.

---

## Ground rules for the executor

- Repository `/Users/singularity/bacter-rs`, branch `web-client`. Run `git -C /Users/singularity/bacter-rs branch --show-current` first; stop if it does not print `web-client`.
- Every command runs from the repository root `/Users/singularity/bacter-rs`.
- Never switch branches, push, rebase or amend. Stage by explicit path, run `git status` before each commit, one-line commit messages without attribution. `?? .idea/` (the owner's IDE folder) stays untracked; it is left out of the expected `git status` listings below.
- Never run `cargo clippy`.
- The code below is `rustfmt`-formatted with the repository's `rustfmt.toml`; transcribe it exactly, so `cargo fmt --all -- --check` stays silent.
- Edits take these forms:
  - "Create `path`" with the whole new file;
  - "Replace the whole contents of `path`";
  - "In `path`, replace" a text which occurs exactly once in the file, "with" its replacement;
  - "In `path`, delete" a text which occurs exactly once in the file;
  - "In `path`, replace everything above `#[cfg(test)]`" (the file's one test module, at its end) with the given code, followed by one blank line;
  - "append inside `mod tests`, directly above its closing brace", which is the last line of the file, after one blank line separating it from the item before;
  - "Append to the end of `path`, after one blank line".
- A new file's failing test is the file holding only its test module (together with its `pub mod` line); the implementation step then replaces everything above `#[cfg(test)]`, which is nothing yet, with the code.
- `lib.rs`, every `mod.rs` and `web/style/main.scss` are given whole each time they change.
- Builds stay warning-free, tests included; a warning is a defect to fix before committing.
- Expected outputs were observed by executing this plan end to end in a scratch copy of this branch, and then playing the result in Chrome. The `finished in` timing of each `test result` line and the build timings are left out. A compile failure is quoted by its first `error` line; later lines differ only in the names they report. A filtered test run lists only the tests of its filter; its `filtered out` count depends on the tasks done before it.
- Sandboxes: commands which bind a loopback port (the server's integration tests, `./scripts/dev/server.sh`, `./scripts/dev/web.sh`, the release server, `wasm-pack test`) fail with `Operation not permitted (os error 1)` inside a command sandbox; run them outside it.
- UI views and styles are exempt from test-first (`docs/conventions/testing.md`); their tasks end in a build check, and Tasks 27 to 29 check them in the browser.
- Browser checks: run `./scripts/dev/server.sh` and `./scripts/dev/web.sh` as background processes (the second serves `http://127.0.0.1:3011` once its first build finishes, about a minute), drive Chrome through the chrome-devtools MCP tools (load them with ToolSearch, for example `select:mcp__plugin_chrome-devtools-mcp_chrome-devtools__new_page,mcp__plugin_chrome-devtools-mcp_chrome-devtools__take_screenshot,mcp__plugin_chrome-devtools-mcp_chrome-devtools__evaluate_script,mcp__plugin_chrome-devtools-mcp_chrome-devtools__press_key,mcp__plugin_chrome-devtools-mcp_chrome-devtools__list_console_messages`), use a second page in its own isolated context for the second player, and stop every process you started by its PID before finishing. A screenshot reaches the page several seconds after it is requested, so an effect lasting a few seconds is best captured from the canvas itself, in the same script which triggers it (Task 27). If the MCP tools are unavailable, say so in the report rather than claiming the browser checks passed.
- The menus' radio buttons are `div`s the accessibility snapshot does not list; click them from `evaluate_script` (`document.querySelectorAll('.menu-radio')[index].click()`), counting them in row order (Skin 0 to 2, 1st ability 3 and 4, 2nd 5 and 6, 3rd 7 and 8, 4th 9, then Auto Assign).

## Decisions taken where the design is silent or contradictory

- The browser/server pair of a component is `#[cfg(feature = "hydrate")] mod browser` / `#[cfg(not(feature = "hydrate"))] mod server`, the form `conditional-compilation.md` shows: with `#[cfg(feature = "ssr")]` the featureless `cargo build --workspace` and `cargo test --workspace` would not compile. Host unit tests run featureless (`cargo test -p web --lib`); the prerender test runs with `--features ssr`.
- No router: the app has one route. The dev server routes only `/` to the shell (other paths are the site root's files, else the shell with 404), and the prerender renders `/`; with no route table there is nothing else to guard.
- `canvas/keyboard.rs` is not hydrate-gated: it maps `KeyboardEvent.code` strings (layout-independent, the original's key positions) and pointer offsets, so it is tested on the host; the driver reads the events.
- No `message/text_measurement.rs`: the message box is an absolutely positioned element which shrinks to its text, so its width is the DOM's measure of the text without a measuring call.
- New modules the design does not list: `connection/` (the notice line and the socket URL, pure), `canvas/scene_camera.rs` and `canvas/scene_overlay.rs` (what each scene draws and shows, pure), `menu/loadout_fields.rs` (rows the join, respawn and pause menus share), `menu/game_menu_view.rs`, and the web crate's own `test_fixture/` (`shared`'s is `#[cfg(test)]` in that crate).
- `shared` changes: `protocol::PROTOCOL_VERSION_QUERY_PARAMETER` and `protocol::get_versioned_websocket_url`, which the server's parser and test helper also use; `Setting::default_value` (the settings signal starts from the declared defaults, since the server render has no localStorage); `tutorial::TUTORIAL_PLAYER_ID` made public for the tutorial HUD.
- `AppSignals` holds every signal the driver writes and the views read; a menu reacts to `request_rejection` only for its own `RequestKind`, and `AppSignals::navigate` clears it, so a rejection never outlives its screen.
- Session ownership (left to this phase by phase 6): the connection's `ClientSession` waits beside the title scene and the tutorial and moves into `ActiveScene::Online` on `JoinAccepted`; `LeftGame`, `GameEnded` and `SynchronisationLost` move it back and start a new title scene.
- The browser sends `SubscribeGameList` each time it opens while connected, since an accepted create, join or spectate ends the server's subscription; the client never sends `UnsubscribeGameList`.
- Lobby notices: `GameEnded`'s "The game has closed" shows under the browser's table, `SynchronisationLost`'s on the title notice line, where the connection's notice takes its place while there is one; an accepted join clears them.
- Screen transitions on the own member state: an alive member leaves a join, spectate or respawn menu or the spectating screen for `Playing`, and an open `PauseSpectate` becomes `PauseGame`; a dead member or a Spectator leaves a join or spectate menu or `Playing` for `Spectating`, and an open `PauseGame` becomes `PauseSpectate`; any other screen stays. A lost connection returns to the title unless the tutorial is open.
- Keys: game keys reach `KeyboardState` only while the input focus is `Game`; a change to `Menu` releases every held key and drops unsent presses. Escape goes back from every screen (the pause and respawn menus back to the screen of the own state). R acts only on `Spectating`. Movement and ability keys prevent the browser's default (scrolling, pressing a focused button); F11 stays the browser's.
- Shot aim: the pointer's offset in CSS pixels from where the crosshair is drawn, refreshed on `mousemove` and every frame, saturated to `i16`.
- The camera follows the crosshair online and draws it only for a playing member (phase 7 left this to the driver); the tutorial draws it over a still world; the title scene draws none. Nothing is drawn online until the first snapshot, so the last frame stays.
- Overlays (ability bar, leaderboard, message box, name labels) sit at the canvas's depth, under the shade of in-game menus, as the original drew them in its canvas. Name labels are computed every frame (the camera moves), only online and only while the setting is on; the other overlays when a tick is applied, written only when they change.
- Ability dials: a shot's and the spore's secretion drain over the secretion duration alone; the original continued from where the flight stopped, which the tick phases do not record.
- The respawn menu prefills Auto Assign from the team choice last sent in a join or respawn (`AppSignals::team_choice`); the design asks for it prefilled and the member state does not record it.
- The create menu's fields live in context (`CreateMenuFields`), so Back from the join step shows what was entered; Host resets them. A field the mode hides takes its default (the leaderboard length in skirmish) or is left out (the player minimum and team count). Out-of-range numbers saturate into the wire types on their side of the range, so the shared validation names the right bound; it reports the first rejected setting, together with a too-long password.
- Rejections the design attaches to a field the menu does not show (`GameFull` at the player cap on the join menu) fall back to the general row, as the original's issue list did.
- A listed game which leaves the list while its join menu is open shows "The game has closed" in the general row.
- The Password fields are plain text fields, as the original's were.
- The pause game menu's Apply sends `UpdateAppearance` only when the colour or skin changed, then stores the settings and returns to the game.
- localStorage: keys `bacter.` plus the setting's storage key, values the `SettingValue` bytes as hexadecimal (no encoding dependency).
- The driver keeps its rAF closure in a `thread_local` and reschedules first, so a frame it skips (its cell already borrowed) does not stop the loop; DOM listeners live for the page (`Closure::forget`); a closed socket is kept until the next close, since its own close handler is running when it closes; commands dispatched while the driver is absent or busy wait in a queue the next driver entry runs.
- A renderer which cannot be created is logged (`the game cannot be drawn in this browser`); the menus still work.
- `console_log` is initialised at Debug as designed; in a dev build under `?renderer=webgl2` this logs wgpu's and naga's debug lines once while the pipelines compile (release builds cap `log` at info).
- `build-site.sh` reads the dev port from `web/Cargo.toml`'s `site-addr` and refuses while anything listens on it; `verify-site-tree.sh` checks the document, every `/pkg/` reference on disk and content-hashed, and at least one reference; `run-release-server.sh` checks `.env` itself and does not let the environment override it.
- The title notice line sits centred below the title box; the design names the line but not its place.

## File structure

- Modify `shared/src/protocol/protocol.rs`: `PROTOCOL_VERSION_QUERY_PARAMETER`, `get_versioned_websocket_url`.
- Modify `server/src/socket/socket_api.rs`, `server/tests/helpers/test_server.rs`: read and send the shared parameter name.
- Modify `shared/src/settings/settings.rs`: `Setting::default_value`.
- Modify `shared/src/tutorial/tutorial.rs`: `TUTORIAL_PLAYER_ID` public.
- Modify `web/src/lib.rs`: the web modules, the gated `client` and `server`, the test fixture and the wasm32 test configuration.
- Modify `web/src/main.rs`: the ssr binary (`serve` or `prerender`).
- Create `web/src/app.rs`: `shell` (ssr), `App` (layers and screen switch), `AppSignals`, `ClientSettings`, `set_if_changed`.
- Create `web/src/server.rs` (ssr): the dev server, the prerender, the manifest options.
- Create `web/src/client/mod.rs`, `hydrate.rs` (the wasm entry), `local_storage_settings.rs` (`LocalStorageSettingsStore`), `fullscreen.rs` (`get_display_mode`), `game_server_config.rs` (`read_websocket_url`), `game_socket.rs` (`GameSocket`); all hydrate-only.
- Create `web/src/screen/mod.rs`, `screen.rs`: `Screen`, `JoinMenuOrigin`, `ScreenBackdropKind`, `LobbyNoticeKind`, `get_game_screen`.
- Create `web/src/connection/mod.rs`, `connection_status.rs` (`ConnectionStatusKind`, `LobbyAccessKind`, `ReconnectOfferKind`), `websocket_url.rs` (`PageOrigin`, `resolve_websocket_url`).
- Create `web/src/canvas/mod.rs`; `game_canvas.rs` (`GameCanvas`, `DriverCommandKind`, `dispatch_driver_command`); `driver.rs` (hydrate: `Driver`, `start`, `dispatch`); `keyboard.rs` (key mapping, default actions, aim); `scene_camera.rs` (`SceneView`, `get_scene_view`); `scene_overlay.rs` (`SceneOverlay`).
- Create `web/src/menu/mod.rs`; `menu_form.rs` (`MenuForm`, field kinds, issues, `RequestRejection`); `loadout_fields.rs` (`LoadoutFields`, `LoadoutFieldValues`, `TeamSize`, shared rows); `game_menu_view.rs` (`GameMenuView`); `create_menu.rs` (`CreateMenu`, `CreateMenuFields`, defaults, validation); `join_menu.rs` (`JoinMenu`, `JoinTarget`, `build_join_request`); `spectate_menu.rs`; `respawn_menu.rs`; `pause_menu.rs` (`PauseGameMenu`, `PauseSpectateMenu`, `PauseTutorialMenu`).
- Create `web/src/browser/mod.rs`, `game_browser.rs`: `GameBrowser`, `GameListView`.
- Create `web/src/title/mod.rs`, `title_menu.rs`: `TitleMenu` with the notice line.
- Create `web/src/hud/mod.rs`; `ability_bar.rs` (`AbilityBarView`, `AbilityBar`); `ability_icon_view.rs` (`AbilityIconView`); `name_labels.rs` (`get_name_labels`, `NameLabels`).
- Create `web/src/leaderboard/mod.rs`, `leaderboard.rs`: `LeaderboardView`, `Leaderboard`.
- Create `web/src/message/mod.rs`, `message_box.rs`: `MessageBox`.
- Create `web/src/tutorial/mod.rs`, `tutorial_view.rs`: `get_tutorial_overlay`.
- Create `web/src/test_fixture/mod.rs`, `test_fixture.rs` (test only): game states, loadouts, members and client sessions.
- Create `web/tests/prerender.rs` (ssr): the prerendered document.
- Create `web/static/game-server.json`, `robots.txt`, `favicon.ico` and `fonts/bacter.ttf` (the original's files).
- Create `web/style/main.scss`, `_tokens.scss`, `_reset.scss`, `_typography.scss`, `_layout.scss` and `components/_menu.scss`, `_browser.scss`, `_title.scss`, `_ability-bar.scss`, `_name-labels.scss`, `_leaderboard.scss`, `_message.scss`.
- Create `scripts/dev/web.sh`, `scripts/test/test-wasm.sh`, `scripts/build/build-site.sh`, `scripts/build/verify-site-tree.sh`, `scripts/build/build-release.sh`, `scripts/run/run-release-server.sh`.
- Modify `docs/architecture/client-web.md`, `overview.md`, `protocol.md`, `testing.md`: the web client as built.

---

### Task 1: Versioned WebSocket URL

The client builds the `/ws` URL the server parses; the parameter name and the URL format move to `shared` so both ends read one definition.

**Files:**
- Modify: `shared/src/protocol/protocol.rs`
- Modify: `server/src/socket/socket_api.rs`
- Modify: `server/tests/helpers/test_server.rs`
- Test: inline `mod tests` in `shared/src/protocol/protocol.rs`

- [ ] **Step 1: Write the failing test**

In `shared/src/protocol/protocol.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn get_versioned_websocket_url_appends_the_protocol_version_query() {
        assert_eq!(
            get_versioned_websocket_url("ws://127.0.0.1:3100/ws"),
            format!("ws://127.0.0.1:3100/ws?protocol_version={PROTOCOL_VERSION}"),
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --lib protocol::protocol::tests::get_versioned_websocket_url`

Expected: FAIL to compile:

```text
error[E0425]: cannot find function `get_versioned_websocket_url` in this scope
```

- [ ] **Step 3: Implement the query parameter and the URL**

In `shared/src/protocol/protocol.rs`, replace:

```rust
pub const PROTOCOL_VERSION: u16 = 1;
```

with:

```rust
pub const PROTOCOL_VERSION: u16 = 1;
/// The `/ws` query parameter carrying the client's `PROTOCOL_VERSION`.
pub const PROTOCOL_VERSION_QUERY_PARAMETER: &str = "protocol_version";

/// `websocket_url` with this build's protocol version as its query.
pub fn get_versioned_websocket_url(websocket_url: &str) -> String {
    format!("{websocket_url}?{PROTOCOL_VERSION_QUERY_PARAMETER}={PROTOCOL_VERSION}")
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --lib protocol::protocol::tests::get_versioned_websocket_url`

Expected: PASS:

```text
test protocol::protocol::tests::get_versioned_websocket_url_appends_the_protocol_version_query ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 464 filtered out
```

- [ ] **Step 5: Read the parameter name from shared on the server**

In `server/src/socket/socket_api.rs`, delete:

```rust
const PROTOCOL_VERSION_PARAMETER: &str = "protocol_version";

```

In `server/src/socket/socket_api.rs`, replace:

```rust
        .find(|(name, _)| *name == PROTOCOL_VERSION_PARAMETER)
```

with:

```rust
        .find(|(name, _)| *name == protocol::PROTOCOL_VERSION_QUERY_PARAMETER)
```

In `server/tests/helpers/test_server.rs`, replace:

```rust
        let query: String = protocol_version.map_or(String::new(), |version| format!("?protocol_version={version}"));
```

with:

```rust
        let query: String = protocol_version.map_or(String::new(), |version| {
            format!("?{}={version}", protocol::PROTOCOL_VERSION_QUERY_PARAMETER)
        });
```

- [ ] **Step 6: Run the server tests which read and send the parameter**

Run: `cargo test -p server --lib socket::socket_api && cargo test -p server --test protocol_errors`

Expected: PASS (the integration tests bind loopback ports):

```text
test socket::socket_api::tests::get_protocol_version_reads_the_query_parameter ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 119 filtered out
test run_connection_closes_on_an_oversize_frame ... ok
test open_socket_refuses_a_handshake_from_a_disallowed_or_missing_origin ... ok
test open_socket_closes_a_connection_with_a_wrong_or_missing_protocol_version ... ok
test run_connection_closes_on_a_text_frame ... ok
test run_connection_closes_every_connection_when_the_server_shuts_down ... ok
test dispatch_answers_requests_beyond_the_rate_limit_with_rate_limited ... ok
test join_game_rejects_an_unbalancing_team ... ok
test dispatch_rejects_requests_outside_their_phase ... ok
test join_game_rejects_a_taken_screen_name ... ok
test run_connection_closes_only_the_connection_which_sent_garbage ... ok
test join_game_rejects_a_full_game ... ok
test open_socket_closes_connections_beyond_the_per_ip_limit_with_server_full ... ok
test join_game_checks_the_password ... ok
test join_game_closes_a_connection_at_the_password_failure_limit ... ok
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

- [ ] **Step 7: Commit**

```sh
git add shared/src/protocol/protocol.rs
git add server/src/socket/socket_api.rs
git add server/tests/helpers/test_server.rs
git status --short
git commit -m "versioned websocket url"
```

Expected `git status --short` before the commit:

```text
M  server/src/socket/socket_api.rs
M  server/tests/helpers/test_server.rs
M  shared/src/protocol/protocol.rs
```

### Task 2: Setting defaults and the tutorial player id

The web client starts its settings signal from the declared defaults before it can read localStorage (the server render has none), and its tutorial HUD reads the tutorial player's member.

**Files:**
- Modify: `shared/src/settings/settings.rs`
- Modify: `shared/src/tutorial/tutorial.rs`
- Test: inline `mod tests` in `shared/src/settings/settings.rs`

- [ ] **Step 1: Write the failing test**

In `shared/src/settings/settings.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn default_value_gives_the_declared_default() {
        let hidden_by_default: Setting<ClientSettingKind, bool> = Setting::new(ClientSettingKind::Messages, false);

        assert!(NAME_LABELS_SETTING.default_value());
        assert!(!hidden_by_default.default_value());
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --lib settings::settings::`

Expected: FAIL to compile:

```text
error[E0599]: no method named `default_value` found for struct `settings::settings::Setting<K, T>` in the current scope
```

- [ ] **Step 3: Implement `default_value` and publish the tutorial player id**

In `shared/src/settings/settings.rs`, replace:

```rust
    pub fn write(&self, store: &impl SettingsStore, value: T) {
        store.store(self.key, value.into());
    }
```

with:

```rust
    pub fn write(&self, store: &impl SettingsStore, value: T) {
        store.store(self.key, value.into());
    }

    pub fn default_value(&self) -> T {
        self.default.clone()
    }
```

In `shared/src/tutorial/tutorial.rs`, replace:

```rust
const TUTORIAL_PLAYER_ID: MemberId = MemberId(0);
```

with:

```rust
pub const TUTORIAL_PLAYER_ID: MemberId = MemberId(0);
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p shared --lib settings::settings::`

Expected: PASS:

```text
test settings::settings::tests::default_value_gives_the_declared_default ... ok
test settings::settings::tests::from_bytes_rejects_empty_truncated_and_unknown_tag_bytes ... ok
test settings::settings::tests::storage_key_differs_per_setting ... ok
test settings::settings::tests::from_bytes_restores_every_value ... ok
test settings::settings::tests::read_returns_the_default_when_absent ... ok
test settings::settings::tests::read_falls_back_to_the_default_on_undecodable_bytes ... ok
test settings::settings::tests::write_then_read_round_trips_through_the_store ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 459 filtered out
```

- [ ] **Step 5: Commit**

```sh
git add shared/src/settings/settings.rs
git add shared/src/tutorial/tutorial.rs
git status --short
git commit -m "setting defaults and tutorial player id"
```

Expected `git status --short` before the commit:

```text
M  shared/src/settings/settings.rs
M  shared/src/tutorial/tutorial.rs
```

### Task 3: Web shell, prerender and static files

The document shell, the `cargo leptos watch` server, the prerender a release build writes as `index.html`, and the hydrate entry point. `App` renders only an empty screen layer for now; later tasks fill it.

**Files:**
- Create: `web/src/app.rs`
- Create: `web/src/server.rs`
- Create: `web/src/client/mod.rs`
- Create: `web/src/client/hydrate.rs`
- Create: `web/static/game-server.json`
- Create: `web/static/robots.txt`
- Create: `web/static/favicon.ico`
- Create: `web/static/fonts/bacter.ttf`
- Create: `web/style/main.scss`
- Create: `web/style/_tokens.scss`
- Create: `web/style/_reset.scss`
- Create: `web/style/_typography.scss`
- Create: `web/style/_layout.scss`
- Create: `scripts/dev/web.sh`
- Create: `web/tests/prerender.rs`
- Modify: `web/src/lib.rs`
- Modify: `web/src/main.rs`
- Test: `web/tests/prerender.rs`

- [ ] **Step 1: Write the failing test**

Create `web/tests/prerender.rs`:

```rust
#![cfg(feature = "ssr")]

use axum::body::Bytes;
use leptos::prelude::LeptosOptions;

/// Rendered here rather than read from a built site, so the assertions need no build.
async fn get_prerendered_document() -> String {
    let leptos_options: LeptosOptions = web::server::read_leptos_manifest_options().unwrap();
    let document_bytes: Bytes = web::server::prerender_document(leptos_options).await.unwrap();

    String::from_utf8(document_bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn prerender_document_references_the_client_bundle() {
    let document: String = get_prerendered_document().await;

    assert!(document.contains("/pkg/"));
    assert!(document.contains(".wasm"));
    assert!(document.contains(".css"));
}

#[tokio::test]
async fn prerender_document_names_the_game_in_the_tab() {
    let document: String = get_prerendered_document().await;

    assert!(document.contains("<title>Bacter</title>"));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --features ssr --test prerender`

Expected: FAIL to compile, `web::server` does not exist yet:

```text
error[E0433]: cannot find `server` in `web`
```

- [ ] **Step 3: Implement the shell, the dev server and the prerender**

Replace the whole contents of `web/src/lib.rs` with:

```rust
// The release view tree's monomorphized type exceeds rustc's default query depth.
#![recursion_limit = "512"]

pub mod app;

// browser runtime glue; the ssr build compiles none of it
#[cfg(feature = "hydrate")]
pub mod client;

// the dev server and the prerender; a browser runs neither
#[cfg(feature = "ssr")]
pub mod server;
```

Replace the whole contents of `web/src/main.rs` with:

```rust
// Laying out the lib's release render future exceeds rustc's default query depth.
#![recursion_limit = "512"]

#[cfg(feature = "ssr")] // a browser build has no server to start
use shared::error::AppError;

#[cfg(feature = "ssr")] // a browser build has no server to start
const PRERENDER_ARGUMENT: &str = "prerender";

#[cfg(feature = "ssr")] // a browser build has no process to exit
enum ExitStatusKind {
    Failed = 1,
    Usage = 64,
}

#[cfg(feature = "ssr")] // a browser build has no server to start
#[tokio::main]
async fn main() {
    let first_argument: Option<String> = std::env::args().nth(1);

    let result: Result<(), AppError> = match first_argument.as_deref() {
        None => web::server::serve().await,
        Some(PRERENDER_ARGUMENT) => web::server::write_prerendered_document().await,
        Some(unrecognized_argument) => {
            eprintln!("unrecognized argument; [argument={unrecognized_argument} expected={PRERENDER_ARGUMENT}]");
            std::process::exit(ExitStatusKind::Usage as i32);
        }
    };

    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(ExitStatusKind::Failed as i32);
    }
}

#[cfg(not(feature = "ssr"))] // the browser build's entry point is the lib's hydrate function
fn main() {}
```

Create `web/src/app.rs`:

```rust
use leptos::prelude::*;

#[cfg(feature = "ssr")] // a browser build renders no document head
use leptos::hydration::{AutoReload, HydrationScripts};
#[cfg(feature = "ssr")] // a browser build renders no document head
use leptos_meta::HashedStylesheet;

#[cfg(feature = "ssr")] // a browser build renders no document
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en-US">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <meta name="theme-color" content="#000000" />
                <title>Bacter</title>
                <link rel="icon" href="/favicon.ico" />
                <HashedStylesheet options=options.clone() id="leptos" />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    view! { <div class="screen-layer"></div> }
}
```

Create `web/src/server.rs`:

```rust
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use any_spawner::Executor;
use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{Request, StatusCode};
use axum::routing;
use leptos::prelude::*;

use shared::error::AppError;

use crate::app;

/// The one route; the screen is client state, never a URL.
pub const PRERENDERED_ROUTE_PATH: &str = "/";
const PRERENDERED_DOCUMENT_NAME: &str = "index.html";
const CRATE_DIRECTORY: &str = env!("CARGO_MANIFEST_DIR");
const MANIFEST_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");

/// `cargo leptos watch`'s server: the shell at `/`, the site root's files elsewhere.
pub async fn serve() -> Result<(), AppError> {
    initialize_executor();

    let configuration: ConfFile = get_configuration(None)
        .map_err(|error| AppError::from_error("reading the leptos settings failed", Box::new(error)))?;
    let leptos_options: LeptosOptions = configuration.leptos_options;
    let address: SocketAddr = leptos_options.site_addr;
    let shell_options: LeptosOptions = leptos_options.clone();
    let router: Router = Router::new()
        .route(
            PRERENDERED_ROUTE_PATH,
            routing::get(leptos_axum::render_app_to_stream(move || {
                app::shell(shell_options.clone())
            })),
        )
        .fallback(leptos_axum::file_and_error_handler(app::shell))
        .with_state(leptos_options);
    let listener: tokio::net::TcpListener = tokio::net::TcpListener::bind(&address).await.map_err(|error| {
        AppError::from_error(
            &format!("binding the dev server failed; [address={address}]"),
            Box::new(error),
        )
    })?;

    leptos::logging::log!("listening; [address=http://{address}]");

    axum::serve(listener, router.into_make_service())
        .await
        .map_err(|error| AppError::from_error("the dev server stopped", Box::new(error)))
}

/// Writes `index.html` into the site root of a release build.
pub async fn write_prerendered_document() -> Result<(), AppError> {
    let leptos_options: LeptosOptions = read_leptos_manifest_options()?;
    let site_root: PathBuf = resolve_site_root(&leptos_options)?;
    let document_bytes: Bytes = prerender_document(leptos_options).await?;
    let document_path: PathBuf = site_root.join(PRERENDERED_DOCUMENT_NAME);

    fs::write(&document_path, &document_bytes).map_err(|error| {
        AppError::from_error(
            &format!(
                "writing the prerendered document failed; [path={}]",
                document_path.display(),
            ),
            Box::new(error),
        )
    })?;

    println!(
        "wrote the prerendered document; [path={} bytes={}]",
        document_path.display(),
        document_bytes.len(),
    );

    Ok(())
}

/// One complete document of the `/` route.
pub async fn prerender_document(leptos_options: LeptosOptions) -> Result<Bytes, AppError> {
    initialize_executor();

    let render_document = leptos_axum::render_app_async(move || app::shell(leptos_options.clone()));
    let request: Request<Body> = Request::builder()
        .uri(PRERENDERED_ROUTE_PATH)
        .body(Body::empty())
        .map_err(|error| AppError::from_error("building the render request failed", Box::new(error)))?;
    let response: axum::response::Response = render_document(request).await;
    let status: StatusCode = response.status();

    if !status.is_success() {
        return Err(AppError::new(&format!(
            "rendering the document did not succeed; [status={status} path={PRERENDERED_ROUTE_PATH}]",
        )));
    }

    axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .map_err(|error| AppError::from_error("reading the rendered document failed", Box::new(error)))
}

/// cargo-leptos sets the `LEPTOS_*` variables only for the processes it runs, so a prerender reads the manifest.
pub fn read_leptos_manifest_options() -> Result<LeptosOptions, AppError> {
    let configuration: ConfFile = get_configuration(Some(MANIFEST_PATH)).map_err(|error| {
        AppError::from_error(
            &format!("reading the leptos settings failed; [path={MANIFEST_PATH}]"),
            Box::new(error),
        )
    })?;

    Ok(configuration.leptos_options)
}

/// Rendering spawns onto a global executor; an error means one is already set.
fn initialize_executor() {
    let _ = Executor::init_tokio();
}

/// `site-root` is relative to the workspace root, the web crate's parent.
fn resolve_site_root(leptos_options: &LeptosOptions) -> Result<PathBuf, AppError> {
    let workspace_root: &Path = Path::new(CRATE_DIRECTORY)
        .parent()
        .ok_or_else(|| AppError::new("the web crate has no parent directory"))?;
    let declared_site_root: PathBuf = workspace_root.join(leptos_options.site_root.as_ref());

    fs::canonicalize(&declared_site_root).map_err(|error| {
        AppError::from_error(
            &format!(
                "the site root does not exist, so no build has run; [path={}]",
                declared_site_root.display(),
            ),
            Box::new(error),
        )
    })
}
```

Create `web/src/client/mod.rs`:

```rust
//! Browser runtime glue; compiled only under the `hydrate` feature.

pub mod hydrate;

pub use hydrate::*;
```

Create `web/src/client/hydrate.rs`:

```rust
use crate::app::App;

/// The entry point cargo-leptos's generated script calls once the page has loaded.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();

    // The wasm32 build caps `log` at info in release, so Debug here leaves release output at info.
    _ = console_log::init_with_level(log::Level::Debug);

    leptos::mount::hydrate_body(App);
}
```

- [ ] **Step 4: Add the static files**

Create `web/static/game-server.json`:

```json
{"websocket_url": "ws://127.0.0.1:3100/ws"}
```

Create `web/static/robots.txt`:

```text
User-agent: *
Allow: /
```

Run: `mkdir -p web/static/fonts && cp /Users/singularity/bacter/public/assets/bacter.ttf web/static/fonts/bacter.ttf && cp /Users/singularity/bacter/public/favicon.ico web/static/favicon.ico && ls web/static web/static/fonts`

The font and the favicon are the original's files, copied as they are ([third-party material](../architecture/third-party.md)). Expected listing:

```text
web/static:
favicon.ico
fonts
game-server.json
robots.txt

web/static/fonts:
bacter.ttf
```

- [ ] **Step 5: Add the base stylesheet**

Create `web/style/main.scss`:

```scss
@use "tokens";
@use "reset";
@use "typography";
@use "layout";
```

Create `web/style/_tokens.scss`:

```scss
// Reference these via var(--token-name); raw values appear nowhere else.
:root {
    --color-black:              rgb(0, 0, 0);
    --color-white:              rgb(255, 255, 255);
    --color-backdrop:           rgb(70, 70, 70); // the scene's clear colour, shown before the first frame
    --color-shadow:             rgb(50, 50, 50); // the backdrop darkened by 20, as the scene's shadows
    --color-world-border:       rgb(210, 210, 210);
    --color-title-box:          rgb(10, 10, 10);
    --color-lobby-disabled:     rgb(110, 110, 110);
    --color-row-even-browser:   rgb(230, 230, 230);
    --color-row-even-menu:      rgb(240, 240, 240);
    --color-browser-pressed:    rgb(210, 210, 210);
    --color-control:            rgb(240, 240, 240);
    --color-control-hover:      rgb(220, 220, 220);
    --color-control-pressed:    rgb(200, 200, 200);
    --color-field-focus:        rgb(230, 230, 230);
    --color-field-border:       rgb(50, 50, 50);
    --color-radio-selected:     rgb(190, 190, 190);
    --color-issue:              rgb(255, 0, 0);
    --color-leaderboard-head:   rgb(200, 200, 200);
    --color-leaderboard-cell:   rgb(245, 245, 245);
    --color-dial:               rgb(215, 215, 215);
    --color-own-effect:         rgb(66, 244, 176);
    --color-enemy-effect:       rgb(255, 141, 135);

    --font-title:               _bacter;
    --font-heading:             Verdana, sans-serif;
    --font-menu:                Georgia, serif;
    --font-overlay:             Helvetica, Arial, sans-serif;
    --font-key:                 Consolas, monospace;

    --z-canvas:                 -2; // the canvas and the overlays drawn over it
    --z-shade:                  -1;
    --z-screen:                 1;
}
```

Create `web/style/_reset.scss`:

```scss
*,
*::before,
*::after {
    box-sizing: border-box;
}

* {
    margin: 0;
    padding: 0;
}

html,
body {
    height: 100%;
}

button,
input,
select {
    font: inherit;
    color: inherit;
}

/* The original hid the number spinners and the list arrow. */
input[type="number"]::-webkit-outer-spin-button,
input[type="number"]::-webkit-inner-spin-button {
    -webkit-appearance: none;
}

input[type="number"] {
    -moz-appearance: textfield;
}

select {
    -webkit-appearance: none;
    -moz-appearance: none;
}
```

Create `web/style/_typography.scss`:

```scss
@font-face {
    font-family: _bacter;
    src: url("/fonts/bacter.ttf");
}

body {
    font-family: var(--font-heading);
}
```

Create `web/style/_layout.scss`:

```scss
body {
    overflow: hidden;
    background: var(--color-backdrop);
    user-select: none;
    -webkit-user-select: none;
}

.game-canvas {
    position: fixed;
    left: 0;
    top: 0;
    display: block;
    width: 100%;
    height: 100%;
    z-index: var(--z-canvas);
}

/* After the canvas in the document, so above it at the same depth. */
.overlay-layer {
    position: fixed;
    inset: 0;
    z-index: var(--z-canvas);
    pointer-events: none;
}

.shade {
    position: fixed;
    inset: 0;
    background: var(--color-white);
    opacity: 0.5;
    z-index: var(--z-shade);
}

.screen-layer {
    position: fixed;
    inset: 0;
    z-index: var(--z-screen);
    pointer-events: none;

    > * {
        pointer-events: auto;
    }
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p web --features ssr --test prerender`

Expected: PASS:

```text
test prerender_document_names_the_game_in_the_tab ... ok
test prerender_document_references_the_client_bundle ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

- [ ] **Step 7: Check the browser build**

Run: `./scripts/test/check-wasm.sh`

Expected: the three checks finish without a warning:

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s)
    Finished `dev` profile [unoptimized + debuginfo] target(s)
    Finished `dev` profile [unoptimized + debuginfo] target(s)
```

- [ ] **Step 8: Add the dev web script**

Create `scripts/dev/web.sh`:

```sh
#!/usr/bin/env bash
# Serves the site with cargo-leptos on the web crate's site-addr, rebuilding on change.

set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}/web"

exec cargo leptos watch
```

Run: `chmod +x scripts/dev/web.sh && ls -l scripts/dev/web.sh | cut -c1-10`

Expected:

```text
-rwxr-xr-x
```

- [ ] **Step 9: Commit**

```sh
git add web/src/lib.rs
git add web/src/main.rs
git add web/tests/prerender.rs
git add web/src/app.rs
git add web/src/server.rs
git add web/src/client/mod.rs
git add web/src/client/hydrate.rs
git add web/static/game-server.json
git add web/static/robots.txt
git add web/static/favicon.ico
git add web/static/fonts/bacter.ttf
git add web/style/main.scss
git add web/style/_tokens.scss
git add web/style/_reset.scss
git add web/style/_typography.scss
git add web/style/_layout.scss
git add scripts/dev/web.sh
git status --short
git commit -m "web shell and prerender"
```

Expected `git status --short` before the commit:

```text
A  scripts/dev/web.sh
A  web/src/app.rs
A  web/src/client/hydrate.rs
A  web/src/client/mod.rs
M  web/src/lib.rs
M  web/src/main.rs
A  web/src/server.rs
A  web/static/favicon.ico
A  web/static/fonts/bacter.ttf
A  web/static/game-server.json
A  web/static/robots.txt
A  web/style/_layout.scss
A  web/style/_reset.scss
A  web/style/_tokens.scss
A  web/style/_typography.scss
A  web/style/main.scss
A  web/tests/prerender.rs
```

### Task 4: Screen model

Which screen shows, what Escape and the Back footers open, how the own member's state and a lost connection move the screen, and the notices of an involuntary leave.

**Files:**
- Create: `web/src/screen/mod.rs`
- Create: `web/src/screen/screen.rs`
- Modify: `web/src/lib.rs`
- Test: inline `mod tests` in `web/src/screen/screen.rs`

- [ ] **Step 1: Write the failing tests**

Replace the whole contents of `web/src/lib.rs` with:

```rust
// The release view tree's monomorphized type exceeds rustc's default query depth.
#![recursion_limit = "512"]

pub mod app;
pub mod screen;

// browser runtime glue; the ssr build compiles none of it
#[cfg(feature = "hydrate")]
pub mod client;

// the dev server and the prerender; a browser runs neither
#[cfg(feature = "ssr")]
pub mod server;
```

Create `web/src/screen/mod.rs`:

```rust
pub mod screen;

pub use screen::*;
```

Create `web/src/screen/screen.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use shared::protocol::{GameModeKindSerial, WorldShapeKindSerial};

    const GAME_ID: GameId = GameId(7);

    fn create_join_menu_from_create() -> Screen {
        Screen::JoinMenu {
            origin: JoinMenuOrigin::Create {
                settings: GameSettingsSerialIn {
                    title: String::from("Petri"),
                    mode: GameModeKindSerial::FreeForAll,
                    world_shape: WorldShapeKindSerial::Rectangle,
                    world_size_pixels: 800,
                    player_minimum: None,
                    player_cap: 16,
                    team_count: None,
                    leaderboard_length: 10,
                },
                password: None,
            },
        }
    }

    fn create_join_menu_from_browser() -> Screen {
        Screen::JoinMenu {
            origin: JoinMenuOrigin::Browser { game_id: GAME_ID },
        }
    }

    #[test]
    fn input_focus_steers_the_game_only_while_playing_spectating_or_in_the_tutorial() {
        let game_focused_screens: Vec<Screen> = [
            Screen::Title,
            Screen::Browser,
            Screen::CreateMenu,
            create_join_menu_from_browser(),
            Screen::SpectateMenu { game_id: GAME_ID },
            Screen::Playing,
            Screen::Spectating,
            Screen::RespawnMenu,
            Screen::PauseGame,
            Screen::PauseSpectate,
            Screen::Tutorial,
            Screen::PauseTutorial,
        ]
        .into_iter()
        .filter(|screen| screen.input_focus() == InputFocusKind::Game)
        .collect();

        assert_eq!(
            game_focused_screens,
            vec![Screen::Playing, Screen::Spectating, Screen::Tutorial],
        );
    }

    #[test]
    fn backdrop_shades_the_canvas_behind_menus_and_the_browser() {
        assert_eq!(Screen::Browser.backdrop(), ScreenBackdropKind::Shaded);
        assert_eq!(create_join_menu_from_create().backdrop(), ScreenBackdropKind::Shaded);
        assert_eq!(Screen::PauseTutorial.backdrop(), ScreenBackdropKind::Shaded);
        assert_eq!(Screen::Title.backdrop(), ScreenBackdropKind::Clear);
        assert_eq!(Screen::Playing.backdrop(), ScreenBackdropKind::Clear);
        assert_eq!(Screen::Tutorial.backdrop(), ScreenBackdropKind::Clear);
    }

    #[test]
    fn get_back_screen_leaves_lobby_screens_towards_the_title() {
        assert_eq!(Screen::Title.get_back_screen(None), None);
        assert_eq!(Screen::Browser.get_back_screen(None), Some(Screen::Title));
        assert_eq!(Screen::CreateMenu.get_back_screen(None), Some(Screen::Title));
        assert_eq!(
            create_join_menu_from_create().get_back_screen(None),
            Some(Screen::CreateMenu),
        );
        assert_eq!(
            create_join_menu_from_browser().get_back_screen(None),
            Some(Screen::Browser),
        );
        assert_eq!(
            Screen::SpectateMenu { game_id: GAME_ID }.get_back_screen(None),
            Some(Screen::Browser),
        );
    }

    #[test]
    fn get_back_screen_pauses_a_game_and_the_tutorial() {
        assert_eq!(Screen::Playing.get_back_screen(None), Some(Screen::PauseGame));
        assert_eq!(Screen::Spectating.get_back_screen(None), Some(Screen::PauseSpectate));
        assert_eq!(Screen::Tutorial.get_back_screen(None), Some(Screen::PauseTutorial));
        assert_eq!(Screen::PauseTutorial.get_back_screen(None), Some(Screen::Tutorial));
    }

    #[test]
    fn get_back_screen_returns_from_a_game_menu_to_the_screen_of_the_own_state() {
        let alive: Option<OwnMemberStateKind> = Some(OwnMemberStateKind::AliveParticipant);
        let dead: Option<OwnMemberStateKind> = Some(OwnMemberStateKind::DeadParticipant);

        assert_eq!(Screen::PauseGame.get_back_screen(alive), Some(Screen::Playing));
        assert_eq!(Screen::PauseGame.get_back_screen(dead), Some(Screen::Spectating));
        assert_eq!(Screen::PauseSpectate.get_back_screen(dead), Some(Screen::Spectating));
        assert_eq!(Screen::RespawnMenu.get_back_screen(dead), Some(Screen::Spectating));
    }

    #[test]
    fn after_own_member_state_change_enters_the_game_from_a_join_or_spectate_menu() {
        assert_eq!(
            create_join_menu_from_create().after_own_member_state_change(OwnMemberStateKind::AliveParticipant),
            Screen::Playing,
        );
        assert_eq!(
            create_join_menu_from_browser().after_own_member_state_change(OwnMemberStateKind::DeadParticipant),
            Screen::Spectating,
        );
        assert_eq!(
            Screen::SpectateMenu { game_id: GAME_ID }.after_own_member_state_change(OwnMemberStateKind::Spectator),
            Screen::Spectating,
        );
    }

    #[test]
    fn after_own_member_state_change_follows_death_and_spawn() {
        assert_eq!(
            Screen::Playing.after_own_member_state_change(OwnMemberStateKind::DeadParticipant),
            Screen::Spectating,
        );
        assert_eq!(
            Screen::Spectating.after_own_member_state_change(OwnMemberStateKind::AliveParticipant),
            Screen::Playing,
        );
        assert_eq!(
            Screen::RespawnMenu.after_own_member_state_change(OwnMemberStateKind::AliveParticipant),
            Screen::Playing,
        );
        assert_eq!(
            Screen::RespawnMenu.after_own_member_state_change(OwnMemberStateKind::DeadParticipant),
            Screen::RespawnMenu,
        );
    }

    #[test]
    fn after_own_member_state_change_keeps_a_pause_menu_open_with_the_options_of_the_new_state() {
        assert_eq!(
            Screen::PauseGame.after_own_member_state_change(OwnMemberStateKind::DeadParticipant),
            Screen::PauseSpectate,
        );
        assert_eq!(
            Screen::PauseSpectate.after_own_member_state_change(OwnMemberStateKind::AliveParticipant),
            Screen::PauseGame,
        );
    }

    #[test]
    fn after_connection_lost_returns_to_the_title_unless_in_the_tutorial() {
        assert_eq!(Screen::Playing.after_connection_lost(), Screen::Title);
        assert_eq!(create_join_menu_from_browser().after_connection_lost(), Screen::Title);
        assert_eq!(Screen::Tutorial.after_connection_lost(), Screen::Tutorial);
        assert_eq!(Screen::PauseTutorial.after_connection_lost(), Screen::PauseTutorial);
    }

    #[test]
    fn text_tells_why_the_game_was_left() {
        assert_eq!(LobbyNoticeKind::GameClosed.text(), "The game has closed");
        assert_eq!(
            LobbyNoticeKind::SynchronisationLost.text(),
            "Lost synchronisation with the game",
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --lib screen::`

Expected: FAIL to compile:

```text
error[E0425]: cannot find type `GameId` in this scope
```

- [ ] **Step 3: Implement the screen model**

In `web/src/screen/screen.rs`, replace everything above `#[cfg(test)]` with:

```rust
use shared::game::GameId;
use shared::play::{InputFocusKind, OwnMemberStateKind};
use shared::protocol::{GameSettingsSerialIn, RejectionKind};

const SYNCHRONISATION_LOST_TEXT: &str = "Lost synchronisation with the game";

/// What the page shows; one route, so never a URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Screen {
    Title,
    Browser,
    CreateMenu,
    JoinMenu { origin: JoinMenuOrigin },
    SpectateMenu { game_id: GameId },
    Playing,
    Spectating,
    RespawnMenu,
    PauseGame,
    PauseSpectate,
    Tutorial,
    PauseTutorial,
}

impl Screen {
    pub fn input_focus(&self) -> InputFocusKind {
        match self {
            Screen::Playing | Screen::Spectating | Screen::Tutorial => InputFocusKind::Game,
            Screen::Title
            | Screen::Browser
            | Screen::CreateMenu
            | Screen::JoinMenu { .. }
            | Screen::SpectateMenu { .. }
            | Screen::RespawnMenu
            | Screen::PauseGame
            | Screen::PauseSpectate
            | Screen::PauseTutorial => InputFocusKind::Menu,
        }
    }

    pub fn backdrop(&self) -> ScreenBackdropKind {
        match self {
            Screen::Browser
            | Screen::CreateMenu
            | Screen::JoinMenu { .. }
            | Screen::SpectateMenu { .. }
            | Screen::RespawnMenu
            | Screen::PauseGame
            | Screen::PauseSpectate
            | Screen::PauseTutorial => ScreenBackdropKind::Shaded,
            Screen::Title | Screen::Playing | Screen::Spectating | Screen::Tutorial => ScreenBackdropKind::Clear,
        }
    }

    /// Escape and every Back footer; `None` where there is nowhere to go back to.
    pub fn get_back_screen(&self, own_member_state: Option<OwnMemberStateKind>) -> Option<Screen> {
        match self {
            Screen::Title => None,
            Screen::Browser | Screen::CreateMenu => Some(Screen::Title),
            Screen::JoinMenu {
                origin: JoinMenuOrigin::Create { .. },
            } => Some(Screen::CreateMenu),
            Screen::JoinMenu {
                origin: JoinMenuOrigin::Browser { .. },
            }
            | Screen::SpectateMenu { .. } => Some(Screen::Browser),
            Screen::Playing => Some(Screen::PauseGame),
            Screen::Spectating => Some(Screen::PauseSpectate),
            Screen::RespawnMenu | Screen::PauseGame | Screen::PauseSpectate => Some(get_game_screen(own_member_state)),
            Screen::Tutorial => Some(Screen::PauseTutorial),
            Screen::PauseTutorial => Some(Screen::Tutorial),
        }
    }

    /// A pause menu stays open, showing the options of the new state.
    pub fn after_own_member_state_change(&self, own_member_state: OwnMemberStateKind) -> Screen {
        match (own_member_state, self) {
            (
                OwnMemberStateKind::AliveParticipant,
                Screen::JoinMenu { .. } | Screen::SpectateMenu { .. } | Screen::Spectating | Screen::RespawnMenu,
            ) => Screen::Playing,
            (OwnMemberStateKind::AliveParticipant, Screen::PauseSpectate) => Screen::PauseGame,
            (
                OwnMemberStateKind::DeadParticipant | OwnMemberStateKind::Spectator,
                Screen::JoinMenu { .. } | Screen::SpectateMenu { .. } | Screen::Playing,
            ) => Screen::Spectating,
            (OwnMemberStateKind::DeadParticipant | OwnMemberStateKind::Spectator, Screen::PauseGame) => {
                Screen::PauseSpectate
            }
            _ => self.clone(),
        }
    }

    /// The tutorial runs locally and continues undisturbed.
    pub fn after_connection_lost(&self) -> Screen {
        match self {
            Screen::Tutorial | Screen::PauseTutorial => self.clone(),
            _ => Screen::Title,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JoinMenuOrigin {
    /// Nothing is created until the join step is submitted.
    Create {
        settings: GameSettingsSerialIn,
        password: Option<String>,
    },
    Browser {
        game_id: GameId,
    },
}

/// The white shade over the canvas behind menus and the browser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenBackdropKind {
    Shaded,
    Clear,
}

/// Told on the title or the browser after a game was left involuntarily.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyNoticeKind {
    GameClosed,
    SynchronisationLost,
}

impl LobbyNoticeKind {
    pub fn text(self) -> String {
        match self {
            LobbyNoticeKind::GameClosed => RejectionKind::GameNotFound.message_text(),
            LobbyNoticeKind::SynchronisationLost => String::from(SYNCHRONISATION_LOST_TEXT),
        }
    }
}

/// `None` until the own member's `Joined` is applied, when the member has no organism yet.
pub fn get_game_screen(own_member_state: Option<OwnMemberStateKind>) -> Screen {
    match own_member_state {
        Some(OwnMemberStateKind::AliveParticipant) => Screen::Playing,
        Some(OwnMemberStateKind::DeadParticipant | OwnMemberStateKind::Spectator) | None => Screen::Spectating,
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p web --lib screen::`

Expected: PASS:

```text
test screen::screen::tests::after_connection_lost_returns_to_the_title_unless_in_the_tutorial ... ok
test screen::screen::tests::after_own_member_state_change_enters_the_game_from_a_join_or_spectate_menu ... ok
test screen::screen::tests::after_own_member_state_change_follows_death_and_spawn ... ok
test screen::screen::tests::backdrop_shades_the_canvas_behind_menus_and_the_browser ... ok
test screen::screen::tests::after_own_member_state_change_keeps_a_pause_menu_open_with_the_options_of_the_new_state ... ok
test screen::screen::tests::get_back_screen_leaves_lobby_screens_towards_the_title ... ok
test screen::screen::tests::get_back_screen_returns_from_a_game_menu_to_the_screen_of_the_own_state ... ok
test screen::screen::tests::input_focus_steers_the_game_only_while_playing_spectating_or_in_the_tutorial ... ok
test screen::screen::tests::text_tells_why_the_game_was_left ... ok
test screen::screen::tests::get_back_screen_pauses_a_game_and_the_tutorial ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

- [ ] **Step 5: Commit**

```sh
git add web/src/lib.rs
git add web/src/screen/mod.rs
git add web/src/screen/screen.rs
git status --short
git commit -m "screen model"
```

Expected `git status --short` before the commit:

```text
M  web/src/lib.rs
A  web/src/screen/mod.rs
A  web/src/screen/screen.rs
```

### Task 5: Connection status and socket URL

The title's notice line and Reconnect offer per connection state, and the socket URL from `game-server.json` (a null URL is the page's own origin).

**Files:**
- Create: `web/src/connection/mod.rs`
- Create: `web/src/connection/connection_status.rs`
- Create: `web/src/connection/websocket_url.rs`
- Modify: `web/src/lib.rs`
- Test: inline `mod tests` in both new files

- [ ] **Step 1: Write the failing tests**

Replace the whole contents of `web/src/lib.rs` with:

```rust
// The release view tree's monomorphized type exceeds rustc's default query depth.
#![recursion_limit = "512"]

pub mod app;
pub mod connection;
pub mod screen;

// browser runtime glue; the ssr build compiles none of it
#[cfg(feature = "hydrate")]
pub mod client;

// the dev server and the prerender; a browser runs neither
#[cfg(feature = "ssr")]
pub mod server;
```

Create `web/src/connection/mod.rs`:

```rust
pub mod connection_status;
pub mod websocket_url;

pub use connection_status::*;
pub use websocket_url::*;
```

Create `web/src/connection/connection_status.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lobby_access_is_enabled_only_while_open() {
        assert_eq!(ConnectionStatusKind::Open.lobby_access(), LobbyAccessKind::Enabled);
        assert_eq!(
            ConnectionStatusKind::Connecting.lobby_access(),
            LobbyAccessKind::Disabled,
        );
        assert_eq!(
            ConnectionStatusKind::Closed { close_reason: None }.lobby_access(),
            LobbyAccessKind::Disabled,
        );
    }

    #[test]
    fn notice_text_tells_the_connection_state_or_the_close_reason() {
        assert_eq!(
            ConnectionStatusKind::Connecting.notice_text(),
            Some("Connecting to the game server"),
        );
        assert_eq!(ConnectionStatusKind::Open.notice_text(), None);
        assert_eq!(
            ConnectionStatusKind::Closed { close_reason: None }.notice_text(),
            Some("Cannot reach the game server"),
        );
        assert_eq!(
            ConnectionStatusKind::Closed {
                close_reason: Some(CloseReasonKind::SlowConsumer),
            }
            .notice_text(),
            Some("Disconnected: the connection could not keep up"),
        );
    }

    #[test]
    fn reconnect_offer_is_made_after_a_close_other_than_a_version_mismatch() {
        assert_eq!(
            ConnectionStatusKind::Closed { close_reason: None }.reconnect_offer(),
            ReconnectOfferKind::Offered,
        );
        assert_eq!(
            ConnectionStatusKind::Closed {
                close_reason: Some(CloseReasonKind::ServerShutdown),
            }
            .reconnect_offer(),
            ReconnectOfferKind::Offered,
        );
        assert_eq!(
            ConnectionStatusKind::Closed {
                close_reason: Some(CloseReasonKind::ProtocolVersionMismatch),
            }
            .reconnect_offer(),
            ReconnectOfferKind::NotOffered,
        );
        assert_eq!(
            ConnectionStatusKind::Connecting.reconnect_offer(),
            ReconnectOfferKind::NotOffered,
        );
        assert_eq!(
            ConnectionStatusKind::Open.reconnect_offer(),
            ReconnectOfferKind::NotOffered,
        );
    }
}
```

Create `web/src/connection/websocket_url.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn create_page_origin(page_protocol: &str) -> PageOrigin {
        PageOrigin {
            protocol: String::from(page_protocol),
            host: String::from("bacter.example:8443"),
        }
    }

    #[test]
    fn resolve_websocket_url_keeps_a_configured_url() {
        assert_eq!(
            resolve_websocket_url(Some("ws://127.0.0.1:3100/ws"), &create_page_origin("http:")),
            protocol::get_versioned_websocket_url("ws://127.0.0.1:3100/ws"),
        );
    }

    #[test]
    fn resolve_websocket_url_follows_the_page_scheme_on_the_same_origin() {
        assert_eq!(
            resolve_websocket_url(None, &create_page_origin("https:")),
            protocol::get_versioned_websocket_url("wss://bacter.example:8443/ws"),
        );
        assert_eq!(
            resolve_websocket_url(None, &create_page_origin("http:")),
            protocol::get_versioned_websocket_url("ws://bacter.example:8443/ws"),
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --lib connection::`

Expected: FAIL to compile:

```text
error[E0425]: cannot find type `PageOrigin` in this scope
```

- [ ] **Step 3: Implement the status and the URL resolution**

In `web/src/connection/connection_status.rs`, replace everything above `#[cfg(test)]` with:

```rust
use shared::protocol::CloseReasonKind;
use shared::protocol::close_reason;

const CONNECTING_TEXT: &str = "Connecting to the game server";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionStatusKind {
    Connecting,
    Open,
    /// `None` for an abnormal close, a failed open or a close code no close reason uses.
    Closed {
        close_reason: Option<CloseReasonKind>,
    },
}

impl ConnectionStatusKind {
    /// Host and Join need the server.
    pub fn lobby_access(self) -> LobbyAccessKind {
        match self {
            ConnectionStatusKind::Open => LobbyAccessKind::Enabled,
            ConnectionStatusKind::Connecting | ConnectionStatusKind::Closed { .. } => LobbyAccessKind::Disabled,
        }
    }

    /// `None` while open.
    pub fn notice_text(self) -> Option<&'static str> {
        match self {
            ConnectionStatusKind::Connecting => Some(CONNECTING_TEXT),
            ConnectionStatusKind::Open => None,
            ConnectionStatusKind::Closed { close_reason } => {
                Some(close_reason.map_or(close_reason::UNREACHABLE_CLIENT_NOTICE, CloseReasonKind::client_notice))
            }
        }
    }

    /// A version mismatch needs a reload, which reconnecting would not give.
    pub fn reconnect_offer(self) -> ReconnectOfferKind {
        match self {
            ConnectionStatusKind::Closed {
                close_reason: Some(CloseReasonKind::ProtocolVersionMismatch),
            }
            | ConnectionStatusKind::Connecting
            | ConnectionStatusKind::Open => ReconnectOfferKind::NotOffered,
            ConnectionStatusKind::Closed { .. } => ReconnectOfferKind::Offered,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyAccessKind {
    Enabled,
    Disabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReconnectOfferKind {
    Offered,
    NotOffered,
}
```

In `web/src/connection/websocket_url.rs`, replace everything above `#[cfg(test)]` with:

```rust
use shared::protocol;

const SECURE_PAGE_PROTOCOL: &str = "https:";
const SECURE_WEBSOCKET_SCHEME: &str = "wss:";
const PLAIN_WEBSOCKET_SCHEME: &str = "ws:";
const WEBSOCKET_PATH: &str = "/ws";

/// The parts of the page's location a same-origin socket URL is built from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageOrigin {
    /// With its colon, as `location.protocol` gives it.
    pub protocol: String,
    /// With any port, as `location.host` gives it.
    pub host: String,
}

/// `configured_websocket_url` comes from `game-server.json`; `None` means the page's own origin.
pub fn resolve_websocket_url(configured_websocket_url: Option<&str>, page_origin: &PageOrigin) -> String {
    let websocket_url: String = match configured_websocket_url {
        Some(configured_websocket_url) => String::from(configured_websocket_url),
        None => get_same_origin_websocket_url(page_origin),
    };

    protocol::get_versioned_websocket_url(&websocket_url)
}

fn get_same_origin_websocket_url(page_origin: &PageOrigin) -> String {
    let scheme: &str = if page_origin.protocol == SECURE_PAGE_PROTOCOL {
        SECURE_WEBSOCKET_SCHEME
    } else {
        PLAIN_WEBSOCKET_SCHEME
    };

    format!("{scheme}//{}{WEBSOCKET_PATH}", page_origin.host)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p web --lib connection::`

Expected: PASS:

```text
test connection::connection_status::tests::lobby_access_is_enabled_only_while_open ... ok
test connection::connection_status::tests::notice_text_tells_the_connection_state_or_the_close_reason ... ok
test connection::connection_status::tests::reconnect_offer_is_made_after_a_close_other_than_a_version_mismatch ... ok
test connection::websocket_url::tests::resolve_websocket_url_keeps_a_configured_url ... ok
test connection::websocket_url::tests::resolve_websocket_url_follows_the_page_scheme_on_the_same_origin ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out
```

- [ ] **Step 5: Commit**

```sh
git add web/src/lib.rs
git add web/src/connection/mod.rs
git add web/src/connection/connection_status.rs
git add web/src/connection/websocket_url.rs
git status --short
git commit -m "connection status and socket url"
```

Expected `git status --short` before the commit:

```text
A  web/src/connection/connection_status.rs
A  web/src/connection/mod.rs
A  web/src/connection/websocket_url.rs
M  web/src/lib.rs
```

### Task 6: App signals and client settings

`AppSignals` is the reactive state the driver writes and the views read, shared through context. It starts with the screen, the connection, the lobby notice, the own member state and the client settings; later tasks add the signals their views need.

**Files:**
- Modify: `web/src/app.rs`
- Test: inline `mod tests` in `web/src/app.rs`

- [ ] **Step 1: Write the failing tests**

Append to the end of `web/src/app.rs`, after one blank line:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use shared::settings::{SettingKey, SettingValue};

    /// An in-memory store, as a platform's store behaves.
    struct MemorySettingsStore {
        stored_values: RefCell<BTreeMap<&'static str, SettingValue>>,
    }

    impl SettingsStore for MemorySettingsStore {
        fn load<K: SettingKey>(&self, key: K) -> Option<SettingValue> {
            self.stored_values.borrow().get(key.storage_key()).cloned()
        }

        fn store<K: SettingKey>(&self, key: K, value: SettingValue) {
            self.stored_values.borrow_mut().insert(key.storage_key(), value);
        }
    }

    fn create_store() -> MemorySettingsStore {
        MemorySettingsStore {
            stored_values: RefCell::new(BTreeMap::new()),
        }
    }

    #[test]
    fn read_gives_the_defaults_from_an_empty_store() {
        assert_eq!(ClientSettings::read(&create_store()), ClientSettings::defaults());
        assert_eq!(
            ClientSettings::defaults(),
            ClientSettings {
                name_labels_shown: true,
                messages_shown: true,
            },
        );
    }

    #[test]
    fn read_gives_back_what_write_stored() {
        let store: MemorySettingsStore = create_store();
        let client_settings: ClientSettings = ClientSettings {
            name_labels_shown: false,
            messages_shown: true,
        };

        client_settings.write(&store);

        assert_eq!(ClientSettings::read(&store), client_settings);
    }

    #[test]
    fn go_back_opens_the_back_screen() {
        let owner: Owner = Owner::new();
        owner.set();
        let signals: AppSignals = AppSignals::new();
        signals.screen.set(Screen::Browser);

        signals.go_back();

        assert_eq!(signals.screen.get_untracked(), Screen::Title);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --lib app::`

Expected: FAIL to compile:

```text
error[E0405]: cannot find trait `SettingsStore` in this scope
```

- [ ] **Step 3: Implement the signals, the client settings and `set_if_changed`**

In `web/src/app.rs`, replace everything above `#[cfg(test)]` with:

```rust
use leptos::prelude::*;

use shared::play::OwnMemberStateKind;
use shared::settings;
use shared::settings::SettingsStore;

use crate::connection::ConnectionStatusKind;
use crate::screen::{LobbyNoticeKind, Screen};

#[cfg(feature = "ssr")] // a browser build renders no document head
use leptos::hydration::{AutoReload, HydrationScripts};
#[cfg(feature = "ssr")] // a browser build renders no document head
use leptos_meta::HashedStylesheet;

/// The reactive state the driver and the views share.
#[derive(Clone, Copy, Debug)]
pub struct AppSignals {
    pub screen: RwSignal<Screen>,
    pub connection_status: RwSignal<ConnectionStatusKind>,
    pub lobby_notice: RwSignal<Option<LobbyNoticeKind>>,
    pub own_member_state: RwSignal<Option<OwnMemberStateKind>>,
    pub client_settings: RwSignal<ClientSettings>,
}

impl AppSignals {
    pub fn new() -> AppSignals {
        AppSignals {
            screen: RwSignal::new(Screen::Title),
            connection_status: RwSignal::new(ConnectionStatusKind::Connecting),
            lobby_notice: RwSignal::new(None),
            own_member_state: RwSignal::new(None),
            client_settings: RwSignal::new(ClientSettings::defaults()),
        }
    }

    pub fn navigate(&self, screen: Screen) {
        self.screen.set(screen);
    }

    /// Escape and the Back footers.
    pub fn go_back(&self) {
        let back_screen: Option<Screen> =
            self.screen.get_untracked().get_back_screen(self.own_member_state.get_untracked());

        if let Some(back_screen) = back_screen {
            self.navigate(back_screen);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientSettings {
    pub name_labels_shown: bool,
    pub messages_shown: bool,
}

impl ClientSettings {
    pub fn defaults() -> ClientSettings {
        ClientSettings {
            name_labels_shown: settings::NAME_LABELS_SETTING.default_value(),
            messages_shown: settings::MESSAGES_SETTING.default_value(),
        }
    }

    pub fn read(store: &impl SettingsStore) -> ClientSettings {
        ClientSettings {
            name_labels_shown: settings::NAME_LABELS_SETTING.read(store),
            messages_shown: settings::MESSAGES_SETTING.read(store),
        }
    }

    pub fn write(&self, store: &impl SettingsStore) {
        settings::NAME_LABELS_SETTING.write(store, self.name_labels_shown);
        settings::MESSAGES_SETTING.write(store, self.messages_shown);
    }
}

#[cfg(feature = "ssr")] // a browser build renders no document
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en-US">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <meta name="theme-color" content="#000000" />
                <title>Bacter</title>
                <link rel="icon" href="/favicon.ico" />
                <HashedStylesheet options=options.clone() id="leptos" />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    let signals: AppSignals = AppSignals::new();
    provide_context(signals);

    view! { <div class="screen-layer"></div> }
}

/// Writes only a different value, so views reading the signal do not re-render for nothing.
pub fn set_if_changed<T: PartialEq + Send + Sync + 'static>(signal: RwSignal<T>, value: T) {
    let is_changed: bool = signal.with_untracked(|current_value| *current_value != value);

    if is_changed {
        signal.set(value);
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p web --lib app::`

Expected: PASS:

```text
test app::tests::read_gives_the_defaults_from_an_empty_store ... ok
test app::tests::read_gives_back_what_write_stored ... ok
test app::tests::go_back_opens_the_back_screen ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out
```

- [ ] **Step 5: Commit**

```sh
git add web/src/app.rs
git status --short
git commit -m "app signals and client settings"
```

Expected `git status --short` before the commit:

```text
M  web/src/app.rs
```

### Task 7: Driver command seam

Views ask the driver for work through `canvas::dispatch_driver_command`. The browser side queues the command for the driver, which Task 27 implements in full; the ssr build has no driver and ignores commands.

**Files:**
- Create: `web/src/canvas/mod.rs`
- Create: `web/src/canvas/game_canvas.rs`
- Create: `web/src/canvas/driver.rs`
- Modify: `web/src/lib.rs`

- [ ] **Step 1: Add the command type and its dispatch**

Replace the whole contents of `web/src/lib.rs` with:

```rust
// The release view tree's monomorphized type exceeds rustc's default query depth.
#![recursion_limit = "512"]

pub mod app;
pub mod canvas;
pub mod connection;
pub mod screen;

// browser runtime glue; the ssr build compiles none of it
#[cfg(feature = "hydrate")]
pub mod client;

// the dev server and the prerender; a browser runs neither
#[cfg(feature = "ssr")]
pub mod server;
```

Create `web/src/canvas/mod.rs`:

```rust
pub mod game_canvas;
// browser runtime glue; the ssr build compiles none of it
#[cfg(feature = "hydrate")]
pub mod driver;

pub use game_canvas::*;
```

Create `web/src/canvas/game_canvas.rs`:

```rust
use shared::protocol::MessageSerialIn;

use crate::app::ClientSettings;

/// What the views ask of the driver, which owns the socket and the scenes.
#[derive(Clone, Debug, PartialEq)]
pub enum DriverCommandKind {
    SendRequest(MessageSerialIn),
    StartTutorial,
    LeaveTutorial,
    Reconnect,
    StoreClientSettings(ClientSettings),
}

#[cfg(feature = "hydrate")] // the ssr build has no driver to command
pub use browser::*;
#[cfg(not(feature = "hydrate"))] // the ssr build has no driver
pub use server::*;

#[cfg(feature = "hydrate")] // the ssr build has no driver to command
mod browser {
    use crate::canvas::DriverCommandKind;
    use crate::canvas::driver;

    pub fn dispatch_driver_command(command: DriverCommandKind) {
        driver::dispatch(command);
    }
}

#[cfg(not(feature = "hydrate"))] // the ssr build renders the canvas without driving it
mod server {
    use crate::canvas::DriverCommandKind;

    pub fn dispatch_driver_command(_command: DriverCommandKind) {}
}
```

Create `web/src/canvas/driver.rs`:

```rust
use std::cell::RefCell;
use std::collections::VecDeque;

use crate::canvas::DriverCommandKind;

thread_local! {
    /// Commands wait here while the driver is busy or not yet started.
    static PENDING_DRIVER_COMMANDS: RefCell<VecDeque<DriverCommandKind>> = const { RefCell::new(VecDeque::new()) };
}

pub fn dispatch(command: DriverCommandKind) {
    PENDING_DRIVER_COMMANDS.with_borrow_mut(|pending_commands| pending_commands.push_back(command));
}
```

- [ ] **Step 2: Build both sides of the seam**

Run: `cargo build -p web 2>&1 | grep -E '^(warning|error)|Finished' && ./scripts/test/check-wasm.sh`

Expected: every build finishes without a warning:

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s)
    Finished `dev` profile [unoptimized + debuginfo] target(s)
    Finished `dev` profile [unoptimized + debuginfo] target(s)
    Finished `dev` profile [unoptimized + debuginfo] target(s)
```

- [ ] **Step 3: Commit**

```sh
git add web/src/lib.rs
git add web/src/canvas/mod.rs
git add web/src/canvas/game_canvas.rs
git add web/src/canvas/driver.rs
git status --short
git commit -m "driver command seam"
```

Expected `git status --short` before the commit:

```text
A  web/src/canvas/driver.rs
A  web/src/canvas/game_canvas.rs
A  web/src/canvas/mod.rs
M  web/src/lib.rs
```

### Task 8: Menu form engine

`MenuForm` draws the original's menu: a header, a two-column table of rows, red issues under their field or in a bottom row, the submit button and the Back footer; Enter in any field submits. Field kinds: Text, Number (floored, 0 and blank empty, the placeholder the default), List, Radios (deselectable, or fixed for the fourth ability) and Button.

**Files:**
- Create: `web/src/menu/mod.rs`
- Create: `web/src/menu/menu_form.rs`
- Create: `web/style/components/_menu.scss`
- Modify: `web/src/lib.rs`
- Modify: `web/style/main.scss`
- Test: inline `mod tests` in `web/src/menu/menu_form.rs`

- [ ] **Step 1: Write the failing tests**

Replace the whole contents of `web/src/lib.rs` with:

```rust
// The release view tree's monomorphized type exceeds rustc's default query depth.
#![recursion_limit = "512"]

pub mod app;
pub mod canvas;
pub mod connection;
pub mod menu;
pub mod screen;

// browser runtime glue; the ssr build compiles none of it
#[cfg(feature = "hydrate")]
pub mod client;

// the dev server and the prerender; a browser runs neither
#[cfg(feature = "ssr")]
pub mod server;
```

Create `web/src/menu/mod.rs`:

```rust
pub mod menu_form;

pub use menu_form::*;
```

Create `web/src/menu/menu_form.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use shared::member::TeamKind;
    use shared::protocol::RangeBoundKind;

    #[test]
    fn place_issues_puts_an_issue_under_its_shown_field() {
        let issues: Vec<MenuIssue> = vec![MenuIssue::at_field(
            MenuFieldKind::ScreenName,
            "Screen name cannot be left empty",
        )];

        let placed_issues: PlacedIssues = place_issues(&issues, &[MenuFieldKind::ScreenName]);

        assert_eq!(
            placed_issues.get_field_texts(MenuFieldKind::ScreenName),
            vec![String::from("Screen name cannot be left empty")],
        );
        assert!(placed_issues.general_issues.is_empty());
    }

    #[test]
    fn place_issues_moves_issues_of_hidden_or_no_fields_to_the_general_row() {
        let issues: Vec<MenuIssue> = vec![
            MenuIssue::at_field(MenuFieldKind::PlayerCap, "Game is at maximum player capacity"),
            MenuIssue {
                field: None,
                text: String::from("The game has closed"),
            },
        ];

        let placed_issues: PlacedIssues = place_issues(&issues, &[MenuFieldKind::ScreenName]);

        assert!(placed_issues.field_issues.is_empty());
        assert_eq!(
            placed_issues.general_issues,
            vec![
                String::from("Game is at maximum player capacity"),
                String::from("The game has closed"),
            ],
        );
    }

    #[test]
    fn get_rejection_field_attaches_rejections_to_their_form_fields() {
        assert_eq!(
            get_rejection_field(RejectionKind::ScreenNameTaken),
            Some(MenuFieldKind::ScreenName),
        );
        assert_eq!(
            get_rejection_field(RejectionKind::TitleTaken),
            Some(MenuFieldKind::GameTitle),
        );
        assert_eq!(
            get_rejection_field(RejectionKind::SettingOutOfRange {
                field: SettingFieldKind::WorldSize,
                bound: RangeBoundKind::Above,
            }),
            Some(MenuFieldKind::WorldWidth),
        );
        assert_eq!(
            get_rejection_field(RejectionKind::PlayerCapBelowTeamCount),
            Some(MenuFieldKind::TeamCount),
        );
        assert_eq!(
            get_rejection_field(RejectionKind::PasswordIncorrect),
            Some(MenuFieldKind::Password),
        );
        assert_eq!(
            get_rejection_field(RejectionKind::TeamUnbalanced {
                requested: TeamKind::Red,
                smaller: TeamKind::Blue,
            }),
            Some(MenuFieldKind::AutoAssign),
        );
        assert_eq!(get_rejection_field(RejectionKind::GameNotFound), None);
        assert_eq!(get_rejection_field(RejectionKind::RoundInProgress), None);
    }

    #[test]
    fn from_rejection_takes_the_rejection_text() {
        assert_eq!(
            MenuIssue::from_rejection(RejectionKind::PasswordRequired),
            MenuIssue::at_field(MenuFieldKind::Password, "A password is required for this game"),
        );
    }

    #[test]
    fn parse_number_input_floors_and_blanks_zero() {
        assert_eq!(parse_number_input("800"), Some(800));
        assert_eq!(parse_number_input("16.9"), Some(16));
        assert_eq!(parse_number_input("-2.5"), Some(-3));
        assert_eq!(parse_number_input("0"), None);
        assert_eq!(parse_number_input("0.4"), None);
        assert_eq!(parse_number_input(""), None);
        assert_eq!(parse_number_input("e"), None);
    }

    #[test]
    fn get_toggled_radio_selects_one_and_deselects_on_a_second_click() {
        assert_eq!(get_toggled_radio(None, 1, RadioSelectionKind::Deselectable), Some(1));
        assert_eq!(get_toggled_radio(Some(0), 1, RadioSelectionKind::Deselectable), Some(1));
        assert_eq!(get_toggled_radio(Some(1), 1, RadioSelectionKind::Deselectable), None);
    }

    #[test]
    fn get_toggled_radio_keeps_a_fixed_selection() {
        assert_eq!(get_toggled_radio(Some(0), 0, RadioSelectionKind::Fixed), Some(0));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --lib menu::menu_form::`

Expected: FAIL to compile:

```text
error[E0425]: cannot find type `MenuIssue` in this scope
```

- [ ] **Step 3: Implement the engine**

In `web/src/menu/menu_form.rs`, replace everything above `#[cfg(test)]` with:

```rust
use leptos::ev;
use leptos::prelude::*;

use shared::protocol::{RejectionKind, RequestKind, SettingFieldKind};

const ENTER_KEY: &str = "Enter";
pub const BACK_TEXT: &str = "← Back";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuFieldKind {
    GameTitle,
    Password,
    WorldShape,
    WorldWidth,
    WorldHeight,
    PlayerMinimum,
    PlayerCap,
    TeamCount,
    LeaderboardLength,
    GameMode,
    ScreenName,
    Color,
    Skin,
    FirstAbility,
    SecondAbility,
    ThirdAbility,
    FourthAbility,
    Team,
    AutoAssign,
    NameLabels,
    Messages,
    LeaveGame,
    LeaveTutorial,
}

/// An error shown in red: under its field, or in the bottom row when `field` is `None` or not shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuIssue {
    pub field: Option<MenuFieldKind>,
    pub text: String,
}

impl MenuIssue {
    pub fn at_field(field: MenuFieldKind, text: &str) -> MenuIssue {
        MenuIssue {
            field: Some(field),
            text: String::from(text),
        }
    }

    pub fn from_rejection(reason: RejectionKind) -> MenuIssue {
        MenuIssue {
            field: get_rejection_field(reason),
            text: reason.message_text(),
        }
    }
}

/// The server's answer to the last request it refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestRejection {
    pub request: RequestKind,
    pub reason: RejectionKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacedIssues {
    pub field_issues: Vec<(MenuFieldKind, String)>,
    pub general_issues: Vec<String>,
}

impl PlacedIssues {
    pub fn get_field_texts(&self, field: MenuFieldKind) -> Vec<String> {
        self.field_issues
            .iter()
            .filter(|(issue_field, _)| *issue_field == field)
            .map(|(_, text)| text.clone())
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldVisibilityKind {
    Shown,
    Hidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldAvailabilityKind {
    Enabled,
    Disabled,
}

/// The fourth ability cannot be deselected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadioSelectionKind {
    Deselectable,
    Fixed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListOption {
    pub label: String,
    /// A CSS colour shown behind the label.
    pub swatch: Option<String>,
}

#[derive(Clone)]
pub enum MenuInput {
    Text {
        value: RwSignal<String>,
    },
    /// `None` is blank and shows the placeholder.
    Number {
        value: RwSignal<Option<i64>>,
        placeholder: u32,
    },
    List {
        selected_index: RwSignal<usize>,
        options: Signal<Vec<ListOption>>,
        availability: Signal<FieldAvailabilityKind>,
    },
    Radios {
        labels: Vec<&'static str>,
        selected_index: RwSignal<Option<usize>>,
        selection: RadioSelectionKind,
    },
    Button {
        on_click: Callback<()>,
    },
}

#[derive(Clone)]
pub struct MenuRow {
    pub field: MenuFieldKind,
    pub label: &'static str,
    pub input: MenuInput,
    pub visibility: Signal<FieldVisibilityKind>,
}

impl MenuRow {
    pub fn shown(field: MenuFieldKind, label: &'static str, input: MenuInput) -> MenuRow {
        MenuRow {
            field,
            label,
            input,
            visibility: Signal::stored(FieldVisibilityKind::Shown),
        }
    }

    pub fn with_visibility(self, visibility: Signal<FieldVisibilityKind>) -> MenuRow {
        MenuRow {
            field: self.field,
            label: self.label,
            input: self.input,
            visibility,
        }
    }
}

/// Header, two-column table, submit button and Back footer; Enter in any field submits.
#[component]
pub fn MenuForm(
    header: &'static str,
    submit_label: &'static str,
    rows: Vec<MenuRow>,
    issues: RwSignal<Vec<MenuIssue>>,
    on_submit: Callback<()>,
    on_back: Callback<()>,
) -> impl IntoView {
    let row_visibilities: Vec<(MenuFieldKind, Signal<FieldVisibilityKind>)> =
        rows.iter().map(|row| (row.field, row.visibility)).collect();
    let placed_issues: Memo<PlacedIssues> = Memo::new(move |_| {
        let shown_fields: Vec<MenuFieldKind> = row_visibilities
            .iter()
            .filter(|(_, visibility)| visibility.get() == FieldVisibilityKind::Shown)
            .map(|(field, _)| *field)
            .collect();

        place_issues(&issues.get(), &shown_fields)
    });
    let submit_on_enter = move |event: ev::KeyboardEvent| {
        if event.key() == ENTER_KEY {
            event.prevent_default();
            on_submit.run(());
        }
    };
    let row_views: Vec<AnyView> = rows.into_iter().map(|row| render_row(row, placed_issues)).collect();

    view! {
        <div class="menu" on:keydown=submit_on_enter>
            <div class="menu-header">
                <h2 class="menu-header-text">{header}</h2>
            </div>
            <div class="menu-content">
                <table class="menu-table">
                    <tbody>
                        {row_views}
                        <Show when=move || !placed_issues.get().general_issues.is_empty()>
                            <tr class="menu-row">
                                <td class="menu-cell menu-cell-label"></td>
                                <td class="menu-cell menu-cell-input">
                                    {move || render_issue_texts(placed_issues.get().general_issues)}
                                </td>
                            </tr>
                        </Show>
                    </tbody>
                </table>
                <button class="menu-submit" type="button" on:click=move |_| on_submit.run(())>
                    {submit_label}
                </button>
            </div>
            <footer class="menu-footer" on:click=move |_| on_back.run(())>
                <p class="menu-footer-text">{BACK_TEXT}</p>
            </footer>
        </div>
    }
}

/// Issues for fields not shown join the general ones, in order.
pub fn place_issues(issues: &[MenuIssue], shown_fields: &[MenuFieldKind]) -> PlacedIssues {
    let mut field_issues: Vec<(MenuFieldKind, String)> = Vec::new();
    let mut general_issues: Vec<String> = Vec::new();

    for issue in issues {
        match issue.field {
            Some(field) if shown_fields.contains(&field) => field_issues.push((field, issue.text.clone())),
            Some(_) | None => general_issues.push(issue.text.clone()),
        }
    }

    PlacedIssues {
        field_issues,
        general_issues,
    }
}

/// `None` for a rejection shown in the bottom row.
pub fn get_rejection_field(reason: RejectionKind) -> Option<MenuFieldKind> {
    match reason {
        RejectionKind::ScreenNameEmpty
        | RejectionKind::ScreenNameTooLong
        | RejectionKind::ScreenNameInvalidCharacter
        | RejectionKind::ScreenNameTaken => Some(MenuFieldKind::ScreenName),
        RejectionKind::TitleEmpty
        | RejectionKind::TitleTooLong
        | RejectionKind::TitleInvalidCharacter
        | RejectionKind::TitleTaken => Some(MenuFieldKind::GameTitle),
        RejectionKind::SettingOutOfRange { field, .. } => Some(get_setting_field(field)),
        RejectionKind::PlayerCapBelowMinimum | RejectionKind::GameFull => Some(MenuFieldKind::PlayerCap),
        RejectionKind::PlayerCapBelowTeamCount => Some(MenuFieldKind::TeamCount),
        RejectionKind::PasswordRequired | RejectionKind::PasswordIncorrect | RejectionKind::PasswordTooLong => {
            Some(MenuFieldKind::Password)
        }
        RejectionKind::TeamUnbalanced { .. } => Some(MenuFieldKind::AutoAssign),
        RejectionKind::SettingNotApplicable { .. }
        | RejectionKind::GameNotFound
        | RejectionKind::SpectatorLimitReached
        | RejectionKind::RoundInProgress
        | RejectionKind::AlreadyInGame
        | RejectionKind::NotInGame
        | RejectionKind::NotDead
        | RejectionKind::ServerFull
        | RejectionKind::ServerBusy
        | RejectionKind::RateLimited => None,
    }
}

/// Floored like the original's inputs; 0 and anything unreadable are blank.
pub fn parse_number_input(input_text: &str) -> Option<i64> {
    let parsed: Option<f64> = input_text.trim().parse::<f64>().ok();
    let floored: f64 = parsed.filter(|value| value.is_finite())?.floor();

    if floored == 0.0 {
        return None;
    }

    Some(floored.clamp(i64::MIN as f64, i64::MAX as f64) as i64)
}

/// Clicking the selected radio of a deselectable group clears it.
pub fn get_toggled_radio(
    selected_index: Option<usize>,
    clicked_index: usize,
    selection: RadioSelectionKind,
) -> Option<usize> {
    match selection {
        RadioSelectionKind::Fixed => selected_index,
        RadioSelectionKind::Deselectable if selected_index == Some(clicked_index) => None,
        RadioSelectionKind::Deselectable => Some(clicked_index),
    }
}

fn get_setting_field(setting_field: SettingFieldKind) -> MenuFieldKind {
    match setting_field {
        SettingFieldKind::WorldSize => MenuFieldKind::WorldWidth,
        SettingFieldKind::PlayerMinimum => MenuFieldKind::PlayerMinimum,
        SettingFieldKind::PlayerCap => MenuFieldKind::PlayerCap,
        SettingFieldKind::TeamCount => MenuFieldKind::TeamCount,
        SettingFieldKind::LeaderboardLength => MenuFieldKind::LeaderboardLength,
    }
}

fn render_row(row: MenuRow, placed_issues: Memo<PlacedIssues>) -> AnyView {
    let MenuRow {
        field,
        label,
        input,
        visibility,
    } = row;

    view! {
        <Show when=move || visibility.get() == FieldVisibilityKind::Shown>
            <tr class="menu-row">
                <td class="menu-cell menu-cell-label">{label}</td>
                <td class="menu-cell menu-cell-input">
                    {render_input(input.clone())}
                    {move || render_issue_texts(placed_issues.get().get_field_texts(field))}
                </td>
            </tr>
        </Show>
    }
    .into_any()
}

fn render_issue_texts(issue_texts: Vec<String>) -> AnyView {
    issue_texts
        .into_iter()
        .map(|issue_text| view! { <p class="menu-issue">{issue_text}</p> })
        .collect_view()
        .into_any()
}

fn render_input(input: MenuInput) -> AnyView {
    match input {
        MenuInput::Text { value } => render_text_input(value),
        MenuInput::Number { value, placeholder } => render_number_input(value, placeholder),
        MenuInput::List {
            selected_index,
            options,
            availability,
        } => render_list_input(selected_index, options, availability),
        MenuInput::Radios {
            labels,
            selected_index,
            selection,
        } => render_radios_input(labels, selected_index, selection),
        MenuInput::Button { on_click } => render_button_input(on_click),
    }
}

fn render_text_input(value: RwSignal<String>) -> AnyView {
    view! {
        <input
            class="menu-input"
            type="text"
            autocomplete="off"
            prop:value=move || value.get()
            on:input=move |event| value.set(event_target_value(&event))
        />
    }
    .into_any()
}

fn render_number_input(value: RwSignal<Option<i64>>, placeholder: u32) -> AnyView {
    view! {
        <input
            class="menu-input"
            type="number"
            autocomplete="off"
            placeholder=placeholder.to_string()
            prop:value=move || value.get().map(|number| number.to_string()).unwrap_or_default()
            on:input=move |event| value.set(parse_number_input(&event_target_value(&event)))
        />
    }
    .into_any()
}

fn render_list_input(
    selected_index: RwSignal<usize>,
    options: Signal<Vec<ListOption>>,
    availability: Signal<FieldAvailabilityKind>,
) -> AnyView {
    let select_option = move |event: ev::Event| {
        let chosen_index: Option<usize> = event_target_value(&event).parse::<usize>().ok();

        if let Some(chosen_index) = chosen_index {
            selected_index.set(chosen_index);
        }
    };
    let option_views = move || {
        options
            .get()
            .into_iter()
            .enumerate()
            .map(|(index, list_option)| {
                let swatch_style: String = list_option
                    .swatch
                    .map(|swatch| format!("background-color: {swatch}"))
                    .unwrap_or_default();

                view! {
                    <option value=index.to_string() style=swatch_style prop:selected=move || selected_index.get() == index>
                        {list_option.label}
                    </option>
                }
            })
            .collect_view()
    };

    view! {
        <select
            class="menu-input menu-list"
            prop:disabled=move || availability.get() == FieldAvailabilityKind::Disabled
            on:change=select_option
        >
            {option_views}
        </select>
    }
    .into_any()
}

fn render_radios_input(
    labels: Vec<&'static str>,
    selected_index: RwSignal<Option<usize>>,
    selection: RadioSelectionKind,
) -> AnyView {
    let radio_views: Vec<AnyView> = labels
        .into_iter()
        .enumerate()
        .map(|(index, label)| {
            let toggle_radio = move |_| {
                selected_index.update(|selected| *selected = get_toggled_radio(*selected, index, selection));
            };

            view! {
                <div class="menu-radio-option">
                    <div
                        class="menu-radio"
                        class:menu-radio-selected=move || selected_index.get() == Some(index)
                        on:click=toggle_radio
                    ></div>
                    <p class="menu-radio-text">{label}</p>
                </div>
            }
            .into_any()
        })
        .collect();

    view! { <div class="menu-radios">{radio_views}</div> }.into_any()
}

fn render_button_input(on_click: Callback<()>) -> AnyView {
    view! { <button class="menu-button" type="button" on:click=move |_| on_click.run(())></button> }.into_any()
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p web --lib menu::menu_form::`

Expected: PASS:

```text
test menu::menu_form::tests::get_rejection_field_attaches_rejections_to_their_form_fields ... ok
test menu::menu_form::tests::from_rejection_takes_the_rejection_text ... ok
test menu::menu_form::tests::get_toggled_radio_keeps_a_fixed_selection ... ok
test menu::menu_form::tests::place_issues_moves_issues_of_hidden_or_no_fields_to_the_general_row ... ok
test menu::menu_form::tests::get_toggled_radio_selects_one_and_deselects_on_a_second_click ... ok
test menu::menu_form::tests::parse_number_input_floors_and_blanks_zero ... ok
test menu::menu_form::tests::place_issues_puts_an_issue_under_its_shown_field ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out
```

- [ ] **Step 5: Add the menu styles**

Create `web/style/components/_menu.scss`:

```scss
.menu {
    position: fixed;
    inset: 0;
}

.menu-header {
    height: 58px;
    padding: 9px 0;
    background: var(--color-black);
    opacity: 0.95;
}

.menu-header-text {
    position: relative;
    top: 9px;
    color: var(--color-white);
    font-family: var(--font-heading);
    font-size: 18px;
    font-weight: bold;
    text-align: center;
}

.menu-content {
    height: calc(100% - 58px);
    overflow: auto;
    padding-bottom: 30px;
}

.menu-table {
    width: 100%;
    border-collapse: collapse;
    color: var(--color-black);
    font-family: var(--font-menu);
    background: var(--color-white);
    opacity: 0.9;
}

.menu-row {
    height: 44px;

    &:nth-child(odd) {
        background: var(--color-white);
    }

    &:nth-child(even) {
        background: var(--color-row-even-menu);
    }
}

.menu-cell {
    width: 50%;
    padding: 5px;
    font-size: 16px;
}

.menu-cell-label {
    text-align: right;
}

.menu-cell-input {
    text-align: left;
}

.menu-input {
    width: 100px;
    height: 29px;
    color: var(--color-black);
    font-family: serif;
    font-size: 14px;
    text-align: center;
    background: var(--color-white);
    border: none;
    border-bottom: 2px solid var(--color-field-border);
    border-radius: 3px;
    outline: none;

    &:focus {
        background: var(--color-field-focus);
    }
}

.menu-list {
    text-align-last: center;
    cursor: pointer;
}

.menu-radio-option {
    padding-bottom: 3px;

    &:last-child {
        padding-bottom: 6px;
    }
}

.menu-radio {
    position: relative;
    top: 4px;
    display: inline-block;
    width: 16px;
    height: 18px;
    margin: 0 8px 0 5px;
    background: var(--color-white);
    border-bottom: 2px solid var(--color-black);
    border-radius: 3px;
    cursor: pointer;
}

.menu-radio-selected {
    background: var(--color-radio-selected);
}

.menu-radio-text {
    display: inline;
    font-family: var(--font-menu);
}

.menu-button {
    position: relative;
    top: 2px;
    width: 45px;
    height: 22px;
    background: var(--color-control);
    border: none;
    border-bottom: 2px solid var(--color-black);
    border-radius: 3px;
    outline: none;
    cursor: pointer;

    &:hover {
        background: var(--color-control-hover);
    }

    &:active {
        background: var(--color-control-pressed);
    }
}

.menu-issue {
    margin: 5px 0 3px;
    color: var(--color-issue);
}

.menu-submit {
    display: block;
    width: 95px;
    height: 33px;
    margin: 20px auto 0;
    color: var(--color-black);
    font-family: var(--font-menu);
    font-size: 14px;
    font-weight: bold;
    background: var(--color-control);
    border: none;
    border-bottom: 2px solid var(--color-black);
    border-radius: 3px;
    outline: none;
    cursor: pointer;

    &:hover {
        background: var(--color-control-hover);
    }

    &:active {
        background: var(--color-control-pressed);
    }
}

.menu-footer {
    position: fixed;
    left: 0;
    bottom: 0;
    width: 100%;
    height: 30px;
    background: var(--color-black);
    opacity: 0.95;
    cursor: pointer;
}

.menu-footer-text {
    position: relative;
    top: 50%;
    margin-left: 10px;
    color: var(--color-white);
    font-family: var(--font-heading);
    font-size: 14px;
    transform: translateY(-50%);
}
```

Replace the whole contents of `web/style/main.scss` with:

```scss
@use "tokens";
@use "reset";
@use "typography";
@use "layout";
@use "components/menu";
```

- [ ] **Step 6: Commit**

```sh
git add web/src/lib.rs
git add web/src/menu/mod.rs
git add web/src/menu/menu_form.rs
git add web/style/main.scss
git add web/style/components/_menu.scss
git status --short
git commit -m "menu form engine"
```

Expected `git status --short` before the commit:

```text
M  web/src/lib.rs
A  web/src/menu/menu_form.rs
A  web/src/menu/mod.rs
A  web/style/components/_menu.scss
M  web/style/main.scss
```

### Task 9: Loadout fields

The rows the join, respawn and pause menus share (Color with swatches, Skin, the four abilities, Team and Auto Assign), their signals, and the loadout and team choice they make.

**Files:**
- Create: `web/src/menu/loadout_fields.rs`
- Modify: `web/src/menu/mod.rs`
- Test: inline `mod tests` in `web/src/menu/loadout_fields.rs`

- [ ] **Step 1: Write the failing tests**

Replace the whole contents of `web/src/menu/mod.rs` with:

```rust
pub mod loadout_fields;
pub mod menu_form;

pub use loadout_fields::*;
pub use menu_form::*;
```

Create `web/src/menu/loadout_fields.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn create_team_sizes(participant_counts: [u32; 3]) -> Vec<TeamSize> {
        [TeamKind::Red, TeamKind::Blue, TeamKind::Green]
            .into_iter()
            .zip(participant_counts)
            .map(|(team, participant_count)| TeamSize {
                team,
                participant_count,
            })
            .collect()
    }

    fn create_complete_values() -> LoadoutFieldValues {
        LoadoutFieldValues {
            color: OrganismColorKind::Lake,
            skin: SkinKind::Ghost,
            first: Some(FirstAbilityKind::Compress),
            second: Some(SecondAbilityKind::Immortality),
            third: Some(ThirdAbilityKind::Toxin),
            team: Some(TeamKind::Blue),
            auto_assign: AutoAssignKind::Unselected,
        }
    }

    #[test]
    fn to_loadout_reports_every_unselected_ability_at_its_field() {
        let values: LoadoutFieldValues = LoadoutFieldValues::join_defaults(&[]);

        assert_eq!(
            values.to_loadout(),
            Err(vec![
                MenuIssue::at_field(MenuFieldKind::FirstAbility, "Select a first ability"),
                MenuIssue::at_field(MenuFieldKind::SecondAbility, "Select a second ability"),
                MenuIssue::at_field(MenuFieldKind::ThirdAbility, "Select a third ability"),
            ]),
        );
    }

    #[test]
    fn to_loadout_builds_the_selected_loadout() {
        assert_eq!(
            create_complete_values().to_loadout(),
            Ok(Loadout {
                appearance: Appearance {
                    color: OrganismColorKind::Lake,
                    skin: SkinKind::Ghost,
                },
                first: FirstAbilityKind::Compress,
                second: SecondAbilityKind::Immortality,
                third: ThirdAbilityKind::Toxin,
            }),
        );
    }

    #[test]
    fn to_team_choice_asks_for_the_selected_team_unless_auto_assign_is_selected() {
        let mut values: LoadoutFieldValues = create_complete_values();

        assert_eq!(
            values.to_team_choice(),
            TeamChoiceKindSerialIn::Team(TeamKindSerial::Blue),
        );

        values.auto_assign = AutoAssignKind::Selected;

        assert_eq!(values.to_team_choice(), TeamChoiceKindSerialIn::Auto);
    }

    #[test]
    fn join_defaults_preselect_the_first_smallest_team() {
        let values: LoadoutFieldValues = LoadoutFieldValues::join_defaults(&create_team_sizes([2, 1, 1]));

        assert_eq!(values.team, Some(TeamKind::Blue));
        assert_eq!(values.color, OrganismColorKind::Fire);
        assert_eq!(values.skin, SkinKind::None);
    }

    #[test]
    fn get_team_options_label_each_team_with_its_size() {
        let labels: Vec<String> = get_team_options(&create_team_sizes([2, 0, 1]))
            .into_iter()
            .map(|list_option| list_option.label)
            .collect();

        assert_eq!(labels, vec!["Red: 2", "Blue: 0", "Green: 1"]);
    }

    #[test]
    fn get_color_options_put_each_colour_on_its_swatch() {
        let color_options: Vec<ListOption> = get_color_options();

        assert_eq!(color_options.len(), member::ORGANISM_COLOR_ORDER.len());
        assert_eq!(
            color_options[0],
            ListOption {
                label: String::from("Fire"),
                swatch: Some(String::from("rgb(255, 90, 81)")),
            },
        );
    }

    #[test]
    fn get_values_gives_back_the_values_new_was_given() {
        let owner: Owner = Owner::new();
        owner.set();
        let mut values: LoadoutFieldValues = create_complete_values();
        values.skin = SkinKind::None;
        values.auto_assign = AutoAssignKind::Selected;

        let loadout_fields: LoadoutFields =
            LoadoutFields::new(&values, Signal::stored(create_team_sizes([0, 0, 0])[..2].to_vec()));

        assert_eq!(loadout_fields.get_values(), values);
    }

    #[test]
    fn get_summary_team_sizes_pairs_the_counts_with_the_teams_in_team_order() {
        assert_eq!(
            get_summary_team_sizes(&[3, 4]),
            vec![
                TeamSize {
                    team: TeamKind::Red,
                    participant_count: 3,
                },
                TeamSize {
                    team: TeamKind::Blue,
                    participant_count: 4,
                },
            ],
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --lib menu::loadout_fields::`

Expected: FAIL to compile:

```text
error[E0425]: cannot find type `TeamSize` in this scope
```

- [ ] **Step 3: Implement the loadout fields**

In `web/src/menu/loadout_fields.rs`, replace everything above `#[cfg(test)]` with:

```rust
use leptos::prelude::*;

use shared::ability::{FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use shared::game::{GameModeKind, GameState};
use shared::member;
use shared::member::{Appearance, OrganismColorKind, SkinKind, TeamKind, TeamSizes};
use shared::protocol::{TeamChoiceKindSerialIn, TeamKindSerial};
use shared::render::RenderColor;
use shared::render::render_color;

use crate::menu::{
    FieldAvailabilityKind, FieldVisibilityKind, ListOption, MenuFieldKind, MenuInput, MenuIssue, MenuRow,
    RadioSelectionKind,
};

/// No radio selected means `SkinKind::None`.
pub const SKIN_RADIO_KINDS: [SkinKind; 3] = [SkinKind::Grid, SkinKind::Circles, SkinKind::Ghost];
pub const SKIN_RADIO_LABELS: [&str; 3] = ["Grid", "Circles", "Ghost"];
pub const FIRST_ABILITY_KINDS: [FirstAbilityKind; 2] = [FirstAbilityKind::Extend, FirstAbilityKind::Compress];
pub const FIRST_ABILITY_LABELS: [&str; 2] = ["Extend", "Compress"];
pub const SECOND_ABILITY_KINDS: [SecondAbilityKind; 2] = [SecondAbilityKind::Immortality, SecondAbilityKind::Freeze];
pub const SECOND_ABILITY_LABELS: [&str; 2] = ["Immortality", "Freeze"];
pub const THIRD_ABILITY_KINDS: [ThirdAbilityKind; 2] = [ThirdAbilityKind::Neutralize, ThirdAbilityKind::Toxin];
pub const THIRD_ABILITY_LABELS: [&str; 2] = ["Neutralize", "Toxin"];
pub const FOURTH_ABILITY_LABELS: [&str; 1] = ["Spore"];
/// A single unlabelled radio, as Auto Assign, Name Labels and Messages are drawn.
pub const SINGLE_RADIO_LABELS: [&str; 1] = [""];
const FIRST_ABILITY_MISSING_TEXT: &str = "Select a first ability";
const SECOND_ABILITY_MISSING_TEXT: &str = "Select a second ability";
const THIRD_ABILITY_MISSING_TEXT: &str = "Select a third ability";

/// Participants on one team.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TeamSize {
    pub team: TeamKind,
    pub participant_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoAssignKind {
    Selected,
    Unselected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoadoutFieldValues {
    pub color: OrganismColorKind,
    pub skin: SkinKind,
    pub first: Option<FirstAbilityKind>,
    pub second: Option<SecondAbilityKind>,
    pub third: Option<ThirdAbilityKind>,
    /// `None` outside skirmish.
    pub team: Option<TeamKind>,
    pub auto_assign: AutoAssignKind,
}

impl LoadoutFieldValues {
    /// The first palette colour, no skin, no ability, the first smallest team.
    pub fn join_defaults(team_sizes: &[TeamSize]) -> LoadoutFieldValues {
        LoadoutFieldValues {
            color: member::ORGANISM_COLOR_ORDER[0],
            skin: SkinKind::None,
            first: None,
            second: None,
            third: None,
            team: get_smallest_team(team_sizes),
            auto_assign: AutoAssignKind::Unselected,
        }
    }

    pub fn from_loadout(loadout: &Loadout, team: Option<TeamKind>, auto_assign: AutoAssignKind) -> LoadoutFieldValues {
        LoadoutFieldValues {
            color: loadout.appearance.color,
            skin: loadout.appearance.skin,
            first: Some(loadout.first),
            second: Some(loadout.second),
            third: Some(loadout.third),
            team,
            auto_assign,
        }
    }

    pub fn appearance(&self) -> Appearance {
        Appearance {
            color: self.color,
            skin: self.skin,
        }
    }

    /// Every unselected ability is an issue at its field.
    pub fn to_loadout(&self) -> Result<Loadout, Vec<MenuIssue>> {
        let mut issues: Vec<MenuIssue> = Vec::new();

        if self.first.is_none() {
            issues.push(MenuIssue::at_field(
                MenuFieldKind::FirstAbility,
                FIRST_ABILITY_MISSING_TEXT,
            ));
        }

        if self.second.is_none() {
            issues.push(MenuIssue::at_field(
                MenuFieldKind::SecondAbility,
                SECOND_ABILITY_MISSING_TEXT,
            ));
        }

        if self.third.is_none() {
            issues.push(MenuIssue::at_field(
                MenuFieldKind::ThirdAbility,
                THIRD_ABILITY_MISSING_TEXT,
            ));
        }

        let (Some(first), Some(second), Some(third)) = (self.first, self.second, self.third) else {
            return Err(issues);
        };

        Ok(Loadout {
            appearance: self.appearance(),
            first,
            second,
            third,
        })
    }

    /// The server ignores the choice outside skirmish.
    pub fn to_team_choice(&self) -> TeamChoiceKindSerialIn {
        match (self.auto_assign, self.team) {
            (AutoAssignKind::Unselected, Some(team)) => TeamChoiceKindSerialIn::Team(TeamKindSerial::from(&team)),
            (AutoAssignKind::Selected, _) | (AutoAssignKind::Unselected, None) => TeamChoiceKindSerialIn::Auto,
        }
    }
}

/// The form signals of a loadout, as indices into the option lists.
#[derive(Clone, Copy, Debug)]
pub struct LoadoutFields {
    pub color_index: RwSignal<usize>,
    pub skin_index: RwSignal<Option<usize>>,
    pub first_index: RwSignal<Option<usize>>,
    pub second_index: RwSignal<Option<usize>>,
    pub third_index: RwSignal<Option<usize>>,
    /// Always the one fourth ability.
    pub fourth_index: RwSignal<Option<usize>>,
    pub team_index: RwSignal<usize>,
    pub auto_assign_index: RwSignal<Option<usize>>,
    /// The teams `team_index` indexes, in team order.
    pub teams: Signal<Vec<TeamKind>>,
}

impl LoadoutFields {
    pub fn new(initial_values: &LoadoutFieldValues, team_sizes: Signal<Vec<TeamSize>>) -> LoadoutFields {
        let teams: Signal<Vec<TeamKind>> =
            Signal::derive(move || team_sizes.get().into_iter().map(|team_size| team_size.team).collect());
        let team_index: usize = initial_values
            .team
            .and_then(|team| teams.get_untracked().iter().position(|game_team| *game_team == team))
            .unwrap_or(0);

        LoadoutFields {
            color_index: RwSignal::new(get_index_of(&member::ORGANISM_COLOR_ORDER, initial_values.color).unwrap_or(0)),
            skin_index: RwSignal::new(get_index_of(&SKIN_RADIO_KINDS, initial_values.skin)),
            first_index: RwSignal::new(
                initial_values.first.and_then(|first| get_index_of(&FIRST_ABILITY_KINDS, first)),
            ),
            second_index: RwSignal::new(
                initial_values.second.and_then(|second| get_index_of(&SECOND_ABILITY_KINDS, second)),
            ),
            third_index: RwSignal::new(
                initial_values.third.and_then(|third| get_index_of(&THIRD_ABILITY_KINDS, third)),
            ),
            fourth_index: RwSignal::new(Some(0)),
            team_index: RwSignal::new(team_index),
            auto_assign_index: RwSignal::new(match initial_values.auto_assign {
                AutoAssignKind::Selected => Some(0),
                AutoAssignKind::Unselected => None,
            }),
            teams,
        }
    }

    pub fn get_values(&self) -> LoadoutFieldValues {
        LoadoutFieldValues {
            color: member::ORGANISM_COLOR_ORDER[self.color_index.get_untracked()],
            skin: self.skin_index.get_untracked().map_or(SkinKind::None, |skin_index| SKIN_RADIO_KINDS[skin_index]),
            first: self.first_index.get_untracked().map(|first_index| FIRST_ABILITY_KINDS[first_index]),
            second: self.second_index.get_untracked().map(|second_index| SECOND_ABILITY_KINDS[second_index]),
            third: self.third_index.get_untracked().map(|third_index| THIRD_ABILITY_KINDS[third_index]),
            team: self.teams.get_untracked().get(self.team_index.get_untracked()).copied(),
            auto_assign: match self.auto_assign_index.get_untracked() {
                Some(_) => AutoAssignKind::Selected,
                None => AutoAssignKind::Unselected,
            },
        }
    }
}

/// Color (hidden in skirmish, where the team gives the colour) and Skin.
pub fn create_appearance_rows(fields: LoadoutFields, mode: Signal<GameModeKind>) -> Vec<MenuRow> {
    vec![
        MenuRow::shown(
            MenuFieldKind::Color,
            "Color",
            MenuInput::List {
                selected_index: fields.color_index,
                options: Signal::stored(get_color_options()),
                availability: Signal::stored(FieldAvailabilityKind::Enabled),
            },
        )
        .with_visibility(Signal::derive(move || match mode.get() {
            GameModeKind::Skirmish => FieldVisibilityKind::Hidden,
            GameModeKind::FreeForAll | GameModeKind::Survival => FieldVisibilityKind::Shown,
        })),
        MenuRow::shown(
            MenuFieldKind::Skin,
            "Skin",
            create_radios_input(&SKIN_RADIO_LABELS, fields.skin_index, RadioSelectionKind::Deselectable),
        ),
    ]
}

/// The first three are required; the fourth is always Spore.
pub fn create_ability_rows(fields: LoadoutFields) -> Vec<MenuRow> {
    vec![
        MenuRow::shown(
            MenuFieldKind::FirstAbility,
            "1st Ability",
            create_radios_input(
                &FIRST_ABILITY_LABELS,
                fields.first_index,
                RadioSelectionKind::Deselectable,
            ),
        ),
        MenuRow::shown(
            MenuFieldKind::SecondAbility,
            "2nd Ability",
            create_radios_input(
                &SECOND_ABILITY_LABELS,
                fields.second_index,
                RadioSelectionKind::Deselectable,
            ),
        ),
        MenuRow::shown(
            MenuFieldKind::ThirdAbility,
            "3rd Ability",
            create_radios_input(
                &THIRD_ABILITY_LABELS,
                fields.third_index,
                RadioSelectionKind::Deselectable,
            ),
        ),
        MenuRow::shown(
            MenuFieldKind::FourthAbility,
            "4th Ability",
            create_radios_input(&FOURTH_ABILITY_LABELS, fields.fourth_index, RadioSelectionKind::Fixed),
        ),
    ]
}

/// Skirmish only; Auto Assign disables the team list.
pub fn create_team_rows(
    fields: LoadoutFields,
    mode: Signal<GameModeKind>,
    team_sizes: Signal<Vec<TeamSize>>,
) -> Vec<MenuRow> {
    let skirmish_visibility: Signal<FieldVisibilityKind> = Signal::derive(move || match mode.get() {
        GameModeKind::Skirmish => FieldVisibilityKind::Shown,
        GameModeKind::FreeForAll | GameModeKind::Survival => FieldVisibilityKind::Hidden,
    });
    let team_availability: Signal<FieldAvailabilityKind> =
        Signal::derive(move || match fields.auto_assign_index.get() {
            Some(_) => FieldAvailabilityKind::Disabled,
            None => FieldAvailabilityKind::Enabled,
        });

    vec![
        MenuRow::shown(
            MenuFieldKind::Team,
            "Team",
            MenuInput::List {
                selected_index: fields.team_index,
                options: Signal::derive(move || get_team_options(&team_sizes.get())),
                availability: team_availability,
            },
        )
        .with_visibility(skirmish_visibility),
        MenuRow::shown(
            MenuFieldKind::AutoAssign,
            "Auto Assign",
            create_radios_input(
                &SINGLE_RADIO_LABELS,
                fields.auto_assign_index,
                RadioSelectionKind::Deselectable,
            ),
        )
        .with_visibility(skirmish_visibility),
    ]
}

pub fn create_radios_input(
    labels: &[&'static str],
    selected_index: RwSignal<Option<usize>>,
    selection: RadioSelectionKind,
) -> MenuInput {
    MenuInput::Radios {
        labels: labels.to_vec(),
        selected_index,
        selection,
    }
}

pub fn get_color_label(color: OrganismColorKind) -> &'static str {
    match color {
        OrganismColorKind::Fire => "Fire",
        OrganismColorKind::Camel => "Camel",
        OrganismColorKind::Clay => "Clay",
        OrganismColorKind::Sun => "Sun",
        OrganismColorKind::Leaf => "Leaf",
        OrganismColorKind::Lime => "Lime",
        OrganismColorKind::Sky => "Sky",
        OrganismColorKind::Lake => "Lake",
        OrganismColorKind::Ocean => "Ocean",
        OrganismColorKind::Royal => "Royal",
        OrganismColorKind::Petal => "Petal",
        OrganismColorKind::Hot => "Hot",
    }
}

/// "Red", as the leaderboard and the team list name a team.
pub fn get_team_label(team: TeamKind) -> String {
    let team_name: &str = team.as_str();
    let mut team_name_characters: std::str::Chars<'_> = team_name.chars();

    match team_name_characters.next() {
        Some(first_character) => first_character.to_uppercase().chain(team_name_characters).collect(),
        None => String::new(),
    }
}

/// In palette order, each on a swatch of its colour.
pub fn get_color_options() -> Vec<ListOption> {
    member::ORGANISM_COLOR_ORDER
        .into_iter()
        .map(|color| ListOption {
            label: String::from(get_color_label(color)),
            swatch: Some(get_css_color(render_color::get_organism_color(color))),
        })
        .collect()
}

/// "Red: 2", in team order.
pub fn get_team_options(team_sizes: &[TeamSize]) -> Vec<ListOption> {
    team_sizes
        .iter()
        .map(|team_size| ListOption {
            label: format!("{}: {}", get_team_label(team_size.team), team_size.participant_count),
            swatch: None,
        })
        .collect()
}

/// The first in team order among the teams with the fewest participants.
pub fn get_smallest_team(team_sizes: &[TeamSize]) -> Option<TeamKind> {
    team_sizes
        .iter()
        .min_by_key(|team_size| team_size.participant_count)
        .map(|team_size| team_size.team)
}

/// `team_sizes` per team in team order, as a game summary carries them.
pub fn get_summary_team_sizes(team_participant_counts: &[u8]) -> Vec<TeamSize> {
    let team_count: u8 = u8::try_from(team_participant_counts.len()).unwrap_or(u8::MAX);

    member::get_game_teams(team_count)
        .into_iter()
        .zip(team_participant_counts)
        .map(|(team, participant_count)| TeamSize {
            team,
            participant_count: u32::from(*participant_count),
        })
        .collect()
}

/// Empty outside skirmish.
pub fn get_state_team_sizes(state: &GameState) -> Vec<TeamSize> {
    let team_count: u8 = match state.settings.mode {
        GameModeKind::Skirmish => state.settings.team_count.unwrap_or(0),
        GameModeKind::FreeForAll | GameModeKind::Survival => 0,
    };
    let team_sizes: TeamSizes = TeamSizes::from_members(&state.members, team_count, None);

    member::get_game_teams(team_count)
        .into_iter()
        .map(|team| TeamSize {
            team,
            participant_count: team_sizes.get_size(team).unwrap_or(0),
        })
        .collect()
}

pub fn get_css_color(color: RenderColor) -> String {
    format!("rgb({}, {}, {})", color.red, color.green, color.blue)
}

fn get_index_of<T: PartialEq>(kinds: &[T], kind: T) -> Option<usize> {
    kinds.iter().position(|candidate| *candidate == kind)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p web --lib menu::loadout_fields::`

Expected: PASS:

```text
test menu::loadout_fields::tests::get_summary_team_sizes_pairs_the_counts_with_the_teams_in_team_order ... ok
test menu::loadout_fields::tests::join_defaults_preselect_the_first_smallest_team ... ok
test menu::loadout_fields::tests::get_color_options_put_each_colour_on_its_swatch ... ok
test menu::loadout_fields::tests::to_loadout_builds_the_selected_loadout ... ok
test menu::loadout_fields::tests::get_team_options_label_each_team_with_its_size ... ok
test menu::loadout_fields::tests::to_team_choice_asks_for_the_selected_team_unless_auto_assign_is_selected ... ok
test menu::loadout_fields::tests::to_loadout_reports_every_unselected_ability_at_its_field ... ok
test menu::loadout_fields::tests::get_values_gives_back_the_values_new_was_given ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 25 filtered out
```

- [ ] **Step 5: Commit**

```sh
git add web/src/menu/mod.rs
git add web/src/menu/loadout_fields.rs
git status --short
git commit -m "loadout fields"
```

Expected `git status --short` before the commit:

```text
A  web/src/menu/loadout_fields.rs
M  web/src/menu/mod.rs
```

### Task 10: Game menu view and test fixture

`shared`'s test fixtures are `#[cfg(test)]` in that crate, so the web crate keeps its own, as the server does; this task has its first consumer, and Tasks 18 to 22 use it too.

**Files:**
- Create: `web/src/test_fixture/mod.rs`
- Create: `web/src/test_fixture/test_fixture.rs`
- Create: `web/src/menu/game_menu_view.rs`
- Modify: `web/src/lib.rs`
- Modify: `web/src/menu/mod.rs`
- Test: inline `mod tests` in `web/src/menu/game_menu_view.rs`

- [ ] **Step 1: Write the fixture and the failing tests**

Replace the whole contents of `web/src/lib.rs` with:

```rust
// The release view tree's monomorphized type exceeds rustc's default query depth.
#![recursion_limit = "512"]

pub mod app;
pub mod canvas;
pub mod connection;
pub mod menu;
pub mod screen;

// fixtures shared by the unit tests of several modules; absent from every non-test build
#[cfg(test)]
pub mod test_fixture;

// browser runtime glue; the ssr build compiles none of it
#[cfg(feature = "hydrate")]
pub mod client;

// the dev server and the prerender; a browser runs neither
#[cfg(feature = "ssr")]
pub mod server;
```

Create `web/src/test_fixture/mod.rs`:

```rust
pub mod test_fixture;

pub use test_fixture::*;
```

Create `web/src/test_fixture/test_fixture.rs`:

```rust
use shared::ability::{FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use shared::game::{GameModeKind, GameSettings, GameState};
use shared::member::{Appearance, Member, MemberId, MemberRoleKind, OrganismColorKind, Score, SkinKind};
use shared::world::WorldShapeKind;

pub const FIXTURE_SEED: u64 = 0x5eed;

pub fn create_state(mode: GameModeKind) -> GameState {
    let player_minimum: Option<u8> = match mode {
        GameModeKind::Survival => Some(2),
        GameModeKind::FreeForAll | GameModeKind::Skirmish => None,
    };
    let team_count: Option<u8> = match mode {
        GameModeKind::Skirmish => Some(2),
        GameModeKind::FreeForAll | GameModeKind::Survival => None,
    };
    let settings: GameSettings = GameSettings {
        title: String::from("Fixture game"),
        mode,
        world_shape: WorldShapeKind::Rectangle,
        world_width_pixels: 800,
        world_height_pixels: 800,
        player_minimum,
        player_cap: 8,
        team_count,
        leaderboard_length: 10,
    };

    GameState::new(settings, FIXTURE_SEED)
}

pub fn create_loadout() -> Loadout {
    Loadout {
        appearance: Appearance {
            color: OrganismColorKind::Ocean,
            skin: SkinKind::Circles,
        },
        first: FirstAbilityKind::Extend,
        second: SecondAbilityKind::Immortality,
        third: ThirdAbilityKind::Neutralize,
    }
}

pub fn create_participant(member_id: MemberId) -> Member {
    Member {
        member_id,
        screen_name: format!("player {}", member_id.0),
        role: MemberRoleKind::Participant,
        loadout: Some(create_loadout()),
        team: None,
        score: Score::zero(),
        organism: None,
    }
}

pub fn insert_member(state: &mut GameState, member: Member) {
    state.next_member_id = member.member_id.next().max(state.next_member_id);
    state.members.insert(member.member_id, member);
}
```

Replace the whole contents of `web/src/menu/mod.rs` with:

```rust
pub mod game_menu_view;
pub mod loadout_fields;
pub mod menu_form;

pub use game_menu_view::*;
pub use loadout_fields::*;
pub use menu_form::*;
```

Create `web/src/menu/game_menu_view.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixture;

    const OWN_MEMBER_ID: MemberId = MemberId(1);

    #[test]
    fn from_state_is_none_before_the_own_member_joins() {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll);

        assert_eq!(GameMenuView::from_state(&state, OWN_MEMBER_ID), None);
    }

    #[test]
    fn from_state_takes_the_own_loadout_team_and_the_team_sizes() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Skirmish);
        let mut own_member: Member = test_fixture::create_participant(OWN_MEMBER_ID);
        own_member.team = Some(TeamKind::Blue);
        test_fixture::insert_member(&mut state, own_member);

        let game_menu_view: GameMenuView = GameMenuView::from_state(&state, OWN_MEMBER_ID).unwrap();

        assert_eq!(game_menu_view.mode, GameModeKind::Skirmish);
        assert_eq!(game_menu_view.own_loadout, Some(test_fixture::create_loadout()));
        assert_eq!(game_menu_view.own_team, Some(TeamKind::Blue));
        assert_eq!(
            game_menu_view.team_sizes,
            vec![
                TeamSize {
                    team: TeamKind::Red,
                    participant_count: 0,
                },
                TeamSize {
                    team: TeamKind::Blue,
                    participant_count: 1,
                },
            ],
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --lib menu::game_menu_view::`

Expected: FAIL to compile:

```text
error[E0425]: cannot find type `MemberId` in this scope
```

- [ ] **Step 3: Implement the view**

In `web/src/menu/game_menu_view.rs`, replace everything above `#[cfg(test)]` with:

```rust
use shared::ability::Loadout;
use shared::game::{GameModeKind, GameState};
use shared::member::{Member, MemberId, TeamKind};

use crate::menu::TeamSize;
use crate::menu::loadout_fields;

/// The own member's part of a game, as the in-game menus show it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameMenuView {
    pub mode: GameModeKind,
    /// `None` for a Spectator.
    pub own_loadout: Option<Loadout>,
    pub own_team: Option<TeamKind>,
    pub team_sizes: Vec<TeamSize>,
}

impl GameMenuView {
    /// `None` until the own member's `Joined` is applied.
    pub fn from_state(state: &GameState, own_member_id: MemberId) -> Option<GameMenuView> {
        let own_member: &Member = state.members.get(&own_member_id)?;

        Some(GameMenuView {
            mode: state.settings.mode,
            own_loadout: own_member.loadout,
            own_team: own_member.team,
            team_sizes: loadout_fields::get_state_team_sizes(state),
        })
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p web --lib menu::game_menu_view::`

Expected: PASS:

```text
test menu::game_menu_view::tests::from_state_is_none_before_the_own_member_joins ... ok
test menu::game_menu_view::tests::from_state_takes_the_own_loadout_team_and_the_team_sizes ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 33 filtered out
```

- [ ] **Step 5: Commit**

```sh
git add web/src/lib.rs
git add web/src/test_fixture/mod.rs
git add web/src/test_fixture/test_fixture.rs
git add web/src/menu/mod.rs
git add web/src/menu/game_menu_view.rs
git status --short
git commit -m "game menu view and test fixture"
```

Expected `git status --short` before the commit:

```text
M  web/src/lib.rs
A  web/src/menu/game_menu_view.rs
M  web/src/menu/mod.rs
A  web/src/test_fixture/mod.rs
A  web/src/test_fixture/test_fixture.rs
```

**Deviations:** the premise above is out of date: `shared::game::test_fixture` is gated `#[cfg(any(test, feature = "test-support"))]` and the web crate's dev builds enable `test-support`, so Task 10's quality fixes deleted `web/src/test_fixture/` and the `test_fixture` module in `web/src/lib.rs`. The web tests import `shared::game::test_fixture` and call `create_state(mode, WorldShapeKind::Rectangle, 800)`; `insert_member` moved to `shared/src/game/test_fixture.rs`. Tasks 18 to 23 reference `crate::test_fixture`: they use the shared fixture for game states, members and loadouts (it already has `create_participant_with_organism`), and the `web/src/lib.rs` replacements in those tasks drop the `test_fixture` module. The web-only client-session helpers of Task 23 create `web/src/test_fixture/` then, holding only those helpers.

### Task 11: Game browser

The full-width table of the original (Title, Mode, Players, Spectators, Player Cap, Join and Spectate), the Back and Online Clients footer, and the lobby notice under the table. Every visit subscribes to the game list while connected: an accepted create, join or spectate ends the server's subscription.

**Files:**
- Create: `web/src/browser/mod.rs`
- Create: `web/src/browser/game_browser.rs`
- Create: `web/style/components/_browser.scss`
- Modify: `web/src/lib.rs`
- Modify: `web/src/app.rs`
- Modify: `web/style/main.scss`
- Test: inline `mod tests` in `web/src/browser/game_browser.rs`

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `web/src/lib.rs` with:

```rust
// The release view tree's monomorphized type exceeds rustc's default query depth.
#![recursion_limit = "512"]

pub mod app;
pub mod browser;
pub mod canvas;
pub mod connection;
pub mod menu;
pub mod screen;

// fixtures shared by the unit tests of several modules; absent from every non-test build
#[cfg(test)]
pub mod test_fixture;

// browser runtime glue; the ssr build compiles none of it
#[cfg(feature = "hydrate")]
pub mod client;

// the dev server and the prerender; a browser runs neither
#[cfg(feature = "ssr")]
pub mod server;
```

Create `web/src/browser/mod.rs`:

```rust
pub mod game_browser;

pub use game_browser::*;
```

Create `web/src/browser/game_browser.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_mode_label_names_each_mode_as_the_original() {
        assert_eq!(get_mode_label(GameModeKind::FreeForAll), "Free for All");
        assert_eq!(get_mode_label(GameModeKind::Skirmish), "Skirmish");
        assert_eq!(get_mode_label(GameModeKind::Survival), "Survival");
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --lib browser::`

Expected: FAIL to compile:

```text
error[E0433]: cannot find type `GameModeKind` in this scope
```

- [ ] **Step 3: Implement the browser and its `game_list` signal**

In `web/src/browser/game_browser.rs`, replace everything above `#[cfg(test)]` with:

```rust
use leptos::prelude::*;

use shared::game::{GameId, GameModeKind, GameSummary};
use shared::protocol::MessageSerialIn;

use crate::app::AppSignals;
use crate::canvas::DriverCommandKind;
use crate::canvas::game_canvas;
use crate::connection::ConnectionStatusKind;
use crate::menu::menu_form;
use crate::screen::{JoinMenuOrigin, LobbyNoticeKind, Screen};

/// The latest `GameList`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameListView {
    pub game_summaries: Vec<GameSummary>,
    pub online_client_count: u32,
}

impl GameListView {
    pub fn empty() -> GameListView {
        GameListView {
            game_summaries: Vec::new(),
            online_client_count: 0,
        }
    }
}

#[component]
pub fn GameBrowser() -> impl IntoView {
    let signals: AppSignals = expect_context();

    // An accepted create, join or spectate ends the server's subscription, so every visit subscribes again.
    Effect::new(move |_| {
        if signals.connection_status.get() == ConnectionStatusKind::Open {
            game_canvas::dispatch_driver_command(DriverCommandKind::SendRequest(MessageSerialIn::SubscribeGameList));
        }
    });

    let game_rows = move || {
        signals
            .game_list
            .get()
            .game_summaries
            .into_iter()
            .map(|game_summary| render_game_row(signals, game_summary))
            .collect_view()
    };
    let online_clients_text = move || format!("Online Clients: {}", signals.game_list.get().online_client_count);
    let notice_text = move || {
        signals
            .lobby_notice
            .get()
            .filter(|lobby_notice| *lobby_notice == LobbyNoticeKind::GameClosed)
            .map(|lobby_notice| lobby_notice.text())
    };

    view! {
        <div class="browser">
            <div class="browser-content">
                <table class="browser-table">
                    <thead>
                        <tr>
                            <th class="browser-title-heading">"Title"</th>
                            <th>"Mode"</th>
                            <th>"Players"</th>
                            <th>"Spectators"</th>
                            <th>"Player Cap"</th>
                            <th class="browser-join-heading" colspan="2">"Bacter"</th>
                        </tr>
                    </thead>
                    <tbody>{game_rows}</tbody>
                </table>
                <p class="browser-notice">{notice_text}</p>
            </div>
            <footer class="browser-footer" on:click=move |_| signals.go_back()>
                <p class="browser-footer-back">{menu_form::BACK_TEXT}</p>
                <p class="browser-footer-clients">{online_clients_text}</p>
            </footer>
        </div>
    }
}

pub fn get_mode_label(mode: GameModeKind) -> &'static str {
    match mode {
        GameModeKind::FreeForAll => "Free for All",
        GameModeKind::Skirmish => "Skirmish",
        GameModeKind::Survival => "Survival",
    }
}

fn render_game_row(signals: AppSignals, game_summary: GameSummary) -> impl IntoView {
    let game_id: GameId = game_summary.game_id;
    let open_join_menu = move |_| {
        signals.navigate(Screen::JoinMenu {
            origin: JoinMenuOrigin::Browser { game_id },
        })
    };
    let open_spectate_menu = move |_| signals.navigate(Screen::SpectateMenu { game_id });

    view! {
        <tr>
            <td class="browser-title">{game_summary.title}</td>
            <td class="browser-mode">{get_mode_label(game_summary.mode)}</td>
            <td>{game_summary.player_count}</td>
            <td>{game_summary.spectator_count}</td>
            <td>{game_summary.player_cap}</td>
            <td class="browser-button" on:click=open_join_menu>"Join"</td>
            <td class="browser-button" on:click=open_spectate_menu>"Spectate"</td>
        </tr>
    }
}
```

In `web/src/app.rs`, replace:

```rust
use shared::settings::SettingsStore;

use crate::connection::ConnectionStatusKind;
use crate::screen::{LobbyNoticeKind, Screen};
```

with:

```rust
use shared::settings::SettingsStore;

use crate::browser::GameListView;
use crate::connection::ConnectionStatusKind;
use crate::screen::{LobbyNoticeKind, Screen};
```

In `web/src/app.rs`, replace:

```rust
    pub connection_status: RwSignal<ConnectionStatusKind>,
    pub lobby_notice: RwSignal<Option<LobbyNoticeKind>>,
    pub own_member_state: RwSignal<Option<OwnMemberStateKind>>,
    pub client_settings: RwSignal<ClientSettings>,
```

with:

```rust
    pub connection_status: RwSignal<ConnectionStatusKind>,
    pub lobby_notice: RwSignal<Option<LobbyNoticeKind>>,
    pub game_list: RwSignal<GameListView>,
    pub own_member_state: RwSignal<Option<OwnMemberStateKind>>,
    pub client_settings: RwSignal<ClientSettings>,
```

In `web/src/app.rs`, replace:

```rust
            connection_status: RwSignal::new(ConnectionStatusKind::Connecting),
            lobby_notice: RwSignal::new(None),
            own_member_state: RwSignal::new(None),
            client_settings: RwSignal::new(ClientSettings::defaults()),
```

with:

```rust
            connection_status: RwSignal::new(ConnectionStatusKind::Connecting),
            lobby_notice: RwSignal::new(None),
            game_list: RwSignal::new(GameListView::empty()),
            own_member_state: RwSignal::new(None),
            client_settings: RwSignal::new(ClientSettings::defaults()),
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p web --lib browser::`

Expected: PASS:

```text
test browser::game_browser::tests::get_mode_label_names_each_mode_as_the_original ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 35 filtered out
```

- [ ] **Step 5: Add the browser styles**

Create `web/style/components/_browser.scss`:

```scss
.browser {
    position: fixed;
    inset: 0;
}

.browser-content {
    height: 100%;
    overflow: auto;
    padding-bottom: 30px;
}

.browser-table {
    width: 100%;
    border-collapse: collapse;
    font-family: var(--font-heading);
    font-size: 15px;

    th {
        height: 32px;
        padding: 8px;
        color: var(--color-white);
        font-size: 16px;
        text-align: center;
        background: var(--color-black);
    }

    tbody {
        opacity: 0.95;
    }

    tbody tr:nth-child(odd) {
        background: var(--color-white);
    }

    tbody tr:nth-child(even) {
        background: var(--color-row-even-browser);
    }

    td {
        height: 24px;
        padding: 5px;
        font-size: 14px;
        text-align: center;
    }
}

.browser-title-heading {
    max-width: 500px;
}

.browser-join-heading {
    font-size: 22px;
}

.browser-title {
    width: 400px;
}

.browser-mode {
    width: 250px;
}

.browser-button {
    min-width: 150px;
    cursor: pointer;

    &:hover {
        background: var(--color-row-even-browser);
    }

    &:active {
        background: var(--color-browser-pressed);
    }
}

.browser-notice {
    padding: 8px;
    color: var(--color-issue);
    font-family: var(--font-heading);
    font-size: 14px;
    text-align: center;
}

.browser-footer {
    position: fixed;
    left: 0;
    bottom: 0;
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
    height: 30px;
    padding: 0 10px;
    color: var(--color-white);
    font-family: var(--font-heading);
    font-size: 14px;
    background: var(--color-black);
    cursor: pointer;
}
```

Replace the whole contents of `web/style/main.scss` with:

```scss
@use "tokens";
@use "reset";
@use "typography";
@use "layout";
@use "components/menu";
@use "components/browser";
```

- [ ] **Step 6: Commit**

```sh
git add web/src/lib.rs
git add web/src/app.rs
git add web/src/browser/mod.rs
git add web/src/browser/game_browser.rs
git add web/style/main.scss
git add web/style/components/_browser.scss
git status --short
git commit -m "game browser"
```

Expected `git status --short` before the commit:

```text
M  web/src/app.rs
A  web/src/browser/game_browser.rs
A  web/src/browser/mod.rs
M  web/src/lib.rs
A  web/style/components/_browser.scss
M  web/style/main.scss
```

### Task 12: Create menu

Game Title, Password, World Shape, World Width and Height (one value: created worlds are square), Player Minimum (srv), Player Cap, Team Count (skm), Leaderboard Length (not skm) and Game Mode. Blank fields take the form defaults, and the settings pass the shared `TryFrom<GameSettingsSerialIn> for GameSettings` before the join step opens; nothing is created until that step is submitted.

**Files:**
- Create: `web/src/menu/create_menu.rs`
- Modify: `web/src/menu/mod.rs`
- Modify: `web/src/app.rs`
- Test: inline `mod tests` in `web/src/menu/create_menu.rs`

- [ ] **Step 1: Write the failing tests**

Replace the whole contents of `web/src/menu/mod.rs` with:

```rust
pub mod create_menu;
pub mod game_menu_view;
pub mod loadout_fields;
pub mod menu_form;

pub use create_menu::*;
pub use game_menu_view::*;
pub use loadout_fields::*;
pub use menu_form::*;
```

Create `web/src/menu/create_menu.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn create_blank_values(mode: GameModeKind) -> CreateMenuValues {
        CreateMenuValues {
            title: String::from("Petri"),
            password: String::new(),
            world_shape: WorldShapeKind::Rectangle,
            world_size: None,
            player_minimum: None,
            player_cap: None,
            team_count: None,
            leaderboard_length: None,
            mode,
        }
    }

    #[test]
    fn resolve_game_settings_fills_blank_fields_with_the_defaults() {
        assert_eq!(
            resolve_game_settings(&create_blank_values(GameModeKind::FreeForAll)),
            GameSettingsSerialIn {
                title: String::from("Petri"),
                mode: GameModeKindSerial::FreeForAll,
                world_shape: WorldShapeKindSerial::Rectangle,
                world_size_pixels: 800,
                player_minimum: None,
                player_cap: 16,
                team_count: None,
                leaderboard_length: 10,
            },
        );
    }

    #[test]
    fn resolve_game_settings_includes_only_the_fields_of_the_mode() {
        let mut skirmish_values: CreateMenuValues = create_blank_values(GameModeKind::Skirmish);
        skirmish_values.player_minimum = Some(6);
        skirmish_values.leaderboard_length = Some(3);
        let mut survival_values: CreateMenuValues = create_blank_values(GameModeKind::Survival);
        survival_values.team_count = Some(3);

        let skirmish_settings: GameSettingsSerialIn = resolve_game_settings(&skirmish_values);
        let survival_settings: GameSettingsSerialIn = resolve_game_settings(&survival_values);

        assert_eq!(skirmish_settings.player_minimum, None);
        assert_eq!(skirmish_settings.team_count, Some(2));
        assert_eq!(skirmish_settings.leaderboard_length, 10);
        assert_eq!(survival_settings.player_minimum, Some(4));
        assert_eq!(survival_settings.team_count, None);
    }

    #[test]
    fn resolve_game_settings_saturates_numbers_outside_the_wire_range() {
        let mut values: CreateMenuValues = create_blank_values(GameModeKind::FreeForAll);
        values.player_cap = Some(-5);
        values.leaderboard_length = Some(1000);
        values.world_size = Some(-1);

        let settings: GameSettingsSerialIn = resolve_game_settings(&values);

        assert_eq!(settings.player_cap, 0);
        assert_eq!(settings.leaderboard_length, 255);
        assert_eq!(settings.world_size_pixels, 0);
    }

    #[test]
    fn validate_create_menu_opens_the_join_step_with_valid_settings() {
        let mut values: CreateMenuValues = create_blank_values(GameModeKind::FreeForAll);
        values.password = String::from("hunter2");

        assert_eq!(
            validate_create_menu(&values),
            Ok(JoinMenuOrigin::Create {
                settings: resolve_game_settings(&values),
                password: Some(String::from("hunter2")),
            }),
        );
    }

    #[test]
    fn validate_create_menu_reports_the_rejected_setting_at_its_field() {
        let mut values: CreateMenuValues = create_blank_values(GameModeKind::FreeForAll);
        values.world_size = Some(200);

        assert_eq!(
            validate_create_menu(&values),
            Err(vec![MenuIssue::at_field(
                MenuFieldKind::WorldWidth,
                "Dimensions must be at least 300 x 300 px",
            )]),
        );
    }

    #[test]
    fn validate_create_menu_reports_a_blank_title_and_a_long_password_together() {
        let mut values: CreateMenuValues = create_blank_values(GameModeKind::Survival);
        values.title = String::from("  ");
        values.password = "p".repeat(protocol_limits::PASSWORD_BYTE_LIMIT + 1);

        assert_eq!(
            validate_create_menu(&values),
            Err(vec![
                MenuIssue::at_field(MenuFieldKind::GameTitle, "Title cannot be left blank"),
                MenuIssue::at_field(MenuFieldKind::Password, "Password can be at most 72 bytes"),
            ]),
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --lib menu::create_menu::`

Expected: FAIL to compile:

```text
error[E0425]: cannot find type `GameModeKind` in this scope
```

- [ ] **Step 3: Implement the create menu and provide its fields**

In `web/src/menu/create_menu.rs`, replace everything above `#[cfg(test)]` with:

```rust
use leptos::prelude::*;

use shared::game::{GameModeKind, GameSettings};
use shared::protocol::protocol_limits;
use shared::protocol::{GameModeKindSerial, GameSettingsSerialIn, RejectionKind, WorldShapeKindSerial};
use shared::world::WorldShapeKind;

use crate::app::AppSignals;
use crate::browser::game_browser;
use crate::menu::{
    FieldAvailabilityKind, FieldVisibilityKind, ListOption, MenuFieldKind, MenuForm, MenuInput, MenuIssue, MenuRow,
};
use crate::screen::{JoinMenuOrigin, Screen};

pub const DEFAULT_WORLD_SIZE_PIXELS: u32 = 800;
pub const DEFAULT_PLAYER_MINIMUM: u8 = 4;
pub const DEFAULT_PLAYER_CAP: u8 = 16;
pub const DEFAULT_TEAM_COUNT: u8 = 2;
pub const DEFAULT_LEADERBOARD_LENGTH: u8 = 10;
const WORLD_SHAPE_KINDS: [WorldShapeKind; 2] = [WorldShapeKind::Rectangle, WorldShapeKind::Ellipse];
const WORLD_SHAPE_LABELS: [&str; 2] = ["Square", "Circle"];
const GAME_MODE_KINDS: [GameModeKind; 3] = [GameModeKind::FreeForAll, GameModeKind::Skirmish, GameModeKind::Survival];
const SURVIVAL_MODES: &[GameModeKind] = &[GameModeKind::Survival];
const SKIRMISH_MODES: &[GameModeKind] = &[GameModeKind::Skirmish];
const RANKED_PLAYER_MODES: &[GameModeKind] = &[GameModeKind::FreeForAll, GameModeKind::Survival];

/// Outlives the create menu: Back from the join step shows the entered values.
#[derive(Clone, Copy, Debug)]
pub struct CreateMenuFields {
    pub title: RwSignal<String>,
    pub password: RwSignal<String>,
    pub world_shape_index: RwSignal<usize>,
    /// World width and height are one value; created worlds are square.
    pub world_size: RwSignal<Option<i64>>,
    pub player_minimum: RwSignal<Option<i64>>,
    pub player_cap: RwSignal<Option<i64>>,
    pub team_count: RwSignal<Option<i64>>,
    pub leaderboard_length: RwSignal<Option<i64>>,
    pub game_mode_index: RwSignal<usize>,
}

impl CreateMenuFields {
    pub fn new() -> CreateMenuFields {
        CreateMenuFields {
            title: RwSignal::new(String::new()),
            password: RwSignal::new(String::new()),
            world_shape_index: RwSignal::new(0),
            world_size: RwSignal::new(None),
            player_minimum: RwSignal::new(None),
            player_cap: RwSignal::new(None),
            team_count: RwSignal::new(None),
            leaderboard_length: RwSignal::new(None),
            game_mode_index: RwSignal::new(0),
        }
    }

    pub fn reset(&self) {
        self.title.set(String::new());
        self.password.set(String::new());
        self.world_shape_index.set(0);
        self.world_size.set(None);
        self.player_minimum.set(None);
        self.player_cap.set(None);
        self.team_count.set(None);
        self.leaderboard_length.set(None);
        self.game_mode_index.set(0);
    }

    pub fn get_values(&self) -> CreateMenuValues {
        CreateMenuValues {
            title: self.title.get_untracked(),
            password: self.password.get_untracked(),
            world_shape: WORLD_SHAPE_KINDS[self.world_shape_index.get_untracked()],
            world_size: self.world_size.get_untracked(),
            player_minimum: self.player_minimum.get_untracked(),
            player_cap: self.player_cap.get_untracked(),
            team_count: self.team_count.get_untracked(),
            leaderboard_length: self.leaderboard_length.get_untracked(),
            mode: GAME_MODE_KINDS[self.game_mode_index.get_untracked()],
        }
    }
}

/// Number fields hold `None` when blank.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateMenuValues {
    pub title: String,
    pub password: String,
    pub world_shape: WorldShapeKind,
    pub world_size: Option<i64>,
    pub player_minimum: Option<i64>,
    pub player_cap: Option<i64>,
    pub team_count: Option<i64>,
    pub leaderboard_length: Option<i64>,
    pub mode: GameModeKind,
}

#[component]
pub fn CreateMenu() -> impl IntoView {
    let signals: AppSignals = expect_context();
    let fields: CreateMenuFields = expect_context();
    let issues: RwSignal<Vec<MenuIssue>> = RwSignal::new(Vec::new());
    let mode: Signal<GameModeKind> = Signal::derive(move || GAME_MODE_KINDS[fields.game_mode_index.get()]);
    let shown_in = move |shown_modes: &'static [GameModeKind]| {
        Signal::derive(move || {
            if shown_modes.contains(&mode.get()) {
                FieldVisibilityKind::Shown
            } else {
                FieldVisibilityKind::Hidden
            }
        })
    };
    let rows: Vec<MenuRow> = vec![
        MenuRow::shown(
            MenuFieldKind::GameTitle,
            "Game Title",
            MenuInput::Text { value: fields.title },
        ),
        MenuRow::shown(
            MenuFieldKind::Password,
            "Password",
            MenuInput::Text { value: fields.password },
        ),
        MenuRow::shown(
            MenuFieldKind::WorldShape,
            "World Shape",
            create_list_input(fields.world_shape_index, &WORLD_SHAPE_LABELS),
        ),
        MenuRow::shown(
            MenuFieldKind::WorldWidth,
            "World Width",
            MenuInput::Number {
                value: fields.world_size,
                placeholder: DEFAULT_WORLD_SIZE_PIXELS,
            },
        ),
        MenuRow::shown(
            MenuFieldKind::WorldHeight,
            "World Height",
            MenuInput::Number {
                value: fields.world_size,
                placeholder: DEFAULT_WORLD_SIZE_PIXELS,
            },
        ),
        MenuRow::shown(
            MenuFieldKind::PlayerMinimum,
            "Player Minimum",
            MenuInput::Number {
                value: fields.player_minimum,
                placeholder: u32::from(DEFAULT_PLAYER_MINIMUM),
            },
        )
        .with_visibility(shown_in(SURVIVAL_MODES)),
        MenuRow::shown(
            MenuFieldKind::PlayerCap,
            "Player Cap",
            MenuInput::Number {
                value: fields.player_cap,
                placeholder: u32::from(DEFAULT_PLAYER_CAP),
            },
        ),
        MenuRow::shown(
            MenuFieldKind::TeamCount,
            "Team Count",
            MenuInput::Number {
                value: fields.team_count,
                placeholder: u32::from(DEFAULT_TEAM_COUNT),
            },
        )
        .with_visibility(shown_in(SKIRMISH_MODES)),
        MenuRow::shown(
            MenuFieldKind::LeaderboardLength,
            "Leaderboard Length",
            MenuInput::Number {
                value: fields.leaderboard_length,
                placeholder: u32::from(DEFAULT_LEADERBOARD_LENGTH),
            },
        )
        .with_visibility(shown_in(RANKED_PLAYER_MODES)),
        MenuRow::shown(
            MenuFieldKind::GameMode,
            "Game Mode",
            create_list_input(
                fields.game_mode_index,
                &GAME_MODE_KINDS.map(game_browser::get_mode_label),
            ),
        ),
    ];
    let on_submit: Callback<()> = Callback::new(move |_| {
        let validated: Result<JoinMenuOrigin, Vec<MenuIssue>> = validate_create_menu(&fields.get_values());

        match validated {
            Ok(origin) => signals.navigate(Screen::JoinMenu { origin }),
            Err(found_issues) => issues.set(found_issues),
        }
    });
    let on_back: Callback<()> = Callback::new(move |_| signals.go_back());

    view! {
        <MenuForm
            header="Game Creation Options"
            submit_label="Create"
            rows=rows
            issues=issues
            on_submit=on_submit
            on_back=on_back
        />
    }
}

/// Blank fields take their defaults; fields the mode hides are left out or take their defaults.
pub fn resolve_game_settings(values: &CreateMenuValues) -> GameSettingsSerialIn {
    let player_minimum: Option<u8> = match values.mode {
        GameModeKind::Survival => Some(get_saturated_u8(values.player_minimum, DEFAULT_PLAYER_MINIMUM)),
        GameModeKind::FreeForAll | GameModeKind::Skirmish => None,
    };
    let team_count: Option<u8> = match values.mode {
        GameModeKind::Skirmish => Some(get_saturated_u8(values.team_count, DEFAULT_TEAM_COUNT)),
        GameModeKind::FreeForAll | GameModeKind::Survival => None,
    };
    let leaderboard_length: u8 = match values.mode {
        GameModeKind::FreeForAll | GameModeKind::Survival => {
            get_saturated_u8(values.leaderboard_length, DEFAULT_LEADERBOARD_LENGTH)
        }
        GameModeKind::Skirmish => DEFAULT_LEADERBOARD_LENGTH,
    };

    GameSettingsSerialIn {
        title: values.title.clone(),
        mode: GameModeKindSerial::from(&values.mode),
        world_shape: WorldShapeKindSerial::from(&values.world_shape),
        world_size_pixels: get_saturated_u32(values.world_size, DEFAULT_WORLD_SIZE_PIXELS),
        player_minimum,
        player_cap: get_saturated_u8(values.player_cap, DEFAULT_PLAYER_CAP),
        team_count,
        leaderboard_length,
    }
}

/// The settings pass the same validation as the server's; the join step follows.
pub fn validate_create_menu(values: &CreateMenuValues) -> Result<JoinMenuOrigin, Vec<MenuIssue>> {
    let settings: GameSettingsSerialIn = resolve_game_settings(values);
    let validated_settings: Result<GameSettings, RejectionKind> = GameSettings::try_from(settings.clone());
    let normalized_password: Result<Option<String>, RejectionKind> =
        protocol_limits::normalize_password(Some(values.password.clone()));

    match (validated_settings, normalized_password) {
        (Ok(_), Ok(password)) => Ok(JoinMenuOrigin::Create { settings, password }),
        (settings_result, password_result) => Err(settings_result
            .err()
            .into_iter()
            .chain(password_result.err())
            .map(MenuIssue::from_rejection)
            .collect()),
    }
}

fn create_list_input(selected_index: RwSignal<usize>, labels: &[&str]) -> MenuInput {
    let options: Vec<ListOption> = labels
        .iter()
        .map(|label| ListOption {
            label: String::from(*label),
            swatch: None,
        })
        .collect();

    MenuInput::List {
        selected_index,
        options: Signal::stored(options),
        availability: Signal::stored(FieldAvailabilityKind::Enabled),
    }
}

/// Out-of-range numbers keep their side of the range, so validation names the right bound.
fn get_saturated_u8(value: Option<i64>, default: u8) -> u8 {
    value.map_or(default, |value| {
        u8::try_from(value.clamp(0, i64::from(u8::MAX))).unwrap_or(u8::MAX)
    })
}

fn get_saturated_u32(value: Option<i64>, default: u32) -> u32 {
    value.map_or(default, |value| {
        u32::try_from(value.clamp(0, i64::from(u32::MAX))).unwrap_or(u32::MAX)
    })
}
```

In `web/src/app.rs`, replace:

```rust
use crate::browser::GameListView;
use crate::connection::ConnectionStatusKind;
use crate::screen::{LobbyNoticeKind, Screen};

#[cfg(feature = "ssr")] // a browser build renders no document head
```

with:

```rust
use crate::browser::GameListView;
use crate::connection::ConnectionStatusKind;
use crate::menu::CreateMenuFields;
use crate::screen::{LobbyNoticeKind, Screen};

#[cfg(feature = "ssr")] // a browser build renders no document head
```

In `web/src/app.rs`, replace:

```rust
    let signals: AppSignals = AppSignals::new();
    provide_context(signals);

    view! { <div class="screen-layer"></div> }
```

with:

```rust
    let signals: AppSignals = AppSignals::new();
    provide_context(signals);
    provide_context(CreateMenuFields::new());

    view! { <div class="screen-layer"></div> }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p web --lib menu::create_menu::`

Expected: PASS:

```text
test menu::create_menu::tests::resolve_game_settings_includes_only_the_fields_of_the_mode ... ok
test menu::create_menu::tests::resolve_game_settings_fills_blank_fields_with_the_defaults ... ok
test menu::create_menu::tests::resolve_game_settings_saturates_numbers_outside_the_wire_range ... ok
test menu::create_menu::tests::validate_create_menu_opens_the_join_step_with_valid_settings ... ok
test menu::create_menu::tests::validate_create_menu_reports_a_blank_title_and_a_long_password_together ... ok
test menu::create_menu::tests::validate_create_menu_reports_the_rejected_setting_at_its_field ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 36 filtered out
```

- [ ] **Step 5: Commit**

```sh
git add web/src/menu/mod.rs
git add web/src/menu/create_menu.rs
git add web/src/app.rs
git status --short
git commit -m "create menu"
```

Expected `git status --short` before the commit:

```text
M  web/src/app.rs
A  web/src/menu/create_menu.rs
M  web/src/menu/mod.rs
```

### Task 13: Join menu

Screen Name, Password (secured games joined from the browser), the loadout rows (Color hidden in skm) and, in skm, the team list preselecting the first smallest team. The create flow's join step sends one `CreateGame`; the browser's sends `JoinGame`. Server rejections land at their fields through `request_rejection`, which `navigate` clears.

**Files:**
- Create: `web/src/menu/join_menu.rs`
- Modify: `web/src/menu/mod.rs`
- Modify: `web/src/menu/menu_form.rs`
- Modify: `web/src/app.rs`
- Test: inline `mod tests` in `web/src/menu/join_menu.rs` and `web/src/app.rs`

- [ ] **Step 1: Write the failing tests**

Replace the whole contents of `web/src/menu/mod.rs` with:

```rust
pub mod create_menu;
pub mod game_menu_view;
pub mod join_menu;
pub mod loadout_fields;
pub mod menu_form;

pub use create_menu::*;
pub use game_menu_view::*;
pub use join_menu::*;
pub use loadout_fields::*;
pub use menu_form::*;
```

Create `web/src/menu/join_menu.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use shared::ability::{FirstAbilityKind, SecondAbilityKind, ThirdAbilityKind};
    use shared::member::{OrganismColorKind, SkinKind, TeamKind};
    use shared::protocol::{
        GameModeKindSerial, GameSettingsSerialIn, TeamChoiceKindSerialIn, TeamKindSerial, WorldShapeKindSerial,
    };

    use crate::menu::AutoAssignKind;

    const GAME_ID: GameId = GameId(3);

    fn create_values() -> LoadoutFieldValues {
        LoadoutFieldValues {
            color: OrganismColorKind::Sun,
            skin: SkinKind::None,
            first: Some(FirstAbilityKind::Extend),
            second: Some(SecondAbilityKind::Freeze),
            third: Some(ThirdAbilityKind::Neutralize),
            team: Some(TeamKind::Red),
            auto_assign: AutoAssignKind::Unselected,
        }
    }

    fn create_settings() -> GameSettingsSerialIn {
        GameSettingsSerialIn {
            title: String::from("Petri"),
            mode: GameModeKindSerial::Skirmish,
            world_shape: WorldShapeKindSerial::Ellipse,
            world_size_pixels: 900,
            player_minimum: None,
            player_cap: 8,
            team_count: Some(3),
            leaderboard_length: 10,
        }
    }

    fn create_game_summary() -> GameSummary {
        GameSummary {
            game_id: GAME_ID,
            title: String::from("Agar"),
            mode: GameModeKind::Skirmish,
            player_count: 3,
            spectator_count: 1,
            player_cap: 8,
            secured: true,
            team_sizes: vec![2, 1],
        }
    }

    fn create_expected_joiner(screen_name: &str) -> JoinerSerialIn {
        JoinerSerialIn {
            screen_name: String::from(screen_name),
            loadout: LoadoutSerial::from(&create_values().to_loadout().unwrap()),
            team: TeamChoiceKindSerialIn::Team(TeamKindSerial::Red),
        }
    }

    #[test]
    fn from_origin_describes_a_game_being_created() {
        let origin: JoinMenuOrigin = JoinMenuOrigin::Create {
            settings: create_settings(),
            password: Some(String::from("hunter2")),
        };

        let target: JoinTarget = JoinTarget::from_origin(&origin, &[]).unwrap();

        assert_eq!(target.mode, GameModeKind::Skirmish);
        assert_eq!(target.password_visibility, FieldVisibilityKind::Hidden);
        assert_eq!(target.team_sizes, get_empty_team_sizes(3));
    }

    #[test]
    fn from_origin_describes_a_listed_game_from_its_summary() {
        let origin: JoinMenuOrigin = JoinMenuOrigin::Browser { game_id: GAME_ID };

        let target: JoinTarget = JoinTarget::from_origin(&origin, &[create_game_summary()]).unwrap();

        assert_eq!(target.mode, GameModeKind::Skirmish);
        assert_eq!(target.password_visibility, FieldVisibilityKind::Shown);
        assert_eq!(target.team_sizes, loadout_fields::get_summary_team_sizes(&[2, 1]));
    }

    #[test]
    fn from_origin_is_none_for_a_game_no_longer_listed() {
        let origin: JoinMenuOrigin = JoinMenuOrigin::Browser { game_id: GameId(4) };

        assert_eq!(JoinTarget::from_origin(&origin, &[create_game_summary()]), None);
    }

    #[test]
    fn build_join_request_creates_the_game_with_the_create_menu_password() {
        let origin: JoinMenuOrigin = JoinMenuOrigin::Create {
            settings: create_settings(),
            password: Some(String::from("hunter2")),
        };

        assert_eq!(
            build_join_request(&origin, "  Blob ", "", &create_values()),
            Ok(MessageSerialIn::CreateGame {
                settings: create_settings(),
                password: Some(String::from("hunter2")),
                joiner: create_expected_joiner("Blob"),
            }),
        );
    }

    #[test]
    fn build_join_request_joins_a_listed_game_with_the_entered_password() {
        let origin: JoinMenuOrigin = JoinMenuOrigin::Browser { game_id: GAME_ID };

        assert_eq!(
            build_join_request(&origin, "Blob", "swordfish", &create_values()),
            Ok(MessageSerialIn::JoinGame {
                game_id: GAME_ID.0,
                password: Some(String::from("swordfish")),
                joiner: create_expected_joiner("Blob"),
            }),
        );
    }

    #[test]
    fn build_join_request_reports_the_screen_name_and_every_missing_ability() {
        let origin: JoinMenuOrigin = JoinMenuOrigin::Browser { game_id: GAME_ID };
        let values: LoadoutFieldValues = LoadoutFieldValues::join_defaults(&[]);

        assert_eq!(
            build_join_request(&origin, " ", "", &values),
            Err(vec![
                MenuIssue::at_field(MenuFieldKind::ScreenName, "Screen name cannot be left empty"),
                MenuIssue::at_field(MenuFieldKind::FirstAbility, "Select a first ability"),
                MenuIssue::at_field(MenuFieldKind::SecondAbility, "Select a second ability"),
                MenuIssue::at_field(MenuFieldKind::ThirdAbility, "Select a third ability"),
            ]),
        );
    }

    #[test]
    fn find_game_summary_finds_the_listed_game_by_id() {
        let game_summaries: Vec<GameSummary> = vec![create_game_summary()];

        assert_eq!(find_game_summary(&game_summaries, GAME_ID), Some(&game_summaries[0]));
        assert_eq!(find_game_summary(&game_summaries, GameId(9)), None);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --lib menu::join_menu::`

Expected: FAIL to compile:

```text
error[E0425]: cannot find type `GameId` in this scope
```

- [ ] **Step 3: Implement the join menu, the rejection and team choice signals, and a clearing `navigate`**

In `web/src/menu/join_menu.rs`, replace everything above `#[cfg(test)]` with:

```rust
use leptos::prelude::*;

use shared::ability::Loadout;
use shared::game::{GameId, GameModeKind, GameSummary};
use shared::member;
use shared::protocol::protocol_limits;
use shared::protocol::{JoinerSerialIn, LoadoutSerial, MessageSerialIn, RejectionKind, RequestKind};

use crate::app::AppSignals;
use crate::canvas::DriverCommandKind;
use crate::canvas::game_canvas;
use crate::menu::loadout_fields;
use crate::menu::menu_form;
use crate::menu::{
    FieldVisibilityKind, LoadoutFieldValues, LoadoutFields, MenuFieldKind, MenuForm, MenuInput, MenuIssue, MenuRow,
    TeamSize,
};
use crate::screen::JoinMenuOrigin;

/// What the join step knows of the game it joins or creates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinTarget {
    pub mode: GameModeKind,
    /// Shown only for a secured listed game; the create flow carries the create menu's password.
    pub password_visibility: FieldVisibilityKind,
    pub team_sizes: Vec<TeamSize>,
}

impl JoinTarget {
    /// `None` for a listed game which is no longer listed.
    pub fn from_origin(origin: &JoinMenuOrigin, game_summaries: &[GameSummary]) -> Option<JoinTarget> {
        match origin {
            JoinMenuOrigin::Create { settings, .. } => Some(JoinTarget {
                mode: GameModeKind::from(settings.mode),
                password_visibility: FieldVisibilityKind::Hidden,
                team_sizes: get_empty_team_sizes(settings.team_count.unwrap_or(0)),
            }),
            JoinMenuOrigin::Browser { game_id } => {
                let game_summary: &GameSummary = find_game_summary(game_summaries, *game_id)?;

                Some(JoinTarget {
                    mode: game_summary.mode,
                    password_visibility: get_password_visibility(game_summary),
                    team_sizes: loadout_fields::get_summary_team_sizes(&game_summary.team_sizes),
                })
            }
        }
    }
}

#[component]
pub fn JoinMenu(origin: JoinMenuOrigin) -> impl IntoView {
    let signals: AppSignals = expect_context();
    let issues: RwSignal<Vec<MenuIssue>> = RwSignal::new(Vec::new());
    let screen_name: RwSignal<String> = RwSignal::new(String::new());
    let password: RwSignal<String> = RwSignal::new(String::new());
    let target_origin: JoinMenuOrigin = origin.clone();
    let target: Memo<Option<JoinTarget>> = Memo::new(move |_| {
        signals
            .game_list
            .with(|game_list| JoinTarget::from_origin(&target_origin, &game_list.game_summaries))
    });
    let mode: Signal<GameModeKind> =
        Signal::derive(move || target.get().map_or(GameModeKind::FreeForAll, |target| target.mode));
    let team_sizes: Signal<Vec<TeamSize>> =
        Signal::derive(move || target.get().map(|target| target.team_sizes).unwrap_or_default());
    let password_visibility: Signal<FieldVisibilityKind> =
        Signal::derive(move || target.get().map_or(FieldVisibilityKind::Hidden, |target| target.password_visibility));
    let initial_values: LoadoutFieldValues = LoadoutFieldValues::join_defaults(&team_sizes.get_untracked());
    let loadout_fields: LoadoutFields = LoadoutFields::new(&initial_values, team_sizes);
    let mut rows: Vec<MenuRow> = vec![
        MenuRow::shown(
            MenuFieldKind::ScreenName,
            "Screen Name",
            MenuInput::Text { value: screen_name },
        ),
        MenuRow::shown(MenuFieldKind::Password, "Password", MenuInput::Text { value: password })
            .with_visibility(password_visibility),
    ];
    rows.extend(loadout_fields::create_appearance_rows(loadout_fields, mode));
    rows.extend(loadout_fields::create_ability_rows(loadout_fields));
    rows.extend(loadout_fields::create_team_rows(loadout_fields, mode, team_sizes));

    menu_form::show_request_rejections(
        signals.request_rejection,
        &[RequestKind::CreateGame, RequestKind::JoinGame],
        issues,
    );

    Effect::new(move |_| {
        if target.get().is_none() {
            issues.set(vec![MenuIssue::from_rejection(RejectionKind::GameNotFound)]);
        }
    });

    let on_submit: Callback<()> = Callback::new(move |_| {
        let values: LoadoutFieldValues = loadout_fields.get_values();
        let built_request: Result<MessageSerialIn, Vec<MenuIssue>> = build_join_request(
            &origin,
            &screen_name.get_untracked(),
            &password.get_untracked(),
            &values,
        );

        match built_request {
            Ok(request) => {
                issues.set(Vec::new());
                signals.team_choice.set(values.to_team_choice());
                game_canvas::dispatch_driver_command(DriverCommandKind::SendRequest(request));
            }
            Err(found_issues) => issues.set(found_issues),
        }
    });
    let on_back: Callback<()> = Callback::new(move |_| signals.go_back());

    view! {
        <MenuForm
            header="Join Game Options"
            submit_label="Join"
            rows=rows
            issues=issues
            on_submit=on_submit
            on_back=on_back
        />
    }
}

/// `CreateGame` from the create flow, `JoinGame` from the browser.
pub fn build_join_request(
    origin: &JoinMenuOrigin,
    screen_name: &str,
    password: &str,
    values: &LoadoutFieldValues,
) -> Result<MessageSerialIn, Vec<MenuIssue>> {
    let credentials: Result<(String, Option<String>), Vec<MenuIssue>> = normalize_credentials(screen_name, password);
    let loadout: Result<Loadout, Vec<MenuIssue>> = values.to_loadout();

    match (credentials, loadout) {
        (Ok((screen_name, password)), Ok(loadout)) => {
            let joiner: JoinerSerialIn = JoinerSerialIn {
                screen_name,
                loadout: LoadoutSerial::from(&loadout),
                team: values.to_team_choice(),
            };

            Ok(create_join_request(origin, password, joiner))
        }
        (credentials_result, loadout_result) => {
            let mut issues: Vec<MenuIssue> = credentials_result.err().unwrap_or_default();
            issues.extend(loadout_result.err().unwrap_or_default());

            Err(issues)
        }
    }
}

/// The trimmed screen name and the password, or an issue for each one which is invalid.
pub fn normalize_credentials(screen_name: &str, password: &str) -> Result<(String, Option<String>), Vec<MenuIssue>> {
    let normalized_screen_name: Result<String, RejectionKind> = protocol_limits::normalize_screen_name(screen_name);
    let normalized_password: Result<Option<String>, RejectionKind> =
        protocol_limits::normalize_password(Some(String::from(password)));

    match (normalized_screen_name, normalized_password) {
        (Ok(screen_name), Ok(password)) => Ok((screen_name, password)),
        (screen_name_result, password_result) => Err(screen_name_result
            .err()
            .into_iter()
            .chain(password_result.err())
            .map(MenuIssue::from_rejection)
            .collect()),
    }
}

pub fn find_game_summary(game_summaries: &[GameSummary], game_id: GameId) -> Option<&GameSummary> {
    game_summaries.iter().find(|game_summary| game_summary.game_id == game_id)
}

pub fn get_password_visibility(game_summary: &GameSummary) -> FieldVisibilityKind {
    if game_summary.secured {
        FieldVisibilityKind::Shown
    } else {
        FieldVisibilityKind::Hidden
    }
}

/// `password` is the join menu's; the create flow carries the create menu's.
fn create_join_request(origin: &JoinMenuOrigin, password: Option<String>, joiner: JoinerSerialIn) -> MessageSerialIn {
    match origin {
        JoinMenuOrigin::Create {
            settings,
            password: created_password,
        } => MessageSerialIn::CreateGame {
            settings: settings.clone(),
            password: created_password.clone(),
            joiner,
        },
        JoinMenuOrigin::Browser { game_id } => MessageSerialIn::JoinGame {
            game_id: game_id.0,
            password,
            joiner,
        },
    }
}

fn get_empty_team_sizes(team_count: u8) -> Vec<TeamSize> {
    member::get_game_teams(team_count)
        .into_iter()
        .map(|team| TeamSize {
            team,
            participant_count: 0,
        })
        .collect()
}
```

In `web/src/menu/menu_form.rs`, replace:

```rust
fn get_setting_field(setting_field: SettingFieldKind) -> MenuFieldKind {
```

with:

```rust
pub fn show_request_rejections(
    request_rejection: RwSignal<Option<RequestRejection>>,
    answered_requests: &'static [RequestKind],
    issues: RwSignal<Vec<MenuIssue>>,
) {
    Effect::new(move |_| {
        let rejection: Option<RequestRejection> = request_rejection.get();

        if let Some(rejection) = rejection
            && answered_requests.contains(&rejection.request)
        {
            issues.set(vec![MenuIssue::from_rejection(rejection.reason)]);
        }
    });
}

fn get_setting_field(setting_field: SettingFieldKind) -> MenuFieldKind {
```

In `web/src/app.rs`, replace:

```rust
use leptos::prelude::*;

use shared::play::OwnMemberStateKind;
use shared::settings;
use shared::settings::SettingsStore;
```

with:

```rust
use leptos::prelude::*;

use shared::play::OwnMemberStateKind;
use shared::protocol::TeamChoiceKindSerialIn;
use shared::settings;
use shared::settings::SettingsStore;
```

In `web/src/app.rs`, replace:

```rust
use crate::browser::GameListView;
use crate::connection::ConnectionStatusKind;
use crate::menu::CreateMenuFields;
use crate::screen::{LobbyNoticeKind, Screen};

#[cfg(feature = "ssr")] // a browser build renders no document head
```

with:

```rust
use crate::browser::GameListView;
use crate::connection::ConnectionStatusKind;
use crate::menu::{CreateMenuFields, RequestRejection};
use crate::screen::{LobbyNoticeKind, Screen};

#[cfg(feature = "ssr")] // a browser build renders no document head
```

In `web/src/app.rs`, replace:

```rust
    pub lobby_notice: RwSignal<Option<LobbyNoticeKind>>,
    pub game_list: RwSignal<GameListView>,
    pub own_member_state: RwSignal<Option<OwnMemberStateKind>>,
    pub client_settings: RwSignal<ClientSettings>,
}

impl AppSignals {
```

with:

```rust
    pub lobby_notice: RwSignal<Option<LobbyNoticeKind>>,
    pub game_list: RwSignal<GameListView>,
    pub request_rejection: RwSignal<Option<RequestRejection>>,
    pub own_member_state: RwSignal<Option<OwnMemberStateKind>>,
    pub client_settings: RwSignal<ClientSettings>,
    /// The team choice last sent in a join or respawn.
    pub team_choice: RwSignal<TeamChoiceKindSerialIn>,
}

impl AppSignals {
```

In `web/src/app.rs`, replace:

```rust
            lobby_notice: RwSignal::new(None),
            game_list: RwSignal::new(GameListView::empty()),
            own_member_state: RwSignal::new(None),
            client_settings: RwSignal::new(ClientSettings::defaults()),
        }
    }
```

with:

```rust
            lobby_notice: RwSignal::new(None),
            game_list: RwSignal::new(GameListView::empty()),
            request_rejection: RwSignal::new(None),
            own_member_state: RwSignal::new(None),
            client_settings: RwSignal::new(ClientSettings::defaults()),
            team_choice: RwSignal::new(TeamChoiceKindSerialIn::Auto),
        }
    }
```

In `web/src/app.rs`, replace:

```rust
    }

    pub fn navigate(&self, screen: Screen) {
        self.screen.set(screen);
    }
```

with:

```rust
    }

    /// A rejection belongs to the screen whose request it answers.
    pub fn navigate(&self, screen: Screen) {
        self.request_rejection.set(None);
        self.screen.set(screen);
    }
```

In `web/src/app.rs`, replace:

```rust
    use std::collections::BTreeMap;

    use shared::settings::{SettingKey, SettingValue};

    /// An in-memory store, as a platform's store behaves.
```

with:

```rust
    use std::collections::BTreeMap;

    use shared::protocol::{RejectionKind, RequestKind};
    use shared::settings::{SettingKey, SettingValue};

    /// An in-memory store, as a platform's store behaves.
```

In `web/src/app.rs`, replace:

```rust
    }

    #[test]
    fn go_back_opens_the_back_screen() {
        let owner: Owner = Owner::new();
        owner.set();
```

with:

```rust
    }

    #[test]
    fn go_back_opens_the_back_screen_and_clears_the_rejection() {
        let owner: Owner = Owner::new();
        owner.set();
```

In `web/src/app.rs`, replace:

```rust
        let signals: AppSignals = AppSignals::new();
        signals.screen.set(Screen::Browser);

        signals.go_back();
```

with:

```rust
        let signals: AppSignals = AppSignals::new();
        signals.screen.set(Screen::Browser);
        signals.request_rejection.set(Some(RequestRejection {
            request: RequestKind::JoinGame,
            reason: RejectionKind::GameFull,
        }));

        signals.go_back();
```

In `web/src/app.rs`, replace:

```rust
        signals.go_back();

        assert_eq!(signals.screen.get_untracked(), Screen::Title);
    }
}
```

with:

```rust
        signals.go_back();

        assert_eq!(signals.screen.get_untracked(), Screen::Title);
        assert_eq!(signals.request_rejection.get_untracked(), None);
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p web --lib -- menu::join_menu:: app::`

Expected: PASS:

```text
test app::tests::read_gives_the_defaults_from_an_empty_store ... ok
test app::tests::read_gives_back_what_write_stored ... ok
test menu::join_menu::tests::build_join_request_joins_a_listed_game_with_the_entered_password ... ok
test menu::join_menu::tests::build_join_request_creates_the_game_with_the_create_menu_password ... ok
test app::tests::go_back_opens_the_back_screen_and_clears_the_rejection ... ok
test menu::join_menu::tests::from_origin_is_none_for_a_game_no_longer_listed ... ok
test menu::join_menu::tests::from_origin_describes_a_game_being_created ... ok
test menu::join_menu::tests::find_game_summary_finds_the_listed_game_by_id ... ok
test menu::join_menu::tests::from_origin_describes_a_listed_game_from_its_summary ... ok
test menu::join_menu::tests::build_join_request_reports_the_screen_name_and_every_missing_ability ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 39 filtered out
```

- [ ] **Step 5: Commit**

```sh
git add web/src/menu/mod.rs
git add web/src/menu/join_menu.rs
git add web/src/menu/menu_form.rs
git add web/src/app.rs
git status --short
git commit -m "join menu"
```

Expected `git status --short` before the commit:

```text
M  web/src/app.rs
A  web/src/menu/join_menu.rs
M  web/src/menu/menu_form.rs
M  web/src/menu/mod.rs
```

### Task 14: Spectate menu

**Files:**
- Create: `web/src/menu/spectate_menu.rs`
- Modify: `web/src/menu/mod.rs`
- Test: inline `mod tests` in `web/src/menu/spectate_menu.rs`

- [ ] **Step 1: Write the failing tests**

Replace the whole contents of `web/src/menu/mod.rs` with:

```rust
pub mod create_menu;
pub mod game_menu_view;
pub mod join_menu;
pub mod loadout_fields;
pub mod menu_form;
pub mod spectate_menu;

pub use create_menu::*;
pub use game_menu_view::*;
pub use join_menu::*;
pub use loadout_fields::*;
pub use menu_form::*;
pub use spectate_menu::*;
```

Create `web/src/menu/spectate_menu.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const GAME_ID: GameId = GameId(5);

    #[test]
    fn build_spectate_request_spectates_with_the_trimmed_screen_name() {
        assert_eq!(
            build_spectate_request(GAME_ID, " Watcher ", ""),
            Ok(MessageSerialIn::SpectateGame {
                game_id: GAME_ID.0,
                password: None,
                screen_name: String::from("Watcher"),
            }),
        );
    }

    #[test]
    fn build_spectate_request_reports_an_empty_screen_name() {
        assert_eq!(
            build_spectate_request(GAME_ID, "", "swordfish"),
            Err(vec![MenuIssue::at_field(
                MenuFieldKind::ScreenName,
                "Screen name cannot be left empty",
            )]),
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --lib menu::spectate_menu::`

Expected: FAIL to compile:

```text
error[E0425]: cannot find type `GameId` in this scope
```

- [ ] **Step 3: Implement the spectate menu**

In `web/src/menu/spectate_menu.rs`, replace everything above `#[cfg(test)]` with:

```rust
use leptos::prelude::*;

use shared::game::GameId;
use shared::protocol::{MessageSerialIn, RequestKind};

use crate::app::AppSignals;
use crate::canvas::DriverCommandKind;
use crate::canvas::game_canvas;
use crate::menu::join_menu;
use crate::menu::menu_form;
use crate::menu::{FieldVisibilityKind, MenuFieldKind, MenuForm, MenuInput, MenuIssue, MenuRow};

#[component]
pub fn SpectateMenu(game_id: GameId) -> impl IntoView {
    let signals: AppSignals = expect_context();
    let issues: RwSignal<Vec<MenuIssue>> = RwSignal::new(Vec::new());
    let screen_name: RwSignal<String> = RwSignal::new(String::new());
    let password: RwSignal<String> = RwSignal::new(String::new());
    let password_visibility: Signal<FieldVisibilityKind> = Signal::derive(move || {
        signals.game_list.with(|game_list| {
            join_menu::find_game_summary(&game_list.game_summaries, game_id)
                .map_or(FieldVisibilityKind::Hidden, join_menu::get_password_visibility)
        })
    });
    let rows: Vec<MenuRow> = vec![
        MenuRow::shown(
            MenuFieldKind::ScreenName,
            "Screen Name",
            MenuInput::Text { value: screen_name },
        ),
        MenuRow::shown(MenuFieldKind::Password, "Password", MenuInput::Text { value: password })
            .with_visibility(password_visibility),
    ];

    menu_form::show_request_rejections(signals.request_rejection, &[RequestKind::SpectateGame], issues);

    let on_submit: Callback<()> = Callback::new(move |_| {
        let built_request: Result<MessageSerialIn, Vec<MenuIssue>> =
            build_spectate_request(game_id, &screen_name.get_untracked(), &password.get_untracked());

        match built_request {
            Ok(request) => {
                issues.set(Vec::new());
                game_canvas::dispatch_driver_command(DriverCommandKind::SendRequest(request));
            }
            Err(found_issues) => issues.set(found_issues),
        }
    });
    let on_back: Callback<()> = Callback::new(move |_| signals.go_back());

    view! {
        <MenuForm
            header="Spectate Game Options"
            submit_label="Spectate"
            rows=rows
            issues=issues
            on_submit=on_submit
            on_back=on_back
        />
    }
}

pub fn build_spectate_request(
    game_id: GameId,
    screen_name: &str,
    password: &str,
) -> Result<MessageSerialIn, Vec<MenuIssue>> {
    join_menu::normalize_credentials(screen_name, password).map(|(screen_name, password)| {
        MessageSerialIn::SpectateGame {
            game_id: game_id.0,
            password,
            screen_name,
        }
    })
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p web --lib menu::spectate_menu::`

Expected: PASS:

```text
test menu::spectate_menu::tests::build_spectate_request_spectates_with_the_trimmed_screen_name ... ok
test menu::spectate_menu::tests::build_spectate_request_reports_an_empty_screen_name ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 49 filtered out
```

- [ ] **Step 5: Commit**

```sh
git add web/src/menu/mod.rs
git add web/src/menu/spectate_menu.rs
git status --short
git commit -m "spectate menu"
```

Expected `git status --short` before the commit:

```text
M  web/src/menu/mod.rs
A  web/src/menu/spectate_menu.rs
```

### Task 15: Respawn menu

Prefilled from the member's loadout and team (a Participant) or the join defaults (a Spectator), Auto Assign as last sent; Spawn sends `Respawn`, Leave Game sends `LeaveGame`.

**Files:**
- Create: `web/src/menu/respawn_menu.rs`
- Modify: `web/src/menu/mod.rs`
- Modify: `web/src/menu/loadout_fields.rs`
- Modify: `web/src/menu/game_menu_view.rs`
- Modify: `web/src/app.rs`
- Test: inline `mod tests` in `web/src/menu/respawn_menu.rs`

- [ ] **Step 1: Write the failing tests**

Replace the whole contents of `web/src/menu/mod.rs` with:

```rust
pub mod create_menu;
pub mod game_menu_view;
pub mod join_menu;
pub mod loadout_fields;
pub mod menu_form;
pub mod respawn_menu;
pub mod spectate_menu;

pub use create_menu::*;
pub use game_menu_view::*;
pub use join_menu::*;
pub use loadout_fields::*;
pub use menu_form::*;
pub use respawn_menu::*;
pub use spectate_menu::*;
```

Create `web/src/menu/respawn_menu.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use shared::protocol::TeamKindSerial;

    use crate::test_fixture;

    fn create_game_menu(own_loadout: Option<Loadout>, own_team: Option<TeamKind>) -> GameMenuView {
        GameMenuView {
            mode: GameModeKind::Skirmish,
            own_loadout,
            own_team,
            team_sizes: vec![
                TeamSize {
                    team: TeamKind::Red,
                    participant_count: 2,
                },
                TeamSize {
                    team: TeamKind::Blue,
                    participant_count: 1,
                },
            ],
        }
    }

    #[test]
    fn get_respawn_values_prefill_a_participant_from_its_loadout_and_team() {
        let game_menu: GameMenuView = create_game_menu(Some(test_fixture::create_loadout()), Some(TeamKind::Red));

        let values: LoadoutFieldValues =
            get_respawn_values(Some(&game_menu), TeamChoiceKindSerialIn::Team(TeamKindSerial::Red));

        assert_eq!(
            values,
            LoadoutFieldValues::from_loadout(
                &test_fixture::create_loadout(),
                Some(TeamKind::Red),
                AutoAssignKind::Unselected,
            ),
        );
    }

    #[test]
    fn get_respawn_values_keep_auto_assign_when_it_was_chosen() {
        let game_menu: GameMenuView = create_game_menu(Some(test_fixture::create_loadout()), Some(TeamKind::Red));

        let values: LoadoutFieldValues = get_respawn_values(Some(&game_menu), TeamChoiceKindSerialIn::Auto);

        assert_eq!(values.auto_assign, AutoAssignKind::Selected);
    }

    #[test]
    fn get_respawn_values_give_a_spectator_the_join_defaults() {
        let game_menu: GameMenuView = create_game_menu(None, None);

        let values: LoadoutFieldValues = get_respawn_values(Some(&game_menu), TeamChoiceKindSerialIn::Auto);

        assert_eq!(values, LoadoutFieldValues::join_defaults(&game_menu.team_sizes));
        assert_eq!(values.team, Some(TeamKind::Blue));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --lib menu::respawn_menu::`

Expected: FAIL to compile:

```text
error[E0425]: cannot find type `Loadout` in this scope
```

- [ ] **Step 3: Implement the respawn menu and the `game_menu` signal**

In `web/src/menu/respawn_menu.rs`, replace everything above `#[cfg(test)]` with:

```rust
use leptos::prelude::*;

use shared::ability::Loadout;
use shared::game::GameModeKind;
use shared::member::TeamKind;
use shared::protocol::{LoadoutSerial, MessageSerialIn, RequestKind, TeamChoiceKindSerialIn};

use crate::app::AppSignals;
use crate::canvas::DriverCommandKind;
use crate::canvas::game_canvas;
use crate::menu::game_menu_view;
use crate::menu::loadout_fields;
use crate::menu::menu_form;
use crate::menu::{
    AutoAssignKind, GameMenuView, LoadoutFieldValues, LoadoutFields, MenuForm, MenuIssue, MenuRow, TeamSize,
};

#[component]
pub fn RespawnMenu() -> impl IntoView {
    let signals: AppSignals = expect_context();
    let issues: RwSignal<Vec<MenuIssue>> = RwSignal::new(Vec::new());
    let mode: Signal<GameModeKind> = game_menu_view::get_game_menu_mode(signals.game_menu);
    let team_sizes: Signal<Vec<TeamSize>> = Signal::derive(move || {
        signals.game_menu.with(|game_menu| {
            game_menu.as_ref().map(|game_menu_view| game_menu_view.team_sizes.clone()).unwrap_or_default()
        })
    });
    let initial_values: LoadoutFieldValues = get_respawn_values(
        signals.game_menu.get_untracked().as_ref(),
        signals.team_choice.get_untracked(),
    );
    let loadout_fields: LoadoutFields = LoadoutFields::new(&initial_values, team_sizes);
    let mut rows: Vec<MenuRow> = loadout_fields::create_appearance_rows(loadout_fields, mode);
    rows.extend(loadout_fields::create_ability_rows(loadout_fields));
    rows.extend(loadout_fields::create_team_rows(loadout_fields, mode, team_sizes));
    rows.push(loadout_fields::create_leave_game_row());

    menu_form::show_request_rejections(signals.request_rejection, &[RequestKind::Respawn], issues);

    let on_submit: Callback<()> = Callback::new(move |_| {
        let values: LoadoutFieldValues = loadout_fields.get_values();
        let loadout: Result<Loadout, Vec<MenuIssue>> = values.to_loadout();

        match loadout {
            Ok(loadout) => {
                issues.set(Vec::new());
                signals.team_choice.set(values.to_team_choice());
                game_canvas::dispatch_driver_command(DriverCommandKind::SendRequest(MessageSerialIn::Respawn {
                    loadout: LoadoutSerial::from(&loadout),
                    team: values.to_team_choice(),
                }));
            }
            Err(found_issues) => issues.set(found_issues),
        }
    });
    let on_back: Callback<()> = Callback::new(move |_| signals.go_back());

    view! {
        <MenuForm
            header="Spawn Options"
            submit_label="Spawn"
            rows=rows
            issues=issues
            on_submit=on_submit
            on_back=on_back
        />
    }
}

/// A Participant respawns as it last played; a Spectator starts from the join defaults.
pub fn get_respawn_values(
    game_menu: Option<&GameMenuView>,
    last_team_choice: TeamChoiceKindSerialIn,
) -> LoadoutFieldValues {
    let team_sizes: &[TeamSize] = game_menu.map_or(&[], |game_menu| game_menu.team_sizes.as_slice());
    let auto_assign: AutoAssignKind = match last_team_choice {
        TeamChoiceKindSerialIn::Auto => AutoAssignKind::Selected,
        TeamChoiceKindSerialIn::Team(_) => AutoAssignKind::Unselected,
    };
    let own_loadout: Option<&Loadout> = game_menu.and_then(|game_menu| game_menu.own_loadout.as_ref());

    match own_loadout {
        Some(own_loadout) => {
            let team: Option<TeamKind> = game_menu
                .and_then(|game_menu| game_menu.own_team)
                .or_else(|| loadout_fields::get_smallest_team(team_sizes));

            LoadoutFieldValues::from_loadout(own_loadout, team, auto_assign)
        }
        None => LoadoutFieldValues::join_defaults(team_sizes),
    }
}
```

In `web/src/menu/loadout_fields.rs`, replace:

```rust
use shared::protocol::{TeamChoiceKindSerialIn, TeamKindSerial};
use shared::render::RenderColor;
use shared::render::render_color;

use crate::menu::{
```

with:

```rust
use shared::protocol::{MessageSerialIn, TeamChoiceKindSerialIn, TeamKindSerial};
use shared::render::RenderColor;
use shared::render::render_color;

use crate::canvas::DriverCommandKind;
use crate::canvas::game_canvas;
use crate::menu::{
```

In `web/src/menu/loadout_fields.rs`, replace:

```rust
pub fn create_radios_input(
```

with:

```rust
pub fn create_leave_game_row() -> MenuRow {
    MenuRow::shown(
        MenuFieldKind::LeaveGame,
        "Leave Game",
        MenuInput::Button {
            on_click: Callback::new(move |_| {
                game_canvas::dispatch_driver_command(DriverCommandKind::SendRequest(MessageSerialIn::LeaveGame));
            }),
        },
    )
}

pub fn create_radios_input(
```

In `web/src/menu/game_menu_view.rs`, replace:

```rust
use shared::ability::Loadout;
use shared::game::{GameModeKind, GameState};
```

with:

```rust
use leptos::prelude::*;

use shared::ability::Loadout;
use shared::game::{GameModeKind, GameState};
```

In `web/src/menu/game_menu_view.rs`, replace:

```rust
            team_sizes: loadout_fields::get_state_team_sizes(state),
        })
    }
}
```

with:

```rust
            team_sizes: loadout_fields::get_state_team_sizes(state),
        })
    }
}

pub fn get_game_menu_mode(game_menu: RwSignal<Option<GameMenuView>>) -> Signal<GameModeKind> {
    Signal::derive(move || {
        game_menu.with(|game_menu| {
            game_menu
                .as_ref()
                .map_or(GameModeKind::FreeForAll, |game_menu_view| game_menu_view.mode)
        })
    })
}
```

In `web/src/app.rs`, replace:

```rust
use crate::browser::GameListView;
use crate::connection::ConnectionStatusKind;
use crate::menu::{CreateMenuFields, RequestRejection};
use crate::screen::{LobbyNoticeKind, Screen};

#[cfg(feature = "ssr")] // a browser build renders no document head
```

with:

```rust
use crate::browser::GameListView;
use crate::connection::ConnectionStatusKind;
use crate::menu::{CreateMenuFields, GameMenuView, RequestRejection};
use crate::screen::{LobbyNoticeKind, Screen};

#[cfg(feature = "ssr")] // a browser build renders no document head
```

In `web/src/app.rs`, replace:

```rust
    pub request_rejection: RwSignal<Option<RequestRejection>>,
    pub own_member_state: RwSignal<Option<OwnMemberStateKind>>,
    pub client_settings: RwSignal<ClientSettings>,
    /// The team choice last sent in a join or respawn.
```

with:

```rust
    pub request_rejection: RwSignal<Option<RequestRejection>>,
    pub own_member_state: RwSignal<Option<OwnMemberStateKind>>,
    pub game_menu: RwSignal<Option<GameMenuView>>,
    pub client_settings: RwSignal<ClientSettings>,
    /// The team choice last sent in a join or respawn.
```

In `web/src/app.rs`, replace:

```rust
            request_rejection: RwSignal::new(None),
            own_member_state: RwSignal::new(None),
            client_settings: RwSignal::new(ClientSettings::defaults()),
            team_choice: RwSignal::new(TeamChoiceKindSerialIn::Auto),
```

with:

```rust
            request_rejection: RwSignal::new(None),
            own_member_state: RwSignal::new(None),
            game_menu: RwSignal::new(None),
            client_settings: RwSignal::new(ClientSettings::defaults()),
            team_choice: RwSignal::new(TeamChoiceKindSerialIn::Auto),
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p web --lib menu::respawn_menu::`

Expected: PASS:

```text
test menu::respawn_menu::tests::get_respawn_values_prefill_a_participant_from_its_loadout_and_team ... ok
test menu::respawn_menu::tests::get_respawn_values_give_a_spectator_the_join_defaults ... ok
test menu::respawn_menu::tests::get_respawn_values_keep_auto_assign_when_it_was_chosen ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 51 filtered out
```

- [ ] **Step 5: Commit**

```sh
git add web/src/menu/mod.rs
git add web/src/menu/respawn_menu.rs
git add web/src/menu/loadout_fields.rs
git add web/src/menu/game_menu_view.rs
git add web/src/app.rs
git status --short
git commit -m "respawn menu"
```

Expected `git status --short` before the commit:

```text
M  web/src/app.rs
M  web/src/menu/game_menu_view.rs
M  web/src/menu/loadout_fields.rs
M  web/src/menu/mod.rs
A  web/src/menu/respawn_menu.rs
```

### Task 16: Pause menus

Pause game: Color (not skm), Skin, Name Labels, Messages, Leave Game; Apply sends `UpdateAppearance` when the appearance changed and stores the settings. Pause spectate: Name Labels, Messages, Leave Game. Pause tutorial: Leave Tutorial, and Back.

**Files:**
- Create: `web/src/menu/pause_menu.rs`
- Modify: `web/src/menu/mod.rs`
- Test: inline `mod tests` in `web/src/menu/pause_menu.rs`

- [ ] **Step 1: Write the failing tests**

Replace the whole contents of `web/src/menu/mod.rs` with:

```rust
pub mod create_menu;
pub mod game_menu_view;
pub mod join_menu;
pub mod loadout_fields;
pub mod menu_form;
pub mod pause_menu;
pub mod respawn_menu;
pub mod spectate_menu;

pub use create_menu::*;
pub use game_menu_view::*;
pub use join_menu::*;
pub use loadout_fields::*;
pub use menu_form::*;
pub use pause_menu::*;
pub use respawn_menu::*;
pub use spectate_menu::*;
```

Create `web/src/menu/pause_menu.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use shared::member::{OrganismColorKind, SkinKind};

    use crate::test_fixture;

    #[test]
    fn get_appearance_update_sends_a_changed_appearance() {
        let appearance: Appearance = Appearance {
            color: OrganismColorKind::Hot,
            skin: SkinKind::Grid,
        };

        assert_eq!(
            get_appearance_update(Some(&test_fixture::create_loadout()), appearance),
            Some(MessageSerialIn::UpdateAppearance {
                appearance: AppearanceSerial::from(&appearance),
            }),
        );
    }

    #[test]
    fn get_appearance_update_sends_nothing_for_an_unchanged_appearance_or_no_loadout() {
        let loadout: Loadout = test_fixture::create_loadout();

        assert_eq!(get_appearance_update(Some(&loadout), loadout.appearance), None);
        assert_eq!(get_appearance_update(None, loadout.appearance), None);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --lib menu::pause_menu::`

Expected: FAIL to compile:

```text
error[E0425]: cannot find type `Appearance` in this scope
```

- [ ] **Step 3: Implement the three pause menus**

In `web/src/menu/pause_menu.rs`, replace everything above `#[cfg(test)]` with:

```rust
use leptos::prelude::*;

use shared::ability::Loadout;
use shared::game::GameModeKind;
use shared::member::Appearance;
use shared::protocol::{AppearanceSerial, MessageSerialIn};

use crate::app::{AppSignals, ClientSettings};
use crate::canvas::DriverCommandKind;
use crate::canvas::game_canvas;
use crate::menu::game_menu_view;
use crate::menu::loadout_fields;
use crate::menu::{
    AutoAssignKind, LoadoutFieldValues, LoadoutFields, MenuFieldKind, MenuForm, MenuInput, MenuIssue, MenuRow,
    RadioSelectionKind,
};

const PAUSE_HEADER: &str = "Pause Options";

/// The Name Labels and Messages radios.
#[derive(Clone, Copy, Debug)]
struct ClientSettingFields {
    name_labels_index: RwSignal<Option<usize>>,
    messages_index: RwSignal<Option<usize>>,
}

impl ClientSettingFields {
    fn new(client_settings: ClientSettings) -> ClientSettingFields {
        ClientSettingFields {
            name_labels_index: RwSignal::new(get_single_radio_index(client_settings.name_labels_shown)),
            messages_index: RwSignal::new(get_single_radio_index(client_settings.messages_shown)),
        }
    }

    fn get_client_settings(&self) -> ClientSettings {
        ClientSettings {
            name_labels_shown: self.name_labels_index.get_untracked().is_some(),
            messages_shown: self.messages_index.get_untracked().is_some(),
        }
    }

    fn create_rows(&self) -> Vec<MenuRow> {
        vec![
            MenuRow::shown(
                MenuFieldKind::NameLabels,
                "Name Labels",
                loadout_fields::create_radios_input(
                    &loadout_fields::SINGLE_RADIO_LABELS,
                    self.name_labels_index,
                    RadioSelectionKind::Deselectable,
                ),
            ),
            MenuRow::shown(
                MenuFieldKind::Messages,
                "Messages",
                loadout_fields::create_radios_input(
                    &loadout_fields::SINGLE_RADIO_LABELS,
                    self.messages_index,
                    RadioSelectionKind::Deselectable,
                ),
            ),
        ]
    }
}

#[component]
pub fn PauseGameMenu() -> impl IntoView {
    let signals: AppSignals = expect_context();
    let issues: RwSignal<Vec<MenuIssue>> = RwSignal::new(Vec::new());
    let own_loadout: Option<Loadout> = signals.game_menu.get_untracked().and_then(|game_menu| game_menu.own_loadout);
    let mode: Signal<GameModeKind> = game_menu_view::get_game_menu_mode(signals.game_menu);
    let initial_values: LoadoutFieldValues = own_loadout.map_or_else(
        || LoadoutFieldValues::join_defaults(&[]),
        |own_loadout| LoadoutFieldValues::from_loadout(&own_loadout, None, AutoAssignKind::Unselected),
    );
    let loadout_fields: LoadoutFields = LoadoutFields::new(&initial_values, Signal::stored(Vec::new()));
    let setting_fields: ClientSettingFields = ClientSettingFields::new(signals.client_settings.get_untracked());
    let mut rows: Vec<MenuRow> = loadout_fields::create_appearance_rows(loadout_fields, mode);
    rows.extend(setting_fields.create_rows());
    rows.push(loadout_fields::create_leave_game_row());

    let on_submit: Callback<()> = Callback::new(move |_| {
        let appearance: Appearance = loadout_fields.get_values().appearance();
        let appearance_update: Option<MessageSerialIn> = get_appearance_update(own_loadout.as_ref(), appearance);

        if let Some(appearance_update) = appearance_update {
            game_canvas::dispatch_driver_command(DriverCommandKind::SendRequest(appearance_update));
        }

        apply_client_settings(signals, setting_fields.get_client_settings());
    });
    let on_back: Callback<()> = Callback::new(move |_| signals.go_back());

    view! {
        <MenuForm
            header=PAUSE_HEADER
            submit_label="Apply"
            rows=rows
            issues=issues
            on_submit=on_submit
            on_back=on_back
        />
    }
}

#[component]
pub fn PauseSpectateMenu() -> impl IntoView {
    let signals: AppSignals = expect_context();
    let issues: RwSignal<Vec<MenuIssue>> = RwSignal::new(Vec::new());
    let setting_fields: ClientSettingFields = ClientSettingFields::new(signals.client_settings.get_untracked());
    let mut rows: Vec<MenuRow> = setting_fields.create_rows();
    rows.push(loadout_fields::create_leave_game_row());

    let on_submit: Callback<()> =
        Callback::new(move |_| apply_client_settings(signals, setting_fields.get_client_settings()));
    let on_back: Callback<()> = Callback::new(move |_| signals.go_back());

    view! {
        <MenuForm
            header=PAUSE_HEADER
            submit_label="Apply"
            rows=rows
            issues=issues
            on_submit=on_submit
            on_back=on_back
        />
    }
}

#[component]
pub fn PauseTutorialMenu() -> impl IntoView {
    let signals: AppSignals = expect_context();
    let issues: RwSignal<Vec<MenuIssue>> = RwSignal::new(Vec::new());
    let rows: Vec<MenuRow> = vec![MenuRow::shown(
        MenuFieldKind::LeaveTutorial,
        "Leave Tutorial",
        MenuInput::Button {
            on_click: Callback::new(move |_| {
                game_canvas::dispatch_driver_command(DriverCommandKind::LeaveTutorial);
            }),
        },
    )];
    let go_back: Callback<()> = Callback::new(move |_| signals.go_back());

    view! {
        <MenuForm
            header=PAUSE_HEADER
            submit_label="Back"
            rows=rows
            issues=issues
            on_submit=go_back
            on_back=go_back
        />
    }
}

/// `None` when nothing would change or the member has no loadout.
pub fn get_appearance_update(own_loadout: Option<&Loadout>, appearance: Appearance) -> Option<MessageSerialIn> {
    let own_loadout: &Loadout = own_loadout?;

    if own_loadout.appearance == appearance {
        return None;
    }

    Some(MessageSerialIn::UpdateAppearance {
        appearance: AppearanceSerial::from(&appearance),
    })
}

fn apply_client_settings(signals: AppSignals, client_settings: ClientSettings) {
    game_canvas::dispatch_driver_command(DriverCommandKind::StoreClientSettings(client_settings));
    signals.client_settings.set(client_settings);
    signals.go_back();
}

fn get_single_radio_index(is_selected: bool) -> Option<usize> {
    if is_selected { Some(0) } else { None }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p web --lib menu::pause_menu::`

Expected: PASS:

```text
test menu::pause_menu::tests::get_appearance_update_sends_nothing_for_an_unchanged_appearance_or_no_loadout ... ok
test menu::pause_menu::tests::get_appearance_update_sends_a_changed_appearance ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 54 filtered out
```

- [ ] **Step 5: Commit**

```sh
git add web/src/menu/mod.rs
git add web/src/menu/pause_menu.rs
git status --short
git commit -m "pause menus"
```

Expected `git status --short` before the commit:

```text
M  web/src/menu/mod.rs
A  web/src/menu/pause_menu.rs
```

### Task 17: Title menu

The original's 170 by 150 box with Host, Join and Tutorial in `_bacter` 29 px; Host and Join are disabled while not connected. Below it, the notice line: the connection's notice, or else the lobby notice, with Reconnect when offered. A view without logic of its own (test-exempt by `docs/conventions/testing.md`); Task 26 mounts it.

**Files:**
- Create: `web/src/title/mod.rs`
- Create: `web/src/title/title_menu.rs`
- Create: `web/style/components/_title.scss`
- Modify: `web/src/lib.rs`
- Modify: `web/style/main.scss`

- [ ] **Step 1: Add the title menu and its styles**

Replace the whole contents of `web/src/lib.rs` with:

```rust
// The release view tree's monomorphized type exceeds rustc's default query depth.
#![recursion_limit = "512"]

pub mod app;
pub mod browser;
pub mod canvas;
pub mod connection;
pub mod menu;
pub mod screen;
pub mod title;

// fixtures shared by the unit tests of several modules; absent from every non-test build
#[cfg(test)]
pub mod test_fixture;

// browser runtime glue; the ssr build compiles none of it
#[cfg(feature = "hydrate")]
pub mod client;

// the dev server and the prerender; a browser runs neither
#[cfg(feature = "ssr")]
pub mod server;
```

Create `web/src/title/mod.rs`:

```rust
pub mod title_menu;

pub use title_menu::*;
```

Create `web/src/title/title_menu.rs`:

```rust
use leptos::prelude::*;

use crate::app::AppSignals;
use crate::canvas::DriverCommandKind;
use crate::canvas::game_canvas;
use crate::connection::{LobbyAccessKind, ReconnectOfferKind};
use crate::menu::CreateMenuFields;
use crate::screen::{LobbyNoticeKind, Screen};

const RECONNECT_TEXT: &str = "Reconnect";

/// The box over the title scene, with the connection notice line beneath it.
#[component]
pub fn TitleMenu() -> impl IntoView {
    let signals: AppSignals = expect_context();
    let create_menu_fields: CreateMenuFields = expect_context();
    let lobby_access: Signal<LobbyAccessKind> = Signal::derive(move || signals.connection_status.get().lobby_access());
    let is_lobby_disabled = move || lobby_access.get() == LobbyAccessKind::Disabled;
    let open_create_menu = move |_| {
        if lobby_access.get_untracked() == LobbyAccessKind::Enabled {
            create_menu_fields.reset();
            signals.navigate(Screen::CreateMenu);
        }
    };
    let open_browser = move |_| {
        if lobby_access.get_untracked() == LobbyAccessKind::Enabled {
            signals.navigate(Screen::Browser);
        }
    };
    let start_tutorial = move |_| game_canvas::dispatch_driver_command(DriverCommandKind::StartTutorial);
    let notice_text = move || {
        let connection_notice: Option<String> = signals.connection_status.get().notice_text().map(String::from);
        let lobby_notice_text = move || {
            signals
                .lobby_notice
                .get()
                .filter(|lobby_notice| *lobby_notice == LobbyNoticeKind::SynchronisationLost)
                .map(|lobby_notice| lobby_notice.text())
        };

        connection_notice.or_else(lobby_notice_text)
    };
    let is_reconnect_offered = move || signals.connection_status.get().reconnect_offer() == ReconnectOfferKind::Offered;
    let reconnect = move |_| game_canvas::dispatch_driver_command(DriverCommandKind::Reconnect);

    view! {
        <div class="title-menu">
            <div class="title-menu-button" class:title-menu-button-disabled=is_lobby_disabled on:click=open_create_menu>
                "Host"
            </div>
            <div class="title-menu-button" class:title-menu-button-disabled=is_lobby_disabled on:click=open_browser>
                "Join"
            </div>
            <div class="title-menu-button" on:click=start_tutorial>
                "Tutorial"
            </div>
        </div>
        <p class="title-notice">
            {notice_text}
            <Show when=is_reconnect_offered>
                <button class="title-reconnect" type="button" on:click=reconnect>
                    {RECONNECT_TEXT}
                </button>
            </Show>
        </p>
    }
}
```

Create `web/style/components/_title.scss`:

```scss
/* Centred on the window, as the original placed it on the title world's centre. */
.title-menu {
    position: fixed;
    left: calc(50% - 85px);
    top: calc(50% - 75px);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12.5px;
    width: 172px;
    height: 152px;
    padding-top: 28px;
    background: var(--color-title-box);
    border: 1px solid var(--color-white);
    border-radius: 1px;
}

.title-menu-button {
    width: 113px;
    height: 25px;
    color: var(--color-white);
    font-family: var(--font-title);
    font-size: 29px;
    line-height: 25px;
    text-align: center;
    cursor: pointer;
}

.title-menu-button-disabled {
    color: var(--color-lobby-disabled);
    cursor: default;
}

.title-notice {
    position: fixed;
    left: 0;
    top: calc(50% + 90px);
    width: 100%;
    color: var(--color-white);
    font-family: var(--font-heading);
    font-size: 14px;
    text-align: center;
    pointer-events: none;
}

.title-reconnect {
    margin-left: 10px;
    padding: 0 8px;
    height: 22px;
    color: var(--color-black);
    font-family: var(--font-menu);
    font-size: 14px;
    background: var(--color-control);
    border: none;
    border-bottom: 2px solid var(--color-black);
    border-radius: 3px;
    cursor: pointer;
    pointer-events: auto;

    &:hover {
        background: var(--color-control-hover);
    }

    &:active {
        background: var(--color-control-pressed);
    }
}
```

Replace the whole contents of `web/style/main.scss` with:

```scss
@use "tokens";
@use "reset";
@use "typography";
@use "layout";
@use "components/menu";
@use "components/browser";
@use "components/title";
```

- [ ] **Step 2: Build it**

Run: `cargo build -p web 2>&1 | grep -E '^(warning|error)|Finished'`

Expected: the build finishes without a warning:

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s)
```

- [ ] **Step 3: Commit**

```sh
git add web/src/lib.rs
git add web/src/title/mod.rs
git add web/src/title/title_menu.rs
git add web/style/main.scss
git add web/style/components/_title.scss
git status --short
git commit -m "title menu"
```

Expected `git status --short` before the commit:

```text
M  web/src/lib.rs
A  web/src/title/mod.rs
A  web/src/title/title_menu.rs
A  web/style/components/_title.scss
M  web/style/main.scss
```

### Task 18: Ability bar

Four slots 100 px apart around the window centre at nine tenths of its height: a black disc r 30 under a grey 215 dial r 29, draining while Active and refilling while Cooling (proportions from ticks), the icon's pen pixels as SVG squares, the key tab (X, C, V, `_`), the shot dial r 8 at x - 41 of a Compress or Freeze slot, and the green (own effect) and red (enemy effect, or the own Toxin) dots.

**Files:**
- Create: `web/src/hud/mod.rs`
- Create: `web/src/hud/ability_bar.rs`
- Create: `web/src/hud/ability_icon_view.rs`
- Create: `web/style/components/_ability-bar.scss`
- Modify: `web/src/lib.rs`
- Modify: `web/src/test_fixture/test_fixture.rs`
- Modify: `web/style/main.scss`
- Test: inline `mod tests` in both new Rust files

- [ ] **Step 1: Write the failing tests**

Replace the whole contents of `web/src/lib.rs` with:

```rust
// The release view tree's monomorphized type exceeds rustc's default query depth.
#![recursion_limit = "512"]

pub mod app;
pub mod browser;
pub mod canvas;
pub mod connection;
pub mod hud;
pub mod menu;
pub mod screen;
pub mod title;

// fixtures shared by the unit tests of several modules; absent from every non-test build
#[cfg(test)]
pub mod test_fixture;

// browser runtime glue; the ssr build compiles none of it
#[cfg(feature = "hydrate")]
pub mod client;

// the dev server and the prerender; a browser runs neither
#[cfg(feature = "ssr")]
pub mod server;
```

In `web/src/test_fixture/test_fixture.rs`, replace:

```rust
use shared::game::{GameModeKind, GameSettings, GameState};
use shared::member::{Appearance, Member, MemberId, MemberRoleKind, OrganismColorKind, Score, SkinKind};
use shared::world::WorldShapeKind;
```

with:

```rust
use shared::game::{GameModeKind, GameSettings, GameState};
use shared::geometry::WorldPoint;
use shared::member::{Appearance, Member, MemberId, MemberRoleKind, OrganismColorKind, Score, SkinKind};
use shared::organism::Organism;
use shared::world::WorldShapeKind;
```

Append to the end of `web/src/test_fixture/test_fixture.rs`, after one blank line:

```rust
pub fn create_participant_with_organism(member_id: MemberId, position: WorldPoint) -> Member {
    let mut member: Member = create_participant(member_id);
    member.organism = Some(Organism::new(position));

    member
}
```

Create `web/src/hud/mod.rs`:

```rust
pub mod ability_bar;
pub mod ability_icon_view;

pub use ability_bar::*;
pub use ability_icon_view::*;
```

Create `web/src/hud/ability_bar.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use shared::ability::Projectile;
    use shared::game::GameModeKind;
    use shared::geometry::{SubpixelPoint, SubpixelVector, WorldPoint};

    use crate::test_fixture;

    const OWN_MEMBER_ID: MemberId = MemberId(0);

    fn create_state_with_own_organism(loadout: Loadout) -> GameState {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll);
        let mut own_member: Member =
            test_fixture::create_participant_with_organism(OWN_MEMBER_ID, WorldPoint { x: 100, y: 100 });
        own_member.loadout = Some(loadout);
        test_fixture::insert_member(&mut state, own_member);
        state.tick = Tick(100);

        state
    }

    fn get_own_abilities_mut(state: &mut GameState) -> &mut OrganismAbilities {
        &mut state.members.get_mut(&OWN_MEMBER_ID).unwrap().organism.as_mut().unwrap().abilities
    }

    fn create_shot() -> Projectile {
        Projectile {
            position: SubpixelPoint { x: 0, y: 0 },
            velocity: SubpixelVector { x: 1, y: 0 },
        }
    }

    #[test]
    fn from_state_is_none_without_an_own_organism() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll);
        test_fixture::insert_member(&mut state, test_fixture::create_participant(OWN_MEMBER_ID));

        assert_eq!(AbilityBarView::from_state(&state, OWN_MEMBER_ID), None);
    }

    #[test]
    fn from_state_shows_ready_abilities_as_full_dials_with_the_loadout_icons() {
        let state: GameState = create_state_with_own_organism(test_fixture::create_loadout());

        let ability_bar: AbilityBarView = AbilityBarView::from_state(&state, OWN_MEMBER_ID).unwrap();
        let icons: Vec<AbilityIconKind> = ability_bar.slots.iter().map(|slot| slot.icon).collect();
        let key_labels: Vec<&str> = ability_bar.slots.iter().map(|slot| slot.key_label).collect();

        assert_eq!(
            icons,
            vec![
                AbilityIconKind::Extend,
                AbilityIconKind::Immortality,
                AbilityIconKind::Neutralize,
                AbilityIconKind::Spore,
            ],
        );
        assert_eq!(key_labels, vec!["X", "C", "V", "_"]);
        assert!(ability_bar.slots.iter().all(|slot| slot.dial == AbilityDialKind::Full));
        assert!(ability_bar.slots.iter().all(|slot| slot.shot_dial.is_none()));
    }

    #[test]
    fn from_state_drains_an_active_ability_and_refills_a_cooling_one() {
        let mut state: GameState = create_state_with_own_organism(test_fixture::create_loadout());
        let extend_active_ticks: u32 = ability_constants::EXTEND_ACTIVE_TICKS;
        let immortality_cooldown_ticks: u32 = ability_constants::IMMORTALITY_COOLDOWN_TICKS;
        let abilities: &mut OrganismAbilities = get_own_abilities_mut(&mut state);
        abilities.first = AbilityPhase::Active {
            ends_at: Tick(100).plus(extend_active_ticks / 2),
        };
        abilities.second = AbilityPhase::Cooling {
            ready_at: Tick(100).plus(immortality_cooldown_ticks / 4),
        };

        let ability_bar: AbilityBarView = AbilityBarView::from_state(&state, OWN_MEMBER_ID).unwrap();

        assert_eq!(
            ability_bar.slots[0].dial,
            AbilityDialKind::Partial {
                filled_proportion: (extend_active_ticks / 2) as f32 / extend_active_ticks as f32,
            },
        );
        assert_eq!(
            ability_bar.slots[1].dial,
            AbilityDialKind::Partial {
                filled_proportion: 1.0 - (immortality_cooldown_ticks / 4) as f32 / immortality_cooldown_ticks as f32,
            },
        );
        assert_eq!(ability_bar.slots[0].effect_dots, vec![EffectDotKind::OwnEffect]);
        assert!(ability_bar.slots[1].effect_dots.is_empty());
    }

    #[test]
    fn from_state_gives_shot_carriers_a_shot_dial() {
        let mut loadout: Loadout = test_fixture::create_loadout();
        loadout.first = FirstAbilityKind::Compress;
        loadout.second = SecondAbilityKind::Freeze;
        let mut state: GameState = create_state_with_own_organism(loadout);
        get_own_abilities_mut(&mut state).shots[ShotEffectKind::Freeze.slot_index()] = ShotPhase::Flying {
            ends_at: Tick(100),
            shot: create_shot(),
        };

        let ability_bar: AbilityBarView = AbilityBarView::from_state(&state, OWN_MEMBER_ID).unwrap();

        assert_eq!(ability_bar.slots[0].icon, AbilityIconKind::Compress);
        assert_eq!(ability_bar.slots[0].shot_dial, Some(AbilityDialKind::Full));
        assert_eq!(
            ability_bar.slots[1].shot_dial,
            Some(AbilityDialKind::Partial { filled_proportion: 0.0 }),
        );
    }

    #[test]
    fn from_state_marks_an_enemy_effect_but_never_a_casters_own_timer() {
        let mut loadout: Loadout = test_fixture::create_loadout();
        loadout.first = FirstAbilityKind::Compress;
        let mut state: GameState = create_state_with_own_organism(loadout);
        let abilities: &mut OrganismAbilities = get_own_abilities_mut(&mut state);
        abilities.first = AbilityPhase::Active { ends_at: Tick(130) };
        abilities.frozen_until = Some(Tick(120));

        let ability_bar: AbilityBarView = AbilityBarView::from_state(&state, OWN_MEMBER_ID).unwrap();

        assert!(ability_bar.slots[0].effect_dots.is_empty());
        assert_eq!(ability_bar.slots[1].effect_dots, vec![EffectDotKind::EnemyEffect]);
    }

    #[test]
    fn from_state_shows_secretion_on_the_fourth_slot() {
        let mut state: GameState = create_state_with_own_organism(test_fixture::create_loadout());
        get_own_abilities_mut(&mut state).spore = SporePhase::Secreting {
            ends_at: Tick(100),
            spores: Vec::new(),
        };

        let ability_bar: AbilityBarView = AbilityBarView::from_state(&state, OWN_MEMBER_ID).unwrap();

        assert_eq!(ability_bar.slots[3].icon, AbilityIconKind::Secrete);
        assert_eq!(
            ability_bar.slots[3].dial,
            AbilityDialKind::Partial { filled_proportion: 0.0 },
        );
    }

    #[test]
    fn get_dial_path_data_draws_a_wedge_clockwise_from_twelve_o_clock() {
        assert_eq!(
            get_dial_path_data(AbilityDialKind::Partial { filled_proportion: 0.0 }, 29.0),
            "",
        );
        assert_eq!(
            get_dial_path_data(
                AbilityDialKind::Partial {
                    filled_proportion: 0.25
                },
                29.0
            ),
            "M 0 0 L 0 -29 A 29 29 0 0 1 29.000 0.000 Z",
        );
        assert_eq!(
            get_dial_path_data(
                AbilityDialKind::Partial {
                    filled_proportion: 0.75
                },
                8.0
            ),
            "M 0 0 L 0 -8 A 8 8 0 1 1 -8.000 -0.000 Z",
        );
        assert_eq!(
            get_dial_path_data(AbilityDialKind::Full, 8.0),
            "M 8 0 A 8 8 0 1 1 -8 0 A 8 8 0 1 1 8 0 Z",
        );
    }
}
```

Create `web/src/hud/ability_icon_view.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_icon_path_data_draws_a_unit_square_for_every_pixel_of_the_pen() {
        let path_data: String = get_icon_path_data(AbilityIconKind::Spore);
        let pixel_count: usize = ability_icon::get_icon_pixels(AbilityIconKind::Spore).len();

        assert!(path_data.starts_with("M-1.5 -1.5h1v1h-1z"));
        assert_eq!(path_data.matches('M').count(), pixel_count);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --lib hud::`

Expected: FAIL to compile:

```text
error[E0425]: cannot find type `MemberId` in this scope
```

- [ ] **Step 3: Implement the bar and the icons**

In `web/src/hud/ability_bar.rs`, replace everything above `#[cfg(test)]` with:

```rust
use leptos::prelude::*;

use shared::ability::ability_constants;
use shared::ability::{
    AbilityIconKind, AbilityPhase, FirstAbilityKind, Loadout, OrganismAbilities, SecondAbilityKind, ShotEffectKind,
    ShotPhase, SporePhase, ThirdAbilityKind,
};
use shared::game::{GameState, Tick};
use shared::member::{Member, MemberId};
use shared::organism::Organism;

use crate::hud::AbilityIconView;

pub const ABILITY_SLOT_COUNT: usize = 4;
/// The original's X, C, V and space, space drawn as an underscore.
pub const ABILITY_KEY_LABELS: [&str; ABILITY_SLOT_COUNT] = ["X", "C", "V", "_"];
const SLOT_SPACING_PIXELS: f32 = 100.0;
/// The first slot's centre, left of the window centre.
const FIRST_SLOT_OFFSET_PIXELS: f32 = -150.0;
const DIAL_RADIUS_PIXELS: f32 = 29.0;
const SHOT_DIAL_RADIUS_PIXELS: f32 = 8.0;

/// A dial's grey disc, full when ready.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AbilityDialKind {
    Full,
    /// Clockwise from twelve o'clock.
    Partial {
        filled_proportion: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectDotKind {
    /// Green, left above the dial: the own Extend, Immortality or Neutralize.
    OwnEffect,
    /// Red, right above the dial: compressed, frozen, or the own Toxin field.
    EnemyEffect,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AbilitySlotView {
    pub icon: AbilityIconKind,
    pub key_label: &'static str,
    pub dial: AbilityDialKind,
    /// The shot a Compress or Freeze slot carries.
    pub shot_dial: Option<AbilityDialKind>,
    pub effect_dots: Vec<EffectDotKind>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AbilityBarView {
    pub slots: [AbilitySlotView; ABILITY_SLOT_COUNT],
}

impl AbilityBarView {
    /// `None` without an own organism.
    pub fn from_state(state: &GameState, own_member_id: MemberId) -> Option<AbilityBarView> {
        let own_member: &Member = state.members.get(&own_member_id)?;
        let loadout: &Loadout = own_member.loadout.as_ref()?;
        let organism: &Organism = own_member.organism.as_ref()?;
        let abilities: &OrganismAbilities = &organism.abilities;
        let tick: Tick = state.tick;

        Some(AbilityBarView {
            slots: [
                get_first_slot(abilities, loadout, tick),
                get_second_slot(abilities, loadout, tick),
                get_third_slot(abilities, loadout, tick),
                get_fourth_slot(abilities, tick),
            ],
        })
    }
}

/// Slots sit 100 px apart around the window centre, at nine tenths of its height.
#[component]
pub fn AbilityBar(ability_bar: Signal<Option<AbilityBarView>>) -> impl IntoView {
    move || {
        ability_bar.get().map(|ability_bar_view| {
            let slot_views: Vec<AnyView> = ability_bar_view
                .slots
                .into_iter()
                .enumerate()
                .map(|(slot_index, slot)| render_slot(slot_index, slot))
                .collect();

            view! { <div class="ability-bar">{slot_views}</div> }
        })
    }
}

/// An SVG path of the dial's filled part, centred on the origin; empty for nothing filled.
pub fn get_dial_path_data(dial: AbilityDialKind, radius: f32) -> String {
    let filled_proportion: f32 = match dial {
        AbilityDialKind::Full => 1.0,
        AbilityDialKind::Partial { filled_proportion } => filled_proportion,
    };

    if filled_proportion <= 0.0 {
        return String::new();
    }

    if filled_proportion >= 1.0 {
        return format!("M {radius} 0 A {radius} {radius} 0 1 1 -{radius} 0 A {radius} {radius} 0 1 1 {radius} 0 Z");
    }

    let end_angle: f32 = filled_proportion * std::f32::consts::TAU;
    let end_x: f32 = radius * end_angle.sin();
    let end_y: f32 = -radius * end_angle.cos();
    let large_arc_flag: u8 = if filled_proportion > 0.5 { 1 } else { 0 };

    format!("M 0 0 L 0 -{radius} A {radius} {radius} 0 {large_arc_flag} 1 {end_x:.3} {end_y:.3} Z")
}

fn render_slot(slot_index: usize, slot: AbilitySlotView) -> AnyView {
    let slot_offset_pixels: f32 = FIRST_SLOT_OFFSET_PIXELS + SLOT_SPACING_PIXELS * slot_index as f32;
    let slot_style: String = format!("left: calc(50% + {slot_offset_pixels}px)");
    let dial_path_data: String = get_dial_path_data(slot.dial, DIAL_RADIUS_PIXELS);
    let shot_dial_view: Option<AnyView> = slot.shot_dial.map(render_shot_dial);
    let effect_dot_views: Vec<AnyView> = slot.effect_dots.into_iter().map(render_effect_dot).collect();

    view! {
        <div class="ability-slot" style=slot_style>
            <div class="ability-key-tab">{slot.key_label}</div>
            <svg class="ability-dial" viewBox="-30 -30 60 60" width="60" height="60">
                <circle class="ability-dial-back" r="30"></circle>
                <path class="ability-dial-fill" d=dial_path_data></path>
            </svg>
            <AbilityIconView icon=slot.icon />
            {shot_dial_view}
            {effect_dot_views}
        </div>
    }
    .into_any()
}

fn render_shot_dial(shot_dial: AbilityDialKind) -> AnyView {
    let shot_dial_path_data: String = get_dial_path_data(shot_dial, SHOT_DIAL_RADIUS_PIXELS);

    view! {
        <svg class="ability-shot-dial" viewBox="-9 -9 18 18" width="18" height="18">
            <path class="ability-shot-dial-fill" d=shot_dial_path_data></path>
        </svg>
    }
    .into_any()
}

fn render_effect_dot(effect_dot: EffectDotKind) -> AnyView {
    let effect_dot_class: &str = match effect_dot {
        EffectDotKind::OwnEffect => "ability-effect-dot ability-effect-dot-own",
        EffectDotKind::EnemyEffect => "ability-effect-dot ability-effect-dot-enemy",
    };

    view! { <div class=effect_dot_class></div> }.into_any()
}

fn get_first_slot(abilities: &OrganismAbilities, loadout: &Loadout, tick: Tick) -> AbilitySlotView {
    let (icon, shot_dial): (AbilityIconKind, Option<AbilityDialKind>) = match loadout.first {
        FirstAbilityKind::Extend => (AbilityIconKind::Extend, None),
        FirstAbilityKind::Compress => (
            AbilityIconKind::Compress,
            Some(get_shot_dial(
                &abilities.shots[ShotEffectKind::Compress.slot_index()],
                tick,
            )),
        ),
    };
    let effect_dots: Vec<EffectDotKind> =
        get_effect_dots(abilities.is_extended(loadout), abilities.compressed_until.is_some());

    AbilitySlotView {
        icon,
        key_label: ABILITY_KEY_LABELS[0],
        dial: get_ability_dial(
            abilities.first,
            loadout.first.active_ticks(),
            loadout.first.cooldown_ticks(),
            tick,
        ),
        shot_dial,
        effect_dots,
    }
}

fn get_second_slot(abilities: &OrganismAbilities, loadout: &Loadout, tick: Tick) -> AbilitySlotView {
    let (icon, shot_dial): (AbilityIconKind, Option<AbilityDialKind>) = match loadout.second {
        SecondAbilityKind::Immortality => (AbilityIconKind::Immortality, None),
        SecondAbilityKind::Freeze => (
            AbilityIconKind::Freeze,
            Some(get_shot_dial(
                &abilities.shots[ShotEffectKind::Freeze.slot_index()],
                tick,
            )),
        ),
    };
    let effect_dots: Vec<EffectDotKind> =
        get_effect_dots(abilities.is_immortal(loadout), abilities.frozen_until.is_some());

    AbilitySlotView {
        icon,
        key_label: ABILITY_KEY_LABELS[1],
        dial: get_ability_dial(
            abilities.second,
            loadout.second.active_ticks(),
            loadout.second.cooldown_ticks(),
            tick,
        ),
        shot_dial,
        effect_dots,
    }
}

fn get_third_slot(abilities: &OrganismAbilities, loadout: &Loadout, tick: Tick) -> AbilitySlotView {
    let icon: AbilityIconKind = match loadout.third {
        ThirdAbilityKind::Neutralize => AbilityIconKind::Neutralize,
        ThirdAbilityKind::Toxin => AbilityIconKind::Toxin,
    };
    let effect_dots: Vec<EffectDotKind> = get_effect_dots(
        abilities.is_neutralize_field_active(loadout),
        abilities.is_toxin_field_active(loadout),
    );

    AbilitySlotView {
        icon,
        key_label: ABILITY_KEY_LABELS[2],
        dial: get_ability_dial(
            abilities.third,
            loadout.third.active_ticks(),
            loadout.third.cooldown_ticks(),
            tick,
        ),
        shot_dial: None,
        effect_dots,
    }
}

fn get_fourth_slot(abilities: &OrganismAbilities, tick: Tick) -> AbilitySlotView {
    let (icon, dial): (AbilityIconKind, AbilityDialKind) = match &abilities.spore {
        SporePhase::Ready => (AbilityIconKind::Spore, AbilityDialKind::Full),
        SporePhase::Flying { ends_at, .. } => (
            AbilityIconKind::Spore,
            get_remaining_dial(*ends_at, ability_constants::SPORE_FLIGHT_TICKS, tick),
        ),
        SporePhase::Secreting { ends_at, .. } => (
            AbilityIconKind::Secrete,
            get_remaining_dial(*ends_at, ability_constants::SPORE_SECRETION_TICKS, tick),
        ),
        SporePhase::Cooling { ready_at } => (
            AbilityIconKind::Spore,
            get_elapsed_dial(*ready_at, ability_constants::SPORE_COOLDOWN_TICKS, tick),
        ),
    };

    AbilitySlotView {
        icon,
        key_label: ABILITY_KEY_LABELS[3],
        dial,
        shot_dial: None,
        effect_dots: Vec::new(),
    }
}

/// Draining while Active, refilling while Cooling.
fn get_ability_dial(phase: AbilityPhase, active_ticks: u32, cooldown_ticks: u32, tick: Tick) -> AbilityDialKind {
    match phase {
        AbilityPhase::Ready => AbilityDialKind::Full,
        AbilityPhase::Active { ends_at } => get_remaining_dial(ends_at, active_ticks, tick),
        AbilityPhase::Cooling { ready_at } => get_elapsed_dial(ready_at, cooldown_ticks, tick),
    }
}

fn get_shot_dial(shot: &ShotPhase, tick: Tick) -> AbilityDialKind {
    match shot {
        ShotPhase::Ready => AbilityDialKind::Full,
        ShotPhase::Flying { ends_at, .. } => get_remaining_dial(*ends_at, ability_constants::SHOT_FLIGHT_TICKS, tick),
        ShotPhase::Secreting { ends_at, .. } => {
            get_remaining_dial(*ends_at, ability_constants::SHOT_SECRETION_TICKS, tick)
        }
        ShotPhase::Cooling { ready_at } => get_elapsed_dial(*ready_at, ability_constants::SHOT_COOLDOWN_TICKS, tick),
    }
}

fn get_remaining_dial(ends_at: Tick, duration_ticks: u32, tick: Tick) -> AbilityDialKind {
    AbilityDialKind::Partial {
        filled_proportion: get_tick_proportion(ends_at.ticks_since(tick), duration_ticks),
    }
}

fn get_elapsed_dial(ready_at: Tick, duration_ticks: u32, tick: Tick) -> AbilityDialKind {
    AbilityDialKind::Partial {
        filled_proportion: 1.0 - get_tick_proportion(ready_at.ticks_since(tick), duration_ticks),
    }
}

fn get_tick_proportion(tick_count: u32, duration_ticks: u32) -> f32 {
    if duration_ticks == 0 {
        return 0.0;
    }

    (tick_count as f32 / duration_ticks as f32).clamp(0.0, 1.0)
}

fn get_effect_dots(has_own_effect: bool, has_enemy_effect: bool) -> Vec<EffectDotKind> {
    [
        (has_own_effect, EffectDotKind::OwnEffect),
        (has_enemy_effect, EffectDotKind::EnemyEffect),
    ]
    .into_iter()
    .filter(|(is_shown, _)| *is_shown)
    .map(|(_, effect_dot)| effect_dot)
    .collect()
}
```

In `web/src/hud/ability_icon_view.rs`, replace everything above `#[cfg(test)]` with:

```rust
use leptos::prelude::*;

use shared::ability::ability_icon;
use shared::ability::{AbilityIconKind, IconPixel};

/// A pixel point of the original's pen covers the square around it.
const PIXEL_HALF_WIDTH: f32 = 0.5;

/// The icon's pixels in black, centred on its slot.
#[component]
pub fn AbilityIconView(icon: AbilityIconKind) -> impl IntoView {
    view! {
        <svg class="ability-icon" viewBox="-30 -30 60 60" width="60" height="60" shape-rendering="crispEdges">
            <path d=get_icon_path_data(icon)></path>
        </svg>
    }
}

/// One unit square per pixel, in drawing order.
pub fn get_icon_path_data(icon: AbilityIconKind) -> String {
    ability_icon::get_icon_pixels(icon)
        .into_iter()
        .map(|icon_pixel: IconPixel| {
            let left: f32 = icon_pixel.x as f32 - PIXEL_HALF_WIDTH;
            let top: f32 = icon_pixel.y as f32 - PIXEL_HALF_WIDTH;

            format!("M{left} {top}h1v1h-1z")
        })
        .collect()
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p web --lib hud::`

Expected: PASS:

```text
test hud::ability_bar::tests::from_state_is_none_without_an_own_organism ... ok
test hud::ability_bar::tests::from_state_drains_an_active_ability_and_refills_a_cooling_one ... ok
test hud::ability_bar::tests::from_state_marks_an_enemy_effect_but_never_a_casters_own_timer ... ok
test hud::ability_bar::tests::from_state_gives_shot_carriers_a_shot_dial ... ok
test hud::ability_bar::tests::from_state_shows_ready_abilities_as_full_dials_with_the_loadout_icons ... ok
test hud::ability_bar::tests::from_state_shows_secretion_on_the_fourth_slot ... ok
test hud::ability_bar::tests::get_dial_path_data_draws_a_wedge_clockwise_from_twelve_o_clock ... ok
test hud::ability_icon_view::tests::get_icon_path_data_draws_a_unit_square_for_every_pixel_of_the_pen ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 56 filtered out
```

- [ ] **Step 5: Add the ability bar styles**

Create `web/style/components/_ability-bar.scss`:

```scss
/* Every slot child is placed from the slot's centre, as the original drew it. */
.ability-bar {
    position: absolute;
    left: 0;
    top: 90%;
    width: 100%;
    height: 0;
}

.ability-slot {
    position: absolute;
    top: 0;
    width: 0;
    height: 0;
}

.ability-key-tab {
    position: absolute;
    left: -12px;
    top: 11px;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    width: 24px;
    height: 38px;
    padding-bottom: 4px;
    color: var(--color-black);
    font-family: var(--font-key);
    font-size: 14px;
    font-weight: bold;
    background: var(--color-dial);
    border: 1px solid var(--color-black);
    border-radius: 0 0 4px 4px;
}

.ability-dial,
.ability-icon {
    position: absolute;
    left: -30px;
    top: -30px;
    overflow: visible;
}

.ability-dial-back {
    fill: var(--color-black);
}

.ability-dial-fill {
    fill: var(--color-dial);
}

.ability-icon {
    fill: var(--color-black);
}

.ability-shot-dial {
    position: absolute;
    left: -50px;
    top: -9px;
    overflow: visible;
}

.ability-shot-dial-fill {
    fill: var(--color-dial);
    stroke: var(--color-black);
    stroke-width: 1px;
}

.ability-effect-dot {
    position: absolute;
    top: -42px;
    width: 10px;
    height: 10px;
    border-radius: 50%;
}

.ability-effect-dot-own {
    left: -14px;
    background: var(--color-own-effect);
}

.ability-effect-dot-enemy {
    left: 4px;
    background: var(--color-enemy-effect);
}
```

Replace the whole contents of `web/style/main.scss` with:

```scss
@use "tokens";
@use "reset";
@use "typography";
@use "layout";
@use "components/menu";
@use "components/browser";
@use "components/title";
@use "components/ability-bar";
```

- [ ] **Step 6: Commit**

```sh
git add web/src/lib.rs
git add web/src/hud/mod.rs
git add web/src/hud/ability_bar.rs
git add web/src/hud/ability_icon_view.rs
git add web/src/test_fixture/test_fixture.rs
git add web/style/main.scss
git add web/style/components/_ability-bar.scss
git status --short
git commit -m "ability bar"
```

Expected `git status --short` before the commit:

```text
A  web/src/hud/ability_bar.rs
A  web/src/hud/ability_icon_view.rs
A  web/src/hud/mod.rs
M  web/src/lib.rs
M  web/src/test_fixture/test_fixture.rs
A  web/style/components/_ability-bar.scss
M  web/style/main.scss
```

### Task 19: Name labels

**Files:**
- Create: `web/src/hud/name_labels.rs`
- Create: `web/style/components/_name-labels.scss`
- Modify: `web/src/hud/mod.rs`
- Modify: `web/style/main.scss`
- Test: inline `mod tests` in `web/src/hud/name_labels.rs`

- [ ] **Step 1: Write the failing tests**

Replace the whole contents of `web/src/hud/mod.rs` with:

```rust
pub mod ability_bar;
pub mod ability_icon_view;
pub mod name_labels;

pub use ability_bar::*;
pub use ability_icon_view::*;
pub use name_labels::*;
```

Create `web/src/hud/name_labels.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use shared::game::GameModeKind;
    use shared::geometry::{LatticeCoordinate, WorldPoint};
    use shared::member::Member;
    use shared::render::{PhysicalSize, ViewportSize};

    use crate::test_fixture;

    const MEMBER_ID: MemberId = MemberId(2);

    fn create_camera() -> Camera {
        Camera {
            view_origin: InterpolatedPoint { x: 50.0, y: 20.0 },
            viewport: ViewportSize {
                physical_size: PhysicalSize {
                    width_pixels: 800,
                    height_pixels: 600,
                },
                device_pixel_ratio: 1.0,
            },
        }
    }

    #[test]
    fn get_name_labels_places_a_label_under_the_centroid_in_view_pixels() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll);
        let mut member: Member =
            test_fixture::create_participant_with_organism(MEMBER_ID, WorldPoint { x: 100, y: 200 });
        member.organism.as_mut().unwrap().cells.insert(LatticeCoordinate { i: 1, j: 0 });
        test_fixture::insert_member(&mut state, member);
        test_fixture::insert_member(&mut state, test_fixture::create_participant(MemberId(3)));

        let name_labels: Vec<NameLabelView> = get_name_labels(&state, &create_camera());
        let disc_radius_pixels: f32 = (36.0 * 2.0 / std::f32::consts::PI).sqrt();

        assert_eq!(
            name_labels,
            vec![NameLabelView {
                member_id: MEMBER_ID,
                text: String::from("player 2"),
                position: ViewPoint {
                    x: 103.0 - 50.0,
                    y: 200.0 + disc_radius_pixels + 20.0 - 20.0,
                },
            }],
        );
    }

    #[test]
    fn get_shown_name_cuts_names_over_thirty_characters_to_twenty_and_an_ellipsis() {
        let thirty_character_name: String = "a".repeat(30);
        let long_name: String = format!("{}{}", "b".repeat(20), "c".repeat(11));

        assert_eq!(get_shown_name(&thirty_character_name), thirty_character_name);
        assert_eq!(get_shown_name(&long_name), format!("{}...", "b".repeat(20)));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --lib hud::name_labels::`

Expected: FAIL to compile:

```text
error[E0425]: cannot find type `MemberId` in this scope
```

- [ ] **Step 3: Implement the labels**

In `web/src/hud/name_labels.rs`, replace everything above `#[cfg(test)]` with:

```rust
use leptos::prelude::*;

use shared::game::GameState;
use shared::geometry;
use shared::member::MemberId;
use shared::organism::{LatticeCentroid, Organism};
use shared::play::InterpolatedPoint;
use shared::render::{Camera, ViewPoint};

/// Longer names are cut to `TRUNCATED_NAME_CHARACTER_COUNT` characters and an ellipsis (faithful).
pub const NAME_LABEL_CHARACTER_LIMIT: usize = 30;
pub const TRUNCATED_NAME_CHARACTER_COUNT: usize = 20;
const TRUNCATION_SUFFIX: &str = "...";
/// The original's two cell widths plus 8 px below the organism's radius.
const LABEL_GAP_PIXELS: f32 = 2.0 * geometry::CELL_WIDTH_PIXELS as f32 + 8.0;

#[derive(Clone, Debug, PartialEq)]
pub struct NameLabelView {
    pub member_id: MemberId,
    pub text: String,
    /// The text's baseline centre.
    pub position: ViewPoint,
}

#[component]
pub fn NameLabels(name_labels: Signal<Vec<NameLabelView>>) -> impl IntoView {
    move || {
        name_labels
            .get()
            .into_iter()
            .map(|name_label| {
                let label_style: String =
                    format!("left: {}px; top: {}px", name_label.position.x, name_label.position.y);

                view! { <p class="name-label" style=label_style>{name_label.text}</p> }
            })
            .collect_view()
    }
}

/// Under every organism's centroid, at the radius of a disc of its cell area.
pub fn get_name_labels(state: &GameState, camera: &Camera) -> Vec<NameLabelView> {
    state
        .members
        .values()
        .filter_map(|member| {
            let organism: &Organism = member.organism.as_ref()?;
            let label_point: InterpolatedPoint = get_label_point(organism)?;

            Some(NameLabelView {
                member_id: member.member_id,
                text: get_shown_name(&member.screen_name),
                position: camera.get_view_point(label_point),
            })
        })
        .collect()
}

pub fn get_shown_name(screen_name: &str) -> String {
    if screen_name.chars().count() <= NAME_LABEL_CHARACTER_LIMIT {
        return String::from(screen_name);
    }

    let truncated_name: String = screen_name.chars().take(TRUNCATED_NAME_CHARACTER_COUNT).collect();

    format!("{truncated_name}{TRUNCATION_SUFFIX}")
}

/// `None` for an organism without cells.
fn get_label_point(organism: &Organism) -> Option<InterpolatedPoint> {
    let centroid: LatticeCentroid = organism.centroid();

    if centroid.cell_count == 0 {
        return None;
    }

    let cell_count: f32 = centroid.cell_count as f32;
    let cell_width_pixels: f32 = geometry::CELL_WIDTH_PIXELS as f32;
    let centre_x: f32 = organism.anchor.x as f32 + cell_width_pixels * centroid.sum_i as f32 / cell_count;
    let centre_y: f32 = organism.anchor.y as f32 + cell_width_pixels * centroid.sum_j as f32 / cell_count;
    let disc_radius_pixels: f32 = (cell_width_pixels * cell_width_pixels * cell_count / std::f32::consts::PI).sqrt();

    Some(InterpolatedPoint {
        x: centre_x,
        y: centre_y + disc_radius_pixels + LABEL_GAP_PIXELS,
    })
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p web --lib hud::name_labels::`

Expected: PASS:

```text
test hud::name_labels::tests::get_shown_name_cuts_names_over_thirty_characters_to_twenty_and_an_ellipsis ... ok
test hud::name_labels::tests::get_name_labels_places_a_label_under_the_centroid_in_view_pixels ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 64 filtered out
```

- [ ] **Step 5: Add the label styles**

Create `web/style/components/_name-labels.scss`:

```scss
/* Centred on the shown text, its baseline at the label point. */
.name-label {
    position: absolute;
    color: var(--color-world-border);
    font-family: var(--font-overlay);
    font-size: 10px;
    line-height: 1;
    white-space: pre;
    transform: translate(-50%, -80%);
}
```

Replace the whole contents of `web/style/main.scss` with:

```scss
@use "tokens";
@use "reset";
@use "typography";
@use "layout";
@use "components/menu";
@use "components/browser";
@use "components/title";
@use "components/ability-bar";
@use "components/name-labels";
```

- [ ] **Step 6: Commit**

```sh
git add web/src/hud/mod.rs
git add web/src/hud/name_labels.rs
git add web/style/main.scss
git add web/style/components/_name-labels.scss
git status --short
git commit -m "name labels"
```

Expected `git status --short` before the commit:

```text
M  web/src/hud/mod.rs
A  web/src/hud/name_labels.rs
A  web/style/components/_name-labels.scss
M  web/style/main.scss
```

### Task 20: Leaderboard

**Files:**
- Create: `web/src/leaderboard/mod.rs`
- Create: `web/src/leaderboard/leaderboard.rs`
- Create: `web/style/components/_leaderboard.scss`
- Modify: `web/src/lib.rs`
- Modify: `web/style/main.scss`
- Test: inline `mod tests` in `web/src/leaderboard/leaderboard.rs`

- [ ] **Step 1: Write the failing tests**

Replace the whole contents of `web/src/lib.rs` with:

```rust
// The release view tree's monomorphized type exceeds rustc's default query depth.
#![recursion_limit = "512"]

pub mod app;
pub mod browser;
pub mod canvas;
pub mod connection;
pub mod hud;
pub mod leaderboard;
pub mod menu;
pub mod screen;
pub mod title;

// fixtures shared by the unit tests of several modules; absent from every non-test build
#[cfg(test)]
pub mod test_fixture;

// browser runtime glue; the ssr build compiles none of it
#[cfg(feature = "hydrate")]
pub mod client;

// the dev server and the prerender; a browser runs neither
#[cfg(feature = "ssr")]
pub mod server;
```

Create `web/src/leaderboard/mod.rs`:

```rust
pub mod leaderboard;

pub use leaderboard::*;
```

Create `web/src/leaderboard/leaderboard.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use shared::game::GameModeKind;
    use shared::member::Member;

    use crate::test_fixture;

    const OWN_MEMBER_ID: MemberId = MemberId(1);
    const OTHER_MEMBER_ID: MemberId = MemberId(2);

    fn create_state_with_two_participants(mode: GameModeKind) -> GameState {
        let mut state: GameState = test_fixture::create_state(mode);
        let mut own_member: Member = test_fixture::create_participant(OWN_MEMBER_ID);
        own_member.score.kills = 3;
        own_member.score.deaths = 2;
        let mut other_member: Member = test_fixture::create_participant(OTHER_MEMBER_ID);
        other_member.score.kills = 5;

        if mode == GameModeKind::Skirmish {
            own_member.team = Some(TeamKind::Blue);
            other_member.team = Some(TeamKind::Red);
        }

        test_fixture::insert_member(&mut state, own_member);
        test_fixture::insert_member(&mut state, other_member);

        state
    }

    #[test]
    fn from_state_lists_free_for_all_players_and_emphasises_the_own_row() {
        let state: GameState = create_state_with_two_participants(GameModeKind::FreeForAll);

        let leaderboard_view: LeaderboardView = LeaderboardView::from_state(&state, OWN_MEMBER_ID);

        assert_eq!(
            leaderboard_view,
            LeaderboardView {
                columns: vec![
                    LeaderboardColumnKind::Player,
                    LeaderboardColumnKind::Kills,
                    LeaderboardColumnKind::Deaths,
                    LeaderboardColumnKind::KillDeathRatio,
                ],
                rows: vec![
                    LeaderboardRowView {
                        cells: vec![
                            String::from("player 2"),
                            String::from("5"),
                            String::from("0"),
                            String::from("∞"),
                        ],
                        emphasis: RowEmphasisKind::Other,
                    },
                    LeaderboardRowView {
                        cells: vec![
                            String::from("player 1"),
                            String::from("3"),
                            String::from("2"),
                            String::from("1.5"),
                        ],
                        emphasis: RowEmphasisKind::Own,
                    },
                ],
            },
        );
    }

    #[test]
    fn from_state_lists_skirmish_teams_and_emphasises_the_own_team() {
        let state: GameState = create_state_with_two_participants(GameModeKind::Skirmish);

        let leaderboard_view: LeaderboardView = LeaderboardView::from_state(&state, OWN_MEMBER_ID);
        let team_cells: Vec<(String, RowEmphasisKind)> =
            leaderboard_view.rows.into_iter().map(|row| (row.cells[0].clone(), row.emphasis)).collect();

        assert_eq!(
            team_cells,
            vec![
                (String::from("Red"), RowEmphasisKind::Other),
                (String::from("Blue"), RowEmphasisKind::Own),
            ],
        );
    }

    #[test]
    fn get_column_heading_names_each_column_as_the_original() {
        assert_eq!(get_column_heading(LeaderboardColumnKind::KillDeathRatio), "K:D");
        assert_eq!(get_column_heading(LeaderboardColumnKind::Wins), "Wins");
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --lib leaderboard::`

Expected: FAIL to compile:

```text
error[E0425]: cannot find type `MemberId` in this scope
```

- [ ] **Step 3: Implement the leaderboard**

In `web/src/leaderboard/leaderboard.rs`, replace everything above `#[cfg(test)]` with:

```rust
use leptos::prelude::*;

use shared::game::GameState;
use shared::member::scoreboard;
use shared::member::{LeaderboardColumnKind, LeaderboardRow, LeaderboardSubjectKind, MemberId, TeamKind};

use crate::menu::loadout_fields;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowEmphasisKind {
    /// The own member, or its team.
    Own,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaderboardRowView {
    /// One per column.
    pub cells: Vec<String>,
    pub emphasis: RowEmphasisKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaderboardView {
    pub columns: Vec<LeaderboardColumnKind>,
    pub rows: Vec<LeaderboardRowView>,
}

impl LeaderboardView {
    pub fn from_state(state: &GameState, own_member_id: MemberId) -> LeaderboardView {
        let columns: Vec<LeaderboardColumnKind> = scoreboard::get_leaderboard_columns(state.settings.mode);
        let own_team: Option<TeamKind> = state.members.get(&own_member_id).and_then(|member| member.team);
        let rows: Vec<LeaderboardRowView> = scoreboard::get_leaderboard_rows(state)
            .iter()
            .map(|leaderboard_row| LeaderboardRowView {
                cells: columns.iter().map(|column| get_cell_text(leaderboard_row, *column)).collect(),
                emphasis: get_row_emphasis(leaderboard_row, own_member_id, own_team),
            })
            .collect();

        LeaderboardView { columns, rows }
    }
}

/// Top right of the window, over the canvas.
#[component]
pub fn Leaderboard(leaderboard: Signal<Option<LeaderboardView>>) -> impl IntoView {
    move || {
        leaderboard.get().map(|leaderboard_view| {
            let heading_views = leaderboard_view
                .columns
                .iter()
                .map(|column| view! { <th>{get_column_heading(*column)}</th> })
                .collect_view();
            let row_views = leaderboard_view
                .rows
                .into_iter()
                .map(|row| {
                    let row_class: &str = match row.emphasis {
                        RowEmphasisKind::Own => "leaderboard-row leaderboard-row-own",
                        RowEmphasisKind::Other => "leaderboard-row",
                    };
                    let cell_views = row.cells.into_iter().map(|cell| view! { <td>{cell}</td> }).collect_view();

                    view! { <tr class=row_class>{cell_views}</tr> }
                })
                .collect_view();

            view! {
                <table class="leaderboard">
                    <thead>
                        <tr>{heading_views}</tr>
                    </thead>
                    <tbody>{row_views}</tbody>
                </table>
            }
        })
    }
}

pub fn get_column_heading(column: LeaderboardColumnKind) -> &'static str {
    match column {
        LeaderboardColumnKind::Player => "Player",
        LeaderboardColumnKind::Team => "Team",
        LeaderboardColumnKind::Kills => "Kills",
        LeaderboardColumnKind::Deaths => "Deaths",
        LeaderboardColumnKind::KillDeathRatio => "K:D",
        LeaderboardColumnKind::Wins => "Wins",
    }
}

fn get_cell_text(leaderboard_row: &LeaderboardRow, column: LeaderboardColumnKind) -> String {
    match (column, &leaderboard_row.subject) {
        (
            LeaderboardColumnKind::Player | LeaderboardColumnKind::Team,
            LeaderboardSubjectKind::Member { screen_name, .. },
        ) => screen_name.clone(),
        (LeaderboardColumnKind::Player | LeaderboardColumnKind::Team, LeaderboardSubjectKind::Team { team }) => {
            loadout_fields::get_team_label(*team)
        }
        (LeaderboardColumnKind::Kills, _) => leaderboard_row.score.kills.to_string(),
        (LeaderboardColumnKind::Deaths, _) => leaderboard_row.score.deaths.to_string(),
        (LeaderboardColumnKind::KillDeathRatio, _) => {
            scoreboard::format_kill_death_ratio(leaderboard_row.score.kills, leaderboard_row.score.deaths)
        }
        (LeaderboardColumnKind::Wins, _) => leaderboard_row.score.wins.to_string(),
    }
}

fn get_row_emphasis(
    leaderboard_row: &LeaderboardRow,
    own_member_id: MemberId,
    own_team: Option<TeamKind>,
) -> RowEmphasisKind {
    let is_own_row: bool = match &leaderboard_row.subject {
        LeaderboardSubjectKind::Member { member_id, .. } => *member_id == own_member_id,
        LeaderboardSubjectKind::Team { team } => own_team == Some(*team),
    };

    if is_own_row {
        RowEmphasisKind::Own
    } else {
        RowEmphasisKind::Other
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p web --lib leaderboard::`

Expected: PASS:

```text
test leaderboard::leaderboard::tests::get_column_heading_names_each_column_as_the_original ... ok
test leaderboard::leaderboard::tests::from_state_lists_free_for_all_players_and_emphasises_the_own_row ... ok
test leaderboard::leaderboard::tests::from_state_lists_skirmish_teams_and_emphasises_the_own_team ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 66 filtered out
```

- [ ] **Step 5: Add the leaderboard styles**

Create `web/style/components/_leaderboard.scss`:

```scss
.leaderboard {
    position: absolute;
    top: 13px;
    right: 15px;
    border-collapse: collapse;
    table-layout: fixed;
    /* Not auto, so the fixed layout applies and the table is the sum of its columns. */
    width: 0;
    color: var(--color-black);
    font-family: var(--font-overlay);
    font-size: 11px;
    box-shadow: 4px 3px 0 var(--color-shadow);

    th,
    td {
        width: 46px;
        height: 22px;
        padding: 0 5px;
        text-align: left;
        white-space: nowrap;
        overflow: hidden;
        border: 1px solid var(--color-black);
    }

    th:first-child,
    td:first-child {
        width: 170px;
    }

    th {
        font-weight: bold;
        background: var(--color-leaderboard-head);
    }

    td {
        background: var(--color-leaderboard-cell);
    }
}

.leaderboard-row-own td {
    font-family: var(--font-heading);
    font-weight: bold;
}
```

Replace the whole contents of `web/style/main.scss` with:

```scss
@use "tokens";
@use "reset";
@use "typography";
@use "layout";
@use "components/menu";
@use "components/browser";
@use "components/title";
@use "components/ability-bar";
@use "components/name-labels";
@use "components/leaderboard";
```

- [ ] **Step 6: Commit**

```sh
git add web/src/lib.rs
git add web/src/leaderboard/mod.rs
git add web/src/leaderboard/leaderboard.rs
git add web/style/main.scss
git add web/style/components/_leaderboard.scss
git status --short
git commit -m "leaderboard"
```

Expected `git status --short` before the commit:

```text
A  web/src/leaderboard/leaderboard.rs
A  web/src/leaderboard/mod.rs
M  web/src/lib.rs
A  web/style/components/_leaderboard.scss
M  web/style/main.scss
```

### Task 21: Message box

Top left, white Helvetica 14 on black with a white border and the (+5,+4) shadow. The box shrinks to its text, which keeps its line breaks, so its width is the DOM's own measure of the text.

**Files:**
- Create: `web/src/message/mod.rs`
- Create: `web/src/message/message_box.rs`
- Create: `web/style/components/_message.scss`
- Modify: `web/src/lib.rs`
- Modify: `web/style/main.scss`

- [ ] **Step 1: Add the message box and its styles**

Replace the whole contents of `web/src/lib.rs` with:

```rust
// The release view tree's monomorphized type exceeds rustc's default query depth.
#![recursion_limit = "512"]

pub mod app;
pub mod browser;
pub mod canvas;
pub mod connection;
pub mod hud;
pub mod leaderboard;
pub mod menu;
pub mod message;
pub mod screen;
pub mod title;

// fixtures shared by the unit tests of several modules; absent from every non-test build
#[cfg(test)]
pub mod test_fixture;

// browser runtime glue; the ssr build compiles none of it
#[cfg(feature = "hydrate")]
pub mod client;

// the dev server and the prerender; a browser runs neither
#[cfg(feature = "ssr")]
pub mod server;
```

Create `web/src/message/mod.rs`:

```rust
pub mod message_box;

pub use message_box::*;
```

Create `web/src/message/message_box.rs`:

```rust
use leptos::prelude::*;

/// Top left; the box takes the width of its text, as the DOM measures it.
#[component]
pub fn MessageBox(message_text: Signal<Option<String>>) -> impl IntoView {
    move || message_text.get().map(|message_text| view! { <p class="message-box">{message_text}</p> })
}
```

Create `web/style/components/_message.scss`:

```scss
/* Shrinks to its text, so the box is the DOM's measure of it; 25 px wider, as the original's. */
.message-box {
    position: absolute;
    left: 12.5px;
    top: 12px;
    padding: 5px 12.5px 3px;
    color: var(--color-white);
    font-family: var(--font-overlay);
    font-size: 14px;
    line-height: 18px;
    white-space: pre;
    background: var(--color-black);
    border: 1px solid var(--color-white);
    box-shadow: 5px 4px 0 var(--color-shadow);
}
```

Replace the whole contents of `web/style/main.scss` with:

```scss
@use "tokens";
@use "reset";
@use "typography";
@use "layout";
@use "components/menu";
@use "components/browser";
@use "components/title";
@use "components/ability-bar";
@use "components/name-labels";
@use "components/leaderboard";
@use "components/message";
```

- [ ] **Step 2: Build it**

Run: `cargo build -p web 2>&1 | grep -E '^(warning|error)|Finished'`

Expected: the build finishes without a warning:

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s)
```

- [ ] **Step 3: Commit**

```sh
git add web/src/lib.rs
git add web/src/message/mod.rs
git add web/src/message/message_box.rs
git add web/style/main.scss
git add web/style/components/_message.scss
git status --short
git commit -m "message box"
```

Expected `git status --short` before the commit:

```text
M  web/src/lib.rs
A  web/src/message/message_box.rs
A  web/src/message/mod.rs
A  web/style/components/_message.scss
M  web/style/main.scss
```

### Task 22: Scene overlay and tutorial view

What the overlays and the in-game menus show of the active scene: nothing over the title scene; the task text and, once the task shows it, the ability bar in the tutorial; the ability bar, leaderboard, message and menu view online.

**Files:**
- Create: `web/src/canvas/scene_overlay.rs`
- Create: `web/src/tutorial/mod.rs`
- Create: `web/src/tutorial/tutorial_view.rs`
- Modify: `web/src/lib.rs`
- Modify: `web/src/canvas/mod.rs`
- Test: inline `mod tests` in both new Rust files

- [ ] **Step 1: Write the failing tests**

Replace the whole contents of `web/src/lib.rs` with:

```rust
// The release view tree's monomorphized type exceeds rustc's default query depth.
#![recursion_limit = "512"]

pub mod app;
pub mod browser;
pub mod canvas;
pub mod connection;
pub mod hud;
pub mod leaderboard;
pub mod menu;
pub mod message;
pub mod screen;
pub mod title;
pub mod tutorial;

// fixtures shared by the unit tests of several modules; absent from every non-test build
#[cfg(test)]
pub mod test_fixture;

// browser runtime glue; the ssr build compiles none of it
#[cfg(feature = "hydrate")]
pub mod client;

// the dev server and the prerender; a browser runs neither
#[cfg(feature = "ssr")]
pub mod server;
```

Replace the whole contents of `web/src/canvas/mod.rs` with:

```rust
pub mod game_canvas;
pub mod scene_overlay;
// browser runtime glue; the ssr build compiles none of it
#[cfg(feature = "hydrate")]
pub mod driver;

pub use game_canvas::*;
pub use scene_overlay::*;
```

Create `web/src/tutorial/mod.rs`:

```rust
pub mod tutorial_view;

pub use tutorial_view::*;
```

Create `web/src/canvas/scene_overlay.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use shared::game::{GameId, GameModeKind, GameState};
    use shared::geometry::WorldPoint;
    use shared::member::MemberId;
    use shared::play::{PixelSize, PlayerControls};
    use shared::protocol;
    use shared::protocol::{GameSnapshotSerialOut, GameStateSerialOut, MessageSerialOut};
    use shared::title::TitleScene;

    use crate::test_fixture;

    const GAME_ID: GameId = GameId(4);
    const OWN_MEMBER_ID: MemberId = MemberId(0);

    fn create_session_joined(controls: &mut PlayerControls) -> ClientSession {
        let mut client_session: ClientSession = ClientSession::new();
        let join_accepted: MessageSerialOut = MessageSerialOut::JoinAccepted {
            game_id: GAME_ID.0,
            member_id: OWN_MEMBER_ID.0,
        };

        client_session.receive(&protocol::encode_message_out(&join_accepted), controls, 0.0);

        client_session
    }

    fn create_session_in_game(state: &GameState, controls: &mut PlayerControls) -> ClientSession {
        let mut client_session: ClientSession = create_session_joined(controls);
        let snapshot: MessageSerialOut = MessageSerialOut::Snapshot(GameSnapshotSerialOut {
            game_id: GAME_ID.0,
            state: GameStateSerialOut::from(state),
        });

        client_session.receive(&protocol::encode_message_out(&snapshot), controls, 0.0);

        client_session
    }

    #[test]
    fn from_active_scene_shows_nothing_over_the_title_scene() {
        let window_size: PixelSize = PixelSize {
            width_pixels: 800,
            height_pixels: 600,
        };
        let active_scene: ActiveScene = ActiveScene::Title(TitleScene::new(window_size, 1));

        assert_eq!(SceneOverlay::from_active_scene(&active_scene), SceneOverlay::empty());
    }

    #[test]
    fn from_active_scene_shows_nothing_before_the_first_snapshot() {
        let mut controls: PlayerControls = PlayerControls::new();
        let active_scene: ActiveScene = ActiveScene::Online(create_session_joined(&mut controls));

        assert_eq!(SceneOverlay::from_active_scene(&active_scene), SceneOverlay::empty());
    }

    #[test]
    fn from_active_scene_shows_the_ability_bar_leaderboard_and_menu_view_of_a_playing_member() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll);
        test_fixture::insert_member(
            &mut state,
            test_fixture::create_participant_with_organism(OWN_MEMBER_ID, WorldPoint { x: 200, y: 200 }),
        );
        let mut controls: PlayerControls = PlayerControls::new();
        let active_scene: ActiveScene = ActiveScene::Online(create_session_in_game(&state, &mut controls));

        let overlay: SceneOverlay = SceneOverlay::from_active_scene(&active_scene);

        assert_eq!(overlay.ability_bar, AbilityBarView::from_state(&state, OWN_MEMBER_ID));
        assert_eq!(
            overlay.leaderboard,
            Some(LeaderboardView::from_state(&state, OWN_MEMBER_ID)),
        );
        assert_eq!(overlay.game_menu, GameMenuView::from_state(&state, OWN_MEMBER_ID));
        assert_eq!(overlay.message_text, None);
    }
}
```

Create `web/src/tutorial/tutorial_view.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use shared::play::frame_pacing;
    use shared::play::{GameKeyKind, InputFocusKind, KeyRepeatKind, KeyTransitionKind, PixelSize, PlayerControls};
    use shared::tutorial::{DisplayModeKind, TutorialTaskKind};

    const SEED: u64 = 11;
    const WINDOW_SIZE: PixelSize = PixelSize {
        width_pixels: 850,
        height_pixels: 650,
    };

    #[test]
    fn get_tutorial_overlay_hides_the_ability_bar_while_moving() {
        let mut controls: PlayerControls = PlayerControls::new();
        let tutorial: Tutorial = Tutorial::new(WINDOW_SIZE, SEED, DisplayModeKind::Windowed, &mut controls);

        let overlay: SceneOverlay = get_tutorial_overlay(&tutorial);

        assert_eq!(overlay.ability_bar, None);
        assert_eq!(
            overlay.message_text.as_deref(),
            Some("Use W-A-S-D (Recommended) or the arrow keys to move"),
        );
    }

    #[test]
    fn get_tutorial_overlay_shows_the_ability_bar_from_the_fullscreen_task() {
        let mut controls: PlayerControls = PlayerControls::new();
        let mut tutorial: Tutorial = Tutorial::new(WINDOW_SIZE, SEED, DisplayModeKind::Windowed, &mut controls);
        controls
            .keyboard
            .key_changed(GameKeyKind::MoveUp, KeyTransitionKind::Pressed, KeyRepeatKind::Initial);
        tutorial.advance_frame(0.0, &mut controls, InputFocusKind::Game);
        tutorial.advance_frame(
            frame_pacing::TICK_PERIOD_MILLISECONDS,
            &mut controls,
            InputFocusKind::Game,
        );

        let overlay: SceneOverlay = get_tutorial_overlay(&tutorial);

        assert_eq!(tutorial.task(), TutorialTaskKind::Fullscreen);
        assert!(overlay.ability_bar.is_some());
        assert_eq!(overlay.leaderboard, None);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --lib -- canvas::scene_overlay:: tutorial::`

Expected: FAIL to compile:

```text
error[E0425]: cannot find type `ClientSession` in this scope
```

- [ ] **Step 3: Implement the overlays of each scene**

In `web/src/canvas/scene_overlay.rs`, replace everything above `#[cfg(test)]` with:

```rust
use shared::play::{ActiveScene, ClientSession};

use crate::hud::AbilityBarView;
use crate::leaderboard::LeaderboardView;
use crate::menu::GameMenuView;
use crate::tutorial::tutorial_view;

/// What the HTML overlays and the in-game menus show of the active scene.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneOverlay {
    pub ability_bar: Option<AbilityBarView>,
    pub leaderboard: Option<LeaderboardView>,
    /// Before any transient message replaces it.
    pub message_text: Option<String>,
    pub game_menu: Option<GameMenuView>,
}

impl SceneOverlay {
    pub fn empty() -> SceneOverlay {
        SceneOverlay {
            ability_bar: None,
            leaderboard: None,
            message_text: None,
            game_menu: None,
        }
    }

    pub fn from_active_scene(active_scene: &ActiveScene) -> SceneOverlay {
        match active_scene {
            ActiveScene::Online(client_session) => SceneOverlay::from_client_session(client_session),
            ActiveScene::Tutorial(tutorial) => tutorial_view::get_tutorial_overlay(tutorial),
            ActiveScene::Title(_) => SceneOverlay::empty(),
        }
    }

    fn from_client_session(client_session: &ClientSession) -> SceneOverlay {
        let message_text: Option<String> = client_session.message_text();
        let (Some(state), Some(own_member_id)) = (client_session.render_state(), client_session.own_member_id()) else {
            return SceneOverlay {
                ability_bar: None,
                leaderboard: None,
                message_text,
                game_menu: None,
            };
        };

        SceneOverlay {
            ability_bar: AbilityBarView::from_state(state, own_member_id),
            leaderboard: Some(LeaderboardView::from_state(state, own_member_id)),
            message_text,
            game_menu: GameMenuView::from_state(state, own_member_id),
        }
    }
}
```

In `web/src/tutorial/tutorial_view.rs`, replace everything above `#[cfg(test)]` with:

```rust
use shared::tutorial;
use shared::tutorial::Tutorial;

use crate::canvas::SceneOverlay;
use crate::hud::AbilityBarView;

/// The task text, and the ability bar once the task shows it.
pub fn get_tutorial_overlay(tutorial: &Tutorial) -> SceneOverlay {
    let ability_bar: Option<AbilityBarView> = if tutorial.is_hud_visible() {
        AbilityBarView::from_state(tutorial.state(), tutorial::TUTORIAL_PLAYER_ID)
    } else {
        None
    };

    SceneOverlay {
        ability_bar,
        leaderboard: None,
        message_text: Some(String::from(tutorial.message_text())),
        game_menu: None,
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p web --lib -- canvas::scene_overlay:: tutorial::`

Expected: PASS:

```text
test canvas::scene_overlay::tests::from_active_scene_shows_nothing_before_the_first_snapshot ... ok
test tutorial::tutorial_view::tests::get_tutorial_overlay_hides_the_ability_bar_while_moving ... ok
test canvas::scene_overlay::tests::from_active_scene_shows_nothing_over_the_title_scene ... ok
test canvas::scene_overlay::tests::from_active_scene_shows_the_ability_bar_leaderboard_and_menu_view_of_a_playing_member ... ok
test tutorial::tutorial_view::tests::get_tutorial_overlay_shows_the_ability_bar_from_the_fullscreen_task ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 69 filtered out
```

- [ ] **Step 5: Commit**

```sh
git add web/src/lib.rs
git add web/src/canvas/mod.rs
git add web/src/canvas/scene_overlay.rs
git add web/src/tutorial/mod.rs
git add web/src/tutorial/tutorial_view.rs
git status --short
git commit -m "scene overlay and tutorial view"
```

Expected `git status --short` before the commit:

```text
M  web/src/canvas/mod.rs
A  web/src/canvas/scene_overlay.rs
M  web/src/lib.rs
A  web/src/tutorial/mod.rs
A  web/src/tutorial/tutorial_view.rs
```

**Deviations:** as Task 10's note describes, `web/src/lib.rs` keeps no `test_fixture` module, and the scene overlay test imports `shared::game::test_fixture` and `shared::world::WorldShapeKind` and calls `create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800)`. The filtered run reports 89 filtered out.

### Task 23: Scene camera

Phase 7 left choosing the camera and the crosshair per scene to this phase: online the camera follows the crosshair, drawn only for a playing member; the title scene and the tutorial keep the world still, and only the tutorial draws a crosshair.

**Files:**
- Create: `web/src/canvas/scene_camera.rs`
- Modify: `web/src/canvas/mod.rs`
- Modify: `web/src/test_fixture/test_fixture.rs`
- Modify: `web/src/canvas/scene_overlay.rs`
- Test: inline `mod tests` in `web/src/canvas/scene_camera.rs`

- [ ] **Step 1: Write the failing tests**

Replace the whole contents of `web/src/canvas/mod.rs` with:

```rust
pub mod game_canvas;
pub mod scene_camera;
pub mod scene_overlay;
// browser runtime glue; the ssr build compiles none of it
#[cfg(feature = "hydrate")]
pub mod driver;

pub use game_canvas::*;
pub use scene_camera::*;
pub use scene_overlay::*;
```

The scene camera's tests are the second user of the scene overlay's session helpers, so they move to the test fixture. In `web/src/test_fixture/test_fixture.rs`, replace:

```rust
use shared::game::{GameModeKind, GameSettings, GameState};
use shared::geometry::WorldPoint;
use shared::member::{Appearance, Member, MemberId, MemberRoleKind, OrganismColorKind, Score, SkinKind};
use shared::organism::Organism;
use shared::world::WorldShapeKind;

pub const FIXTURE_SEED: u64 = 0x5eed;
```

with:

```rust
use shared::game::{GameId, GameModeKind, GameSettings, GameState};
use shared::geometry::WorldPoint;
use shared::member::{Appearance, Member, MemberId, MemberRoleKind, OrganismColorKind, Score, SkinKind};
use shared::organism::Organism;
use shared::play::{ClientSession, PlayerControls};
use shared::protocol;
use shared::protocol::{GameSnapshotSerialOut, GameStateSerialOut, MessageSerialOut};
use shared::world::WorldShapeKind;

pub const FIXTURE_SEED: u64 = 0x5eed;
pub const FIXTURE_GAME_ID: GameId = GameId(4);
```

Append to the end of `web/src/test_fixture/test_fixture.rs`, after one blank line:

```rust
pub fn create_session_joined(own_member_id: MemberId, controls: &mut PlayerControls) -> ClientSession {
    let mut client_session: ClientSession = ClientSession::new();
    let join_accepted: MessageSerialOut = MessageSerialOut::JoinAccepted {
        game_id: FIXTURE_GAME_ID.0,
        member_id: own_member_id.0,
    };

    client_session.receive(&protocol::encode_message_out(&join_accepted), controls, 0.0);

    client_session
}

pub fn create_session_in_game(
    state: &GameState,
    own_member_id: MemberId,
    controls: &mut PlayerControls,
) -> ClientSession {
    let mut client_session: ClientSession = create_session_joined(own_member_id, controls);
    let snapshot: MessageSerialOut = MessageSerialOut::Snapshot(GameSnapshotSerialOut {
        game_id: FIXTURE_GAME_ID.0,
        state: GameStateSerialOut::from(state),
    });

    client_session.receive(&protocol::encode_message_out(&snapshot), controls, 0.0);

    client_session
}
```

In `web/src/canvas/scene_overlay.rs`, replace:

```rust
    use super::*;
    use shared::game::{GameId, GameModeKind, GameState};
    use shared::geometry::WorldPoint;
    use shared::member::MemberId;
    use shared::play::{PixelSize, PlayerControls};
    use shared::protocol;
    use shared::protocol::{GameSnapshotSerialOut, GameStateSerialOut, MessageSerialOut};
    use shared::title::TitleScene;

    use crate::test_fixture;

    const GAME_ID: GameId = GameId(4);
    const OWN_MEMBER_ID: MemberId = MemberId(0);

    fn create_session_joined(controls: &mut PlayerControls) -> ClientSession {
        let mut client_session: ClientSession = ClientSession::new();
        let join_accepted: MessageSerialOut = MessageSerialOut::JoinAccepted {
            game_id: GAME_ID.0,
            member_id: OWN_MEMBER_ID.0,
        };

        client_session.receive(&protocol::encode_message_out(&join_accepted), controls, 0.0);

        client_session
    }

    fn create_session_in_game(state: &GameState, controls: &mut PlayerControls) -> ClientSession {
        let mut client_session: ClientSession = create_session_joined(controls);
        let snapshot: MessageSerialOut = MessageSerialOut::Snapshot(GameSnapshotSerialOut {
            game_id: GAME_ID.0,
            state: GameStateSerialOut::from(state),
        });

        client_session.receive(&protocol::encode_message_out(&snapshot), controls, 0.0);

        client_session
    }
```

with:

```rust
    use super::*;
    use shared::game::{GameModeKind, GameState};
    use shared::geometry::WorldPoint;
    use shared::member::MemberId;
    use shared::play::{PixelSize, PlayerControls};
    use shared::title::TitleScene;

    use crate::test_fixture;

    const OWN_MEMBER_ID: MemberId = MemberId(0);
```

In `web/src/canvas/scene_overlay.rs`, replace:

```rust
        let active_scene: ActiveScene = ActiveScene::Online(create_session_joined(&mut controls));
```

with:

```rust
        let client_session: ClientSession = test_fixture::create_session_joined(OWN_MEMBER_ID, &mut controls);
        let active_scene: ActiveScene = ActiveScene::Online(client_session);
```

In `web/src/canvas/scene_overlay.rs`, replace:

```rust
        let active_scene: ActiveScene = ActiveScene::Online(create_session_in_game(&state, &mut controls));
```

with:

```rust
        let client_session: ClientSession = test_fixture::create_session_in_game(&state, OWN_MEMBER_ID, &mut controls);
        let active_scene: ActiveScene = ActiveScene::Online(client_session);
```

Create `web/src/canvas/scene_camera.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use shared::game::{GameModeKind, GameState};
    use shared::geometry::{SubpixelPoint, WorldPoint};
    use shared::member::MemberId;
    use shared::play::{ClientSession, PixelSize};
    use shared::render::PhysicalSize;
    use shared::title::TitleScene;
    use shared::tutorial::{DisplayModeKind, Tutorial};

    use crate::test_fixture;

    const OWN_MEMBER_ID: MemberId = MemberId(0);
    const SEED: u64 = 5;
    const WINDOW_SIZE: PixelSize = PixelSize {
        width_pixels: 800,
        height_pixels: 600,
    };
    const VIEWPORT: ViewportSize = ViewportSize {
        physical_size: PhysicalSize {
            width_pixels: 1600,
            height_pixels: 1200,
        },
        device_pixel_ratio: 2.0,
    };

    fn create_controls_at(position: SubpixelPoint) -> PlayerControls {
        let mut controls: PlayerControls = PlayerControls::new();
        controls.crosshair.snap_to(position);

        controls
    }

    #[test]
    fn get_scene_view_follows_the_crosshair_online_without_drawing_it_outside_play() {
        let controls: PlayerControls = create_controls_at(SubpixelPoint {
            x: 1024 * 500,
            y: 1024 * 400,
        });
        let active_scene: ActiveScene = ActiveScene::Online(ClientSession::new());

        let scene_view: SceneView = get_scene_view(&active_scene, &controls, VIEWPORT);

        assert_eq!(
            scene_view.camera,
            Camera::centred_on(InterpolatedPoint { x: 500.0, y: 400.0 }, VIEWPORT),
        );
        assert_eq!(scene_view.crosshair, None);
    }

    #[test]
    fn get_scene_view_draws_the_crosshair_of_a_playing_member() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll);
        test_fixture::insert_member(
            &mut state,
            test_fixture::create_participant_with_organism(OWN_MEMBER_ID, WorldPoint { x: 200, y: 200 }),
        );
        let mut controls: PlayerControls = PlayerControls::new();
        let client_session: ClientSession = test_fixture::create_session_in_game(&state, OWN_MEMBER_ID, &mut controls);
        let active_scene: ActiveScene = ActiveScene::Online(client_session);

        let scene_view: SceneView = get_scene_view(&active_scene, &controls, VIEWPORT);

        assert_eq!(
            scene_view.crosshair,
            Some(InterpolatedPoint::from_subpixel_point(controls.crosshair.position())),
        );
    }

    #[test]
    fn get_scene_view_draws_no_crosshair_for_a_dead_participant() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll);
        test_fixture::insert_member(&mut state, test_fixture::create_participant(OWN_MEMBER_ID));
        let mut controls: PlayerControls = PlayerControls::new();
        let client_session: ClientSession = test_fixture::create_session_in_game(&state, OWN_MEMBER_ID, &mut controls);
        let active_scene: ActiveScene = ActiveScene::Online(client_session);

        let scene_view: SceneView = get_scene_view(&active_scene, &controls, VIEWPORT);

        assert_eq!(scene_view.crosshair, None);
    }

    #[test]
    fn get_scene_view_draws_the_tutorial_crosshair_over_a_still_world() {
        let mut controls: PlayerControls = PlayerControls::new();
        let tutorial: Tutorial = Tutorial::new(WINDOW_SIZE, SEED, DisplayModeKind::Windowed, &mut controls);
        let crosshair: InterpolatedPoint = InterpolatedPoint::from_subpixel_point(controls.crosshair.position());

        let scene_view: SceneView = get_scene_view(&ActiveScene::Tutorial(tutorial), &controls, VIEWPORT);

        assert_eq!(scene_view.camera, Camera::for_local_game(VIEWPORT));
        assert_eq!(scene_view.crosshair, Some(crosshair));
    }

    #[test]
    fn get_scene_view_draws_no_crosshair_over_the_title_scene() {
        let controls: PlayerControls = PlayerControls::new();
        let active_scene: ActiveScene = ActiveScene::Title(TitleScene::new(WINDOW_SIZE, SEED));

        let scene_view: SceneView = get_scene_view(&active_scene, &controls, VIEWPORT);

        assert_eq!(scene_view.camera, Camera::for_local_game(VIEWPORT));
        assert_eq!(scene_view.crosshair, None);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --lib canvas::scene_camera::`

Expected: FAIL to compile:

```text
error[E0425]: cannot find type `ViewportSize` in this scope
```

- [ ] **Step 3: Implement the scene view**

In `web/src/canvas/scene_camera.rs`, replace everything above `#[cfg(test)]` with:

```rust
use shared::play::{ActiveScene, InterpolatedPoint, OwnMemberStateKind, PlayerControls};
use shared::render::{Camera, ViewportSize};

/// The camera a frame is drawn with, and the crosshair it draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneView {
    pub camera: Camera,
    pub crosshair: Option<InterpolatedPoint>,
}

/// Online the camera follows the crosshair, drawn only for a playing member; local games keep the world still.
pub fn get_scene_view(active_scene: &ActiveScene, controls: &PlayerControls, viewport: ViewportSize) -> SceneView {
    let crosshair: InterpolatedPoint = InterpolatedPoint::from_subpixel_point(controls.crosshair.position());

    match active_scene {
        ActiveScene::Online(client_session) => SceneView {
            camera: Camera::centred_on(crosshair, viewport),
            crosshair: match client_session.own_member_state() {
                Some(OwnMemberStateKind::AliveParticipant) => Some(crosshair),
                Some(OwnMemberStateKind::DeadParticipant | OwnMemberStateKind::Spectator) | None => None,
            },
        },
        ActiveScene::Tutorial(_) => SceneView {
            camera: Camera::for_local_game(viewport),
            crosshair: Some(crosshair),
        },
        ActiveScene::Title(_) => SceneView {
            camera: Camera::for_local_game(viewport),
            crosshair: None,
        },
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p web --lib canvas::scene_camera::`

Expected: PASS:

```text
test canvas::scene_camera::tests::get_scene_view_follows_the_crosshair_online_without_drawing_it_outside_play ... ok
test canvas::scene_camera::tests::get_scene_view_draws_the_tutorial_crosshair_over_a_still_world ... ok
test canvas::scene_camera::tests::get_scene_view_draws_no_crosshair_over_the_title_scene ... ok
test canvas::scene_camera::tests::get_scene_view_draws_the_crosshair_of_a_playing_member ... ok
test canvas::scene_camera::tests::get_scene_view_draws_no_crosshair_for_a_dead_participant ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 74 filtered out
```

- [ ] **Step 5: Commit**

```sh
git add web/src/canvas/mod.rs
git add web/src/canvas/scene_camera.rs
git add web/src/canvas/scene_overlay.rs
git add web/src/test_fixture/test_fixture.rs
git status --short
git commit -m "scene camera"
```

Expected `git status --short` before the commit:

```text
M  web/src/canvas/mod.rs
A  web/src/canvas/scene_camera.rs
M  web/src/canvas/scene_overlay.rs
M  web/src/test_fixture/test_fixture.rs
```

### Task 24: Keyboard and pointer mapping

The original's keys by `KeyboardEvent.code` (A/W/S/D and the arrows, X, C, V, Space, R, Escape, F11), which keys keep the browser's default action, and the shot aim from the pointer.

**Files:**
- Create: `web/src/canvas/keyboard.rs`
- Modify: `web/src/canvas/mod.rs`
- Test: inline `mod tests` in `web/src/canvas/keyboard.rs`

- [ ] **Step 1: Write the failing tests**

Replace the whole contents of `web/src/canvas/mod.rs` with:

```rust
pub mod game_canvas;
pub mod keyboard;
pub mod scene_camera;
pub mod scene_overlay;
// browser runtime glue; the ssr build compiles none of it
#[cfg(feature = "hydrate")]
pub mod driver;

pub use game_canvas::*;
pub use keyboard::*;
pub use scene_camera::*;
pub use scene_overlay::*;
```

Create `web/src/canvas/keyboard.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_game_key_maps_the_original_keys_and_arrows() {
        assert_eq!(get_game_key("KeyW"), Some(GameKeyKind::MoveUp));
        assert_eq!(get_game_key("ArrowUp"), Some(GameKeyKind::MoveUp));
        assert_eq!(get_game_key("KeyA"), Some(GameKeyKind::MoveLeft));
        assert_eq!(get_game_key("ArrowDown"), Some(GameKeyKind::MoveDown));
        assert_eq!(get_game_key("KeyX"), Some(GameKeyKind::FirstAbility));
        assert_eq!(get_game_key("KeyC"), Some(GameKeyKind::SecondAbility));
        assert_eq!(get_game_key("KeyV"), Some(GameKeyKind::ThirdAbility));
        assert_eq!(get_game_key("Space"), Some(GameKeyKind::FourthAbility));
        assert_eq!(get_game_key("KeyR"), Some(GameKeyKind::Respawn));
        assert_eq!(get_game_key("Escape"), Some(GameKeyKind::Pause));
        assert_eq!(get_game_key("F11"), Some(GameKeyKind::Fullscreen));
        assert_eq!(get_game_key("KeyQ"), None);
    }

    #[test]
    fn get_key_repeat_tells_an_auto_repeat_apart() {
        assert_eq!(get_key_repeat(true), KeyRepeatKind::Repeated);
        assert_eq!(get_key_repeat(false), KeyRepeatKind::Initial);
    }

    #[test]
    fn get_key_default_action_keeps_the_browser_fullscreen_key() {
        assert_eq!(
            get_key_default_action(GameKeyKind::FourthAbility),
            KeyDefaultActionKind::Prevented,
        );
        assert_eq!(
            get_key_default_action(GameKeyKind::MoveLeft),
            KeyDefaultActionKind::Prevented,
        );
        assert_eq!(
            get_key_default_action(GameKeyKind::Fullscreen),
            KeyDefaultActionKind::Allowed,
        );
    }

    #[test]
    fn get_aim_vector_is_the_rounded_offset_from_the_crosshair() {
        assert_eq!(
            get_aim_vector(ViewPoint { x: 410.6, y: 290.0 }, ViewPoint { x: 400.0, y: 300.0 }),
            AimVector { x: 11, y: -10 },
        );
        assert_eq!(
            get_aim_vector(ViewPoint { x: 1.0e9, y: -1.0e9 }, ViewPoint { x: 0.0, y: 0.0 }),
            AimVector {
                x: i16::MAX,
                y: i16::MIN,
            },
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --lib canvas::keyboard::`

Expected: FAIL to compile:

```text
error[E0422]: cannot find struct, variant or union type `ViewPoint` in this scope
```

- [ ] **Step 3: Implement the mapping**

In `web/src/canvas/keyboard.rs`, replace everything above `#[cfg(test)]` with:

```rust
use shared::ability::AimVector;
use shared::play::{GameKeyKind, KeyRepeatKind};
use shared::render::ViewPoint;

/// Whether the browser may also act on a key the game uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyDefaultActionKind {
    Allowed,
    Prevented,
}

/// `KeyboardEvent.code`, so the keys sit where the original's did on any layout.
pub fn get_game_key(code: &str) -> Option<GameKeyKind> {
    match code {
        "KeyA" | "ArrowLeft" => Some(GameKeyKind::MoveLeft),
        "KeyW" | "ArrowUp" => Some(GameKeyKind::MoveUp),
        "KeyD" | "ArrowRight" => Some(GameKeyKind::MoveRight),
        "KeyS" | "ArrowDown" => Some(GameKeyKind::MoveDown),
        "KeyX" => Some(GameKeyKind::FirstAbility),
        "KeyC" => Some(GameKeyKind::SecondAbility),
        "KeyV" => Some(GameKeyKind::ThirdAbility),
        "Space" => Some(GameKeyKind::FourthAbility),
        "KeyR" => Some(GameKeyKind::Respawn),
        "Escape" => Some(GameKeyKind::Pause),
        "F11" => Some(GameKeyKind::Fullscreen),
        _ => None,
    }
}

pub fn get_key_repeat(is_repeat: bool) -> KeyRepeatKind {
    if is_repeat {
        KeyRepeatKind::Repeated
    } else {
        KeyRepeatKind::Initial
    }
}

/// Arrows and space would scroll or press a focused button; F11 stays the browser's fullscreen key.
pub fn get_key_default_action(key: GameKeyKind) -> KeyDefaultActionKind {
    match key {
        GameKeyKind::MoveLeft
        | GameKeyKind::MoveUp
        | GameKeyKind::MoveRight
        | GameKeyKind::MoveDown
        | GameKeyKind::FirstAbility
        | GameKeyKind::SecondAbility
        | GameKeyKind::ThirdAbility
        | GameKeyKind::FourthAbility => KeyDefaultActionKind::Prevented,
        GameKeyKind::Respawn | GameKeyKind::Pause | GameKeyKind::Fullscreen => KeyDefaultActionKind::Allowed,
    }
}

/// The pointer's offset from the crosshair on screen, in CSS pixels, saturated to the wire range.
pub fn get_aim_vector(pointer: ViewPoint, crosshair: ViewPoint) -> AimVector {
    AimVector {
        x: get_saturated_aim_coordinate(pointer.x - crosshair.x),
        y: get_saturated_aim_coordinate(pointer.y - crosshair.y),
    }
}

fn get_saturated_aim_coordinate(offset_pixels: f32) -> i16 {
    offset_pixels.round().clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p web --lib canvas::keyboard::`

Expected: PASS:

```text
test canvas::keyboard::tests::get_aim_vector_is_the_rounded_offset_from_the_crosshair ... ok
test canvas::keyboard::tests::get_game_key_maps_the_original_keys_and_arrows ... ok
test canvas::keyboard::tests::get_key_default_action_keeps_the_browser_fullscreen_key ... ok
test canvas::keyboard::tests::get_key_repeat_tells_an_auto_repeat_apart ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 79 filtered out
```

- [ ] **Step 5: Commit**

```sh
git add web/src/canvas/mod.rs
git add web/src/canvas/keyboard.rs
git status --short
git commit -m "keyboard and pointer mapping"
```

Expected `git status --short` before the commit:

```text
A  web/src/canvas/keyboard.rs
M  web/src/canvas/mod.rs
```

### Task 25: Browser glue

`LocalStorageSettingsStore` (values as hexadecimal under `bacter.` keys), the display mode by the original's test, `game-server.json` read with `fetch` and `JSON.parse`, and `GameSocket`, a binary WebSocket whose handlers live as long as it does.

**Files:**
- Create: `web/src/client/local_storage_settings.rs`
- Create: `web/src/client/fullscreen.rs`
- Create: `web/src/client/game_server_config.rs`
- Create: `web/src/client/game_socket.rs`
- Create: `scripts/test/test-wasm.sh`
- Modify: `web/src/lib.rs`
- Modify: `web/src/client/mod.rs`
- Test: `#[wasm_bindgen_test]` module in `web/src/client/local_storage_settings.rs`

- [ ] **Step 1: Write the failing localStorage tests and the script which runs them**

Replace the whole contents of `web/src/lib.rs` with:

```rust
// The release view tree's monomorphized type exceeds rustc's default query depth.
#![recursion_limit = "512"]

pub mod app;
pub mod browser;
pub mod canvas;
pub mod connection;
pub mod hud;
pub mod leaderboard;
pub mod menu;
pub mod message;
pub mod screen;
pub mod title;
pub mod tutorial;

// fixtures shared by the unit tests of several modules; absent from every non-test build
#[cfg(test)]
pub mod test_fixture;

// browser runtime glue; the ssr build compiles none of it
#[cfg(feature = "hydrate")]
pub mod client;

// the dev server and the prerender; a browser runs neither
#[cfg(feature = "ssr")]
pub mod server;

// other targets have no browser to run #[wasm_bindgen_test] cases in
#[cfg(all(test, target_arch = "wasm32"))]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);
```

Replace the whole contents of `web/src/client/mod.rs` with:

```rust
//! Browser runtime glue; compiled only under the `hydrate` feature.

pub mod hydrate;
pub mod local_storage_settings;

pub use hydrate::*;
pub use local_storage_settings::*;
```

Create `web/src/client/local_storage_settings.rs`:

```rust
#[cfg(all(test, target_arch = "wasm32"))] // reads and writes the browser's localStorage
mod tests {
    use super::*;
    use shared::settings;
    use shared::settings::ClientSettingKind;
    use wasm_bindgen_test::wasm_bindgen_test;

    fn clear_stored_settings() {
        let storage: Storage = get_local_storage().unwrap();

        for key in [ClientSettingKind::NameLabels, ClientSettingKind::Messages] {
            storage.remove_item(&get_storage_key(key)).unwrap();
        }
    }

    #[wasm_bindgen_test]
    fn load_returns_the_stored_value() {
        clear_stored_settings();
        let store: LocalStorageSettingsStore = LocalStorageSettingsStore;

        store.store(ClientSettingKind::NameLabels, SettingValue::Bool(false));

        assert_eq!(
            store.load(ClientSettingKind::NameLabels),
            Some(SettingValue::Bool(false)),
        );
        assert!(!settings::NAME_LABELS_SETTING.read(&store));
    }

    #[wasm_bindgen_test]
    fn load_returns_none_for_a_missing_value() {
        clear_stored_settings();

        assert_eq!(LocalStorageSettingsStore.load(ClientSettingKind::Messages), None);
        assert!(settings::MESSAGES_SETTING.read(&LocalStorageSettingsStore));
    }

    #[wasm_bindgen_test]
    fn load_returns_none_for_a_corrupt_value() {
        clear_stored_settings();
        let storage: Storage = get_local_storage().unwrap();

        storage.set_item(&get_storage_key(ClientSettingKind::Messages), "0g").unwrap();

        assert_eq!(LocalStorageSettingsStore.load(ClientSettingKind::Messages), None);
    }
}
```

Create `scripts/test/test-wasm.sh`:

```sh
#!/usr/bin/env bash
# Runs the web crate's #[wasm_bindgen_test] cases in headless Chrome.
# wasm-pack needs a package manifest, so this runs from web/.
#
# Extra arguments are passed to wasm-pack, for example:
#   ./scripts/test/test-wasm.sh -- load_returns

set -euo pipefail

readonly MISSING_PROGRAM_EXIT_STATUS=1

function require_program {
    local program_name="$1"
    local install_hint="$2"

    local program_path
    program_path=$(command -v "${program_name}" || true)

    if test -z "${program_path}"; then
        echo "error: the \`${program_name}\` program is required; install it with: ${install_hint}" >&2
        exit "${MISSING_PROGRAM_EXIT_STATUS}"
    fi
}

require_program "wasm-pack" "cargo install wasm-pack"
require_program "chromedriver" "a build matching the installed Chrome major version, on PATH"

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}/web"

exec wasm-pack test --headless --chrome --features hydrate "$@"
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `chmod +x scripts/test/test-wasm.sh && cargo test -p web --target wasm32-unknown-unknown --features hydrate --no-run`

Expected: FAIL to compile (`test-wasm.sh` runs the same build first):

```text
error[E0425]: cannot find type `Storage` in this scope
```

- [ ] **Step 3: Implement the store**

In `web/src/client/local_storage_settings.rs`, replace everything above `#[cfg(all(test, target_arch = "wasm32"))]` with:

```rust
use std::fmt::Write;

use wasm_bindgen::JsValue;
use web_sys::Storage;

use shared::settings::{SettingKey, SettingValue, SettingsStore};

const STORAGE_KEY_PREFIX: &str = "bacter.";

/// localStorage holds strings, so each value's bytes are stored as hexadecimal.
pub struct LocalStorageSettingsStore;

impl SettingsStore for LocalStorageSettingsStore {
    fn load<K: SettingKey>(&self, key: K) -> Option<SettingValue> {
        let storage: Storage = get_local_storage()?;
        let stored_text: String = storage.get_item(&get_storage_key(key)).ok()??;
        let bytes: Vec<u8> = decode_hexadecimal(&stored_text)?;

        SettingValue::from_bytes(&bytes)
    }

    fn store<K: SettingKey>(&self, key: K, value: SettingValue) {
        let Some(storage) = get_local_storage() else {
            log::warn!("settings not stored: localStorage is unavailable");

            return;
        };

        let stored: Result<(), JsValue> =
            storage.set_item(&get_storage_key(key), &encode_hexadecimal(&value.to_bytes()));

        if let Err(error) = stored {
            log::warn!("storing a setting failed; [key={} error={error:?}]", key.storage_key());
        }
    }
}

fn get_local_storage() -> Option<Storage> {
    web_sys::window()?.local_storage().ok()?
}

fn get_storage_key<K: SettingKey>(key: K) -> String {
    format!("{STORAGE_KEY_PREFIX}{}", key.storage_key())
}

fn encode_hexadecimal(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");

        text
    })
}

/// `None` for an odd length or a character outside hexadecimal.
fn decode_hexadecimal(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }

    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(text.get(index..index + 2)?, 16).ok())
        .collect()
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `./scripts/test/test-wasm.sh`

Expected, in headless Chrome:

```text
test client::local_storage_settings::tests::load_returns_the_stored_value ... ok
test client::local_storage_settings::tests::load_returns_none_for_a_missing_value ... ok
test client::local_storage_settings::tests::load_returns_none_for_a_corrupt_value ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 filtered out
```

`chromedriver` must match the installed Chrome's major version; a mismatch fails with `Error: http status: 404` before any test runs. With a mismatched driver, run the same tests in a visible browser instead: from `web/`, `NO_HEADLESS=1 WASM_BINDGEN_TEST_ADDRESS=127.0.0.1:8123 CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner cargo test --target wasm32-unknown-unknown --features hydrate`, open `http://127.0.0.1:8123` in Chrome, read the same three `ok` lines, then stop the runner by its PID. Behind an HTTP proxy, `NO_PROXY=127.0.0.1,localhost` keeps the driver's loopback traffic direct.

- [ ] **Step 5: Add the display mode, the game server config and the socket**

Create `web/src/client/fullscreen.rs`:

```rust
use shared::tutorial::DisplayModeKind;

/// The original's test: a fullscreen window has no browser chrome above the page.
pub fn get_display_mode() -> DisplayModeKind {
    let Some(window) = web_sys::window() else {
        return DisplayModeKind::Windowed;
    };

    let outer_height: f64 = window.outer_height().ok().and_then(|height| height.as_f64()).unwrap_or(0.0);
    let inner_height: f64 = window.inner_height().ok().and_then(|height| height.as_f64()).unwrap_or(0.0);

    if outer_height - inner_height < 1.0 {
        DisplayModeKind::Fullscreen
    } else {
        DisplayModeKind::Windowed
    }
}
```

Create `web/src/client/game_server_config.rs`:

```rust
use js_sys::{JSON, Promise, Reflect};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Response, Window};

use shared::error::AppError;

use crate::connection;
use crate::connection::PageOrigin;

const GAME_SERVER_CONFIG_PATH: &str = "/game-server.json";
const WEBSOCKET_URL_FIELD: &str = "websocket_url";

/// The socket URL `game-server.json` names, versioned; a null URL is the page's own origin.
pub async fn read_websocket_url() -> Result<String, AppError> {
    let window: Window = get_window()?;
    let configured_websocket_url: Option<String> = read_configured_websocket_url(&window).await?;
    let page_origin: PageOrigin = get_page_origin(&window)?;

    Ok(connection::resolve_websocket_url(
        configured_websocket_url.as_deref(),
        &page_origin,
    ))
}

async fn read_configured_websocket_url(window: &Window) -> Result<Option<String>, AppError> {
    let response_value: JsValue = JsFuture::from(window.fetch_with_str(GAME_SERVER_CONFIG_PATH))
        .await
        .map_err(|error| get_js_error("fetching the game server config failed", &error))?;
    let response: Response = response_value
        .dyn_into::<Response>()
        .map_err(|error| get_js_error("the game server config fetch gave no response", &error))?;

    if !response.ok() {
        return Err(AppError::new(&format!(
            "the game server config is unavailable; [status={}]",
            response.status(),
        )));
    }

    let text_promise: Promise =
        response.text().map_err(|error| get_js_error("reading the game server config failed", &error))?;
    let text_value: JsValue = JsFuture::from(text_promise)
        .await
        .map_err(|error| get_js_error("reading the game server config failed", &error))?;
    let text: String = text_value.as_string().unwrap_or_default();
    let config: JsValue =
        JSON::parse(&text).map_err(|error| get_js_error("the game server config is not JSON", &error))?;
    let websocket_url_value: JsValue = Reflect::get(&config, &JsValue::from_str(WEBSOCKET_URL_FIELD))
        .map_err(|error| get_js_error("the game server config is not a JSON object", &error))?;

    if websocket_url_value.is_undefined() {
        return Err(AppError::new("the game server config has no websocket_url"));
    }

    Ok(websocket_url_value.as_string())
}

fn get_page_origin(window: &Window) -> Result<PageOrigin, AppError> {
    let protocol: String = window
        .location()
        .protocol()
        .map_err(|error| get_js_error("reading the page protocol failed", &error))?;
    let host: String =
        window.location().host().map_err(|error| get_js_error("reading the page host failed", &error))?;

    Ok(PageOrigin { protocol, host })
}

fn get_window() -> Result<Window, AppError> {
    web_sys::window().ok_or_else(|| AppError::new("the page has no window"))
}

fn get_js_error(message: &str, error: &JsValue) -> AppError {
    AppError::new(&format!("{message}; [error={error:?}]"))
}
```

Create `web/src/client/game_socket.rs`:

```rust
use js_sys::{ArrayBuffer, Uint8Array};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{BinaryType, CloseEvent, Event, MessageEvent, WebSocket};

use shared::error::AppError;

const OPEN_EVENT: &str = "open";
const MESSAGE_EVENT: &str = "message";
const CLOSE_EVENT: &str = "close";

/// A binary WebSocket whose handlers live as long as it does.
pub struct GameSocket {
    websocket: WebSocket,
    open_listener: Closure<dyn FnMut(Event)>,
    message_listener: Closure<dyn FnMut(MessageEvent)>,
    close_listener: Closure<dyn FnMut(CloseEvent)>,
}

impl GameSocket {
    /// A failed connection reports itself through `on_close` with code 1006.
    pub fn open(
        url: &str,
        mut on_open: impl FnMut() + 'static,
        mut on_message: impl FnMut(Vec<u8>) + 'static,
        mut on_close: impl FnMut(u16) + 'static,
    ) -> Result<GameSocket, AppError> {
        let websocket: WebSocket = WebSocket::new(url)
            .map_err(|error| AppError::new(&format!("opening the game socket failed; [url={url} error={error:?}]")))?;
        websocket.set_binary_type(BinaryType::Arraybuffer);

        let open_listener: Closure<dyn FnMut(Event)> = Closure::new(move |_event: Event| on_open());
        let message_listener: Closure<dyn FnMut(MessageEvent)> = Closure::new(move |event: MessageEvent| {
            let frame: Option<Vec<u8>> = get_binary_frame(&event.data());

            match frame {
                Some(frame) => on_message(frame),
                None => log::warn!("text frame from the game server dropped"),
            }
        });
        let close_listener: Closure<dyn FnMut(CloseEvent)> =
            Closure::new(move |event: CloseEvent| on_close(event.code()));

        let game_socket: GameSocket = GameSocket {
            websocket,
            open_listener,
            message_listener,
            close_listener,
        };
        game_socket.add_listeners()?;

        Ok(game_socket)
    }

    pub fn send(&self, frame: &[u8]) {
        let sent: Result<(), JsValue> = self.websocket.send_with_u8_array(frame);

        if let Err(error) = sent {
            log::warn!("sending to the game server failed; [error={error:?}]");
        }
    }

    fn get_listeners(&self) -> [(&'static str, &JsValue); 3] {
        [
            (OPEN_EVENT, self.open_listener.as_ref()),
            (MESSAGE_EVENT, self.message_listener.as_ref()),
            (CLOSE_EVENT, self.close_listener.as_ref()),
        ]
    }

    fn add_listeners(&self) -> Result<(), AppError> {
        for (event_type, listener) in self.get_listeners() {
            self.websocket
                .add_event_listener_with_callback(event_type, listener.unchecked_ref())
                .map_err(|error| AppError::new(&format!("listening on the game socket failed; [error={error:?}]")))?;
        }

        Ok(())
    }
}

impl Drop for GameSocket {
    /// Detaches the handlers before they are freed; closing an already closed socket does nothing.
    fn drop(&mut self) {
        for (event_type, listener) in self.get_listeners() {
            let _ = self.websocket.remove_event_listener_with_callback(event_type, listener.unchecked_ref());
        }

        let _ = self.websocket.close();
    }
}

fn get_binary_frame(data: &JsValue) -> Option<Vec<u8>> {
    let array_buffer: &ArrayBuffer = data.dyn_ref::<ArrayBuffer>()?;

    Some(Uint8Array::new(array_buffer).to_vec())
}
```

Replace the whole contents of `web/src/client/mod.rs` with:

```rust
//! Browser runtime glue; compiled only under the `hydrate` feature.

pub mod fullscreen;
pub mod game_server_config;
pub mod game_socket;
pub mod hydrate;
pub mod local_storage_settings;

pub use fullscreen::*;
pub use game_server_config::*;
pub use game_socket::*;
pub use hydrate::*;
pub use local_storage_settings::*;
```

- [ ] **Step 6: Check the browser build**

Run: `./scripts/test/check-wasm.sh`

Expected: the three checks finish without a warning:

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s)
    Finished `dev` profile [unoptimized + debuginfo] target(s)
    Finished `dev` profile [unoptimized + debuginfo] target(s)
```

- [ ] **Step 7: Commit**

```sh
git add web/src/lib.rs
git add web/src/client/mod.rs
git add web/src/client/local_storage_settings.rs
git add web/src/client/fullscreen.rs
git add web/src/client/game_server_config.rs
git add web/src/client/game_socket.rs
git add scripts/test/test-wasm.sh
git status --short
git commit -m "browser glue"
```

Expected `git status --short` before the commit:

```text
A  scripts/test/test-wasm.sh
A  web/src/client/fullscreen.rs
A  web/src/client/game_server_config.rs
A  web/src/client/game_socket.rs
A  web/src/client/local_storage_settings.rs
M  web/src/client/mod.rs
M  web/src/lib.rs
```

### Task 26: App layers and screens

The overlay signals join `AppSignals`; `App` stacks the overlays (z -2, above where the canvas will be), the shade behind menus and the browser (z -1) and the screen's UI (z 1), and renders the screen from a `Memo` so equal screens do not remount.

**Files:**
- Modify: `web/src/app.rs`
- Modify: `web/tests/prerender.rs`
- Test: `web/tests/prerender.rs`

- [ ] **Step 1: Write the failing test**

In `web/tests/prerender.rs`, replace:

```rust
    assert!(document.contains("<title>Bacter</title>"));
}
```

with:

```rust
    assert!(document.contains("<title>Bacter</title>"));
}

#[tokio::test]
async fn prerender_document_opens_on_the_title_screen() {
    let document: String = get_prerendered_document().await;

    assert!(document.contains(r#"<div class="title-menu">"#));
    assert!(document.contains("Connecting to the game server"));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --features ssr --test prerender`

Expected: FAIL: the document has no title menu yet:

```text
test prerender_document_opens_on_the_title_screen ... FAILED
test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

- [ ] **Step 3: Mount the overlays, the shade and the screens**

In `web/src/app.rs`, replace:

```rust
use shared::settings::SettingsStore;

use crate::browser::GameListView;
use crate::connection::ConnectionStatusKind;
use crate::menu::{CreateMenuFields, GameMenuView, RequestRejection};
use crate::screen::{LobbyNoticeKind, Screen};

#[cfg(feature = "ssr")] // a browser build renders no document head
```

with:

```rust
use shared::settings::SettingsStore;

use crate::browser::{GameBrowser, GameListView};
use crate::connection::ConnectionStatusKind;
use crate::hud::{AbilityBar, AbilityBarView, NameLabelView, NameLabels};
use crate::leaderboard::{Leaderboard, LeaderboardView};
use crate::menu::{
    CreateMenu, CreateMenuFields, GameMenuView, JoinMenu, PauseGameMenu, PauseSpectateMenu, PauseTutorialMenu,
    RequestRejection, RespawnMenu, SpectateMenu,
};
use crate::message::MessageBox;
use crate::screen::{LobbyNoticeKind, Screen, ScreenBackdropKind};
use crate::title::TitleMenu;

#[cfg(feature = "ssr")] // a browser build renders no document head
```

In `web/src/app.rs`, replace:

```rust
    pub own_member_state: RwSignal<Option<OwnMemberStateKind>>,
    pub game_menu: RwSignal<Option<GameMenuView>>,
    pub client_settings: RwSignal<ClientSettings>,
    /// The team choice last sent in a join or respawn.
```

with:

```rust
    pub own_member_state: RwSignal<Option<OwnMemberStateKind>>,
    pub game_menu: RwSignal<Option<GameMenuView>>,
    pub ability_bar: RwSignal<Option<AbilityBarView>>,
    pub leaderboard: RwSignal<Option<LeaderboardView>>,
    /// `None` while the Messages setting is off.
    pub message_text: RwSignal<Option<String>>,
    pub name_labels: RwSignal<Vec<NameLabelView>>,
    pub client_settings: RwSignal<ClientSettings>,
    /// The team choice last sent in a join or respawn.
```

In `web/src/app.rs`, replace:

```rust
            own_member_state: RwSignal::new(None),
            game_menu: RwSignal::new(None),
            client_settings: RwSignal::new(ClientSettings::defaults()),
            team_choice: RwSignal::new(TeamChoiceKindSerialIn::Auto),
```

with:

```rust
            own_member_state: RwSignal::new(None),
            game_menu: RwSignal::new(None),
            ability_bar: RwSignal::new(None),
            leaderboard: RwSignal::new(None),
            message_text: RwSignal::new(None),
            name_labels: RwSignal::new(Vec::new()),
            client_settings: RwSignal::new(ClientSettings::defaults()),
            team_choice: RwSignal::new(TeamChoiceKindSerialIn::Auto),
```

In `web/src/app.rs`, replace:

```rust
}

#[component]
pub fn App() -> impl IntoView {
```

with:

```rust
}

/// Canvas and overlays (z -2), the shade behind menus (z -1), the screen's UI (z 1).
#[component]
pub fn App() -> impl IntoView {
```

In `web/src/app.rs`, replace:

```rust
    provide_context(CreateMenuFields::new());

    view! { <div class="screen-layer"></div> }
}

/// Writes only a different value, so views reading the signal do not re-render for nothing.
```

with:

```rust
    provide_context(CreateMenuFields::new());

    let screen: Memo<Screen> = Memo::new(move |_| signals.screen.get());
    let is_shaded = move || screen.get().backdrop() == ScreenBackdropKind::Shaded;

    view! {
        <div class="overlay-layer">
            <NameLabels name_labels=signals.name_labels.into() />
            <Leaderboard leaderboard=signals.leaderboard.into() />
            <MessageBox message_text=signals.message_text.into() />
            <AbilityBar ability_bar=signals.ability_bar.into() />
        </div>
        <Show when=is_shaded>
            <div class="shade"></div>
        </Show>
        <div class="screen-layer">{move || render_screen(screen.get())}</div>
    }
}

/// Writes only a different value, so views reading the signal do not re-render for nothing.
```

In `web/src/app.rs`, replace:

```rust
    if is_changed {
        signal.set(value);
    }
}
```

with:

```rust
    if is_changed {
        signal.set(value);
    }
}

fn render_screen(screen: Screen) -> AnyView {
    match screen {
        Screen::Title => view! { <TitleMenu /> }.into_any(),
        Screen::Browser => view! { <GameBrowser /> }.into_any(),
        Screen::CreateMenu => view! { <CreateMenu /> }.into_any(),
        Screen::JoinMenu { origin } => view! { <JoinMenu origin=origin /> }.into_any(),
        Screen::SpectateMenu { game_id } => view! { <SpectateMenu game_id=game_id /> }.into_any(),
        Screen::RespawnMenu => view! { <RespawnMenu /> }.into_any(),
        Screen::PauseGame => view! { <PauseGameMenu /> }.into_any(),
        Screen::PauseSpectate => view! { <PauseSpectateMenu /> }.into_any(),
        Screen::PauseTutorial => view! { <PauseTutorialMenu /> }.into_any(),
        Screen::Playing | Screen::Spectating | Screen::Tutorial => ().into_any(),
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p web --features ssr --test prerender`

Expected: PASS:

```text
test prerender_document_names_the_game_in_the_tab ... ok
test prerender_document_references_the_client_bundle ... ok
test prerender_document_opens_on_the_title_screen ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

- [ ] **Step 5: Commit**

```sh
git add web/src/app.rs
git add web/tests/prerender.rs
git status --short
git commit -m "app layers and screens"
```

Expected `git status --short` before the commit:

```text
M  web/src/app.rs
M  web/tests/prerender.rs
```

### Task 27: Driver and game canvas

The driver ([client session, frame pacing and driving](../architecture/client-web.md#client-session-frame-pacing-and-driving)): `start` creates the renderer from the canvas (`?renderer=webgl2` forces WebGL2), a title scene with a seed from `Math.random`, the window listeners (keys, pointer, resize, blur) and the animation frame loop, then reads `game-server.json` and opens the socket. Every frame advances the active scene with the screen's input focus, uploads the tick layer when a tick was applied and draws, then publishes the overlays; every socket frame steps the session and turns its effects into socket frames, signals and screens. `GameCanvas` starts it once the canvas is mounted, and `App` mounts the canvas under every other layer.

**Files:**
- Modify: `web/src/canvas/driver.rs`
- Modify: `web/src/canvas/game_canvas.rs`
- Modify: `web/src/app.rs`
- Modify: `web/tests/prerender.rs`
- Test: `web/tests/prerender.rs`; the game itself in the browser

- [ ] **Step 1: Write the failing test**

In `web/tests/prerender.rs`, replace:

```rust
    assert!(document.contains("Connecting to the game server"));
}
```

with:

```rust
    assert!(document.contains("Connecting to the game server"));
}

#[tokio::test]
async fn prerender_document_contains_the_canvas_root() {
    let document: String = get_prerendered_document().await;

    assert!(document.contains(r#"<canvas class="game-canvas""#));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p web --features ssr --test prerender`

Expected: FAIL: the document has no canvas yet:

```text
test prerender_document_contains_the_canvas_root ... FAILED
test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

- [ ] **Step 3: Implement the driver**

Replace the whole contents of `web/src/canvas/driver.rs` with:

```rust
use std::cell::RefCell;
use std::collections::VecDeque;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Event, HtmlCanvasElement, KeyboardEvent, MouseEvent, Window};

use leptos::prelude::*;

use shared::error::AppError;
use shared::play::message_text;
use shared::play::own_member_state;
use shared::play::{
    ActiveScene, ClientEffect, ClientSession, FrameOutput, GameKeyKind, InputFocusKind, InterpolatedPoint,
    InterpolationFrame, KeyRepeatKind, KeyTransitionKind, PixelSize, PlayerControls, RespawnKeyOutcomeKind,
    TransientMessageKind,
};
use shared::protocol::CloseReasonKind;
use shared::render::{GameRenderer, PhysicalSize, RenderScene, RendererBackendKind, ViewPoint, ViewportSize};
use shared::title::TitleScene;
use shared::tutorial::Tutorial;

use crate::app;
use crate::app::{AppSignals, ClientSettings};
use crate::browser::GameListView;
use crate::canvas::keyboard;
use crate::canvas::scene_camera;
use crate::canvas::{DriverCommandKind, KeyDefaultActionKind, SceneOverlay, SceneView};
use crate::client;
use crate::client::{GameSocket, LocalStorageSettingsStore};
use crate::connection::ConnectionStatusKind;
use crate::hud;
use crate::hud::NameLabelView;
use crate::menu::RequestRejection;
use crate::screen::{LobbyNoticeKind, Screen};

thread_local! {
    static DRIVER: RefCell<Option<Driver>> = const { RefCell::new(None) };
    /// Commands wait here while the driver is busy or not yet started.
    static PENDING_DRIVER_COMMANDS: RefCell<VecDeque<DriverCommandKind>> = const { RefCell::new(VecDeque::new()) };
    static ANIMATION_FRAME_CALLBACK: RefCell<Option<Closure<dyn FnMut(f64)>>> = const { RefCell::new(None) };
}

const FORCE_WEBGL2_QUERY_PARAMETER: &str = "renderer=webgl2";
/// `Math.random` gives 53 random bits.
const RANDOM_SEED_SCALE: f64 = 9_007_199_254_740_992.0;
const KEYDOWN_EVENT: &str = "keydown";
const KEYUP_EVENT: &str = "keyup";
const MOUSEMOVE_EVENT: &str = "mousemove";
const RESIZE_EVENT: &str = "resize";
const BLUR_EVENT: &str = "blur";

#[derive(Clone, Copy, Debug)]
struct TransientMessage {
    kind: TransientMessageKind,
    shown_until_milliseconds: f64,
}

/// Whether a frame drew a newly applied tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TickLayerChangeKind {
    Changed,
    Unchanged,
}

/// Outside the reactive graph: wgpu resources are `!Send`.
struct Driver {
    /// `None` when this browser cannot create one.
    renderer: Option<GameRenderer>,
    canvas: HtmlCanvasElement,
    viewport: ViewportSize,
    active_scene: ActiveScene,
    /// The connection's session while no online game is the active scene.
    lobby_session: Option<ClientSession>,
    controls: PlayerControls,
    socket: Option<GameSocket>,
    /// Kept until the next close: its own close handler is running when it closes.
    retired_socket: Option<GameSocket>,
    signals: AppSignals,
    settings_store: LocalStorageSettingsStore,
    render_scene: Option<RenderScene>,
    scene_message_text: Option<String>,
    transient_message: Option<TransientMessage>,
    pointer: ViewPoint,
    input_focus: InputFocusKind,
}

impl Driver {
    fn run_animation_frame(&mut self, now_milliseconds: f64) {
        self.update_input_focus();
        self.update_pointer_aim();

        let frame_output: FrameOutput =
            self.active_scene.advance_frame(now_milliseconds, &mut self.controls, self.input_focus);
        self.handle_effects(frame_output.effects);

        let tick_layer_change: TickLayerChangeKind = match frame_output.tick_applied {
            Some(_) => TickLayerChangeKind::Changed,
            None => TickLayerChangeKind::Unchanged,
        };

        if tick_layer_change == TickLayerChangeKind::Changed {
            self.publish_scene_overlay();
        }

        self.draw(frame_output.interpolation_alpha, tick_layer_change);
        self.publish_name_labels();
        self.publish_message_text(now_milliseconds);
    }

    fn run_command(&mut self, command: DriverCommandKind) {
        match command {
            DriverCommandKind::SendRequest(request) => {
                let effects: Vec<ClientEffect> = match self.get_session_mut() {
                    Some(client_session) => client_session.send_request(request),
                    None => {
                        log::warn!("request dropped without a connection; [request={request:?}]");

                        Vec::new()
                    }
                };

                self.handle_effects(effects);
            }
            DriverCommandKind::StartTutorial => self.start_tutorial(),
            DriverCommandKind::LeaveTutorial => self.leave_tutorial(),
            DriverCommandKind::Reconnect => self.reconnect(),
            DriverCommandKind::StoreClientSettings(client_settings) => self.store_client_settings(client_settings),
        }
    }

    fn get_session_mut(&mut self) -> Option<&mut ClientSession> {
        match &mut self.active_scene {
            ActiveScene::Online(client_session) => Some(client_session),
            ActiveScene::Tutorial(_) | ActiveScene::Title(_) => self.lobby_session.as_mut(),
        }
    }

    fn receive_frame(&mut self, frame: &[u8]) {
        let now_milliseconds: f64 = get_now_milliseconds();
        let effects: Vec<ClientEffect> = match &mut self.active_scene {
            ActiveScene::Online(client_session) => client_session.receive(frame, &mut self.controls, now_milliseconds),
            ActiveScene::Tutorial(_) | ActiveScene::Title(_) => match self.lobby_session.as_mut() {
                Some(client_session) => client_session.receive(frame, &mut self.controls, now_milliseconds),
                None => Vec::new(),
            },
        };

        self.handle_effects(effects);
    }

    fn handle_effects(&mut self, effects: Vec<ClientEffect>) {
        for effect in effects {
            self.handle_effect(effect);
        }
    }

    fn handle_effect(&mut self, effect: ClientEffect) {
        match effect {
            ClientEffect::SendFrame(frame) => self.send_frame(&frame),
            ClientEffect::GameListReceived {
                game_summaries,
                online_client_count,
            } => self.signals.game_list.set(GameListView {
                game_summaries,
                online_client_count,
            }),
            ClientEffect::JoinAccepted { .. } => self.enter_online_scene(),
            ClientEffect::RequestRejected { request, reason } => {
                log::info!("request rejected; [request={request:?} reason={reason:?}]");
                self.signals.request_rejection.set(Some(RequestRejection { request, reason }));
            }
            ClientEffect::OwnMemberStateChanged(own_member_state) => {
                self.signals.own_member_state.set(Some(own_member_state));

                let screen: Screen = self.signals.screen.get_untracked();
                let next_screen: Screen = screen.after_own_member_state_change(own_member_state);

                if next_screen != screen {
                    self.signals.navigate(next_screen);
                }
            }
            ClientEffect::OwnSpawnRejected => self.show_transient_message(TransientMessageKind::NoRoomToSpawn),
            ClientEffect::ResyncStarted | ClientEffect::ResyncFinished => self.publish_scene_overlay(),
            ClientEffect::LeftGame => self.return_to_lobby(Screen::Browser, None),
            ClientEffect::GameEnded => self.return_to_lobby(Screen::Browser, Some(LobbyNoticeKind::GameClosed)),
            ClientEffect::SynchronisationLost => {
                self.return_to_lobby(Screen::Title, Some(LobbyNoticeKind::SynchronisationLost));
            }
            ClientEffect::ConnectionLost(close_reason) => self.lose_connection(close_reason),
            ClientEffect::OwnOrganismSpawned { .. } | ClientEffect::TickApplied { .. } => {}
        }
    }

    fn send_frame(&self, frame: &[u8]) {
        match &self.socket {
            Some(socket) => socket.send(frame),
            None => log::warn!("frame dropped without a connection"),
        }
    }

    fn enter_online_scene(&mut self) {
        let Some(client_session) = self.lobby_session.take() else {
            log::warn!("join accepted without a lobby session");

            return;
        };

        self.signals.lobby_notice.set(None);
        self.replace_active_scene(ActiveScene::Online(client_session));
    }

    fn return_to_lobby(&mut self, screen: Screen, lobby_notice: Option<LobbyNoticeKind>) {
        let previous_scene: ActiveScene = self.replace_active_scene(ActiveScene::Title(self.create_title_scene()));

        if let ActiveScene::Online(client_session) = previous_scene {
            self.lobby_session = Some(client_session);
        }

        self.signals.lobby_notice.set(lobby_notice);
        self.signals.navigate(screen);
    }

    fn lose_connection(&mut self, close_reason: Option<CloseReasonKind>) {
        log::info!("game server connection lost; [close_reason={close_reason:?}]");

        self.retired_socket = self.socket.take();
        self.lobby_session = None;

        if matches!(self.active_scene, ActiveScene::Online(_)) {
            self.replace_active_scene(ActiveScene::Title(self.create_title_scene()));
        }

        self.signals.connection_status.set(ConnectionStatusKind::Closed { close_reason });

        let screen: Screen = self.signals.screen.get_untracked();
        let next_screen: Screen = screen.after_connection_lost();

        if next_screen != screen {
            self.signals.navigate(next_screen);
        }
    }

    /// The overlays and the uploaded tick layer belong to the scene they were built from.
    fn replace_active_scene(&mut self, active_scene: ActiveScene) -> ActiveScene {
        let previous_scene: ActiveScene = std::mem::replace(&mut self.active_scene, active_scene);

        self.render_scene = None;
        self.transient_message = None;
        self.release_keyboard();

        if !matches!(self.active_scene, ActiveScene::Online(_)) {
            self.signals.own_member_state.set(None);
        }

        self.publish_scene_overlay();

        previous_scene
    }

    fn start_tutorial(&mut self) {
        if !matches!(self.active_scene, ActiveScene::Title(_)) {
            return;
        }

        let window_size: PixelSize = get_window_size(&self.canvas);
        let tutorial: Tutorial = Tutorial::new(
            window_size,
            get_random_seed(),
            client::get_display_mode(),
            &mut self.controls,
        );

        self.replace_active_scene(ActiveScene::Tutorial(tutorial));
        self.signals.navigate(Screen::Tutorial);
    }

    fn leave_tutorial(&mut self) {
        if !matches!(self.active_scene, ActiveScene::Tutorial(_)) {
            return;
        }

        self.replace_active_scene(ActiveScene::Title(self.create_title_scene()));
        self.signals.navigate(Screen::Title);
    }

    fn create_title_scene(&self) -> TitleScene {
        TitleScene::new(get_window_size(&self.canvas), get_random_seed())
    }

    fn reconnect(&mut self) {
        if self.socket.is_some() {
            return;
        }

        self.signals.connection_status.set(ConnectionStatusKind::Connecting);
        connect();
    }

    fn open_socket(&mut self, websocket_url: Result<String, AppError>) {
        let websocket_url: String = match websocket_url {
            Ok(websocket_url) => websocket_url,
            Err(error) => {
                log::warn!("the game server url is unknown; [error={error}]");
                self.lose_connection(None);

                return;
            }
        };

        let opened_socket: Result<GameSocket, AppError> = GameSocket::open(
            &websocket_url,
            || with_driver(Driver::socket_opened),
            |frame: Vec<u8>| with_driver(|driver: &mut Driver| driver.receive_frame(&frame)),
            |close_code: u16| with_driver(|driver: &mut Driver| driver.socket_closed(close_code)),
        );

        match opened_socket {
            Ok(socket) => self.socket = Some(socket),
            Err(error) => {
                log::warn!("opening the game socket failed; [error={error}]");
                self.lose_connection(None);
            }
        }
    }

    fn socket_opened(&mut self) {
        log::info!("game server connection open");

        self.lobby_session = Some(ClientSession::new());
        self.signals.connection_status.set(ConnectionStatusKind::Open);
    }

    fn socket_closed(&mut self, close_code: u16) {
        let effects: Vec<ClientEffect> = match self.get_session_mut() {
            Some(client_session) => client_session.connection_closed(close_code),
            None => vec![ClientEffect::ConnectionLost(CloseReasonKind::from_close_code(
                close_code,
            ))],
        };

        self.handle_effects(effects);
    }

    fn store_client_settings(&mut self, client_settings: ClientSettings) {
        client_settings.write(&self.settings_store);
    }

    fn key_down(&mut self, event: &KeyboardEvent) {
        let Some(key) = keyboard::get_game_key(&event.code()) else {
            return;
        };

        let screen: Screen = self.signals.screen.get_untracked();

        match key {
            GameKeyKind::Pause => {
                self.signals.go_back();

                return;
            }
            GameKeyKind::Respawn if screen == Screen::Spectating => self.press_respawn_key(),
            _ => {}
        }

        if screen.input_focus() != InputFocusKind::Game {
            return;
        }

        self.controls.keyboard.key_changed(
            key,
            KeyTransitionKind::Pressed,
            keyboard::get_key_repeat(event.repeat()),
        );

        if keyboard::get_key_default_action(key) == KeyDefaultActionKind::Prevented {
            event.prevent_default();
        }
    }

    fn key_up(&mut self, event: &KeyboardEvent) {
        let Some(key) = keyboard::get_game_key(&event.code()) else {
            return;
        };

        self.controls.keyboard.key_changed(key, KeyTransitionKind::Released, KeyRepeatKind::Initial);
    }

    fn press_respawn_key(&mut self) {
        let ActiveScene::Online(client_session) = &self.active_scene else {
            return;
        };

        let (Some(state), Some(own_member_id)) = (client_session.render_state(), client_session.own_member_id()) else {
            return;
        };

        let respawn_key_outcome: RespawnKeyOutcomeKind =
            own_member_state::get_respawn_key_outcome(state, own_member_id);

        match respawn_key_outcome {
            RespawnKeyOutcomeKind::OpenRespawnMenu => self.signals.navigate(Screen::RespawnMenu),
            RespawnKeyOutcomeKind::ShowGameAtCapacity => {
                self.show_transient_message(TransientMessageKind::GameAtCapacity);
            }
            RespawnKeyOutcomeKind::Ignore => {}
        }
    }

    fn move_pointer(&mut self, event: &MouseEvent) {
        self.pointer = ViewPoint {
            x: event.client_x() as f32,
            y: event.client_y() as f32,
        };

        self.update_pointer_aim();
    }

    /// The aim is the pointer's offset from where the crosshair is drawn, which moves every frame.
    fn update_pointer_aim(&mut self) {
        let scene_view: SceneView = scene_camera::get_scene_view(&self.active_scene, &self.controls, self.viewport);
        let crosshair: InterpolatedPoint = InterpolatedPoint::from_subpixel_point(self.controls.crosshair.position());
        let crosshair_view_point: ViewPoint = scene_view.camera.get_view_point(crosshair);

        self.controls.keyboard.aim_changed(keyboard::get_aim_vector(self.pointer, crosshair_view_point));
    }

    /// A menu opening ends every held key and drops the presses not yet sent.
    fn update_input_focus(&mut self) {
        let input_focus: InputFocusKind = self.signals.screen.get_untracked().input_focus();

        if input_focus == InputFocusKind::Menu && self.input_focus == InputFocusKind::Game {
            self.release_keyboard();
        }

        self.input_focus = input_focus;
    }

    fn release_keyboard(&mut self) {
        self.controls.keyboard.release_all_keys();
        self.controls.keyboard.discard_ability_presses();
    }

    fn release_all_keys(&mut self) {
        self.controls.keyboard.release_all_keys();
    }

    fn resize(&mut self) {
        self.viewport = configure_canvas_backing_store(&self.canvas);

        if let Some(renderer) = self.renderer.as_mut() {
            renderer.resize(self.viewport.physical_size);
        }

        self.render_scene = None;

        let window_size: PixelSize = get_window_size(&self.canvas);

        match &mut self.active_scene {
            ActiveScene::Title(title_scene) => title_scene.resize(window_size),
            ActiveScene::Tutorial(tutorial) => {
                tutorial.display_mode_changed(client::get_display_mode());
                tutorial.resize(window_size, &mut self.controls);
            }
            ActiveScene::Online(_) => {}
        }
    }

    fn draw(&mut self, interpolation_alpha: f32, tick_layer_change: TickLayerChangeKind) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };

        let Some(state) = self.active_scene.render_state() else {
            return;
        };

        let scene_view: SceneView = scene_camera::get_scene_view(&self.active_scene, &self.controls, self.viewport);
        let interpolation: InterpolationFrame = InterpolationFrame {
            alpha: interpolation_alpha,
        };

        match (self.render_scene.as_mut(), tick_layer_change) {
            (Some(render_scene), TickLayerChangeKind::Unchanged) => {
                render_scene.refresh_frame(state, &interpolation, scene_view.camera, scene_view.crosshair);
            }
            (Some(_), TickLayerChangeKind::Changed) | (None, _) => {
                let render_scene: RenderScene =
                    RenderScene::from_state(state, &interpolation, scene_view.camera, scene_view.crosshair);
                renderer.upload_tick_layer(&render_scene.tick_layer);
                self.render_scene = Some(render_scene);
            }
        }

        let Some(render_scene) = self.render_scene.as_ref() else {
            return;
        };

        let drawn: Result<(), AppError> = renderer.draw_frame(render_scene);

        if let Err(error) = drawn {
            log::error!("drawing a frame failed; [error={error}]");
        }
    }

    fn publish_scene_overlay(&mut self) {
        let scene_overlay: SceneOverlay = SceneOverlay::from_active_scene(&self.active_scene);

        app::set_if_changed(self.signals.ability_bar, scene_overlay.ability_bar);
        app::set_if_changed(self.signals.leaderboard, scene_overlay.leaderboard);
        app::set_if_changed(self.signals.game_menu, scene_overlay.game_menu);
        self.scene_message_text = scene_overlay.message_text;
    }

    /// Online only, and only while the Name Labels setting is on.
    fn publish_name_labels(&self) {
        let is_shown: bool = self.signals.client_settings.get_untracked().name_labels_shown;
        let name_labels: Vec<NameLabelView> = match (&self.active_scene, is_shown) {
            (ActiveScene::Online(client_session), true) => client_session
                .render_state()
                .map(|state| {
                    let scene_view: SceneView =
                        scene_camera::get_scene_view(&self.active_scene, &self.controls, self.viewport);

                    hud::get_name_labels(state, &scene_view.camera)
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        };

        app::set_if_changed(self.signals.name_labels, name_labels);
    }

    fn publish_message_text(&mut self, now_milliseconds: f64) {
        let is_transient_message_expired: bool = self
            .transient_message
            .is_some_and(|transient_message| transient_message.shown_until_milliseconds <= now_milliseconds);

        if is_transient_message_expired {
            self.transient_message = None;
        }

        let is_shown: bool = self.signals.client_settings.get_untracked().messages_shown;
        let message_text: Option<String> = match (is_shown, self.transient_message) {
            (false, _) => None,
            (true, Some(transient_message)) => Some(transient_message.kind.text()),
            (true, None) => self.scene_message_text.clone(),
        };

        app::set_if_changed(self.signals.message_text, message_text);
    }

    fn show_transient_message(&mut self, kind: TransientMessageKind) {
        self.transient_message = Some(TransientMessage {
            kind,
            shown_until_milliseconds: get_now_milliseconds() + message_text::TRANSIENT_MESSAGE_DURATION_MILLISECONDS,
        });
    }
}

pub fn start(canvas: HtmlCanvasElement, signals: AppSignals) {
    leptos::task::spawn_local(async move {
        let created: Result<(), AppError> = create_driver(canvas, signals).await;

        match created {
            Ok(()) => connect(),
            Err(error) => log::error!("the game client could not start; [error={error}]"),
        }
    });
}

pub fn dispatch(command: DriverCommandKind) {
    PENDING_DRIVER_COMMANDS.with_borrow_mut(|pending_commands| pending_commands.push_back(command));

    run_pending_commands();
}

async fn create_driver(canvas: HtmlCanvasElement, signals: AppSignals) -> Result<(), AppError> {
    let settings_store: LocalStorageSettingsStore = LocalStorageSettingsStore;
    signals.client_settings.set(ClientSettings::read(&settings_store));

    let viewport: ViewportSize = configure_canvas_backing_store(&canvas);
    let renderer: Option<GameRenderer> = create_renderer(&canvas, viewport).await;
    let driver: Driver = Driver {
        renderer,
        active_scene: ActiveScene::Title(TitleScene::new(get_window_size(&canvas), get_random_seed())),
        canvas,
        viewport,
        lobby_session: None,
        controls: PlayerControls::new(),
        socket: None,
        retired_socket: None,
        signals,
        settings_store,
        render_scene: None,
        scene_message_text: None,
        transient_message: None,
        pointer: ViewPoint { x: 0.0, y: 0.0 },
        input_focus: InputFocusKind::Menu,
    };

    DRIVER.with_borrow_mut(|driver_slot| *driver_slot = Some(driver));

    install_browser_listeners()?;
    schedule_animation_frame();

    Ok(())
}

async fn create_renderer(canvas: &HtmlCanvasElement, viewport: ViewportSize) -> Option<GameRenderer> {
    let created_renderer: Result<GameRenderer, AppError> =
        GameRenderer::from_canvas(canvas.clone(), get_renderer_backend(), viewport.physical_size).await;

    match created_renderer {
        Ok(renderer) => {
            log::info!("renderer created; [adapter={:?}]", renderer.adapter_info());

            Some(renderer)
        }
        Err(error) => {
            log::error!("the game cannot be drawn in this browser; [error={error}]");

            None
        }
    }
}

fn connect() {
    leptos::task::spawn_local(async move {
        let websocket_url: Result<String, AppError> = client::read_websocket_url().await;

        with_driver(|driver: &mut Driver| driver.open_socket(websocket_url));
    });
}

/// Does nothing while the driver is absent or already borrowed; a pending command waits for the next frame.
fn with_driver(action: impl FnOnce(&mut Driver)) {
    DRIVER.with(|driver_slot| {
        let Ok(mut driver_slot) = driver_slot.try_borrow_mut() else {
            return;
        };

        if let Some(driver) = driver_slot.as_mut() {
            action(driver);
        }
    });

    run_pending_commands();
}

fn run_pending_commands() {
    DRIVER.with(|driver_slot| {
        let Ok(mut driver_slot) = driver_slot.try_borrow_mut() else {
            return;
        };

        let Some(driver) = driver_slot.as_mut() else {
            return;
        };

        loop {
            let pending_command: Option<DriverCommandKind> =
                PENDING_DRIVER_COMMANDS.with_borrow_mut(|pending_commands| pending_commands.pop_front());

            let Some(command) = pending_command else {
                break;
            };

            driver.run_command(command);
        }
    });
}

fn schedule_animation_frame() {
    ANIMATION_FRAME_CALLBACK.with_borrow_mut(|animation_frame_callback| {
        let callback: &Closure<dyn FnMut(f64)> = animation_frame_callback
            .get_or_insert_with(|| Closure::new(|now_milliseconds: f64| run_animation_frame(now_milliseconds)));

        let Some(window) = web_sys::window() else {
            return;
        };

        let scheduled: Result<i32, JsValue> = window.request_animation_frame(callback.as_ref().unchecked_ref());

        if let Err(error) = scheduled {
            log::error!("scheduling an animation frame failed; [error={error:?}]");
        }
    });
}

/// Rescheduled first, so a frame the driver skips does not stop the loop.
fn run_animation_frame(now_milliseconds: f64) {
    schedule_animation_frame();

    with_driver(|driver: &mut Driver| driver.run_animation_frame(now_milliseconds));
}

/// Installed for the page's lifetime.
fn install_browser_listeners() -> Result<(), AppError> {
    let window: Window = web_sys::window().ok_or_else(|| AppError::new("the page has no window"))?;

    add_window_listener(&window, KEYDOWN_EVENT, |event: Event| {
        if let Some(keyboard_event) = event.dyn_ref::<KeyboardEvent>() {
            with_driver(|driver: &mut Driver| driver.key_down(keyboard_event));
        }
    })?;
    add_window_listener(&window, KEYUP_EVENT, |event: Event| {
        if let Some(keyboard_event) = event.dyn_ref::<KeyboardEvent>() {
            with_driver(|driver: &mut Driver| driver.key_up(keyboard_event));
        }
    })?;
    add_window_listener(&window, MOUSEMOVE_EVENT, |event: Event| {
        if let Some(mouse_event) = event.dyn_ref::<MouseEvent>() {
            with_driver(|driver: &mut Driver| driver.move_pointer(mouse_event));
        }
    })?;
    add_window_listener(&window, RESIZE_EVENT, |_event: Event| with_driver(Driver::resize))?;
    add_window_listener(&window, BLUR_EVENT, |_event: Event| {
        with_driver(Driver::release_all_keys)
    })?;

    Ok(())
}

fn add_window_listener(
    window: &Window,
    event_type: &str,
    listener: impl FnMut(Event) + 'static,
) -> Result<(), AppError> {
    let listener_closure: Closure<dyn FnMut(Event)> = Closure::new(listener);
    let added: Result<(), JsValue> =
        window.add_event_listener_with_callback(event_type, listener_closure.as_ref().unchecked_ref());

    added.map_err(|error| AppError::new(&format!("listening for {event_type} failed; [error={error:?}]")))?;
    listener_closure.forget();

    Ok(())
}

/// The canvas fills the window; its drawing buffer is sized in physical pixels.
fn configure_canvas_backing_store(canvas: &HtmlCanvasElement) -> ViewportSize {
    let device_pixel_ratio: f64 = web_sys::window().map_or(1.0, |window| window.device_pixel_ratio());
    let physical_size: PhysicalSize = PhysicalSize {
        width_pixels: get_physical_pixels(canvas.client_width(), device_pixel_ratio),
        height_pixels: get_physical_pixels(canvas.client_height(), device_pixel_ratio),
    }
    .at_least_one_pixel();

    canvas.set_width(physical_size.width_pixels);
    canvas.set_height(physical_size.height_pixels);

    ViewportSize {
        physical_size,
        device_pixel_ratio: device_pixel_ratio as f32,
    }
}

fn get_physical_pixels(css_pixels: i32, device_pixel_ratio: f64) -> u32 {
    (f64::from(css_pixels.max(0)) * device_pixel_ratio).round() as u32
}

/// Local games are sized in CSS pixels, as the original's window was.
fn get_window_size(canvas: &HtmlCanvasElement) -> PixelSize {
    PixelSize {
        width_pixels: u32::try_from(canvas.client_width()).unwrap_or(0),
        height_pixels: u32::try_from(canvas.client_height()).unwrap_or(0),
    }
}

/// `?renderer=webgl2` forces the WebGL2 backend for parity checks.
fn get_renderer_backend() -> RendererBackendKind {
    let query: String = web_sys::window().and_then(|window| window.location().search().ok()).unwrap_or_default();
    let is_webgl2_forced: bool =
        query.trim_start_matches('?').split('&').any(|parameter| parameter == FORCE_WEBGL2_QUERY_PARAMETER);

    if is_webgl2_forced {
        RendererBackendKind::ForceGl
    } else {
        RendererBackendKind::Default
    }
}

fn get_random_seed() -> u64 {
    (js_sys::Math::random() * RANDOM_SEED_SCALE) as u64
}

/// The clock `requestAnimationFrame` stamps its callbacks with.
fn get_now_milliseconds() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map_or(0.0, |performance| performance.now())
}
```

- [ ] **Step 4: Mount the canvas and start the driver**

Replace the whole contents of `web/src/canvas/game_canvas.rs` with:

```rust
use leptos::html::Canvas;
use leptos::prelude::*;

use shared::protocol::MessageSerialIn;

use crate::app::{AppSignals, ClientSettings};

/// What the views ask of the driver, which owns the socket and the scenes.
#[derive(Clone, Debug, PartialEq)]
pub enum DriverCommandKind {
    SendRequest(MessageSerialIn),
    StartTutorial,
    LeaveTutorial,
    Reconnect,
    StoreClientSettings(ClientSettings),
}

/// Behind every other layer.
#[component]
pub fn GameCanvas() -> impl IntoView {
    let signals: AppSignals = expect_context();
    let canvas_ref: NodeRef<Canvas> = NodeRef::new();

    start_driver_when_mounted(canvas_ref, signals);

    view! { <canvas node_ref=canvas_ref class="game-canvas"></canvas> }
}

#[cfg(feature = "hydrate")] // the ssr build has no driver to command
pub use browser::*;
#[cfg(not(feature = "hydrate"))] // the ssr build has no driver
pub use server::*;

#[cfg(feature = "hydrate")] // the ssr build has no driver to command
mod browser {
    use leptos::html::Canvas;
    use leptos::prelude::*;

    use crate::app::AppSignals;
    use crate::canvas::DriverCommandKind;
    use crate::canvas::driver;

    pub fn start_driver_when_mounted(canvas_ref: NodeRef<Canvas>, signals: AppSignals) {
        Effect::new(move |_| {
            if let Some(canvas) = canvas_ref.get() {
                driver::start(canvas, signals);
            }
        });
    }

    pub fn dispatch_driver_command(command: DriverCommandKind) {
        driver::dispatch(command);
    }
}

#[cfg(not(feature = "hydrate"))] // the ssr build renders the canvas without driving it
mod server {
    use leptos::html::Canvas;
    use leptos::prelude::*;

    use crate::app::AppSignals;
    use crate::canvas::DriverCommandKind;

    pub fn start_driver_when_mounted(_canvas_ref: NodeRef<Canvas>, _signals: AppSignals) {}

    pub fn dispatch_driver_command(_command: DriverCommandKind) {}
}
```

In `web/src/app.rs`, replace:

```rust
use shared::settings::SettingsStore;

use crate::browser::{GameBrowser, GameListView};
use crate::connection::ConnectionStatusKind;
use crate::hud::{AbilityBar, AbilityBarView, NameLabelView, NameLabels};
```

with:

```rust
use shared::settings::SettingsStore;

use crate::browser::{GameBrowser, GameListView};
use crate::canvas::GameCanvas;
use crate::connection::ConnectionStatusKind;
use crate::hud::{AbilityBar, AbilityBarView, NameLabelView, NameLabels};
```

In `web/src/app.rs`, replace:

```rust
    let is_shaded = move || screen.get().backdrop() == ScreenBackdropKind::Shaded;

    view! {
        <div class="overlay-layer">
            <NameLabels name_labels=signals.name_labels.into() />
```

with:

```rust
    let is_shaded = move || screen.get().backdrop() == ScreenBackdropKind::Shaded;

    view! {
        <GameCanvas />
        <div class="overlay-layer">
            <NameLabels name_labels=signals.name_labels.into() />
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p web --features ssr --test prerender`

Expected: PASS:

```text
test prerender_document_references_the_client_bundle ... ok
test prerender_document_contains_the_canvas_root ... ok
test prerender_document_names_the_game_in_the_tab ... ok
test prerender_document_opens_on_the_title_screen ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

- [ ] **Step 6: Check the browser build**

Run: `./scripts/test/check-wasm.sh`

Expected: the three checks finish without a warning:

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s)
    Finished `dev` profile [unoptimized + debuginfo] target(s)
    Finished `dev` profile [unoptimized + debuginfo] target(s)
```

- [ ] **Step 7: Run the web unit tests**

Run: `cargo test -p web --lib`

Expected: PASS, every web unit test:

```text
test result: ok. 83 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

- [ ] **Step 8: Play in the browser**

Start the servers as background processes (outside a sandbox): `./scripts/dev/server.sh`, then `./scripts/dev/web.sh`; wait until the web log prints `Serving at http://127.0.0.1:3011`. Open `http://127.0.0.1:3011` in Chrome through the MCP tools and check, comparing with `docs/architecture/client-web.md` and the original's `public/css` and `public/js/ui`:

- Title: dummy organisms grow on a black world inset 25 px with its shadow; the box with Host, Join and Tutorial in the bacter font sits at the centre; "Connecting to the game server" shows until the socket opens, then no notice. The console shows `renderer created; [adapter=... backend: BrowserWebGpu ...]` and `game server connection open`, and no warning or error.
- Host: the create menu (black header "Game Creation Options", striped rows, Create, Back footer). Game Mode Skirmish shows Team Count and hides Leaderboard Length; Survival shows Player Minimum. A blank title with World Width 200 shows "Title cannot be left blank" under Game Title, and typing in World Width fills World Height. Enter a title, width 600, mode Free for All, and Create: the join menu ("Join Game Options") without a Password row.
- Join step: submitting with no ability shows "Select a first ability", "Select a second ability" and "Select a third ability" under their rows. Enter a screen name, pick a skin and three abilities, Join: the game opens, the camera centred on your organism with the white crosshair, the leaderboard top right with your row in bold, your name label under the organism, and the ability bar at nine tenths of the height with the loadout's icons, the key tabs X, C, V and `_`, and a shot dial left of a Compress or Freeze slot.
- Hold D for about 1.5 s (`evaluate_script`: dispatch `keydown` with `code: 'KeyD'` on `window`, wait, dispatch `keyup`): the world moves left by about 64 px (42.5 px/s) and the organism follows.
- Abilities: in one `evaluate_script`, dispatch X and V presses (`keydown` then `keyup`), wait 500 ms, then return `document.querySelector('.game-canvas').toDataURL('image/png')` with `filePath` set; decode the data URL in a `/tmp` script and read the PNG. The canvas shows the third ability's field around the crosshair, grey with a green ring (Neutralize) or a red one (Toxin). In the DOM, an active slot carries its dot (`.ability-effect-dot-own`, green, for the own Extend, Immortality or Neutralize; `.ability-effect-dot-enemy`, red, for the own Toxin), and sampling the slot's `.ability-dial-fill` `d` attribute every 500 ms shows the dial draining clockwise from twelve o'clock over the ability's duration, then refilling over its cooldown.
- A second page (new page with `isolatedContext` set): Join lists the game ("Free for All", 1 player); join with another name and loadout. Both pages show both organisms, both leaderboard rows and both name labels; the console of each shows no warning (no resync).
- Death and respawn: on the second page hold S for about 12 s, until the organism has left the world and died; the message box shows "Press 'R' to Spawn", the HUD disappears and the death counts on the leaderboard. Press R: "Spawn Options" prefilled with the last loadout (selected radios carry `menu-radio-selected`); Spawn returns to the game.
- Escape: "Pause Options" with Color, Skin, Name Labels, Messages and Leave Game; choose another colour, deselect Name Labels, Apply: the organism changes colour on both pages, labels disappear on this page, and `localStorage` holds `bacter.name_labels` = `0100`. Escape, Leave Game: the browser lists the game again.
- Spectate with the first player's name: "Name matches that of another player" under Screen Name; with a new name: spectating, "Press 'R' to Spawn" in the message box.
- Escape, Leave Game, Back, Tutorial: "Use W-A-S-D (Recommended) or the arrow keys to move" in the message box and the crosshair on a still world; a movement key moves the task on to the fullscreen text with the ability bar shown; Escape, Leave Tutorial returns to the title.
- Stop the game server (its PID from `lsof -nP -t -iTCP:3100 -sTCP:LISTEN`): the title shows "Disconnected from the game server" and Reconnect, Host and Join grey. Start the server again and click Reconnect: the notice goes and Host and Join work.
- `?renderer=webgl2`: the console shows `backend: Gl` in `renderer created`, and the title scene draws as before.

Stop every process you started by its PID (the `cargo-leptos` watcher and its `target/debug/web` child, the game server).

- [ ] **Step 9: Commit**

```sh
git add web/src/canvas/driver.rs
git add web/src/canvas/game_canvas.rs
git add web/src/app.rs
git add web/tests/prerender.rs
git status --short
git commit -m "driver and game canvas"
```

Expected `git status --short` before the commit:

```text
M  web/src/app.rs
M  web/src/canvas/driver.rs
M  web/src/canvas/game_canvas.rs
M  web/tests/prerender.rs
```

### Task 28: Site and release scripts

`build-site.sh` (release build with hashed names, the prerendered `index.html`, `game-server.json` rewritten to a null URL, then `verify-site-tree.sh`), `build-release.sh` (the site and the release server) and `run-release-server.sh` (the production settings checked, then the server).

**Files:**
- Create: `scripts/build/build-site.sh`
- Create: `scripts/build/verify-site-tree.sh`
- Create: `scripts/build/build-release.sh`
- Create: `scripts/run/run-release-server.sh`

- [ ] **Step 1: Add the scripts**

Create `scripts/build/build-site.sh`:

```sh
#!/usr/bin/env bash
# Builds the deployable site into target/site: a release build, the prerendered document, and a
# game-server.json naming the page's own origin.
#
# Usage:
#   ./scripts/build/build-site.sh

set -euo pipefail

readonly MISSING_PROGRAM_EXIT_STATUS=1

function fail {
    echo "error: $1" >&2
    exit 1
}

function require_program {
    local program_name="$1"
    local install_hint="$2"

    local program_path
    program_path=$(command -v "${program_name}" || true)

    if test -z "${program_path}"; then
        echo "error: the \`${program_name}\` program is required; install it with: ${install_hint}" >&2
        exit "${MISSING_PROGRAM_EXIT_STATUS}"
    fi
}

# `cargo leptos watch` serves this same site root, and a release build replaces everything in it.
function refuse_while_the_dev_server_runs {
    local web_manifest_path="$1"

    local site_address
    site_address=$(grep -E '^site-addr[[:space:]]*=' "${web_manifest_path}" | head -1 | sed -E 's/^[^=]*=[[:space:]]*"(.*)".*$/\1/')

    local site_port="${site_address##*:}"

    local listener_ids
    listener_ids=$(lsof -nP -t -iTCP:"${site_port}" -sTCP:LISTEN || true)

    if test -n "${listener_ids}"; then
        fail "something listens on ${site_address}, where the dev server serves the site root this build replaces; stop it first"
    fi
}

function main {
    local repository_root
    repository_root=$(git rev-parse --show-toplevel)

    local web_directory="${repository_root}/web"
    local site_directory="${repository_root}/target/site"
    local web_binary="${repository_root}/target/release/web"

    require_program "cargo-leptos" "cargo install --locked cargo-leptos"
    require_program "jq" "brew install jq"
    require_program "lsof" "the lsof package of the system's package manager"
    refuse_while_the_dev_server_runs "${web_directory}/Cargo.toml"

    # Hashed names let the server cache /pkg/* as immutable; both the build and the prerender read this.
    export LEPTOS_HASH_FILES=true

    echo "building the release site"
    cd "${web_directory}"
    cargo leptos build --release

    if test ! -x "${web_binary}"; then
        fail "the release build produced no web binary at ${web_binary}"
    fi

    echo "prerendering the document"
    "${web_binary}" prerender

    # A null URL makes the page connect to its own origin.
    echo "naming the page's origin in game-server.json"
    jq '.websocket_url = null' "${web_directory}/static/game-server.json" > "${site_directory}/game-server.json"

    "${repository_root}/scripts/build/verify-site-tree.sh" "${site_directory}"
}

main
```

Create `scripts/build/verify-site-tree.sh`:

```sh
#!/usr/bin/env bash
# Checks that a site tree is complete: the prerendered document, and every /pkg/ asset it references on disk
# under a content-hashed name.
#
# Usage:
#   ./scripts/build/verify-site-tree.sh [<site-directory>]

set -euo pipefail

# cargo-leptos inserts a base64url digest between the stem and the extension.
readonly HASHED_NAME_PATTERN='\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9]+$'

function fail {
    echo "error: $1" >&2
    exit 1
}

function main {
    local repository_root
    repository_root=$(git rev-parse --show-toplevel)

    local site_directory="${1:-${repository_root}/target/site}"
    local document_path="${site_directory}/index.html"

    if test ! -d "${site_directory}"; then
        fail "${site_directory} does not exist; run ./scripts/build/build-site.sh"
    fi

    if test ! -f "${document_path}"; then
        fail "${document_path} is missing, so the site would serve no document at /"
    fi

    local referenced_paths
    referenced_paths=$(grep -o '/pkg/[A-Za-z0-9._-]*' "${document_path}" | sort -u || true)

    local reference_count=0

    for referenced_path in ${referenced_paths}; do
        if test ! -f "${site_directory}${referenced_path}"; then
            fail "the document references ${referenced_path}, which is not in ${site_directory}"
        fi

        local hashed_name
        hashed_name=$(printf '%s' "${referenced_path}" | grep -E "${HASHED_NAME_PATTERN}" || true)

        # The server caches /pkg/* as immutable, which is right only while a rebuild changes the name.
        if test -z "${hashed_name}"; then
            fail "the document references ${referenced_path}, whose name carries no content hash; build with ./scripts/build/build-site.sh"
        fi

        reference_count=$((reference_count + 1))
    done

    if test "${reference_count}" -eq 0; then
        fail "the document references no /pkg/ assets, so it would load nothing"
    fi

    echo "${site_directory}: prerendered document present, ${reference_count} referenced assets present and content-hashed"
}

main "$@"
```

Create `scripts/build/build-release.sh`:

```sh
#!/usr/bin/env bash
# Builds the site and the release game server, which serves the site and the game socket from one origin.
#
# Usage:
#   ./scripts/build/build-release.sh

set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}"

./scripts/build/build-site.sh
cargo build --release -p server
```

Create `scripts/run/run-release-server.sh`:

```sh
#!/usr/bin/env bash
# Runs the release game server with ./.env, after checking the settings a production server needs.
#
# Usage:
#   ./scripts/run/run-release-server.sh

set -euo pipefail

readonly REQUIRED_VARIABLES=(
    BACTER_BIND_ADDRESS
    BACTER_PORT
    BACTER_SITE_ROOT
    BACTER_ALLOWED_ORIGINS
)

function fail {
    echo "error: $1" >&2
    exit 1
}

function require_variables {
    for variable_name in "${REQUIRED_VARIABLES[@]}"; do
        local variable_value="${!variable_name:-}"

        if test -z "${variable_value}"; then
            fail "${variable_name} is not set in ./.env"
        fi
    done
}

function require_site_root {
    if test ! -f "${BACTER_SITE_ROOT}/index.html"; then
        fail "${BACTER_SITE_ROOT}/index.html is missing; run ./scripts/build/build-release.sh"
    fi
}

# TLS needs both PEM files, or neither for plain HTTP.
function require_tls_files {
    local certificate_path="${BACTER_TLS_CERTIFICATE_PATH:-}"
    local private_key_path="${BACTER_TLS_PRIVATE_KEY_PATH:-}"

    if test -z "${certificate_path}" && test -z "${private_key_path}"; then
        return 0
    fi

    if test -z "${certificate_path}" || test -z "${private_key_path}"; then
        fail "set both BACTER_TLS_CERTIFICATE_PATH and BACTER_TLS_PRIVATE_KEY_PATH, or neither"
    fi

    for pem_path in "${certificate_path}" "${private_key_path}"; do
        if test ! -r "${pem_path}"; then
            fail "${pem_path} is not readable"
        fi
    done
}

function main {
    local repository_root
    repository_root=$(git rev-parse --show-toplevel)
    cd "${repository_root}"

    if test ! -f ./.env; then
        fail "./.env is missing; run ./setup.sh first"
    fi

    unset "${REQUIRED_VARIABLES[@]}" BACTER_TLS_CERTIFICATE_PATH BACTER_TLS_PRIVATE_KEY_PATH

    set -a
    source ./.env
    set +a

    require_variables
    require_site_root
    require_tls_files

    if test ! -x ./target/release/server; then
        fail "./target/release/server is missing; run ./scripts/build/build-release.sh"
    fi

    exec ./target/release/server
}

main
```

Run: `chmod +x scripts/build/build-site.sh scripts/build/verify-site-tree.sh scripts/build/build-release.sh scripts/run/run-release-server.sh && ls -l scripts/build/build-site.sh scripts/build/verify-site-tree.sh scripts/build/build-release.sh scripts/run/run-release-server.sh | cut -c1-10`

Expected:

```text
-rwxr-xr-x
-rwxr-xr-x
-rwxr-xr-x
-rwxr-xr-x
```

- [ ] **Step 2: Build the site**

Run: `./scripts/build/build-site.sh 2>&1 | tail -4`

Expected: nothing may listen on port 3011 (stop `./scripts/dev/web.sh` first). The release build takes several minutes; the last lines:

```text
prerendering the document
wrote the prerendered document; [path=/Users/singularity/bacter-rs/target/site/index.html bytes=...]
naming the page's origin in game-server.json
/Users/singularity/bacter-rs/target/site: prerendered document present, 3 referenced assets present and content-hashed
```

- [ ] **Step 3: Check the same-origin config**

Run: `cat target/site/game-server.json`

Expected:

```text
{
  "websocket_url": null
}
```

- [ ] **Step 4: Build the release server**

Run: `./scripts/build/build-release.sh 2>&1 | tail -1`

Expected: `build-release.sh` runs `build-site.sh` again, then builds the server; the last line:

```text
    Finished `release` profile [optimized] target(s)
```

- [ ] **Step 5: Check the release server's settings check**

Run: `./scripts/run/run-release-server.sh; echo "exit=$?"`

Expected: with the development `.env` (`BACTER_SITE_ROOT` unset), the script refuses to start:

```text
error: BACTER_SITE_ROOT is not set in ./.env
exit=1
```

- [ ] **Step 6: Serve the site and the socket from one origin**

Serve the built site and the socket from one origin, without editing `.env`: run `BACTER_SITE_ROOT=target/site BACTER_ALLOWED_ORIGINS=http://127.0.0.1:3100 ./target/release/server` in the background (outside a sandbox; the server's `.env` loading keeps variables already set), open `http://127.0.0.1:3100/` in Chrome and check in `evaluate_script` that `await (await fetch('/game-server.json')).json()` is `{"websocket_url":null}`, `fetch('/fonts/bacter.ttf')` answers 200, and Join shows the browser with "Online Clients: 1" (the page connected to its own origin). Stop the server by its PID.

- [ ] **Step 7: Commit**

```sh
git add scripts/build/build-site.sh
git add scripts/build/verify-site-tree.sh
git add scripts/build/build-release.sh
git add scripts/run/run-release-server.sh
git status --short
git commit -m "site and release scripts"
```

Expected `git status --short` before the commit:

```text
A  scripts/build/build-release.sh
A  scripts/build/build-site.sh
A  scripts/build/verify-site-tree.sh
A  scripts/run/run-release-server.sh
```

### Task 29: Architecture as built, and the done condition

The architecture records the decisions this plan took (below), then every suite runs and two browsers play one game.

**Files:**
- Modify: `docs/architecture/client-web.md`
- Modify: `docs/architecture/overview.md`
- Modify: `docs/architecture/protocol.md`
- Modify: `docs/architecture/testing.md`

- [ ] **Step 1: Record the web client as built**

In `docs/architecture/overview.md`, replace:

```markdown
  - Whole browser-only modules are gated at their `pub mod` with a one-line WHY comment: `client/` (`// browser runtime glue; the ssr build compiles none of it`), `canvas/driver.rs`, `canvas/keyboard.rs`, `message/text_measurement.rs`.
  - A component which needs one browser call holds a same-file `#[cfg(feature = "hydrate")] mod browser` / `#[cfg(feature = "ssr")] mod server` pair exposing the same function (the ssr one a no-op), per `conditional-compilation.md`.
```

with:

```markdown
  - Whole browser-only modules are gated at their `pub mod` with a one-line WHY comment: `client/` (`// browser runtime glue; the ssr build compiles none of it`) and `canvas/driver.rs`.
  - A component which needs a browser call holds a same-file `#[cfg(feature = "hydrate")] mod browser` / `#[cfg(not(feature = "hydrate"))] mod server` pair exposing the same functions (the server ones no-ops), per `conditional-compilation.md`; the complement is `not(hydrate)` rather than `ssr` so the featureless workspace build compiles too (`canvas/game_canvas.rs`).
```

In `docs/architecture/overview.md`, replace:

```markdown
lib.rs  main.rs  app.rs       shell() (ssr), App (screen switch)
server.rs                     (ssr) dev serve(), prerender
client/                       (hydrate)
         hydrate.rs           single #[wasm_bindgen] entry
         game_socket.rs       web-sys WebSocket wrapper
         game_server_config.rs  reads static/game-server.json
         local_storage_settings.rs  SettingsStore over localStorage
         fullscreen.rs
screen/  screen.rs            Screen enum, navigation, Escape mapping
canvas/  game_canvas.rs       canvas component (mount through a browser/server submodule pair)
         driver.rs            (hydrate) thread_local DRIVER, rAF scheduling, renderer ownership
         keyboard.rs          (hydrate) KeyboardEvent to GameKeyKind
title/   title_menu.rs        includes the connection notice line (client-web.md#shell)
browser/ game_browser.rs
menu/    menu_form.rs         generic form engine, field kinds, per-mode visibility
         create_menu.rs  join_menu.rs  spectate_menu.rs  respawn_menu.rs  pause_menu.rs
hud/     ability_bar.rs  ability_icon_view.rs  name_labels.rs
leaderboard/ leaderboard.rs
message/ message_box.rs
         text_measurement.rs  (hydrate) DOM text width
tutorial/ tutorial_view.rs
style/   main.scss + partials (_tokens, _reset, _typography, _layout, components/_*.scss)
static/  fonts/bacter.ttf, favicon, robots.txt, game-server.json
tests/   prerender.rs         #![cfg(feature = "ssr")]
```

with:

```markdown
lib.rs  main.rs  app.rs       shell() (ssr), App (layers, screen switch), AppSignals, ClientSettings
server.rs                     (ssr) dev serve(), prerender
client/                       (hydrate)
         hydrate.rs           single #[wasm_bindgen] entry
         game_socket.rs       web-sys WebSocket wrapper
         game_server_config.rs  reads static/game-server.json
         local_storage_settings.rs  SettingsStore over localStorage
         fullscreen.rs        DisplayModeKind by the original's test
screen/  screen.rs            Screen enum, navigation, Escape mapping, LobbyNoticeKind
connection/ connection_status.rs  ConnectionStatusKind, notice line, Reconnect offer
         websocket_url.rs     socket URL from game-server.json or the page's origin
canvas/  game_canvas.rs       canvas component, DriverCommandKind (browser/server submodule pair)
         driver.rs            (hydrate) thread_local DRIVER, rAF scheduling, renderer ownership, command queue
         keyboard.rs          KeyboardEvent.code to GameKeyKind, default actions, shot aim
         scene_camera.rs      camera and crosshair per scene
         scene_overlay.rs     overlay and in-game menu views per scene
title/   title_menu.rs        includes the connection notice line (client-web.md#shell)
browser/ game_browser.rs      GameListView
menu/    menu_form.rs         generic form engine, field kinds, per-mode visibility, issue placement
         loadout_fields.rs    rows shared by the join, respawn and pause menus
         game_menu_view.rs    the own member's game, for the in-game menus
         create_menu.rs  join_menu.rs  spectate_menu.rs  respawn_menu.rs  pause_menu.rs
hud/     ability_bar.rs  ability_icon_view.rs  name_labels.rs
leaderboard/ leaderboard.rs
message/ message_box.rs
tutorial/ tutorial_view.rs
test_fixture/ test_fixture.rs (test) game states for unit tests
style/   main.scss + partials (_tokens, _reset, _typography, _layout, components/_*.scss)
static/  fonts/bacter.ttf, favicon, robots.txt, game-server.json
tests/   prerender.rs         #![cfg(feature = "ssr")]
```

In `docs/architecture/client-web.md`, replace:

```markdown
- `LeftGame` returns to Browser; `GameEnded` returns to Browser with the notice "The game has closed"; `SynchronisationLost` returns to Title with "Lost synchronisation with the game".
```

with:

```markdown
- `LeftGame` returns to Browser; `GameEnded` returns to Browser with the notice "The game has closed"; `SynchronisationLost` returns to Title with "Lost synchronisation with the game".
  - The browser shows its notice under the table; the title shows it on the notice line while connected.
- The browser subscribes to the game list each time it opens while connected (an accepted create, join or spectate ends the subscription); the client never unsubscribes.
```

In `docs/architecture/client-web.md`, replace:

```markdown
- `alert()` replaced by the message box or inline form errors.
```

with:

```markdown
- `alert()` replaced by the message box or inline form errors.
- A renderer which cannot be created is logged (`the game cannot be drawn in this browser`); the menus keep working.
- The title notice line sits centred below the title box.
```

In `docs/architecture/client-web.md`, replace:

```markdown
- Pause game: Color (not skm), Skin, Name Labels, Messages, Leave Game; Apply sends `UpdateAppearance` and stores settings.
```

with:

```markdown
- Pause game: Color (not skm), Skin, Name Labels, Messages, Leave Game; Apply sends `UpdateAppearance` when the colour or skin changed, and stores settings.
```

In `docs/architecture/client-web.md`, replace:

```markdown
shot or secretion timer disc r 8 at x - 41; active dots:
```

with:

```markdown
shot or secretion timer disc r 8 at x - 41 (a shot's or spore's secretion dial drains over the secretion duration alone; the original continued from where the flight stopped); active dots:
```

In `docs/architecture/client-web.md`, replace:

```markdown
- `web/src/client/local_storage_settings.rs`: `LocalStorageSettingsStore`.
```

with:

```markdown
- `web/src/client/local_storage_settings.rs`: `LocalStorageSettingsStore`; key `bacter.` plus the setting's storage key, value the `SettingValue` bytes as lowercase hexadecimal.
```

In `docs/architecture/client-web.md`, replace:

```markdown
- `keydown`/`keyup`: `keyboard.key_changed(GameKeyKind, KeyTransitionKind, repeat)` (`canvas/keyboard.rs`).
```

with:

```markdown
- `keydown`/`keyup`: `keyboard.key_changed(GameKeyKind, KeyTransitionKind, repeat)`, keys from `KeyboardEvent.code` (`canvas/keyboard.rs`):
  - game keys reach the `KeyboardState` only while the screen's input focus is `Game`; a change to `Menu` releases every held key and drops unsent presses;
  - Escape goes back from every screen ([shell](#shell)); R acts on `Spectating` only.
- Pointer: the shot aim is the pointer's offset from the crosshair's view point, refreshed on `mousemove` and every frame.
- The connection's `ClientSession` waits beside the title scene and the tutorial and moves into `ActiveScene::Online` on `JoinAccepted`; `LeftGame`, `GameEnded` and `SynchronisationLost` move it back.
- Views command the driver with `canvas::dispatch_driver_command(DriverCommandKind)`; commands queue until the driver can run them.
```

In `docs/architecture/client-web.md`, replace:

```markdown
- HTML, Leptos.
```

with:

```markdown
- HTML, Leptos.
- Drawn under the shade of the in-game menus, as the original drew them in its canvas.
```

In `docs/architecture/client-web.md`, replace:

```markdown
  - Box width from measured text width (DOM), not character count.
```

with:

```markdown
  - The box shrinks to its text, so its width is the DOM's measure of the text, not a character count.
```

In `docs/architecture/protocol.md`, replace:

```markdown
- One binary WebSocket per client at `/ws?protocol_version=<PROTOCOL_VERSION>`; text frames are a protocol error.
```

with:

```markdown
- One binary WebSocket per client at `/ws?protocol_version=<PROTOCOL_VERSION>` (`protocol::get_versioned_websocket_url`); text frames are a protocol error.
```

In `docs/architecture/testing.md`, replace:

```markdown
- `lobby/lobby_broadcast.rs`: revision on summary and client-count changes, at most one list per second.
```

with:

```markdown
- `lobby/lobby_broadcast.rs`: revision on summary and client-count changes, at most one list per second.
- `web` (host `#[test]`, `cargo test -p web --lib`):
  - `screen/screen.rs`: input focus, backdrop, back screens, own-state and connection-loss transitions, lobby notices;
  - `connection/`: notices, Reconnect offer, socket URL;
  - `menu/menu_form.rs`: issue placement, rejection fields, number input, radio toggling;
  - `menu/create_menu.rs`, `join_menu.rs`, `spectate_menu.rs`, `respawn_menu.rs`, `pause_menu.rs`: defaults, validation and the requests built;
  - `menu/loadout_fields.rs`, `menu/game_menu_view.rs`: loadouts, team choices, team options;
  - `hud/`: dials, shot dials, effect dots, icon paths, name label placement and truncation;
  - `leaderboard/leaderboard.rs`: rows and emphasis per mode;
  - `canvas/`: scene views, scene overlays, key mapping, aim;
  - `tutorial/tutorial_view.rs`, `app.rs`: tutorial overlay, client settings, back navigation.
```

In `docs/architecture/testing.md`, replace:

```markdown
- `web/tests/prerender.rs` (ssr): prerendered document contains the canvas root, `/pkg/`, `.wasm`, `.css`, `<title>`.
```

with:

```markdown
- `web/tests/prerender.rs` (ssr): prerendered document contains the canvas root, the title screen, `/pkg/`, `.wasm`, `.css`, `<title>`.
```

- [ ] **Step 2: Run every suite**

Run: `cargo test --workspace 2>&1 | grep -E '^test result' && cargo test -p web --features ssr --test prerender 2>&1 | grep -E '^test result'`

Expected: every `test result` line is `ok` (the server's integration tests bind loopback ports):

```text
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 120 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 13 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 505 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: ok. 83 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

- [ ] **Step 3: Check the browser builds and the formatting**

Run: `./scripts/test/check-wasm.sh 2>&1 | grep -E '^(warning|error)|Finished' && cargo fmt --all -- --check && echo formatted`

Expected:

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s)
    Finished `dev` profile [unoptimized + debuginfo] target(s)
    Finished `dev` profile [unoptimized + debuginfo] target(s)
formatted
```

- [ ] **Step 4: Play two browsers together**

The roadmap's done condition, with the servers of Task 27 running again: `cargo leptos watch` serves the game against `./scripts/dev/server.sh`, and two pages in separate isolated contexts create and join one Skirmish game (2 teams): the joining page's team list reads "Red: 1" and "Blue: 0" with Blue preselected, choosing Red shows "Cannot join red team because it already has more players than blue" under Auto Assign, and Blue joins; both pages show the "Team, Kills, Deaths, K:D" leaderboard with their own team in bold and both organisms. Stop every process you started by its PID.

- [ ] **Step 5: Commit**

```sh
git add docs/architecture/client-web.md
git add docs/architecture/overview.md
git add docs/architecture/protocol.md
git add docs/architecture/testing.md
git status --short
git commit -m "architecture docs for the web client"
```

Expected `git status --short` before the commit:

```text
M  docs/architecture/client-web.md
M  docs/architecture/overview.md
M  docs/architecture/protocol.md
M  docs/architecture/testing.md
```

## Coverage of the roadmap

- Shell (app.rs, hydrate, layers, screens): Tasks 3, 6 and 26.
- Screens and navigation: Task 4; title: Task 17; game browser: Task 11.
- Menu form engine: Task 8; create, join, spectate, respawn and pause menus: Tasks 9, 10 and 12 to 16.
- HUD: ability bar and icons Task 18, name labels Task 19; leaderboard: Task 20; message box: Task 21; tutorial view: Task 22.
- Driver: Task 27, with the scene camera (Task 23), the scene overlays (Task 22), the keyboard (Task 24) and the command seam (Task 7).
- Socket, `game-server.json`, local storage settings: Tasks 5 and 25.
- Styles and font: Task 3 (base, font face and the font file) and each view's task.
- `game-server.json` and prerender: Task 3, with the title screen and canvas assertions in Tasks 26 and 27.
- `build-site.sh`, `verify-site-tree.sh`, `build-release.sh`, `run-release-server.sh`: Task 28; `scripts/dev/web.sh` (in the dev workflow, not yet present): Task 3; `scripts/test/test-wasm.sh` (the localStorage tests of testing.md): Task 25.
- Done condition: the prerender test (Tasks 26 and 27), a playable game under `cargo leptos watch` (Task 27) and two browsers together (Tasks 27 and 29).
- Left to phase 9: the wasm replay test under wasmtime, the growth statistics script, the manual verification list (Safari, device pixel ratios, throttled networks), the bug-fix ledger walk-through and the documentation pass.

## PR description

**Problem:** bacter-rs could simulate, serve and draw a game, but had no browser client to play it.

**Solution:** the `web` crate: a prerendered Leptos shell with the original's title, game browser and create, join, spectate, respawn and pause menus on a shared menu form engine; HTML overlays for the ability bar, leaderboard, message box and name labels; and a browser driver which owns the wgpu renderer, the title scene, tutorial and online session, the keyboard and the WebSocket. Settings persist in localStorage; `build-site.sh`, `build-release.sh` and `run-release-server.sh` build and serve the static site and the game socket from one origin. Host unit tests cover what every view shows, and a prerender test the document. Repository: `bacter-rs` only.
