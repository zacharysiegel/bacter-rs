use bitcode::{Decode, Encode};

use crate::error::AppError;
use crate::member::TeamKind;
use crate::protocol::TeamKindSerial;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum RequestKindSerialOut {
    SubscribeGameList,
    CreateGame,
    JoinGame,
    SpectateGame,
    Respawn,
    UpdateAppearance,
    LeaveGame,
}

impl From<&RequestKind> for RequestKindSerialOut {
    fn from(request: &RequestKind) -> RequestKindSerialOut {
        match request {
            RequestKind::SubscribeGameList => RequestKindSerialOut::SubscribeGameList,
            RequestKind::CreateGame => RequestKindSerialOut::CreateGame,
            RequestKind::JoinGame => RequestKindSerialOut::JoinGame,
            RequestKind::SpectateGame => RequestKindSerialOut::SpectateGame,
            RequestKind::Respawn => RequestKindSerialOut::Respawn,
            RequestKind::UpdateAppearance => RequestKindSerialOut::UpdateAppearance,
            RequestKind::LeaveGame => RequestKindSerialOut::LeaveGame,
        }
    }
}

impl From<RequestKindSerialOut> for RequestKind {
    fn from(request_serial_out: RequestKindSerialOut) -> RequestKind {
        match request_serial_out {
            RequestKindSerialOut::SubscribeGameList => RequestKind::SubscribeGameList,
            RequestKindSerialOut::CreateGame => RequestKind::CreateGame,
            RequestKindSerialOut::JoinGame => RequestKind::JoinGame,
            RequestKindSerialOut::SpectateGame => RequestKind::SpectateGame,
            RequestKindSerialOut::Respawn => RequestKind::Respawn,
            RequestKindSerialOut::UpdateAppearance => RequestKind::UpdateAppearance,
            RequestKindSerialOut::LeaveGame => RequestKind::LeaveGame,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum SettingFieldKindSerialOut {
    WorldSize,
    PlayerMinimum,
    PlayerCap,
    TeamCount,
    LeaderboardLength,
}

impl From<&SettingFieldKind> for SettingFieldKindSerialOut {
    fn from(field: &SettingFieldKind) -> SettingFieldKindSerialOut {
        match field {
            SettingFieldKind::WorldSize => SettingFieldKindSerialOut::WorldSize,
            SettingFieldKind::PlayerMinimum => SettingFieldKindSerialOut::PlayerMinimum,
            SettingFieldKind::PlayerCap => SettingFieldKindSerialOut::PlayerCap,
            SettingFieldKind::TeamCount => SettingFieldKindSerialOut::TeamCount,
            SettingFieldKind::LeaderboardLength => SettingFieldKindSerialOut::LeaderboardLength,
        }
    }
}

impl From<SettingFieldKindSerialOut> for SettingFieldKind {
    fn from(field_serial_out: SettingFieldKindSerialOut) -> SettingFieldKind {
        match field_serial_out {
            SettingFieldKindSerialOut::WorldSize => SettingFieldKind::WorldSize,
            SettingFieldKindSerialOut::PlayerMinimum => SettingFieldKind::PlayerMinimum,
            SettingFieldKindSerialOut::PlayerCap => SettingFieldKind::PlayerCap,
            SettingFieldKindSerialOut::TeamCount => SettingFieldKind::TeamCount,
            SettingFieldKindSerialOut::LeaderboardLength => SettingFieldKind::LeaderboardLength,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum RangeBoundKindSerialOut {
    Below,
    Above,
}

impl From<&RangeBoundKind> for RangeBoundKindSerialOut {
    fn from(bound: &RangeBoundKind) -> RangeBoundKindSerialOut {
        match bound {
            RangeBoundKind::Below => RangeBoundKindSerialOut::Below,
            RangeBoundKind::Above => RangeBoundKindSerialOut::Above,
        }
    }
}

impl From<RangeBoundKindSerialOut> for RangeBoundKind {
    fn from(bound_serial_out: RangeBoundKindSerialOut) -> RangeBoundKind {
        match bound_serial_out {
            RangeBoundKindSerialOut::Below => RangeBoundKind::Below,
            RangeBoundKindSerialOut::Above => RangeBoundKind::Above,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum RejectionKindSerialOut {
    ScreenNameEmpty,
    ScreenNameTooLong,
    ScreenNameInvalidCharacter,
    ScreenNameTaken,
    TitleEmpty,
    TitleTooLong,
    TitleInvalidCharacter,
    TitleTaken,
    SettingOutOfRange {
        field: SettingFieldKindSerialOut,
        bound: RangeBoundKindSerialOut,
    },
    SettingNotApplicable {
        field: SettingFieldKindSerialOut,
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
        requested: TeamKindSerial,
        smaller: TeamKindSerial,
    },
    RoundInProgress,
    AlreadyInGame,
    NotInGame,
    NotDead,
    ServerFull,
    ServerBusy,
    RateLimited,
}

impl From<&RejectionKind> for RejectionKindSerialOut {
    fn from(rejection_kind: &RejectionKind) -> RejectionKindSerialOut {
        match rejection_kind {
            RejectionKind::ScreenNameEmpty => RejectionKindSerialOut::ScreenNameEmpty,
            RejectionKind::ScreenNameTooLong => RejectionKindSerialOut::ScreenNameTooLong,
            RejectionKind::ScreenNameInvalidCharacter => RejectionKindSerialOut::ScreenNameInvalidCharacter,
            RejectionKind::ScreenNameTaken => RejectionKindSerialOut::ScreenNameTaken,
            RejectionKind::TitleEmpty => RejectionKindSerialOut::TitleEmpty,
            RejectionKind::TitleTooLong => RejectionKindSerialOut::TitleTooLong,
            RejectionKind::TitleInvalidCharacter => RejectionKindSerialOut::TitleInvalidCharacter,
            RejectionKind::TitleTaken => RejectionKindSerialOut::TitleTaken,
            RejectionKind::SettingOutOfRange { field, bound } => RejectionKindSerialOut::SettingOutOfRange {
                field: SettingFieldKindSerialOut::from(field),
                bound: RangeBoundKindSerialOut::from(bound),
            },
            RejectionKind::SettingNotApplicable { field } => RejectionKindSerialOut::SettingNotApplicable {
                field: SettingFieldKindSerialOut::from(field),
            },
            RejectionKind::PlayerCapBelowMinimum => RejectionKindSerialOut::PlayerCapBelowMinimum,
            RejectionKind::PlayerCapBelowTeamCount => RejectionKindSerialOut::PlayerCapBelowTeamCount,
            RejectionKind::PasswordRequired => RejectionKindSerialOut::PasswordRequired,
            RejectionKind::PasswordIncorrect => RejectionKindSerialOut::PasswordIncorrect,
            RejectionKind::PasswordTooLong => RejectionKindSerialOut::PasswordTooLong,
            RejectionKind::GameNotFound => RejectionKindSerialOut::GameNotFound,
            RejectionKind::GameFull => RejectionKindSerialOut::GameFull,
            RejectionKind::SpectatorLimitReached => RejectionKindSerialOut::SpectatorLimitReached,
            RejectionKind::TeamUnbalanced { requested, smaller } => RejectionKindSerialOut::TeamUnbalanced {
                requested: TeamKindSerial::from(requested),
                smaller: TeamKindSerial::from(smaller),
            },
            RejectionKind::RoundInProgress => RejectionKindSerialOut::RoundInProgress,
            RejectionKind::AlreadyInGame => RejectionKindSerialOut::AlreadyInGame,
            RejectionKind::NotInGame => RejectionKindSerialOut::NotInGame,
            RejectionKind::NotDead => RejectionKindSerialOut::NotDead,
            RejectionKind::ServerFull => RejectionKindSerialOut::ServerFull,
            RejectionKind::ServerBusy => RejectionKindSerialOut::ServerBusy,
            RejectionKind::RateLimited => RejectionKindSerialOut::RateLimited,
        }
    }
}

impl From<RejectionKindSerialOut> for RejectionKind {
    fn from(rejection_kind_serial_out: RejectionKindSerialOut) -> RejectionKind {
        match rejection_kind_serial_out {
            RejectionKindSerialOut::ScreenNameEmpty => RejectionKind::ScreenNameEmpty,
            RejectionKindSerialOut::ScreenNameTooLong => RejectionKind::ScreenNameTooLong,
            RejectionKindSerialOut::ScreenNameInvalidCharacter => RejectionKind::ScreenNameInvalidCharacter,
            RejectionKindSerialOut::ScreenNameTaken => RejectionKind::ScreenNameTaken,
            RejectionKindSerialOut::TitleEmpty => RejectionKind::TitleEmpty,
            RejectionKindSerialOut::TitleTooLong => RejectionKind::TitleTooLong,
            RejectionKindSerialOut::TitleInvalidCharacter => RejectionKind::TitleInvalidCharacter,
            RejectionKindSerialOut::TitleTaken => RejectionKind::TitleTaken,
            RejectionKindSerialOut::SettingOutOfRange { field, bound } => RejectionKind::SettingOutOfRange {
                field: SettingFieldKind::from(field),
                bound: RangeBoundKind::from(bound),
            },
            RejectionKindSerialOut::SettingNotApplicable { field } => RejectionKind::SettingNotApplicable {
                field: SettingFieldKind::from(field),
            },
            RejectionKindSerialOut::PlayerCapBelowMinimum => RejectionKind::PlayerCapBelowMinimum,
            RejectionKindSerialOut::PlayerCapBelowTeamCount => RejectionKind::PlayerCapBelowTeamCount,
            RejectionKindSerialOut::PasswordRequired => RejectionKind::PasswordRequired,
            RejectionKindSerialOut::PasswordIncorrect => RejectionKind::PasswordIncorrect,
            RejectionKindSerialOut::PasswordTooLong => RejectionKind::PasswordTooLong,
            RejectionKindSerialOut::GameNotFound => RejectionKind::GameNotFound,
            RejectionKindSerialOut::GameFull => RejectionKind::GameFull,
            RejectionKindSerialOut::SpectatorLimitReached => RejectionKind::SpectatorLimitReached,
            RejectionKindSerialOut::TeamUnbalanced { requested, smaller } => RejectionKind::TeamUnbalanced {
                requested: TeamKind::from(requested),
                smaller: TeamKind::from(smaller),
            },
            RejectionKindSerialOut::RoundInProgress => RejectionKind::RoundInProgress,
            RejectionKindSerialOut::AlreadyInGame => RejectionKind::AlreadyInGame,
            RejectionKindSerialOut::NotInGame => RejectionKind::NotInGame,
            RejectionKindSerialOut::NotDead => RejectionKind::NotDead,
            RejectionKindSerialOut::ServerFull => RejectionKind::ServerFull,
            RejectionKindSerialOut::ServerBusy => RejectionKind::ServerBusy,
            RejectionKindSerialOut::RateLimited => RejectionKind::RateLimited,
        }
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

    fn get_every_rejection_kind() -> Vec<RejectionKind> {
        let fields: [SettingFieldKind; 5] = [
            SettingFieldKind::WorldSize,
            SettingFieldKind::PlayerMinimum,
            SettingFieldKind::PlayerCap,
            SettingFieldKind::TeamCount,
            SettingFieldKind::LeaderboardLength,
        ];
        let mut rejection_kinds: Vec<RejectionKind> = vec![
            RejectionKind::ScreenNameEmpty,
            RejectionKind::ScreenNameTooLong,
            RejectionKind::ScreenNameInvalidCharacter,
            RejectionKind::ScreenNameTaken,
            RejectionKind::TitleEmpty,
            RejectionKind::TitleTooLong,
            RejectionKind::TitleInvalidCharacter,
            RejectionKind::TitleTaken,
            RejectionKind::PlayerCapBelowMinimum,
            RejectionKind::PlayerCapBelowTeamCount,
            RejectionKind::PasswordRequired,
            RejectionKind::PasswordIncorrect,
            RejectionKind::PasswordTooLong,
            RejectionKind::GameNotFound,
            RejectionKind::GameFull,
            RejectionKind::SpectatorLimitReached,
            RejectionKind::TeamUnbalanced {
                requested: TeamKind::Pink,
                smaller: TeamKind::Blue,
            },
            RejectionKind::RoundInProgress,
            RejectionKind::AlreadyInGame,
            RejectionKind::NotInGame,
            RejectionKind::NotDead,
            RejectionKind::ServerFull,
            RejectionKind::ServerBusy,
            RejectionKind::RateLimited,
        ];

        for field in fields {
            rejection_kinds.push(RejectionKind::SettingNotApplicable { field });

            for bound in [RangeBoundKind::Below, RangeBoundKind::Above] {
                rejection_kinds.push(RejectionKind::SettingOutOfRange { field, bound });
            }
        }

        rejection_kinds
    }

    #[test]
    fn rejection_kind_serial_out_converts_back_to_every_rejection() {
        for rejection_kind in get_every_rejection_kind() {
            assert_eq!(
                RejectionKind::from(RejectionKindSerialOut::from(&rejection_kind)),
                rejection_kind
            );
        }
    }

    #[test]
    fn request_kind_serial_out_converts_back_to_every_request() {
        let requests: [RequestKind; 7] = [
            RequestKind::SubscribeGameList,
            RequestKind::CreateGame,
            RequestKind::JoinGame,
            RequestKind::SpectateGame,
            RequestKind::Respawn,
            RequestKind::UpdateAppearance,
            RequestKind::LeaveGame,
        ];

        for request in requests {
            assert_eq!(RequestKind::from(RequestKindSerialOut::from(&request)), request);
        }
    }
}
