use crate::geometry::WorldPoint;
use crate::member::{Appearance, MemberId, OrganismColorKind, SkinKind};
use crate::play::local_game;
use crate::play::{LocalJoiner, PixelSize};
use crate::random::Pcg32;

/// The original's default range plus 30.
pub const TUTORIAL_BOT_MINIMUM_DISTANCE_PIXELS: i64 = 80;
pub const TUTORIAL_BOT_EXCLUDED_COLORS: [OrganismColorKind; 2] = [OrganismColorKind::Sun, OrganismColorKind::Lime];
/// A world too small for the minimum distance takes the last candidate.
const BOT_POSITION_ATTEMPT_LIMIT: u32 = 64;
const TUTORIAL_BOT_SCREEN_NAME: &str = "Bot";

/// The target of the compress, freeze, toxin and spore tasks; its cursor never moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TutorialBot {
    pub member_id: MemberId,
    pub cursor: WorldPoint,
}

pub fn create_tutorial_bot_joiner(
    scene_rng: &mut Pcg32,
    world_size: PixelSize,
    player_cursor: WorldPoint,
) -> LocalJoiner {
    LocalJoiner {
        screen_name: String::from(TUTORIAL_BOT_SCREEN_NAME),
        appearance: Appearance {
            color: local_game::draw_color_excluding(scene_rng, &TUTORIAL_BOT_EXCLUDED_COLORS),
            skin: SkinKind::None,
        },
        cursor: draw_bot_cursor(scene_rng, world_size, player_cursor),
    }
}

fn draw_bot_cursor(scene_rng: &mut Pcg32, world_size: PixelSize, player_cursor: WorldPoint) -> WorldPoint {
    let minimum_distance_squared: i64 = TUTORIAL_BOT_MINIMUM_DISTANCE_PIXELS * TUTORIAL_BOT_MINIMUM_DISTANCE_PIXELS;
    let mut candidate: WorldPoint = draw_world_point(scene_rng, world_size);

    for _ in 1..BOT_POSITION_ATTEMPT_LIMIT {
        if candidate.distance_squared(player_cursor) >= minimum_distance_squared {
            return candidate;
        }

        candidate = draw_world_point(scene_rng, world_size);
    }

    candidate
}

fn draw_world_point(scene_rng: &mut Pcg32, world_size: PixelSize) -> WorldPoint {
    WorldPoint {
        x: draw_world_coordinate(scene_rng, world_size.width_pixels),
        y: draw_world_coordinate(scene_rng, world_size.height_pixels),
    }
}

/// The world's centre when no pixel is clear of both edges.
fn draw_world_coordinate(scene_rng: &mut Pcg32, world_length_pixels: u32) -> i32 {
    let drawn_coordinate: Option<i32> = local_game::draw_cell_coordinate(scene_rng, 0, i64::from(world_length_pixels));

    drawn_coordinate.unwrap_or_else(|| get_centre_coordinate(world_length_pixels))
}

fn get_centre_coordinate(world_length_pixels: u32) -> i32 {
    i32::try_from(world_length_pixels / 2).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORLD_SIZE: PixelSize = PixelSize {
        width_pixels: 400,
        height_pixels: 300,
    };
    const PLAYER_CURSOR: WorldPoint = WorldPoint { x: 200, y: 150 };

    #[test]
    fn create_tutorial_bot_joiner_places_the_bot_inside_the_world_away_from_the_player() {
        for seed in 0..100 {
            let mut scene_rng: Pcg32 = local_game::create_scene_rng(seed);

            let bot_joiner: LocalJoiner = create_tutorial_bot_joiner(&mut scene_rng, WORLD_SIZE, PLAYER_CURSOR);
            let distance_squared: i64 = bot_joiner.cursor.distance_squared(PLAYER_CURSOR);

            assert!(distance_squared >= TUTORIAL_BOT_MINIMUM_DISTANCE_PIXELS * TUTORIAL_BOT_MINIMUM_DISTANCE_PIXELS);
            assert!((7..394).contains(&bot_joiner.cursor.x));
            assert!((7..294).contains(&bot_joiner.cursor.y));
        }
    }

    #[test]
    fn create_tutorial_bot_joiner_draws_neither_an_excluded_color_nor_a_skin() {
        for seed in 0..100 {
            let mut scene_rng: Pcg32 = local_game::create_scene_rng(seed);

            let appearance: Appearance =
                create_tutorial_bot_joiner(&mut scene_rng, WORLD_SIZE, PLAYER_CURSOR).appearance;

            assert!(!TUTORIAL_BOT_EXCLUDED_COLORS.contains(&appearance.color));
            assert_eq!(appearance.skin, SkinKind::None);
        }
    }

    #[test]
    fn create_tutorial_bot_joiner_takes_the_last_candidate_in_a_world_too_small_for_the_distance() {
        let mut scene_rng: Pcg32 = local_game::create_scene_rng(0);
        let small_world_size: PixelSize = PixelSize {
            width_pixels: 60,
            height_pixels: 60,
        };

        let bot_joiner: LocalJoiner =
            create_tutorial_bot_joiner(&mut scene_rng, small_world_size, WorldPoint { x: 30, y: 30 });

        assert!((7..54).contains(&bot_joiner.cursor.x));
    }
}
