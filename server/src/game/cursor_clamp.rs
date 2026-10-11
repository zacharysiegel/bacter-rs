use shared::geometry::SubpixelPoint;

/// 42.5 px/s is 2.975 px, or 3046.4 subpixels, per tick.
pub const MOVE_SUBPIXELS_PER_TICK: i64 = 3046;
pub const MOVE_TOLERANCE_SUBPIXELS_PER_TICK: i64 = 128;
pub const MOVE_BUDGET_CAP_TICKS: i64 = 15;
const MOVE_BUDGET_SUBPIXELS_PER_TICK: i64 = MOVE_SUBPIXELS_PER_TICK + MOVE_TOLERANCE_SUBPIXELS_PER_TICK;
const MOVE_BUDGET_CAP_SUBPIXELS: i64 = MOVE_BUDGET_CAP_TICKS * MOVE_BUDGET_SUBPIXELS_PER_TICK;

/// Holds one organism's cursor to the movement speed, with a catch-up allowance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorClamp {
    applied_cursor: SubpixelPoint,
    budget_subpixels: i64,
}

impl CursorClamp {
    /// With an empty budget.
    pub fn new(spawn_cursor: SubpixelPoint) -> CursorClamp {
        CursorClamp {
            applied_cursor: spawn_cursor,
            budget_subpixels: 0,
        }
    }

    pub fn applied_cursor(&self) -> SubpixelPoint {
        self.applied_cursor
    }

    pub fn add_tick_budget(&mut self) {
        self.budget_subpixels = (self.budget_subpixels + MOVE_BUDGET_SUBPIXELS_PER_TICK).min(MOVE_BUDGET_CAP_SUBPIXELS);
    }

    /// Beyond the budget the cursor moves towards `target` by the whole budget.
    pub fn apply(&mut self, target: SubpixelPoint) -> CursorClampKind {
        let dx: i64 = i64::from(target.x) - i64::from(self.applied_cursor.x);
        let dy: i64 = i64::from(target.y) - i64::from(self.applied_cursor.y);
        let distance_squared: i64 = dx * dx + dy * dy;

        if distance_squared <= self.budget_subpixels * self.budget_subpixels {
            let distance: i64 = (distance_squared as f64).sqrt().ceil() as i64;
            self.applied_cursor = target;
            self.budget_subpixels = (self.budget_subpixels - distance).max(0);

            return CursorClampKind::WithinBudget;
        }

        let scale: f64 = self.budget_subpixels as f64 / (distance_squared as f64).sqrt();
        self.applied_cursor = SubpixelPoint {
            x: get_moved_coordinate(self.applied_cursor.x, dx, scale),
            y: get_moved_coordinate(self.applied_cursor.y, dy, scale),
        };
        self.budget_subpixels = 0;

        CursorClampKind::Clamped
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorClampKind {
    WithinBudget,
    Clamped,
}

/// Lies between the coordinate and its target, so within the coordinate limit.
fn get_moved_coordinate(coordinate: i32, difference: i64, scale: f64) -> i32 {
    let moved_coordinate: i64 = i64::from(coordinate) + (difference as f64 * scale).round() as i64;

    i32::try_from(moved_coordinate).unwrap_or(coordinate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::protocol::protocol_limits;

    fn create_clamp_with_budget_ticks(budget_tick_count: u32) -> CursorClamp {
        let mut clamp: CursorClamp = CursorClamp::new(SubpixelPoint { x: 0, y: 0 });

        for _ in 0..budget_tick_count {
            clamp.add_tick_budget();
        }

        clamp
    }

    #[test]
    fn apply_moves_to_a_target_within_the_budget_and_spends_the_distance() {
        let mut clamp: CursorClamp = create_clamp_with_budget_ticks(1);

        assert_eq!(
            clamp.apply(SubpixelPoint { x: 3000, y: 0 }),
            CursorClampKind::WithinBudget
        );
        assert_eq!(clamp.applied_cursor(), SubpixelPoint { x: 3000, y: 0 });
        assert_eq!(clamp.apply(SubpixelPoint { x: 3200, y: 0 }), CursorClampKind::Clamped);
        assert_eq!(clamp.applied_cursor(), SubpixelPoint { x: 3174, y: 0 });
    }

    #[test]
    fn apply_moves_along_the_direction_by_the_budget_beyond_it() {
        let mut clamp: CursorClamp = create_clamp_with_budget_ticks(1);

        assert_eq!(
            clamp.apply(SubpixelPoint { x: 30_000, y: -40_000 }),
            CursorClampKind::Clamped
        );
        assert_eq!(clamp.applied_cursor(), SubpixelPoint { x: 1904, y: -2539 });
        assert_eq!(
            clamp.apply(SubpixelPoint { x: 1905, y: -2539 }),
            CursorClampKind::Clamped
        );
        assert_eq!(clamp.applied_cursor(), SubpixelPoint { x: 1904, y: -2539 });
    }

    #[test]
    fn add_tick_budget_stops_at_the_cap() {
        let mut clamp: CursorClamp = create_clamp_with_budget_ticks(20);

        assert_eq!(
            clamp.apply(SubpixelPoint { x: 100_000, y: 0 }),
            CursorClampKind::Clamped
        );
        assert_eq!(clamp.applied_cursor(), SubpixelPoint { x: 47_610, y: 0 });
    }

    #[test]
    fn new_starts_at_the_spawn_cursor_with_an_empty_budget() {
        let spawn_cursor: SubpixelPoint = SubpixelPoint { x: 51_200, y: 102_400 };
        let mut clamp: CursorClamp = CursorClamp::new(spawn_cursor);

        assert_eq!(clamp.apply(spawn_cursor), CursorClampKind::WithinBudget);
        assert_eq!(
            clamp.apply(SubpixelPoint { x: 51_201, y: 102_400 }),
            CursorClampKind::Clamped
        );
        assert_eq!(clamp.applied_cursor(), spawn_cursor);
    }

    #[test]
    fn apply_handles_opposite_corners_of_the_coordinate_limit() {
        let limit: i32 = protocol_limits::COORDINATE_LIMIT_SUBPIXELS;
        let mut clamp: CursorClamp = CursorClamp::new(SubpixelPoint { x: -limit, y: -limit });

        for _ in 0..15 {
            clamp.add_tick_budget();
        }

        assert_eq!(
            clamp.apply(SubpixelPoint { x: limit, y: limit }),
            CursorClampKind::Clamped
        );
        assert_eq!(
            clamp.applied_cursor(),
            SubpixelPoint {
                x: -limit + 33_665,
                y: -limit + 33_665,
            },
        );
    }
}
