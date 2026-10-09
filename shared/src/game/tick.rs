pub const TICK_PERIOD_MILLISECONDS: u32 = 70;
const MILLISECONDS_PER_SECOND: u32 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tick(pub u32);

impl Tick {
    pub fn next(self) -> Tick {
        Tick(self.0.wrapping_add(1))
    }

    pub fn plus(self, tick_count: u32) -> Tick {
        Tick(self.0.wrapping_add(tick_count))
    }

    /// Zero when `earlier` is not before `self`.
    pub fn ticks_since(self, earlier: Tick) -> u32 {
        self.0.saturating_sub(earlier.0)
    }
}

/// Rounded to the nearest tick, halves up.
pub const fn get_ticks_from_milliseconds(milliseconds: u32) -> u32 {
    (milliseconds + TICK_PERIOD_MILLISECONDS / 2) / TICK_PERIOD_MILLISECONDS
}

pub fn get_seconds_rounded_up(tick_count: u32) -> u32 {
    let milliseconds: u64 = u64::from(tick_count) * u64::from(TICK_PERIOD_MILLISECONDS);
    let seconds: u64 = milliseconds.div_ceil(u64::from(MILLISECONDS_PER_SECOND));

    u32::try_from(seconds).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plus_adds_ticks() {
        assert_eq!(Tick(10).plus(57), Tick(67));
    }

    #[test]
    fn ticks_since_counts_forward_and_stops_at_zero() {
        assert_eq!(Tick(114).ticks_since(Tick(14)), 100);
        assert_eq!(Tick(3).ticks_since(Tick(9)), 0);
    }

    #[test]
    fn get_ticks_from_milliseconds_rounds_to_the_nearest_tick() {
        assert_eq!(get_ticks_from_milliseconds(4500), 64);
        assert_eq!(get_ticks_from_milliseconds(6000), 86);
        assert_eq!(get_ticks_from_milliseconds(35), 1);
        assert_eq!(get_ticks_from_milliseconds(34), 0);
    }

    #[test]
    fn get_seconds_rounded_up_takes_the_ceiling() {
        assert_eq!(get_seconds_rounded_up(114), 8);
        assert_eq!(get_seconds_rounded_up(100), 7);
        assert_eq!(get_seconds_rounded_up(101), 8);
        assert_eq!(get_seconds_rounded_up(0), 0);
    }
}
