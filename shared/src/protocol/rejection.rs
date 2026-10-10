use crate::error::AppError;
use crate::member::TeamKind;
use crate::protocol::protocol_limits;

const GENERAL_FAULT_TEXT: &str = "Request is not valid right now";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestKind {
    SubscribeGameList,
    CreateGame,
    JoinGame,
    SpectateGame,
    Respawn,
    UpdateAppearance,
    LeaveGame,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingFieldKind {
    WorldSize,
    PlayerMinimum,
    PlayerCap,
    TeamCount,
    LeaderboardLength,
}

impl SettingFieldKind {
    pub fn lowest(self) -> u32 {
        match self {
            SettingFieldKind::WorldSize => protocol_limits::WORLD_SIZE_LOWEST_PIXELS,
            SettingFieldKind::PlayerMinimum => u32::from(protocol_limits::PLAYER_MINIMUM_LOWEST),
            SettingFieldKind::PlayerCap => u32::from(protocol_limits::PLAYER_CAP_LOWEST),
            SettingFieldKind::TeamCount => u32::from(protocol_limits::TEAM_COUNT_LOWEST),
            SettingFieldKind::LeaderboardLength => u32::from(protocol_limits::LEADERBOARD_LENGTH_LOWEST),
        }
    }

    pub fn highest(self) -> u32 {
        match self {
            SettingFieldKind::WorldSize => protocol_limits::WORLD_SIZE_HIGHEST_PIXELS,
            SettingFieldKind::PlayerMinimum | SettingFieldKind::PlayerCap => {
                u32::from(protocol_limits::PLAYER_CAP_HIGHEST)
            }
            SettingFieldKind::TeamCount => u32::from(protocol_limits::TEAM_COUNT_HIGHEST),
            SettingFieldKind::LeaderboardLength => u32::from(protocol_limits::LEADERBOARD_LENGTH_HIGHEST),
        }
    }

    fn label(self) -> &'static str {
        match self {
            SettingFieldKind::WorldSize => "Dimensions",
            SettingFieldKind::PlayerMinimum => "Player minimum",
            SettingFieldKind::PlayerCap => "Player cap",
            SettingFieldKind::TeamCount => "Team count",
            SettingFieldKind::LeaderboardLength => "Leaderboard length",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeBoundKind {
    Below,
    Above,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RejectionKind {
    ScreenNameEmpty,
    ScreenNameTooLong,
    ScreenNameInvalidCharacter,
    ScreenNameTaken,
    TitleEmpty,
    TitleTooLong,
    TitleInvalidCharacter,
    TitleTaken,
    SettingOutOfRange {
        field: SettingFieldKind,
        bound: RangeBoundKind,
    },
    SettingNotApplicable {
        field: SettingFieldKind,
    },
    PlayerCapBelowMinimum,
    PlayerCapBelowTeamCount,
    PasswordRequired,
    PasswordIncorrect,
    PasswordTooLong,
    GameNotFound,
    GameFull,
    SpectatorLimitReached,
    TeamUnbalanced {
        requested: TeamKind,
        smaller: TeamKind,
    },
    RoundInProgress,
    AlreadyInGame,
    NotInGame,
    NotDead,
    ServerFull,
    ServerBusy,
    RateLimited,
}

impl RejectionKind {
    pub fn message_text(self) -> String {
        match self {
            RejectionKind::ScreenNameEmpty => String::from("Screen name cannot be left empty"),
            RejectionKind::ScreenNameTooLong => format!(
                "Screen name can be at most {} characters",
                protocol_limits::SCREEN_NAME_CHARACTER_LIMIT,
            ),
            RejectionKind::ScreenNameInvalidCharacter => String::from("Screen name cannot contain control characters"),
            RejectionKind::ScreenNameTaken => String::from("Name matches that of another player"),
            RejectionKind::TitleEmpty => String::from("Title cannot be left blank"),
            RejectionKind::TitleTooLong => {
                format!(
                    "Title can be at most {} characters",
                    protocol_limits::TITLE_CHARACTER_LIMIT
                )
            }
            RejectionKind::TitleInvalidCharacter => String::from("Title cannot contain control characters"),
            RejectionKind::TitleTaken => String::from("Title matches that of another game"),
            RejectionKind::SettingOutOfRange { field, bound } => get_out_of_range_text(field, bound),
            RejectionKind::SettingNotApplicable { .. } => String::from("Setting does not apply to this mode"),
            RejectionKind::PlayerCapBelowMinimum => String::from("Player cap cannot be less than player minimum"),
            RejectionKind::PlayerCapBelowTeamCount => {
                String::from("Player cap cannot be less than the number of teams")
            }
            RejectionKind::PasswordRequired => String::from("A password is required for this game"),
            RejectionKind::PasswordIncorrect => String::from("Password is invalid"),
            RejectionKind::PasswordTooLong => {
                format!("Password can be at most {} bytes", protocol_limits::PASSWORD_BYTE_LIMIT)
            }
            RejectionKind::GameNotFound => String::from("The game has closed"),
            RejectionKind::GameFull => String::from("Game is at maximum player capacity"),
            RejectionKind::SpectatorLimitReached => String::from("Game is at maximum spectator capacity"),
            RejectionKind::TeamUnbalanced { requested, smaller } => format!(
                "Cannot join {} team because it already has more players than {}",
                requested.as_str(),
                smaller.as_str(),
            ),
            RejectionKind::RoundInProgress => String::from("Wait for the round to complete"),
            RejectionKind::AlreadyInGame | RejectionKind::NotInGame | RejectionKind::NotDead => {
                String::from(GENERAL_FAULT_TEXT)
            }
            RejectionKind::ServerFull => String::from("The server is at maximum game capacity"),
            RejectionKind::ServerBusy => String::from("The server is busy; try again"),
            RejectionKind::RateLimited => String::from("Too many requests; try again"),
        }
    }

    /// For a payload which fails a request rule outside a request, such as a screen name in a snapshot.
    pub fn to_app_error(self) -> AppError {
        AppError::new(&format!("rejected: {self:?}"))
    }
}

fn get_out_of_range_text(field: SettingFieldKind, bound: RangeBoundKind) -> String {
    let (relation, limit): (&str, u32) = match bound {
        RangeBoundKind::Below => ("must be at least", field.lowest()),
        RangeBoundKind::Above => ("can be at most", field.highest()),
    };
    let limit_text: String = match field {
        SettingFieldKind::WorldSize => format!("{limit} x {limit} px"),
        SettingFieldKind::PlayerMinimum
        | SettingFieldKind::PlayerCap
        | SettingFieldKind::TeamCount
        | SettingFieldKind::LeaderboardLength => limit.to_string(),
    };

    format!("{} {relation} {limit_text}", field.label())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_text_gives_the_text_of_every_rejection() {
        let expected_texts: Vec<(RejectionKind, &str)> = vec![
            (RejectionKind::ScreenNameEmpty, "Screen name cannot be left empty"),
            (
                RejectionKind::ScreenNameTooLong,
                "Screen name can be at most 64 characters",
            ),
            (
                RejectionKind::ScreenNameInvalidCharacter,
                "Screen name cannot contain control characters",
            ),
            (RejectionKind::ScreenNameTaken, "Name matches that of another player"),
            (RejectionKind::TitleEmpty, "Title cannot be left blank"),
            (RejectionKind::TitleTooLong, "Title can be at most 64 characters"),
            (
                RejectionKind::TitleInvalidCharacter,
                "Title cannot contain control characters",
            ),
            (RejectionKind::TitleTaken, "Title matches that of another game"),
            (
                RejectionKind::SettingNotApplicable {
                    field: SettingFieldKind::TeamCount,
                },
                "Setting does not apply to this mode",
            ),
            (
                RejectionKind::PlayerCapBelowMinimum,
                "Player cap cannot be less than player minimum",
            ),
            (
                RejectionKind::PlayerCapBelowTeamCount,
                "Player cap cannot be less than the number of teams",
            ),
            (RejectionKind::PasswordRequired, "A password is required for this game"),
            (RejectionKind::PasswordIncorrect, "Password is invalid"),
            (RejectionKind::PasswordTooLong, "Password can be at most 72 bytes"),
            (RejectionKind::GameNotFound, "The game has closed"),
            (RejectionKind::GameFull, "Game is at maximum player capacity"),
            (
                RejectionKind::SpectatorLimitReached,
                "Game is at maximum spectator capacity",
            ),
            (
                RejectionKind::TeamUnbalanced {
                    requested: TeamKind::Red,
                    smaller: TeamKind::Green,
                },
                "Cannot join red team because it already has more players than green",
            ),
            (RejectionKind::RoundInProgress, "Wait for the round to complete"),
            (RejectionKind::AlreadyInGame, "Request is not valid right now"),
            (RejectionKind::NotInGame, "Request is not valid right now"),
            (RejectionKind::NotDead, "Request is not valid right now"),
            (RejectionKind::ServerFull, "The server is at maximum game capacity"),
            (RejectionKind::ServerBusy, "The server is busy; try again"),
            (RejectionKind::RateLimited, "Too many requests; try again"),
        ];

        for (rejection_kind, expected_text) in expected_texts {
            assert_eq!(rejection_kind.message_text(), expected_text);
        }
    }

    #[test]
    fn message_text_names_the_limit_of_an_out_of_range_setting() {
        let expected_texts: Vec<(SettingFieldKind, RangeBoundKind, &str)> = vec![
            (
                SettingFieldKind::WorldSize,
                RangeBoundKind::Below,
                "Dimensions must be at least 300 x 300 px",
            ),
            (
                SettingFieldKind::WorldSize,
                RangeBoundKind::Above,
                "Dimensions can be at most 100000 x 100000 px",
            ),
            (
                SettingFieldKind::PlayerMinimum,
                RangeBoundKind::Below,
                "Player minimum must be at least 2",
            ),
            (
                SettingFieldKind::PlayerMinimum,
                RangeBoundKind::Above,
                "Player minimum can be at most 32",
            ),
            (
                SettingFieldKind::PlayerCap,
                RangeBoundKind::Below,
                "Player cap must be at least 2",
            ),
            (
                SettingFieldKind::PlayerCap,
                RangeBoundKind::Above,
                "Player cap can be at most 32",
            ),
            (
                SettingFieldKind::TeamCount,
                RangeBoundKind::Below,
                "Team count must be at least 2",
            ),
            (
                SettingFieldKind::TeamCount,
                RangeBoundKind::Above,
                "Team count can be at most 4",
            ),
            (
                SettingFieldKind::LeaderboardLength,
                RangeBoundKind::Below,
                "Leaderboard length must be at least 1",
            ),
            (
                SettingFieldKind::LeaderboardLength,
                RangeBoundKind::Above,
                "Leaderboard length can be at most 20",
            ),
        ];

        for (field, bound, expected_text) in expected_texts {
            assert_eq!(
                RejectionKind::SettingOutOfRange { field, bound }.message_text(),
                expected_text
            );
        }
    }

    #[test]
    fn to_app_error_names_the_rejection() {
        let error: AppError = RejectionKind::ScreenNameTooLong.to_app_error();

        assert_eq!(error.message, "Error: rejected: ScreenNameTooLong");
    }
}
