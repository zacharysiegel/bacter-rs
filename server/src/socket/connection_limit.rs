use std::net::IpAddr;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU32, Ordering};

use dashmap::DashMap;
use dashmap::mapref::entry::Entry;

use shared::protocol::protocol_limits;

use crate::registries::ServerRegistries;

pub static CONNECTION_COUNT: AtomicU32 = AtomicU32::new(0);
pub static CONNECTIONS_PER_IP: LazyLock<DashMap<IpAddr, u32>> = LazyLock::new(DashMap::new);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionLimitReached;

/// Counts the connection against the server and its IP; nothing stays reserved on `Err`.
pub fn reserve_connection(registries: ServerRegistries, ip: IpAddr) -> Result<(), ConnectionLimitReached> {
    let previous_connection_count: u32 = registries.connection_count.fetch_add(1, Ordering::SeqCst);

    if previous_connection_count >= protocol_limits::CONNECTION_LIMIT_PER_SERVER {
        release_server_connection(registries);

        return Err(ConnectionLimitReached);
    }

    let ip_reservation: Result<(), ConnectionLimitReached> = reserve_ip_connection(registries, ip);

    if ip_reservation.is_err() {
        release_server_connection(registries);
    }

    ip_reservation
}

pub fn release_connection(registries: ServerRegistries, ip: IpAddr) {
    release_server_connection(registries);

    let Entry::Occupied(mut occupied_entry) = registries.connections_per_ip.entry(ip) else {
        return;
    };

    if *occupied_entry.get() <= 1 {
        occupied_entry.remove();
    } else {
        *occupied_entry.get_mut() -= 1;
    }
}

fn release_server_connection(registries: ServerRegistries) {
    registries.connection_count.fetch_sub(1, Ordering::SeqCst);
}

fn reserve_ip_connection(registries: ServerRegistries, ip: IpAddr) -> Result<(), ConnectionLimitReached> {
    match registries.connections_per_ip.entry(ip) {
        Entry::Occupied(mut occupied_entry) => {
            if *occupied_entry.get() >= protocol_limits::CONNECTION_LIMIT_PER_IP {
                return Err(ConnectionLimitReached);
            }

            *occupied_entry.get_mut() += 1;

            Ok(())
        }
        Entry::Vacant(vacant_entry) => {
            vacant_entry.insert(1);

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 7));
    const OTHER_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 8));

    #[test]
    fn reserve_connection_rejects_beyond_the_per_ip_limit_only_for_that_ip() {
        let registries: ServerRegistries = ServerRegistries::leaked();

        for _ in 0..protocol_limits::CONNECTION_LIMIT_PER_IP {
            reserve_connection(registries, IP).unwrap();
        }

        assert_eq!(reserve_connection(registries, IP), Err(ConnectionLimitReached));
        assert_eq!(reserve_connection(registries, OTHER_IP), Ok(()));
        assert_eq!(
            registries.connection_count.load(Ordering::SeqCst),
            protocol_limits::CONNECTION_LIMIT_PER_IP + 1,
        );
    }

    #[test]
    fn reserve_connection_rejects_beyond_the_server_limit_without_counting_the_ip() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        registries.connection_count.store(protocol_limits::CONNECTION_LIMIT_PER_SERVER, Ordering::SeqCst);

        assert_eq!(reserve_connection(registries, IP), Err(ConnectionLimitReached));
        assert_eq!(
            registries.connection_count.load(Ordering::SeqCst),
            protocol_limits::CONNECTION_LIMIT_PER_SERVER,
        );
        assert!(registries.connections_per_ip.is_empty());
    }

    #[test]
    fn release_connection_frees_the_slots_and_forgets_an_ip_without_connections() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        reserve_connection(registries, IP).unwrap();
        reserve_connection(registries, IP).unwrap();

        release_connection(registries, IP);

        assert_eq!(*registries.connections_per_ip.get(&IP).unwrap(), 1);

        release_connection(registries, IP);

        assert!(registries.connections_per_ip.is_empty());
        assert_eq!(registries.connection_count.load(Ordering::SeqCst), 0);
    }
}
