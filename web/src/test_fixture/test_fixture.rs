use shared::ability::{FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use shared::game::{GameModeKind, GameSettings, GameState};
use shared::member::{Appearance, Member, MemberId, MemberRoleKind, OrganismColorKind, Score, SkinKind};
use shared::world::WorldShapeKind;

pub const FIXTURE_SEED: u64 = 0x5eed;

pub fn create_state(mode: GameModeKind) -> GameState {
    let player_minimum: Option<u8> = match mode {
        GameModeKind::Survival => Some(2),
        GameModeKind::FreeForAll | GameModeKind::Skirmish => None,
    };
    let team_count: Option<u8> = match mode {
        GameModeKind::Skirmish => Some(2),
        GameModeKind::FreeForAll | GameModeKind::Survival => None,
    };
    let settings: GameSettings = GameSettings {
        title: String::from("Fixture game"),
        mode,
        world_shape: WorldShapeKind::Rectangle,
        world_width_pixels: 800,
        world_height_pixels: 800,
        player_minimum,
        player_cap: 8,
        team_count,
        leaderboard_length: 10,
    };

    GameState::new(settings, FIXTURE_SEED)
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

pub fn create_participant(member_id: MemberId) -> Member {
    Member {
        member_id,
        screen_name: format!("player {}", member_id.0),
        role: MemberRoleKind::Participant,
        loadout: Some(create_loadout()),
        team: None,
        score: Score::zero(),
        organism: None,
    }
}

pub fn insert_member(state: &mut GameState, member: Member) {
    state.next_member_id = member.member_id.next().max(state.next_member_id);
    state.members.insert(member.member_id, member);
}
