use std::collections::VecDeque;
use std::f64::consts::FRAC_1_SQRT_2;

use crate::game::Tick;
use crate::geometry;
use crate::geometry::{SubpixelPoint, SubpixelVector};
use crate::play::{KeyboardState, MovementDirection};
use crate::protocol::protocol_limits;

pub const CROSSHAIR_SPEED_PIXELS_PER_SECOND: f64 = 42.5;
pub const SPECTATOR_CAMERA_SPEED_PIXELS_PER_SECOND: f64 = 62.5;
pub const SENT_CURSOR_HISTORY_LENGTH: usize = 64;
const MILLISECONDS_PER_SECOND: f64 = 1000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SentCursor {
    pub client_tick: Tick,
    pub cursor: SubpixelPoint,
}

/// The newest `SENT_CURSOR_HISTORY_LENGTH` sent cursors, oldest first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SentCursorHistory {
    sent_cursors: VecDeque<SentCursor>,
}

impl SentCursorHistory {
    pub fn new() -> SentCursorHistory {
        SentCursorHistory {
            sent_cursors: VecDeque::with_capacity(SENT_CURSOR_HISTORY_LENGTH),
        }
    }

    pub fn record(&mut self, client_tick: Tick, cursor: SubpixelPoint) {
        if self.sent_cursors.len() == SENT_CURSOR_HISTORY_LENGTH {
            self.sent_cursors.pop_front();
        }

        self.sent_cursors.push_back(SentCursor { client_tick, cursor });
    }

    pub fn get(&self, client_tick: Tick) -> Option<SubpixelPoint> {
        self.sent_cursors
            .iter()
            .find(|sent_cursor| sent_cursor.client_tick == client_tick)
            .map(|sent_cursor| sent_cursor.cursor)
    }

    /// Moves the cursor sent for `client_tick` and every later one.
    pub fn shift_from(&mut self, client_tick: Tick, shift: SubpixelVector) {
        for sent_cursor in self.sent_cursors.iter_mut() {
            if sent_cursor.client_tick >= client_tick {
                sent_cursor.cursor = get_shifted_point(sent_cursor.cursor, shift);
            }
        }
    }

    pub fn clear(&mut self) {
        self.sent_cursors.clear();
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CrosshairController {
    /// Subpixels, unrounded, so frames shorter than a subpixel of movement still move it.
    position_x: f64,
    position_y: f64,
    sent_cursors: SentCursorHistory,
}

impl CrosshairController {
    pub fn new(position: SubpixelPoint) -> CrosshairController {
        CrosshairController {
            position_x: f64::from(position.x),
            position_y: f64::from(position.y),
            sent_cursors: SentCursorHistory::new(),
        }
    }

    pub fn position(&self) -> SubpixelPoint {
        SubpixelPoint {
            x: get_rounded_coordinate(self.position_x),
            y: get_rounded_coordinate(self.position_y),
        }
    }

    pub fn advance(
        &mut self,
        keyboard: &KeyboardState,
        frame_duration_milliseconds: f64,
        speed_pixels_per_second: f64,
    ) {
        let direction: MovementDirection = keyboard.movement_direction();
        let is_diagonal: bool = direction.x != 0 && direction.y != 0;
        let axis_scale: f64 = if is_diagonal { FRAC_1_SQRT_2 } else { 1.0 };
        let distance_subpixels: f64 =
            speed_pixels_per_second * f64::from(geometry::SUBPIXELS_PER_PIXEL) * frame_duration_milliseconds
                / MILLISECONDS_PER_SECOND
                * axis_scale;

        self.position_x = get_limited_coordinate(self.position_x + f64::from(direction.x) * distance_subpixels);
        self.position_y = get_limited_coordinate(self.position_y + f64::from(direction.y) * distance_subpixels);
    }

    /// Forgets the sent cursors, which the snap makes stale.
    pub fn snap_to(&mut self, position: SubpixelPoint) {
        self.position_x = f64::from(position.x);
        self.position_y = f64::from(position.y);
        self.sent_cursors.clear();
    }

    pub fn record_sent(&mut self, client_tick: Tick, cursor: SubpixelPoint) {
        self.sent_cursors.record(client_tick, cursor);
    }

    /// The server clamped the cursor sent for `client_tick` to `cursor`.
    pub fn apply_correction(&mut self, client_tick: Tick, cursor: SubpixelPoint) {
        let Some(sent_cursor) = self.sent_cursors.get(client_tick) else {
            self.snap_to(cursor);

            return;
        };

        let shift: SubpixelVector = SubpixelVector {
            x: cursor.x - sent_cursor.x,
            y: cursor.y - sent_cursor.y,
        };

        self.position_x = get_limited_coordinate(self.position_x + f64::from(shift.x));
        self.position_y = get_limited_coordinate(self.position_y + f64::from(shift.y));
        self.sent_cursors.shift_from(client_tick, shift);
    }
}

fn get_limited_coordinate(coordinate: f64) -> f64 {
    let limit: f64 = f64::from(protocol_limits::COORDINATE_LIMIT_SUBPIXELS);

    coordinate.clamp(-limit, limit)
}

/// Within the coordinate limit, so the conversion cannot saturate.
fn get_rounded_coordinate(coordinate: f64) -> i32 {
    get_limited_coordinate(coordinate).round() as i32
}

fn get_shifted_point(point: SubpixelPoint, shift: SubpixelVector) -> SubpixelPoint {
    SubpixelPoint {
        x: get_rounded_coordinate(f64::from(point.x) + f64::from(shift.x)),
        y: get_rounded_coordinate(f64::from(point.y) + f64::from(shift.y)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::{GameKeyKind, KeyRepeatKind, KeyTransitionKind};

    const ORIGIN: SubpixelPoint = SubpixelPoint { x: 0, y: 0 };

    fn hold_keys(keys: &[GameKeyKind]) -> KeyboardState {
        let mut keyboard: KeyboardState = KeyboardState::new();

        for key in keys {
            keyboard.key_changed(*key, KeyTransitionKind::Pressed, KeyRepeatKind::Initial);
        }

        keyboard
    }

    fn create_point(x: i32) -> SubpixelPoint {
        SubpixelPoint { x, y: 0 }
    }

    #[test]
    fn advance_moves_at_the_speed_along_one_axis() {
        let mut crosshair: CrosshairController = CrosshairController::new(ORIGIN);

        crosshair.advance(
            &hold_keys(&[GameKeyKind::MoveRight]),
            1000.0,
            CROSSHAIR_SPEED_PIXELS_PER_SECOND,
        );

        assert_eq!(crosshair.position(), SubpixelPoint { x: 43_520, y: 0 });
    }

    #[test]
    fn advance_scales_a_diagonal_by_cos_45() {
        let mut crosshair: CrosshairController = CrosshairController::new(ORIGIN);

        crosshair.advance(
            &hold_keys(&[GameKeyKind::MoveLeft, GameKeyKind::MoveDown]),
            1000.0,
            CROSSHAIR_SPEED_PIXELS_PER_SECOND,
        );

        assert_eq!(crosshair.position(), SubpixelPoint { x: -30_773, y: 30_773 });
    }

    #[test]
    fn advance_accumulates_movement_finer_than_a_subpixel() {
        let mut crosshair: CrosshairController = CrosshairController::new(ORIGIN);
        let keyboard: KeyboardState = hold_keys(&[GameKeyKind::MoveUp]);

        for _ in 0..4 {
            crosshair.advance(&keyboard, 250.0, SPECTATOR_CAMERA_SPEED_PIXELS_PER_SECOND);
        }

        assert_eq!(crosshair.position(), SubpixelPoint { x: 0, y: -64_000 });
    }

    #[test]
    fn advance_does_not_move_with_opposite_keys() {
        let mut crosshair: CrosshairController = CrosshairController::new(ORIGIN);

        crosshair.advance(
            &hold_keys(&[GameKeyKind::MoveLeft, GameKeyKind::MoveRight]),
            1000.0,
            CROSSHAIR_SPEED_PIXELS_PER_SECOND,
        );

        assert_eq!(crosshair.position(), ORIGIN);
    }

    #[test]
    fn advance_stops_at_the_coordinate_limit() {
        let mut crosshair: CrosshairController =
            CrosshairController::new(create_point(protocol_limits::COORDINATE_LIMIT_SUBPIXELS - 10));

        crosshair.advance(
            &hold_keys(&[GameKeyKind::MoveRight]),
            1000.0,
            CROSSHAIR_SPEED_PIXELS_PER_SECOND,
        );

        assert_eq!(
            crosshair.position(),
            create_point(protocol_limits::COORDINATE_LIMIT_SUBPIXELS),
        );
    }

    #[test]
    fn apply_correction_shifts_the_crosshair_once_for_a_clamp_burst() {
        let mut crosshair: CrosshairController = CrosshairController::new(create_point(17_000));
        crosshair.record_sent(Tick(10), create_point(10_000));
        crosshair.record_sent(Tick(11), create_point(13_000));
        crosshair.record_sent(Tick(12), create_point(16_000));

        crosshair.apply_correction(Tick(10), create_point(8000));

        assert_eq!(crosshair.position(), create_point(15_000));

        crosshair.apply_correction(Tick(11), create_point(11_000));
        crosshair.apply_correction(Tick(12), create_point(14_000));

        assert_eq!(crosshair.position(), create_point(15_000));
    }

    #[test]
    fn apply_correction_snaps_for_a_tick_no_longer_in_the_history() {
        let mut crosshair: CrosshairController = CrosshairController::new(ORIGIN);

        for tick in 0..=SENT_CURSOR_HISTORY_LENGTH {
            crosshair.record_sent(Tick(u32::try_from(tick).unwrap()), ORIGIN);
        }

        crosshair.apply_correction(Tick(0), create_point(-500));

        assert_eq!(crosshair.position(), create_point(-500));
    }

    #[test]
    fn snap_to_moves_the_crosshair_and_forgets_the_sent_cursors() {
        let mut crosshair: CrosshairController = CrosshairController::new(ORIGIN);
        crosshair.record_sent(Tick(3), create_point(100));

        crosshair.snap_to(create_point(9000));
        crosshair.apply_correction(Tick(3), create_point(50));

        assert_eq!(crosshair.position(), create_point(50));
    }
}
