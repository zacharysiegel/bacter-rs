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
