use std::sync::LazyLock;

pub static GROWTH_CHANCE_TABLES: LazyLock<GrowthChanceTables> = LazyLock::new(GrowthChanceTables::build);

const DRAW_SPACE_SIZE: f64 = 4_294_967_296.0;
const ALWAYS_PASSES_THRESHOLD: u64 = 1 << 32;
const NEVER_PASSES_THRESHOLD: u64 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrowthStateKind {
    Default,
    Compressed,
    Extended,
}

impl GrowthStateKind {
    pub fn coefficient(self) -> f64 {
        match self {
            GrowthStateKind::Default => -27.5,
            GrowthStateKind::Compressed => -31.5,
            GrowthStateKind::Extended => -25.5,
        }
    }

    pub fn range_pixels(self) -> i64 {
        match self {
            GrowthStateKind::Default => 50,
            GrowthStateKind::Compressed => 40,
            GrowthStateKind::Extended => 70,
        }
    }

    pub fn range_squared(self) -> i64 {
        self.range_pixels() * self.range_pixels()
    }

    /// The largest `distance_squared` whose birth chance is not negative.
    pub fn birth_table_last_distance_squared(self) -> i64 {
        match self {
            GrowthStateKind::Default => 1365,
            GrowthStateKind::Compressed => 525,
            GrowthStateKind::Extended => 2448,
        }
    }
}

/// Exclusive u32 draw thresholds indexed by `distance_squared`: a draw `u` passes when `u < threshold`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrowthChanceTable {
    birth_thresholds: Vec<u64>,
    death_thresholds: Vec<u64>,
}

impl GrowthChanceTable {
    fn build(growth_state: GrowthStateKind) -> GrowthChanceTable {
        let birth_thresholds: Vec<u64> = (0..=growth_state.birth_table_last_distance_squared())
            .map(|distance_squared| get_threshold(get_birth_chance(growth_state, distance_squared)))
            .collect();
        let death_thresholds: Vec<u64> = (0..=growth_state.range_squared())
            .map(|distance_squared| get_threshold(get_death_chance(growth_state, distance_squared)))
            .collect();

        GrowthChanceTable {
            birth_thresholds,
            death_thresholds,
        }
    }

    pub fn birth_thresholds(&self) -> &[u64] {
        &self.birth_thresholds
    }

    pub fn death_thresholds(&self) -> &[u64] {
        &self.death_thresholds
    }

    /// Never passes past the end of the table.
    pub fn birth_passes(&self, distance_squared: i64, draw: u32) -> bool {
        let threshold: u64 = get_entry(&self.birth_thresholds, distance_squared).unwrap_or(NEVER_PASSES_THRESHOLD);

        u64::from(draw) < threshold
    }

    /// Always passes past the end of the table, where death is forced.
    pub fn death_passes(&self, distance_squared: i64, draw: u32) -> bool {
        let threshold: u64 = get_entry(&self.death_thresholds, distance_squared).unwrap_or(ALWAYS_PASSES_THRESHOLD);

        u64::from(draw) < threshold
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrowthChanceTables {
    default: GrowthChanceTable,
    compressed: GrowthChanceTable,
    extended: GrowthChanceTable,
}

impl GrowthChanceTables {
    fn build() -> GrowthChanceTables {
        GrowthChanceTables {
            default: GrowthChanceTable::build(GrowthStateKind::Default),
            compressed: GrowthChanceTable::build(GrowthStateKind::Compressed),
            extended: GrowthChanceTable::build(GrowthStateKind::Extended),
        }
    }

    pub fn get(&self, growth_state: GrowthStateKind) -> &GrowthChanceTable {
        match growth_state {
            GrowthStateKind::Default => &self.default,
            GrowthStateKind::Compressed => &self.compressed,
            GrowthStateKind::Extended => &self.extended,
        }
    }
}

/// The original's `coefficient * ln(r + 1) + 100`, in percent.
fn get_birth_chance(growth_state: GrowthStateKind, distance_squared: i64) -> f64 {
    let distance: f64 = libm::sqrt(distance_squared as f64);

    growth_state.coefficient() * libm::log(distance + 1.0) + 100.0
}

/// The original's `coefficient * ln(range + 1 - r) + 100`, in percent.
fn get_death_chance(growth_state: GrowthStateKind, distance_squared: i64) -> f64 {
    let distance: f64 = libm::sqrt(distance_squared as f64);
    let range: f64 = growth_state.range_pixels() as f64;

    growth_state.coefficient() * libm::log(range + 1.0 - distance) + 100.0
}

/// The original passes when `random * 100 <= chance`; scaled to u32 draws that is `u <= floor(chance / 100 * 2^32)`.
fn get_threshold(chance: f64) -> u64 {
    if chance < 0.0 {
        return NEVER_PASSES_THRESHOLD;
    }

    let largest_passing_draw: f64 = libm::floor(chance / 100.0 * DRAW_SPACE_SIZE);

    (largest_passing_draw as u64 + 1).min(ALWAYS_PASSES_THRESHOLD)
}

fn get_entry(thresholds: &[u64], distance_squared: i64) -> Option<u64> {
    let index: usize = usize::try_from(distance_squared).ok()?;

    thresholds.get(index).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GROWTH_STATES: [GrowthStateKind; 3] = [
        GrowthStateKind::Default,
        GrowthStateKind::Compressed,
        GrowthStateKind::Extended,
    ];
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    const GOLDEN_TABLE_DIGEST: u64 = 0x75a8_9b54_1a01_86e5;

    fn get_tables_digest(tables: &GrowthChanceTables) -> u64 {
        let mut digest: u64 = FNV_OFFSET_BASIS;

        for growth_state in GROWTH_STATES {
            let table: &GrowthChanceTable = tables.get(growth_state);
            let thresholds: Vec<u64> = [table.birth_thresholds(), table.death_thresholds()].concat();

            for threshold in thresholds {
                for byte in threshold.to_le_bytes() {
                    digest ^= u64::from(byte);
                    digest = digest.wrapping_mul(FNV_PRIME);
                }
            }
        }

        digest
    }

    #[test]
    fn build_gives_the_designed_lengths() {
        let lengths: Vec<(usize, usize)> = GROWTH_STATES
            .iter()
            .map(|growth_state| {
                let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(*growth_state);

                (table.birth_thresholds().len(), table.death_thresholds().len())
            })
            .collect();
        let total_length: usize = lengths.iter().map(|(birth_length, death_length)| birth_length + death_length).sum();

        assert_eq!(lengths, vec![(1366, 2501), (526, 1601), (2449, 4901)]);
        assert_eq!(total_length, 13_344);
    }

    #[test]
    fn build_matches_the_golden_digest() {
        assert_eq!(get_tables_digest(&GROWTH_CHANCE_TABLES), GOLDEN_TABLE_DIGEST);
    }

    #[test]
    fn birth_table_last_distance_squared_is_the_last_non_negative_chance() {
        for growth_state in GROWTH_STATES {
            let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(growth_state);

            assert_ne!(table.birth_thresholds().last(), Some(&NEVER_PASSES_THRESHOLD));
            assert!(get_birth_chance(growth_state, growth_state.birth_table_last_distance_squared() + 1) < 0.0);
        }
    }

    #[test]
    fn birth_passes_always_at_the_cursor() {
        let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(GrowthStateKind::Default);

        assert_eq!(table.birth_thresholds()[0], ALWAYS_PASSES_THRESHOLD);
        assert!(table.birth_passes(0, u32::MAX));
    }

    #[test]
    fn birth_passes_never_past_the_table_end() {
        let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(GrowthStateKind::Default);

        assert!(!table.birth_passes(1366, 0));
        assert!(!table.birth_passes(1_000_000, 0));
    }

    #[test]
    fn birth_passes_below_the_threshold_only() {
        let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(GrowthStateKind::Default);
        let threshold: u64 = table.birth_thresholds()[36];
        let largest_passing_draw: u32 = u32::try_from(threshold - 1).unwrap();

        assert!(table.birth_passes(36, largest_passing_draw));
        assert!(!table.birth_passes(36, largest_passing_draw + 1));
    }

    #[test]
    fn death_passes_always_at_the_range() {
        let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(GrowthStateKind::Default);

        assert_eq!(table.death_thresholds()[2500], ALWAYS_PASSES_THRESHOLD);
        assert!(table.death_passes(2500, u32::MAX));
        assert!(table.death_passes(2501, u32::MAX));
    }

    #[test]
    fn death_passes_never_near_the_cursor() {
        let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(GrowthStateKind::Default);

        assert_eq!(table.death_thresholds()[0], NEVER_PASSES_THRESHOLD);
        assert!(!table.death_passes(0, 0));
    }
}
