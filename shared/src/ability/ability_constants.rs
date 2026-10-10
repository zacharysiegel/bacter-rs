use crate::game::tick;
use crate::geometry;

/// The original moved projectiles once per 40 ms server packet.
const ORIGINAL_PACKET_PERIOD_MILLISECONDS: i32 = 40;
const ORIGINAL_SPORE_PIXELS_PER_PACKET: i32 = 6;
const ORIGINAL_SHOT_PIXELS_PER_PACKET: i32 = 5;

pub const EXTEND_ACTIVE_TICKS: u32 = tick::get_ticks_from_milliseconds(4500);
pub const EXTEND_COOLDOWN_TICKS: u32 = tick::get_ticks_from_milliseconds(4000);
pub const COMPRESS_EFFECT_TICKS: u32 = tick::get_ticks_from_milliseconds(3500);
pub const COMPRESS_COOLDOWN_TICKS: u32 = tick::get_ticks_from_milliseconds(4000);
pub const IMMORTALITY_ACTIVE_TICKS: u32 = tick::get_ticks_from_milliseconds(3500);
pub const IMMORTALITY_COOLDOWN_TICKS: u32 = tick::get_ticks_from_milliseconds(6000);
pub const FREEZE_EFFECT_TICKS: u32 = tick::get_ticks_from_milliseconds(4000);
pub const FREEZE_COOLDOWN_TICKS: u32 = tick::get_ticks_from_milliseconds(6000);
pub const NEUTRALIZE_ACTIVE_TICKS: u32 = tick::get_ticks_from_milliseconds(3500);
pub const NEUTRALIZE_COOLDOWN_TICKS: u32 = tick::get_ticks_from_milliseconds(6500);
pub const TOXIN_ACTIVE_TICKS: u32 = tick::get_ticks_from_milliseconds(4000);
pub const TOXIN_COOLDOWN_TICKS: u32 = tick::get_ticks_from_milliseconds(6000);
pub const SPORE_FLIGHT_TICKS: u32 = tick::get_ticks_from_milliseconds(1700);
/// Counted from the end of the flight or of the secretion.
pub const SPORE_COOLDOWN_TICKS: u32 = tick::get_ticks_from_milliseconds(7500);
pub const SPORE_SECRETION_TICKS: u32 = tick::get_ticks_from_milliseconds(800);
pub const SHOT_FLIGHT_TICKS: u32 = tick::get_ticks_from_milliseconds(1500);
/// Per slot, counted from the end of the flight or of the secretion.
pub const SHOT_COOLDOWN_TICKS: u32 = tick::get_ticks_from_milliseconds(2000);
pub const SHOT_SECRETION_TICKS: u32 = tick::get_ticks_from_milliseconds(800);

pub const SPORE_SPEED_SUBPIXELS_PER_TICK: i32 =
    get_subpixels_per_tick_from_original_pixels_per_packet(ORIGINAL_SPORE_PIXELS_PER_PACKET);
pub const SHOT_SPEED_SUBPIXELS_PER_TICK: i32 =
    get_subpixels_per_tick_from_original_pixels_per_packet(ORIGINAL_SHOT_PIXELS_PER_PACKET);

/// Floor of `72 * 2.9²` px² in subpixel²; the comparison is `<=`.
pub const SPORE_SECRETION_RADIUS_SQUARED_SUBPIXELS: i64 = 634_933_739;
/// Floor of `151.38` px² in subpixel²; the comparison is `<=`.
pub const SHOT_SECRETION_RADIUS_SQUARED_SUBPIXELS: i64 = 158_733_434;
/// Neutralize and toxin, 60 px; the comparison is `<=`.
pub const FIELD_RADIUS_SQUARED_PIXELS: i64 = 3600;

const fn get_subpixels_per_tick_from_original_pixels_per_packet(pixels_per_packet: i32) -> i32 {
    pixels_per_packet * geometry::SUBPIXELS_PER_PIXEL * tick::TICK_PERIOD_MILLISECONDS as i32
        / ORIGINAL_PACKET_PERIOD_MILLISECONDS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_match_the_designed_tick_counts() {
        assert_eq!(
            [
                EXTEND_ACTIVE_TICKS,
                EXTEND_COOLDOWN_TICKS,
                COMPRESS_EFFECT_TICKS,
                COMPRESS_COOLDOWN_TICKS,
                IMMORTALITY_ACTIVE_TICKS,
                IMMORTALITY_COOLDOWN_TICKS,
                FREEZE_EFFECT_TICKS,
                FREEZE_COOLDOWN_TICKS,
                NEUTRALIZE_ACTIVE_TICKS,
                NEUTRALIZE_COOLDOWN_TICKS,
                TOXIN_ACTIVE_TICKS,
                TOXIN_COOLDOWN_TICKS,
            ],
            [64, 57, 50, 57, 50, 86, 57, 86, 50, 93, 57, 86],
        );
        assert_eq!(
            [
                SPORE_FLIGHT_TICKS,
                SPORE_COOLDOWN_TICKS,
                SPORE_SECRETION_TICKS,
                SHOT_FLIGHT_TICKS,
                SHOT_COOLDOWN_TICKS,
                SHOT_SECRETION_TICKS,
            ],
            [24, 107, 11, 21, 29, 11],
        );
    }

    #[test]
    fn speeds_scale_the_original_packet_speeds_to_ticks() {
        assert_eq!(SPORE_SPEED_SUBPIXELS_PER_TICK, 10_752);
        assert_eq!(SHOT_SPEED_SUBPIXELS_PER_TICK, 8960);
    }
}
