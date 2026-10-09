# Determinism

## Determinism rules
- Integer state only. No `f32`/`f64` stored in `GameState`.
- Floats allowed only in pure functions whose inputs and outputs are integers, using only IEEE-exact operations: `+ - * /`, `sqrt`, `abs`, `floor`, `round`, int/float conversion of values below 2^53. No `mul_add`.
- Logarithm: only `libm::log`, only while building the chance tables at startup ([growth chance tables](#growth-chance-tables)).
- RNG: hand-written PCG32 XSH-RR (`random/pcg32.rs`, owner decision), 64-bit state and increment, encoded on the wire as `Pcg32SerialOut` ([wire types](./protocol.md#wire-types)), one per game, seeded by the server from `getrandom`, part of `GameState`.
  - Seed: one `u64`. `Pcg32::from_seed(seed: u64)` is the reference `pcg32_srandom_r(seed, PCG32_STREAM)` with the fixed constant `PCG32_STREAM: u64`; `GameState::new(settings: GameSettings, seed: u64)` calls it.
  - `next_u32()`; `below(bound: u32) -> u32` via multiply-high (`(u64 * bound) >> 32`).
  - Draw order is defined by the iteration order below; nothing else draws.
- Iteration order everywhere: members ascending `MemberId`, except the birth phase (rotated start, [growth rules](./simulation.md#growth-rules)); cells ascending `LatticeCoordinate` (row-major); neighbour directions left, up, right, down (original order).
- State is canonical: `CellOccupancy` re-tightened each tick, collections are `BTreeMap`/`Vec` in defined order; so its wire form `GameStateSerialOut` and the checksum over it ([checksums and resync](./protocol.md#checksums-and-resync)) are canonical too.
- Projectile directions: unit vector from integer offsets with f64 `sqrt` and division, scaled by speed in subpixels, `round`ed to integer once at launch. No trigonometry.
- NaN cannot enter state: all float results are rounded to integers and zero-length vectors are handled before division.

## Forbidden operations
- Forbidden in `shared` outside `render/` and `play/`:
  - `ln`, `log*`, `exp*`, `powf`, `powi`, `sin`, `cos`, `tan`, `atan*`, `hypot`, `cbrt` from std;
  - `mul_add`;
  - `HashMap`, `HashSet`;
  - `usize` in `GameState` or any wire type.
- `libm::` paths are allowed.
- Enforced by the source-scanning test `shared/tests/forbidden_operations.rs` ([determinism tests](./testing.md#determinism-tests)).

## Growth chance tables
- Module: `organism/growth_chance_table.rs`.
- Original: birth `chance = coefficient * ln(r + 1) + 100`; death `chance = coefficient * ln(range + 1 - r) + 100`; roll passes when `random * 100 <= chance`.
- Cursor and cell centres are integer px, so `distance_squared = dx*dx + dy*dy` is an exact integer and the tables are indexed by `distance_squared`.
- `static GROWTH_CHANCE_TABLES: LazyLock<GrowthChanceTables>`; one table pair per `GrowthStateKind { Default, Compressed, Extended }`.
- Entry = exclusive u32 threshold as u64: `floor(chance / 100 * 2^32) + 1`, saturated at `2^32`; 0 when chance < 0. A draw `u` passes when `(u as u64) < entry`.
- Built once with `libm::log(libm::sqrt(distance_squared as f64) + 1.0)` (birth) and `libm::log(range + 1 - sqrt(distance_squared))` (death).

| State      | Coefficient | Range | Birth table `distance_squared` | Death table `distance_squared` |
| ---------- | ----------- | ----- | ------------------------------ | ------------------------------ |
| Default    | -27.5       | 50    | 0..=1365                       | 0..=2500                       |
| Compressed | -31.5       | 40    | 0..=525                        | 0..=1600                       |
| Extended   | -25.5       | 70    | 0..=2448                       | 0..=4900                       |

- 13,344 entries in total. Birth `distance_squared` past the table end: chance 0, but the draw is still consumed ([growth rules](./simulation.md#growth-rules)). Death `distance_squared > range²`: forced death, no draw.
- Golden test: FNV-1a 64 digest of all tables committed; a libm change that alters any entry fails the test.
