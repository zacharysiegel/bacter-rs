use bitcode::{Decode, Encode};

use crate::game::{GameModeKind, GameSettings, GameState};
use crate::protocol::MemberSerialOut;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Tick, test_fixture};
    use crate::member::MemberId;

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
}
