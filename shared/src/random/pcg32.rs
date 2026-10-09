pub const PCG32_STREAM: u64 = 0xda3e_39cb_94b9_5bdb;
const PCG32_MULTIPLIER: u64 = 6_364_136_223_846_793_005;

/// PCG32 XSH-RR.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pcg32 {
    state: u64,
    increment: u64,
}

impl Pcg32 {
    pub fn from_seed(seed: u64) -> Pcg32 {
        Pcg32::from_seed_and_stream(seed, PCG32_STREAM)
    }

    /// The reference `pcg32_srandom_r(initial_state, stream)`.
    pub fn from_seed_and_stream(initial_state: u64, stream: u64) -> Pcg32 {
        let mut generator: Pcg32 = Pcg32 {
            state: 0,
            increment: (stream << 1) | 1,
        };

        generator.advance();
        generator.state = generator.state.wrapping_add(initial_state);
        generator.advance();

        generator
    }

    /// `None` when `increment` is even.
    pub fn from_parts(state: u64, increment: u64) -> Option<Pcg32> {
        if increment % 2 == 0 {
            return None;
        }

        Some(Pcg32 { state, increment })
    }

    pub fn state(&self) -> u64 {
        self.state
    }

    pub fn increment(&self) -> u64 {
        self.increment
    }

    pub fn next_u32(&mut self) -> u32 {
        let previous_state: u64 = self.state;
        self.advance();

        // Truncation to the low 32 bits is the XSH step of the reference output function.
        let xor_shifted: u32 = (((previous_state >> 18) ^ previous_state) >> 27) as u32;
        let rotation: u32 = (previous_state >> 59) as u32;

        xor_shifted.rotate_right(rotation)
    }

    /// Uniform in `[0, bound)` by multiply-high; `bound` 0 gives 0.
    pub fn below(&mut self, bound: u32) -> u32 {
        let draw: u64 = u64::from(self.next_u32());

        ((draw * u64::from(bound)) >> 32) as u32
    }

    fn advance(&mut self) {
        self.state = self.state.wrapping_mul(PCG32_MULTIPLIER).wrapping_add(self.increment);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_seed_and_stream_matches_reference_vectors() {
        let mut generator: Pcg32 = Pcg32::from_seed_and_stream(42, 54);
        let draws: Vec<u32> = (0..6).map(|_| generator.next_u32()).collect();

        assert_eq!(
            draws,
            vec![0xa15c02b7, 0x7b47f409, 0xba1d3330, 0x83d2f293, 0xbfa4784b, 0xcbed606e],
        );
    }

    #[test]
    fn from_seed_matches_golden_sequence() {
        let mut generator: Pcg32 = Pcg32::from_seed(0x0123_4567_89ab_cdef);
        let draws: Vec<u32> = (0..8).map(|_| generator.next_u32()).collect();

        assert_eq!(
            draws,
            vec![
                0x683ae4b0, 0x465b94e2, 0x8e78504b, 0x716c5c5d, 0x086b6029, 0x1aa1bf3a, 0x32af10fa, 0xa01c2abc,
            ],
        );
    }

    #[test]
    fn below_takes_the_high_word_of_the_scaled_draw() {
        let mut generator: Pcg32 = Pcg32::from_seed_and_stream(42, 54);

        assert_eq!(generator.below(100), 63);
        assert_eq!(generator.below(100), 48);
        assert_eq!(generator.below(1), 0);
    }

    #[test]
    fn from_parts_rejects_even_increment() {
        assert_eq!(Pcg32::from_parts(7, 10), None);
    }

    #[test]
    fn from_parts_restores_a_generator_mid_sequence() {
        let mut generator: Pcg32 = Pcg32::from_seed(99);
        generator.next_u32();

        let mut restored: Pcg32 = Pcg32::from_parts(generator.state(), generator.increment()).unwrap();

        assert_eq!(restored.next_u32(), generator.next_u32());
        assert_eq!(restored, generator);
    }
}
