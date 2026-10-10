use std::cmp::Reverse;

use crate::game::{GameModeKind, GameState};
use crate::member;
use crate::member::{Member, MemberId, MemberRoleKind, TeamKind};

const INFINITE_RATIO_TEXT: &str = "∞";
const ZERO_RATIO_TEXT: &str = "0";
const HUNDREDTHS_PER_WHOLE: u64 = 100;
const HUNDREDTHS_PER_TENTH: u64 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Score {
    pub kills: u32,
    pub deaths: u32,
    pub wins: u32,
}

impl Score {
    pub fn zero() -> Score {
        Score {
            kills: 0,
            deaths: 0,
            wins: 0,
        }
    }

    fn plus(self, other: Score) -> Score {
        Score {
            kills: self.kills + other.kills,
            deaths: self.deaths + other.deaths,
            wins: self.wins + other.wins,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LeaderboardSubjectKind {
    Member { member_id: MemberId, screen_name: String },
    Team { team: TeamKind },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaderboardRow {
    pub subject: LeaderboardSubjectKind,
    /// A team row sums its current members.
    pub score: Score,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaderboardColumnKind {
    Player,
    Team,
    Kills,
    Deaths,
    KillDeathRatio,
    Wins,
}

pub fn get_leaderboard_columns(mode: GameModeKind) -> Vec<LeaderboardColumnKind> {
    match mode {
        GameModeKind::FreeForAll => vec![
            LeaderboardColumnKind::Player,
            LeaderboardColumnKind::Kills,
            LeaderboardColumnKind::Deaths,
            LeaderboardColumnKind::KillDeathRatio,
        ],
        GameModeKind::Skirmish => vec![
            LeaderboardColumnKind::Team,
            LeaderboardColumnKind::Kills,
            LeaderboardColumnKind::Deaths,
            LeaderboardColumnKind::KillDeathRatio,
        ],
        GameModeKind::Survival => vec![
            LeaderboardColumnKind::Player,
            LeaderboardColumnKind::Wins,
            LeaderboardColumnKind::Kills,
        ],
    }
}

/// Player rows are cut to the leaderboard length; team rows are all shown, in team order.
pub fn get_leaderboard_rows(state: &GameState) -> Vec<LeaderboardRow> {
    match state.settings.mode {
        GameModeKind::FreeForAll => get_sorted_member_rows(state, get_free_for_all_sort_key),
        GameModeKind::Survival => get_sorted_member_rows(state, get_survival_sort_key),
        GameModeKind::Skirmish => get_team_rows(state),
    }
}

/// "∞" for kills without deaths, "0" for neither, else rounded half up to at most two decimals.
pub fn format_kill_death_ratio(kills: u32, deaths: u32) -> String {
    if deaths == 0 {
        let ratio_text: &str = if kills == 0 {
            ZERO_RATIO_TEXT
        } else {
            INFINITE_RATIO_TEXT
        };

        return String::from(ratio_text);
    }

    let hundredths: u64 = divide_rounding_half_up(u64::from(kills) * HUNDREDTHS_PER_WHOLE, u64::from(deaths));
    let whole: u64 = hundredths / HUNDREDTHS_PER_WHOLE;
    let decimal_hundredths: u64 = hundredths % HUNDREDTHS_PER_WHOLE;

    match decimal_hundredths {
        0 => format!("{whole}"),
        tenths_only if tenths_only % HUNDREDTHS_PER_TENTH == 0 => {
            format!("{whole}.{}", tenths_only / HUNDREDTHS_PER_TENTH)
        }
        _ => format!("{whole}.{decimal_hundredths:02}"),
    }
}

fn divide_rounding_half_up(numerator: u64, denominator: u64) -> u64 {
    (2 * numerator + denominator) / (2 * denominator)
}

fn get_sorted_member_rows<SortKey: Ord>(
    state: &GameState,
    get_sort_key: fn(&Member) -> SortKey,
) -> Vec<LeaderboardRow> {
    let mut participants: Vec<&Member> = get_participants(state);
    participants.sort_by_key(|participant| get_sort_key(participant));

    get_member_rows(&participants, state.settings.leaderboard_length)
}

fn get_free_for_all_sort_key(participant: &Member) -> (Reverse<u32>, u32, MemberId) {
    (
        Reverse(participant.score.kills),
        participant.score.deaths,
        participant.member_id,
    )
}

fn get_survival_sort_key(participant: &Member) -> (Reverse<u32>, Reverse<u32>, MemberId) {
    (
        Reverse(participant.score.kills),
        Reverse(participant.score.wins),
        participant.member_id,
    )
}

/// Pure Spectators are not on the board; dead Participants are.
fn get_participants(state: &GameState) -> Vec<&Member> {
    state.members.values().filter(|member| member.role == MemberRoleKind::Participant).collect()
}

fn get_member_rows(participants: &[&Member], leaderboard_length: u8) -> Vec<LeaderboardRow> {
    participants
        .iter()
        .take(usize::from(leaderboard_length))
        .map(|participant| LeaderboardRow {
            subject: LeaderboardSubjectKind::Member {
                member_id: participant.member_id,
                screen_name: participant.screen_name.clone(),
            },
            score: participant.score,
        })
        .collect()
}

fn get_team_rows(state: &GameState) -> Vec<LeaderboardRow> {
    let teams: Vec<TeamKind> = member::get_game_teams(state.settings.team_count.unwrap_or(0));

    teams
        .into_iter()
        .map(|team| {
            let team_score: Score = state
                .members
                .values()
                .filter(|member| member.team == Some(team))
                .fold(Score::zero(), |team_score, member| team_score.plus(member.score));

            LeaderboardRow {
                subject: LeaderboardSubjectKind::Team { team },
                score: team_score,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use crate::world::WorldShapeKind;

    fn create_state_with_scores(mode: GameModeKind, scores: &[(u32, u32, u32)]) -> GameState {
        let mut state: GameState = test_fixture::create_state(mode, WorldShapeKind::Rectangle, 800);

        for (index, (kills, deaths, wins)) in scores.iter().enumerate() {
            let member_id: MemberId = MemberId(u32::try_from(index).unwrap());
            let mut member: Member = test_fixture::create_participant(member_id);
            member.score = Score {
                kills: *kills,
                deaths: *deaths,
                wins: *wins,
            };
            state.members.insert(member_id, member);
        }

        state
    }

    fn get_member_ids(leaderboard_rows: &[LeaderboardRow]) -> Vec<MemberId> {
        leaderboard_rows
            .iter()
            .filter_map(|leaderboard_row| match &leaderboard_row.subject {
                LeaderboardSubjectKind::Member { member_id, .. } => Some(*member_id),
                LeaderboardSubjectKind::Team { .. } => None,
            })
            .collect()
    }

    #[test]
    fn get_leaderboard_rows_orders_free_for_all_by_kills_then_deaths_then_id() {
        let state: GameState =
            create_state_with_scores(GameModeKind::FreeForAll, &[(1, 0, 0), (3, 2, 0), (3, 1, 0), (1, 0, 0)]);

        assert_eq!(
            get_member_ids(&get_leaderboard_rows(&state)),
            vec![MemberId(2), MemberId(1), MemberId(0), MemberId(3)],
        );
    }

    #[test]
    fn get_leaderboard_rows_orders_survival_by_kills_then_wins_then_id() {
        let state: GameState =
            create_state_with_scores(GameModeKind::Survival, &[(2, 0, 1), (2, 5, 3), (4, 0, 0), (2, 0, 1)]);

        assert_eq!(
            get_member_ids(&get_leaderboard_rows(&state)),
            vec![MemberId(2), MemberId(1), MemberId(0), MemberId(3)],
        );
    }

    #[test]
    fn get_leaderboard_rows_cuts_player_rows_to_the_leaderboard_length() {
        let mut state: GameState =
            create_state_with_scores(GameModeKind::FreeForAll, &[(0, 0, 0), (5, 0, 0), (2, 0, 0)]);
        state.settings.leaderboard_length = 2;

        assert_eq!(
            get_member_ids(&get_leaderboard_rows(&state)),
            vec![MemberId(1), MemberId(2)],
        );
    }

    #[test]
    fn get_leaderboard_rows_leaves_out_pure_spectators() {
        let mut state: GameState = create_state_with_scores(GameModeKind::FreeForAll, &[(0, 0, 0), (0, 0, 0)]);
        state.members.get_mut(&MemberId(0)).unwrap().role = MemberRoleKind::Spectator;

        assert_eq!(get_member_ids(&get_leaderboard_rows(&state)), vec![MemberId(1)]);
    }

    #[test]
    fn get_leaderboard_rows_sums_teams_in_team_order() {
        let mut state: GameState = create_state_with_scores(GameModeKind::Skirmish, &[(1, 2, 0), (4, 0, 0), (2, 3, 0)]);
        state.settings.leaderboard_length = 1;

        for (member_id, team) in [
            (MemberId(0), TeamKind::Blue),
            (MemberId(1), TeamKind::Red),
            (MemberId(2), TeamKind::Blue),
        ] {
            state.members.get_mut(&member_id).unwrap().team = Some(team);
        }

        assert_eq!(
            get_leaderboard_rows(&state),
            vec![
                LeaderboardRow {
                    subject: LeaderboardSubjectKind::Team { team: TeamKind::Red },
                    score: Score {
                        kills: 4,
                        deaths: 0,
                        wins: 0,
                    },
                },
                LeaderboardRow {
                    subject: LeaderboardSubjectKind::Team { team: TeamKind::Blue },
                    score: Score {
                        kills: 3,
                        deaths: 5,
                        wins: 0,
                    },
                },
            ],
        );
    }

    #[test]
    fn get_leaderboard_columns_follow_the_mode() {
        assert_eq!(
            get_leaderboard_columns(GameModeKind::FreeForAll),
            vec![
                LeaderboardColumnKind::Player,
                LeaderboardColumnKind::Kills,
                LeaderboardColumnKind::Deaths,
                LeaderboardColumnKind::KillDeathRatio,
            ],
        );
        assert_eq!(
            get_leaderboard_columns(GameModeKind::Skirmish),
            vec![
                LeaderboardColumnKind::Team,
                LeaderboardColumnKind::Kills,
                LeaderboardColumnKind::Deaths,
                LeaderboardColumnKind::KillDeathRatio,
            ],
        );
        assert_eq!(
            get_leaderboard_columns(GameModeKind::Survival),
            vec![
                LeaderboardColumnKind::Player,
                LeaderboardColumnKind::Wins,
                LeaderboardColumnKind::Kills,
            ],
        );
    }

    #[test]
    fn format_kill_death_ratio_handles_zero_deaths() {
        assert_eq!(format_kill_death_ratio(0, 0), "0");
        assert_eq!(format_kill_death_ratio(3, 0), "∞");
    }

    #[test]
    fn format_kill_death_ratio_drops_trailing_zeros() {
        assert_eq!(format_kill_death_ratio(2, 1), "2");
        assert_eq!(format_kill_death_ratio(3, 2), "1.5");
        assert_eq!(format_kill_death_ratio(1, 3), "0.33");
        assert_eq!(format_kill_death_ratio(2, 3), "0.67");
        assert_eq!(format_kill_death_ratio(0, 4), "0");
    }

    #[test]
    fn format_kill_death_ratio_rounds_halves_up() {
        assert_eq!(format_kill_death_ratio(1, 8), "0.13");
        assert_eq!(format_kill_death_ratio(1, 200), "0.01");
    }
}
