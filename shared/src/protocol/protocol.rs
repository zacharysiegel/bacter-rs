use crate::protocol::GameStateSerialOut;

/// Bumped on any change to a wire type.
pub const PROTOCOL_VERSION: u16 = 1;

pub fn encode_game_state(state_serial_out: &GameStateSerialOut) -> Vec<u8> {
    bitcode::encode(state_serial_out)
}
