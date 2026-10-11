use std::sync::atomic::Ordering;

use bytes::Bytes;

use shared::game::GameSummary;
use shared::protocol::{GameSummarySerialOut, MessageSerialOut};

use crate::registries::ServerRegistries;
use crate::socket::connection_model;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameList {
    /// In ascending game id.
    pub game_summaries: Vec<GameSummary>,
    pub online_client_count: u32,
}

impl GameList {
    pub fn encode_frame(&self) -> Bytes {
        connection_model::encode_frame(&MessageSerialOut::GameList {
            game_summaries: self.game_summaries.iter().map(GameSummarySerialOut::from).collect(),
            online_client_count: self.online_client_count,
        })
    }
}

pub fn get_game_list(registries: ServerRegistries) -> GameList {
    let mut game_summaries: Vec<GameSummary> =
        registries.games.iter().map(|entry| entry.value().summary.borrow().clone()).collect();
    game_summaries.sort_by_key(|game_summary| game_summary.game_id);

    GameList {
        game_summaries,
        online_client_count: registries.connection_count.load(Ordering::SeqCst),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use shared::game::{GameId, GameModeKind, GameSettings};
    use shared::protocol;

    #[test]
    fn get_game_list_lists_every_game_in_game_id_order_with_the_client_count() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        let settings: GameSettings = test_fixture::create_settings(GameModeKind::FreeForAll, 8);

        for game_id in [GameId(9), GameId(2), GameId(5)] {
            registries.games.insert(game_id, test_fixture::create_game_handle(game_id, settings.clone()));
        }

        registries.connection_count.store(3, Ordering::SeqCst);
        let game_list: GameList = get_game_list(registries);

        assert_eq!(
            game_list.game_summaries.iter().map(|game_summary| game_summary.game_id).collect::<Vec<GameId>>(),
            vec![GameId(2), GameId(5), GameId(9)],
        );
        assert_eq!(game_list.online_client_count, 3);
    }

    #[test]
    fn encode_frame_encodes_a_game_list_message() {
        let game_list: GameList = GameList {
            game_summaries: Vec::new(),
            online_client_count: 4,
        };

        assert_eq!(
            protocol::decode_message_out(&game_list.encode_frame()).unwrap(),
            MessageSerialOut::GameList {
                game_summaries: Vec::new(),
                online_client_count: 4,
            },
        );
    }
}
