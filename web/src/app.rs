use leptos::prelude::*;

use shared::play::OwnMemberStateKind;
use shared::protocol::TeamChoiceKindSerialIn;
use shared::settings;
use shared::settings::SettingsStore;

use crate::browser::{GameBrowser, GameListView};
use crate::canvas::GameCanvas;
use crate::connection::ConnectionStatusKind;
use crate::hud::{AbilityBar, AbilityBarView, NameLabelView, NameLabels};
use crate::leaderboard::{Leaderboard, LeaderboardView};
use crate::menu::{
    CreateMenu, CreateMenuFields, GameMenuView, JoinMenu, PauseGameMenu, PauseSpectateMenu, PauseTutorialMenu,
    RequestRejection, RespawnMenu, SpectateMenu,
};
use crate::message::MessageBox;
use crate::screen::{LobbyNoticeKind, Screen, ScreenBackdropKind};
use crate::title::{PrerenderedTitleWorld, TitleMenu};

#[cfg(feature = "ssr")] // a browser build renders no document
pub use ssr::shell;

/// The reactive state the driver and the views share.
#[derive(Clone, Copy, Debug)]
pub struct AppSignals {
    pub screen: RwSignal<Screen>,
    pub connection_status: RwSignal<ConnectionStatusKind>,
    pub first_frame: RwSignal<FirstFrameKind>,
    pub lobby_notice: RwSignal<Option<LobbyNoticeKind>>,
    pub game_list: RwSignal<GameListView>,
    pub request_rejection: RwSignal<Option<RequestRejection>>,
    pub own_member_state: RwSignal<Option<OwnMemberStateKind>>,
    pub game_menu: RwSignal<Option<GameMenuView>>,
    pub ability_bar: RwSignal<Option<AbilityBarView>>,
    pub leaderboard: RwSignal<Option<LeaderboardView>>,
    /// `None` while the Messages setting is off.
    pub message_text: RwSignal<Option<String>>,
    pub name_labels: RwSignal<Vec<NameLabelView>>,
    pub client_settings: RwSignal<ClientSettings>,
    /// The team choice last sent in a join or respawn.
    pub team_choice: RwSignal<TeamChoiceKindSerialIn>,
}

impl AppSignals {
    pub fn new() -> AppSignals {
        AppSignals {
            screen: RwSignal::new(Screen::Title),
            connection_status: RwSignal::new(ConnectionStatusKind::Loading),
            first_frame: RwSignal::new(FirstFrameKind::NotDrawn),
            lobby_notice: RwSignal::new(None),
            game_list: RwSignal::new(GameListView::empty()),
            request_rejection: RwSignal::new(None),
            own_member_state: RwSignal::new(None),
            game_menu: RwSignal::new(None),
            ability_bar: RwSignal::new(None),
            leaderboard: RwSignal::new(None),
            message_text: RwSignal::new(None),
            name_labels: RwSignal::new(Vec::new()),
            client_settings: RwSignal::new(ClientSettings::defaults()),
            team_choice: RwSignal::new(TeamChoiceKindSerialIn::Auto),
        }
    }

    /// A rejection belongs to the screen whose request it answers.
    pub fn navigate(&self, screen: Screen) {
        self.request_rejection.set(None);
        self.screen.set(screen);
    }

    pub fn lobby_notice_text(&self, kind: LobbyNoticeKind) -> Option<String> {
        self.lobby_notice
            .get()
            .filter(|lobby_notice| *lobby_notice == kind)
            .map(|lobby_notice| lobby_notice.text())
    }

    pub fn go_back(&self) {
        let back_screen: Option<Screen> =
            self.screen.get_untracked().get_back_screen(self.own_member_state.get_untracked());

        if let Some(back_screen) = back_screen {
            self.navigate(back_screen);
        }
    }
}

/// Whether the renderer has drawn a frame on the canvas yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstFrameKind {
    NotDrawn,
    Drawn,
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
mod ssr {
    use leptos::hydration::{AutoReload, HydrationScripts};
    use leptos::prelude::*;
    use leptos_meta::HashedStylesheet;

    use super::App;

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
}

#[component]
pub fn App() -> impl IntoView {
    let signals: AppSignals = AppSignals::new();
    provide_context(signals);
    provide_context(CreateMenuFields::new());

    let screen: Memo<Screen> = Memo::new(move |_| signals.screen.get());
    let is_shaded = move || screen.get().backdrop() == ScreenBackdropKind::Shaded;

    view! {
        <PrerenderedTitleWorld />
        <GameCanvas />
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

pub fn set_if_changed<T: PartialEq + Send + Sync + 'static>(signal: RwSignal<T>, value: T) {
    let is_changed: bool = signal.with_untracked(|current_value| *current_value != value);

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

#[cfg(test)]
mod tests {
    use super::*;

    use leptos::reactive::effect::ImmediateEffect;

    use shared::protocol::{RejectionKind, RequestKind};
    use shared::settings::InMemorySettingsStore;

    #[test]
    fn read_gives_the_defaults_from_an_empty_store() {
        let empty_store_settings: ClientSettings = ClientSettings::read(&InMemorySettingsStore::default());

        assert_eq!(empty_store_settings, ClientSettings::defaults());
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
        let store: InMemorySettingsStore = InMemorySettingsStore::default();
        let client_settings: ClientSettings = ClientSettings {
            name_labels_shown: false,
            messages_shown: false,
        };

        client_settings.write(&store);

        assert_eq!(ClientSettings::read(&store), client_settings);
    }

    #[test]
    fn go_back_opens_the_back_screen_and_clears_the_rejection() {
        let owner: Owner = Owner::new();
        owner.set();
        let signals: AppSignals = AppSignals::new();
        signals.screen.set(Screen::Browser);
        signals.request_rejection.set(Some(RequestRejection {
            request: RequestKind::JoinGame,
            reason: RejectionKind::GameFull,
        }));

        signals.go_back();

        assert_eq!(signals.screen.get_untracked(), Screen::Title);
        assert_eq!(signals.request_rejection.get_untracked(), None);
    }

    #[test]
    fn go_back_on_title_keeps_the_screen() {
        let owner: Owner = Owner::new();
        owner.set();
        let signals: AppSignals = AppSignals::new();

        signals.go_back();

        assert_eq!(signals.screen.get_untracked(), Screen::Title);
    }

    #[test]
    fn lobby_notice_text_gives_only_the_requested_notice() {
        let owner: Owner = Owner::new();
        owner.set();
        let signals: AppSignals = AppSignals::new();

        assert_eq!(signals.lobby_notice_text(LobbyNoticeKind::GameClosed), None);

        signals.lobby_notice.set(Some(LobbyNoticeKind::GameClosed));

        assert_eq!(
            signals.lobby_notice_text(LobbyNoticeKind::GameClosed),
            Some(LobbyNoticeKind::GameClosed.text()),
        );
        assert_eq!(signals.lobby_notice_text(LobbyNoticeKind::SynchronisationLost), None);
    }

    #[test]
    fn set_if_changed_notifies_only_for_a_different_value() {
        let owner: Owner = Owner::new();
        owner.set();
        let signal: RwSignal<u32> = RwSignal::new(1);
        let run_count: RwSignal<u32> = RwSignal::new(0);
        let _effect: ImmediateEffect = ImmediateEffect::new_isomorphic(move || {
            signal.track();
            run_count.update_untracked(|count| *count += 1);
        });

        set_if_changed(signal, 1);
        assert_eq!(run_count.get_untracked(), 1);

        set_if_changed(signal, 2);
        assert_eq!(run_count.get_untracked(), 2);
        assert_eq!(signal.get_untracked(), 2);
    }
}
