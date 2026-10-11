use std::net::IpAddr;
use std::sync::LazyLock;
use std::sync::atomic::AtomicU32;

use dashmap::DashMap;

pub static CONNECTION_COUNT: AtomicU32 = AtomicU32::new(0);
pub static CONNECTIONS_PER_IP: LazyLock<DashMap<IpAddr, u32>> = LazyLock::new(DashMap::new);
