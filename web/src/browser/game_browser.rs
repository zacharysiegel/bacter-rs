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
    let notice_text = move || signals.lobby_notice_text(LobbyNoticeKind::GameClosed);

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
