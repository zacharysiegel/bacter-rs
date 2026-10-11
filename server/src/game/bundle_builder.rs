use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use shared::ability::{AbilityPressSet, AimVector};
use shared::game::{GameState, InputBundle, MemberEvent, PlayerInput, PlayerTickInput, Tick};
use shared::geometry;
use shared::geometry::{SubpixelPoint, WorldPoint};
use shared::member::MemberId;
use shared::organism::Organism;

use crate::game::{CursorClamp, CursorClampKind};

/// Turns the member events and inputs received between two ticks into the next bundle.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BundleBuilder {
    member_events: Vec<MemberEvent>,
    member_inputs: BTreeMap<MemberId, MemberInputState>,
    pending_inputs: BTreeMap<MemberId, PendingInput>,
}

impl BundleBuilder {
    pub fn new() -> BundleBuilder {
        BundleBuilder::default()
    }

    /// Applied in arrival order.
    pub fn queue_member_event(&mut self, member_event: MemberEvent) {
        self.member_events.push(member_event);
    }

    pub fn queue_input(&mut self, state: &GameState, member_id: MemberId, input: PlayerInput) -> InputAcceptanceKind {
        if input.client_tick > state.tick {
            return InputAcceptanceKind::AheadOfServer;
        }

        let has_organism: bool = state.members.get(&member_id).is_some_and(|member| member.organism.is_some());
        let member_input: Option<&mut MemberInputState> = self.member_inputs.get_mut(&member_id);
        let Some(member_input) = member_input.filter(|_| has_organism) else {
            return InputAcceptanceKind::WithoutOrganism;
        };

        if member_input.last_client_tick.is_some_and(|last_client_tick| input.client_tick <= last_client_tick) {
            return InputAcceptanceKind::Stale;
        }

        if input.client_tick < member_input.spawn_tick {
            return InputAcceptanceKind::BeforeSpawn;
        }

        member_input.last_client_tick = Some(input.client_tick);

        match self.pending_inputs.entry(member_id) {
            Entry::Vacant(entry) => {
                entry.insert(PendingInput::from_input(input));
            }
            Entry::Occupied(mut entry) => entry.get_mut().merge(input),
        }

        InputAcceptanceKind::Accepted
    }

    pub fn record_spawn(&mut self, member_id: MemberId, cursor: WorldPoint, spawn_tick: Tick) {
        let last_client_tick: Option<Tick> =
            self.member_inputs.get(&member_id).and_then(|member_input| member_input.last_client_tick);

        self.member_inputs.insert(
            member_id,
            MemberInputState {
                clamp: CursorClamp::new(cursor.to_subpixel_point()),
                spawn_tick,
                last_client_tick,
            },
        );
        self.pending_inputs.remove(&member_id);
    }

    pub fn forget_member(&mut self, member_id: MemberId) {
        self.member_inputs.remove(&member_id);
        self.pending_inputs.remove(&member_id);
    }

    /// The bundle for the tick after `state`; a member appears in it only when its cursor moved or it pressed.
    pub fn build(&mut self, state: &GameState) -> BuiltBundle {
        let mut pending_inputs: BTreeMap<MemberId, PendingInput> = std::mem::take(&mut self.pending_inputs);
        let mut player_inputs: Vec<PlayerTickInput> = Vec::new();
        let mut cursor_corrections: Vec<CursorCorrection> = Vec::new();

        for (member_id, member_input) in &mut self.member_inputs {
            let organism: Option<&Organism> = state.members.get(member_id).and_then(|member| member.organism.as_ref());
            let Some(organism) = organism else {
                continue;
            };

            member_input.clamp.add_tick_budget();

            let Some(pending_input) = pending_inputs.remove(member_id) else {
                continue;
            };

            let applied_input: AppliedPendingInput =
                member_input.apply_pending_input(*member_id, pending_input, organism.cursor);

            cursor_corrections.extend(applied_input.cursor_correction);
            player_inputs.extend(applied_input.player_input);
        }

        BuiltBundle {
            bundle: InputBundle {
                tick: state.tick.next(),
                member_events: std::mem::take(&mut self.member_events),
                player_inputs,
            },
            cursor_corrections,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputAcceptanceKind {
    Accepted,
    /// Sampled after a bundle the server has not built; a protocol violation.
    AheadOfServer,
    WithoutOrganism,
    /// Not after the member's last accepted input.
    Stale,
    /// Sampled before the client saw its spawn.
    BeforeSpawn,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuiltBundle {
    pub bundle: InputBundle,
    pub cursor_corrections: Vec<CursorCorrection>,
}

/// The cursor the server applied in place of the member's input at `client_tick`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorCorrection {
    pub member_id: MemberId,
    pub client_tick: Tick,
    pub cursor: SubpixelPoint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MemberInputState {
    clamp: CursorClamp,
    spawn_tick: Tick,
    last_client_tick: Option<Tick>,
}

impl MemberInputState {
    fn apply_pending_input(
        &mut self,
        member_id: MemberId,
        pending_input: PendingInput,
        organism_cursor: WorldPoint,
    ) -> AppliedPendingInput {
        let clamp_kind: CursorClampKind = self.clamp.apply(pending_input.target);
        let applied_cursor: SubpixelPoint = self.clamp.applied_cursor();

        let cursor_correction: Option<CursorCorrection> = match clamp_kind {
            CursorClampKind::Clamped => Some(CursorCorrection {
                member_id,
                client_tick: pending_input.client_tick,
                cursor: applied_cursor,
            }),
            CursorClampKind::WithinBudget => None,
        };

        let cursor: WorldPoint = get_rounded_world_point(applied_cursor);
        let has_change: bool = cursor != organism_cursor || pending_input.ability_presses != AbilityPressSet::NONE;

        let player_input: Option<PlayerTickInput> = has_change.then_some(PlayerTickInput {
            member_id,
            cursor,
            ability_presses: pending_input.ability_presses,
            aim: pending_input.aim,
        });

        AppliedPendingInput {
            cursor_correction,
            player_input,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AppliedPendingInput {
    cursor_correction: Option<CursorCorrection>,
    player_input: Option<PlayerTickInput>,
}

/// The inputs a member sent since the last bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PendingInput {
    client_tick: Tick,
    target: SubpixelPoint,
    ability_presses: AbilityPressSet,
    aim: Option<AimVector>,
}

impl PendingInput {
    fn from_input(input: PlayerInput) -> PendingInput {
        PendingInput {
            client_tick: input.client_tick,
            target: input.cursor,
            ability_presses: input.ability_presses,
            aim: input.aim.filter(|_| input.ability_presses.has_shot_slot_press()),
        }
    }

    /// The latest cursor wins, presses accumulate, and the aim comes with the input which pressed a shot slot.
    fn merge(&mut self, input: PlayerInput) {
        self.client_tick = input.client_tick;
        self.target = input.cursor;

        if input.ability_presses.has_shot_slot_press() {
            self.aim = input.aim;
        }

        self.ability_presses = self.ability_presses.with(input.ability_presses);
    }
}

/// Halves round up.
fn get_rounded_world_point(subpixel_point: SubpixelPoint) -> WorldPoint {
    WorldPoint {
        x: get_rounded_pixels(subpixel_point.x),
        y: get_rounded_pixels(subpixel_point.y),
    }
}

fn get_rounded_pixels(subpixels: i32) -> i32 {
    (subpixels + geometry::SUBPIXELS_PER_PIXEL / 2).div_euclid(geometry::SUBPIXELS_PER_PIXEL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use shared::game::GameModeKind;
    use shared::member::{Member, MemberRoleKind, Score};

    const MEMBER_ID: MemberId = MemberId(0);
    const SPAWN_CURSOR: WorldPoint = WorldPoint { x: 100, y: 200 };

    fn create_state_with_organism(tick: Tick) -> GameState {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);
        state.members.insert(
            MEMBER_ID,
            Member {
                member_id: MEMBER_ID,
                screen_name: String::from("Blob"),
                role: MemberRoleKind::Participant,
                loadout: Some(test_fixture::create_loadout()),
                team: None,
                score: Score::zero(),
                organism: Some(Organism::new(SPAWN_CURSOR)),
            },
        );
        state.next_member_id = MemberId(1);
        state.tick = tick;

        state
    }

    fn create_builder_with_spawn(spawn_tick: Tick) -> BundleBuilder {
        let mut bundle_builder: BundleBuilder = BundleBuilder::new();
        bundle_builder.record_spawn(MEMBER_ID, SPAWN_CURSOR, spawn_tick);

        bundle_builder
    }

    fn create_input(client_tick: u32, cursor: WorldPoint, ability_presses: AbilityPressSet) -> PlayerInput {
        PlayerInput {
            client_tick: Tick(client_tick),
            cursor: cursor.to_subpixel_point(),
            ability_presses,
            aim: None,
        }
    }

    #[test]
    fn build_puts_the_queued_member_events_in_the_next_bundle_in_order() {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);
        let mut bundle_builder: BundleBuilder = BundleBuilder::new();
        bundle_builder.queue_member_event(MemberEvent::Left { member_id: MemberId(4) });
        bundle_builder.queue_member_event(MemberEvent::Left { member_id: MemberId(2) });

        let built_bundle: BuiltBundle = bundle_builder.build(&state);

        assert_eq!(
            built_bundle.bundle,
            InputBundle {
                tick: Tick(1),
                member_events: vec![
                    MemberEvent::Left { member_id: MemberId(4) },
                    MemberEvent::Left { member_id: MemberId(2) },
                ],
                player_inputs: Vec::new(),
            },
        );
        assert_eq!(bundle_builder.build(&state).bundle.member_events, Vec::new());
    }

    #[test]
    fn build_applies_a_move_within_the_budget_without_a_correction() {
        let state: GameState = create_state_with_organism(Tick(5));
        let mut bundle_builder: BundleBuilder = create_builder_with_spawn(Tick(5));
        let target: WorldPoint = WorldPoint { x: 103, y: 200 };

        bundle_builder.queue_input(&state, MEMBER_ID, create_input(5, target, AbilityPressSet::NONE));
        let built_bundle: BuiltBundle = bundle_builder.build(&state);

        assert_eq!(
            built_bundle.bundle.player_inputs,
            vec![PlayerTickInput {
                member_id: MEMBER_ID,
                cursor: target,
                ability_presses: AbilityPressSet::NONE,
                aim: None,
            }],
        );
        assert_eq!(built_bundle.cursor_corrections, Vec::new());
    }

    #[test]
    fn build_clamps_a_jump_and_reports_the_correction() {
        let state: GameState = create_state_with_organism(Tick(5));
        let mut bundle_builder: BundleBuilder = create_builder_with_spawn(Tick(5));

        bundle_builder.queue_input(
            &state,
            MEMBER_ID,
            create_input(5, WorldPoint { x: 1100, y: 200 }, AbilityPressSet::NONE),
        );
        let built_bundle: BuiltBundle = bundle_builder.build(&state);
        let spawn_subpixel_cursor: SubpixelPoint = SPAWN_CURSOR.to_subpixel_point();

        assert_eq!(
            built_bundle.cursor_corrections,
            vec![CursorCorrection {
                member_id: MEMBER_ID,
                client_tick: Tick(5),
                cursor: SubpixelPoint {
                    x: spawn_subpixel_cursor.x + 3174,
                    y: spawn_subpixel_cursor.y,
                },
            }],
        );
        assert_eq!(
            built_bundle.bundle.player_inputs[0].cursor,
            WorldPoint { x: 103, y: 200 }
        );
    }

    #[test]
    fn build_accumulates_budget_over_ticks_without_input() {
        let state: GameState = create_state_with_organism(Tick(5));
        let mut bundle_builder: BundleBuilder = create_builder_with_spawn(Tick(5));
        bundle_builder.build(&state);
        bundle_builder.build(&state);

        bundle_builder.queue_input(
            &state,
            MEMBER_ID,
            create_input(5, WorldPoint { x: 109, y: 200 }, AbilityPressSet::NONE),
        );
        let built_bundle: BuiltBundle = bundle_builder.build(&state);

        assert_eq!(built_bundle.cursor_corrections, Vec::new());
        assert_eq!(
            built_bundle.bundle.player_inputs[0].cursor,
            WorldPoint { x: 109, y: 200 }
        );
    }

    #[test]
    fn build_leaves_out_a_member_which_neither_moved_nor_pressed() {
        let state: GameState = create_state_with_organism(Tick(5));
        let mut bundle_builder: BundleBuilder = create_builder_with_spawn(Tick(5));

        bundle_builder.queue_input(&state, MEMBER_ID, create_input(5, SPAWN_CURSOR, AbilityPressSet::NONE));

        assert_eq!(bundle_builder.build(&state).bundle.player_inputs, Vec::new());
    }

    #[test]
    fn build_ors_the_presses_of_a_tick_and_keeps_the_aim_of_the_shot_press() {
        let state: GameState = create_state_with_organism(Tick(7));
        let mut bundle_builder: BundleBuilder = create_builder_with_spawn(Tick(5));
        let aim: AimVector = AimVector { x: 12, y: -3 };
        let mut shot_input: PlayerInput = create_input(5, SPAWN_CURSOR, AbilityPressSet::FIRST);
        shot_input.aim = Some(aim);

        bundle_builder.queue_input(&state, MEMBER_ID, shot_input);
        bundle_builder.queue_input(
            &state,
            MEMBER_ID,
            create_input(6, SPAWN_CURSOR, AbilityPressSet::FOURTH),
        );
        let built_bundle: BuiltBundle = bundle_builder.build(&state);

        assert_eq!(
            built_bundle.bundle.player_inputs,
            vec![PlayerTickInput {
                member_id: MEMBER_ID,
                cursor: SPAWN_CURSOR,
                ability_presses: AbilityPressSet::FIRST.with(AbilityPressSet::FOURTH),
                aim: Some(aim),
            }],
        );
    }

    #[test]
    fn queue_input_rejects_a_client_tick_ahead_of_the_server() {
        let state: GameState = create_state_with_organism(Tick(5));
        let mut bundle_builder: BundleBuilder = create_builder_with_spawn(Tick(5));

        assert_eq!(
            bundle_builder.queue_input(&state, MEMBER_ID, create_input(6, SPAWN_CURSOR, AbilityPressSet::NONE)),
            InputAcceptanceKind::AheadOfServer,
        );
    }

    #[test]
    fn queue_input_drops_a_client_tick_not_after_the_last_accepted_one() {
        let state: GameState = create_state_with_organism(Tick(9));
        let mut bundle_builder: BundleBuilder = create_builder_with_spawn(Tick(5));

        assert_eq!(
            bundle_builder.queue_input(&state, MEMBER_ID, create_input(8, SPAWN_CURSOR, AbilityPressSet::NONE)),
            InputAcceptanceKind::Accepted,
        );
        assert_eq!(
            bundle_builder.queue_input(&state, MEMBER_ID, create_input(8, SPAWN_CURSOR, AbilityPressSet::NONE)),
            InputAcceptanceKind::Stale,
        );
        assert_eq!(
            bundle_builder.queue_input(&state, MEMBER_ID, create_input(7, SPAWN_CURSOR, AbilityPressSet::NONE)),
            InputAcceptanceKind::Stale,
        );
    }

    #[test]
    fn queue_input_ignores_an_input_sampled_before_the_spawn() {
        let state: GameState = create_state_with_organism(Tick(9));
        let mut bundle_builder: BundleBuilder = create_builder_with_spawn(Tick(8));

        assert_eq!(
            bundle_builder.queue_input(&state, MEMBER_ID, create_input(7, SPAWN_CURSOR, AbilityPressSet::FIRST)),
            InputAcceptanceKind::BeforeSpawn,
        );
        assert_eq!(bundle_builder.build(&state).bundle.player_inputs, Vec::new());
    }

    #[test]
    fn queue_input_ignores_a_member_without_an_organism() {
        let mut state: GameState = create_state_with_organism(Tick(9));
        let mut bundle_builder: BundleBuilder = create_builder_with_spawn(Tick(5));
        state.members.get_mut(&MEMBER_ID).unwrap().organism = None;

        assert_eq!(
            bundle_builder.queue_input(&state, MEMBER_ID, create_input(8, SPAWN_CURSOR, AbilityPressSet::FIRST)),
            InputAcceptanceKind::WithoutOrganism,
        );
        assert_eq!(
            bundle_builder.queue_input(
                &state,
                MemberId(7),
                create_input(8, SPAWN_CURSOR, AbilityPressSet::FIRST)
            ),
            InputAcceptanceKind::WithoutOrganism,
        );
    }

    #[test]
    fn record_spawn_resets_the_clamp_to_the_spawn_cursor_with_an_empty_budget() {
        let state: GameState = create_state_with_organism(Tick(5));
        let mut bundle_builder: BundleBuilder = create_builder_with_spawn(Tick(1));
        bundle_builder.build(&state);
        bundle_builder.build(&state);

        bundle_builder.record_spawn(MEMBER_ID, SPAWN_CURSOR, Tick(5));
        bundle_builder.queue_input(
            &state,
            MEMBER_ID,
            create_input(5, WorldPoint { x: 104, y: 200 }, AbilityPressSet::NONE),
        );
        let built_bundle: BuiltBundle = bundle_builder.build(&state);
        let spawn_subpixel_cursor: SubpixelPoint = SPAWN_CURSOR.to_subpixel_point();

        assert_eq!(
            built_bundle.bundle.player_inputs[0].cursor,
            WorldPoint { x: 103, y: 200 }
        );
        assert_eq!(
            built_bundle.cursor_corrections,
            vec![CursorCorrection {
                member_id: MEMBER_ID,
                client_tick: Tick(5),
                cursor: SubpixelPoint {
                    x: spawn_subpixel_cursor.x + 3174,
                    y: spawn_subpixel_cursor.y,
                },
            }],
        );
    }

    #[test]
    fn get_rounded_world_point_rounds_halves_up_on_both_signs() {
        assert_eq!(
            get_rounded_world_point(SubpixelPoint { x: 1535, y: -1536 }),
            WorldPoint { x: 1, y: -1 },
        );
        assert_eq!(
            get_rounded_world_point(SubpixelPoint { x: 1536, y: -1537 }),
            WorldPoint { x: 2, y: -2 },
        );
    }
}
