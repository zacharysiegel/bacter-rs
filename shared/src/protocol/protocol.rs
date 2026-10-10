use crate::protocol::GameStateSerialOut;

pub fn encode_game_state(state_serial_out: &GameStateSerialOut) -> Vec<u8> {
    bitcode::encode(state_serial_out)
}
