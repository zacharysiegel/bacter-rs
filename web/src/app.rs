use leptos::prelude::*;

use shared::play::OwnMemberStateKind;
use shared::settings;
use shared::settings::SettingsStore;

use crate::connection::ConnectionStatusKind;
use crate::screen::{LobbyNoticeKind, Screen};

#[cfg(feature = "ssr")] // a browser build renders no document
pub use ssr::shell;

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

    view! { <div class="screen-layer"></div> }
}

pub fn set_if_changed<T: PartialEq + Send + Sync + 'static>(signal: RwSignal<T>, value: T) {
    let is_changed: bool = signal.with_untracked(|current_value| *current_value != value);

    if is_changed {
        signal.set(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use leptos::reactive::effect::ImmediateEffect;

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
    fn go_back_opens_the_back_screen() {
        let owner: Owner = Owner::new();
        owner.set();
        let signals: AppSignals = AppSignals::new();
        signals.screen.set(Screen::Browser);

        signals.go_back();

        assert_eq!(signals.screen.get_untracked(), Screen::Title);
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
