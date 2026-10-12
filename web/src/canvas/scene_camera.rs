use shared::play::{ActiveScene, InterpolatedPoint, OwnMemberStateKind, PlayerControls};
use shared::render::{Camera, ViewportSize};

/// The camera a frame is drawn with, and the crosshair it draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneView {
    pub camera: Camera,
    pub crosshair: Option<InterpolatedPoint>,
}

/// Online the camera follows the crosshair, drawn only for a playing member; local games keep the world still.
pub fn get_scene_view(active_scene: &ActiveScene, controls: &PlayerControls, viewport: ViewportSize) -> SceneView {
    let crosshair: InterpolatedPoint = InterpolatedPoint::from_subpixel_point(controls.crosshair.position());

    match active_scene {
        ActiveScene::Online(client_session) => SceneView {
            camera: Camera::centred_on(crosshair, viewport),
            crosshair: match client_session.own_member_state() {
                Some(OwnMemberStateKind::AliveParticipant) => Some(crosshair),
                Some(OwnMemberStateKind::DeadParticipant | OwnMemberStateKind::Spectator) | None => None,
            },
        },
        ActiveScene::Tutorial(_) => SceneView {
            camera: Camera::for_local_game(viewport),
            crosshair: Some(crosshair),
        },
        ActiveScene::Title(_) => SceneView {
            camera: Camera::for_local_game(viewport),
            crosshair: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::game::{GameModeKind, GameState};
    use shared::geometry::{SubpixelPoint, WorldPoint};
    use shared::member::MemberId;
    use shared::play::{ClientSession, PixelSize};
    use shared::render::PhysicalSize;
    use shared::title::TitleScene;
    use shared::tutorial::{DisplayModeKind, Tutorial};
    use shared::world::WorldShapeKind;

    use crate::test_fixture;

    const OWN_MEMBER_ID: MemberId = MemberId(0);
    const SEED: u64 = 5;
    const WINDOW_SIZE: PixelSize = PixelSize {
        width_pixels: 800,
        height_pixels: 600,
    };
    const VIEWPORT: ViewportSize = ViewportSize {
        physical_size: PhysicalSize {
            width_pixels: 1600,
            height_pixels: 1200,
        },
        device_pixel_ratio: 2.0,
    };

    fn create_controls_at(position: SubpixelPoint) -> PlayerControls {
        let mut controls: PlayerControls = PlayerControls::new();
        controls.crosshair.snap_to(position);

        controls
    }

    #[test]
    fn get_scene_view_follows_the_crosshair_online_without_drawing_it_outside_play() {
        let controls: PlayerControls = create_controls_at(SubpixelPoint {
            x: 1024 * 500,
            y: 1024 * 400,
        });
        let active_scene: ActiveScene = ActiveScene::Online(ClientSession::new());

        let scene_view: SceneView = get_scene_view(&active_scene, &controls, VIEWPORT);

        assert_eq!(
            scene_view.camera,
            Camera::centred_on(InterpolatedPoint { x: 500.0, y: 400.0 }, VIEWPORT),
        );
        assert_eq!(scene_view.crosshair, None);
    }

    #[test]
    fn get_scene_view_draws_the_crosshair_of_a_playing_member() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        test_fixture::insert_member(
            &mut state,
            test_fixture::create_participant_with_organism(OWN_MEMBER_ID, WorldPoint { x: 200, y: 200 }),
        );
        let mut controls: PlayerControls = PlayerControls::new();
        let client_session: ClientSession = test_fixture::create_session_in_game(&state, OWN_MEMBER_ID, &mut controls);
        let active_scene: ActiveScene = ActiveScene::Online(client_session);

        let scene_view: SceneView = get_scene_view(&active_scene, &controls, VIEWPORT);

        assert_eq!(
            scene_view.crosshair,
            Some(InterpolatedPoint::from_subpixel_point(controls.crosshair.position())),
        );
    }

    #[test]
    fn get_scene_view_draws_no_crosshair_for_a_dead_participant() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        test_fixture::insert_member(&mut state, test_fixture::create_participant(OWN_MEMBER_ID));
        let mut controls: PlayerControls = PlayerControls::new();
        let client_session: ClientSession = test_fixture::create_session_in_game(&state, OWN_MEMBER_ID, &mut controls);
        let active_scene: ActiveScene = ActiveScene::Online(client_session);

        let scene_view: SceneView = get_scene_view(&active_scene, &controls, VIEWPORT);

        assert_eq!(scene_view.crosshair, None);
    }

    #[test]
    fn get_scene_view_draws_the_tutorial_crosshair_over_a_still_world() {
        let mut controls: PlayerControls = PlayerControls::new();
        let tutorial: Tutorial = Tutorial::new(WINDOW_SIZE, SEED, DisplayModeKind::Windowed, &mut controls);
        let crosshair: InterpolatedPoint = InterpolatedPoint::from_subpixel_point(controls.crosshair.position());

        let scene_view: SceneView = get_scene_view(&ActiveScene::Tutorial(tutorial), &controls, VIEWPORT);

        assert_eq!(scene_view.camera, Camera::for_local_game(VIEWPORT));
        assert_eq!(scene_view.crosshair, Some(crosshair));
    }

    #[test]
    fn get_scene_view_draws_no_crosshair_over_the_title_scene() {
        let controls: PlayerControls = PlayerControls::new();
        let active_scene: ActiveScene = ActiveScene::Title(TitleScene::new(WINDOW_SIZE, SEED));

        let scene_view: SceneView = get_scene_view(&active_scene, &controls, VIEWPORT);

        assert_eq!(scene_view.camera, Camera::for_local_game(VIEWPORT));
        assert_eq!(scene_view.crosshair, None);
    }
}
