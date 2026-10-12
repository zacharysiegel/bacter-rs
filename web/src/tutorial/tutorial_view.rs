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
