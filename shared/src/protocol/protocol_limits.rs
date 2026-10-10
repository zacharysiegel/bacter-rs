use crate::geometry;

pub const INBOUND_FRAME_BYTE_LIMIT: usize = 4096;
pub const SCREEN_NAME_CHARACTER_LIMIT: usize = 64;
pub const TITLE_CHARACTER_LIMIT: usize = 64;
/// bcrypt reads at most 72 bytes.
pub const PASSWORD_BYTE_LIMIT: usize = 72;
pub const WORLD_SIZE_LOWEST_PIXELS: u32 = 300;
pub const WORLD_SIZE_HIGHEST_PIXELS: u32 = 100_000;
pub const PLAYER_MINIMUM_LOWEST: u8 = 2;
pub const PLAYER_CAP_LOWEST: u8 = 2;
pub const PLAYER_CAP_HIGHEST: u8 = 32;
pub const TEAM_COUNT_LOWEST: u8 = 2;
pub const TEAM_COUNT_HIGHEST: u8 = 4;
pub const LEADERBOARD_LENGTH_LOWEST: u8 = 1;
pub const LEADERBOARD_LENGTH_HIGHEST: u8 = 20;
/// Pure Spectators.
pub const SPECTATOR_LIMIT_PER_GAME: u32 = 64;
pub const MEMBER_LIMIT_HIGHEST: u32 = get_member_limit(PLAYER_CAP_HIGHEST);
pub const GAME_LIMIT_PER_SERVER: u32 = 256;
pub const CONNECTION_LIMIT_PER_SERVER: u32 = 4096;
pub const CONNECTION_LIMIT_PER_IP: u32 = 16;
pub const PASSWORD_FAILURE_LIMIT_PER_IP: u32 = 20;
pub const PASSWORD_FAILURE_WINDOW_SECONDS: u64 = 600;
/// Per axis, either sign.
pub const COORDINATE_LIMIT_SUBPIXELS: i32 = 1 << 28;
pub const COORDINATE_LIMIT_PIXELS: i32 = COORDINATE_LIMIT_SUBPIXELS / geometry::SUBPIXELS_PER_PIXEL;
pub const LATTICE_COORDINATE_LIMIT: i32 = COORDINATE_LIMIT_PIXELS / geometry::CELL_WIDTH_PIXELS;

/// Participants of any status plus Spectators.
pub const fn get_member_limit(player_cap: u8) -> u32 {
    player_cap as u32 + SPECTATOR_LIMIT_PER_GAME
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_member_limit_adds_the_spectator_limit_to_the_player_cap() {
        assert_eq!(get_member_limit(16), 80);
        assert_eq!(MEMBER_LIMIT_HIGHEST, 96);
    }

    #[test]
    fn coordinate_limits_derive_from_the_subpixel_limit() {
        assert_eq!(COORDINATE_LIMIT_PIXELS, 262_144);
        assert_eq!(LATTICE_COORDINATE_LIMIT, 43_690);
    }
}
