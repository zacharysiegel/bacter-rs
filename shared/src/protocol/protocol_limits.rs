use crate::geometry;
use crate::protocol::RejectionKind;

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

const SCREEN_NAME_REJECTION_KINDS: TextRejectionKinds = TextRejectionKinds {
    empty: RejectionKind::ScreenNameEmpty,
    too_long: RejectionKind::ScreenNameTooLong,
    invalid_character: RejectionKind::ScreenNameInvalidCharacter,
};
const TITLE_REJECTION_KINDS: TextRejectionKinds = TextRejectionKinds {
    empty: RejectionKind::TitleEmpty,
    too_long: RejectionKind::TitleTooLong,
    invalid_character: RejectionKind::TitleInvalidCharacter,
};

struct TextRejectionKinds {
    empty: RejectionKind,
    too_long: RejectionKind,
    invalid_character: RejectionKind,
}

/// Participants of any status plus Spectators.
pub const fn get_member_limit(player_cap: u8) -> u32 {
    player_cap as u32 + SPECTATOR_LIMIT_PER_GAME
}

/// The trimmed screen name.
pub fn normalize_screen_name(screen_name: &str) -> Result<String, RejectionKind> {
    let trimmed_screen_name: &str = screen_name.trim();
    check_screen_name(trimmed_screen_name)?;

    Ok(String::from(trimmed_screen_name))
}

pub fn check_screen_name(screen_name: &str) -> Result<(), RejectionKind> {
    check_text(screen_name, SCREEN_NAME_CHARACTER_LIMIT, &SCREEN_NAME_REJECTION_KINDS)
}

/// The trimmed title.
pub fn normalize_title(title: &str) -> Result<String, RejectionKind> {
    let trimmed_title: &str = title.trim();
    check_title(trimmed_title)?;

    Ok(String::from(trimmed_title))
}

pub fn check_title(title: &str) -> Result<(), RejectionKind> {
    check_text(title, TITLE_CHARACTER_LIMIT, &TITLE_REJECTION_KINDS)
}

/// An empty password means none; passwords are not trimmed.
pub fn normalize_password(password: Option<String>) -> Result<Option<String>, RejectionKind> {
    let Some(password): Option<String> = password.filter(|password| !password.is_empty()) else {
        return Ok(None);
    };

    if password.len() > PASSWORD_BYTE_LIMIT {
        return Err(RejectionKind::PasswordTooLong);
    }

    Ok(Some(password))
}

fn check_text(text: &str, character_limit: usize, rejection_kinds: &TextRejectionKinds) -> Result<(), RejectionKind> {
    if text.is_empty() {
        return Err(rejection_kinds.empty);
    }

    if text.chars().count() > character_limit {
        return Err(rejection_kinds.too_long);
    }

    if text.chars().any(char::is_control) {
        return Err(rejection_kinds.invalid_character);
    }

    Ok(())
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

    #[test]
    fn normalize_screen_name_trims_and_accepts_the_character_limit() {
        let longest_screen_name: String = "é".repeat(64);

        assert_eq!(normalize_screen_name("  Blob  "), Ok(String::from("Blob")));
        assert_eq!(
            normalize_screen_name(&longest_screen_name),
            Ok(longest_screen_name.clone())
        );
    }

    #[test]
    fn normalize_screen_name_rejects_empty_long_and_control_text() {
        assert_eq!(normalize_screen_name(" \t "), Err(RejectionKind::ScreenNameEmpty));
        assert_eq!(
            normalize_screen_name(&"a".repeat(65)),
            Err(RejectionKind::ScreenNameTooLong)
        );
        assert_eq!(
            normalize_screen_name("Bl\u{7}ob"),
            Err(RejectionKind::ScreenNameInvalidCharacter)
        );
    }

    #[test]
    fn check_screen_name_does_not_trim() {
        assert_eq!(check_screen_name(" "), Ok(()));
        assert_eq!(check_screen_name(""), Err(RejectionKind::ScreenNameEmpty));
    }

    #[test]
    fn normalize_title_trims_and_checks_the_title_rules() {
        assert_eq!(normalize_title(" Arena "), Ok(String::from("Arena")));
        assert_eq!(normalize_title(&"t".repeat(64)), Ok("t".repeat(64)));
        assert_eq!(normalize_title(""), Err(RejectionKind::TitleEmpty));
        assert_eq!(normalize_title(&"t".repeat(65)), Err(RejectionKind::TitleTooLong));
        assert_eq!(normalize_title("Are\nna"), Err(RejectionKind::TitleInvalidCharacter));
    }

    #[test]
    fn normalize_password_treats_empty_as_none_and_limits_bytes() {
        let longest_password: String = "p".repeat(72);

        assert_eq!(normalize_password(None), Ok(None));
        assert_eq!(normalize_password(Some(String::new())), Ok(None));
        assert_eq!(
            normalize_password(Some(String::from(" pass "))),
            Ok(Some(String::from(" pass ")))
        );
        assert_eq!(
            normalize_password(Some(longest_password.clone())),
            Ok(Some(longest_password))
        );
        assert_eq!(
            normalize_password(Some("é".repeat(37))),
            Err(RejectionKind::PasswordTooLong)
        );
    }
}
