use std::sync::LazyLock;

use bytes::Bytes;
use dashmap::DashMap;
use tokio::sync::mpsc;

use crate::socket::ConnectionId;

pub static LOBBY_SUBSCRIBERS: LazyLock<DashMap<ConnectionId, mpsc::Sender<Bytes>>> = LazyLock::new(DashMap::new);
