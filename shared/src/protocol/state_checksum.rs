use crate::game::GameState;
use crate::protocol;
use crate::protocol::GameStateSerialOut;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a 64 over the wire encoding of the state.
pub fn get_state_checksum(state: &GameState) -> u64 {
    let state_bytes: Vec<u8> = protocol::encode_game_state(&GameStateSerialOut::from(state));

    get_fnv1a_64_digest(&state_bytes)
}

pub fn get_fnv1a_64_digest(bytes: &[u8]) -> u64 {
    let mut digest: u64 = FNV_OFFSET_BASIS;

    for byte in bytes {
        digest ^= u64::from(*byte);
        digest = digest.wrapping_mul(FNV_PRIME);
    }

    digest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::{LatticeCoordinate, WorldPoint};
    use crate::member::MemberId;
    use crate::organism::CellOccupancy;
    use crate::world::WorldShapeKind;

    fn create_state() -> GameState {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        state.members.insert(
            MemberId(0),
            test_fixture::create_participant_with_organism(MemberId(0), WorldPoint { x: 100, y: 100 }),
        );

        state
    }

    #[test]
    fn get_fnv1a_64_digest_matches_the_reference_vectors() {
        assert_eq!(get_fnv1a_64_digest(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(get_fnv1a_64_digest(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(get_fnv1a_64_digest(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn get_state_checksum_is_equal_for_equal_states() {
        assert_eq!(get_state_checksum(&create_state()), get_state_checksum(&create_state()));
    }

    #[test]
    fn get_state_checksum_covers_the_random_number_generator() {
        let state: GameState = create_state();
        let mut drawn_state: GameState = create_state();
        drawn_state.rng.next_u32();

        assert_ne!(get_state_checksum(&state), get_state_checksum(&drawn_state));
    }

    #[test]
    fn get_state_checksum_covers_the_cells() {
        let state: GameState = create_state();
        let mut grown_state: GameState = create_state();
        test_fixture::get_organism_mut(&mut grown_state, MemberId(0))
            .cells
            .insert(LatticeCoordinate { i: 1, j: 0 });

        assert_ne!(get_state_checksum(&state), get_state_checksum(&grown_state));
    }

    #[test]
    fn get_state_checksum_ignores_the_insertion_order_of_retightened_cells() {
        let mut first_state: GameState = create_state();
        let mut second_state: GameState = create_state();

        for (i, j) in [(1, 0), (0, 1), (-4, -4)] {
            test_fixture::get_organism_mut(&mut first_state, MemberId(0))
                .cells
                .insert(LatticeCoordinate { i, j });
        }

        for (i, j) in [(0, 1), (1, 0)] {
            test_fixture::get_organism_mut(&mut second_state, MemberId(0))
                .cells
                .insert(LatticeCoordinate { i, j });
        }

        let first_cells: &mut CellOccupancy = &mut test_fixture::get_organism_mut(&mut first_state, MemberId(0)).cells;
        first_cells.remove(LatticeCoordinate { i: -4, j: -4 });
        first_cells.retighten();
        test_fixture::get_organism_mut(&mut second_state, MemberId(0)).cells.retighten();

        assert_eq!(get_state_checksum(&first_state), get_state_checksum(&second_state));
    }
}
