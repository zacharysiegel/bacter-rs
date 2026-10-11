use shared::game::GameState;
use shared::member::{MemberId, MemberRoleKind, TeamKind, TeamSizes};

/// What the requests admitted since the last bundle add to the state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PendingAdmissions {
    screen_names: Vec<String>,
    member_count: u32,
    spectator_count: u32,
    spawning_member_ids: Vec<MemberId>,
    /// Of the pending spawns in skirmish.
    team_additions: Vec<TeamKind>,
}

impl PendingAdmissions {
    pub fn new() -> PendingAdmissions {
        PendingAdmissions::default()
    }

    pub fn clear(&mut self) {
        *self = PendingAdmissions::new();
    }

    pub fn is_screen_name_taken(&self, state: &GameState, screen_name: &str) -> bool {
        let is_member_name: bool = state.members.values().any(|member| member.screen_name == screen_name);

        is_member_name || self.screen_names.iter().any(|pending_name| pending_name == screen_name)
    }

    pub fn get_member_count(&self, state: &GameState) -> u32 {
        get_count(state.members.len()) + self.member_count
    }

    /// Pure Spectators.
    pub fn get_spectator_count(&self, state: &GameState) -> u32 {
        let spectator_count: usize =
            state.members.values().filter(|member| member.role == MemberRoleKind::Spectator).count();

        get_count(spectator_count) + self.spectator_count
    }

    /// Alive organisms plus pending spawns.
    pub fn get_spawning_count(&self, state: &GameState) -> u32 {
        state.alive_organism_count() + get_count(self.spawning_member_ids.len())
    }

    pub fn is_spawn_pending(&self, member_id: MemberId) -> bool {
        self.spawning_member_ids.contains(&member_id)
    }

    /// `excluded_member_id` is not counted.
    pub fn get_team_sizes(&self, state: &GameState, team_count: u8, excluded_member_id: Option<MemberId>) -> TeamSizes {
        let mut team_sizes: TeamSizes = TeamSizes::from_members(&state.members, team_count, excluded_member_id);

        for team in &self.team_additions {
            team_sizes.add_member(*team);
        }

        team_sizes
    }

    pub fn get_next_member_id(&self, state: &GameState) -> MemberId {
        MemberId(state.next_member_id.0 + self.member_count)
    }

    pub fn record_member(&mut self, screen_name: &str, role: MemberRoleKind) {
        self.screen_names.push(String::from(screen_name));
        self.member_count += 1;

        if role == MemberRoleKind::Spectator {
            self.spectator_count += 1;
        }
    }

    pub fn record_spawn(&mut self, member_id: MemberId, team: Option<TeamKind>) {
        self.spawning_member_ids.push(member_id);
        self.team_additions.extend(team);
    }
}

fn get_count(length: usize) -> u32 {
    u32::try_from(length).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use shared::game::GameModeKind;
    use shared::geometry::WorldPoint;
    use shared::member::{Member, Score};
    use shared::organism::Organism;

    fn create_member(member_id: u32, role: MemberRoleKind, team: Option<TeamKind>, is_alive: bool) -> Member {
        Member {
            member_id: MemberId(member_id),
            screen_name: format!("member {member_id}"),
            role,
            loadout: Some(test_fixture::create_loadout()),
            team,
            score: Score::zero(),
            organism: is_alive.then(|| Organism::new(WorldPoint { x: 100, y: 100 })),
        }
    }

    fn create_state_with_members(mode: GameModeKind, members: Vec<Member>) -> GameState {
        let mut state: GameState = test_fixture::create_state(mode, 8);

        for member in members {
            state.next_member_id = member.member_id.next();
            state.members.insert(member.member_id, member);
        }

        state
    }

    #[test]
    fn is_screen_name_taken_checks_members_and_pending_names() {
        let state: GameState = create_state_with_members(
            GameModeKind::FreeForAll,
            vec![create_member(0, MemberRoleKind::Participant, None, true)],
        );
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();
        pending_admissions.record_member("Blob", MemberRoleKind::Participant);

        assert!(pending_admissions.is_screen_name_taken(&state, "member 0"));
        assert!(pending_admissions.is_screen_name_taken(&state, "Blob"));
        assert!(!pending_admissions.is_screen_name_taken(&state, "blob"));
    }

    #[test]
    fn get_member_count_and_the_other_counts_add_pending_admissions_to_the_state() {
        let state: GameState = create_state_with_members(
            GameModeKind::FreeForAll,
            vec![
                create_member(0, MemberRoleKind::Participant, None, true),
                create_member(1, MemberRoleKind::Participant, None, false),
                create_member(2, MemberRoleKind::Spectator, None, false),
            ],
        );
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();
        pending_admissions.record_member("Watcher", MemberRoleKind::Spectator);
        pending_admissions.record_member("Late", MemberRoleKind::Participant);
        pending_admissions.record_spawn(MemberId(4), None);
        pending_admissions.record_spawn(MemberId(1), None);

        assert_eq!(pending_admissions.get_member_count(&state), 5);
        assert_eq!(pending_admissions.get_spectator_count(&state), 2);
        assert_eq!(pending_admissions.get_spawning_count(&state), 3);
        assert_eq!(pending_admissions.get_next_member_id(&state), MemberId(5));
        assert!(pending_admissions.is_spawn_pending(MemberId(1)));
        assert!(!pending_admissions.is_spawn_pending(MemberId(0)));
    }

    #[test]
    fn get_team_sizes_adds_pending_teams_and_skips_the_excluded_member() {
        let state: GameState = create_state_with_members(
            GameModeKind::Skirmish,
            vec![
                create_member(0, MemberRoleKind::Participant, Some(TeamKind::Red), true),
                create_member(1, MemberRoleKind::Participant, Some(TeamKind::Red), false),
            ],
        );
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();
        pending_admissions.record_spawn(MemberId(2), Some(TeamKind::Blue));

        let team_sizes: TeamSizes = pending_admissions.get_team_sizes(&state, 2, Some(MemberId(1)));

        assert_eq!(team_sizes.get_size(TeamKind::Red), Some(1));
        assert_eq!(team_sizes.get_size(TeamKind::Blue), Some(1));
    }

    #[test]
    fn clear_forgets_every_pending_admission() {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();
        pending_admissions.record_member("Blob", MemberRoleKind::Participant);
        pending_admissions.record_spawn(MemberId(0), None);

        pending_admissions.clear();

        assert_eq!(pending_admissions, PendingAdmissions::new());
        assert_eq!(pending_admissions.get_next_member_id(&state), MemberId(0));
    }
}
