use leptos::prelude::*;

use shared::ability::{FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use shared::game::{GameModeKind, GameState};
use shared::member;
use shared::member::{Appearance, OrganismColorKind, SkinKind, TeamKind, TeamSizes};
use shared::protocol::{MessageSerialIn, TeamChoiceKindSerialIn, TeamKindSerial};
use shared::render::RenderColor;
use shared::render::render_color;

use crate::canvas::DriverCommandKind;
use crate::canvas::game_canvas;
use crate::menu::{
    FieldAvailabilityKind, FieldVisibilityKind, ListOption, MenuFieldKind, MenuInput, MenuIssue, MenuRow,
    RadioSelectionKind,
};

/// No radio selected means `SkinKind::None`.
pub const SKIN_RADIO_KINDS: [SkinKind; 3] = [SkinKind::Grid, SkinKind::Circles, SkinKind::Ghost];
pub const SKIN_RADIO_LABELS: [&str; 3] = ["Grid", "Circles", "Ghost"];
pub const FIRST_ABILITY_KINDS: [FirstAbilityKind; 2] = [FirstAbilityKind::Extend, FirstAbilityKind::Compress];
pub const FIRST_ABILITY_LABELS: [&str; 2] = ["Extend", "Compress"];
pub const SECOND_ABILITY_KINDS: [SecondAbilityKind; 2] = [SecondAbilityKind::Immortality, SecondAbilityKind::Freeze];
pub const SECOND_ABILITY_LABELS: [&str; 2] = ["Immortality", "Freeze"];
pub const THIRD_ABILITY_KINDS: [ThirdAbilityKind; 2] = [ThirdAbilityKind::Neutralize, ThirdAbilityKind::Toxin];
pub const THIRD_ABILITY_LABELS: [&str; 2] = ["Neutralize", "Toxin"];
pub const FOURTH_ABILITY_LABELS: [&str; 1] = ["Spore"];
/// A single unlabelled radio.
pub const SINGLE_RADIO_LABELS: [&str; 1] = [""];
const FIRST_ABILITY_MISSING_TEXT: &str = "Select a first ability";
const SECOND_ABILITY_MISSING_TEXT: &str = "Select a second ability";
const THIRD_ABILITY_MISSING_TEXT: &str = "Select a third ability";

/// Participants on one team.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TeamSize {
    pub team: TeamKind,
    pub participant_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoAssignKind {
    Selected,
    Unselected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoadoutFieldValues {
    pub color: OrganismColorKind,
    pub skin: SkinKind,
    pub first: Option<FirstAbilityKind>,
    pub second: Option<SecondAbilityKind>,
    pub third: Option<ThirdAbilityKind>,
    /// `None` outside skirmish.
    pub team: Option<TeamKind>,
    pub auto_assign: AutoAssignKind,
}

impl LoadoutFieldValues {
    /// The first palette colour, no skin, no ability, the first smallest team.
    pub fn join_defaults(team_sizes: &[TeamSize]) -> LoadoutFieldValues {
        LoadoutFieldValues {
            color: member::ORGANISM_COLOR_ORDER[0],
            skin: SkinKind::None,
            first: None,
            second: None,
            third: None,
            team: get_smallest_team(team_sizes),
            auto_assign: AutoAssignKind::Unselected,
        }
    }

    pub fn from_loadout(loadout: &Loadout, team: Option<TeamKind>, auto_assign: AutoAssignKind) -> LoadoutFieldValues {
        LoadoutFieldValues {
            color: loadout.appearance.color,
            skin: loadout.appearance.skin,
            first: Some(loadout.first),
            second: Some(loadout.second),
            third: Some(loadout.third),
            team,
            auto_assign,
        }
    }

    pub fn appearance(&self) -> Appearance {
        Appearance {
            color: self.color,
            skin: self.skin,
        }
    }

    /// Every unselected ability is an issue at its field.
    pub fn to_loadout(&self) -> Result<Loadout, Vec<MenuIssue>> {
        let mut issues: Vec<MenuIssue> = Vec::new();

        if self.first.is_none() {
            issues.push(MenuIssue::at_field(
                MenuFieldKind::FirstAbility,
                FIRST_ABILITY_MISSING_TEXT,
            ));
        }

        if self.second.is_none() {
            issues.push(MenuIssue::at_field(
                MenuFieldKind::SecondAbility,
                SECOND_ABILITY_MISSING_TEXT,
            ));
        }

        if self.third.is_none() {
            issues.push(MenuIssue::at_field(
                MenuFieldKind::ThirdAbility,
                THIRD_ABILITY_MISSING_TEXT,
            ));
        }

        let (Some(first), Some(second), Some(third)) = (self.first, self.second, self.third) else {
            return Err(issues);
        };

        Ok(Loadout {
            appearance: self.appearance(),
            first,
            second,
            third,
        })
    }

    /// The server ignores the choice outside skirmish.
    pub fn to_team_choice(&self) -> TeamChoiceKindSerialIn {
        match (self.auto_assign, self.team) {
            (AutoAssignKind::Unselected, Some(team)) => TeamChoiceKindSerialIn::Team(TeamKindSerial::from(&team)),
            (AutoAssignKind::Selected, _) | (AutoAssignKind::Unselected, None) => TeamChoiceKindSerialIn::Auto,
        }
    }
}

/// The form signals of a loadout, as indices into the option lists.
#[derive(Clone, Copy, Debug)]
pub struct LoadoutFields {
    pub color_index: RwSignal<usize>,
    pub skin_index: RwSignal<Option<usize>>,
    pub first_index: RwSignal<Option<usize>>,
    pub second_index: RwSignal<Option<usize>>,
    pub third_index: RwSignal<Option<usize>>,
    /// Always the one fourth ability.
    pub fourth_index: RwSignal<Option<usize>>,
    pub team_index: RwSignal<usize>,
    pub auto_assign_index: RwSignal<Option<usize>>,
    /// The teams `team_index` indexes, in team order.
    pub teams: Signal<Vec<TeamKind>>,
}

impl LoadoutFields {
    pub fn new(initial_values: &LoadoutFieldValues, team_sizes: Signal<Vec<TeamSize>>) -> LoadoutFields {
        let teams: Signal<Vec<TeamKind>> =
            Signal::derive(move || team_sizes.get().into_iter().map(|team_size| team_size.team).collect());
        let team_index: usize = initial_values
            .team
            .and_then(|team| teams.get_untracked().iter().position(|game_team| *game_team == team))
            .unwrap_or(0);

        LoadoutFields {
            color_index: RwSignal::new(get_index_of(&member::ORGANISM_COLOR_ORDER, initial_values.color).unwrap_or(0)),
            skin_index: RwSignal::new(get_index_of(&SKIN_RADIO_KINDS, initial_values.skin)),
            first_index: RwSignal::new(
                initial_values.first.and_then(|first| get_index_of(&FIRST_ABILITY_KINDS, first)),
            ),
            second_index: RwSignal::new(
                initial_values.second.and_then(|second| get_index_of(&SECOND_ABILITY_KINDS, second)),
            ),
            third_index: RwSignal::new(
                initial_values.third.and_then(|third| get_index_of(&THIRD_ABILITY_KINDS, third)),
            ),
            fourth_index: RwSignal::new(Some(0)),
            team_index: RwSignal::new(team_index),
            auto_assign_index: RwSignal::new(match initial_values.auto_assign {
                AutoAssignKind::Selected => Some(0),
                AutoAssignKind::Unselected => None,
            }),
            teams,
        }
    }

    pub fn get_values(&self) -> LoadoutFieldValues {
        LoadoutFieldValues {
            color: member::ORGANISM_COLOR_ORDER[self.color_index.get_untracked()],
            skin: self.skin_index.get_untracked().map_or(SkinKind::None, |skin_index| SKIN_RADIO_KINDS[skin_index]),
            first: self.first_index.get_untracked().map(|first_index| FIRST_ABILITY_KINDS[first_index]),
            second: self.second_index.get_untracked().map(|second_index| SECOND_ABILITY_KINDS[second_index]),
            third: self.third_index.get_untracked().map(|third_index| THIRD_ABILITY_KINDS[third_index]),
            team: self.teams.get_untracked().get(self.team_index.get_untracked()).copied(),
            auto_assign: match self.auto_assign_index.get_untracked() {
                Some(_) => AutoAssignKind::Selected,
                None => AutoAssignKind::Unselected,
            },
        }
    }
}

/// Color (hidden in skirmish, where the team gives the colour) and Skin.
pub fn create_appearance_rows(fields: LoadoutFields, mode: Signal<GameModeKind>) -> Vec<MenuRow> {
    vec![
        MenuRow::shown(
            MenuFieldKind::Color,
            "Color",
            MenuInput::List {
                selected_index: fields.color_index,
                options: Signal::stored(get_color_options()),
                availability: Signal::stored(FieldAvailabilityKind::Enabled),
            },
        )
        .with_visibility(Signal::derive(move || match mode.get() {
            GameModeKind::Skirmish => FieldVisibilityKind::Hidden,
            GameModeKind::FreeForAll | GameModeKind::Survival => FieldVisibilityKind::Shown,
        })),
        MenuRow::shown(
            MenuFieldKind::Skin,
            "Skin",
            create_radios_input(&SKIN_RADIO_LABELS, fields.skin_index, RadioSelectionKind::Deselectable),
        ),
    ]
}

/// The first three are required; the fourth is always Spore.
pub fn create_ability_rows(fields: LoadoutFields) -> Vec<MenuRow> {
    vec![
        MenuRow::shown(
            MenuFieldKind::FirstAbility,
            "1st Ability",
            create_radios_input(
                &FIRST_ABILITY_LABELS,
                fields.first_index,
                RadioSelectionKind::Deselectable,
            ),
        ),
        MenuRow::shown(
            MenuFieldKind::SecondAbility,
            "2nd Ability",
            create_radios_input(
                &SECOND_ABILITY_LABELS,
                fields.second_index,
                RadioSelectionKind::Deselectable,
            ),
        ),
        MenuRow::shown(
            MenuFieldKind::ThirdAbility,
            "3rd Ability",
            create_radios_input(
                &THIRD_ABILITY_LABELS,
                fields.third_index,
                RadioSelectionKind::Deselectable,
            ),
        ),
        MenuRow::shown(
            MenuFieldKind::FourthAbility,
            "4th Ability",
            create_radios_input(&FOURTH_ABILITY_LABELS, fields.fourth_index, RadioSelectionKind::Fixed),
        ),
    ]
}

/// Skirmish only; Auto Assign disables the team list.
pub fn create_team_rows(
    fields: LoadoutFields,
    mode: Signal<GameModeKind>,
    team_sizes: Signal<Vec<TeamSize>>,
) -> Vec<MenuRow> {
    let skirmish_visibility: Signal<FieldVisibilityKind> = Signal::derive(move || match mode.get() {
        GameModeKind::Skirmish => FieldVisibilityKind::Shown,
        GameModeKind::FreeForAll | GameModeKind::Survival => FieldVisibilityKind::Hidden,
    });
    let team_availability: Signal<FieldAvailabilityKind> =
        Signal::derive(move || match fields.auto_assign_index.get() {
            Some(_) => FieldAvailabilityKind::Disabled,
            None => FieldAvailabilityKind::Enabled,
        });

    vec![
        MenuRow::shown(
            MenuFieldKind::Team,
            "Team",
            MenuInput::List {
                selected_index: fields.team_index,
                options: Signal::derive(move || get_team_options(&team_sizes.get())),
                availability: team_availability,
            },
        )
        .with_visibility(skirmish_visibility),
        MenuRow::shown(
            MenuFieldKind::AutoAssign,
            "Auto Assign",
            create_radios_input(
                &SINGLE_RADIO_LABELS,
                fields.auto_assign_index,
                RadioSelectionKind::Deselectable,
            ),
        )
        .with_visibility(skirmish_visibility),
    ]
}

pub fn create_leave_game_row() -> MenuRow {
    MenuRow::shown(
        MenuFieldKind::LeaveGame,
        "Leave Game",
        MenuInput::Button {
            on_click: Callback::new(move |_| {
                game_canvas::dispatch_driver_command(DriverCommandKind::SendRequest(MessageSerialIn::LeaveGame));
            }),
        },
    )
}

pub fn create_radios_input(
    labels: &[&'static str],
    selected_index: RwSignal<Option<usize>>,
    selection: RadioSelectionKind,
) -> MenuInput {
    MenuInput::Radios {
        labels: labels.to_vec(),
        selected_index,
        selection,
    }
}

pub fn get_color_label(color: OrganismColorKind) -> &'static str {
    match color {
        OrganismColorKind::Fire => "Fire",
        OrganismColorKind::Camel => "Camel",
        OrganismColorKind::Clay => "Clay",
        OrganismColorKind::Sun => "Sun",
        OrganismColorKind::Leaf => "Leaf",
        OrganismColorKind::Lime => "Lime",
        OrganismColorKind::Sky => "Sky",
        OrganismColorKind::Lake => "Lake",
        OrganismColorKind::Ocean => "Ocean",
        OrganismColorKind::Royal => "Royal",
        OrganismColorKind::Petal => "Petal",
        OrganismColorKind::Hot => "Hot",
    }
}

/// Capitalised team name, e.g. "Red".
pub fn get_team_label(team: TeamKind) -> String {
    let team_name: &str = team.as_str();
    let mut team_name_characters: std::str::Chars<'_> = team_name.chars();

    match team_name_characters.next() {
        Some(first_character) => first_character.to_uppercase().chain(team_name_characters).collect(),
        None => String::new(),
    }
}

/// In palette order, each on a swatch of its colour.
pub fn get_color_options() -> Vec<ListOption> {
    member::ORGANISM_COLOR_ORDER
        .into_iter()
        .map(|color| ListOption {
            label: String::from(get_color_label(color)),
            swatch: Some(get_css_color(render_color::get_organism_color(color))),
        })
        .collect()
}

/// "Red: 2", in team order.
pub fn get_team_options(team_sizes: &[TeamSize]) -> Vec<ListOption> {
    team_sizes
        .iter()
        .map(|team_size| ListOption {
            label: format!("{}: {}", get_team_label(team_size.team), team_size.participant_count),
            swatch: None,
        })
        .collect()
}

/// The first in team order among the teams with the fewest participants.
pub fn get_smallest_team(team_sizes: &[TeamSize]) -> Option<TeamKind> {
    let participant_counts: Vec<(TeamKind, u32)> =
        team_sizes.iter().map(|team_size| (team_size.team, team_size.participant_count)).collect();

    TeamSizes::from_participant_counts(participant_counts).get_smallest_teams().first().copied()
}

/// `team_participant_counts[i]` is the count of the i-th team in team order.
pub fn get_summary_team_sizes(team_participant_counts: &[u8]) -> Vec<TeamSize> {
    let team_count: u8 = u8::try_from(team_participant_counts.len()).unwrap_or(u8::MAX);

    member::get_game_teams(team_count)
        .into_iter()
        .zip(team_participant_counts)
        .map(|(team, participant_count)| TeamSize {
            team,
            participant_count: u32::from(*participant_count),
        })
        .collect()
}

/// Empty outside skirmish.
pub fn get_state_team_sizes(state: &GameState) -> Vec<TeamSize> {
    let team_count: u8 = match state.settings.mode {
        GameModeKind::Skirmish => state.settings.team_count.unwrap_or(0),
        GameModeKind::FreeForAll | GameModeKind::Survival => 0,
    };
    let team_sizes: TeamSizes = TeamSizes::from_members(&state.members, team_count, None);

    member::get_game_teams(team_count)
        .into_iter()
        .map(|team| TeamSize {
            team,
            participant_count: team_sizes.get_size(team).unwrap_or(0),
        })
        .collect()
}

pub fn get_css_color(color: RenderColor) -> String {
    format!("rgb({}, {}, {})", color.red, color.green, color.blue)
}

fn get_index_of<T: PartialEq>(kinds: &[T], kind: T) -> Option<usize> {
    kinds.iter().position(|candidate| *candidate == kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::game::test_fixture;
    use shared::member::{Member, MemberId};
    use shared::world::WorldShapeKind;

    fn create_team_sizes(participant_counts: [u32; 3]) -> Vec<TeamSize> {
        [TeamKind::Red, TeamKind::Blue, TeamKind::Green]
            .into_iter()
            .zip(participant_counts)
            .map(|(team, participant_count)| TeamSize {
                team,
                participant_count,
            })
            .collect()
    }

    fn create_state_with_team_members(mode: GameModeKind) -> GameState {
        let mut state: GameState = test_fixture::create_state(mode, WorldShapeKind::Rectangle, 300);
        let member_teams: [(Member, TeamKind); 4] = [
            (test_fixture::create_participant(MemberId(0)), TeamKind::Blue),
            (test_fixture::create_participant(MemberId(1)), TeamKind::Blue),
            (test_fixture::create_spectator(MemberId(2)), TeamKind::Red),
            (test_fixture::create_participant(MemberId(3)), TeamKind::Green),
        ];

        for (mut member, team) in member_teams {
            member.team = Some(team);
            state.members.insert(member.member_id, member);
        }

        state
    }

    fn create_complete_values() -> LoadoutFieldValues {
        LoadoutFieldValues {
            color: OrganismColorKind::Lake,
            skin: SkinKind::Ghost,
            first: Some(FirstAbilityKind::Compress),
            second: Some(SecondAbilityKind::Immortality),
            third: Some(ThirdAbilityKind::Toxin),
            team: Some(TeamKind::Blue),
            auto_assign: AutoAssignKind::Unselected,
        }
    }

    #[test]
    fn to_loadout_reports_every_unselected_ability_at_its_field() {
        let values: LoadoutFieldValues = LoadoutFieldValues::join_defaults(&[]);

        assert_eq!(
            values.to_loadout(),
            Err(vec![
                MenuIssue::at_field(MenuFieldKind::FirstAbility, "Select a first ability"),
                MenuIssue::at_field(MenuFieldKind::SecondAbility, "Select a second ability"),
                MenuIssue::at_field(MenuFieldKind::ThirdAbility, "Select a third ability"),
            ]),
        );
    }

    #[test]
    fn to_loadout_builds_the_selected_loadout() {
        assert_eq!(
            create_complete_values().to_loadout(),
            Ok(Loadout {
                appearance: Appearance {
                    color: OrganismColorKind::Lake,
                    skin: SkinKind::Ghost,
                },
                first: FirstAbilityKind::Compress,
                second: SecondAbilityKind::Immortality,
                third: ThirdAbilityKind::Toxin,
            }),
        );
    }

    #[test]
    fn to_team_choice_asks_for_the_selected_team_unless_auto_assign_is_selected() {
        let mut values: LoadoutFieldValues = create_complete_values();

        assert_eq!(
            values.to_team_choice(),
            TeamChoiceKindSerialIn::Team(TeamKindSerial::Blue),
        );

        values.auto_assign = AutoAssignKind::Selected;

        assert_eq!(values.to_team_choice(), TeamChoiceKindSerialIn::Auto);
    }

    #[test]
    fn join_defaults_preselect_the_first_smallest_team() {
        let values: LoadoutFieldValues = LoadoutFieldValues::join_defaults(&create_team_sizes([2, 1, 1]));

        assert_eq!(values.team, Some(TeamKind::Blue));
        assert_eq!(values.color, OrganismColorKind::Fire);
        assert_eq!(values.skin, SkinKind::None);
    }

    #[test]
    fn get_team_options_label_each_team_with_its_size() {
        let labels: Vec<String> = get_team_options(&create_team_sizes([2, 0, 1]))
            .into_iter()
            .map(|list_option| list_option.label)
            .collect();

        assert_eq!(labels, vec!["Red: 2", "Blue: 0", "Green: 1"]);
    }

    #[test]
    fn get_color_options_put_each_colour_on_its_swatch() {
        let color_options: Vec<ListOption> = get_color_options();

        assert_eq!(color_options.len(), member::ORGANISM_COLOR_ORDER.len());
        assert_eq!(
            color_options[0],
            ListOption {
                label: String::from("Fire"),
                swatch: Some(String::from("rgb(255, 90, 81)")),
            },
        );
    }

    #[test]
    fn get_values_gives_back_the_values_new_was_given() {
        let owner: Owner = Owner::new();
        owner.set();
        let mut values: LoadoutFieldValues = create_complete_values();
        values.skin = SkinKind::None;
        values.auto_assign = AutoAssignKind::Selected;

        let loadout_fields: LoadoutFields =
            LoadoutFields::new(&values, Signal::stored(create_team_sizes([0, 0, 0])[..2].to_vec()));

        assert_eq!(loadout_fields.get_values(), values);
    }

    #[test]
    fn get_state_team_sizes_counts_the_participants_of_the_game_teams_in_skirmish() {
        let state: GameState = create_state_with_team_members(GameModeKind::Skirmish);

        assert_eq!(state.settings.team_count, Some(2));
        assert_eq!(
            get_state_team_sizes(&state),
            vec![
                TeamSize {
                    team: TeamKind::Red,
                    participant_count: 0,
                },
                TeamSize {
                    team: TeamKind::Blue,
                    participant_count: 2,
                },
            ],
        );
    }

    #[test]
    fn get_state_team_sizes_is_empty_in_free_for_all() {
        let state: GameState = create_state_with_team_members(GameModeKind::FreeForAll);

        assert_eq!(get_state_team_sizes(&state), Vec::new());
    }

    #[test]
    fn get_state_team_sizes_is_empty_in_survival() {
        let state: GameState = create_state_with_team_members(GameModeKind::Survival);

        assert_eq!(get_state_team_sizes(&state), Vec::new());
    }

    #[test]
    fn get_summary_team_sizes_pairs_the_counts_with_the_teams_in_team_order() {
        assert_eq!(
            get_summary_team_sizes(&[3, 4]),
            vec![
                TeamSize {
                    team: TeamKind::Red,
                    participant_count: 3,
                },
                TeamSize {
                    team: TeamKind::Blue,
                    participant_count: 4,
                },
            ],
        );
    }
}
