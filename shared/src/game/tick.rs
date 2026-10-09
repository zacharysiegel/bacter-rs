pub const TICK_PERIOD_MILLISECONDS: u32 = 70;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tick(pub u32);

impl Tick {
    pub fn next(self) -> Tick {
        Tick(self.0.wrapping_add(1))
    }
}
