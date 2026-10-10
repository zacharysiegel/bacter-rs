use std::collections::BTreeMap;

use bitcode::{Decode, Encode};

use crate::error::AppError;
use crate::game::{GameModeKind, GameSettings, GameState, Tick};
use crate::geometry::Subpixels;
use crate::member;
use crate::member::{Member, MemberId, MemberRoleKind, TeamKind};
use crate::protocol::{MemberSerialOut, RejectionKind};
use crate::protocol::{geometry_serial, protocol_limits};
use crate::random::Pcg32;
use crate::round::{RoundPhase, RoundState};
use crate::world::{World, WorldBounds, WorldShapeKind};

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct GameStateSerialOut {
    pub tick: u32,
    pub settings: GameSettingsSerialOut,
    pub rng: Pcg32SerialOut,
    pub world: WorldSerialOut,
    pub round: Option<RoundStateSerialOut>,
    /// Ascending `member_id`.
    pub members: Vec<MemberSerialOut>,
    pub next_member_id: u32,
}

impl From<&GameState> for GameStateSerialOut {
    fn from(state: &GameState) -> GameStateSerialOut {
        GameStateSerialOut {
            tick: state.tick.0,
            settings: GameSettingsSerialOut::from(&state.settings),
            rng: Pcg32SerialOut::from(&state.rng),
            world: WorldSerialOut::from(&state.world),
            round: state.round.as_ref().map(RoundStateSerialOut::from),
            members: state.members.values().map(MemberSerialOut::from).collect(),
            next_member_id: state.next_member_id.0,
        }
    }
}

impl TryFrom<GameStateSerialOut> for GameState {
    type Error = AppError;

    fn try_from(state_serial_out: GameStateSerialOut) -> Result<GameState, AppError> {
        let settings: GameSettings = GameSettings::try_from(state_serial_out.settings)?;
        let round: Option<RoundState> = state_serial_out.round.map(RoundState::from);
        let is_survival: bool = settings.mode == GameModeKind::Survival;

        if round.is_some() != is_survival {
            return Err(AppError::new("a game has a round state exactly in survival"));
        }

        let next_member_id: MemberId = MemberId(state_serial_out.next_member_id);
        let members: BTreeMap<MemberId, Member> = convert_members(state_serial_out.members, &settings, next_member_id)?;

        Ok(GameState {
            tick: Tick(state_serial_out.tick),
            settings,
            rng: Pcg32::try_from(state_serial_out.rng)?,
            world: World::try_from(state_serial_out.world)?,
            round,
            members,
            next_member_id,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct GameSettingsSerialOut {
    pub title: String,
    pub mode: GameModeKindSerial,
    pub world_shape: WorldShapeKindSerial,
    pub world_width_pixels: u32,
    pub world_height_pixels: u32,
    pub player_minimum: Option<u8>,
    pub player_cap: u8,
    pub team_count: Option<u8>,
    pub leaderboard_length: u8,
}

impl From<&GameSettings> for GameSettingsSerialOut {
    fn from(settings: &GameSettings) -> GameSettingsSerialOut {
        GameSettingsSerialOut {
            title: settings.title.clone(),
            mode: GameModeKindSerial::from(&settings.mode),
            world_shape: WorldShapeKindSerial::from(&settings.world_shape),
            world_width_pixels: settings.world_width_pixels,
            world_height_pixels: settings.world_height_pixels,
            player_minimum: settings.player_minimum,
            player_cap: settings.player_cap,
            team_count: settings.team_count,
            leaderboard_length: settings.leaderboard_length,
        }
    }
}

impl TryFrom<GameSettingsSerialOut> for GameSettings {
    type Error = AppError;

    fn try_from(settings_serial_out: GameSettingsSerialOut) -> Result<GameSettings, AppError> {
        let settings: GameSettings = GameSettings {
            title: settings_serial_out.title,
            mode: GameModeKind::from(settings_serial_out.mode),
            world_shape: WorldShapeKind::from(settings_serial_out.world_shape),
            world_width_pixels: settings_serial_out.world_width_pixels,
            world_height_pixels: settings_serial_out.world_height_pixels,
            player_minimum: settings_serial_out.player_minimum,
            player_cap: settings_serial_out.player_cap,
            team_count: settings_serial_out.team_count,
            leaderboard_length: settings_serial_out.leaderboard_length,
        };

        settings.validate().map_err(RejectionKind::to_app_error)?;

        Ok(settings)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum GameModeKindSerial {
    FreeForAll,
    Skirmish,
    Survival,
}

impl From<&GameModeKind> for GameModeKindSerial {
    fn from(mode: &GameModeKind) -> GameModeKindSerial {
        match mode {
            GameModeKind::FreeForAll => GameModeKindSerial::FreeForAll,
            GameModeKind::Skirmish => GameModeKindSerial::Skirmish,
            GameModeKind::Survival => GameModeKindSerial::Survival,
        }
    }
}

impl From<GameModeKindSerial> for GameModeKind {
    fn from(mode_serial: GameModeKindSerial) -> GameModeKind {
        match mode_serial {
            GameModeKindSerial::FreeForAll => GameModeKind::FreeForAll,
            GameModeKindSerial::Skirmish => GameModeKind::Skirmish,
            GameModeKindSerial::Survival => GameModeKind::Survival,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum WorldShapeKindSerial {
    Rectangle,
    Ellipse,
}

impl From<&WorldShapeKind> for WorldShapeKindSerial {
    fn from(world_shape: &WorldShapeKind) -> WorldShapeKindSerial {
        match world_shape {
            WorldShapeKind::Rectangle => WorldShapeKindSerial::Rectangle,
            WorldShapeKind::Ellipse => WorldShapeKindSerial::Ellipse,
        }
    }
}

impl From<WorldShapeKindSerial> for WorldShapeKind {
    fn from(world_shape_serial: WorldShapeKindSerial) -> WorldShapeKind {
        match world_shape_serial {
            WorldShapeKindSerial::Rectangle => WorldShapeKind::Rectangle,
            WorldShapeKindSerial::Ellipse => WorldShapeKind::Ellipse,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Pcg32SerialOut {
    pub state: u64,
    pub increment: u64,
}

impl From<&Pcg32> for Pcg32SerialOut {
    fn from(rng: &Pcg32) -> Pcg32SerialOut {
        Pcg32SerialOut {
            state: rng.state(),
            increment: rng.increment(),
        }
    }
}

impl TryFrom<Pcg32SerialOut> for Pcg32 {
    type Error = AppError;

    fn try_from(rng_serial_out: Pcg32SerialOut) -> Result<Pcg32, AppError> {
        Pcg32::from_parts(rng_serial_out.state, rng_serial_out.increment)
            .ok_or_else(|| AppError::new("the random number generator increment is even"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct WorldSerialOut {
    pub shape: WorldShapeKindSerial,
    pub bounds: WorldBoundsSerialOut,
    pub initial_bounds: WorldBoundsSerialOut,
}

impl From<&World> for WorldSerialOut {
    fn from(world: &World) -> WorldSerialOut {
        WorldSerialOut {
            shape: WorldShapeKindSerial::from(&world.shape),
            bounds: WorldBoundsSerialOut::from(&world.bounds),
            initial_bounds: WorldBoundsSerialOut::from(&world.initial_bounds),
        }
    }
}

impl TryFrom<WorldSerialOut> for World {
    type Error = AppError;

    fn try_from(world_serial_out: WorldSerialOut) -> Result<World, AppError> {
        Ok(World {
            shape: WorldShapeKind::from(world_serial_out.shape),
            bounds: WorldBounds::try_from(world_serial_out.bounds)?,
            initial_bounds: WorldBounds::try_from(world_serial_out.initial_bounds)?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct WorldBoundsSerialOut {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

impl From<&WorldBounds> for WorldBoundsSerialOut {
    fn from(bounds: &WorldBounds) -> WorldBoundsSerialOut {
        WorldBoundsSerialOut {
            left: bounds.left.0,
            top: bounds.top.0,
            width: bounds.width.0,
            height: bounds.height.0,
        }
    }
}

impl TryFrom<WorldBoundsSerialOut> for WorldBounds {
    type Error = AppError;

    fn try_from(bounds_serial_out: WorldBoundsSerialOut) -> Result<WorldBounds, AppError> {
        Ok(WorldBounds {
            left: Subpixels(geometry_serial::check_coordinate(
                bounds_serial_out.left,
                protocol_limits::COORDINATE_LIMIT_SUBPIXELS,
            )?),
            top: Subpixels(geometry_serial::check_coordinate(
                bounds_serial_out.top,
                protocol_limits::COORDINATE_LIMIT_SUBPIXELS,
            )?),
            width: Subpixels(check_world_extent(bounds_serial_out.width)?),
            height: Subpixels(check_world_extent(bounds_serial_out.height)?),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct RoundStateSerialOut {
    pub phase: RoundPhaseSerialOut,
    pub phase_started_at: u32,
}

impl From<&RoundState> for RoundStateSerialOut {
    fn from(round: &RoundState) -> RoundStateSerialOut {
        RoundStateSerialOut {
            phase: RoundPhaseSerialOut::from(&round.phase),
            phase_started_at: round.phase_started_at.0,
        }
    }
}

impl From<RoundStateSerialOut> for RoundState {
    fn from(round_serial_out: RoundStateSerialOut) -> RoundState {
        RoundState {
            phase: RoundPhase::from(round_serial_out.phase),
            phase_started_at: Tick(round_serial_out.phase_started_at),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum RoundPhaseSerialOut {
    Waiting,
    PreRound,
    Playing,
    PostRound,
}

impl From<&RoundPhase> for RoundPhaseSerialOut {
    fn from(phase: &RoundPhase) -> RoundPhaseSerialOut {
        match phase {
            RoundPhase::Waiting => RoundPhaseSerialOut::Waiting,
            RoundPhase::PreRound => RoundPhaseSerialOut::PreRound,
            RoundPhase::Playing => RoundPhaseSerialOut::Playing,
            RoundPhase::PostRound => RoundPhaseSerialOut::PostRound,
        }
    }
}

impl From<RoundPhaseSerialOut> for RoundPhase {
    fn from(phase_serial_out: RoundPhaseSerialOut) -> RoundPhase {
        match phase_serial_out {
            RoundPhaseSerialOut::Waiting => RoundPhase::Waiting,
            RoundPhaseSerialOut::PreRound => RoundPhase::PreRound,
            RoundPhaseSerialOut::Playing => RoundPhase::Playing,
            RoundPhaseSerialOut::PostRound => RoundPhase::PostRound,
        }
    }
}

fn convert_members(
    member_serial_outs: Vec<MemberSerialOut>,
    settings: &GameSettings,
    next_member_id: MemberId,
) -> Result<BTreeMap<MemberId, Member>, AppError> {
    let member_count: u32 = u32::try_from(member_serial_outs.len())?;

    if member_count > protocol_limits::get_member_limit(settings.player_cap) {
        return Err(AppError::new("a game has more members than its member limit"));
    }

    let mut members: BTreeMap<MemberId, Member> = BTreeMap::new();

    for member_serial_out in member_serial_outs {
        let member: Member = Member::try_from(member_serial_out)?;
        let previous_member_id: Option<MemberId> = members.last_key_value().map(|(member_id, _)| *member_id);

        if previous_member_id.is_some_and(|previous_member_id| previous_member_id >= member.member_id) {
            return Err(AppError::new("members are not in strictly ascending member id order"));
        }

        if member.member_id >= next_member_id {
            return Err(AppError::new("a member id is not below the next member id"));
        }

        check_member_team(&member, settings)?;
        members.insert(member.member_id, member);
    }

    Ok(members)
}

/// A Participant has one of the game's teams exactly in skirmish; a Spectator never has a team.
fn check_member_team(member: &Member, settings: &GameSettings) -> Result<(), AppError> {
    let game_teams: Vec<TeamKind> = settings.team_count.map(member::get_game_teams).unwrap_or_default();
    let is_skirmish_participant: bool =
        settings.mode == GameModeKind::Skirmish && member.role == MemberRoleKind::Participant;
    let is_team_valid: bool = match member.team {
        Some(team) => is_skirmish_participant && game_teams.contains(&team),
        None => !is_skirmish_participant,
    };

    if !is_team_valid {
        return Err(AppError::new(
            "a member's team does not match its role and the game's teams",
        ));
    }

    Ok(())
}

fn check_world_extent(extent: i32) -> Result<i32, AppError> {
    if extent < 0 {
        return Err(AppError::new("a world extent is negative"));
    }

    geometry_serial::check_coordinate(extent, protocol_limits::COORDINATE_LIMIT_SUBPIXELS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use crate::geometry::WorldPoint;

    #[test]
    fn from_lists_members_in_ascending_id() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Survival, WorldShapeKind::Ellipse, 800);
        state.tick = Tick(12);
        state.next_member_id = MemberId(9);

        for member_id in [MemberId(8), MemberId(2), MemberId(5)] {
            state.members.insert(member_id, test_fixture::create_participant(member_id));
        }

        let state_serial_out: GameStateSerialOut = GameStateSerialOut::from(&state);
        let member_ids: Vec<u32> = state_serial_out.members.iter().map(|member| member.member_id).collect();

        assert_eq!(member_ids, vec![2, 5, 8]);
        assert_eq!(state_serial_out.tick, 12);
        assert_eq!(state_serial_out.next_member_id, 9);
        assert_eq!(
            state_serial_out.rng,
            Pcg32SerialOut {
                state: state.rng.state(),
                increment: state.rng.increment(),
            },
        );
        assert_eq!(
            state_serial_out.round,
            Some(RoundStateSerialOut {
                phase: RoundPhaseSerialOut::Waiting,
                phase_started_at: 0,
            }),
        );
    }

    #[test]
    fn from_copies_the_settings_and_the_world() {
        let state: GameState = test_fixture::create_state(GameModeKind::Skirmish, WorldShapeKind::Ellipse, 800);
        let state_serial_out: GameStateSerialOut = GameStateSerialOut::from(&state);
        let bounds_serial_out: WorldBoundsSerialOut = WorldBoundsSerialOut {
            left: 0,
            top: 0,
            width: 819_200,
            height: 819_200,
        };

        assert_eq!(
            state_serial_out.settings,
            GameSettingsSerialOut {
                title: String::from("Fixture game"),
                mode: GameModeKindSerial::Skirmish,
                world_shape: WorldShapeKindSerial::Ellipse,
                world_width_pixels: 800,
                world_height_pixels: 800,
                player_minimum: None,
                player_cap: 8,
                team_count: Some(2),
                leaderboard_length: 10,
            },
        );
        assert_eq!(
            state_serial_out.world,
            WorldSerialOut {
                shape: WorldShapeKindSerial::Ellipse,
                bounds: bounds_serial_out,
                initial_bounds: bounds_serial_out,
            },
        );
    }

    fn create_state_with_members(mode: GameModeKind) -> GameState {
        let mut state: GameState = test_fixture::create_state(mode, WorldShapeKind::Rectangle, 800);
        state.tick = Tick(40);
        state.next_member_id = MemberId(6);

        for member_id in [MemberId(1), MemberId(4)] {
            let mut member: Member =
                test_fixture::create_participant_with_organism(member_id, WorldPoint { x: 200, y: 300 });

            if mode == GameModeKind::Skirmish {
                member.team = Some(TeamKind::Blue);
            }

            state.members.insert(member_id, member);
        }

        let mut spectator: Member = test_fixture::create_participant(MemberId(5));
        spectator.role = MemberRoleKind::Spectator;
        spectator.loadout = None;
        state.members.insert(MemberId(5), spectator);

        state
    }

    #[test]
    fn try_from_restores_the_state_of_every_mode() {
        for mode in [GameModeKind::FreeForAll, GameModeKind::Skirmish, GameModeKind::Survival] {
            let state: GameState = create_state_with_members(mode);

            assert_eq!(GameState::try_from(GameStateSerialOut::from(&state)).unwrap(), state);
        }
    }

    #[test]
    fn try_from_rejects_unordered_and_duplicate_members() {
        let state_serial_out: GameStateSerialOut =
            GameStateSerialOut::from(&create_state_with_members(GameModeKind::FreeForAll));
        let mut unordered_state_serial_out: GameStateSerialOut = state_serial_out.clone();
        unordered_state_serial_out.members.swap(0, 1);
        let mut duplicate_state_serial_out: GameStateSerialOut = state_serial_out;
        duplicate_state_serial_out.members[1] = duplicate_state_serial_out.members[0].clone();

        assert!(GameState::try_from(unordered_state_serial_out).is_err());
        assert!(GameState::try_from(duplicate_state_serial_out).is_err());
    }

    #[test]
    fn try_from_rejects_a_member_id_not_below_the_next_member_id() {
        let mut state_serial_out: GameStateSerialOut =
            GameStateSerialOut::from(&create_state_with_members(GameModeKind::FreeForAll));
        state_serial_out.next_member_id = 5;

        assert!(GameState::try_from(state_serial_out).is_err());
    }

    #[test]
    fn try_from_rejects_an_even_random_number_generator_increment() {
        let mut state_serial_out: GameStateSerialOut =
            GameStateSerialOut::from(&create_state_with_members(GameModeKind::FreeForAll));
        state_serial_out.rng.increment = 2;

        assert!(GameState::try_from(state_serial_out).is_err());
    }

    #[test]
    fn try_from_rejects_a_team_inconsistent_with_the_mode() {
        let mut free_for_all_state: GameState = create_state_with_members(GameModeKind::FreeForAll);
        free_for_all_state.members.get_mut(&MemberId(1)).unwrap().team = Some(TeamKind::Red);
        let mut skirmish_state: GameState = create_state_with_members(GameModeKind::Skirmish);
        skirmish_state.members.get_mut(&MemberId(4)).unwrap().team = None;
        let mut outside_team_state: GameState = create_state_with_members(GameModeKind::Skirmish);
        outside_team_state.members.get_mut(&MemberId(4)).unwrap().team = Some(TeamKind::Pink);
        let mut spectator_team_state: GameState = create_state_with_members(GameModeKind::Skirmish);
        spectator_team_state.members.get_mut(&MemberId(5)).unwrap().team = Some(TeamKind::Red);

        for state in [
            free_for_all_state,
            skirmish_state,
            outside_team_state,
            spectator_team_state,
        ] {
            assert!(GameState::try_from(GameStateSerialOut::from(&state)).is_err());
        }
    }

    #[test]
    fn try_from_rejects_a_round_inconsistent_with_the_mode() {
        let mut survival_state: GameState = create_state_with_members(GameModeKind::Survival);
        survival_state.round = None;
        let mut free_for_all_state: GameState = create_state_with_members(GameModeKind::FreeForAll);
        free_for_all_state.round = Some(RoundState {
            phase: RoundPhase::Playing,
            phase_started_at: Tick(3),
        });

        assert!(GameState::try_from(GameStateSerialOut::from(&survival_state)).is_err());
        assert!(GameState::try_from(GameStateSerialOut::from(&free_for_all_state)).is_err());
    }

    #[test]
    fn try_from_rejects_a_loadout_inconsistent_with_the_role() {
        let mut state: GameState = create_state_with_members(GameModeKind::FreeForAll);
        state.members.get_mut(&MemberId(1)).unwrap().loadout = None;

        assert!(GameState::try_from(GameStateSerialOut::from(&state)).is_err());
    }

    #[test]
    fn try_from_rejects_invalid_settings() {
        let mut state_serial_out: GameStateSerialOut =
            GameStateSerialOut::from(&create_state_with_members(GameModeKind::FreeForAll));
        state_serial_out.settings.team_count = Some(2);

        assert!(GameState::try_from(state_serial_out).is_err());
    }

    #[test]
    fn try_from_rejects_more_members_than_the_member_limit() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        let member_limit: u32 = protocol_limits::get_member_limit(state.settings.player_cap);

        for member_index in 0..=member_limit {
            state.members.insert(
                MemberId(member_index),
                test_fixture::create_participant(MemberId(member_index)),
            );
        }

        state.next_member_id = MemberId(member_limit + 1);

        assert!(GameState::try_from(GameStateSerialOut::from(&state)).is_err());
    }

    #[test]
    fn try_from_rejects_a_negative_world_extent() {
        let mut state_serial_out: GameStateSerialOut =
            GameStateSerialOut::from(&create_state_with_members(GameModeKind::FreeForAll));
        state_serial_out.world.bounds.width = -1;

        assert!(GameState::try_from(state_serial_out).is_err());
    }
}
