use crate::game::GameState;
use crate::play::{ClientSession, FrameOutput, PlayerControls};
use crate::title::TitleScene;
use crate::tutorial::Tutorial;

/// Whether keyboard input steers the game or a menu over it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputFocusKind {
    Game,
    Menu,
}

/// The scene the platform advances every frame and renders.
#[derive(Clone, Debug, PartialEq)]
pub enum ActiveScene {
    Online(ClientSession),
    Tutorial(Tutorial),
    Title(TitleScene),
}

impl ActiveScene {
    pub fn advance_frame(
        &mut self,
        now_milliseconds: f64,
        controls: &mut PlayerControls,
        input_focus: InputFocusKind,
    ) -> FrameOutput {
        match self {
            ActiveScene::Online(client_session) => {
                client_session.advance_frame(now_milliseconds, controls, input_focus)
            }
            ActiveScene::Tutorial(tutorial) => tutorial.advance_frame(now_milliseconds, controls, input_focus),
            ActiveScene::Title(title_scene) => title_scene.advance_frame(now_milliseconds),
        }
    }

    /// `None` while an online scene has no game state.
    pub fn render_state(&self) -> Option<&GameState> {
        match self {
            ActiveScene::Online(client_session) => client_session.render_state(),
            ActiveScene::Tutorial(tutorial) => Some(tutorial.state()),
            ActiveScene::Title(title_scene) => Some(title_scene.state()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Tick;
    use crate::geometry::SubpixelPoint;
    use crate::play::frame_pacing;
    use crate::play::{GameKeyKind, KeyRepeatKind, KeyTransitionKind, PixelSize};
    use crate::tutorial::DisplayModeKind;

    const SEED: u64 = 3;
    const WINDOW_SIZE: PixelSize = PixelSize {
        width_pixels: 850,
        height_pixels: 650,
    };
    /// The crosshair's travel in one tick at `CROSSHAIR_SPEED_PIXELS_PER_SECOND`.
    const CROSSHAIR_TRAVEL_SUBPIXELS_PER_TICK: i32 = 3046;

    #[test]
    fn advance_frame_steps_the_title_scene() {
        let mut controls: PlayerControls = PlayerControls::new();
        let mut active_scene: ActiveScene = ActiveScene::Title(TitleScene::new(WINDOW_SIZE, SEED));

        active_scene.advance_frame(0.0, &mut controls, InputFocusKind::Menu);
        let frame_output: FrameOutput = active_scene.advance_frame(
            frame_pacing::TICK_PERIOD_MILLISECONDS,
            &mut controls,
            InputFocusKind::Menu,
        );

        assert_eq!(frame_output.tick_applied, Some(Tick(2)));
        assert_eq!(active_scene.render_state().map(|state| state.tick), Some(Tick(2)));
    }

    #[test]
    fn advance_frame_steers_the_tutorial_crosshair_with_the_game_focus() {
        let mut controls: PlayerControls = PlayerControls::new();
        let tutorial: Tutorial = Tutorial::new(WINDOW_SIZE, SEED, DisplayModeKind::Windowed, &mut controls);
        let start: SubpixelPoint = controls.crosshair.position();
        let mut active_scene: ActiveScene = ActiveScene::Tutorial(tutorial);

        controls.keyboard.key_changed(
            GameKeyKind::MoveRight,
            KeyTransitionKind::Pressed,
            KeyRepeatKind::Initial,
        );

        active_scene.advance_frame(0.0, &mut controls, InputFocusKind::Game);
        active_scene.advance_frame(
            frame_pacing::TICK_PERIOD_MILLISECONDS,
            &mut controls,
            InputFocusKind::Game,
        );

        assert_eq!(
            controls.crosshair.position(),
            SubpixelPoint {
                x: start.x + CROSSHAIR_TRAVEL_SUBPIXELS_PER_TICK,
                y: start.y,
            },
        );
    }

    #[test]
    fn render_state_is_none_for_an_online_scene_outside_a_game() {
        let active_scene: ActiveScene = ActiveScene::Online(ClientSession::new());

        assert_eq!(active_scene.render_state(), None);
    }
}
