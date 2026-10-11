use crate::game::GameState;
use crate::member::{Member, MemberId, MemberRoleKind};
use crate::play::crosshair;
use crate::round;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnMemberStateKind {
    AliveParticipant,
    DeadParticipant,
    Spectator,
}

impl OwnMemberStateKind {
    pub fn from_member(member: &Member) -> OwnMemberStateKind {
        match (&member.organism, member.role) {
            (Some(_), _) => OwnMemberStateKind::AliveParticipant,
            (None, MemberRoleKind::Participant) => OwnMemberStateKind::DeadParticipant,
            (None, MemberRoleKind::Spectator) => OwnMemberStateKind::Spectator,
        }
    }

    /// Without an organism the crosshair is the spectating camera.
    pub fn crosshair_speed_pixels_per_second(self) -> f64 {
        match self {
            OwnMemberStateKind::AliveParticipant => crosshair::CROSSHAIR_SPEED_PIXELS_PER_SECOND,
            OwnMemberStateKind::DeadParticipant | OwnMemberStateKind::Spectator => {
                crosshair::SPECTATOR_CAMERA_SPEED_PIXELS_PER_SECOND
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RespawnKeyOutcomeKind {
    OpenRespawnMenu,
    ShowGameAtCapacity,
    Ignore,
}

/// `None` until the member's `Joined` has been applied.
pub fn get_own_member_state(state: &GameState, member_id: MemberId) -> Option<OwnMemberStateKind> {
    state.members.get(&member_id).map(OwnMemberStateKind::from_member)
}

pub fn get_respawn_key_outcome(state: &GameState, member_id: MemberId) -> RespawnKeyOutcomeKind {
    let own_member_state: Option<OwnMemberStateKind> = get_own_member_state(state, member_id);
    let is_without_organism: bool = matches!(
        own_member_state,
        Some(OwnMemberStateKind::DeadParticipant | OwnMemberStateKind::Spectator)
    );

    if !is_without_organism || round::is_round_in_progress(state) {
        return RespawnKeyOutcomeKind::Ignore;
    }

    if state.is_at_player_cap() {
        return RespawnKeyOutcomeKind::ShowGameAtCapacity;
    }

    RespawnKeyOutcomeKind::OpenRespawnMenu
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameModeKind, Tick, test_fixture};
    use crate::geometry::WorldPoint;
    use crate::round::{RoundPhase, RoundState};
    use crate::world::WorldShapeKind;

    const OWN_MEMBER_ID: MemberId = MemberId(0);
    const OTHER_MEMBER_ID: MemberId = MemberId(1);

    fn create_state_with_own_member(mode: GameModeKind, own_member: Member) -> GameState {
        let mut state: GameState = test_fixture::create_state(mode, WorldShapeKind::Rectangle, 800);
        state.members.insert(OWN_MEMBER_ID, own_member);

        state
    }

    fn set_round_phase(state: &mut GameState, phase: RoundPhase) {
        state.round = Some(RoundState {
            phase,
            phase_started_at: Tick(0),
        });
    }

    #[test]
    fn from_member_classifies_alive_dead_and_spectating_members() {
        let alive_member: Member =
            test_fixture::create_participant_with_organism(OWN_MEMBER_ID, WorldPoint { x: 100, y: 100 });

        assert_eq!(
            OwnMemberStateKind::from_member(&alive_member),
            OwnMemberStateKind::AliveParticipant,
        );
        assert_eq!(
            OwnMemberStateKind::from_member(&test_fixture::create_participant(OWN_MEMBER_ID)),
            OwnMemberStateKind::DeadParticipant,
        );
        assert_eq!(
            OwnMemberStateKind::from_member(&test_fixture::create_spectator(OWN_MEMBER_ID)),
            OwnMemberStateKind::Spectator,
        );
    }

    #[test]
    fn get_own_member_state_is_none_before_the_member_joins() {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);

        assert_eq!(get_own_member_state(&state, OWN_MEMBER_ID), None);
    }

    #[test]
    fn crosshair_speed_pixels_per_second_moves_a_camera_faster_than_a_crosshair() {
        assert_eq!(
            OwnMemberStateKind::AliveParticipant.crosshair_speed_pixels_per_second(),
            42.5,
        );
        assert_eq!(
            OwnMemberStateKind::DeadParticipant.crosshair_speed_pixels_per_second(),
            62.5,
        );
        assert_eq!(OwnMemberStateKind::Spectator.crosshair_speed_pixels_per_second(), 62.5);
    }

    #[test]
    fn get_respawn_key_outcome_ignores_an_alive_participant() {
        let state: GameState = create_state_with_own_member(
            GameModeKind::FreeForAll,
            test_fixture::create_participant_with_organism(OWN_MEMBER_ID, WorldPoint { x: 100, y: 100 }),
        );

        assert_eq!(
            get_respawn_key_outcome(&state, OWN_MEMBER_ID),
            RespawnKeyOutcomeKind::Ignore,
        );
    }

    #[test]
    fn get_respawn_key_outcome_opens_the_respawn_menu_below_the_cap() {
        let state: GameState = create_state_with_own_member(
            GameModeKind::FreeForAll,
            test_fixture::create_participant(OWN_MEMBER_ID),
        );

        assert_eq!(
            get_respawn_key_outcome(&state, OWN_MEMBER_ID),
            RespawnKeyOutcomeKind::OpenRespawnMenu,
        );
    }

    #[test]
    fn get_respawn_key_outcome_shows_the_capacity_message_at_the_cap() {
        let mut state: GameState =
            create_state_with_own_member(GameModeKind::Skirmish, test_fixture::create_spectator(OWN_MEMBER_ID));
        state.settings.player_cap = 1;
        state.members.insert(
            OTHER_MEMBER_ID,
            test_fixture::create_participant_with_organism(OTHER_MEMBER_ID, WorldPoint { x: 100, y: 100 }),
        );

        assert_eq!(
            get_respawn_key_outcome(&state, OWN_MEMBER_ID),
            RespawnKeyOutcomeKind::ShowGameAtCapacity,
        );
    }

    #[test]
    fn get_respawn_key_outcome_allows_spawning_only_before_a_round_plays() {
        let mut state: GameState =
            create_state_with_own_member(GameModeKind::Survival, test_fixture::create_participant(OWN_MEMBER_ID));
        let mut outcomes: Vec<RespawnKeyOutcomeKind> = Vec::new();

        for phase in [
            RoundPhase::Waiting,
            RoundPhase::PreRound,
            RoundPhase::Playing,
            RoundPhase::PostRound,
        ] {
            set_round_phase(&mut state, phase);
            outcomes.push(get_respawn_key_outcome(&state, OWN_MEMBER_ID));
        }

        assert_eq!(
            outcomes,
            vec![
                RespawnKeyOutcomeKind::OpenRespawnMenu,
                RespawnKeyOutcomeKind::OpenRespawnMenu,
                RespawnKeyOutcomeKind::Ignore,
                RespawnKeyOutcomeKind::Ignore,
            ],
        );
    }
}
