use crate::ability::{FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use crate::game::{GameModeKind, GameSettings, GameState};
use crate::geometry::WorldPoint;
use crate::member::{Appearance, Member, MemberId, MemberRoleKind, OrganismColorKind, Score, SkinKind};
use crate::organism::Organism;
use crate::world::WorldShapeKind;

pub const FIXTURE_SEED: u64 = 0x5eed;

pub fn create_settings(mode: GameModeKind, world_shape: WorldShapeKind, world_size_pixels: u32) -> GameSettings {
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
        world_shape,
        world_width_pixels: world_size_pixels,
        world_height_pixels: world_size_pixels,
        player_minimum,
        player_cap: 8,
        team_count,
        leaderboard_length: 10,
    }
}

pub fn create_state(mode: GameModeKind, world_shape: WorldShapeKind, world_size_pixels: u32) -> GameState {
    GameState::new(create_settings(mode, world_shape, world_size_pixels), FIXTURE_SEED)
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

pub fn create_participant_with_organism(member_id: MemberId, position: WorldPoint) -> Member {
    let mut member: Member = create_participant(member_id);
    member.organism = Some(Organism::new(position));

    member
}

pub fn get_organism(state: &GameState, member_id: MemberId) -> &Organism {
    state.members[&member_id].organism.as_ref().unwrap()
}

pub fn get_organism_mut(state: &mut GameState, member_id: MemberId) -> &mut Organism {
    state.members.get_mut(&member_id).unwrap().organism.as_mut().unwrap()
}
