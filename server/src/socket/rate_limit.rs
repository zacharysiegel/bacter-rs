use std::time::{Duration, Instant};

pub const INPUT_RATE_PER_SECOND: f64 = 30.0;
pub const INPUT_BURST: f64 = 45.0;
pub const REQUEST_RATE_PER_SECOND: f64 = 4.0;
pub const REQUEST_BURST: f64 = 8.0;
pub const RATE_LIMIT_ABUSE_PERIOD: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TokenBucket {
    rate_per_second: f64,
    burst: f64,
    tokens: f64,
    refilled_at: Instant,
    /// Since the first refusal after the last token taken.
    refusing_since: Option<Instant>,
}

impl TokenBucket {
    /// Starts full.
    pub fn new(rate_per_second: f64, burst: f64, now: Instant) -> TokenBucket {
        TokenBucket {
            rate_per_second,
            burst,
            tokens: burst,
            refilled_at: now,
            refusing_since: None,
        }
    }

    pub fn create_input_bucket(now: Instant) -> TokenBucket {
        TokenBucket::new(INPUT_RATE_PER_SECOND, INPUT_BURST, now)
    }

    pub fn create_request_bucket(now: Instant) -> TokenBucket {
        TokenBucket::new(REQUEST_RATE_PER_SECOND, REQUEST_BURST, now)
    }

    pub fn take(&mut self, now: Instant) -> TokenTakeKind {
        self.refill(now);

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            self.refusing_since = None;

            return TokenTakeKind::Taken;
        }

        let refusing_since: Instant = *self.refusing_since.get_or_insert(now);

        if now.duration_since(refusing_since) >= RATE_LIMIT_ABUSE_PERIOD {
            return TokenTakeKind::Abused;
        }

        TokenTakeKind::Refused
    }

    fn refill(&mut self, now: Instant) {
        let elapsed_seconds: f64 = now.saturating_duration_since(self.refilled_at).as_secs_f64();
        self.tokens = (self.tokens + elapsed_seconds * self.rate_per_second).min(self.burst);
        self.refilled_at = now;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenTakeKind {
    Taken,
    Refused,
    /// Refused without a token taken for the whole abuse period.
    Abused,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn take_repeatedly(bucket: &mut TokenBucket, take_count: u32, now: Instant) -> Vec<TokenTakeKind> {
        (0..take_count).map(|_| bucket.take(now)).collect()
    }

    #[test]
    fn take_allows_the_burst_then_refuses() {
        let now: Instant = Instant::now();
        let mut bucket: TokenBucket = TokenBucket::create_request_bucket(now);

        let take_kinds: Vec<TokenTakeKind> = take_repeatedly(&mut bucket, 9, now);

        assert_eq!(take_kinds[..8], [TokenTakeKind::Taken; 8]);
        assert_eq!(take_kinds[8], TokenTakeKind::Refused);
    }

    #[test]
    fn take_refills_at_the_rate() {
        let start: Instant = Instant::now();
        let mut bucket: TokenBucket = TokenBucket::create_request_bucket(start);
        take_repeatedly(&mut bucket, 8, start);

        let after_half_a_second: Instant = start + Duration::from_millis(500);

        assert_eq!(
            take_repeatedly(&mut bucket, 3, after_half_a_second),
            vec![TokenTakeKind::Taken, TokenTakeKind::Taken, TokenTakeKind::Refused],
        );
    }

    #[test]
    fn take_refills_no_further_than_the_burst() {
        let start: Instant = Instant::now();
        let mut bucket: TokenBucket = TokenBucket::create_input_bucket(start);

        let take_kinds: Vec<TokenTakeKind> = take_repeatedly(&mut bucket, 46, start + Duration::from_secs(60));

        assert_eq!(take_kinds[..45], [TokenTakeKind::Taken; 45]);
        assert_eq!(take_kinds[45], TokenTakeKind::Refused);
    }

    #[test]
    fn take_reports_abuse_after_refusing_for_the_abuse_period() {
        let start: Instant = Instant::now();
        let mut bucket: TokenBucket = TokenBucket::new(0.0, 1.0, start);
        bucket.take(start);

        assert_eq!(bucket.take(start + Duration::from_secs(1)), TokenTakeKind::Refused);
        assert_eq!(
            bucket.take(start + Duration::from_millis(10_999)),
            TokenTakeKind::Refused
        );
        assert_eq!(bucket.take(start + Duration::from_secs(11)), TokenTakeKind::Abused);
    }

    #[test]
    fn take_forgets_refusals_once_a_token_is_taken() {
        let start: Instant = Instant::now();
        let mut bucket: TokenBucket = TokenBucket::new(0.1, 1.0, start);
        bucket.take(start);

        assert_eq!(bucket.take(start + Duration::from_secs(1)), TokenTakeKind::Refused);
        assert_eq!(bucket.take(start + Duration::from_millis(10_500)), TokenTakeKind::Taken);
        assert_eq!(bucket.take(start + Duration::from_secs(12)), TokenTakeKind::Refused);
    }
}
