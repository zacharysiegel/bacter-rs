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
        x: local_game::draw_cell_coordinate_or_centre(scene_rng, 0, i64::from(world_size.width_pixels)),
        y: local_game::draw_cell_coordinate_or_centre(scene_rng, 0, i64::from(world_size.height_pixels)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry;
    use std::ops::Range;

    const WORLD_SIZE: PixelSize = PixelSize {
        width_pixels: 400,
        height_pixels: 300,
    };
    const PLAYER_CURSOR: WorldPoint = WorldPoint { x: 200, y: 150 };
    const MINIMUM_DISTANCE_SQUARED: i64 = TUTORIAL_BOT_MINIMUM_DISTANCE_PIXELS * TUTORIAL_BOT_MINIMUM_DISTANCE_PIXELS;

    /// The coordinates at which a cell centre is clear of both world edges.
    fn get_cell_coordinate_range(world_length_pixels: u32) -> Range<i32> {
        let world_length: i32 = i32::try_from(world_length_pixels).unwrap();

        geometry::CELL_WIDTH_PIXELS + 1..world_length - geometry::CELL_WIDTH_PIXELS
    }

    fn is_inside_world(cursor: WorldPoint, world_size: PixelSize) -> bool {
        get_cell_coordinate_range(world_size.width_pixels).contains(&cursor.x)
            && get_cell_coordinate_range(world_size.height_pixels).contains(&cursor.y)
    }

    #[test]
    fn create_tutorial_bot_joiner_places_the_bot_inside_the_world_away_from_the_player() {
        for seed in 0..100 {
            let mut scene_rng: Pcg32 = local_game::create_scene_rng(seed);

            let bot_joiner: LocalJoiner = create_tutorial_bot_joiner(&mut scene_rng, WORLD_SIZE, PLAYER_CURSOR);
            let distance_squared: i64 = bot_joiner.cursor.distance_squared(PLAYER_CURSOR);

            assert!(distance_squared >= MINIMUM_DISTANCE_SQUARED);
            assert!(is_inside_world(bot_joiner.cursor, WORLD_SIZE));
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
        let small_world_size: PixelSize = PixelSize {
            width_pixels: 60,
            height_pixels: 60,
        };
        let small_world_player_cursor: WorldPoint = WorldPoint { x: 30, y: 30 };
        let mut scene_rng: Pcg32 = local_game::create_scene_rng(0);
        let mut expected_rng: Pcg32 = local_game::create_scene_rng(0);

        let bot_joiner: LocalJoiner =
            create_tutorial_bot_joiner(&mut scene_rng, small_world_size, small_world_player_cursor);

        let expected_color: OrganismColorKind =
            local_game::draw_color_excluding(&mut expected_rng, &TUTORIAL_BOT_EXCLUDED_COLORS);
        let candidate_points: Vec<WorldPoint> = (0..BOT_POSITION_ATTEMPT_LIMIT)
            .map(|_| draw_world_point(&mut expected_rng, small_world_size))
            .collect();

        assert_eq!(bot_joiner.appearance.color, expected_color);
        assert!(candidate_points.iter().all(|candidate_point| {
            candidate_point.distance_squared(small_world_player_cursor) < MINIMUM_DISTANCE_SQUARED
        }));
        assert!(is_inside_world(bot_joiner.cursor, small_world_size));
        assert_eq!(Some(&bot_joiner.cursor), candidate_points.last());
    }
}
