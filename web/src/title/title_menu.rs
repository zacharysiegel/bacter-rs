use leptos::prelude::*;

use shared::geometry;

use crate::app::AppSignals;
use crate::canvas::DriverCommandKind;
use crate::canvas::game_canvas;
use crate::connection::{ActivityIndicatorKind, MenuButtonAccessKind, ReconnectOfferKind, StatusBoxView};
use crate::menu::CreateMenuFields;
use crate::screen::{LobbyNoticeKind, Screen};

const RECONNECT_TEXT: &str = "Reconnect";
/// Each grows in turn, as an organism's cells do.
const ACTIVITY_INDICATOR_CELL_COUNT: usize = 4;

/// The box over the title scene, with the status box beneath it.
#[component]
pub fn TitleMenu() -> impl IntoView {
    let signals: AppSignals = expect_context();
    let create_menu_fields: CreateMenuFields = expect_context();

    let lobby_access: Signal<MenuButtonAccessKind> =
        Signal::derive(move || signals.connection_status.get().lobby_access());
    let tutorial_access: Signal<MenuButtonAccessKind> =
        Signal::derive(move || signals.connection_status.get().tutorial_access());

    let open_create_menu = move || {
        create_menu_fields.reset();
        signals.navigate(Screen::CreateMenu);
    };
    let open_browser = move || signals.navigate(Screen::Browser);
    let start_tutorial = move || game_canvas::dispatch_driver_command(DriverCommandKind::StartTutorial);

    let status_box = move || {
        let connection_status_box: Option<StatusBoxView> = signals.connection_status.get().status_box();

        connection_status_box
            .or_else(|| signals.lobby_notice_text(LobbyNoticeKind::SynchronisationLost).map(StatusBoxView::notice))
    };

    view! {
        <div class="title-menu">
            <TitleMenuButton text="Host" access=lobby_access on_click=open_create_menu />
            <TitleMenuButton text="Join" access=lobby_access on_click=open_browser />
            <TitleMenuButton text="Tutorial" access=tutorial_access on_click=start_tutorial />
        </div>
        <div class="title-status-row">{move || status_box().map(|status_box| view! { <TitleStatusBox status_box /> })}</div>
    }
}

/// A disabled button ignores clicks.
#[component]
fn TitleMenuButton(
    text: &'static str,
    access: Signal<MenuButtonAccessKind>,
    on_click: impl Fn() + Send + Sync + 'static,
) -> impl IntoView {
    let is_disabled = move || access.get() == MenuButtonAccessKind::Disabled;
    let click = move |_| {
        if access.get_untracked() == MenuButtonAccessKind::Enabled {
            on_click();
        }
    };

    view! {
        <div class="title-menu-button" class:title-menu-button-disabled=is_disabled on:click=click>
            {text}
        </div>
    }
}

#[component]
fn TitleStatusBox(status_box: StatusBoxView) -> impl IntoView {
    let reconnect = move |_| game_canvas::dispatch_driver_command(DriverCommandKind::Reconnect);
    let activity_indicator =
        (status_box.activity_indicator == ActivityIndicatorKind::Shown).then(|| view! { <ActivityIndicator /> });
    let reconnect_button = (status_box.reconnect_offer == ReconnectOfferKind::Offered).then(|| {
        view! {
            <button class="title-reconnect" type="button" on:click=reconnect>
                {RECONNECT_TEXT}
            </button>
        }
    });

    view! {
        <div class="title-status">
            <span class="title-status-text">{status_box.text}</span>
            {activity_indicator}
            {reconnect_button}
        </div>
    }
}

/// Animated by CSS alone, so it moves before the client bundle has started.
#[component]
fn ActivityIndicator() -> impl IntoView {
    let cell_style: String = format!(
        "--activity-indicator-cell-width: {}px; --activity-indicator-cell-count: {}",
        geometry::CELL_WIDTH_PIXELS,
        ACTIVITY_INDICATOR_CELL_COUNT,
    );

    view! {
        <span class="activity-indicator" style=cell_style>
            {(0..ACTIVITY_INDICATOR_CELL_COUNT)
                .map(|_| view! { <span class="activity-indicator-cell"></span> })
                .collect_view()}
        </span>
    }
}
