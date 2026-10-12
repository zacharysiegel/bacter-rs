use leptos::prelude::*;

use shared::game::{GameModeKind, GameSettings};
use shared::protocol::protocol_limits;
use shared::protocol::{GameModeKindSerial, GameSettingsSerialIn, RejectionKind, WorldShapeKindSerial};
use shared::world::WorldShapeKind;

use crate::app::AppSignals;
use crate::browser::game_browser;
use crate::menu::{
    FieldAvailabilityKind, FieldVisibilityKind, ListOption, MenuFieldKind, MenuForm, MenuInput, MenuIssue, MenuRow,
};
use crate::screen::{JoinMenuOrigin, Screen};

pub const DEFAULT_WORLD_SIZE_PIXELS: u32 = 800;
pub const DEFAULT_PLAYER_MINIMUM: u8 = 4;
pub const DEFAULT_PLAYER_CAP: u8 = 16;
pub const DEFAULT_TEAM_COUNT: u8 = 2;
pub const DEFAULT_LEADERBOARD_LENGTH: u8 = 10;
const WORLD_SHAPE_KINDS: [WorldShapeKind; 2] = [WorldShapeKind::Rectangle, WorldShapeKind::Ellipse];
const WORLD_SHAPE_LABELS: [&str; 2] = ["Square", "Circle"];
const GAME_MODE_KINDS: [GameModeKind; 3] = [GameModeKind::FreeForAll, GameModeKind::Skirmish, GameModeKind::Survival];
const SURVIVAL_MODES: &[GameModeKind] = &[GameModeKind::Survival];
const SKIRMISH_MODES: &[GameModeKind] = &[GameModeKind::Skirmish];
const RANKED_PLAYER_MODES: &[GameModeKind] = &[GameModeKind::FreeForAll, GameModeKind::Survival];

/// Outlives the create menu: Back from the join step shows the entered values.
#[derive(Clone, Copy, Debug)]
pub struct CreateMenuFields {
    pub title: RwSignal<String>,
    pub password: RwSignal<String>,
    pub world_shape_index: RwSignal<usize>,
    /// World width and height are one value; created worlds are square.
    pub world_size: RwSignal<Option<i64>>,
    pub player_minimum: RwSignal<Option<i64>>,
    pub player_cap: RwSignal<Option<i64>>,
    pub team_count: RwSignal<Option<i64>>,
    pub leaderboard_length: RwSignal<Option<i64>>,
    pub game_mode_index: RwSignal<usize>,
}

impl CreateMenuFields {
    pub fn new() -> CreateMenuFields {
        CreateMenuFields {
            title: RwSignal::new(String::new()),
            password: RwSignal::new(String::new()),
            world_shape_index: RwSignal::new(0),
            world_size: RwSignal::new(None),
            player_minimum: RwSignal::new(None),
            player_cap: RwSignal::new(None),
            team_count: RwSignal::new(None),
            leaderboard_length: RwSignal::new(None),
            game_mode_index: RwSignal::new(0),
        }
    }

    pub fn reset(&self) {
        self.title.set(String::new());
        self.password.set(String::new());
        self.world_shape_index.set(0);
        self.world_size.set(None);
        self.player_minimum.set(None);
        self.player_cap.set(None);
        self.team_count.set(None);
        self.leaderboard_length.set(None);
        self.game_mode_index.set(0);
    }

    pub fn get_values(&self) -> CreateMenuValues {
        CreateMenuValues {
            title: self.title.get_untracked(),
            password: self.password.get_untracked(),
            world_shape: WORLD_SHAPE_KINDS[self.world_shape_index.get_untracked()],
            world_size: self.world_size.get_untracked(),
            player_minimum: self.player_minimum.get_untracked(),
            player_cap: self.player_cap.get_untracked(),
            team_count: self.team_count.get_untracked(),
            leaderboard_length: self.leaderboard_length.get_untracked(),
            mode: GAME_MODE_KINDS[self.game_mode_index.get_untracked()],
        }
    }
}

/// Number fields hold `None` when blank.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateMenuValues {
    pub title: String,
    pub password: String,
    pub world_shape: WorldShapeKind,
    pub world_size: Option<i64>,
    pub player_minimum: Option<i64>,
    pub player_cap: Option<i64>,
    pub team_count: Option<i64>,
    pub leaderboard_length: Option<i64>,
    pub mode: GameModeKind,
}

#[component]
pub fn CreateMenu() -> impl IntoView {
    let signals: AppSignals = expect_context();
    let fields: CreateMenuFields = expect_context();
    let issues: RwSignal<Vec<MenuIssue>> = RwSignal::new(Vec::new());
    let mode: Signal<GameModeKind> = Signal::derive(move || GAME_MODE_KINDS[fields.game_mode_index.get()]);
    let shown_in = move |shown_modes: &'static [GameModeKind]| {
        Signal::derive(move || {
            if shown_modes.contains(&mode.get()) {
                FieldVisibilityKind::Shown
            } else {
                FieldVisibilityKind::Hidden
            }
        })
    };
    let rows: Vec<MenuRow> = vec![
        MenuRow::shown(
            MenuFieldKind::GameTitle,
            "Game Title",
            MenuInput::Text { value: fields.title },
        ),
        MenuRow::shown(
            MenuFieldKind::Password,
            "Password",
            MenuInput::Text { value: fields.password },
        ),
        MenuRow::shown(
            MenuFieldKind::WorldShape,
            "World Shape",
            create_list_input(fields.world_shape_index, &WORLD_SHAPE_LABELS),
        ),
        MenuRow::shown(
            MenuFieldKind::WorldWidth,
            "World Width",
            MenuInput::Number {
                value: fields.world_size,
                placeholder: DEFAULT_WORLD_SIZE_PIXELS,
            },
        ),
        MenuRow::shown(
            MenuFieldKind::WorldHeight,
            "World Height",
            MenuInput::Number {
                value: fields.world_size,
                placeholder: DEFAULT_WORLD_SIZE_PIXELS,
            },
        ),
        MenuRow::shown(
            MenuFieldKind::PlayerMinimum,
            "Player Minimum",
            MenuInput::Number {
                value: fields.player_minimum,
                placeholder: u32::from(DEFAULT_PLAYER_MINIMUM),
            },
        )
        .with_visibility(shown_in(SURVIVAL_MODES)),
        MenuRow::shown(
            MenuFieldKind::PlayerCap,
            "Player Cap",
            MenuInput::Number {
                value: fields.player_cap,
                placeholder: u32::from(DEFAULT_PLAYER_CAP),
            },
        ),
        MenuRow::shown(
            MenuFieldKind::TeamCount,
            "Team Count",
            MenuInput::Number {
                value: fields.team_count,
                placeholder: u32::from(DEFAULT_TEAM_COUNT),
            },
        )
        .with_visibility(shown_in(SKIRMISH_MODES)),
        MenuRow::shown(
            MenuFieldKind::LeaderboardLength,
            "Leaderboard Length",
            MenuInput::Number {
                value: fields.leaderboard_length,
                placeholder: u32::from(DEFAULT_LEADERBOARD_LENGTH),
            },
        )
        .with_visibility(shown_in(RANKED_PLAYER_MODES)),
        MenuRow::shown(
            MenuFieldKind::GameMode,
            "Game Mode",
            create_list_input(
                fields.game_mode_index,
                &GAME_MODE_KINDS.map(game_browser::get_mode_label),
            ),
        ),
    ];
    let on_submit: Callback<()> = Callback::new(move |_| {
        let validated: Result<JoinMenuOrigin, Vec<MenuIssue>> = validate_create_menu(&fields.get_values());

        match validated {
            Ok(origin) => signals.navigate(Screen::JoinMenu { origin }),
            Err(found_issues) => issues.set(found_issues),
        }
    });
    let on_back: Callback<()> = Callback::new(move |_| signals.go_back());

    view! {
        <MenuForm
            header="Game Creation Options"
            submit_label="Create"
            rows=rows
            issues=issues
            on_submit=on_submit
            on_back=on_back
        />
    }
}

/// Blank fields take their defaults; fields the mode hides are left out or take their defaults.
pub fn resolve_game_settings(values: &CreateMenuValues) -> GameSettingsSerialIn {
    let player_minimum: Option<u8> = match values.mode {
        GameModeKind::Survival => Some(get_saturated_u8(values.player_minimum, DEFAULT_PLAYER_MINIMUM)),
        GameModeKind::FreeForAll | GameModeKind::Skirmish => None,
    };
    let team_count: Option<u8> = match values.mode {
        GameModeKind::Skirmish => Some(get_saturated_u8(values.team_count, DEFAULT_TEAM_COUNT)),
        GameModeKind::FreeForAll | GameModeKind::Survival => None,
    };
    let leaderboard_length: u8 = match values.mode {
        GameModeKind::FreeForAll | GameModeKind::Survival => {
            get_saturated_u8(values.leaderboard_length, DEFAULT_LEADERBOARD_LENGTH)
        }
        GameModeKind::Skirmish => DEFAULT_LEADERBOARD_LENGTH,
    };

    GameSettingsSerialIn {
        title: values.title.clone(),
        mode: GameModeKindSerial::from(&values.mode),
        world_shape: WorldShapeKindSerial::from(&values.world_shape),
        world_size_pixels: get_saturated_u32(values.world_size, DEFAULT_WORLD_SIZE_PIXELS),
        player_minimum,
        player_cap: get_saturated_u8(values.player_cap, DEFAULT_PLAYER_CAP),
        team_count,
        leaderboard_length,
    }
}

/// The settings pass the same validation as the server's; the join step follows.
pub fn validate_create_menu(values: &CreateMenuValues) -> Result<JoinMenuOrigin, Vec<MenuIssue>> {
    let settings: GameSettingsSerialIn = resolve_game_settings(values);
    let validated_settings: Result<GameSettings, RejectionKind> = GameSettings::try_from(settings.clone());
    let normalized_password: Result<Option<String>, RejectionKind> =
        protocol_limits::normalize_password(Some(values.password.clone()));

    match (validated_settings, normalized_password) {
        (Ok(_), Ok(password)) => Ok(JoinMenuOrigin::Create { settings, password }),
        (settings_result, password_result) => Err(settings_result
            .err()
            .into_iter()
            .chain(password_result.err())
            .map(MenuIssue::from_rejection)
            .collect()),
    }
}

fn create_list_input(selected_index: RwSignal<usize>, labels: &[&str]) -> MenuInput {
    let options: Vec<ListOption> = labels
        .iter()
        .map(|label| ListOption {
            label: String::from(*label),
            swatch: None,
        })
        .collect();

    MenuInput::List {
        selected_index,
        options: Signal::stored(options),
        availability: Signal::stored(FieldAvailabilityKind::Enabled),
    }
}

/// Out-of-range numbers keep their side of the range, so validation names the right bound.
fn get_saturated_u8(value: Option<i64>, default: u8) -> u8 {
    value.map_or(default, |value| {
        u8::try_from(value.clamp(0, i64::from(u8::MAX))).unwrap_or(u8::MAX)
    })
}

fn get_saturated_u32(value: Option<i64>, default: u32) -> u32 {
    value.map_or(default, |value| {
        u32::try_from(value.clamp(0, i64::from(u32::MAX))).unwrap_or(u32::MAX)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_blank_values(mode: GameModeKind) -> CreateMenuValues {
        CreateMenuValues {
            title: String::from("Petri"),
            password: String::new(),
            world_shape: WorldShapeKind::Rectangle,
            world_size: None,
            player_minimum: None,
            player_cap: None,
            team_count: None,
            leaderboard_length: None,
            mode,
        }
    }

    #[test]
    fn resolve_game_settings_fills_blank_fields_with_the_defaults() {
        assert_eq!(
            resolve_game_settings(&create_blank_values(GameModeKind::FreeForAll)),
            GameSettingsSerialIn {
                title: String::from("Petri"),
                mode: GameModeKindSerial::FreeForAll,
                world_shape: WorldShapeKindSerial::Rectangle,
                world_size_pixels: 800,
                player_minimum: None,
                player_cap: 16,
                team_count: None,
                leaderboard_length: 10,
            },
        );
    }

    #[test]
    fn resolve_game_settings_includes_only_the_fields_of_the_mode() {
        let mut skirmish_values: CreateMenuValues = create_blank_values(GameModeKind::Skirmish);
        skirmish_values.player_minimum = Some(6);
        skirmish_values.leaderboard_length = Some(3);
        let mut survival_values: CreateMenuValues = create_blank_values(GameModeKind::Survival);
        survival_values.team_count = Some(3);

        let skirmish_settings: GameSettingsSerialIn = resolve_game_settings(&skirmish_values);
        let survival_settings: GameSettingsSerialIn = resolve_game_settings(&survival_values);

        assert_eq!(skirmish_settings.player_minimum, None);
        assert_eq!(skirmish_settings.team_count, Some(2));
        assert_eq!(skirmish_settings.leaderboard_length, 10);
        assert_eq!(survival_settings.player_minimum, Some(4));
        assert_eq!(survival_settings.team_count, None);
    }

    #[test]
    fn resolve_game_settings_saturates_numbers_outside_the_wire_range() {
        let mut values: CreateMenuValues = create_blank_values(GameModeKind::FreeForAll);
        values.player_cap = Some(-5);
        values.leaderboard_length = Some(1000);
        values.world_size = Some(-1);

        let settings: GameSettingsSerialIn = resolve_game_settings(&values);

        assert_eq!(settings.player_cap, 0);
        assert_eq!(settings.leaderboard_length, 255);
        assert_eq!(settings.world_size_pixels, 0);
    }

    #[test]
    fn validate_create_menu_opens_the_join_step_with_valid_settings() {
        let mut values: CreateMenuValues = create_blank_values(GameModeKind::FreeForAll);
        values.password = String::from("hunter2");

        assert_eq!(
            validate_create_menu(&values),
            Ok(JoinMenuOrigin::Create {
                settings: resolve_game_settings(&values),
                password: Some(String::from("hunter2")),
            }),
        );
    }

    #[test]
    fn validate_create_menu_reports_the_rejected_setting_at_its_field() {
        let mut values: CreateMenuValues = create_blank_values(GameModeKind::FreeForAll);
        values.world_size = Some(200);

        assert_eq!(
            validate_create_menu(&values),
            Err(vec![MenuIssue::at_field(
                MenuFieldKind::WorldWidth,
                "Dimensions must be at least 300 x 300 px",
            )]),
        );
    }

    #[test]
    fn validate_create_menu_reports_a_blank_title_and_a_long_password_together() {
        let mut values: CreateMenuValues = create_blank_values(GameModeKind::Survival);
        values.title = String::from("  ");
        values.password = "p".repeat(protocol_limits::PASSWORD_BYTE_LIMIT + 1);

        assert_eq!(
            validate_create_menu(&values),
            Err(vec![
                MenuIssue::at_field(MenuFieldKind::GameTitle, "Title cannot be left blank"),
                MenuIssue::at_field(MenuFieldKind::Password, "Password can be at most 72 bytes"),
            ]),
        );
    }
}
