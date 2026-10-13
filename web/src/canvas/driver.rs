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
use shared::protocol::close_reason;
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
use crate::connection::{ConnectionStatusKind, PageShowEffectKind};
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
const PAGEHIDE_EVENT: &str = "pagehide";
const PAGESHOW_EVENT: &str = "pageshow";

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

                self.navigate_if_changed(next_screen);
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

        self.navigate_if_changed(next_screen);
    }

    fn navigate_if_changed(&self, next_screen: Screen) {
        let screen: Screen = self.signals.screen.get_untracked();

        // navigating also clears the request rejection
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

    /// The back/forward cache keeps a hidden page's socket open.
    fn page_hidden(&mut self) {
        if let Some(socket) = self.socket.take() {
            socket.close_normally();
        }
    }

    fn page_shown(&mut self, page_show_effect: PageShowEffectKind) {
        match page_show_effect {
            PageShowEffectKind::ConnectionLost => self.socket_closed(close_reason::NORMAL_CLOSURE_CODE),
            PageShowEffectKind::Unchanged => {}
        }
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

/// Does nothing while the driver is absent or already borrowed.
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
    add_window_listener(&window, PAGEHIDE_EVENT, |_event: Event| {
        with_driver(Driver::page_hidden)
    })?;
    add_window_listener(&window, PAGESHOW_EVENT, |event: Event| {
        let page_show_effect: PageShowEffectKind = client::get_page_show_effect(&event);

        with_driver(|driver: &mut Driver| driver.page_shown(page_show_effect));
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
