use crate::geometry;
use crate::geometry::{SubpixelPoint, Subpixels, WorldPoint};

pub const SURVIVAL_SHRINK_MINIMUM_SUBPIXELS: i32 = 200 * geometry::SUBPIXELS_PER_PIXEL;
pub const SURVIVAL_SHRINK_SUBPIXELS_PER_TICK: i32 = 286;
const CELL_EXTENT_SUBPIXELS: i64 = (geometry::CELL_WIDTH_PIXELS * geometry::SUBPIXELS_PER_PIXEL) as i64;
const CELL_CORNER_SIGNS: [(i64, i64); 4] = [(-1, -1), (1, -1), (1, 1), (-1, 1)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldShapeKind {
    Rectangle,
    Ellipse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldBounds {
    pub left: Subpixels,
    pub top: Subpixels,
    pub width: Subpixels,
    pub height: Subpixels,
}

impl WorldBounds {
    pub fn from_pixel_size(width_pixels: u32, height_pixels: u32) -> WorldBounds {
        WorldBounds {
            left: Subpixels(0),
            top: Subpixels(0),
            width: get_subpixels(width_pixels),
            height: get_subpixels(height_pixels),
        }
    }

    /// Against the ellipse inscribed in these bounds; a point on the ellipse counts as outside.
    pub fn is_outside_ellipse(&self, point: SubpixelPoint) -> bool {
        self.is_outside_ellipse_at(i64::from(point.x), i64::from(point.y))
    }

    fn is_outside_ellipse_at(&self, x: i64, y: i64) -> bool {
        let left: i128 = i128::from(self.left.0);
        let top: i128 = i128::from(self.top.0);
        let width: i128 = i128::from(self.width.0);
        let height: i128 = i128::from(self.height.0);
        let doubled_dx: i128 = 2 * i128::from(x) - (2 * left + width);
        let doubled_dy: i128 = 2 * i128::from(y) - (2 * top + height);

        doubled_dx * doubled_dx * height * height + doubled_dy * doubled_dy * width * width
            >= width * width * height * height
    }

    fn right(&self) -> i64 {
        i64::from(self.left.0) + i64::from(self.width.0)
    }

    fn bottom(&self) -> i64 {
        i64::from(self.top.0) + i64::from(self.height.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct World {
    pub shape: WorldShapeKind,
    pub bounds: WorldBounds,
    pub initial_bounds: WorldBounds,
}

impl World {
    pub fn new(shape: WorldShapeKind, bounds: WorldBounds) -> World {
        World {
            shape,
            bounds,
            initial_bounds: bounds,
        }
    }

    /// Whether the cell's box, centre ±6 px, lies strictly inside the world.
    pub fn contains_cell(&self, center: WorldPoint) -> bool {
        let center_x: i64 = i64::from(center.x) * i64::from(geometry::SUBPIXELS_PER_PIXEL);
        let center_y: i64 = i64::from(center.y) * i64::from(geometry::SUBPIXELS_PER_PIXEL);

        match self.shape {
            WorldShapeKind::Rectangle => self.rectangle_contains_cell(center_x, center_y),
            WorldShapeKind::Ellipse => self.ellipse_contains_cell(center_x, center_y),
        }
    }

    /// One tick of survival shrinking about the centre.
    pub fn shrink(&mut self) {
        let is_above_minimum: bool = self.bounds.width.0 > SURVIVAL_SHRINK_MINIMUM_SUBPIXELS
            && self.bounds.height.0 > SURVIVAL_SHRINK_MINIMUM_SUBPIXELS;
        if !is_above_minimum {
            return;
        }

        let half_step: i32 = SURVIVAL_SHRINK_SUBPIXELS_PER_TICK / 2;

        self.bounds = WorldBounds {
            left: Subpixels(self.bounds.left.0 + half_step),
            top: Subpixels(self.bounds.top.0 + half_step),
            width: Subpixels(self.bounds.width.0 - SURVIVAL_SHRINK_SUBPIXELS_PER_TICK),
            height: Subpixels(self.bounds.height.0 - SURVIVAL_SHRINK_SUBPIXELS_PER_TICK),
        };
    }

    pub fn restore_initial_bounds(&mut self) {
        self.bounds = self.initial_bounds;
    }

    fn rectangle_contains_cell(&self, center_x: i64, center_y: i64) -> bool {
        let is_outside: bool = center_x - CELL_EXTENT_SUBPIXELS <= i64::from(self.bounds.left.0)
            || center_x + CELL_EXTENT_SUBPIXELS >= self.bounds.right()
            || center_y - CELL_EXTENT_SUBPIXELS <= i64::from(self.bounds.top.0)
            || center_y + CELL_EXTENT_SUBPIXELS >= self.bounds.bottom();

        !is_outside
    }

    fn ellipse_contains_cell(&self, center_x: i64, center_y: i64) -> bool {
        CELL_CORNER_SIGNS.iter().all(|(sign_x, sign_y)| {
            let corner_x: i64 = center_x + sign_x * CELL_EXTENT_SUBPIXELS;
            let corner_y: i64 = center_y + sign_y * CELL_EXTENT_SUBPIXELS;

            !self.bounds.is_outside_ellipse_at(corner_x, corner_y)
        })
    }
}

fn get_subpixels(pixels: u32) -> Subpixels {
    let subpixels: i64 = i64::from(pixels) * i64::from(geometry::SUBPIXELS_PER_PIXEL);

    Subpixels(i32::try_from(subpixels).unwrap_or(i32::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_world(shape: WorldShapeKind, left_pixels: i32, width_pixels: u32, height_pixels: u32) -> World {
        let mut bounds: WorldBounds = WorldBounds::from_pixel_size(width_pixels, height_pixels);
        bounds.left = Subpixels(left_pixels * geometry::SUBPIXELS_PER_PIXEL);

        World::new(shape, bounds)
    }

    #[test]
    fn contains_cell_rectangle_excludes_touching_the_border() {
        let world: World = create_world(WorldShapeKind::Rectangle, 0, 800, 800);

        assert!(!world.contains_cell(WorldPoint { x: 6, y: 400 }));
        assert!(world.contains_cell(WorldPoint { x: 7, y: 400 }));
        assert!(!world.contains_cell(WorldPoint { x: 794, y: 400 }));
        assert!(world.contains_cell(WorldPoint { x: 793, y: 400 }));
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 6 }));
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 794 }));
    }

    #[test]
    fn contains_cell_rectangle_follows_an_offset_origin() {
        let world: World = create_world(WorldShapeKind::Rectangle, 1000, 800, 800);

        assert!(!world.contains_cell(WorldPoint { x: 1006, y: 400 }));
        assert!(world.contains_cell(WorldPoint { x: 1007, y: 400 }));
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 400 }));
    }

    #[test]
    fn contains_cell_rectangle_follows_shrunk_subpixel_bounds() {
        let mut world: World = create_world(WorldShapeKind::Rectangle, 0, 800, 800);
        for _ in 0..1000 {
            world.shrink();
        }

        assert_eq!(world.bounds.left, Subpixels(143_000));
        assert!(!world.contains_cell(WorldPoint { x: 145, y: 400 }));
        assert!(world.contains_cell(WorldPoint { x: 146, y: 400 }));
    }

    #[test]
    fn contains_cell_ellipse_tests_every_corner() {
        let world: World = create_world(WorldShapeKind::Ellipse, 0, 800, 800);

        assert!(world.contains_cell(WorldPoint { x: 400, y: 400 }));
        assert!(world.contains_cell(WorldPoint { x: 400, y: 7 }));
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 6 }));
        assert!(!world.contains_cell(WorldPoint { x: 60, y: 60 }));
    }

    #[test]
    fn contains_cell_ellipse_is_centered_on_offset_bounds() {
        let world: World = create_world(WorldShapeKind::Ellipse, 1000, 800, 800);

        assert!(world.contains_cell(WorldPoint { x: 1400, y: 7 }));
        assert!(!world.contains_cell(WorldPoint { x: 1400, y: 6 }));
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 400 }));
    }

    #[test]
    fn contains_cell_ellipse_uses_both_semi_axes() {
        let world: World = create_world(WorldShapeKind::Ellipse, 0, 800, 400);

        assert!(world.contains_cell(WorldPoint { x: 400, y: 7 }));
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 6 }));
        assert!(world.contains_cell(WorldPoint { x: 7, y: 200 }));
        assert!(!world.contains_cell(WorldPoint { x: 6, y: 200 }));
    }

    #[test]
    fn contains_cell_ellipse_follows_shrunk_subpixel_bounds() {
        let mut world: World = create_world(WorldShapeKind::Ellipse, 0, 800, 800);
        let is_inside_before_shrinking: bool = world.contains_cell(WorldPoint { x: 400, y: 145 });
        for _ in 0..1000 {
            world.shrink();
        }

        assert!(is_inside_before_shrinking);
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 145 }));
        assert!(world.contains_cell(WorldPoint { x: 400, y: 146 }));
    }

    #[test]
    fn is_outside_ellipse_counts_the_boundary_as_outside() {
        let bounds: WorldBounds = WorldBounds::from_pixel_size(800, 800);

        assert!(bounds.is_outside_ellipse(WorldPoint { x: 400, y: 0 }.to_subpixel_point()));
        assert!(!bounds.is_outside_ellipse(WorldPoint { x: 400, y: 1 }.to_subpixel_point()));
    }

    #[test]
    fn shrink_moves_each_edge_inward_by_half_a_step() {
        let mut world: World = create_world(WorldShapeKind::Rectangle, 0, 800, 800);
        world.shrink();

        assert_eq!(
            world.bounds,
            WorldBounds {
                left: Subpixels(143),
                top: Subpixels(143),
                width: Subpixels(819_200 - 286),
                height: Subpixels(819_200 - 286),
            },
        );
        assert_eq!(world.initial_bounds, WorldBounds::from_pixel_size(800, 800));
    }

    #[test]
    fn shrink_stops_once_a_dimension_reaches_the_minimum() {
        let mut world: World = World::new(
            WorldShapeKind::Rectangle,
            WorldBounds {
                left: Subpixels(0),
                top: Subpixels(0),
                width: Subpixels(205_000),
                height: Subpixels(400_000),
            },
        );
        world.shrink();
        world.shrink();

        assert_eq!(world.bounds.width, Subpixels(205_000 - 286));
        assert_eq!(world.bounds.height, Subpixels(400_000 - 286));
    }

    #[test]
    fn restore_initial_bounds_undoes_shrinking() {
        let mut world: World = create_world(WorldShapeKind::Ellipse, 0, 800, 800);
        world.shrink();
        world.restore_initial_bounds();

        assert_eq!(world.bounds, world.initial_bounds);
    }
}
