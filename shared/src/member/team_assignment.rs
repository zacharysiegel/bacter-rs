use std::collections::BTreeMap;

use crate::member;
use crate::member::{Member, MemberId, MemberRoleKind, TeamKind};

/// Participants on each team of the game, in team order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamSizes {
    participant_counts: Vec<(TeamKind, u32)>,
}

impl TeamSizes {
    /// `excluded_member_id` is not counted.
    pub fn from_members(
        members: &BTreeMap<MemberId, Member>,
        team_count: u8,
        excluded_member_id: Option<MemberId>,
    ) -> TeamSizes {
        let mut team_sizes: TeamSizes = TeamSizes {
            participant_counts: member::get_game_teams(team_count).into_iter().map(|team| (team, 0)).collect(),
        };

        for member in members.values() {
            let Some(team) = member.team else {
                continue;
            };

            let is_counted: bool =
                member.role == MemberRoleKind::Participant && Some(member.member_id) != excluded_member_id;

            if is_counted {
                team_sizes.add_member(team);
            }
        }

        team_sizes
    }

    /// Teams outside the game are ignored.
    pub fn add_member(&mut self, team: TeamKind) {
        let participant_count: Option<&mut u32> = self
            .participant_counts
            .iter_mut()
            .find(|(counted_team, _)| *counted_team == team)
            .map(|(_, participant_count)| participant_count);

        if let Some(participant_count) = participant_count {
            *participant_count += 1;
        }
    }

    /// `None` for a team outside the game.
    pub fn get_size(&self, team: TeamKind) -> Option<u32> {
        self.participant_counts
            .iter()
            .find(|(counted_team, _)| *counted_team == team)
            .map(|(_, participant_count)| *participant_count)
    }

    /// In team order.
    pub fn get_smallest_teams(&self) -> Vec<TeamKind> {
        let smallest_size: Option<u32> =
            self.participant_counts.iter().map(|(_, participant_count)| *participant_count).min();

        self.participant_counts
            .iter()
            .filter(|(_, participant_count)| Some(*participant_count) == smallest_size)
            .map(|(team, _)| *team)
            .collect()
    }

    /// The first team, in team order, with fewer participants than `team_size`.
    fn find_smaller_team(&self, team_size: u32) -> Option<TeamKind> {
        self.participant_counts
            .iter()
            .find(|(_, participant_count)| *participant_count < team_size)
            .map(|(team, _)| *team)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamChoiceRejectionKind {
    TeamNotInGame,
    TeamUnbalanced { smaller_team: TeamKind },
}

/// Allowed when the member is already on `requested_team`, or when `requested_team` is among the smallest teams.
pub fn check_team_choice(
    team_sizes: &TeamSizes,
    current_team: Option<TeamKind>,
    requested_team: TeamKind,
) -> Result<(), TeamChoiceRejectionKind> {
    let Some(requested_size) = team_sizes.get_size(requested_team) else {
        return Err(TeamChoiceRejectionKind::TeamNotInGame);
    };

    if current_team == Some(requested_team) {
        return Ok(());
    }

    match team_sizes.find_smaller_team(requested_size) {
        Some(smaller_team) => Err(TeamChoiceRejectionKind::TeamUnbalanced { smaller_team }),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;

    fn create_members(teams: &[TeamKind]) -> BTreeMap<MemberId, Member> {
        let mut members: BTreeMap<MemberId, Member> = BTreeMap::new();

        for (index, team) in teams.iter().enumerate() {
            let member_id: MemberId = MemberId(u32::try_from(index).unwrap());
            let mut member: Member = test_fixture::create_participant(member_id);
            member.team = Some(*team);
            members.insert(member_id, member);
        }

        members
    }

    #[test]
    fn from_members_counts_participants_of_the_game_teams() {
        let mut members: BTreeMap<MemberId, Member> =
            create_members(&[TeamKind::Red, TeamKind::Red, TeamKind::Green, TeamKind::Pink]);
        members.get_mut(&MemberId(1)).unwrap().role = MemberRoleKind::Spectator;

        let team_sizes: TeamSizes = TeamSizes::from_members(&members, 3, None);

        assert_eq!(team_sizes.get_size(TeamKind::Red), Some(1));
        assert_eq!(team_sizes.get_size(TeamKind::Blue), Some(0));
        assert_eq!(team_sizes.get_size(TeamKind::Green), Some(1));
        assert_eq!(team_sizes.get_size(TeamKind::Pink), None);
    }

    #[test]
    fn from_members_leaves_out_the_excluded_member() {
        let members: BTreeMap<MemberId, Member> = create_members(&[TeamKind::Red, TeamKind::Blue]);

        let team_sizes: TeamSizes = TeamSizes::from_members(&members, 2, Some(MemberId(0)));

        assert_eq!(team_sizes.get_size(TeamKind::Red), Some(0));
        assert_eq!(team_sizes.get_size(TeamKind::Blue), Some(1));
    }

    #[test]
    fn add_member_counts_a_pending_member() {
        let mut team_sizes: TeamSizes = TeamSizes::from_members(&BTreeMap::new(), 2, None);

        team_sizes.add_member(TeamKind::Blue);
        team_sizes.add_member(TeamKind::Pink);

        assert_eq!(team_sizes.get_size(TeamKind::Blue), Some(1));
        assert_eq!(team_sizes.get_size(TeamKind::Pink), None);
    }

    #[test]
    fn get_smallest_teams_lists_every_team_of_the_minimum_size() {
        let members: BTreeMap<MemberId, Member> = create_members(&[TeamKind::Blue]);

        let team_sizes: TeamSizes = TeamSizes::from_members(&members, 3, None);

        assert_eq!(team_sizes.get_smallest_teams(), vec![TeamKind::Red, TeamKind::Green]);
    }

    #[test]
    fn check_team_choice_allows_a_smallest_team() {
        let members: BTreeMap<MemberId, Member> = create_members(&[TeamKind::Red]);
        let team_sizes: TeamSizes = TeamSizes::from_members(&members, 3, None);

        assert_eq!(check_team_choice(&team_sizes, None, TeamKind::Blue), Ok(()));
        assert_eq!(check_team_choice(&team_sizes, None, TeamKind::Green), Ok(()));
    }

    #[test]
    fn check_team_choice_names_the_first_smaller_team() {
        let members: BTreeMap<MemberId, Member> =
            create_members(&[TeamKind::Red, TeamKind::Red, TeamKind::Blue, TeamKind::Red]);
        let team_sizes: TeamSizes = TeamSizes::from_members(&members, 3, None);

        assert_eq!(
            check_team_choice(&team_sizes, None, TeamKind::Red),
            Err(TeamChoiceRejectionKind::TeamUnbalanced {
                smaller_team: TeamKind::Blue,
            }),
        );
        assert_eq!(
            check_team_choice(&team_sizes, None, TeamKind::Blue),
            Err(TeamChoiceRejectionKind::TeamUnbalanced {
                smaller_team: TeamKind::Green,
            }),
        );
    }

    #[test]
    fn check_team_choice_allows_staying_on_the_current_team() {
        let members: BTreeMap<MemberId, Member> = create_members(&[TeamKind::Red, TeamKind::Red, TeamKind::Red]);
        let team_sizes: TeamSizes = TeamSizes::from_members(&members, 2, Some(MemberId(0)));

        assert_eq!(
            check_team_choice(&team_sizes, Some(TeamKind::Red), TeamKind::Red),
            Ok(()),
        );
    }

    #[test]
    fn check_team_choice_rejects_a_team_outside_the_game() {
        let team_sizes: TeamSizes = TeamSizes::from_members(&BTreeMap::new(), 2, None);

        assert_eq!(
            check_team_choice(&team_sizes, None, TeamKind::Green),
            Err(TeamChoiceRejectionKind::TeamNotInGame),
        );
    }
}
