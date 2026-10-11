use crate::game::GameState;
use crate::member::MemberId;
use crate::protocol::RejectionKind;
use crate::round;
use crate::round::{RoundPhase, RoundState};

pub const TRANSIENT_MESSAGE_DURATION_MILLISECONDS: f64 = 3000.0;
/// While a resync waits for its snapshot.
pub const CATCHING_UP_TEXT: &str = "Catching up";
const PRESS_TO_SPAWN_TEXT: &str = "Press 'R' to Spawn";
const NO_ROOM_TO_SPAWN_TEXT: &str = "No room to spawn; press 'R' to try again";

/// Shown for `TRANSIENT_MESSAGE_DURATION_MILLISECONDS` in place of the game's message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransientMessageKind {
    NoRoomToSpawn,
    GameAtCapacity,
}

impl TransientMessageKind {
    pub fn text(self) -> String {
        match self {
            TransientMessageKind::NoRoomToSpawn => String::from(NO_ROOM_TO_SPAWN_TEXT),
            TransientMessageKind::GameAtCapacity => RejectionKind::GameFull.message_text(),
        }
    }
}

/// `None` when there is nothing to tell the member.
pub fn get_message_text(state: &GameState, own_member_id: MemberId) -> Option<String> {
    let own_member_has_organism: bool = state.members.get(&own_member_id)?.organism.is_some();

    match state.round {
        Some(round) => get_round_message_text(state, round, own_member_has_organism),
        None => (!own_member_has_organism).then(|| String::from(PRESS_TO_SPAWN_TEXT)),
    }
}

fn get_round_message_text(state: &GameState, round: RoundState, own_member_has_organism: bool) -> Option<String> {
    match round.phase {
        RoundPhase::Waiting => Some(get_waiting_text(state)),
        RoundPhase::PreRound => round
            .get_countdown_seconds(state.tick)
            .map(|countdown_seconds| format!("Round begins in: {countdown_seconds}")),
        RoundPhase::Playing => (!own_member_has_organism).then(|| RejectionKind::RoundInProgress.message_text()),
        RoundPhase::PostRound => round
            .get_countdown_seconds(state.tick)
            .map(|countdown_seconds| format!("Round ends in: {countdown_seconds}")),
    }
}

fn get_waiting_text(state: &GameState) -> String {
    let player_minimum: u32 = u32::from(state.settings.player_minimum.unwrap_or(0));
    let missing_player_count: u32 = player_minimum.saturating_sub(round::count_participants(state));

    if missing_player_count == 1 {
        return String::from("Waiting for 1 more player to join");
    }

    format!("Waiting for {missing_player_count} more players to join")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameModeKind, Tick, test_fixture};
    use crate::geometry::WorldPoint;
    use crate::member::{Member, MemberRoleKind};
    use crate::round::RoundState;
    use crate::world::WorldShapeKind;

    const OWN_MEMBER_ID: MemberId = MemberId(0);

    fn create_state(mode: GameModeKind, own_member: Member) -> GameState {
        let mut state: GameState = test_fixture::create_state(mode, WorldShapeKind::Rectangle, 800);
        state.members.insert(OWN_MEMBER_ID, own_member);

        state
    }

    fn create_alive_member() -> Member {
        test_fixture::create_participant_with_organism(OWN_MEMBER_ID, WorldPoint { x: 100, y: 100 })
    }

    fn create_spectator() -> Member {
        let mut member: Member = test_fixture::create_participant(OWN_MEMBER_ID);
        member.role = MemberRoleKind::Spectator;
        member.loadout = None;

        member
    }

    fn create_survival_state(own_member: Member, phase: RoundPhase, tick: Tick) -> GameState {
        let mut state: GameState = create_state(GameModeKind::Survival, own_member);
        state.round = Some(RoundState {
            phase,
            phase_started_at: Tick(0),
        });
        state.tick = tick;

        state
    }

    fn get_own_message_text(state: &GameState) -> Option<String> {
        get_message_text(state, OWN_MEMBER_ID)
    }

    #[test]
    fn get_message_text_without_rounds_asks_a_member_without_an_organism_to_spawn() {
        let alive_state: GameState = create_state(GameModeKind::FreeForAll, create_alive_member());
        let dead_state: GameState =
            create_state(GameModeKind::Skirmish, test_fixture::create_participant(OWN_MEMBER_ID));
        let spectator_state: GameState = create_state(GameModeKind::FreeForAll, create_spectator());

        assert_eq!(get_own_message_text(&alive_state), None);
        assert_eq!(
            get_own_message_text(&dead_state),
            Some(String::from("Press 'R' to Spawn")),
        );
        assert_eq!(
            get_own_message_text(&spectator_state),
            Some(String::from("Press 'R' to Spawn")),
        );
    }

    #[test]
    fn get_message_text_while_waiting_counts_the_missing_players() {
        let singular_state: GameState = create_survival_state(create_alive_member(), RoundPhase::Waiting, Tick(5));
        let plural_state: GameState = create_survival_state(create_spectator(), RoundPhase::Waiting, Tick(5));

        assert_eq!(
            get_own_message_text(&singular_state),
            Some(String::from("Waiting for 1 more player to join")),
        );
        assert_eq!(
            get_own_message_text(&plural_state),
            Some(String::from("Waiting for 2 more players to join")),
        );
    }

    #[test]
    fn get_message_text_counts_down_before_and_after_a_round() {
        let pre_round_state: GameState = create_survival_state(create_alive_member(), RoundPhase::PreRound, Tick(14));
        let post_round_state: GameState = create_survival_state(create_spectator(), RoundPhase::PostRound, Tick(0));

        assert_eq!(
            get_own_message_text(&pre_round_state),
            Some(String::from("Round begins in: 7")),
        );
        assert_eq!(
            get_own_message_text(&post_round_state),
            Some(String::from("Round ends in: 8")),
        );
    }

    #[test]
    fn get_message_text_while_playing_asks_only_a_member_without_an_organism_to_wait() {
        let alive_state: GameState = create_survival_state(create_alive_member(), RoundPhase::Playing, Tick(5));
        let dead_state: GameState = create_survival_state(
            test_fixture::create_participant(OWN_MEMBER_ID),
            RoundPhase::Playing,
            Tick(5),
        );

        assert_eq!(get_own_message_text(&alive_state), None);
        assert_eq!(
            get_own_message_text(&dead_state),
            Some(String::from("Wait for the round to complete")),
        );
    }

    #[test]
    fn get_message_text_is_none_for_a_member_not_in_the_game() {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);

        assert_eq!(get_own_message_text(&state), None);
    }

    #[test]
    fn text_gives_the_transient_message_texts() {
        assert_eq!(
            TransientMessageKind::NoRoomToSpawn.text(),
            "No room to spawn; press 'R' to try again",
        );
        assert_eq!(
            TransientMessageKind::GameAtCapacity.text(),
            "Game is at maximum player capacity",
        );
    }
}
