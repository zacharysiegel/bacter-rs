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
