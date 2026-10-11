use crate::ability::{FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use crate::game;
use crate::game::{
    GameModeKind, GameSettings, GameState, InputBundle, MemberEvent, PlayerTickInput, SimulationEvent, StepError,
};
use crate::geometry;
use crate::geometry::{LatticeCoordinate, WorldPoint};
use crate::member;
use crate::member::{Appearance, MemberId, MemberRoleKind, OrganismColorKind};
use crate::organism::spawn;
use crate::organism::{CellOccupancy, Organism, PlacementError};
use crate::random::Pcg32;
use crate::world::{World, WorldBounds, WorldShapeKind};

/// The world is inset by this on every side of the window.
pub const LOCAL_WORLD_MARGIN_PIXELS: u32 = 25;
/// Separates the scene's own draws (colours, positions) from the game's.
pub const LOCAL_SCENE_RNG_STREAM: u64 = 0x5ce4_e5ce_4e5c_e4e5;
const LOCAL_GAME_TITLE: &str = "Local game";
const LOCAL_LEADERBOARD_LENGTH: u8 = 1;
const LOCAL_WORLD_MINIMUM_PIXELS: u32 = 1;
/// A cell centre this close to a world edge is outside the world.
const CELL_BORDER_PIXELS: i64 = geometry::CELL_WIDTH_PIXELS as i64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelSize {
    pub width_pixels: u32,
    pub height_pixels: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalJoiner {
    pub screen_name: String,
    pub appearance: Appearance,
    pub cursor: WorldPoint,
}

/// A game stepped by the client alone, with nobody to agree with, so its state may be edited between steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalGame {
    state: GameState,
}

impl LocalGame {
    /// The joiners join in tick 1 and are then placed at their cursors.
    pub fn new(window_size: PixelSize, player_cap: u8, seed: u64, local_joiners: &[LocalJoiner]) -> LocalGame {
        let settings: GameSettings = create_local_settings(get_world_size(window_size), player_cap);
        let mut local_game: LocalGame = LocalGame {
            state: GameState::new(settings, seed),
        };
        let joined_events: Vec<MemberEvent> = local_joiners
            .iter()
            .enumerate()
            .map(|(index, local_joiner)| create_joined_event(get_local_member_id(index), local_joiner))
            .collect();
        local_game.step_local_tick(joined_events, Vec::new());

        let cursors: Vec<(MemberId, WorldPoint)> = local_joiners
            .iter()
            .enumerate()
            .map(|(index, local_joiner)| (get_local_member_id(index), local_joiner.cursor))
            .collect();
        local_game.place_missing_organisms(&cursors);

        local_game
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut GameState {
        &mut self.state
    }

    pub fn world_size(&self) -> PixelSize {
        PixelSize {
            width_pixels: self.state.settings.world_width_pixels,
            height_pixels: self.state.settings.world_height_pixels,
        }
    }

    /// Local bundles always fit the state, so a step error is a defect, logged and skipped.
    pub fn step_local_tick(
        &mut self,
        member_events: Vec<MemberEvent>,
        player_inputs: Vec<PlayerTickInput>,
    ) -> Vec<SimulationEvent> {
        let bundle: InputBundle = InputBundle {
            tick: self.state.tick.next(),
            member_events,
            player_inputs,
        };

        let stepped: Result<Vec<SimulationEvent>, StepError> = game::step(&mut self.state, &bundle);

        match stepped {
            Ok(simulation_events) => simulation_events,
            Err(step_error) => {
                log::error!("local tick could not be stepped; [error={step_error:?}]");

                Vec::new()
            }
        }
    }

    /// Only members without an organism are placed.
    pub fn place_missing_organisms(&mut self, cursors: &[(MemberId, WorldPoint)]) {
        for (member_id, cursor) in cursors {
            let is_missing_organism: bool =
                self.state.members.get(member_id).is_some_and(|member| member.organism.is_none());

            if !is_missing_organism {
                continue;
            }

            let placed: Result<(), PlacementError> = spawn::place_organism(&mut self.state, *member_id, *cursor);

            if let Err(placement_error) = placed {
                log::error!("local organism could not be placed; [error={placement_error:?}]");
            }
        }
    }

    /// Every organism keeps its abilities and restarts as one cell at its rescaled cursor.
    pub fn resize(&mut self, window_size: PixelSize) {
        let old_size: PixelSize = self.world_size();
        let new_size: PixelSize = get_world_size(window_size);

        self.state.settings.world_width_pixels = new_size.width_pixels;
        self.state.settings.world_height_pixels = new_size.height_pixels;
        self.state.world = World::new(
            WorldShapeKind::Rectangle,
            WorldBounds::from_pixel_size(new_size.width_pixels, new_size.height_pixels),
        );

        for member in self.state.members.values_mut() {
            let Some(organism) = member.organism.as_mut() else {
                continue;
            };

            reset_to_one_cell(organism, rescale_point(organism.cursor, old_size, new_size));
        }
    }
}

pub fn get_world_size(window_size: PixelSize) -> PixelSize {
    PixelSize {
        width_pixels: get_inset_length(window_size.width_pixels),
        height_pixels: get_inset_length(window_size.height_pixels),
    }
}

/// The local games' loadout; only the appearance differs between members.
pub fn create_local_loadout(appearance: Appearance) -> Loadout {
    Loadout {
        appearance,
        first: FirstAbilityKind::Extend,
        second: SecondAbilityKind::Immortality,
        third: ThirdAbilityKind::Neutralize,
    }
}

pub fn create_joined_event(member_id: MemberId, local_joiner: &LocalJoiner) -> MemberEvent {
    MemberEvent::Joined {
        member_id,
        screen_name: local_joiner.screen_name.clone(),
        role: MemberRoleKind::Participant,
        loadout: Some(create_local_loadout(local_joiner.appearance)),
        team: None,
    }
}

pub fn rescale_point(point: WorldPoint, old_size: PixelSize, new_size: PixelSize) -> WorldPoint {
    WorldPoint {
        x: rescale_coordinate(point.x, old_size.width_pixels, new_size.width_pixels),
        y: rescale_coordinate(point.y, old_size.height_pixels, new_size.height_pixels),
    }
}

pub fn create_scene_rng(seed: u64) -> Pcg32 {
    Pcg32::from_seed_and_stream(seed, LOCAL_SCENE_RNG_STREAM)
}

/// `None` for no elements.
pub fn draw_element<T: Copy>(rng: &mut Pcg32, elements: &[T]) -> Option<T> {
    let element_count: u32 = u32::try_from(elements.len()).ok()?;
    let drawn_index: usize = usize::try_from(rng.below(element_count)).ok()?;

    elements.get(drawn_index).copied()
}

/// Uniform over the palette without `excluded_colors`.
pub fn draw_color_excluding(rng: &mut Pcg32, excluded_colors: &[OrganismColorKind]) -> OrganismColorKind {
    let colors: Vec<OrganismColorKind> =
        member::ORGANISM_COLOR_ORDER.into_iter().filter(|color| !excluded_colors.contains(color)).collect();

    draw_element(rng, &colors).unwrap_or(OrganismColorKind::Fire)
}

/// Uniform over the whole pixels between `start_pixels` and `end_pixels` at which a cell centre is clear of both.
pub fn draw_cell_coordinate(rng: &mut Pcg32, start_pixels: i64, end_pixels: i64) -> i32 {
    let minimum: i64 = start_pixels + CELL_BORDER_PIXELS + 1;
    let end: i64 = end_pixels - CELL_BORDER_PIXELS;
    let span: u32 = u32::try_from((end - minimum).max(0)).unwrap_or(0);

    i32::try_from(minimum + i64::from(rng.below(span))).unwrap_or(0)
}

fn create_local_settings(world_size: PixelSize, player_cap: u8) -> GameSettings {
    GameSettings {
        title: String::from(LOCAL_GAME_TITLE),
        mode: GameModeKind::FreeForAll,
        world_shape: WorldShapeKind::Rectangle,
        world_width_pixels: world_size.width_pixels,
        world_height_pixels: world_size.height_pixels,
        player_minimum: None,
        player_cap,
        team_count: None,
        leaderboard_length: LOCAL_LEADERBOARD_LENGTH,
    }
}

fn get_local_member_id(index: usize) -> MemberId {
    MemberId(u32::try_from(index).unwrap_or(u32::MAX))
}

fn get_inset_length(window_length_pixels: u32) -> u32 {
    window_length_pixels.saturating_sub(2 * LOCAL_WORLD_MARGIN_PIXELS).max(LOCAL_WORLD_MINIMUM_PIXELS)
}

fn reset_to_one_cell(organism: &mut Organism, cursor: WorldPoint) {
    organism.anchor = cursor;
    organism.cursor = cursor;
    organism.cells = CellOccupancy::with_cell(LatticeCoordinate { i: 0, j: 0 });
}

fn rescale_coordinate(coordinate: i32, old_length_pixels: u32, new_length_pixels: u32) -> i32 {
    let rescaled: i64 = i64::from(coordinate) * i64::from(new_length_pixels) / i64::from(old_length_pixels.max(1));

    i32::try_from(rescaled).unwrap_or(coordinate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::AbilityPressSet;
    use crate::game::{PlayerTickInput, Tick};
    use crate::geometry;
    use crate::member;
    use crate::member::{OrganismColorKind, SkinKind};

    const SEED: u64 = 0x10ca1;
    const WINDOW_SIZE: PixelSize = PixelSize {
        width_pixels: 850,
        height_pixels: 650,
    };

    fn create_local_joiner(cursor: WorldPoint) -> LocalJoiner {
        LocalJoiner {
            screen_name: String::from("Local"),
            appearance: Appearance {
                color: OrganismColorKind::Lake,
                skin: SkinKind::None,
            },
            cursor,
        }
    }

    fn create_local_game(cursors: &[WorldPoint]) -> LocalGame {
        let local_joiners: Vec<LocalJoiner> = cursors.iter().map(|cursor| create_local_joiner(*cursor)).collect();

        LocalGame::new(WINDOW_SIZE, 10, SEED, &local_joiners)
    }

    fn get_organism(local_game: &LocalGame, member_id: MemberId) -> &Organism {
        local_game.state().members[&member_id].organism.as_ref().unwrap()
    }

    #[test]
    fn get_world_size_takes_the_margins_off_the_window() {
        assert_eq!(
            get_world_size(WINDOW_SIZE),
            PixelSize {
                width_pixels: 800,
                height_pixels: 600,
            },
        );
        assert_eq!(
            get_world_size(PixelSize {
                width_pixels: 30,
                height_pixels: 0,
            }),
            PixelSize {
                width_pixels: 1,
                height_pixels: 1,
            },
        );
    }

    #[test]
    fn new_joins_every_local_joiner_in_the_first_tick_and_places_it_at_its_cursor() {
        let local_game: LocalGame = create_local_game(&[WorldPoint { x: 100, y: 120 }, WorldPoint { x: 500, y: 400 }]);
        let state: &GameState = local_game.state();

        assert_eq!(state.tick, Tick(1));
        assert_eq!(state.settings.mode, GameModeKind::FreeForAll);
        assert_eq!(state.settings.world_shape, WorldShapeKind::Rectangle);
        assert_eq!(
            (state.settings.world_width_pixels, state.settings.world_height_pixels),
            (800, 600),
        );
        assert_eq!(state.settings.player_cap, 10);
        assert_eq!(state.next_member_id, MemberId(2));
        assert_eq!(
            get_organism(&local_game, MemberId(1)).cursor,
            WorldPoint { x: 500, y: 400 }
        );
        assert_eq!(get_organism(&local_game, MemberId(1)).cells.count(), 1);
        assert_eq!(
            state.members[&MemberId(0)].loadout,
            Some(create_local_loadout(
                create_local_joiner(WorldPoint { x: 0, y: 0 }).appearance
            )),
        );
    }

    #[test]
    fn step_local_tick_steps_the_next_tick_with_the_inputs() {
        let mut local_game: LocalGame = create_local_game(&[WorldPoint { x: 100, y: 120 }]);

        local_game.step_local_tick(
            Vec::new(),
            vec![PlayerTickInput {
                member_id: MemberId(0),
                cursor: WorldPoint { x: 103, y: 120 },
                ability_presses: AbilityPressSet::NONE,
                aim: None,
            }],
        );

        assert_eq!(local_game.state().tick, Tick(2));
        assert_eq!(
            get_organism(&local_game, MemberId(0)).cursor,
            WorldPoint { x: 103, y: 120 }
        );
    }

    #[test]
    fn place_missing_organisms_places_only_members_without_an_organism() {
        let mut local_game: LocalGame =
            create_local_game(&[WorldPoint { x: 100, y: 120 }, WorldPoint { x: 500, y: 400 }]);
        local_game.state_mut().members.get_mut(&MemberId(1)).unwrap().organism = None;

        local_game.place_missing_organisms(&[
            (MemberId(0), WorldPoint { x: 1, y: 1 }),
            (MemberId(1), WorldPoint { x: 300, y: 310 }),
        ]);

        assert_eq!(
            get_organism(&local_game, MemberId(0)).cursor,
            WorldPoint { x: 100, y: 120 }
        );
        assert_eq!(
            get_organism(&local_game, MemberId(1)).cursor,
            WorldPoint { x: 300, y: 310 }
        );
    }

    #[test]
    fn resize_rescales_the_cursors_and_resets_the_organisms_to_one_cell() {
        let mut local_game: LocalGame = create_local_game(&[WorldPoint { x: 400, y: 300 }]);

        for _ in 0..20 {
            local_game.step_local_tick(Vec::new(), Vec::new());
        }

        assert!(get_organism(&local_game, MemberId(0)).cells.count() > 1);

        local_game.resize(PixelSize {
            width_pixels: 450,
            height_pixels: 350,
        });

        let organism: &Organism = get_organism(&local_game, MemberId(0));

        assert_eq!(organism.cursor, WorldPoint { x: 200, y: 150 });
        assert_eq!(organism.anchor, WorldPoint { x: 200, y: 150 });
        assert_eq!(organism.cells.count(), 1);
        assert_eq!(
            local_game.world_size(),
            PixelSize {
                width_pixels: 400,
                height_pixels: 300,
            },
        );
        assert_eq!(
            local_game.state().world.bounds.width.0,
            400 * geometry::SUBPIXELS_PER_PIXEL,
        );
    }

    #[test]
    fn rescale_point_keeps_the_proportion_of_the_world() {
        let old_size: PixelSize = PixelSize {
            width_pixels: 800,
            height_pixels: 600,
        };
        let new_size: PixelSize = PixelSize {
            width_pixels: 1200,
            height_pixels: 300,
        };

        assert_eq!(
            rescale_point(WorldPoint { x: 200, y: 450 }, old_size, new_size),
            WorldPoint { x: 300, y: 225 },
        );
    }

    #[test]
    fn draw_element_draws_from_the_elements_and_none_from_no_elements() {
        let mut rng: Pcg32 = create_scene_rng(SEED);

        let color: Option<OrganismColorKind> = draw_element(&mut rng, &member::ORGANISM_COLOR_ORDER);

        assert!(color.is_some_and(|color| member::ORGANISM_COLOR_ORDER.contains(&color)));
        assert_eq!(draw_element::<SkinKind>(&mut rng, &[]), None);
    }

    #[test]
    fn draw_color_excluding_never_draws_an_excluded_color() {
        let mut rng: Pcg32 = create_scene_rng(SEED);
        let excluded_colors: [OrganismColorKind; 2] = [OrganismColorKind::Sun, OrganismColorKind::Sky];

        for _ in 0..200 {
            assert!(!excluded_colors.contains(&draw_color_excluding(&mut rng, &excluded_colors)));
        }
    }

    #[test]
    fn draw_cell_coordinate_keeps_a_cell_clear_of_both_edges() {
        let mut rng: Pcg32 = create_scene_rng(SEED);
        let coordinates: Vec<i32> = (0..200).map(|_| draw_cell_coordinate(&mut rng, 100, 120)).collect();

        assert!(coordinates.iter().all(|coordinate| (107..114).contains(coordinate)));
        assert!(coordinates.contains(&107));
        assert!(coordinates.contains(&113));
        assert_eq!(draw_cell_coordinate(&mut rng, 0, 5), 7);
    }
}
