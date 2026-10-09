use std::collections::BTreeMap;

use crate::game::{GameModeKind, GameSettings, Tick};
use crate::member::{Member, MemberId};
use crate::random::Pcg32;
use crate::round::{RoundPhase, RoundState};
use crate::world::{World, WorldBounds};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameState {
    pub tick: Tick,
    /// Immutable after creation.
    pub settings: GameSettings,
    pub rng: Pcg32,
    pub world: World,
    /// Some only in survival.
    pub round: Option<RoundState>,
    pub members: BTreeMap<MemberId, Member>,
    /// One past the highest member id joined so far.
    pub next_member_id: MemberId,
}

impl GameState {
    /// Tick 0, no members.
    pub fn new(settings: GameSettings, seed: u64) -> GameState {
        let bounds: WorldBounds =
            WorldBounds::from_pixel_size(settings.world_width_pixels, settings.world_height_pixels);
        let round: Option<RoundState> = match settings.mode {
            GameModeKind::Survival => Some(RoundState {
                phase: RoundPhase::Waiting,
                phase_started_at: Tick(0),
            }),
            GameModeKind::FreeForAll | GameModeKind::Skirmish => None,
        };

        GameState {
            tick: Tick(0),
            world: World::new(settings.world_shape, bounds),
            settings,
            rng: Pcg32::from_seed(seed),
            round,
            members: BTreeMap::new(),
            next_member_id: MemberId(0),
        }
    }

    pub fn alive_organism_count(&self) -> u32 {
        let alive_organism_count: usize = self.members.values().filter(|member| member.organism.is_some()).count();

        u32::try_from(alive_organism_count).unwrap_or(u32::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use crate::geometry::{Subpixels, WorldPoint};
    use crate::world::WorldShapeKind;

    #[test]
    fn new_starts_empty_at_tick_zero() {
        let settings: GameSettings =
            test_fixture::create_settings(GameModeKind::FreeForAll, WorldShapeKind::Ellipse, 800);
        let state: GameState = GameState::new(settings, 17);

        assert_eq!(state.tick, Tick(0));
        assert!(state.members.is_empty());
        assert_eq!(state.next_member_id, MemberId(0));
        assert_eq!(state.rng, Pcg32::from_seed(17));
        assert_eq!(state.round, None);
    }

    #[test]
    fn new_builds_the_world_from_the_settings() {
        let mut settings: GameSettings =
            test_fixture::create_settings(GameModeKind::FreeForAll, WorldShapeKind::Ellipse, 1200);
        settings.world_height_pixels = 700;
        let state: GameState = GameState::new(settings, 0);

        assert_eq!(state.world.shape, WorldShapeKind::Ellipse);
        assert_eq!(state.world.bounds.width, Subpixels(1200 * 1024));
        assert_eq!(state.world.bounds.height, Subpixels(700 * 1024));
        assert_eq!(state.world.initial_bounds, state.world.bounds);
    }

    #[test]
    fn new_waits_for_a_round_only_in_survival() {
        let survival_state: GameState =
            test_fixture::create_state(GameModeKind::Survival, WorldShapeKind::Rectangle, 800);
        let skirmish_state: GameState =
            test_fixture::create_state(GameModeKind::Skirmish, WorldShapeKind::Rectangle, 800);

        assert_eq!(
            survival_state.round,
            Some(RoundState {
                phase: RoundPhase::Waiting,
                phase_started_at: Tick(0),
            }),
        );
        assert_eq!(skirmish_state.round, None);
    }

    #[test]
    fn alive_organism_count_counts_members_with_an_organism() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        state.members.insert(
            MemberId(0),
            test_fixture::create_participant_with_organism(MemberId(0), WorldPoint { x: 100, y: 100 }),
        );
        state.members.insert(MemberId(1), test_fixture::create_participant(MemberId(1)));

        assert_eq!(state.alive_organism_count(), 1);
    }
}
