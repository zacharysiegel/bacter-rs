use leptos::prelude::*;

use shared::ability::Loadout;
use shared::game::{GameId, GameModeKind, GameSummary};
use shared::member;
use shared::protocol::protocol_limits;
use shared::protocol::{JoinerSerialIn, LoadoutSerial, MessageSerialIn, RejectionKind, RequestKind};

use crate::app::AppSignals;
use crate::canvas::DriverCommandKind;
use crate::canvas::game_canvas;
use crate::menu::loadout_fields;
use crate::menu::menu_form;
use crate::menu::{
    FieldVisibilityKind, LoadoutFieldValues, LoadoutFields, MenuFieldKind, MenuForm, MenuInput, MenuIssue, MenuRow,
    TeamSize,
};
use crate::screen::JoinMenuOrigin;

/// What the join step knows of the game it joins or creates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinTarget {
    pub mode: GameModeKind,
    /// Shown only for a secured listed game; the create flow carries the create menu's password.
    pub password_visibility: FieldVisibilityKind,
    pub team_sizes: Vec<TeamSize>,
}

impl JoinTarget {
    /// `None` for a listed game which is no longer listed.
    pub fn from_origin(origin: &JoinMenuOrigin, game_summaries: &[GameSummary]) -> Option<JoinTarget> {
        match origin {
            JoinMenuOrigin::Create { settings, .. } => Some(JoinTarget {
                mode: GameModeKind::from(settings.mode),
                password_visibility: FieldVisibilityKind::Hidden,
                team_sizes: get_empty_team_sizes(settings.team_count.unwrap_or(0)),
            }),
            JoinMenuOrigin::Browser { game_id } => {
                let game_summary: &GameSummary = find_game_summary(game_summaries, *game_id)?;

                Some(JoinTarget {
                    mode: game_summary.mode,
                    password_visibility: get_password_visibility(game_summary),
                    team_sizes: loadout_fields::get_summary_team_sizes(&game_summary.team_sizes),
                })
            }
        }
    }
}

#[component]
pub fn JoinMenu(origin: JoinMenuOrigin) -> impl IntoView {
    let signals: AppSignals = expect_context();
    let issues: RwSignal<Vec<MenuIssue>> = RwSignal::new(Vec::new());
    let screen_name: RwSignal<String> = RwSignal::new(String::new());
    let password: RwSignal<String> = RwSignal::new(String::new());

    let target_origin: JoinMenuOrigin = origin.clone();
    let target: Memo<Option<JoinTarget>> = Memo::new(move |_| {
        signals
            .game_list
            .with(|game_list| JoinTarget::from_origin(&target_origin, &game_list.game_summaries))
    });
    let mode: Signal<GameModeKind> =
        Signal::derive(move || target.get().map_or(GameModeKind::FreeForAll, |target| target.mode));
    let team_sizes: Signal<Vec<TeamSize>> =
        Signal::derive(move || target.get().map(|target| target.team_sizes).unwrap_or_default());
    let password_visibility: Signal<FieldVisibilityKind> =
        Signal::derive(move || target.get().map_or(FieldVisibilityKind::Hidden, |target| target.password_visibility));

    let initial_values: LoadoutFieldValues = LoadoutFieldValues::join_defaults(&team_sizes.get_untracked());
    let loadout_fields: LoadoutFields = LoadoutFields::new(&initial_values, team_sizes);

    let rows: Vec<MenuRow> = create_rows(
        screen_name,
        password,
        password_visibility,
        loadout_fields,
        mode,
        team_sizes,
    );

    menu_form::show_request_rejections(
        signals.request_rejection,
        &[RequestKind::CreateGame, RequestKind::JoinGame],
        issues,
    );
    show_unlisted_game(target, issues);

    let on_submit: Callback<()> = Callback::new(move |_| {
        let values: LoadoutFieldValues = loadout_fields.get_values();
        let built_request: Result<MessageSerialIn, Vec<MenuIssue>> = build_join_request(
            &origin,
            &screen_name.get_untracked(),
            &password.get_untracked(),
            &values,
        );

        match built_request {
            Ok(request) => {
                issues.set(Vec::new());
                signals.team_choice.set(values.to_team_choice());
                game_canvas::dispatch_driver_command(DriverCommandKind::SendRequest(request));
            }
            Err(found_issues) => issues.set(found_issues),
        }
    });
    let on_back: Callback<()> = Callback::new(move |_| signals.go_back());

    view! {
        <MenuForm
            header="Join Game Options"
            submit_label="Join"
            rows=rows
            issues=issues
            on_submit=on_submit
            on_back=on_back
        />
    }
}

fn create_rows(
    screen_name: RwSignal<String>,
    password: RwSignal<String>,
    password_visibility: Signal<FieldVisibilityKind>,
    loadout_fields: LoadoutFields,
    mode: Signal<GameModeKind>,
    team_sizes: Signal<Vec<TeamSize>>,
) -> Vec<MenuRow> {
    let mut rows: Vec<MenuRow> = vec![
        MenuRow::shown(
            MenuFieldKind::ScreenName,
            "Screen Name",
            MenuInput::Text { value: screen_name },
        ),
        MenuRow::shown(MenuFieldKind::Password, "Password", MenuInput::Text { value: password })
            .with_visibility(password_visibility),
    ];

    rows.extend(loadout_fields::create_appearance_rows(loadout_fields, mode));
    rows.extend(loadout_fields::create_ability_rows(loadout_fields));
    rows.extend(loadout_fields::create_team_rows(loadout_fields, mode, team_sizes));

    rows
}

fn show_unlisted_game(target: Memo<Option<JoinTarget>>, issues: RwSignal<Vec<MenuIssue>>) {
    Effect::new(move |_| {
        if target.get().is_none() {
            issues.set(vec![MenuIssue::from_rejection(RejectionKind::GameNotFound)]);
        }
    });
}

/// `CreateGame` from the create flow, `JoinGame` from the browser.
pub fn build_join_request(
    origin: &JoinMenuOrigin,
    screen_name: &str,
    password: &str,
    values: &LoadoutFieldValues,
) -> Result<MessageSerialIn, Vec<MenuIssue>> {
    let credentials: Result<(String, Option<String>), Vec<MenuIssue>> = normalize_credentials(screen_name, password);
    let loadout: Result<Loadout, Vec<MenuIssue>> = values.to_loadout();
    let ((screen_name, password), loadout): ((String, Option<String>), Loadout) =
        menu_form::combine_issue_results(credentials, loadout)?;

    let joiner: JoinerSerialIn = JoinerSerialIn {
        screen_name,
        loadout: LoadoutSerial::from(&loadout),
        team: values.to_team_choice(),
    };

    Ok(create_join_request(origin, password, joiner))
}

/// The trimmed screen name and the password, or an issue for each one which is invalid.
pub fn normalize_credentials(screen_name: &str, password: &str) -> Result<(String, Option<String>), Vec<MenuIssue>> {
    let normalized_screen_name: Result<String, RejectionKind> = protocol_limits::normalize_screen_name(screen_name);
    let normalized_password: Result<Option<String>, RejectionKind> =
        protocol_limits::normalize_password(Some(String::from(password)));

    menu_form::combine_rejection_results(normalized_screen_name, normalized_password)
}

pub fn find_game_summary(game_summaries: &[GameSummary], game_id: GameId) -> Option<&GameSummary> {
    game_summaries.iter().find(|game_summary| game_summary.game_id == game_id)
}

pub fn get_password_visibility(game_summary: &GameSummary) -> FieldVisibilityKind {
    if game_summary.secured {
        FieldVisibilityKind::Shown
    } else {
        FieldVisibilityKind::Hidden
    }
}

/// `password` is the join menu's; the create flow carries the create menu's.
fn create_join_request(origin: &JoinMenuOrigin, password: Option<String>, joiner: JoinerSerialIn) -> MessageSerialIn {
    match origin {
        JoinMenuOrigin::Create {
            settings,
            password: created_password,
        } => MessageSerialIn::CreateGame {
            settings: settings.clone(),
            password: created_password.clone(),
            joiner,
        },
        JoinMenuOrigin::Browser { game_id } => MessageSerialIn::JoinGame {
            game_id: game_id.0,
            password,
            joiner,
        },
    }
}

fn get_empty_team_sizes(team_count: u8) -> Vec<TeamSize> {
    member::get_game_teams(team_count)
        .into_iter()
        .map(|team| TeamSize {
            team,
            participant_count: 0,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::ability::{FirstAbilityKind, SecondAbilityKind, ThirdAbilityKind};
    use shared::member::{OrganismColorKind, SkinKind, TeamKind};
    use shared::protocol::{
        GameModeKindSerial, GameSettingsSerialIn, TeamChoiceKindSerialIn, TeamKindSerial, WorldShapeKindSerial,
    };

    use crate::menu::AutoAssignKind;

    const GAME_ID: GameId = GameId(3);

    fn create_values() -> LoadoutFieldValues {
        LoadoutFieldValues {
            color: OrganismColorKind::Sun,
            skin: SkinKind::None,
            first: Some(FirstAbilityKind::Extend),
            second: Some(SecondAbilityKind::Freeze),
            third: Some(ThirdAbilityKind::Neutralize),
            team: Some(TeamKind::Red),
            auto_assign: AutoAssignKind::Unselected,
        }
    }

    fn create_settings() -> GameSettingsSerialIn {
        GameSettingsSerialIn {
            title: String::from("Petri"),
            mode: GameModeKindSerial::Skirmish,
            world_shape: WorldShapeKindSerial::Ellipse,
            world_size_pixels: 900,
            player_minimum: None,
            player_cap: 8,
            team_count: Some(3),
            leaderboard_length: 10,
        }
    }

    fn create_game_summary() -> GameSummary {
        GameSummary {
            game_id: GAME_ID,
            title: String::from("Agar"),
            mode: GameModeKind::Skirmish,
            player_count: 3,
            spectator_count: 1,
            player_cap: 8,
            secured: true,
            team_sizes: vec![2, 1],
        }
    }

    fn create_expected_joiner(screen_name: &str) -> JoinerSerialIn {
        JoinerSerialIn {
            screen_name: String::from(screen_name),
            loadout: LoadoutSerial::from(&create_values().to_loadout().unwrap()),
            team: TeamChoiceKindSerialIn::Team(TeamKindSerial::Red),
        }
    }

    #[test]
    fn from_origin_describes_a_game_being_created() {
        let origin: JoinMenuOrigin = JoinMenuOrigin::Create {
            settings: create_settings(),
            password: Some(String::from("hunter2")),
        };

        let target: JoinTarget = JoinTarget::from_origin(&origin, &[]).unwrap();

        assert_eq!(target.mode, GameModeKind::Skirmish);
        assert_eq!(target.password_visibility, FieldVisibilityKind::Hidden);
        assert_eq!(
            target.team_sizes,
            vec![
                TeamSize {
                    team: TeamKind::Red,
                    participant_count: 0,
                },
                TeamSize {
                    team: TeamKind::Blue,
                    participant_count: 0,
                },
                TeamSize {
                    team: TeamKind::Green,
                    participant_count: 0,
                },
            ],
        );
    }

    #[test]
    fn from_origin_describes_a_listed_game_from_its_summary() {
        let origin: JoinMenuOrigin = JoinMenuOrigin::Browser { game_id: GAME_ID };

        let target: JoinTarget = JoinTarget::from_origin(&origin, &[create_game_summary()]).unwrap();

        assert_eq!(target.mode, GameModeKind::Skirmish);
        assert_eq!(target.password_visibility, FieldVisibilityKind::Shown);
        assert_eq!(
            target.team_sizes,
            vec![
                TeamSize {
                    team: TeamKind::Red,
                    participant_count: 2,
                },
                TeamSize {
                    team: TeamKind::Blue,
                    participant_count: 1,
                },
            ],
        );
    }

    #[test]
    fn from_origin_hides_the_password_of_an_unsecured_listed_game() {
        let origin: JoinMenuOrigin = JoinMenuOrigin::Browser { game_id: GAME_ID };
        let mut game_summary: GameSummary = create_game_summary();
        game_summary.secured = false;

        let target: JoinTarget = JoinTarget::from_origin(&origin, &[game_summary]).unwrap();

        assert_eq!(target.password_visibility, FieldVisibilityKind::Hidden);
    }

    #[test]
    fn from_origin_is_none_for_a_game_no_longer_listed() {
        let origin: JoinMenuOrigin = JoinMenuOrigin::Browser { game_id: GameId(4) };

        assert_eq!(JoinTarget::from_origin(&origin, &[create_game_summary()]), None);
    }

    #[test]
    fn build_join_request_creates_the_game_with_the_create_menu_password() {
        let origin: JoinMenuOrigin = JoinMenuOrigin::Create {
            settings: create_settings(),
            password: Some(String::from("hunter2")),
        };

        assert_eq!(
            build_join_request(&origin, "  Blob ", "", &create_values()),
            Ok(MessageSerialIn::CreateGame {
                settings: create_settings(),
                password: Some(String::from("hunter2")),
                joiner: create_expected_joiner("Blob"),
            }),
        );
    }

    #[test]
    fn build_join_request_joins_a_listed_game_with_the_entered_password() {
        let origin: JoinMenuOrigin = JoinMenuOrigin::Browser { game_id: GAME_ID };

        assert_eq!(
            build_join_request(&origin, "Blob", "swordfish", &create_values()),
            Ok(MessageSerialIn::JoinGame {
                game_id: GAME_ID.0,
                password: Some(String::from("swordfish")),
                joiner: create_expected_joiner("Blob"),
            }),
        );
    }

    #[test]
    fn build_join_request_reports_the_screen_name_and_every_missing_ability() {
        let origin: JoinMenuOrigin = JoinMenuOrigin::Browser { game_id: GAME_ID };
        let values: LoadoutFieldValues = LoadoutFieldValues::join_defaults(&[]);

        assert_eq!(
            build_join_request(&origin, " ", "", &values),
            Err(vec![
                MenuIssue::at_field(MenuFieldKind::ScreenName, "Screen name cannot be left empty"),
                MenuIssue::at_field(MenuFieldKind::FirstAbility, "Select a first ability"),
                MenuIssue::at_field(MenuFieldKind::SecondAbility, "Select a second ability"),
                MenuIssue::at_field(MenuFieldKind::ThirdAbility, "Select a third ability"),
            ]),
        );
    }

    #[test]
    fn build_join_request_reports_a_password_over_the_limit() {
        let origin: JoinMenuOrigin = JoinMenuOrigin::Browser { game_id: GAME_ID };
        let long_password: String = "p".repeat(protocol_limits::PASSWORD_BYTE_LIMIT + 1);

        assert_eq!(
            build_join_request(&origin, "Blob", &long_password, &create_values()),
            Err(vec![MenuIssue::at_field(
                MenuFieldKind::Password,
                &RejectionKind::PasswordTooLong.message_text(),
            )]),
        );
    }

    #[test]
    fn find_game_summary_finds_the_listed_game_by_id() {
        let game_summaries: Vec<GameSummary> = vec![create_game_summary()];

        assert_eq!(find_game_summary(&game_summaries, GAME_ID), Some(&game_summaries[0]));
        assert_eq!(find_game_summary(&game_summaries, GameId(9)), None);
    }
}
