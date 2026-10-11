use std::path::PathBuf;

use tokio::sync::{mpsc, watch};

use shared::ability::{FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use shared::game::{GameId, GameModeKind, GameSettings, GameState, GameSummary};
use shared::geometry::WorldPoint;
use shared::member::{
    Appearance, Joiner, Member, MemberId, MemberRoleKind, OrganismColorKind, Score, SkinKind, TeamChoiceKind, TeamKind,
};
use shared::organism::Organism;
use shared::world::WorldShapeKind;

use crate::game::{GameHandle, GameInput, GameRequest};

pub const FIXTURE_SEED: u64 = 0x5eed;

pub fn create_settings(mode: GameModeKind, player_cap: u8) -> GameSettings {
    let player_minimum: Option<u8> = match mode {
        GameModeKind::Survival => Some(2),
        GameModeKind::FreeForAll | GameModeKind::Skirmish => None,
    };
    let team_count: Option<u8> = match mode {
        GameModeKind::Skirmish => Some(2),
        GameModeKind::FreeForAll | GameModeKind::Survival => None,
    };

    GameSettings {
        title: String::from("Fixture game"),
        mode,
        world_shape: WorldShapeKind::Rectangle,
        world_width_pixels: 800,
        world_height_pixels: 800,
        player_minimum,
        player_cap,
        team_count,
        leaderboard_length: 10,
    }
}

pub fn create_state(mode: GameModeKind, player_cap: u8) -> GameState {
    GameState::new(create_settings(mode, player_cap), FIXTURE_SEED)
}

pub fn create_loadout() -> Loadout {
    Loadout {
        appearance: Appearance {
            color: OrganismColorKind::Ocean,
            skin: SkinKind::Circles,
        },
        first: FirstAbilityKind::Extend,
        second: SecondAbilityKind::Immortality,
        third: ThirdAbilityKind::Neutralize,
    }
}

pub fn create_joiner(screen_name: &str, team: TeamChoiceKind) -> Joiner {
    Joiner {
        screen_name: String::from(screen_name),
        loadout: create_loadout(),
        team,
    }
}

pub fn create_member(
    member_id: MemberId,
    screen_name: &str,
    role: MemberRoleKind,
    team: Option<TeamKind>,
    is_alive: bool,
) -> Member {
    Member {
        member_id,
        screen_name: String::from(screen_name),
        role,
        loadout: Some(create_loadout()),
        team,
        score: Score::zero(),
        organism: is_alive.then(|| Organism::new(WorldPoint { x: 100, y: 100 })),
    }
}

/// Unconnected channels; the receivers are dropped.
pub fn create_game_handle(game_id: GameId, settings: GameSettings) -> GameHandle {
    let (request_sender, _): (mpsc::Sender<GameRequest>, mpsc::Receiver<GameRequest>) = mpsc::channel(1);
    let (input_sender, _): (mpsc::Sender<GameInput>, mpsc::Receiver<GameInput>) = mpsc::channel(1);
    let state: GameState = GameState::new(settings.clone(), FIXTURE_SEED);
    let (_, summary_receiver): (watch::Sender<GameSummary>, watch::Receiver<GameSummary>) =
        watch::channel(GameSummary::from_game_state(game_id, &state, false));

    GameHandle {
        requests: request_sender,
        inputs: input_sender,
        summary: summary_receiver,
        password_hash: None,
        settings,
    }
}

/// Empty, and unique to the test process and `name`.
pub fn create_temporary_directory(name: &str) -> PathBuf {
    let directory: PathBuf = std::env::temp_dir().join(format!("bacter-server-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("the temporary directory is created");

    directory
}
