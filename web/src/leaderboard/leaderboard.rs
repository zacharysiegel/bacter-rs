use leptos::prelude::*;

use shared::game::GameState;
use shared::member::scoreboard;
use shared::member::{LeaderboardColumnKind, LeaderboardRow, LeaderboardSubjectKind, MemberId, TeamKind};

use crate::menu::loadout_fields;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowEmphasisKind {
    /// The own member, or its team.
    Own,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaderboardRowView {
    /// One per column.
    pub cells: Vec<String>,
    pub emphasis: RowEmphasisKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaderboardView {
    pub columns: Vec<LeaderboardColumnKind>,
    pub rows: Vec<LeaderboardRowView>,
}

impl LeaderboardView {
    pub fn from_state(state: &GameState, own_member_id: MemberId) -> LeaderboardView {
        let columns: Vec<LeaderboardColumnKind> = scoreboard::get_leaderboard_columns(state.settings.mode);
        let own_team: Option<TeamKind> = state.members.get(&own_member_id).and_then(|member| member.team);
        let rows: Vec<LeaderboardRowView> = scoreboard::get_leaderboard_rows(state)
            .iter()
            .map(|leaderboard_row| LeaderboardRowView {
                cells: columns.iter().map(|column| get_cell_text(leaderboard_row, *column)).collect(),
                emphasis: get_row_emphasis(leaderboard_row, own_member_id, own_team),
            })
            .collect();

        LeaderboardView { columns, rows }
    }
}

/// Top right of the window, over the canvas.
#[component]
pub fn Leaderboard(leaderboard: Signal<Option<LeaderboardView>>) -> impl IntoView {
    move || {
        leaderboard.get().map(|leaderboard_view| {
            let heading_views = leaderboard_view
                .columns
                .iter()
                .map(|column| view! { <th>{get_column_heading(*column)}</th> })
                .collect_view();
            let row_views = leaderboard_view
                .rows
                .into_iter()
                .map(|row| {
                    let row_class: &str = match row.emphasis {
                        RowEmphasisKind::Own => "leaderboard-row leaderboard-row-own",
                        RowEmphasisKind::Other => "leaderboard-row",
                    };
                    let cell_views = row.cells.into_iter().map(|cell| view! { <td>{cell}</td> }).collect_view();

                    view! { <tr class=row_class>{cell_views}</tr> }
                })
                .collect_view();

            view! {
                <table class="leaderboard">
                    <thead>
                        <tr>{heading_views}</tr>
                    </thead>
                    <tbody>{row_views}</tbody>
                </table>
            }
        })
    }
}

pub fn get_column_heading(column: LeaderboardColumnKind) -> &'static str {
    match column {
        LeaderboardColumnKind::Player => "Player",
        LeaderboardColumnKind::Team => "Team",
        LeaderboardColumnKind::Kills => "Kills",
        LeaderboardColumnKind::Deaths => "Deaths",
        LeaderboardColumnKind::KillDeathRatio => "K:D",
        LeaderboardColumnKind::Wins => "Wins",
    }
}

fn get_cell_text(leaderboard_row: &LeaderboardRow, column: LeaderboardColumnKind) -> String {
    match (column, &leaderboard_row.subject) {
        (
            LeaderboardColumnKind::Player | LeaderboardColumnKind::Team,
            LeaderboardSubjectKind::Member { screen_name, .. },
        ) => screen_name.clone(),
        (LeaderboardColumnKind::Player | LeaderboardColumnKind::Team, LeaderboardSubjectKind::Team { team }) => {
            loadout_fields::get_team_label(*team)
        }
        (LeaderboardColumnKind::Kills, _) => leaderboard_row.score.kills.to_string(),
        (LeaderboardColumnKind::Deaths, _) => leaderboard_row.score.deaths.to_string(),
        (LeaderboardColumnKind::KillDeathRatio, _) => {
            scoreboard::format_kill_death_ratio(leaderboard_row.score.kills, leaderboard_row.score.deaths)
        }
        (LeaderboardColumnKind::Wins, _) => leaderboard_row.score.wins.to_string(),
    }
}

fn get_row_emphasis(
    leaderboard_row: &LeaderboardRow,
    own_member_id: MemberId,
    own_team: Option<TeamKind>,
) -> RowEmphasisKind {
    let is_own_row: bool = match &leaderboard_row.subject {
        LeaderboardSubjectKind::Member { member_id, .. } => *member_id == own_member_id,
        LeaderboardSubjectKind::Team { team } => own_team == Some(*team),
    };

    if is_own_row {
        RowEmphasisKind::Own
    } else {
        RowEmphasisKind::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::game::GameModeKind;
    use shared::game::test_fixture;
    use shared::member::Member;
    use shared::world::WorldShapeKind;

    const OWN_MEMBER_ID: MemberId = MemberId(1);
    const OTHER_MEMBER_ID: MemberId = MemberId(2);

    fn create_state_with_two_participants(mode: GameModeKind) -> GameState {
        let mut state: GameState = test_fixture::create_state(mode, WorldShapeKind::Rectangle, 800);
        let mut own_member: Member = test_fixture::create_participant(OWN_MEMBER_ID);
        own_member.score.kills = 3;
        own_member.score.deaths = 2;
        let mut other_member: Member = test_fixture::create_participant(OTHER_MEMBER_ID);
        other_member.score.kills = 5;

        if mode == GameModeKind::Skirmish {
            own_member.team = Some(TeamKind::Blue);
            other_member.team = Some(TeamKind::Red);
        }

        test_fixture::insert_member(&mut state, own_member);
        test_fixture::insert_member(&mut state, other_member);

        state
    }

    #[test]
    fn from_state_lists_free_for_all_players_and_emphasises_the_own_row() {
        let state: GameState = create_state_with_two_participants(GameModeKind::FreeForAll);

        let leaderboard_view: LeaderboardView = LeaderboardView::from_state(&state, OWN_MEMBER_ID);

        assert_eq!(
            leaderboard_view,
            LeaderboardView {
                columns: vec![
                    LeaderboardColumnKind::Player,
                    LeaderboardColumnKind::Kills,
                    LeaderboardColumnKind::Deaths,
                    LeaderboardColumnKind::KillDeathRatio,
                ],
                rows: vec![
                    LeaderboardRowView {
                        cells: vec![
                            String::from("player 2"),
                            String::from("5"),
                            String::from("0"),
                            String::from("∞"),
                        ],
                        emphasis: RowEmphasisKind::Other,
                    },
                    LeaderboardRowView {
                        cells: vec![
                            String::from("player 1"),
                            String::from("3"),
                            String::from("2"),
                            String::from("1.5"),
                        ],
                        emphasis: RowEmphasisKind::Own,
                    },
                ],
            },
        );
    }

    #[test]
    fn from_state_lists_skirmish_teams_and_emphasises_the_own_team() {
        let state: GameState = create_state_with_two_participants(GameModeKind::Skirmish);

        let leaderboard_view: LeaderboardView = LeaderboardView::from_state(&state, OWN_MEMBER_ID);
        let team_cells: Vec<(String, RowEmphasisKind)> =
            leaderboard_view.rows.into_iter().map(|row| (row.cells[0].clone(), row.emphasis)).collect();

        assert_eq!(
            team_cells,
            vec![
                (String::from("Red"), RowEmphasisKind::Other),
                (String::from("Blue"), RowEmphasisKind::Own),
            ],
        );
    }

    #[test]
    fn get_column_heading_names_each_column_as_the_original() {
        assert_eq!(get_column_heading(LeaderboardColumnKind::KillDeathRatio), "K:D");
        assert_eq!(get_column_heading(LeaderboardColumnKind::Wins), "Wins");
    }
}
