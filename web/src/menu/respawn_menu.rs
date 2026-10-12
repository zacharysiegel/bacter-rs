use leptos::prelude::*;

use shared::ability::Loadout;
use shared::game::GameModeKind;
use shared::member::TeamKind;
use shared::protocol::{LoadoutSerial, MessageSerialIn, RequestKind, TeamChoiceKindSerialIn};

use crate::app::AppSignals;
use crate::canvas::DriverCommandKind;
use crate::canvas::game_canvas;
use crate::menu::game_menu_view;
use crate::menu::loadout_fields;
use crate::menu::menu_form;
use crate::menu::{
    AutoAssignKind, GameMenuView, LoadoutFieldValues, LoadoutFields, MenuForm, MenuIssue, MenuRow, TeamSize,
};

#[component]
pub fn RespawnMenu() -> impl IntoView {
    let signals: AppSignals = expect_context();
    let issues: RwSignal<Vec<MenuIssue>> = RwSignal::new(Vec::new());
    let mode: Signal<GameModeKind> = game_menu_view::get_game_menu_mode(signals.game_menu);
    let team_sizes: Signal<Vec<TeamSize>> = Signal::derive(move || {
        signals.game_menu.with(|game_menu| {
            game_menu.as_ref().map(|game_menu_view| game_menu_view.team_sizes.clone()).unwrap_or_default()
        })
    });
    let initial_values: LoadoutFieldValues = get_respawn_values(
        signals.game_menu.get_untracked().as_ref(),
        signals.team_choice.get_untracked(),
    );
    let loadout_fields: LoadoutFields = LoadoutFields::new(&initial_values, team_sizes);
    let rows: Vec<MenuRow> = create_rows(loadout_fields, mode, team_sizes);

    menu_form::show_request_rejections(signals.request_rejection, &[RequestKind::Respawn], issues);

    let on_submit: Callback<()> = Callback::new(move |_| {
        let values: LoadoutFieldValues = loadout_fields.get_values();
        let loadout: Result<Loadout, Vec<MenuIssue>> = values.to_loadout();

        match loadout {
            Ok(loadout) => {
                let team_choice: TeamChoiceKindSerialIn = values.to_team_choice();

                issues.set(Vec::new());
                signals.team_choice.set(team_choice);
                game_canvas::dispatch_driver_command(DriverCommandKind::SendRequest(MessageSerialIn::Respawn {
                    loadout: LoadoutSerial::from(&loadout),
                    team: team_choice,
                }));
            }
            Err(found_issues) => issues.set(found_issues),
        }
    });
    let on_back: Callback<()> = Callback::new(move |_| signals.go_back());

    view! {
        <MenuForm
            header="Spawn Options"
            submit_label="Spawn"
            rows=rows
            issues=issues
            on_submit=on_submit
            on_back=on_back
        />
    }
}

fn create_rows(
    loadout_fields: LoadoutFields,
    mode: Signal<GameModeKind>,
    team_sizes: Signal<Vec<TeamSize>>,
) -> Vec<MenuRow> {
    let mut rows: Vec<MenuRow> = loadout_fields::create_appearance_rows(loadout_fields, mode);

    rows.extend(loadout_fields::create_ability_rows(loadout_fields));
    rows.extend(loadout_fields::create_team_rows(loadout_fields, mode, team_sizes));
    rows.push(loadout_fields::create_leave_game_row());

    rows
}

/// A Participant respawns as it last played; a Spectator starts from the join defaults.
pub fn get_respawn_values(
    game_menu: Option<&GameMenuView>,
    last_team_choice: TeamChoiceKindSerialIn,
) -> LoadoutFieldValues {
    let team_sizes: &[TeamSize] = game_menu.map_or(&[], |game_menu| game_menu.team_sizes.as_slice());
    let auto_assign: AutoAssignKind = match last_team_choice {
        TeamChoiceKindSerialIn::Auto => AutoAssignKind::Selected,
        TeamChoiceKindSerialIn::Team(_) => AutoAssignKind::Unselected,
    };
    let own_loadout: Option<&Loadout> = game_menu.and_then(|game_menu| game_menu.own_loadout.as_ref());

    match own_loadout {
        Some(own_loadout) => {
            let team: Option<TeamKind> = game_menu
                .and_then(|game_menu| game_menu.own_team)
                .or_else(|| loadout_fields::get_smallest_team(team_sizes));

            LoadoutFieldValues::from_loadout(own_loadout, team, auto_assign)
        }
        None => LoadoutFieldValues::join_defaults(team_sizes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::game::test_fixture;
    use shared::protocol::TeamKindSerial;

    fn create_game_menu(own_loadout: Option<Loadout>, own_team: Option<TeamKind>) -> GameMenuView {
        GameMenuView {
            mode: GameModeKind::Skirmish,
            own_loadout,
            own_team,
            team_sizes: vec![
                TeamSize {
                    team: TeamKind::Red,
                    participant_count: 2,
                },
                TeamSize {
                    team: TeamKind::Blue,
                    participant_count: 1,
                },
            ],
        }
    }

    #[test]
    fn get_respawn_values_prefill_a_participant_from_its_loadout_and_team() {
        let game_menu: GameMenuView = create_game_menu(Some(test_fixture::create_loadout()), Some(TeamKind::Red));

        let values: LoadoutFieldValues =
            get_respawn_values(Some(&game_menu), TeamChoiceKindSerialIn::Team(TeamKindSerial::Red));

        assert_eq!(
            values,
            LoadoutFieldValues::from_loadout(
                &test_fixture::create_loadout(),
                Some(TeamKind::Red),
                AutoAssignKind::Unselected,
            ),
        );
    }

    #[test]
    fn get_respawn_values_preselect_the_smallest_team_for_a_participant_without_a_team() {
        let game_menu: GameMenuView = create_game_menu(Some(test_fixture::create_loadout()), None);

        let values: LoadoutFieldValues = get_respawn_values(Some(&game_menu), TeamChoiceKindSerialIn::Auto);

        assert_eq!(values.team, Some(TeamKind::Blue));
    }

    #[test]
    fn get_respawn_values_give_the_join_defaults_without_a_game_menu() {
        let values: LoadoutFieldValues = get_respawn_values(None, TeamChoiceKindSerialIn::Auto);

        assert_eq!(values, LoadoutFieldValues::join_defaults(&[]));
        assert_eq!(values.team, None);
    }

    #[test]
    fn get_respawn_values_keep_auto_assign_when_it_was_chosen() {
        let game_menu: GameMenuView = create_game_menu(Some(test_fixture::create_loadout()), Some(TeamKind::Red));

        let values: LoadoutFieldValues = get_respawn_values(Some(&game_menu), TeamChoiceKindSerialIn::Auto);

        assert_eq!(values.auto_assign, AutoAssignKind::Selected);
    }

    #[test]
    fn get_respawn_values_give_a_spectator_the_join_defaults() {
        let game_menu: GameMenuView = create_game_menu(None, None);

        let values: LoadoutFieldValues = get_respawn_values(Some(&game_menu), TeamChoiceKindSerialIn::Auto);

        assert_eq!(values, LoadoutFieldValues::join_defaults(&game_menu.team_sizes));
        assert_eq!(values.team, Some(TeamKind::Blue));
    }
}
