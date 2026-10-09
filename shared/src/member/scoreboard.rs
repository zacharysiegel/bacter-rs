#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Score {
    pub kills: u32,
    pub deaths: u32,
    pub wins: u32,
}

impl Score {
    pub fn zero() -> Score {
        Score {
            kills: 0,
            deaths: 0,
            wins: 0,
        }
    }
}
