use leptos::ev;
use leptos::prelude::*;

use shared::protocol::{RejectionKind, RequestKind, SettingFieldKind};

const ENTER_KEY: &str = "Enter";
pub const BACK_TEXT: &str = "← Back";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuFieldKind {
    GameTitle,
    Password,
    WorldShape,
    WorldWidth,
    WorldHeight,
    PlayerMinimum,
    PlayerCap,
    TeamCount,
    LeaderboardLength,
    GameMode,
    ScreenName,
    Color,
    Skin,
    FirstAbility,
    SecondAbility,
    ThirdAbility,
    FourthAbility,
    Team,
    AutoAssign,
    NameLabels,
    Messages,
    LeaveGame,
    LeaveTutorial,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuIssue {
    /// `None` is shown in the bottom row.
    pub field: Option<MenuFieldKind>,
    pub text: String,
}

impl MenuIssue {
    pub fn at_field(field: MenuFieldKind, text: &str) -> MenuIssue {
        MenuIssue {
            field: Some(field),
            text: String::from(text),
        }
    }

    pub fn from_rejection(reason: RejectionKind) -> MenuIssue {
        MenuIssue {
            field: get_rejection_field(reason),
            text: reason.message_text(),
        }
    }
}

/// The server's answer to the last request it refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestRejection {
    pub request: RequestKind,
    pub reason: RejectionKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacedIssues {
    pub field_issues: Vec<(MenuFieldKind, String)>,
    pub general_issues: Vec<String>,
}

impl PlacedIssues {
    pub fn get_field_texts(&self, field: MenuFieldKind) -> Vec<String> {
        self.field_issues
            .iter()
            .filter(|(issue_field, _)| *issue_field == field)
            .map(|(_, text)| text.clone())
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldVisibilityKind {
    Shown,
    Hidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldAvailabilityKind {
    Enabled,
    Disabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadioSelectionKind {
    Deselectable,
    /// Clicks never change the selection.
    Fixed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListOption {
    pub label: String,
    /// A CSS colour shown behind the label.
    pub swatch: Option<String>,
}

#[derive(Clone)]
pub enum MenuInput {
    Text {
        value: RwSignal<String>,
    },
    /// `None` is blank and shows the placeholder.
    Number {
        value: RwSignal<Option<i64>>,
        placeholder: u32,
    },
    List {
        selected_index: RwSignal<usize>,
        options: Signal<Vec<ListOption>>,
        availability: Signal<FieldAvailabilityKind>,
    },
    Radios {
        labels: Vec<&'static str>,
        selected_index: RwSignal<Option<usize>>,
        selection: RadioSelectionKind,
    },
    Button {
        on_click: Callback<()>,
    },
}

#[derive(Clone)]
pub struct MenuRow {
    pub field: MenuFieldKind,
    pub label: &'static str,
    pub input: MenuInput,
    pub visibility: Signal<FieldVisibilityKind>,
}

impl MenuRow {
    pub fn shown(field: MenuFieldKind, label: &'static str, input: MenuInput) -> MenuRow {
        MenuRow {
            field,
            label,
            input,
            visibility: Signal::stored(FieldVisibilityKind::Shown),
        }
    }

    pub fn with_visibility(self, visibility: Signal<FieldVisibilityKind>) -> MenuRow {
        MenuRow {
            field: self.field,
            label: self.label,
            input: self.input,
            visibility,
        }
    }
}

/// Enter in a field submits.
#[component]
pub fn MenuForm(
    header: &'static str,
    submit_label: &'static str,
    rows: Vec<MenuRow>,
    issues: RwSignal<Vec<MenuIssue>>,
    on_submit: Callback<()>,
    on_back: Callback<()>,
) -> impl IntoView {
    let row_visibilities: Vec<(MenuFieldKind, Signal<FieldVisibilityKind>)> =
        rows.iter().map(|row| (row.field, row.visibility)).collect();
    let placed_issues: Memo<PlacedIssues> = Memo::new(move |_| {
        let shown_fields: Vec<MenuFieldKind> = row_visibilities
            .iter()
            .filter(|(_, visibility)| visibility.get() == FieldVisibilityKind::Shown)
            .map(|(field, _)| *field)
            .collect();

        place_issues(&issues.get(), &shown_fields)
    });
    let row_views: Vec<AnyView> = rows.into_iter().map(|row| render_row(row, placed_issues, on_submit)).collect();

    view! {
        <div class="menu">
            <div class="menu-header">
                <h2 class="menu-header-text">{header}</h2>
            </div>
            <div class="menu-content">
                <table class="menu-table">
                    <tbody>
                        {row_views}
                        <Show when=move || !placed_issues.get().general_issues.is_empty()>
                            <tr class="menu-row">
                                <td class="menu-cell menu-cell-label"></td>
                                <td class="menu-cell menu-cell-input">
                                    {move || render_issue_texts(placed_issues.get().general_issues)}
                                </td>
                            </tr>
                        </Show>
                    </tbody>
                </table>
                <button class="menu-submit" type="button" on:click=move |_| on_submit.run(())>
                    {submit_label}
                </button>
            </div>
            <footer class="menu-footer" on:click=move |_| on_back.run(())>
                <p class="menu-footer-text">{BACK_TEXT}</p>
            </footer>
        </div>
    }
}

/// Issues for fields not shown join the general ones, in order.
pub fn place_issues(issues: &[MenuIssue], shown_fields: &[MenuFieldKind]) -> PlacedIssues {
    let mut field_issues: Vec<(MenuFieldKind, String)> = Vec::new();
    let mut general_issues: Vec<String> = Vec::new();

    for issue in issues {
        match issue.field {
            Some(field) if shown_fields.contains(&field) => field_issues.push((field, issue.text.clone())),
            Some(_) | None => general_issues.push(issue.text.clone()),
        }
    }

    PlacedIssues {
        field_issues,
        general_issues,
    }
}

/// `None` for a rejection shown in the bottom row.
pub fn get_rejection_field(reason: RejectionKind) -> Option<MenuFieldKind> {
    match reason {
        RejectionKind::ScreenNameEmpty
        | RejectionKind::ScreenNameTooLong
        | RejectionKind::ScreenNameInvalidCharacter
        | RejectionKind::ScreenNameTaken => Some(MenuFieldKind::ScreenName),
        RejectionKind::TitleEmpty
        | RejectionKind::TitleTooLong
        | RejectionKind::TitleInvalidCharacter
        | RejectionKind::TitleTaken => Some(MenuFieldKind::GameTitle),
        RejectionKind::SettingOutOfRange { field, .. } => Some(get_setting_field(field)),
        RejectionKind::PlayerCapBelowMinimum | RejectionKind::GameFull => Some(MenuFieldKind::PlayerCap),
        RejectionKind::PlayerCapBelowTeamCount => Some(MenuFieldKind::TeamCount),
        RejectionKind::PasswordRequired | RejectionKind::PasswordIncorrect | RejectionKind::PasswordTooLong => {
            Some(MenuFieldKind::Password)
        }
        RejectionKind::TeamUnbalanced { .. } => Some(MenuFieldKind::AutoAssign),
        RejectionKind::SettingNotApplicable { .. }
        | RejectionKind::GameNotFound
        | RejectionKind::SpectatorLimitReached
        | RejectionKind::RoundInProgress
        | RejectionKind::AlreadyInGame
        | RejectionKind::NotInGame
        | RejectionKind::NotDead
        | RejectionKind::ServerFull
        | RejectionKind::ServerBusy
        | RejectionKind::RateLimited => None,
    }
}

/// Floored like the original's inputs; 0 and anything unreadable are blank.
pub fn parse_number_input(input_text: &str) -> Option<i64> {
    let parsed: Option<f64> = input_text.trim().parse::<f64>().ok();
    let floored: f64 = parsed.filter(|value| value.is_finite())?.floor();

    if floored == 0.0 {
        return None;
    }

    Some(floored as i64)
}

/// Clicking the selected radio of a deselectable group clears it.
pub fn get_toggled_radio(
    selected_index: Option<usize>,
    clicked_index: usize,
    selection: RadioSelectionKind,
) -> Option<usize> {
    match selection {
        RadioSelectionKind::Fixed => selected_index,
        RadioSelectionKind::Deselectable if selected_index == Some(clicked_index) => None,
        RadioSelectionKind::Deselectable => Some(clicked_index),
    }
}

fn get_setting_field(setting_field: SettingFieldKind) -> MenuFieldKind {
    match setting_field {
        SettingFieldKind::WorldSize => MenuFieldKind::WorldWidth,
        SettingFieldKind::PlayerMinimum => MenuFieldKind::PlayerMinimum,
        SettingFieldKind::PlayerCap => MenuFieldKind::PlayerCap,
        SettingFieldKind::TeamCount => MenuFieldKind::TeamCount,
        SettingFieldKind::LeaderboardLength => MenuFieldKind::LeaderboardLength,
    }
}

fn create_submit_on_enter(on_submit: Callback<()>) -> impl Fn(ev::KeyboardEvent) + Copy + 'static {
    move |event: ev::KeyboardEvent| {
        if event.key() == ENTER_KEY {
            event.prevent_default();
            on_submit.run(());
        }
    }
}

fn render_row(row: MenuRow, placed_issues: Memo<PlacedIssues>, on_submit: Callback<()>) -> AnyView {
    let MenuRow {
        field,
        label,
        input,
        visibility,
    } = row;

    view! {
        <Show when=move || visibility.get() == FieldVisibilityKind::Shown>
            <tr class="menu-row">
                <td class="menu-cell menu-cell-label">{label}</td>
                <td class="menu-cell menu-cell-input">
                    {render_input(input.clone(), on_submit)}
                    {move || render_issue_texts(placed_issues.get().get_field_texts(field))}
                </td>
            </tr>
        </Show>
    }
    .into_any()
}

fn render_issue_texts(issue_texts: Vec<String>) -> AnyView {
    issue_texts
        .into_iter()
        .map(|issue_text| view! { <p class="menu-issue">{issue_text}</p> })
        .collect_view()
        .into_any()
}

fn render_input(input: MenuInput, on_submit: Callback<()>) -> AnyView {
    match input {
        MenuInput::Text { value } => render_text_input(value, on_submit),
        MenuInput::Number { value, placeholder } => render_number_input(value, placeholder, on_submit),
        MenuInput::List {
            selected_index,
            options,
            availability,
        } => render_list_input(selected_index, options, availability, on_submit),
        MenuInput::Radios {
            labels,
            selected_index,
            selection,
        } => render_radios_input(labels, selected_index, selection, on_submit),
        MenuInput::Button { on_click } => render_button_input(on_click),
    }
}

fn render_text_input(value: RwSignal<String>, on_submit: Callback<()>) -> AnyView {
    view! {
        <input
            class="menu-input"
            type="text"
            autocomplete="off"
            prop:value=move || value.get()
            on:input=move |event| value.set(event_target_value(&event))
            on:keydown=create_submit_on_enter(on_submit)
        />
    }
    .into_any()
}

fn render_number_input(value: RwSignal<Option<i64>>, placeholder: u32, on_submit: Callback<()>) -> AnyView {
    view! {
        <input
            class="menu-input"
            type="number"
            autocomplete="off"
            placeholder=placeholder.to_string()
            prop:value=move || value.get().map(|number| number.to_string()).unwrap_or_default()
            on:input=move |event| value.set(parse_number_input(&event_target_value(&event)))
            on:keydown=create_submit_on_enter(on_submit)
        />
    }
    .into_any()
}

fn render_list_input(
    selected_index: RwSignal<usize>,
    options: Signal<Vec<ListOption>>,
    availability: Signal<FieldAvailabilityKind>,
    on_submit: Callback<()>,
) -> AnyView {
    let select_option = move |event: ev::Event| {
        let chosen_index: Option<usize> = event_target_value(&event).parse::<usize>().ok();

        if let Some(chosen_index) = chosen_index {
            selected_index.set(chosen_index);
        }
    };
    let option_views = move || {
        options
            .get()
            .into_iter()
            .enumerate()
            .map(|(index, list_option)| {
                let swatch_style: String = list_option
                    .swatch
                    .map(|swatch| format!("background-color: {swatch}"))
                    .unwrap_or_default();

                view! {
                    <option value=index.to_string() style=swatch_style prop:selected=move || selected_index.get() == index>
                        {list_option.label}
                    </option>
                }
            })
            .collect_view()
    };

    view! {
        <select
            class="menu-input menu-list"
            prop:disabled=move || availability.get() == FieldAvailabilityKind::Disabled
            on:change=select_option
            on:keydown=create_submit_on_enter(on_submit)
        >
            {option_views}
        </select>
    }
    .into_any()
}

fn render_radios_input(
    labels: Vec<&'static str>,
    selected_index: RwSignal<Option<usize>>,
    selection: RadioSelectionKind,
    on_submit: Callback<()>,
) -> AnyView {
    let radio_views: Vec<AnyView> = labels
        .into_iter()
        .enumerate()
        .map(|(index, label)| {
            let toggle_radio = move |_| {
                selected_index.update(|selected| *selected = get_toggled_radio(*selected, index, selection));
            };

            view! {
                <div class="menu-radio-option" on:keydown=create_submit_on_enter(on_submit)>
                    <div
                        class="menu-radio"
                        class:menu-radio-selected=move || selected_index.get() == Some(index)
                        on:click=toggle_radio
                    ></div>
                    <p class="menu-radio-text">{label}</p>
                </div>
            }
            .into_any()
        })
        .collect();

    view! { <div class="menu-radios">{radio_views}</div> }.into_any()
}

fn render_button_input(on_click: Callback<()>) -> AnyView {
    view! { <button class="menu-button" type="button" on:click=move |_| on_click.run(())></button> }.into_any()
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::member::TeamKind;
    use shared::protocol::RangeBoundKind;

    #[test]
    fn place_issues_puts_an_issue_under_its_shown_field() {
        let issues: Vec<MenuIssue> = vec![MenuIssue::at_field(
            MenuFieldKind::ScreenName,
            "Screen name cannot be left empty",
        )];

        let placed_issues: PlacedIssues = place_issues(&issues, &[MenuFieldKind::ScreenName]);

        assert_eq!(
            placed_issues.get_field_texts(MenuFieldKind::ScreenName),
            vec![String::from("Screen name cannot be left empty")],
        );
        assert!(placed_issues.general_issues.is_empty());
    }

    #[test]
    fn place_issues_moves_issues_of_hidden_or_no_fields_to_the_general_row() {
        let issues: Vec<MenuIssue> = vec![
            MenuIssue::at_field(MenuFieldKind::PlayerCap, "Game is at maximum player capacity"),
            MenuIssue {
                field: None,
                text: String::from("The game has closed"),
            },
        ];

        let placed_issues: PlacedIssues = place_issues(&issues, &[MenuFieldKind::ScreenName]);

        assert!(placed_issues.field_issues.is_empty());
        assert_eq!(
            placed_issues.general_issues,
            vec![
                String::from("Game is at maximum player capacity"),
                String::from("The game has closed"),
            ],
        );
    }

    #[test]
    fn get_rejection_field_attaches_rejections_to_their_form_fields() {
        assert_eq!(
            get_rejection_field(RejectionKind::ScreenNameTaken),
            Some(MenuFieldKind::ScreenName),
        );
        assert_eq!(
            get_rejection_field(RejectionKind::TitleTaken),
            Some(MenuFieldKind::GameTitle),
        );
        assert_eq!(
            get_rejection_field(RejectionKind::SettingOutOfRange {
                field: SettingFieldKind::WorldSize,
                bound: RangeBoundKind::Above,
            }),
            Some(MenuFieldKind::WorldWidth),
        );
        assert_eq!(
            get_rejection_field(RejectionKind::PlayerCapBelowTeamCount),
            Some(MenuFieldKind::TeamCount),
        );
        assert_eq!(
            get_rejection_field(RejectionKind::PasswordIncorrect),
            Some(MenuFieldKind::Password),
        );
        assert_eq!(
            get_rejection_field(RejectionKind::TeamUnbalanced {
                requested: TeamKind::Red,
                smaller: TeamKind::Blue,
            }),
            Some(MenuFieldKind::AutoAssign),
        );
        assert_eq!(get_rejection_field(RejectionKind::GameNotFound), None);
        assert_eq!(get_rejection_field(RejectionKind::RoundInProgress), None);
    }

    #[test]
    fn from_rejection_takes_the_rejection_text() {
        assert_eq!(
            MenuIssue::from_rejection(RejectionKind::PasswordRequired),
            MenuIssue::at_field(MenuFieldKind::Password, "A password is required for this game"),
        );
    }

    #[test]
    fn parse_number_input_floors_and_blanks_zero() {
        assert_eq!(parse_number_input("800"), Some(800));
        assert_eq!(parse_number_input("16.9"), Some(16));
        assert_eq!(parse_number_input("-2.5"), Some(-3));
        assert_eq!(parse_number_input("0"), None);
        assert_eq!(parse_number_input("0.4"), None);
        assert_eq!(parse_number_input(""), None);
        assert_eq!(parse_number_input("e"), None);
    }

    #[test]
    fn get_toggled_radio_selects_one_and_deselects_on_a_second_click() {
        assert_eq!(get_toggled_radio(None, 1, RadioSelectionKind::Deselectable), Some(1));
        assert_eq!(get_toggled_radio(Some(0), 1, RadioSelectionKind::Deselectable), Some(1));
        assert_eq!(get_toggled_radio(Some(1), 1, RadioSelectionKind::Deselectable), None);
    }

    #[test]
    fn get_toggled_radio_keeps_a_fixed_selection() {
        assert_eq!(get_toggled_radio(Some(0), 0, RadioSelectionKind::Fixed), Some(0));
    }
}
