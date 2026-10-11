use std::collections::VecDeque;
use std::net::IpAddr;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use dashmap::mapref::entry::Entry;
use dashmap::mapref::one::RefMut;
use shared::protocol::protocol_limits;

pub static PASSWORD_FAILURES_PER_IP: LazyLock<DashMap<IpAddr, PasswordFailureWindow>> = LazyLock::new(DashMap::new);

const PASSWORD_FAILURE_WINDOW: Duration = Duration::from_secs(protocol_limits::PASSWORD_FAILURE_WINDOW_SECONDS);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PasswordFailureWindow {
    /// Oldest first.
    failure_instants: VecDeque<Instant>,
}

impl PasswordFailureWindow {
    fn forget_expired(&mut self, now: Instant) {
        while let Some(oldest_failure_instant) = self.failure_instants.front().copied() {
            if now.saturating_duration_since(oldest_failure_instant) < PASSWORD_FAILURE_WINDOW {
                break;
            }

            self.failure_instants.pop_front();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasswordAttemptKind {
    Allowed,
    RateLimited,
}

pub fn check_attempt(
    password_failures_per_ip: &DashMap<IpAddr, PasswordFailureWindow>,
    ip: IpAddr,
    now: Instant,
) -> PasswordAttemptKind {
    let Entry::Occupied(mut occupied_entry) = password_failures_per_ip.entry(ip) else {
        return PasswordAttemptKind::Allowed;
    };

    occupied_entry.get_mut().forget_expired(now);
    let failure_count: u32 = u32::try_from(occupied_entry.get().failure_instants.len()).unwrap_or(u32::MAX);

    if failure_count == 0 {
        occupied_entry.remove();
    }

    if failure_count >= protocol_limits::PASSWORD_FAILURE_LIMIT_PER_IP {
        return PasswordAttemptKind::RateLimited;
    }

    PasswordAttemptKind::Allowed
}

pub fn record_failure(password_failures_per_ip: &DashMap<IpAddr, PasswordFailureWindow>, ip: IpAddr, now: Instant) {
    let mut window: RefMut<'_, IpAddr, PasswordFailureWindow> = password_failures_per_ip.entry(ip).or_default();
    window.forget_expired(now);
    window.failure_instants.push_back(now);
}

pub fn forget_expired_failures(password_failures_per_ip: &DashMap<IpAddr, PasswordFailureWindow>, now: Instant) {
    password_failures_per_ip.retain(|_, window| {
        window.forget_expired(now);

        !window.failure_instants.is_empty()
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 7));
    const OTHER_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 8));

    fn record_failures(failures: &DashMap<IpAddr, PasswordFailureWindow>, failure_count: u32, now: Instant) {
        for _ in 0..failure_count {
            record_failure(failures, IP, now);
        }
    }

    #[test]
    fn check_attempt_allows_an_ip_below_the_failure_limit() {
        let failures: DashMap<IpAddr, PasswordFailureWindow> = DashMap::new();
        let now: Instant = Instant::now();
        record_failures(&failures, protocol_limits::PASSWORD_FAILURE_LIMIT_PER_IP - 1, now);

        assert_eq!(check_attempt(&failures, IP, now), PasswordAttemptKind::Allowed);
    }

    #[test]
    fn check_attempt_limits_an_ip_at_the_failure_limit_only() {
        let failures: DashMap<IpAddr, PasswordFailureWindow> = DashMap::new();
        let now: Instant = Instant::now();
        record_failures(&failures, protocol_limits::PASSWORD_FAILURE_LIMIT_PER_IP, now);

        assert_eq!(check_attempt(&failures, IP, now), PasswordAttemptKind::RateLimited);
        assert_eq!(check_attempt(&failures, OTHER_IP, now), PasswordAttemptKind::Allowed);
    }

    #[test]
    fn check_attempt_forgets_failures_older_than_the_window() {
        let failures: DashMap<IpAddr, PasswordFailureWindow> = DashMap::new();
        let start: Instant = Instant::now();
        let half_limit: u32 = protocol_limits::PASSWORD_FAILURE_LIMIT_PER_IP / 2;
        let later: Instant = start + PASSWORD_FAILURE_WINDOW / 2;
        record_failures(&failures, half_limit, start);
        record_failures(&failures, protocol_limits::PASSWORD_FAILURE_LIMIT_PER_IP - half_limit, later);

        assert_eq!(
            check_attempt(&failures, IP, start + PASSWORD_FAILURE_WINDOW - Duration::from_secs(1)),
            PasswordAttemptKind::RateLimited
        );
        assert_eq!(
            check_attempt(&failures, IP, start + PASSWORD_FAILURE_WINDOW),
            PasswordAttemptKind::Allowed
        );
    }

    #[test]
    fn check_attempt_removes_an_ip_whose_failures_all_expired() {
        let failures: DashMap<IpAddr, PasswordFailureWindow> = DashMap::new();
        let start: Instant = Instant::now();
        record_failures(&failures, 1, start);

        check_attempt(&failures, IP, start + PASSWORD_FAILURE_WINDOW);

        assert!(failures.is_empty());
    }

    #[test]
    fn forget_expired_failures_removes_only_ips_whose_failures_all_expired() {
        let failures: DashMap<IpAddr, PasswordFailureWindow> = DashMap::new();
        let start: Instant = Instant::now();
        let later: Instant = start + Duration::from_secs(1);
        record_failure(&failures, IP, start);
        record_failure(&failures, OTHER_IP, start);
        record_failure(&failures, OTHER_IP, later);

        forget_expired_failures(&failures, start + PASSWORD_FAILURE_WINDOW);

        assert!(!failures.contains_key(&IP));
        assert_eq!(failures.get(&OTHER_IP).unwrap().failure_instants, VecDeque::from([later]));
    }
}
