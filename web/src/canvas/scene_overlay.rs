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

#[cfg(test)]
mod tests {
    use super::*;
    use shared::game::{GameModeKind, GameState};
    use shared::geometry::WorldPoint;
    use shared::member::MemberId;
    use shared::play::{PixelSize, PlayerControls};
    use shared::title::TitleScene;
    use shared::world::WorldShapeKind;

    use crate::test_fixture;

    const OWN_MEMBER_ID: MemberId = MemberId(0);

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
        let client_session: ClientSession = test_fixture::create_session_joined(OWN_MEMBER_ID, &mut controls);
        let active_scene: ActiveScene = ActiveScene::Online(client_session);

        assert_eq!(SceneOverlay::from_active_scene(&active_scene), SceneOverlay::empty());
    }

    #[test]
    fn from_active_scene_shows_the_ability_bar_leaderboard_and_menu_view_of_a_playing_member() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        test_fixture::insert_member(
            &mut state,
            test_fixture::create_participant_with_organism(OWN_MEMBER_ID, WorldPoint { x: 200, y: 200 }),
        );
        let mut controls: PlayerControls = PlayerControls::new();
        let client_session: ClientSession = test_fixture::create_session_in_game(&state, OWN_MEMBER_ID, &mut controls);
        let active_scene: ActiveScene = ActiveScene::Online(client_session);

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
