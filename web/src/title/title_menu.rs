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

        connection_notice.or_else(|| signals.lobby_notice_text(LobbyNoticeKind::SynchronisationLost))
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
