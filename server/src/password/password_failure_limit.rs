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
    failed_at: VecDeque<Instant>,
}

impl PasswordFailureWindow {
    fn forget_expired(&mut self, now: Instant) {
        while let Some(oldest_failed_at) = self.failed_at.front().copied() {
            if now.saturating_duration_since(oldest_failed_at) < PASSWORD_FAILURE_WINDOW {
                break;
            }

            self.failed_at.pop_front();
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
    let failure_count: u32 = u32::try_from(occupied_entry.get().failed_at.len()).unwrap_or(u32::MAX);

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
    window.failed_at.push_back(now);
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
        record_failures(&failures, 19, now);

        assert_eq!(check_attempt(&failures, IP, now), PasswordAttemptKind::Allowed);
    }

    #[test]
    fn check_attempt_limits_an_ip_at_the_failure_limit_only() {
        let failures: DashMap<IpAddr, PasswordFailureWindow> = DashMap::new();
        let now: Instant = Instant::now();
        record_failures(&failures, 20, now);

        assert_eq!(check_attempt(&failures, IP, now), PasswordAttemptKind::RateLimited);
        assert_eq!(check_attempt(&failures, OTHER_IP, now), PasswordAttemptKind::Allowed);
    }

    #[test]
    fn check_attempt_forgets_failures_older_than_the_window() {
        let failures: DashMap<IpAddr, PasswordFailureWindow> = DashMap::new();
        let start: Instant = Instant::now();
        record_failures(&failures, 10, start);
        record_failures(&failures, 10, start + Duration::from_secs(300));

        assert_eq!(
            check_attempt(&failures, IP, start + Duration::from_secs(599)),
            PasswordAttemptKind::RateLimited
        );
        assert_eq!(
            check_attempt(&failures, IP, start + Duration::from_secs(600)),
            PasswordAttemptKind::Allowed
        );
    }

    #[test]
    fn check_attempt_removes_an_ip_whose_failures_all_expired() {
        let failures: DashMap<IpAddr, PasswordFailureWindow> = DashMap::new();
        let start: Instant = Instant::now();
        record_failures(&failures, 1, start);

        check_attempt(&failures, IP, start + Duration::from_secs(600));

        assert!(failures.is_empty());
    }
}
