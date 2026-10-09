use std::cmp::Ordering;

pub const CELL_WIDTH_PIXELS: i32 = 6;
pub const SUBPIXELS_PER_PIXEL: i32 = 1024;
pub const NEIGHBOR_DIRECTIONS: [NeighborDirection; 4] = [
    NeighborDirection::Left,
    NeighborDirection::Up,
    NeighborDirection::Right,
    NeighborDirection::Down,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Subpixels(pub i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldPoint {
    pub x: i32,
    pub y: i32,
}

impl WorldPoint {
    pub fn distance_squared(self, other: WorldPoint) -> i64 {
        let dx: i64 = i64::from(self.x) - i64::from(other.x);
        let dy: i64 = i64::from(self.y) - i64::from(other.y);

        dx * dx + dy * dy
    }

    /// Whether two cell centres are close enough to collide, touching edges included.
    pub fn is_within_cell_collision(self, other: WorldPoint) -> bool {
        let dx: i64 = i64::from(self.x) - i64::from(other.x);
        let dy: i64 = i64::from(self.y) - i64::from(other.y);
        let collision_extent: i64 = i64::from(CELL_WIDTH_PIXELS);

        dx.abs() <= collision_extent && dy.abs() <= collision_extent
    }

    pub fn to_subpixel_point(self) -> SubpixelPoint {
        SubpixelPoint {
            x: self.x * SUBPIXELS_PER_PIXEL,
            y: self.y * SUBPIXELS_PER_PIXEL,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubpixelPoint {
    pub x: i32,
    pub y: i32,
}

impl SubpixelPoint {
    pub fn distance_squared(self, other: SubpixelPoint) -> i64 {
        let dx: i64 = i64::from(self.x) - i64::from(other.x);
        let dy: i64 = i64::from(self.y) - i64::from(other.y);

        dx * dx + dy * dy
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubpixelVector {
    pub x: i32,
    pub y: i32,
}

/// Ordered row-major: by `j`, then by `i`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LatticeCoordinate {
    pub i: i32,
    pub j: i32,
}

impl LatticeCoordinate {
    pub fn neighbor(self, direction: NeighborDirection) -> LatticeCoordinate {
        let (offset_i, offset_j): (i32, i32) = direction.offset();

        LatticeCoordinate {
            i: self.i + offset_i,
            j: self.j + offset_j,
        }
    }

    pub fn to_cell_center(self, anchor: WorldPoint) -> WorldPoint {
        WorldPoint {
            x: anchor.x + CELL_WIDTH_PIXELS * self.i,
            y: anchor.y + CELL_WIDTH_PIXELS * self.j,
        }
    }
}

impl Ord for LatticeCoordinate {
    fn cmp(&self, other: &LatticeCoordinate) -> Ordering {
        self.j.cmp(&other.j).then(self.i.cmp(&other.i))
    }
}

impl PartialOrd for LatticeCoordinate {
    fn partial_cmp(&self, other: &LatticeCoordinate) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeighborDirection {
    Left,
    Up,
    Right,
    Down,
}

impl NeighborDirection {
    pub fn offset(self) -> (i32, i32) {
        match self {
            NeighborDirection::Left => (-1, 0),
            NeighborDirection::Up => (0, -1),
            NeighborDirection::Right => (1, 0),
            NeighborDirection::Down => (0, 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lattice_coordinate_cmp_orders_rows_before_columns() {
        let mut lattice_coordinates: Vec<LatticeCoordinate> = vec![
            LatticeCoordinate { i: 0, j: 1 },
            LatticeCoordinate { i: 5, j: 0 },
            LatticeCoordinate { i: -3, j: 1 },
            LatticeCoordinate { i: 2, j: -1 },
        ];
        lattice_coordinates.sort();

        assert_eq!(
            lattice_coordinates,
            vec![
                LatticeCoordinate { i: 2, j: -1 },
                LatticeCoordinate { i: 5, j: 0 },
                LatticeCoordinate { i: -3, j: 1 },
                LatticeCoordinate { i: 0, j: 1 },
            ],
        );
    }

    #[test]
    fn neighbor_follows_left_up_right_down() {
        let origin: LatticeCoordinate = LatticeCoordinate { i: 0, j: 0 };
        let neighbors: Vec<LatticeCoordinate> =
            NEIGHBOR_DIRECTIONS.iter().map(|direction| origin.neighbor(*direction)).collect();

        assert_eq!(
            neighbors,
            vec![
                LatticeCoordinate { i: -1, j: 0 },
                LatticeCoordinate { i: 0, j: -1 },
                LatticeCoordinate { i: 1, j: 0 },
                LatticeCoordinate { i: 0, j: 1 },
            ],
        );
    }

    #[test]
    fn to_cell_center_scales_by_cell_width_from_anchor() {
        let anchor: WorldPoint = WorldPoint { x: 100, y: 200 };

        assert_eq!(
            LatticeCoordinate { i: 2, j: -3 }.to_cell_center(anchor),
            WorldPoint { x: 112, y: 182 },
        );
    }

    #[test]
    fn distance_squared_is_exact_at_coordinate_limits() {
        let first: SubpixelPoint = SubpixelPoint {
            x: -(1 << 28),
            y: -(1 << 28),
        };
        let second: SubpixelPoint = SubpixelPoint { x: 1 << 28, y: 1 << 28 };

        assert_eq!(first.distance_squared(second), 1_i64 << 59);
        assert_eq!(
            WorldPoint { x: 3, y: -4 }.distance_squared(WorldPoint { x: 0, y: 0 }),
            25,
        );
    }

    #[test]
    fn is_within_cell_collision_includes_touching_edges() {
        let center: WorldPoint = WorldPoint { x: 100, y: 100 };

        assert!(center.is_within_cell_collision(WorldPoint { x: 106, y: 94 }));
        assert!(!center.is_within_cell_collision(WorldPoint { x: 107, y: 100 }));
        assert!(!center.is_within_cell_collision(WorldPoint { x: 100, y: 93 }));
    }

    #[test]
    fn to_subpixel_point_multiplies_by_subpixels_per_pixel() {
        assert_eq!(
            WorldPoint { x: 3, y: -2 }.to_subpixel_point(),
            SubpixelPoint { x: 3072, y: -2048 },
        );
    }
}
