use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use bytes::Bytes;
use tokio::sync::mpsc::error::TrySendError;

use shared::game::Tick;
use shared::member::MemberId;
use shared::protocol::CloseReasonKind;

use crate::game::ConnectionEndpoint;
use crate::socket::{CloseCauseKind, ConnectionControl, ConnectionId};

pub const SLOW_CONSUMER_LIMIT: Duration = Duration::from_secs(10);
pub const SNAPSHOT_MINIMUM_INTERVAL_TICKS: u32 = 29;

/// The member connections of a game, each fed through its own bounded membership queue.
#[derive(Default)]
pub struct Recipients {
    recipients: BTreeMap<MemberId, Recipient>,
}

impl Recipients {
    pub fn new() -> Recipients {
        Recipients::default()
    }

    pub fn is_empty(&self) -> bool {
        self.recipients.is_empty()
    }

    pub fn contains(&self, member_id: MemberId) -> bool {
        self.recipients.contains_key(&member_id)
    }

    /// `snapshot_tick` is the tick of the snapshot already queued for the member.
    pub fn add(&mut self, member_id: MemberId, endpoint: ConnectionEndpoint, snapshot_tick: Tick) {
        self.recipients.insert(
            member_id,
            Recipient {
                endpoint,
                delivery: DeliveryKind::Streaming,
                last_snapshot_tick: snapshot_tick,
                is_snapshot_requested: false,
            },
        );
    }

    pub fn remove(&mut self, member_id: MemberId) -> Option<ConnectionEndpoint> {
        self.recipients.remove(&member_id).map(|recipient| recipient.endpoint)
    }

    /// The connection closes with `cause`.
    pub fn close_member(&mut self, member_id: MemberId, cause: CloseCauseKind) -> Option<ConnectionEndpoint> {
        let endpoint: ConnectionEndpoint = self.remove(member_id)?;
        let _ = endpoint.control.send(ConnectionControl::Close(cause));

        Some(endpoint)
    }

    pub fn remove_connection(&mut self, connection_id: ConnectionId) -> Option<MemberId> {
        let member_id: MemberId = self
            .recipients
            .iter()
            .find(|(_, recipient)| recipient.endpoint.connection_id == connection_id)
            .map(|(member_id, _)| *member_id)?;
        self.recipients.remove(&member_id);

        Some(member_id)
    }

    /// Skips recipients awaiting a snapshot.
    pub fn send_to_member(&mut self, member_id: MemberId, frame: Bytes, now: Instant) {
        let Some(recipient) = self.recipients.get_mut(&member_id) else {
            return;
        };

        recipient.send(frame, now);
    }

    /// Skips recipients awaiting a snapshot.
    pub fn broadcast(&mut self, frame: &Bytes, now: Instant) {
        for recipient in self.recipients.values_mut() {
            recipient.send(frame.clone(), now);
        }
    }

    /// Repeats coalesce.
    pub fn request_snapshot(&mut self, member_id: MemberId) {
        let Some(recipient) = self.recipients.get_mut(&member_id) else {
            return;
        };

        recipient.is_snapshot_requested = true;
    }

    /// Requested snapshots whose minimum interval has passed, and slow consumers whose queue has drained below half.
    pub fn get_snapshot_member_ids(&self, tick: Tick) -> Vec<MemberId> {
        self.recipients
            .iter()
            .filter(|(_, recipient)| recipient.is_snapshot_due(tick))
            .map(|(member_id, _)| *member_id)
            .collect()
    }

    /// Bundles to the member resume after the snapshot.
    pub fn send_snapshot(&mut self, member_id: MemberId, snapshot_frame: Bytes, tick: Tick, now: Instant) {
        let Some(recipient) = self.recipients.get_mut(&member_id) else {
            return;
        };

        recipient.delivery = DeliveryKind::Streaming;
        recipient.is_snapshot_requested = false;
        recipient.last_snapshot_tick = tick;
        recipient.send(snapshot_frame, now);
    }

    /// Removes recipients whose connection is gone and slow consumers past the limit, which are told to close.
    pub fn remove_departed(&mut self, now: Instant) -> Vec<MemberId> {
        let departures: Vec<(MemberId, DepartureKind)> = self
            .recipients
            .iter()
            .filter_map(|(member_id, recipient)| recipient.get_departure(now).map(|departure| (*member_id, departure)))
            .collect();

        for (member_id, departure) in &departures {
            match departure {
                DepartureKind::ConnectionGone => self.remove(*member_id),
                DepartureKind::SlowConsumer => {
                    self.close_member(*member_id, CloseCauseKind::Reason(CloseReasonKind::SlowConsumer))
                }
            };
        }

        departures.into_iter().map(|(member_id, _)| member_id).collect()
    }
}

struct Recipient {
    endpoint: ConnectionEndpoint,
    delivery: DeliveryKind,
    last_snapshot_tick: Tick,
    is_snapshot_requested: bool,
}

impl Recipient {
    fn send(&mut self, frame: Bytes, now: Instant) {
        if self.delivery != DeliveryKind::Streaming {
            return;
        }

        let send_result: Result<(), TrySendError<Bytes>> = self.endpoint.outbound.try_send(frame);

        if let Err(TrySendError::Full(_)) = send_result {
            self.delivery = DeliveryKind::AwaitingDrain { full_since: now };
        }
    }

    fn is_snapshot_due(&self, tick: Tick) -> bool {
        match self.delivery {
            DeliveryKind::Streaming => {
                self.is_snapshot_requested
                    && tick.ticks_since(self.last_snapshot_tick) >= SNAPSHOT_MINIMUM_INTERVAL_TICKS
            }
            DeliveryKind::AwaitingDrain { .. } => {
                self.endpoint.outbound.capacity() > self.endpoint.outbound.max_capacity() / 2
            }
        }
    }

    fn get_departure(&self, now: Instant) -> Option<DepartureKind> {
        if self.endpoint.outbound.is_closed() {
            return Some(DepartureKind::ConnectionGone);
        }

        let DeliveryKind::AwaitingDrain { full_since } = self.delivery else {
            return None;
        };

        if now.saturating_duration_since(full_since) >= SLOW_CONSUMER_LIMIT {
            return Some(DepartureKind::SlowConsumer);
        }

        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeliveryKind {
    Streaming,
    /// The queue was full; nothing more is queued until a fresh snapshot.
    AwaitingDrain {
        full_since: Instant,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DepartureKind {
    ConnectionGone,
    SlowConsumer,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::socket::Membership;
    use crate::socket::connection_model;

    const MEMBER_ID: MemberId = MemberId(3);

    fn add_recipient(recipients: &mut Recipients, member_id: MemberId, snapshot_tick: Tick) -> Membership {
        let (endpoint, membership): (ConnectionEndpoint, Membership) =
            connection_model::create_membership(ConnectionId(u64::from(member_id.0)));
        recipients.add(member_id, endpoint, snapshot_tick);

        membership
    }

    fn fill_queue(recipients: &mut Recipients, now: Instant) {
        for _ in 0..=connection_model::MEMBERSHIP_CHANNEL_CAPACITY {
            recipients.broadcast(&Bytes::from_static(b"bundle"), now);
        }
    }

    fn drain(membership: &mut Membership) -> Vec<Bytes> {
        let mut frames: Vec<Bytes> = Vec::new();

        while let Ok(frame) = membership.outbound.try_recv() {
            frames.push(frame);
        }

        frames
    }

    #[test]
    fn broadcast_queues_each_frame_for_every_recipient() {
        let mut recipients: Recipients = Recipients::new();
        let mut first_membership: Membership = add_recipient(&mut recipients, MemberId(0), Tick(0));
        let mut second_membership: Membership = add_recipient(&mut recipients, MemberId(1), Tick(0));

        recipients.broadcast(&Bytes::from_static(b"bundle"), Instant::now());
        recipients.send_to_member(MemberId(1), Bytes::from_static(b"correction"), Instant::now());

        assert_eq!(drain(&mut first_membership), vec![Bytes::from_static(b"bundle")]);
        assert_eq!(
            drain(&mut second_membership),
            vec![Bytes::from_static(b"bundle"), Bytes::from_static(b"correction")],
        );
    }

    #[test]
    fn get_snapshot_member_ids_waits_for_the_minimum_interval_and_coalesces_requests() {
        let mut recipients: Recipients = Recipients::new();
        let _membership: Membership = add_recipient(&mut recipients, MEMBER_ID, Tick(10));

        recipients.request_snapshot(MEMBER_ID);
        recipients.request_snapshot(MEMBER_ID);

        assert_eq!(recipients.get_snapshot_member_ids(Tick(38)), Vec::new());
        assert_eq!(recipients.get_snapshot_member_ids(Tick(39)), vec![MEMBER_ID]);

        recipients.send_snapshot(MEMBER_ID, Bytes::from_static(b"snapshot"), Tick(39), Instant::now());

        assert_eq!(recipients.get_snapshot_member_ids(Tick(100)), Vec::new());
    }

    #[test]
    fn broadcast_stops_queueing_for_a_full_queue_until_it_drains_and_gets_a_snapshot() {
        let mut recipients: Recipients = Recipients::new();
        let mut membership: Membership = add_recipient(&mut recipients, MEMBER_ID, Tick(0));
        let now: Instant = Instant::now();
        fill_queue(&mut recipients, now);

        assert_eq!(recipients.get_snapshot_member_ids(Tick(1)), Vec::new());

        for _ in 0..connection_model::MEMBERSHIP_CHANNEL_CAPACITY / 2 + 1 {
            membership.outbound.try_recv().unwrap();
        }

        assert_eq!(recipients.get_snapshot_member_ids(Tick(1)), vec![MEMBER_ID]);

        drain(&mut membership);
        recipients.broadcast(&Bytes::from_static(b"skipped"), now);
        recipients.send_snapshot(MEMBER_ID, Bytes::from_static(b"snapshot"), Tick(1), now);
        recipients.broadcast(&Bytes::from_static(b"bundle"), now);

        assert_eq!(
            drain(&mut membership),
            vec![Bytes::from_static(b"snapshot"), Bytes::from_static(b"bundle")],
        );
        assert_eq!(recipients.remove_departed(now + SLOW_CONSUMER_LIMIT), Vec::new());
    }

    #[test]
    fn remove_departed_closes_a_slow_consumer_still_full_after_the_limit() {
        let mut recipients: Recipients = Recipients::new();
        let membership: Membership = add_recipient(&mut recipients, MEMBER_ID, Tick(0));
        let now: Instant = Instant::now();
        fill_queue(&mut recipients, now);

        assert_eq!(
            recipients.remove_departed(now + Duration::from_millis(9_999)),
            Vec::new()
        );
        assert_eq!(recipients.remove_departed(now + SLOW_CONSUMER_LIMIT), vec![MEMBER_ID]);
        assert!(!recipients.contains(MEMBER_ID));
        assert_eq!(
            *membership.control.borrow(),
            ConnectionControl::Close(CloseCauseKind::Reason(CloseReasonKind::SlowConsumer)),
        );
    }

    #[test]
    fn remove_departed_removes_a_recipient_whose_connection_is_gone() {
        let mut recipients: Recipients = Recipients::new();
        let membership: Membership = add_recipient(&mut recipients, MEMBER_ID, Tick(0));
        let _other_membership: Membership = add_recipient(&mut recipients, MemberId(4), Tick(0));

        drop(membership);

        assert_eq!(recipients.remove_departed(Instant::now()), vec![MEMBER_ID]);
        assert!(recipients.contains(MemberId(4)));
    }

    #[test]
    fn remove_connection_finds_the_member_of_a_connection() {
        let mut recipients: Recipients = Recipients::new();
        let _membership: Membership = add_recipient(&mut recipients, MEMBER_ID, Tick(0));

        assert_eq!(recipients.remove_connection(ConnectionId(9)), None);
        assert_eq!(recipients.remove_connection(ConnectionId(3)), Some(MEMBER_ID));
        assert!(recipients.is_empty());
    }
}
