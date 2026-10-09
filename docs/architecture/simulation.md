# Simulation

- Crate: `shared`.

## Contract
- `pub fn step(state: &mut GameState, bundle: &InputBundle) -> Result<Vec<SimulationEvent>, StepError>` in `game/step.rs`.
  - Mutates in place; equivalent to `(state, bundle) -> (state', events)`.
  - Reads no clock, no environment, no global mutable state, does no I/O.
  - `StepError::TickMismatch { expected, received }` when `bundle.tick != state.tick + 1`; state untouched.
- Every time-like quantity is a `Tick(u32)`; 1 tick = 70 ms (`TICK_PERIOD_MILLISECONDS = 70`), nominal 14.29 Hz.
- Millisecond constants of the original are converted to ticks by rounding to nearest (table in [abilities](#abilities)).
- Tutorial and title scene run the same `step` locally with locally built bundles.

## Coordinates and types
- Module: `geometry/geometry.rs`.
- `WorldPoint { x: i32, y: i32 }`: world pixels; 1 world px = 1 CSS px on screen (no zoom, faithful).
- `LatticeCoordinate { i: i32, j: i32 }`: cell index on an organism's own lattice; `Ord` is row-major (`j`, then `i`).
- Cell centre = `organism.anchor + CELL_WIDTH_PIXELS * (i, j)`; `CELL_WIDTH_PIXELS = 6`.
- Each organism has its own lattice anchored at its integer spawn cursor; lattices of different organisms are not aligned (faithful).
- `Subpixels` (i32), `SubpixelPoint { x: i32, y: i32 }`, `SubpixelVector { x: i32, y: i32 }`: 1/1024 px (`SUBPIXELS_PER_PIXEL = 1024`, owner decision), the only unit finer than a whole pixel; used for projectiles, client cursor input and world bounds (the 100000 px maximum is 102,400,000 subpixels, within i32).
- All squared distances in i64; ellipse products in i128.

## Data model

```
GameState {
  tick: Tick,
  settings: GameSettings,            immutable after creation
  rng: Pcg32,
  world: World,
  round: Option<RoundState>,         Some only for srv
  members: BTreeMap<MemberId, Member>,
  next_member_id: MemberId,          one past the highest id joined so far (protocol.md#input-bundle)
}

GameSettings { title: String, mode: GameModeKind, world_shape: WorldShapeKind, world_width_pixels: u32, world_height_pixels: u32,
               player_minimum: Option<u8>, player_cap: u8, team_count: Option<u8>, leaderboard_length: u8 }
  player_minimum Some only in srv; team_count Some only in skm
GameModeKind { FreeForAll, Skirmish, Survival }       code(): "ffa" | "skm" | "srv"

World { shape: WorldShapeKind, bounds: WorldBounds, initial_bounds: WorldBounds }
WorldShapeKind { Rectangle, Ellipse }
WorldBounds { left: Subpixels, top: Subpixels, width: Subpixels, height: Subpixels }

Member {
  member_id: MemberId(u32),           never reused within a game
  screen_name: String,
  role: MemberRoleKind,               Participant | Spectator
  loadout: Option<Loadout>,           Some for Participant
  team: Option<TeamKind>,             Some only in skm
  score: Score { kills: u32, deaths: u32, wins: u32 },
  organism: Option<Organism>,         None when dead, awaiting spawn, or spectating
}

Loadout { appearance: Appearance, first: FirstAbilityKind, second: SecondAbilityKind, third: ThirdAbilityKind }
Appearance { color: OrganismColorKind, skin: SkinKind }
FirstAbilityKind { Extend, Compress }   SecondAbilityKind { Immortality, Freeze }   ThirdAbilityKind { Neutralize, Toxin }
SkinKind { Grid, Circles, Ghost, None }
OrganismColorKind { Fire, Camel, Clay, Sun, Leaf, Lime, Sky, Lake, Ocean, Royal, Petal, Hot }
TeamKind { Red, Blue, Green, Pink }     forced colours Fire, Sky, Lime, Petal

Organism {
  anchor: WorldPoint,
  cells: CellOccupancy,
  cursor: WorldPoint,
  last_hitter: Option<MemberId>,
  abilities: OrganismAbilities,
}

CellOccupancy { origin: LatticeCoordinate, width: u16, height: u16, row_bitmap_bytes: Vec<u8> }
  dense bitmap over the tight bounding box, row-major, bit i of byte = column;
  re-tightened at the end of every tick so equal cell sets give equal `CellOccupancy` values.

OrganismAbilities {
  first: AbilityPhase,                extend self-cast, or compress caster-side timer after a hit
  second: AbilityPhase,               immortality self-cast, or freeze caster-side timer after a hit
  third: AbilityPhase,
  third_center: Option<WorldPoint>,   neutralize or toxin centre while Active
  spore: SporePhase,
  shots: [ShotPhase; 2],              slot 0 carries compress, slot 1 carries freeze
  compressed_until: Option<Tick>,     effect received from an enemy shot
  frozen_until: Option<Tick>,
}
AbilityPhase { Ready, Active { ends_at: Tick }, Cooling { ready_at: Tick } }
SporePhase { Ready, Flying { ends_at, spores: Vec<Projectile> }, Secreting { ends_at, spores: Vec<Projectile> }, Cooling { ready_at } }
ShotPhase { Ready, Flying { ends_at, shot: Projectile }, Secreting { ends_at, center: SubpixelPoint }, Cooling { ready_at } }
Projectile { position: SubpixelPoint, velocity: SubpixelVector }

RoundState { phase: RoundPhase, phase_started_at: Tick }
RoundPhase { Waiting, PreRound, Playing, PostRound }
```

- No simulation type derives `bitcode::Encode`/`Decode` (owner decision); the snapshot carries `GameStateSerialOut`, its wire mirror, RNG state included ([snapshot](./protocol.md#snapshot), [wire types](./protocol.md#wire-types)).
- No parallel arrays: organism, abilities, score, team all hang off `Member` keyed by `MemberId`.
- Leaderboard is not stored; `scoreboard::get_leaderboard_rows(&GameState) -> Vec<LeaderboardRow>` derives it.
- Teams are not stored separately; derived from `Member.team`.
- `World` supports width != height (title and tutorial worlds follow the window, and their `GameSettings` are built directly with mode `FreeForAll`, [tutorial and title](./client-web.md#tutorial-and-title)); `GameSettingsSerialIn` has one `world_size_pixels`, which `TryFrom` copies into both dimensions, so created games are square by construction.
- `GameSettings` has no password flag: whether a game is secured is a fact of the server's `GameHandle.password_hash` ([registries](./server.md#registries)), shown to clients through `GameSummary.secured` ([server to client](./protocol.md#server-to-client)).
- Ability predicates (`ability/ability_model.rs`), the only way any phase reads an ability's state, since `first`/`second`/`third` are shared between the self-cast and the caster-side timer:
  - Extended: `first` Active and loadout first = Extend.
  - Immortal: `second` Active and loadout second = Immortality.
  - Neutralize field active: `third` Active and loadout third = Neutralize.
  - Toxin field active: `third` Active and loadout third = Toxin.
  - Compressed: `compressed_until` set. Frozen: `frozen_until` set.
  - A Compress or Freeze caster's Active `first`/`second` is a cooldown display only and satisfies none of these.

## Tick order
- Module: `game/step.rs`.
Phase 1 processes events strictly in bundle order (not grouped by kind); every other phase iterates members in ascending `MemberId`, except births (rotated start, [growth rules](#growth-rules)).

1. Membership events from the bundle (`MemberEvent`, [input bundle](./protocol.md#input-bundle)): `Joined` (adds the member; never spawns), `Left` (member and all its projectiles and fields removed), `SpawnRequested`, `AppearanceChanged`.
2. Cursor updates: `organism.cursor = input.cursor` for each `PlayerTickInput`.
3. Timer expiry: every `AbilityPhase`, `SporePhase`, `ShotPhase`, `compressed_until`, `frozen_until` whose tick has arrived advances (Active to Cooling, Cooling to Ready, Flying to Cooling, Secreting to Cooling, received effects cleared).
4. Ability presses from inputs ([abilities](#abilities)), alive organisms only.
5. Projectile flight: `position += velocity` for every Flying spore and shot, including those launched in phase 4 of this tick, so a projectile moves exactly its flight duration in ticks (expiry in phase 3 of tick `ends_at` comes before any move).
6. World: survival shrink while `RoundPhase::Playing` ([world, spawn, rounds](#world-spawn-rounds)).
7. Births, all organisms ([growth rules](#growth-rules)).
8. Natural deaths, all organisms ([growth rules](#growth-rules)).
9. Damage: acid and toxin, all victims ([abilities](#abilities)).
10. Death bookkeeping: each organism with zero cells: `deaths += 1`; kill credit (rule 2 of [deferred questionable rules](#deferred-questionable-rules)); organism set to `None`; `SimulationEvent::OrganismDied { member_id, credited_to }`.
11. Round transitions ([world, spawn, rounds](#world-spawn-rounds)), including force spawns.
12. `CellOccupancy` re-tightened; `tick = bundle.tick`.

- Interpretation: the original ran birth, death and damage per organism on each owner's machine; the rewrite runs each phase for all organisms in turn, so an organism's births see opponents' births from earlier members in the same tick and opponents' deaths from the previous tick.

## Growth rules
- Faithful to the original.
- Exposed cell: fewer than 4 orthogonal own neighbours. Enclosed cells never die naturally.
- Adjacent sites: for each cell in order, for each direction left, up, right, down whose neighbour is empty, emit that site. No deduplication: a site next to k cells is emitted k times (rule 4).
- Adjacent sites and the coefficient/range are computed once at the start of the birth phase.
- Growth state: Compressed XOR Extended (predicates, [data model](#data-model)) selects Compressed or Extended; neither or both selects Default.
- Birth phase, skipped entirely while frozen:
  - For each emitted site:
    - skip if outside the world (unified border rule, [world, spawn, rounds](#world-spawn-rounds));
    - skip if it collides with any other organism's cell, teammates included (rule 3): collision when `|dx| <= 6 && |dy| <= 6` between centres (touching edges count, faithful AABB);
    - draw one u32; born if the draw passes and the site is not already occupied (cells born earlier this tick count).
  - Born cells are visible immediately to later sites, later organisms' collision checks, and this tick's death phase.
- Natural death phase, skipped while Frozen or Immortal (predicates, [data model](#data-model)):
  - Exposed set computed once after births; newly born cells can die this tick.
  - For each exposed cell in order: `distance_squared > range²` removes it (no draw); else outside the world removes it (no draw); else draw one u32 and remove if it passes.
- Birth phase member order (owner decision): members with an organism, in ascending `MemberId`, rotated to start at index `tick % organism_count`, so no member always gets first claim on a contested site.
- Opponent collision index: per-tick `BTreeMap<(i32, i32), Vec<WorldPoint>>` keyed by `floor(center / 6)`, rebuilt before the birth phase, updated as cells are born.

## Abilities
- Faithful to the original.

Constants (`ability/ability_constants.rs`):

| Item                 | Original ms | Ticks | Notes                                      |
| -------------------- | ----------- | ----- | ------------------------------------------ |
| Extend active        | 4500        | 64    | self                                       |
| Extend cooldown      | 4000        | 57    | from end                                   |
| Compress on target   | 3500        | 50    | caster `first` Active 50 then Cooling 57   |
| Compress cooldown    | 4000        | 57    |                                            |
| Immortality active   | 3500        | 50    |                                            |
| Immortality cooldown | 6000        | 86    |                                            |
| Freeze on target     | 4000        | 57    | caster `second` Active 57 then Cooling 86  |
| Freeze cooldown      | 6000        | 86    |                                            |
| Neutralize active    | 3500        | 50    | radius 60 px at cursor                     |
| Neutralize cooldown  | 6500        | 93    |                                            |
| Toxin active         | 4000        | 57    | radius 60 px at cursor                     |
| Toxin cooldown       | 6000        | 86    |                                            |
| Spore flight         | 1700        | 24    | 10.5 px/tick = 10752 subpx/tick            |
| Spore cooldown       | 7500        | 107   | from flight end or secretion end           |
| Spore secretion      | 800         | 11    | radius 24.607 px                           |
| Shot flight          | 1500        | 21    | 8.75 px/tick = 8960 subpx/tick             |
| Shot cooldown        | 2000        | 29    | per slot, from flight end or secretion end |
| Shot secretion       | 800         | 11    | radius 12.304 px                           |

- Speeds: original 6 px and 5 px per 40 ms server packet; times 70/40 per tick.
- Radii compared squared in subpixel² (cell centres converted to subpixels): spore secretion `R² = 72 * 2.9² px² = 605.52 px²` → `SPORE_SECRETION_RADIUS_SQUARED_SUBPIXELS = 634_933_739`; shot secretion `R² = 151.38 px²` → `SHOT_SECRETION_RADIUS_SQUARED_SUBPIXELS = 158_733_434`. Comparison is `distance_squared <= *_RADIUS_SQUARED_SUBPIXELS` (original `<=`); the constants are floors of non-integer R², so `<=` against the floor is exact.
- Neutralize and toxin: `FIELD_RADIUS_SQUARED_PIXELS = 3600`, `distance_squared <= 3600`, integer px.
- `AbilityPressSet` (u8 bits): `FIRST` (X), `SECOND` (C), `THIRD` (V), `FOURTH` (Space). Presses require an alive organism.
- X:
  - Extend chosen and `first` Ready: `first = Active(64)`.
  - Else Compress chosen and `first` Ready: shot slot 0 press.
- C:
  - Immortality chosen and Ready: `second = Active(50)`.
  - Else Freeze chosen and Ready: shot slot 1 press.
- V: Neutralize or Toxin chosen and Ready: `third = Active`, `third_center = cursor` (rule 6).
- Space:
  - `spore` Ready: launch spores.
  - `spore` Flying: secrete (spores stop, `Secreting(11)`).
- Shot slot press:
  - Slot Ready: launch, needs a non-zero `aim` (else no-op, slot stays Ready).
  - Slot Flying: pop (below).
  - Otherwise no-op.
- Spore launch:
  - All exposed cells leave the organism and become spores at their centres.
  - Spores are taken one at a time in lattice order; each spore's direction is its centre minus the centroid of the cells still in the organism (all cells minus the spores already taken), computed as `cell * count - sum` (integers), and then the cell is removed (faithful progressive centroid, `Ability.js:511-523`).
  - A spore whose offset is zero is removed from the organism and creates no projectile (faithful: the original's NaN spore vanished). This always happens to the last spore of an organism with no enclosed cells.
  - Interpretation: the original took spores in cell array (birth) order; the rewrite has no birth order and uses lattice order.
  - May remove the last cells (rule 7).
- Shot launch:
  - Candidate cells: exposed cells; the one maximising the cosine between (cell - centroid) and `aim` wins; ties go to the first in lattice order. Compared exactly in i128 via squared, sign-aware products.
  - Zero-offset cells are candidates only when no other exposed cell exists.
  - The chosen cell leaves the organism; the shot flies along `aim` (not along the cell's offset, faithful).
  - May remove the last cell (rule 7).
- Shot pop:
  - Shot stops; `ShotPhase::Secreting(11)` at its position.
  - Effect: for each other member (ascending id) that is not a teammate, if any cell centre is within the shot radius and not inside that target's own active neutralize, apply the carried effect once:
    - slot 0: target `compressed_until = tick + 50` (restarts); caster `first = Active(50)`.
    - slot 1: target `frozen_until = tick + 57`; caster `second = Active(57)`.
  - On a miss only the shot slot cools; the carried ability stays Ready (faithful).
- Damage phase (`ability/damage.rs`), per victim organism, per ability owner in ascending id:
  - Spore secretions: a cell is removed by the first spore within radius.
  - Shot secretions, slot 0 then slot 1.
  - Toxin (Toxin field active, [data model](#data-model)): never affects the owner.
  - Every source skips a cell inside any member's Neutralize field (rule 5) and skips teammates of the owner (skm).
  - Spore and shot acid do hit the owner's own cells (rule 1).
  - `last_hitter` is set to the owner on each actual removal; the last removal in iteration order wins.
- Death of an organism removes its spores, shots, secretions and fields (faithful: the original removed the dead player's ability object).
- Spawn resets all phases to Ready and clears received effects.

## World, spawn, rounds
- Unified border rule (`World::contains_cell(center)`), used by birth and death (rule 8 of the [decisions record](./overview.md#game-rules)):
  - Rectangle: outside when `cx - 6 <= left || cx + 6 >= right || cy - 6 <= top || cy + 6 >= bottom` (inclusive, in subpixels).
  - Ellipse: outside when any corner `(cx ± 6, cy ± 6)` satisfies `(2dx)²·h² + (2dy)²·w² >= w²·h²` with `dx, dy` measured from the world's actual centre (fixes the missing `world.x/y` offset).
- Survival shrink, each tick in `Playing` while width and height both exceed 200 px (`204_800` subpixels): width and height each `-286` subpixels, left and top each `+143` (0.2793 px per tick, 3.990 px/s per dimension against the original's 4 px/s; 800 px to 200 px in 150.4 s against 150 s).
- Shrinking stops when `Playing` ends; the world stays at its shrunk size through `PostRound`, and leaving `PostRound` restores `bounds = initial_bounds` (whole rectangle).
- Spawn (`organism/spawn.rs`):
  - Whole-pixel range: `left_px = ceil(left / 1024)`, `right_px = floor((left + width) / 1024)` (subpixels, integer division rounding as named); top and bottom likewise.
  - Candidate cursor x uniform in `[left_px + 53, right_px - 53)`, y likewise (`53 = 50 + 6/2`, faithful), via `rng.below`.
  - Rejected when (all comparisons inclusive, as the original's `<=`):
    - ellipse world: the cursor point itself is outside, `(2dx)²·h² + (2dy)²·w² >= w²·h²` from the world's actual centre (faithful point test, not `contains_cell`; centre offset fixed);
    - within `|dx| <= 6 && |dy| <= 6` of any cell of any organism;
    - within any active spore secretion, shot secretion or toxin field radius.
  - Up to `SPAWN_ATTEMPT_LIMIT = 64` candidates; none valid gives `SimulationEvent::SpawnRejected { member_id, reason: PositionNotFound }` and the member stays without an organism.
  - Accepted: `anchor = cursor = candidate`, one cell at lattice (0, 0), `SimulationEvent::OrganismSpawned { member_id, cursor }`.
- Spawn allowed when: member is a Participant (spawn requests carry a loadout and convert Spectators), has no organism, alive organism count < `player_cap`, and either no rounds or `RoundPhase` is `Waiting` or `PreRound`.
  - `step` checks this as an invariant and emits `SpawnRejected { reason: CapReached | RoundInProgress | AlreadyAlive }` when it fails; the server's admission ([lifecycle](./server.md#lifecycle)) makes these unreachable for server games, and the tutorial and title never request them.
- `spawn::place_organism(&mut GameState, MemberId, WorldPoint)`: one cell at an explicit position, no RNG, no hazard check; used only by the tutorial and title scene between steps ([tutorial and title](./client-web.md#tutorial-and-title)), never by server games.
- Rounds (srv only). Participant count = members with role Participant.

| Phase     | Condition                        | Transition                                        |
| --------- | -------------------------------- | ------------------------------------------------- |
| Waiting   | participants >= `player_minimum` | PreRound                                          |
| PreRound  | participants < `player_minimum`  | Waiting                                           |
| PreRound  | elapsed == 100 ticks (7000 ms)   | force spawn                                       |
| PreRound  | elapsed >= 114 ticks (8000 ms)   | Playing                                           |
| Playing   | alive organisms <= 1             | PostRound; the sole survivor, if any, `wins += 1` |
| PostRound | elapsed >= 114 ticks             | restore world, Waiting, force spawn               |

- One participant predicate for start and cancel (fixes the player count versus member count mismatch).
- Force spawn: every Participant in ascending id, up to `player_cap`, is (re)spawned at a new position with its loadout; existing organisms are discarded without counting a death.
- Pure Spectators are never force spawned.
- Countdown seconds for messages: `ceil(remaining_ticks * 70 / 1000)`, from ticks, never wall clocks.

## Deferred questionable rules
- Kept as in the original.
1. A player's own spore and shot acid damages its own cells; the death that follows is a suicide with no kill credit.
2. Kill credit goes to `last_hitter` whenever the organism reaches zero cells, however long after the hit; no credit when `last_hitter` is the victim or is no longer a member. Interpretation: `last_hitter` is recorded only on an actual removal, so a teammate's hit no longer sets it (the original set it before the team check). Neutralized hits never set it in the original either (the neutralize skip precedes the assignment, `Org.js:435-443`), which is faithful.
3. Teammates block each other's births (collision ignores teams).
4. An empty site adjacent to k cells is rolled k times per tick.
5. An active neutralize protects any player's cells inside it from all acid and toxin.
6. Toxin and neutralize centre on the cursor at activation.
7. Spore and shot launches can remove an organism's last cells and kill it.

## Scoring and leaderboard
- Module: `member/scoreboard.rs`.
- `ffa`: rows per Participant; kills descending, deaths ascending, then `MemberId` ascending. Columns Player, Kills, Deaths, K:D.
- `skm`: rows per team (`team_count` rows), sums over current members, in fixed team order Red, Blue, Green, Pink, never sorted (faithful, `Board.js` iterates team index). Columns Team, Kills, Deaths, K:D.
- `srv`: rows per Participant; kills descending, wins descending, then `MemberId`. Columns Player, Wins, Kills.
- K:D display: "∞" for n/0, "0" for 0/0, else rounded to at most 2 decimals with trailing zeros dropped ("1.5", "2", "0.33"; faithful `round(x * 100) / 100`), computed from integers.
- Row count `min(leaderboard_length, rows)`; skm shows all team rows.
- Interpretation: pure Spectators are not on the board; dead Participants are.

## Teams
- Module: `member/team_assignment.rs`.
- `skm` teams: the first `team_count` of Red, Blue, Green, Pink; colour forced from the team.
- Manual choice allowed when the member is already on that team, or when the team's size, counting members other than the requester, equals the minimum over all teams.
- Auto assign: uniformly among the smallest teams; the choice is made by the server (its own RNG, not the game RNG) and arrives in the `MemberEvent`.

## Simulation events
- Module: `game/simulation_event.rs`.
- `MemberJoined`, `MemberLeft`, `OrganismSpawned { member_id, cursor }`, `SpawnRejected { member_id, reason: SpawnRejectionKind }` (`PositionNotFound`, `CapReached`, `RoundInProgress`, `AlreadyAlive`), `OrganismDied { member_id, credited_to: Option<MemberId> }`, `RoundPhaseChanged { phase }`, `RoundWon { member_id }`, `EffectApplied { target, caster, kind }`.
- Consumers: server (logging, lobby summary, clamp reset on spawn), client (crosshair snap on own spawn, menus, messages). Not part of the checksum.
