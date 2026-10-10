use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use shared::ability::{AbilityPressSet, AimVector, FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use shared::game;
use shared::game::{GameModeKind, GameSettings, GameState, InputBundle, MemberEvent, PlayerTickInput, Tick};
use shared::geometry::WorldPoint;
use shared::member::{Appearance, MemberId, MemberRoleKind, OrganismColorKind, SkinKind};
use shared::protocol::state_checksum;
use shared::random::Pcg32;
use shared::world::WorldShapeKind;

const SCRIPTED_TICK_COUNT: u32 = 10_000;
const SCRIPTED_PLAYER_COUNT: u32 = 16;
const SCRIPTED_PLAYER_MINIMUM: u8 = 4;
const SCRIPTED_LEADERBOARD_LENGTH: u8 = 10;
const GAME_SEED: u64 = 0x5c41_97ed_0000_0001;
const SCRIPT_SEED: u64 = 0x5c41_97ed_0000_0002;
const WORLD_SIZE_PIXELS: u32 = 800;
const CURSOR_TARGET_MARGIN_PIXELS: u32 = 100;
const CURSOR_STEP_PIXELS: i32 = 3;
const PRESS_CHANCE_DENOMINATOR: u32 = 32;
const AIM_EXTENT_PIXELS: u32 = 100;
const JOIN_TICK: Tick = Tick(1);
const LEAVE_TICK: Tick = Tick(4000);
const LEAVING_MEMBER_ID: MemberId = MemberId(3);
const REJOIN_TICK: Tick = Tick(4001);
const APPEARANCE_CHANGE_TICK: Tick = Tick(6000);
const APPEARANCE_CHANGING_MEMBER_ID: MemberId = MemberId(5);
const COLORS: [OrganismColorKind; 4] = [
    OrganismColorKind::Fire,
    OrganismColorKind::Leaf,
    OrganismColorKind::Lake,
    OrganismColorKind::Royal,
];
const SKINS: [SkinKind; 4] = [SkinKind::Grid, SkinKind::Circles, SkinKind::Ghost, SkinKind::None];
const PRESSES: [AbilityPressSet; 4] = [
    AbilityPressSet::FIRST,
    AbilityPressSet::SECOND,
    AbilityPressSet::THIRD,
    AbilityPressSet::FOURTH,
];

/// Players who wander towards random targets and press at random.
struct ScriptedPlayers {
    script_rng: Pcg32,
    cursor_targets: BTreeMap<MemberId, WorldPoint>,
}

impl ScriptedPlayers {
    fn new() -> ScriptedPlayers {
        ScriptedPlayers {
            script_rng: Pcg32::from_seed(SCRIPT_SEED),
            cursor_targets: BTreeMap::new(),
        }
    }

    fn create_bundle(&mut self, state: &GameState) -> InputBundle {
        let tick: Tick = state.tick.next();
        let member_events: Vec<MemberEvent> = get_member_events(state, tick);
        let mut player_inputs: Vec<PlayerTickInput> = Vec::new();

        for member in state.members.values() {
            let Some(organism) = &member.organism else {
                continue;
            };

            player_inputs.push(self.create_player_input(member.member_id, organism.cursor));
        }

        InputBundle {
            tick,
            member_events,
            player_inputs,
        }
    }

    fn create_player_input(&mut self, member_id: MemberId, cursor: WorldPoint) -> PlayerTickInput {
        let cursor_target: WorldPoint = self.get_cursor_target(member_id, cursor);
        let next_cursor: WorldPoint = WorldPoint {
            x: cursor.x + (cursor_target.x - cursor.x).clamp(-CURSOR_STEP_PIXELS, CURSOR_STEP_PIXELS),
            y: cursor.y + (cursor_target.y - cursor.y).clamp(-CURSOR_STEP_PIXELS, CURSOR_STEP_PIXELS),
        };
        let mut ability_presses: AbilityPressSet = AbilityPressSet::NONE;

        for press in PRESSES {
            let is_pressed: bool = self.script_rng.below(PRESS_CHANCE_DENOMINATOR) == 0;

            if is_pressed {
                ability_presses = ability_presses.with(press);
            }
        }

        let is_shot_pressed: bool =
            ability_presses.contains(AbilityPressSet::FIRST) || ability_presses.contains(AbilityPressSet::SECOND);
        let aim: Option<AimVector> = if is_shot_pressed { Some(self.draw_aim()) } else { None };

        PlayerTickInput {
            member_id,
            cursor: next_cursor,
            ability_presses,
            aim,
        }
    }

    /// A new target once the cursor reaches the current one.
    fn get_cursor_target(&mut self, member_id: MemberId, cursor: WorldPoint) -> WorldPoint {
        let current_target: Option<WorldPoint> = self.cursor_targets.get(&member_id).copied();

        if let Some(cursor_target) = current_target.filter(|cursor_target| *cursor_target != cursor) {
            return cursor_target;
        }

        let cursor_target: WorldPoint = WorldPoint {
            x: self.draw_target_coordinate(),
            y: self.draw_target_coordinate(),
        };
        self.cursor_targets.insert(member_id, cursor_target);

        cursor_target
    }

    fn draw_target_coordinate(&mut self) -> i32 {
        let span: u32 = WORLD_SIZE_PIXELS - 2 * CURSOR_TARGET_MARGIN_PIXELS;

        i32::try_from(CURSOR_TARGET_MARGIN_PIXELS + self.script_rng.below(span)).unwrap()
    }

    /// Includes the zero aim, which launches no shot.
    fn draw_aim(&mut self) -> AimVector {
        let x: i32 = i32::try_from(self.script_rng.below(2 * AIM_EXTENT_PIXELS + 1)).unwrap();
        let y: i32 = i32::try_from(self.script_rng.below(2 * AIM_EXTENT_PIXELS + 1)).unwrap();
        let extent: i32 = i32::try_from(AIM_EXTENT_PIXELS).unwrap();

        AimVector {
            x: i16::try_from(x - extent).unwrap(),
            y: i16::try_from(y - extent).unwrap(),
        }
    }
}

fn create_scripted_settings() -> GameSettings {
    GameSettings {
        title: String::from("Scripted game"),
        mode: GameModeKind::Survival,
        world_shape: WorldShapeKind::Rectangle,
        world_width_pixels: WORLD_SIZE_PIXELS,
        world_height_pixels: WORLD_SIZE_PIXELS,
        player_minimum: Some(SCRIPTED_PLAYER_MINIMUM),
        player_cap: u8::try_from(SCRIPTED_PLAYER_COUNT).unwrap(),
        team_count: None,
        leaderboard_length: SCRIPTED_LEADERBOARD_LENGTH,
    }
}

/// Bit 0 picks the first ability, bit 1 the second, bit 2 the third.
fn create_loadout(player_index: u32) -> Loadout {
    let appearance_count: u32 = u32::try_from(COLORS.len()).unwrap();
    let appearance_index: usize = usize::try_from(player_index % appearance_count).unwrap();

    Loadout {
        appearance: Appearance {
            color: COLORS[appearance_index],
            skin: SKINS[appearance_index],
        },
        first: if player_index & 1 == 0 {
            FirstAbilityKind::Extend
        } else {
            FirstAbilityKind::Compress
        },
        second: if player_index & 2 == 0 {
            SecondAbilityKind::Immortality
        } else {
            SecondAbilityKind::Freeze
        },
        third: if player_index & 4 == 0 {
            ThirdAbilityKind::Neutralize
        } else {
            ThirdAbilityKind::Toxin
        },
    }
}

fn create_joined_participant_events(member_id: MemberId) -> Vec<MemberEvent> {
    let loadout: Loadout = create_loadout(member_id.0);

    vec![
        MemberEvent::Joined {
            member_id,
            screen_name: format!("scripted player {}", member_id.0),
            role: MemberRoleKind::Participant,
            loadout: Some(loadout),
            team: None,
        },
        MemberEvent::SpawnRequested {
            member_id,
            loadout,
            team: None,
        },
    ]
}

fn get_member_events(state: &GameState, tick: Tick) -> Vec<MemberEvent> {
    match tick {
        JOIN_TICK => {
            let mut member_events: Vec<MemberEvent> = (0..SCRIPTED_PLAYER_COUNT)
                .flat_map(|player_index| create_joined_participant_events(MemberId(player_index)))
                .collect();

            member_events.push(MemberEvent::Joined {
                member_id: MemberId(SCRIPTED_PLAYER_COUNT),
                screen_name: String::from("scripted spectator"),
                role: MemberRoleKind::Spectator,
                loadout: None,
                team: None,
            });

            member_events
        }
        LEAVE_TICK => vec![MemberEvent::Left {
            member_id: LEAVING_MEMBER_ID,
        }],
        REJOIN_TICK => create_joined_participant_events(state.next_member_id),
        APPEARANCE_CHANGE_TICK => vec![MemberEvent::AppearanceChanged {
            member_id: APPEARANCE_CHANGING_MEMBER_ID,
            appearance: Appearance {
                color: OrganismColorKind::Hot,
                skin: SkinKind::Ghost,
            },
        }],
        _ => Vec::new(),
    }
}

/// The checksum of the state after each tick.
fn run_scripted_game() -> Vec<u64> {
    let mut state: GameState = GameState::new(create_scripted_settings(), GAME_SEED);
    let mut scripted_players: ScriptedPlayers = ScriptedPlayers::new();
    let mut checksums: Vec<u64> = Vec::new();

    for _ in 0..SCRIPTED_TICK_COUNT {
        let bundle: InputBundle = scripted_players.create_bundle(&state);

        game::step(&mut state, &bundle).unwrap();

        checksums.push(state_checksum::get_state_checksum(&state));
    }

    checksums
}

fn get_checksum_fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scripted_game.checksums")
}

/// One line per tick: the tick, then the checksum in 16 hexadecimal digits.
fn format_checksums(checksums: &[u64]) -> String {
    checksums.iter().zip(1_u32..).map(|(checksum, tick)| format!("{tick} {checksum:016x}\n")).collect()
}

#[test]
fn step_gives_identical_checksums_in_two_runs() {
    assert_eq!(run_scripted_game(), run_scripted_game());
}

#[test]
fn step_matches_the_golden_checksum_sequence() {
    let golden_checksums: String = fs::read_to_string(get_checksum_fixture_path()).unwrap();

    assert_eq!(format_checksums(&run_scripted_game()), golden_checksums);
}

#[test]
#[ignore = "rewrites the golden checksums; ./scripts/test/regenerate-replay-fixtures.sh"]
fn regenerate_scripted_game_checksums() {
    let fixture_path: PathBuf = get_checksum_fixture_path();

    fs::create_dir_all(fixture_path.parent().unwrap()).unwrap();
    fs::write(fixture_path, format_checksums(&run_scripted_game())).unwrap();
}
