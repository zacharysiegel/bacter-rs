use crate::game::{GameState, Tick};
use crate::geometry::WorldPoint;
use crate::member;
use crate::member::{Appearance, MemberId, SkinKind};
use crate::play::local_game;
use crate::play::{FrameOutput, FramePacer, LocalGame, LocalJoiner, PixelSize};
use crate::random::Pcg32;

pub const TITLE_DUMMY_COUNT: usize = 10;
const QUADRANT_COUNT: usize = 4;
pub const TITLE_QUADRANT_LIMIT: u32 = TITLE_DUMMY_COUNT as u32 / QUADRANT_COUNT as u32 + 1;
const TITLE_PLAYER_CAP: u8 = TITLE_DUMMY_COUNT as u8;
const _: () = assert!(TITLE_QUADRANT_LIMIT as usize * QUADRANT_COUNT >= TITLE_DUMMY_COUNT);
/// In half-world units: top left, top right, bottom right, bottom left.
const QUADRANT_OFFSETS: [(i64, i64); QUADRANT_COUNT] = [(0, 0), (1, 0), (1, 1), (0, 1)];

/// Dummy organisms growing in a local game, never steered.
#[derive(Clone, Debug, PartialEq)]
pub struct TitleScene {
    local_game: LocalGame,
    pacer: FramePacer,
    /// A dummy's cursor never moves; a dummy without an organism is placed again there.
    dummy_cursors: Vec<(MemberId, WorldPoint)>,
}

impl TitleScene {
    pub fn new(window_size: PixelSize, seed: u64) -> TitleScene {
        let mut scene_rng: Pcg32 = local_game::create_scene_rng(seed);
        let dummies: Vec<LocalJoiner> = create_dummies(&mut scene_rng, local_game::get_world_size(window_size));
        let local_game: LocalGame = LocalGame::new(window_size, TITLE_PLAYER_CAP, seed, &dummies);
        let dummy_cursors: Vec<(MemberId, WorldPoint)> = local_game
            .state()
            .members
            .values()
            .filter_map(|member| member.organism.as_ref().map(|organism| (member.member_id, organism.cursor)))
            .collect();

        TitleScene {
            local_game,
            pacer: FramePacer::new(),
            dummy_cursors,
        }
    }

    pub fn state(&self) -> &GameState {
        self.local_game.state()
    }

    pub fn advance_frame(&mut self, now_milliseconds: f64) -> FrameOutput {
        let frame_duration_milliseconds: f64 = self.pacer.begin_frame(now_milliseconds);
        let local_tick_count: u32 = self.pacer.take_local_tick_count(frame_duration_milliseconds);
        let mut tick_applied: Option<Tick> = None;

        for _ in 0..local_tick_count {
            self.local_game.step_local_tick(Vec::new(), Vec::new());
            self.local_game.place_missing_organisms(&self.dummy_cursors);
            tick_applied = Some(self.local_game.state().tick);
        }

        if tick_applied.is_some() {
            self.pacer.record_tick_applied(now_milliseconds);
        }

        FrameOutput {
            interpolation_alpha: self.pacer.get_interpolation_alpha(now_milliseconds),
            tick_applied,
            effects: Vec::new(),
        }
    }

    pub fn resize(&mut self, window_size: PixelSize) {
        let old_size: PixelSize = self.local_game.world_size();

        self.local_game.resize(window_size);

        let new_size: PixelSize = self.local_game.world_size();

        for (_, cursor) in self.dummy_cursors.iter_mut() {
            *cursor = local_game::rescale_point(*cursor, old_size, new_size);
        }
    }
}

fn create_dummies(scene_rng: &mut Pcg32, world_size: PixelSize) -> Vec<LocalJoiner> {
    let mut quadrant_counts: [u32; QUADRANT_COUNT] = [0; QUADRANT_COUNT];

    (0..TITLE_DUMMY_COUNT)
        .map(|index| LocalJoiner {
            screen_name: format!("Dummy {index}"),
            appearance: draw_appearance(scene_rng),
            cursor: draw_cursor(scene_rng, world_size, &mut quadrant_counts),
        })
        .collect()
}

/// Any palette colour and any skin, `None` included.
fn draw_appearance(scene_rng: &mut Pcg32) -> Appearance {
    Appearance {
        color: local_game::draw_color_excluding(scene_rng, &[]),
        skin: local_game::draw_element(scene_rng, &member::SKIN_ORDER).unwrap_or(SkinKind::None),
    }
}

/// Uniform in a quadrant drawn among those still below the quadrant limit.
fn draw_cursor(
    scene_rng: &mut Pcg32,
    world_size: PixelSize,
    quadrant_counts: &mut [u32; QUADRANT_COUNT],
) -> WorldPoint {
    let open_quadrant_indices: Vec<usize> = (0..QUADRANT_OFFSETS.len())
        .filter(|quadrant_index| quadrant_counts[*quadrant_index] < TITLE_QUADRANT_LIMIT)
        .collect();
    let quadrant_index: usize = local_game::draw_element(scene_rng, &open_quadrant_indices).unwrap_or(0);

    quadrant_counts[quadrant_index] += 1;

    let (x_offset, y_offset): (i64, i64) = QUADRANT_OFFSETS[quadrant_index];

    WorldPoint {
        x: draw_quadrant_coordinate(scene_rng, world_size.width_pixels, x_offset),
        y: draw_quadrant_coordinate(scene_rng, world_size.height_pixels, y_offset),
    }
}

/// The quadrant's centre when no pixel in it is clear of its edges.
fn draw_quadrant_coordinate(scene_rng: &mut Pcg32, world_length_pixels: u32, half_offset: i64) -> i32 {
    let half_length: i64 = i64::from(world_length_pixels / 2);
    let start_pixels: i64 = half_length * half_offset;
    let drawn_coordinate: Option<i32> =
        local_game::draw_cell_coordinate(scene_rng, start_pixels, start_pixels + half_length);

    drawn_coordinate.unwrap_or_else(|| get_quadrant_centre_coordinate(start_pixels, half_length))
}

fn get_quadrant_centre_coordinate(start_pixels: i64, half_length: i64) -> i32 {
    i32::try_from(start_pixels + half_length / 2).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Tick;
    use crate::organism::Organism;

    const SEED: u64 = 0x7171e;
    const WINDOW_SIZE: PixelSize = PixelSize {
        width_pixels: 1050,
        height_pixels: 650,
    };

    fn get_dummy_organisms(title_scene: &TitleScene) -> Vec<&Organism> {
        title_scene.state().members.values().filter_map(|member| member.organism.as_ref()).collect()
    }

    fn get_quadrant_index(cursor: WorldPoint, world_size: PixelSize) -> usize {
        let is_right: bool = cursor.x >= i32::try_from(world_size.width_pixels / 2).unwrap();
        let is_bottom: bool = cursor.y >= i32::try_from(world_size.height_pixels / 2).unwrap();

        QUADRANT_OFFSETS
            .iter()
            .position(|offsets| *offsets == (i64::from(is_right), i64::from(is_bottom)))
            .unwrap()
    }

    #[test]
    fn new_places_every_dummy_inside_the_world_with_at_most_the_quadrant_limit_per_quadrant() {
        let title_scene: TitleScene = TitleScene::new(WINDOW_SIZE, SEED);
        let world_size: PixelSize = local_game::get_world_size(WINDOW_SIZE);
        let dummy_organisms: Vec<&Organism> = get_dummy_organisms(&title_scene);
        let mut quadrant_counts: [u32; QUADRANT_COUNT] = [0; QUADRANT_COUNT];

        for organism in &dummy_organisms {
            quadrant_counts[get_quadrant_index(organism.cursor, world_size)] += 1;

            assert!(title_scene.state().world.contains_cell(organism.cursor));
        }

        assert_eq!(dummy_organisms.len(), TITLE_DUMMY_COUNT);
        assert!(quadrant_counts.iter().all(|quadrant_count| *quadrant_count <= TITLE_QUADRANT_LIMIT));
    }

    #[test]
    fn new_draws_the_same_scene_for_the_same_seed() {
        assert_eq!(TitleScene::new(WINDOW_SIZE, SEED), TitleScene::new(WINDOW_SIZE, SEED));
        assert_ne!(
            TitleScene::new(WINDOW_SIZE, SEED).state(),
            TitleScene::new(WINDOW_SIZE, SEED + 1).state(),
        );
    }

    #[test]
    fn advance_frame_steps_one_local_tick_per_tick_period() {
        let mut title_scene: TitleScene = TitleScene::new(WINDOW_SIZE, SEED);

        let first_output: FrameOutput = title_scene.advance_frame(1000.0);
        let second_output: FrameOutput = title_scene.advance_frame(1070.0);
        let third_output: FrameOutput = title_scene.advance_frame(1210.0);

        assert_eq!(first_output.tick_applied, None);
        assert_eq!(second_output.tick_applied, Some(Tick(2)));
        assert_eq!(third_output.tick_applied, Some(Tick(4)));
        assert_eq!(third_output.interpolation_alpha, 0.0);
        assert_eq!(third_output.effects, Vec::new());
    }

    #[test]
    fn advance_frame_places_a_dead_dummy_again_at_its_cursor() {
        let mut title_scene: TitleScene = TitleScene::new(WINDOW_SIZE, SEED);
        let cursor: WorldPoint = title_scene.state().members[&MemberId(3)].organism.as_ref().unwrap().cursor;
        title_scene.local_game.state_mut().members.get_mut(&MemberId(3)).unwrap().organism = None;

        title_scene.advance_frame(0.0);
        title_scene.advance_frame(70.0);

        let organism: &Organism = title_scene.state().members[&MemberId(3)].organism.as_ref().unwrap();

        assert_eq!(organism.cursor, cursor);
        assert_eq!(organism.cells.count(), 1);
    }

    #[test]
    fn resize_rescales_the_dummy_cursors_and_resets_the_dummies_to_one_cell() {
        let mut title_scene: TitleScene = TitleScene::new(WINDOW_SIZE, SEED);
        let cursor: WorldPoint = title_scene.state().members[&MemberId(0)].organism.as_ref().unwrap().cursor;

        title_scene.resize(PixelSize {
            width_pixels: 2050,
            height_pixels: 1250,
        });

        let organism: &Organism = title_scene.state().members[&MemberId(0)].organism.as_ref().unwrap();

        assert_eq!(
            organism.cursor,
            WorldPoint {
                x: cursor.x * 2,
                y: cursor.y * 2,
            },
        );
        assert_eq!(organism.cells.count(), 1);
        assert_eq!(title_scene.dummy_cursors[0], (MemberId(0), organism.cursor));
    }
}
