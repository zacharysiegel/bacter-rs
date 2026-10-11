use crate::game::tick;

pub const FRAME_DURATION_LIMIT_MILLISECONDS: f64 = 250.0;
pub const LOCAL_TICKS_PER_FRAME_LIMIT: u32 = 4;
pub const TICK_PERIOD_MILLISECONDS: f64 = tick::TICK_PERIOD_MILLISECONDS as f64;

#[derive(Clone, Debug, PartialEq)]
pub struct FramePacer {
    previous_frame_at_milliseconds: Option<f64>,
    local_tick_accumulator_milliseconds: f64,
    tick_applied_at_milliseconds: Option<f64>,
}

impl FramePacer {
    pub fn new() -> FramePacer {
        FramePacer {
            previous_frame_at_milliseconds: None,
            local_tick_accumulator_milliseconds: 0.0,
            tick_applied_at_milliseconds: None,
        }
    }

    /// Zero for the first frame and for a clock which went backwards.
    pub fn begin_frame(&mut self, now_milliseconds: f64) -> f64 {
        let frame_duration_milliseconds: f64 = match self.previous_frame_at_milliseconds {
            Some(previous_frame_at_milliseconds) => {
                (now_milliseconds - previous_frame_at_milliseconds).clamp(0.0, FRAME_DURATION_LIMIT_MILLISECONDS)
            }
            None => 0.0,
        };

        self.previous_frame_at_milliseconds = Some(now_milliseconds);

        frame_duration_milliseconds
    }

    /// Ticks due beyond the per-frame limit are dropped; the part of a tick not yet due carries over.
    pub fn take_local_tick_count(&mut self, frame_duration_milliseconds: f64) -> u32 {
        self.local_tick_accumulator_milliseconds += frame_duration_milliseconds;

        let due_tick_count: f64 = (self.local_tick_accumulator_milliseconds / TICK_PERIOD_MILLISECONDS).floor();
        self.local_tick_accumulator_milliseconds -= due_tick_count * TICK_PERIOD_MILLISECONDS;

        (due_tick_count as u32).min(LOCAL_TICKS_PER_FRAME_LIMIT)
    }

    pub fn record_tick_applied(&mut self, now_milliseconds: f64) {
        self.tick_applied_at_milliseconds = Some(now_milliseconds);
    }

    /// 1 before the first applied tick.
    pub fn get_interpolation_alpha(&self, now_milliseconds: f64) -> f32 {
        let Some(tick_applied_at_milliseconds) = self.tick_applied_at_milliseconds else {
            return 1.0;
        };

        let alpha: f64 = (now_milliseconds - tick_applied_at_milliseconds) / TICK_PERIOD_MILLISECONDS;

        alpha.clamp(0.0, 1.0) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_frame_gives_zero_for_the_first_frame_then_the_elapsed_time() {
        let mut pacer: FramePacer = FramePacer::new();

        assert_eq!(pacer.begin_frame(5000.0), 0.0);
        assert_eq!(pacer.begin_frame(5016.5), 16.5);
    }

    #[test]
    fn begin_frame_clamps_a_long_frame_to_the_frame_duration_limit() {
        let mut pacer: FramePacer = FramePacer::new();
        pacer.begin_frame(0.0);

        assert_eq!(pacer.begin_frame(9000.0), FRAME_DURATION_LIMIT_MILLISECONDS);
        assert_eq!(pacer.begin_frame(8000.0), 0.0);
    }

    #[test]
    fn take_local_tick_count_carries_the_remainder_to_the_next_frame() {
        let mut pacer: FramePacer = FramePacer::new();

        let local_tick_counts: Vec<u32> = [100.0, 50.0, 59.0, 1.0]
            .into_iter()
            .map(|frame_duration_milliseconds| pacer.take_local_tick_count(frame_duration_milliseconds))
            .collect();

        assert_eq!(local_tick_counts, vec![1, 1, 0, 1]);
    }

    #[test]
    fn take_local_tick_count_drops_the_ticks_beyond_the_per_frame_limit() {
        let mut pacer: FramePacer = FramePacer::new();

        assert_eq!(pacer.take_local_tick_count(1000.0), LOCAL_TICKS_PER_FRAME_LIMIT);
        assert_eq!(pacer.take_local_tick_count(49.0), 0);
        assert_eq!(pacer.take_local_tick_count(1.0), 1);
    }

    #[test]
    fn get_interpolation_alpha_is_the_proportion_of_a_tick_since_the_last_applied_tick() {
        let mut pacer: FramePacer = FramePacer::new();

        assert_eq!(pacer.get_interpolation_alpha(1000.0), 1.0);

        pacer.record_tick_applied(1000.0);

        assert_eq!(pacer.get_interpolation_alpha(1035.0), 0.5);
        assert_eq!(pacer.get_interpolation_alpha(2000.0), 1.0);
        assert_eq!(pacer.get_interpolation_alpha(900.0), 0.0);
    }
}
