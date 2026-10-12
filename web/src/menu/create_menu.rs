use leptos::prelude::*;

use shared::game::{GameModeKind, GameSettings};
use shared::protocol::protocol_limits;
use shared::protocol::{GameModeKindSerial, GameSettingsSerialIn, RejectionKind, WorldShapeKindSerial};
use shared::world::WorldShapeKind;

use crate::app::AppSignals;
use crate::browser::game_browser;
use crate::menu::menu_form;
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
const BLANK_WORLD_SHAPE_INDEX: usize = 0;
const BLANK_GAME_MODE_INDEX: usize = 0;

/// Lives in app context, beyond the create menu's lifetime.
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
        let blank_values: CreateMenuValues = CreateMenuValues::blank();

        CreateMenuFields {
            title: RwSignal::new(blank_values.title),
            password: RwSignal::new(blank_values.password),
            world_shape_index: RwSignal::new(BLANK_WORLD_SHAPE_INDEX),
            world_size: RwSignal::new(blank_values.world_size),
            player_minimum: RwSignal::new(blank_values.player_minimum),
            player_cap: RwSignal::new(blank_values.player_cap),
            team_count: RwSignal::new(blank_values.team_count),
            leaderboard_length: RwSignal::new(blank_values.leaderboard_length),
            game_mode_index: RwSignal::new(BLANK_GAME_MODE_INDEX),
        }
    }

    pub fn reset(&self) {
        let blank_values: CreateMenuValues = CreateMenuValues::blank();

        self.title.set(blank_values.title);
        self.password.set(blank_values.password);
        self.world_shape_index.set(BLANK_WORLD_SHAPE_INDEX);
        self.world_size.set(blank_values.world_size);
        self.player_minimum.set(blank_values.player_minimum);
        self.player_cap.set(blank_values.player_cap);
        self.team_count.set(blank_values.team_count);
        self.leaderboard_length.set(blank_values.leaderboard_length);
        self.game_mode_index.set(BLANK_GAME_MODE_INDEX);
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

impl CreateMenuValues {
    pub fn blank() -> CreateMenuValues {
        CreateMenuValues {
            title: String::new(),
            password: String::new(),
            world_shape: WORLD_SHAPE_KINDS[BLANK_WORLD_SHAPE_INDEX],
            world_size: None,
            player_minimum: None,
            player_cap: None,
            team_count: None,
            leaderboard_length: None,
            mode: GAME_MODE_KINDS[BLANK_GAME_MODE_INDEX],
        }
    }
}

#[component]
pub fn CreateMenu() -> impl IntoView {
    let signals: AppSignals = expect_context();
    let fields: CreateMenuFields = expect_context();
    let issues: RwSignal<Vec<MenuIssue>> = RwSignal::new(Vec::new());
    let mode: Signal<GameModeKind> = Signal::derive(move || GAME_MODE_KINDS[fields.game_mode_index.get()]);

    let rows: Vec<MenuRow> = create_rows(fields, mode);

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

fn create_rows(fields: CreateMenuFields, mode: Signal<GameModeKind>) -> Vec<MenuRow> {
    vec![
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
            create_number_input(fields.world_size, DEFAULT_WORLD_SIZE_PIXELS),
        ),
        MenuRow::shown(
            MenuFieldKind::WorldHeight,
            "World Height",
            create_number_input(fields.world_size, DEFAULT_WORLD_SIZE_PIXELS),
        ),
        MenuRow::shown(
            MenuFieldKind::PlayerMinimum,
            "Player Minimum",
            create_number_input(fields.player_minimum, u32::from(DEFAULT_PLAYER_MINIMUM)),
        )
        .with_visibility(Signal::derive(move || get_player_minimum_visibility(mode.get()))),
        MenuRow::shown(
            MenuFieldKind::PlayerCap,
            "Player Cap",
            create_number_input(fields.player_cap, u32::from(DEFAULT_PLAYER_CAP)),
        ),
        MenuRow::shown(
            MenuFieldKind::TeamCount,
            "Team Count",
            create_number_input(fields.team_count, u32::from(DEFAULT_TEAM_COUNT)),
        )
        .with_visibility(Signal::derive(move || get_team_count_visibility(mode.get()))),
        MenuRow::shown(
            MenuFieldKind::LeaderboardLength,
            "Leaderboard Length",
            create_number_input(fields.leaderboard_length, u32::from(DEFAULT_LEADERBOARD_LENGTH)),
        )
        .with_visibility(Signal::derive(move || get_leaderboard_length_visibility(mode.get()))),
        MenuRow::shown(
            MenuFieldKind::GameMode,
            "Game Mode",
            create_list_input(
                fields.game_mode_index,
                &GAME_MODE_KINDS.map(game_browser::get_mode_label),
            ),
        ),
    ]
}

fn get_player_minimum_visibility(mode: GameModeKind) -> FieldVisibilityKind {
    match mode {
        GameModeKind::Survival => FieldVisibilityKind::Shown,
        GameModeKind::FreeForAll | GameModeKind::Skirmish => FieldVisibilityKind::Hidden,
    }
}

fn get_team_count_visibility(mode: GameModeKind) -> FieldVisibilityKind {
    match mode {
        GameModeKind::Skirmish => FieldVisibilityKind::Shown,
        GameModeKind::FreeForAll | GameModeKind::Survival => FieldVisibilityKind::Hidden,
    }
}

fn get_leaderboard_length_visibility(mode: GameModeKind) -> FieldVisibilityKind {
    match mode {
        GameModeKind::FreeForAll | GameModeKind::Survival => FieldVisibilityKind::Shown,
        GameModeKind::Skirmish => FieldVisibilityKind::Hidden,
    }
}

/// Blank fields take their defaults; fields the mode hides are left out or take their defaults.
pub fn resolve_game_settings(values: &CreateMenuValues) -> GameSettingsSerialIn {
    let player_minimum: Option<u8> = match get_player_minimum_visibility(values.mode) {
        FieldVisibilityKind::Shown => Some(get_saturated(values.player_minimum, DEFAULT_PLAYER_MINIMUM, u8::MAX)),
        FieldVisibilityKind::Hidden => None,
    };
    let team_count: Option<u8> = match get_team_count_visibility(values.mode) {
        FieldVisibilityKind::Shown => Some(get_saturated(values.team_count, DEFAULT_TEAM_COUNT, u8::MAX)),
        FieldVisibilityKind::Hidden => None,
    };
    let leaderboard_length: u8 = match get_leaderboard_length_visibility(values.mode) {
        FieldVisibilityKind::Shown => get_saturated(values.leaderboard_length, DEFAULT_LEADERBOARD_LENGTH, u8::MAX),
        FieldVisibilityKind::Hidden => DEFAULT_LEADERBOARD_LENGTH,
    };

    GameSettingsSerialIn {
        title: values.title.clone(),
        mode: GameModeKindSerial::from(&values.mode),
        world_shape: WorldShapeKindSerial::from(&values.world_shape),
        world_size_pixels: get_saturated(values.world_size, DEFAULT_WORLD_SIZE_PIXELS, u32::MAX),
        player_minimum,
        player_cap: get_saturated(values.player_cap, DEFAULT_PLAYER_CAP, u8::MAX),
        team_count,
        leaderboard_length,
    }
}

/// The settings pass the same validation as the server's.
pub fn validate_create_menu(values: &CreateMenuValues) -> Result<JoinMenuOrigin, Vec<MenuIssue>> {
    let settings: GameSettingsSerialIn = resolve_game_settings(values);
    let validated_settings: Result<GameSettings, RejectionKind> = GameSettings::try_from(settings.clone());
    let normalized_password: Result<Option<String>, RejectionKind> =
        protocol_limits::normalize_password(Some(values.password.clone()));

    let (_, password): (GameSettings, Option<String>) =
        menu_form::combine_rejection_results(validated_settings, normalized_password)?;

    Ok(JoinMenuOrigin::Create { settings, password })
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

fn create_number_input(value: RwSignal<Option<i64>>, placeholder: u32) -> MenuInput {
    MenuInput::Number { value, placeholder }
}

/// Out-of-range numbers keep their side of the range, so validation names the right bound.
fn get_saturated<T: TryFrom<i64> + Into<i64> + Copy>(value: Option<i64>, default: T, max: T) -> T {
    value.map_or(default, |value| T::try_from(value.clamp(0, max.into())).unwrap_or(max))
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
