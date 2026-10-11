use crate::game::{GameModeKind, GameState};
use crate::member;
use crate::member::{TeamKind, TeamSizes};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GameId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameSummary {
    pub game_id: GameId,
    pub title: String,
    pub mode: GameModeKind,
    /// Alive organisms.
    pub player_count: u8,
    /// Members without an organism.
    pub spectator_count: u8,
    pub player_cap: u8,
    pub secured: bool,
    /// Participants per team, in team order.
    pub team_sizes: Vec<u8>,
}

impl GameSummary {
    pub fn from_game_state(game_id: GameId, state: &GameState, secured: bool) -> GameSummary {
        let member_count: usize = state.members.len();
        let alive_organism_count: u32 = state.alive_organism_count();
        let spectator_count: usize = member_count.saturating_sub(usize::try_from(alive_organism_count).unwrap_or(0));

        GameSummary {
            game_id,
            title: state.settings.title.clone(),
            mode: state.settings.mode,
            player_count: u8::try_from(alive_organism_count).unwrap_or(u8::MAX),
            spectator_count: u8::try_from(spectator_count).unwrap_or(u8::MAX),
            player_cap: state.settings.player_cap,
            secured,
            team_sizes: get_team_sizes(state),
        }
    }
}

/// Empty outside skirmish.
fn get_team_sizes(state: &GameState) -> Vec<u8> {
    let Some(team_count): Option<u8> = state.settings.team_count else {
        return Vec::new();
    };

    let team_sizes: TeamSizes = TeamSizes::from_members(&state.members, team_count, None);

    member::get_game_teams(team_count)
        .into_iter()
        .map(|team: TeamKind| team_sizes.get_size(team).unwrap_or(0))
        .map(|team_size: u32| u8::try_from(team_size).unwrap_or(u8::MAX))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use crate::geometry::WorldPoint;
    use crate::member::{Member, MemberId, MemberRoleKind};
    use crate::world::WorldShapeKind;

    #[test]
    fn from_game_state_counts_alive_organisms_and_members_without_one() {
        let mut state: GameState =
            test_fixture::create_state_with_organisms(GameModeKind::FreeForAll, 800, &[WorldPoint { x: 100, y: 100 }]);
        state.members.insert(MemberId(1), test_fixture::create_participant(MemberId(1)));
        let mut spectator: Member = test_fixture::create_participant(MemberId(2));
        spectator.role = MemberRoleKind::Spectator;
        spectator.loadout = None;
        state.members.insert(MemberId(2), spectator);

        assert_eq!(
            GameSummary::from_game_state(GameId(4), &state, true),
            GameSummary {
                game_id: GameId(4),
                title: String::from("Fixture game"),
                mode: GameModeKind::FreeForAll,
                player_count: 1,
                spectator_count: 2,
                player_cap: 8,
                secured: true,
                team_sizes: Vec::new(),
            },
        );
    }

    #[test]
    fn from_game_state_lists_participants_per_team_in_skirmish() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Skirmish, WorldShapeKind::Rectangle, 800);

        for (member_id, team) in [(0, TeamKind::Blue), (1, TeamKind::Blue), (2, TeamKind::Red)] {
            let mut member: Member = test_fixture::create_participant(MemberId(member_id));
            member.team = Some(team);
            state.members.insert(MemberId(member_id), member);
        }

        assert_eq!(
            GameSummary::from_game_state(GameId(0), &state, false).team_sizes,
            vec![1, 2]
        );
    }
}
