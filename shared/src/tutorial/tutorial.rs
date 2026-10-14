use crate::ability::ability_constants;
use crate::ability::{
    AbilityPressSet, FirstAbilityKind, Loadout, SecondAbilityKind, ShotEffectKind, SporePhase, ThirdAbilityKind,
};
use crate::game::tick;
use crate::game::{GameState, MemberEvent, PlayerTickInput, SimulationEvent, Tick};
use crate::geometry::WorldPoint;
use crate::member::{Appearance, MemberId, OrganismColorKind, SkinKind};
use crate::organism::Organism;
use crate::play::{
    AbilityPressSample, FrameOutput, FramePacer, GameKeyKind, InputFocusKind, KeyboardState, LocalGame, LocalJoiner,
    PixelSize, PlayerControls,
};
use crate::play::{crosshair, local_game};
use crate::random::Pcg32;
use crate::tutorial::TutorialBot;
use crate::tutorial::tutorial_bot;

pub const FULLSCREEN_TASK_TICKS: u32 = tick::get_ticks_from_milliseconds(3500);
pub const SURVIVE_TASK_TICKS: u32 = tick::get_ticks_from_milliseconds(4500);
pub const SHOOT_TASK_TICKS: u32 = tick::get_ticks_from_milliseconds(10_000);
/// Bright colours which would hide the crosshair.
pub const TUTORIAL_PLAYER_EXCLUDED_COLORS: [OrganismColorKind; 2] = [OrganismColorKind::Sun, OrganismColorKind::Sky];
pub const TUTORIAL_PLAYER_ID: MemberId = MemberId(0);
const TUTORIAL_PLAYER_CAP: u8 = 2;
const TUTORIAL_PLAYER_SCREEN_NAME: &str = "Player";

/// In task order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TutorialTaskKind {
    Move,
    Fullscreen,
    Survive,
    Extend,
    Immortality,
    Neutralize,
    Shoot,
    Compress,
    Freeze,
    Toxin,
    Spore,
    Done,
}

impl TutorialTaskKind {
    pub fn next(self, display_mode: DisplayModeKind) -> TutorialTaskKind {
        match self {
            TutorialTaskKind::Move => match display_mode {
                DisplayModeKind::Windowed => TutorialTaskKind::Fullscreen,
                DisplayModeKind::Fullscreen => TutorialTaskKind::Survive,
            },
            TutorialTaskKind::Fullscreen => TutorialTaskKind::Survive,
            TutorialTaskKind::Survive => TutorialTaskKind::Extend,
            TutorialTaskKind::Extend => TutorialTaskKind::Immortality,
            TutorialTaskKind::Immortality => TutorialTaskKind::Neutralize,
            TutorialTaskKind::Neutralize => TutorialTaskKind::Shoot,
            TutorialTaskKind::Shoot => TutorialTaskKind::Compress,
            TutorialTaskKind::Compress => TutorialTaskKind::Freeze,
            TutorialTaskKind::Freeze => TutorialTaskKind::Toxin,
            TutorialTaskKind::Toxin => TutorialTaskKind::Spore,
            TutorialTaskKind::Spore | TutorialTaskKind::Done => TutorialTaskKind::Done,
        }
    }

    /// `None` for a task which ends on a condition.
    pub fn timed_task_ticks(self) -> Option<u32> {
        match self {
            TutorialTaskKind::Fullscreen => Some(FULLSCREEN_TASK_TICKS),
            TutorialTaskKind::Survive => Some(SURVIVE_TASK_TICKS),
            TutorialTaskKind::Shoot => Some(SHOOT_TASK_TICKS),
            TutorialTaskKind::Move
            | TutorialTaskKind::Extend
            | TutorialTaskKind::Immortality
            | TutorialTaskKind::Neutralize
            | TutorialTaskKind::Compress
            | TutorialTaskKind::Freeze
            | TutorialTaskKind::Toxin
            | TutorialTaskKind::Spore
            | TutorialTaskKind::Done => None,
        }
    }

    /// The original's per-task `activated` toggles.
    pub fn enabled_abilities(self) -> AbilityPressSet {
        match self {
            TutorialTaskKind::Move
            | TutorialTaskKind::Fullscreen
            | TutorialTaskKind::Survive
            | TutorialTaskKind::Done => AbilityPressSet::NONE,
            TutorialTaskKind::Extend | TutorialTaskKind::Compress => AbilityPressSet::FIRST,
            TutorialTaskKind::Immortality | TutorialTaskKind::Freeze => AbilityPressSet::SECOND,
            TutorialTaskKind::Neutralize | TutorialTaskKind::Toxin => AbilityPressSet::THIRD,
            TutorialTaskKind::Shoot => AbilityPressSet::FIRST.with(AbilityPressSet::SECOND),
            TutorialTaskKind::Spore => AbilityPressSet::FOURTH,
        }
    }

    pub fn first_ability(self) -> FirstAbilityKind {
        if self >= TutorialTaskKind::Shoot {
            return FirstAbilityKind::Compress;
        }

        FirstAbilityKind::Extend
    }

    pub fn second_ability(self) -> SecondAbilityKind {
        if self >= TutorialTaskKind::Shoot {
            return SecondAbilityKind::Freeze;
        }

        SecondAbilityKind::Immortality
    }

    pub fn third_ability(self) -> ThirdAbilityKind {
        if self >= TutorialTaskKind::Toxin {
            return ThirdAbilityKind::Toxin;
        }

        ThirdAbilityKind::Neutralize
    }

    pub fn is_hud_visible(self) -> bool {
        !matches!(self, TutorialTaskKind::Move | TutorialTaskKind::Survive)
    }

    /// Verbatim from the original's `Message.js`.
    pub fn message_text(self, stepping: TutorialSteppingKind) -> &'static str {
        match (self, stepping) {
            (TutorialTaskKind::Move, _) => "Use W-A-S-D (Recommended) or the arrow keys to move",
            (TutorialTaskKind::Fullscreen, _) => "Press F11 to enter fullscreen mode (Recommended)",
            (TutorialTaskKind::Survive, _) => "If the crosshair is too far from the organism, it will die",
            (TutorialTaskKind::Extend, _) => "Use the EXTEND ability to increase the organism's size",
            (TutorialTaskKind::Immortality, _) => "The IMMORTALITY ability will stop the natural atrophe of cells",
            (TutorialTaskKind::Neutralize, _) => "NEUTRALIZE will create a bubble of safety from enemy attacks",
            (TutorialTaskKind::Shoot, _) => {
                "To COMPRESS or FREEZE an enemy, press the ability key to launch a spore in the direction of the cursor\nThen press it again to activate the ability"
            }
            (TutorialTaskKind::Compress, _) => {
                "On hit, COMPRESS shrinks the size of the targeted enemy\nCOMPRESS the bot to progress"
            }
            (TutorialTaskKind::Freeze, _) => {
                "On hit, FREEZE halts all natural processes within the enemy organism\nFREEZE the bot to progress"
            }
            (TutorialTaskKind::Toxin, _) => {
                "TOXIN creates a localized bubble in which only you can survive\nDamage the bot to progress"
            }
            (TutorialTaskKind::Spore, TutorialSteppingKind::Running) => {
                "Use SPORE to jettison outer cells in all directions (Space Bar)\nDamage the bot to progress"
            }
            (TutorialTaskKind::Spore, TutorialSteppingKind::PausedForSecretion) => {
                "Reactivate the ability to cause all spores to secrete an acid, killing enemy cells"
            }
            (TutorialTaskKind::Done, _) => "Now that you're ready, press ESC to return to the menu",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayModeKind {
    Windowed,
    Fullscreen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TutorialSteppingKind {
    Running,
    /// Halfway through a spore flight in the spore task, until the spores are made to secrete.
    PausedForSecretion,
}

/// A local game teaching the controls one task at a time.
#[derive(Clone, Debug, PartialEq)]
pub struct Tutorial {
    local_game: LocalGame,
    pacer: FramePacer,
    scene_rng: Pcg32,
    task: TutorialTaskKind,
    /// The tick at which the task ends: set on entry to a timed task, or when the task's condition is met.
    task_deadline: Option<Tick>,
    stepping: TutorialSteppingKind,
    display_mode: DisplayModeKind,
    bot: Option<TutorialBot>,
    /// Applied in the next local tick.
    pending_member_events: Vec<MemberEvent>,
}

impl Tutorial {
    /// The player starts at the world centre, with the crosshair on it.
    pub fn new(
        window_size: PixelSize,
        seed: u64,
        display_mode: DisplayModeKind,
        controls: &mut PlayerControls,
    ) -> Tutorial {
        let mut scene_rng: Pcg32 = local_game::create_scene_rng(seed);
        let world_size: PixelSize = local_game::get_world_size(window_size);
        let player_cursor: WorldPoint = local_game::get_world_centre(world_size);
        let player_joiner: LocalJoiner = LocalJoiner {
            screen_name: String::from(TUTORIAL_PLAYER_SCREEN_NAME),
            appearance: Appearance {
                color: local_game::draw_color_excluding(&mut scene_rng, &TUTORIAL_PLAYER_EXCLUDED_COLORS),
                skin: SkinKind::None,
            },
            cursor: player_cursor,
        };

        controls.crosshair.snap_to(player_cursor.to_subpixel_point());

        Tutorial {
            local_game: LocalGame::new(window_size, TUTORIAL_PLAYER_CAP, seed, &[player_joiner]),
            pacer: FramePacer::new(),
            scene_rng,
            task: TutorialTaskKind::Move,
            task_deadline: None,
            stepping: TutorialSteppingKind::Running,
            display_mode,
            bot: None,
            pending_member_events: Vec::new(),
        }
    }

    pub fn state(&self) -> &GameState {
        self.local_game.state()
    }

    pub fn task(&self) -> TutorialTaskKind {
        self.task
    }

    pub fn message_text(&self) -> &'static str {
        self.task.message_text(self.stepping)
    }

    pub fn is_hud_visible(&self) -> bool {
        self.task.is_hud_visible()
    }

    pub fn enabled_abilities(&self) -> AbilityPressSet {
        self.task.enabled_abilities()
    }

    pub fn display_mode_changed(&mut self, display_mode: DisplayModeKind) {
        self.display_mode = display_mode;
    }

    pub fn advance_frame(
        &mut self,
        now_milliseconds: f64,
        controls: &mut PlayerControls,
        input_focus: InputFocusKind,
    ) -> FrameOutput {
        let frame_duration_milliseconds: f64 = self.pacer.begin_frame(now_milliseconds);
        let is_steering: bool = input_focus == InputFocusKind::Game;

        if is_steering && self.stepping == TutorialSteppingKind::Running {
            controls.crosshair.advance(
                &controls.keyboard,
                frame_duration_milliseconds,
                crosshair::CROSSHAIR_SPEED_PIXELS_PER_SECOND,
            );
        }

        if is_steering {
            let player_cursor: WorldPoint = controls.crosshair.position().to_rounded_world_point();
            self.check_held_key_tasks(&controls.keyboard, player_cursor);
        }

        let local_tick_count: u32 = self.pacer.take_local_tick_count(frame_duration_milliseconds);
        let steppable_tick_count: u32 = self.take_steppable_tick_count(local_tick_count, &controls.keyboard);
        let mut tick_applied: Option<Tick> = None;

        for _ in 0..steppable_tick_count {
            self.step_tick(controls);
            tick_applied = Some(self.state().tick);

            if self.stepping != TutorialSteppingKind::Running {
                break;
            }
        }

        if tick_applied.is_some() {
            self.pacer.record_tick_applied(now_milliseconds);
        }

        FrameOutput {
            interpolation_alpha: self.pacer.get_interpolation_alpha(now_milliseconds),
            tick_applied,
            effects: Vec::new(),
        }
    }

    /// The crosshair and the bot's cursor keep their place in the world; every organism restarts as one cell.
    pub fn resize(&mut self, window_size: PixelSize, controls: &mut PlayerControls) {
        let old_size: PixelSize = self.local_game.world_size();

        self.local_game.resize(window_size);

        let new_size: PixelSize = self.local_game.world_size();
        let crosshair_point: WorldPoint = controls.crosshair.position().to_rounded_world_point();
        let rescaled_crosshair_point: WorldPoint = local_game::rescale_point(crosshair_point, old_size, new_size);

        controls.crosshair.snap_to(rescaled_crosshair_point.to_subpixel_point());

        if let Some(bot) = self.bot.as_mut() {
            bot.cursor = local_game::rescale_point(bot.cursor, old_size, new_size);
        }
    }

    fn check_held_key_tasks(&mut self, keyboard: &KeyboardState, player_cursor: WorldPoint) {
        let is_task_done: bool = match self.task {
            TutorialTaskKind::Move => keyboard.is_any_movement_key_held(),
            TutorialTaskKind::Fullscreen => keyboard.is_held(GameKeyKind::Fullscreen),
            TutorialTaskKind::Survive
            | TutorialTaskKind::Extend
            | TutorialTaskKind::Immortality
            | TutorialTaskKind::Neutralize
            | TutorialTaskKind::Shoot
            | TutorialTaskKind::Compress
            | TutorialTaskKind::Freeze
            | TutorialTaskKind::Toxin
            | TutorialTaskKind::Spore
            | TutorialTaskKind::Done => false,
        };

        if is_task_done {
            self.start_task(self.task.next(self.display_mode), player_cursor);
        }
    }

    /// While paused for secretion, no tick is stepped until the secreting press is pending.
    fn take_steppable_tick_count(&mut self, local_tick_count: u32, keyboard: &KeyboardState) -> u32 {
        if self.stepping == TutorialSteppingKind::Running {
            return local_tick_count;
        }

        let is_secretion_pressed: bool = keyboard.pending_ability_presses().contains(AbilityPressSet::FOURTH);

        if !is_secretion_pressed {
            return 0;
        }

        self.stepping = TutorialSteppingKind::Running;

        local_tick_count
    }

    fn step_tick(&mut self, controls: &mut PlayerControls) {
        let sample: AbilityPressSample = controls.keyboard.take_ability_presses().masked(self.enabled_abilities());
        let player_cursor: WorldPoint = controls.crosshair.position().to_rounded_world_point();
        let player_input: PlayerTickInput = PlayerTickInput {
            member_id: TUTORIAL_PLAYER_ID,
            cursor: player_cursor,
            ability_presses: sample.ability_presses,
            aim: sample.aim,
        };
        let member_events: Vec<MemberEvent> = std::mem::take(&mut self.pending_member_events);

        let simulation_events: Vec<SimulationEvent> =
            self.local_game.step_local_tick(member_events, vec![player_input]);

        self.advance_task(&sample, &simulation_events, player_cursor);
        self.place_missing_organisms(player_cursor);
        self.pause_for_secretion_when_due();
    }

    fn advance_task(
        &mut self,
        sample: &AbilityPressSample,
        simulation_events: &[SimulationEvent],
        player_cursor: WorldPoint,
    ) {
        let tick: Tick = self.state().tick;

        if self.task_deadline.is_none() {
            let condition_ticks: Option<u32> = self.get_task_condition_ticks(sample, simulation_events);
            self.task_deadline = condition_ticks.map(|task_ticks| tick.plus(task_ticks));
        }

        let is_task_due: bool = self.task_deadline.is_some_and(|task_deadline| task_deadline <= tick);

        if is_task_due {
            self.start_task(self.task.next(self.display_mode), player_cursor);
        }
    }

    /// The ticks until the task ends, once its condition is met; each is the duration of what the condition started.
    fn get_task_condition_ticks(
        &self,
        sample: &AbilityPressSample,
        simulation_events: &[SimulationEvent],
    ) -> Option<u32> {
        let ability_presses: AbilityPressSet = sample.ability_presses;

        match self.task {
            TutorialTaskKind::Extend => {
                ability_presses.contains(AbilityPressSet::FIRST).then_some(ability_constants::EXTEND_ACTIVE_TICKS)
            }
            TutorialTaskKind::Immortality => ability_presses
                .contains(AbilityPressSet::SECOND)
                .then_some(ability_constants::IMMORTALITY_ACTIVE_TICKS),
            TutorialTaskKind::Neutralize => ability_presses
                .contains(AbilityPressSet::THIRD)
                .then_some(ability_constants::NEUTRALIZE_ACTIVE_TICKS),
            TutorialTaskKind::Compress => self
                .was_effect_applied_to_bot(simulation_events, ShotEffectKind::Compress)
                .then_some(ability_constants::COMPRESS_EFFECT_TICKS),
            TutorialTaskKind::Freeze => self
                .was_effect_applied_to_bot(simulation_events, ShotEffectKind::Freeze)
                .then_some(ability_constants::FREEZE_EFFECT_TICKS),
            TutorialTaskKind::Toxin => {
                self.was_bot_hit_by_player(simulation_events).then_some(ability_constants::TOXIN_ACTIVE_TICKS)
            }
            TutorialTaskKind::Spore => (self.is_player_secreting() && self.was_bot_hit_by_player(simulation_events))
                .then_some(ability_constants::SPORE_SECRETION_TICKS),
            TutorialTaskKind::Move
            | TutorialTaskKind::Fullscreen
            | TutorialTaskKind::Survive
            | TutorialTaskKind::Shoot
            | TutorialTaskKind::Done => None,
        }
    }

    fn start_task(&mut self, task: TutorialTaskKind, player_cursor: WorldPoint) {
        self.task = task;
        self.task_deadline = task.timed_task_ticks().map(|task_ticks| self.state().tick.plus(task_ticks));
        self.write_player_loadout();

        match task {
            TutorialTaskKind::Compress => self.add_bot(player_cursor),
            TutorialTaskKind::Freeze | TutorialTaskKind::Toxin | TutorialTaskKind::Spore => {
                self.clear_bot_last_hitter()
            }
            TutorialTaskKind::Move
            | TutorialTaskKind::Fullscreen
            | TutorialTaskKind::Survive
            | TutorialTaskKind::Extend
            | TutorialTaskKind::Immortality
            | TutorialTaskKind::Neutralize
            | TutorialTaskKind::Shoot
            | TutorialTaskKind::Done => {}
        }
    }

    fn write_player_loadout(&mut self) {
        let task: TutorialTaskKind = self.task;
        let player_loadout: Option<&mut Loadout> = self
            .local_game
            .state_mut()
            .members
            .get_mut(&TUTORIAL_PLAYER_ID)
            .and_then(|player| player.loadout.as_mut());

        let Some(loadout) = player_loadout else {
            return;
        };

        *loadout = Loadout {
            appearance: loadout.appearance,
            first: task.first_ability(),
            second: task.second_ability(),
            third: task.third_ability(),
        };
    }

    /// Joins in the next tick and is placed after it.
    fn add_bot(&mut self, player_cursor: WorldPoint) {
        if self.bot.is_some() {
            return;
        }

        let bot_joiner: LocalJoiner =
            tutorial_bot::create_tutorial_bot_joiner(&mut self.scene_rng, self.local_game.world_size(), player_cursor);
        let member_id: MemberId = self.state().next_member_id;

        self.pending_member_events.push(local_game::create_joined_event(member_id, &bot_joiner));
        self.bot = Some(TutorialBot {
            member_id,
            cursor: bot_joiner.cursor,
        });
    }

    fn clear_bot_last_hitter(&mut self) {
        let Some(bot) = self.bot else {
            return;
        };

        let bot_organism: Option<&mut Organism> = self.get_member_organism_mut(bot.member_id);

        if let Some(bot_organism) = bot_organism {
            bot_organism.last_hitter = None;
        }
    }

    /// The player at the crosshair, the bot at its own cursor.
    fn place_missing_organisms(&mut self, player_cursor: WorldPoint) {
        let mut member_cursors: Vec<(MemberId, WorldPoint)> = vec![(TUTORIAL_PLAYER_ID, player_cursor)];
        member_cursors.extend(self.bot.map(|bot| (bot.member_id, bot.cursor)));

        self.local_game.place_missing_organisms(&member_cursors);
    }

    fn pause_for_secretion_when_due(&mut self) {
        if self.task != TutorialTaskKind::Spore || self.stepping != TutorialSteppingKind::Running {
            return;
        }

        let player_spore: Option<&SporePhase> =
            self.get_member_organism(TUTORIAL_PLAYER_ID).map(|organism| &organism.abilities.spore);

        let Some(SporePhase::Flying { ends_at, .. }) = player_spore else {
            return;
        };

        let remaining_flight_ticks: u32 = ends_at.ticks_since(self.state().tick);
        let elapsed_flight_ticks: u32 = ability_constants::SPORE_FLIGHT_TICKS.saturating_sub(remaining_flight_ticks);

        if elapsed_flight_ticks >= ability_constants::SPORE_FLIGHT_TICKS / 2 {
            self.stepping = TutorialSteppingKind::PausedForSecretion;
        }
    }

    fn was_effect_applied_to_bot(&self, simulation_events: &[SimulationEvent], effect_kind: ShotEffectKind) -> bool {
        let Some(bot) = self.bot else {
            return false;
        };

        simulation_events.iter().any(|simulation_event| {
            matches!(
                simulation_event,
                SimulationEvent::EffectApplied { target, caster, kind }
                    if *target == bot.member_id && *caster == TUTORIAL_PLAYER_ID && *kind == effect_kind
            )
        })
    }

    /// A hit which killed the bot shows only in the death's kill credit.
    fn was_bot_hit_by_player(&self, simulation_events: &[SimulationEvent]) -> bool {
        let Some(bot) = self.bot else {
            return false;
        };

        let is_last_hitter: bool = self
            .get_member_organism(bot.member_id)
            .is_some_and(|organism| organism.last_hitter == Some(TUTORIAL_PLAYER_ID));
        let was_killed_by_player: bool = simulation_events.iter().any(|simulation_event| {
            matches!(
                simulation_event,
                SimulationEvent::OrganismDied { member_id, credited_to: Some(TUTORIAL_PLAYER_ID) }
                    if *member_id == bot.member_id
            )
        });

        is_last_hitter || was_killed_by_player
    }

    fn is_player_secreting(&self) -> bool {
        self.get_member_organism(TUTORIAL_PLAYER_ID)
            .is_some_and(|organism| matches!(organism.abilities.spore, SporePhase::Secreting { .. }))
    }

    fn get_member_organism(&self, member_id: MemberId) -> Option<&Organism> {
        self.state().members.get(&member_id)?.organism.as_ref()
    }

    fn get_member_organism_mut(&mut self, member_id: MemberId) -> Option<&mut Organism> {
        self.local_game.state_mut().members.get_mut(&member_id)?.organism.as_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use crate::geometry::SubpixelPoint;
    use crate::play::frame_pacing;
    use crate::play::{KeyRepeatKind, KeyTransitionKind};

    const SEED: u64 = 0x7070;
    const WINDOW_SIZE: PixelSize = PixelSize {
        width_pixels: 850,
        height_pixels: 650,
    };
    const WORLD_CENTER: WorldPoint = WorldPoint { x: 400, y: 300 };
    /// Grows the player's organism past one cell, so it has spores to launch.
    const GROWTH_TICKS_BEFORE_SPORE: u32 = 40;
    /// Tick periods of frames run while paused, none of which may step a tick.
    const TICKS_RUN_WHILE_PAUSED: u32 = 20;
    /// The crosshair's travel in one tick at `CROSSHAIR_SPEED_PIXELS_PER_SECOND`.
    const CROSSHAIR_TRAVEL_SUBPIXELS_PER_TICK: i32 = 3046;
    /// The most tick periods one frame steps, within `FRAME_DURATION_LIMIT_MILLISECONDS`.
    const MULTI_TICK_FRAME_TICKS: u32 = 3;

    struct TutorialHarness {
        tutorial: Tutorial,
        controls: PlayerControls,
        now_milliseconds: f64,
    }

    impl TutorialHarness {
        fn new(display_mode: DisplayModeKind) -> TutorialHarness {
            let mut controls: PlayerControls = PlayerControls::new();
            let mut tutorial: Tutorial = Tutorial::new(WINDOW_SIZE, SEED, display_mode, &mut controls);
            tutorial.advance_frame(0.0, &mut controls, InputFocusKind::Game);

            TutorialHarness {
                tutorial,
                controls,
                now_milliseconds: 0.0,
            }
        }

        /// One frame per tick period.
        fn run_ticks(&mut self, tick_count: u32) {
            for _ in 0..tick_count {
                self.run_frame_spanning_ticks(1);
            }
        }

        fn run_frame_spanning_ticks(&mut self, tick_count: u32) {
            self.now_milliseconds += f64::from(tick_count) * frame_pacing::TICK_PERIOD_MILLISECONDS;
            self.tutorial.advance_frame(self.now_milliseconds, &mut self.controls, InputFocusKind::Game);
        }

        fn change_key(&mut self, key: GameKeyKind, transition: KeyTransitionKind) {
            self.controls.keyboard.key_changed(key, transition, KeyRepeatKind::Initial);
        }

        fn press(&mut self, key: GameKeyKind) {
            self.change_key(key, KeyTransitionKind::Pressed);
            self.change_key(key, KeyTransitionKind::Released);
        }

        fn crosshair_point(&self) -> WorldPoint {
            self.controls.crosshair.position().to_rounded_world_point()
        }

        fn start_task(&mut self, task: TutorialTaskKind) {
            let player_cursor: WorldPoint = self.crosshair_point();
            self.tutorial.start_task(task, player_cursor);
        }

        fn player_organism(&self) -> &Organism {
            test_fixture::get_organism(self.tutorial.state(), TUTORIAL_PLAYER_ID)
        }

        fn player_organism_mut(&mut self) -> &mut Organism {
            test_fixture::get_organism_mut(self.tutorial.local_game.state_mut(), TUTORIAL_PLAYER_ID)
        }

        fn bot(&self) -> TutorialBot {
            self.tutorial.bot.unwrap()
        }

        fn bot_organism(&self) -> &Organism {
            test_fixture::get_organism(self.tutorial.state(), self.bot().member_id)
        }

        fn bot_organism_mut(&mut self) -> &mut Organism {
            let bot_member_id: MemberId = self.bot().member_id;

            test_fixture::get_organism_mut(self.tutorial.local_game.state_mut(), bot_member_id)
        }

        /// The task's condition, met in the tick just stepped.
        fn meet_condition(&mut self, simulation_events: &[SimulationEvent]) {
            let player_cursor: WorldPoint = self.crosshair_point();
            self.tutorial.advance_task(&AbilityPressSample::NONE, simulation_events, player_cursor);
        }
    }

    const TASK_ORDER: [TutorialTaskKind; 12] = [
        TutorialTaskKind::Move,
        TutorialTaskKind::Fullscreen,
        TutorialTaskKind::Survive,
        TutorialTaskKind::Extend,
        TutorialTaskKind::Immortality,
        TutorialTaskKind::Neutralize,
        TutorialTaskKind::Shoot,
        TutorialTaskKind::Compress,
        TutorialTaskKind::Freeze,
        TutorialTaskKind::Toxin,
        TutorialTaskKind::Spore,
        TutorialTaskKind::Done,
    ];

    #[test]
    fn next_follows_the_task_order_and_stays_done() {
        let next_tasks: Vec<TutorialTaskKind> =
            TASK_ORDER.iter().map(|task| task.next(DisplayModeKind::Windowed)).collect();

        assert_eq!(next_tasks[..11], TASK_ORDER[1..]);
        assert_eq!(next_tasks[11], TutorialTaskKind::Done);
    }

    #[test]
    fn next_skips_the_fullscreen_task_in_fullscreen() {
        assert_eq!(
            TutorialTaskKind::Move.next(DisplayModeKind::Fullscreen),
            TutorialTaskKind::Survive,
        );
    }

    #[test]
    fn enabled_abilities_enables_the_abilities_of_the_task_only() {
        let enabled_abilities: Vec<AbilityPressSet> = TASK_ORDER.iter().map(|task| task.enabled_abilities()).collect();

        assert_eq!(
            enabled_abilities,
            vec![
                AbilityPressSet::NONE,
                AbilityPressSet::NONE,
                AbilityPressSet::NONE,
                AbilityPressSet::FIRST,
                AbilityPressSet::SECOND,
                AbilityPressSet::THIRD,
                AbilityPressSet::FIRST.with(AbilityPressSet::SECOND),
                AbilityPressSet::FIRST,
                AbilityPressSet::SECOND,
                AbilityPressSet::THIRD,
                AbilityPressSet::FOURTH,
                AbilityPressSet::NONE,
            ],
        );
    }

    #[test]
    fn first_ability_switches_to_compress_from_the_shoot_task() {
        let first_abilities: [FirstAbilityKind; 3] = [
            TutorialTaskKind::Extend.first_ability(),
            TutorialTaskKind::Shoot.first_ability(),
            TutorialTaskKind::Toxin.first_ability(),
        ];

        assert_eq!(
            first_abilities,
            [
                FirstAbilityKind::Extend,
                FirstAbilityKind::Compress,
                FirstAbilityKind::Compress,
            ],
        );
    }

    #[test]
    fn second_ability_switches_to_freeze_from_the_shoot_task() {
        let second_abilities: [SecondAbilityKind; 3] = [
            TutorialTaskKind::Extend.second_ability(),
            TutorialTaskKind::Shoot.second_ability(),
            TutorialTaskKind::Toxin.second_ability(),
        ];

        assert_eq!(
            second_abilities,
            [
                SecondAbilityKind::Immortality,
                SecondAbilityKind::Freeze,
                SecondAbilityKind::Freeze,
            ],
        );
    }

    #[test]
    fn third_ability_switches_to_toxin_from_the_toxin_task() {
        let third_abilities: [ThirdAbilityKind; 3] = [
            TutorialTaskKind::Extend.third_ability(),
            TutorialTaskKind::Shoot.third_ability(),
            TutorialTaskKind::Toxin.third_ability(),
        ];

        assert_eq!(
            third_abilities,
            [
                ThirdAbilityKind::Neutralize,
                ThirdAbilityKind::Neutralize,
                ThirdAbilityKind::Toxin,
            ],
        );
    }

    #[test]
    fn is_hud_visible_hides_the_hud_during_the_move_and_survive_tasks() {
        let hidden_tasks: Vec<TutorialTaskKind> =
            TASK_ORDER.into_iter().filter(|task| !task.is_hud_visible()).collect();

        assert_eq!(hidden_tasks, vec![TutorialTaskKind::Move, TutorialTaskKind::Survive]);
    }

    #[test]
    fn message_text_gives_the_original_task_texts() {
        assert_eq!(
            TutorialTaskKind::Move.message_text(TutorialSteppingKind::Running),
            "Use W-A-S-D (Recommended) or the arrow keys to move",
        );
        assert_eq!(
            TutorialTaskKind::Compress.message_text(TutorialSteppingKind::Running),
            "On hit, COMPRESS shrinks the size of the targeted enemy\nCOMPRESS the bot to progress",
        );
        assert_eq!(
            TutorialTaskKind::Spore.message_text(TutorialSteppingKind::PausedForSecretion),
            "Reactivate the ability to cause all spores to secrete an acid, killing enemy cells",
        );
        assert_eq!(
            TutorialTaskKind::Done.message_text(TutorialSteppingKind::Running),
            "Now that you're ready, press ESC to return to the menu",
        );
    }

    #[test]
    fn timed_task_ticks_times_the_fullscreen_survive_and_shoot_tasks() {
        let timed_tasks: Vec<(TutorialTaskKind, u32)> = TASK_ORDER
            .into_iter()
            .filter_map(|task| task.timed_task_ticks().map(|task_ticks| (task, task_ticks)))
            .collect();

        assert_eq!(
            timed_tasks,
            vec![
                (TutorialTaskKind::Fullscreen, 50),
                (TutorialTaskKind::Survive, 64),
                (TutorialTaskKind::Shoot, 143),
            ],
        );
    }

    #[test]
    fn new_places_the_player_at_the_world_center_under_the_crosshair() {
        let harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        let player_organism: &Organism = harness.player_organism();

        assert_eq!(player_organism.cursor, WORLD_CENTER);
        assert_eq!(harness.controls.crosshair.position(), WORLD_CENTER.to_subpixel_point());
        assert_eq!(harness.tutorial.task(), TutorialTaskKind::Move);
        assert_eq!(harness.tutorial.state().settings.player_cap, 2);

        let appearance: Appearance = harness.tutorial.state().members[&TUTORIAL_PLAYER_ID].loadout.unwrap().appearance;

        assert!(!TUTORIAL_PLAYER_EXCLUDED_COLORS.contains(&appearance.color));
        assert_eq!(appearance.skin, SkinKind::None);
    }

    #[test]
    fn advance_frame_ends_the_move_task_once_a_movement_key_is_held() {
        let mut windowed_harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        let mut fullscreen_harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Fullscreen);

        windowed_harness.run_ticks(3);

        assert_eq!(windowed_harness.tutorial.task(), TutorialTaskKind::Move);

        for harness in [&mut windowed_harness, &mut fullscreen_harness] {
            harness.change_key(GameKeyKind::MoveUp, KeyTransitionKind::Pressed);
            harness.run_ticks(1);
        }

        assert_eq!(windowed_harness.tutorial.task(), TutorialTaskKind::Fullscreen);
        assert_eq!(fullscreen_harness.tutorial.task(), TutorialTaskKind::Survive);

        let world_center_subpixels: SubpixelPoint = WORLD_CENTER.to_subpixel_point();

        assert_eq!(
            windowed_harness.controls.crosshair.position(),
            SubpixelPoint {
                x: world_center_subpixels.x,
                y: world_center_subpixels.y - CROSSHAIR_TRAVEL_SUBPIXELS_PER_TICK,
            },
        );
    }

    #[test]
    fn advance_frame_ends_the_fullscreen_task_on_the_fullscreen_key_or_after_its_time() {
        let mut key_harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        let mut timed_harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        key_harness.start_task(TutorialTaskKind::Fullscreen);
        timed_harness.start_task(TutorialTaskKind::Fullscreen);

        key_harness.change_key(GameKeyKind::Fullscreen, KeyTransitionKind::Pressed);
        key_harness.run_ticks(1);
        timed_harness.run_ticks(FULLSCREEN_TASK_TICKS - 1);

        assert_eq!(key_harness.tutorial.task(), TutorialTaskKind::Survive);
        assert_eq!(timed_harness.tutorial.task(), TutorialTaskKind::Fullscreen);

        timed_harness.run_ticks(1);

        assert_eq!(timed_harness.tutorial.task(), TutorialTaskKind::Survive);
    }

    #[test]
    fn advance_frame_masks_the_presses_of_abilities_the_task_has_not_enabled() {
        let mut harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        harness.start_task(TutorialTaskKind::Survive);

        harness.press(GameKeyKind::FirstAbility);
        harness.run_ticks(1);

        assert!(harness.player_organism().abilities.first.is_ready());

        harness.run_ticks(SURVIVE_TASK_TICKS);
        harness.press(GameKeyKind::FirstAbility);
        harness.run_ticks(1);

        assert_eq!(harness.tutorial.task(), TutorialTaskKind::Extend);
        assert!(harness.player_organism().abilities.first.is_active());
    }

    #[test]
    fn advance_frame_ends_an_ability_task_when_the_ability_it_pressed_ends() {
        let mut harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        harness.start_task(TutorialTaskKind::Extend);

        harness.press(GameKeyKind::FirstAbility);
        harness.run_ticks(ability_constants::EXTEND_ACTIVE_TICKS);

        assert_eq!(harness.tutorial.task(), TutorialTaskKind::Extend);

        harness.run_ticks(1);

        assert_eq!(harness.tutorial.task(), TutorialTaskKind::Immortality);
    }

    #[test]
    fn start_task_writes_the_loadout_of_the_task() {
        let mut harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);

        harness.start_task(TutorialTaskKind::Shoot);
        let shoot_loadout: Loadout = harness.tutorial.state().members[&TUTORIAL_PLAYER_ID].loadout.unwrap();
        harness.start_task(TutorialTaskKind::Toxin);
        let toxin_loadout: Loadout = harness.tutorial.state().members[&TUTORIAL_PLAYER_ID].loadout.unwrap();

        assert_eq!(
            (shoot_loadout.first, shoot_loadout.second, shoot_loadout.third),
            (
                FirstAbilityKind::Compress,
                SecondAbilityKind::Freeze,
                ThirdAbilityKind::Neutralize
            ),
        );
        assert_eq!(toxin_loadout.third, ThirdAbilityKind::Toxin);
    }

    #[test]
    fn start_task_places_the_bot_for_the_compress_task_away_from_the_player() {
        let mut harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);

        harness.start_task(TutorialTaskKind::Compress);
        harness.run_ticks(1);

        let bot: TutorialBot = harness.bot();
        let bot_cursor: WorldPoint = harness.bot_organism().cursor;

        assert_eq!(bot.member_id, MemberId(1));
        assert_eq!(bot_cursor, bot.cursor);
        assert!(
            bot_cursor.distance_squared(WORLD_CENTER)
                >= tutorial_bot::TUTORIAL_BOT_MINIMUM_DISTANCE_PIXELS
                    * tutorial_bot::TUTORIAL_BOT_MINIMUM_DISTANCE_PIXELS
        );
    }

    #[test]
    fn advance_task_ends_the_compress_task_once_the_compress_effect_on_the_bot_ends() {
        let mut harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        harness.start_task(TutorialTaskKind::Compress);
        harness.run_ticks(1);
        let bot_member_id: MemberId = harness.bot().member_id;
        harness.bot_organism_mut().last_hitter = Some(TUTORIAL_PLAYER_ID);

        harness.meet_condition(&[SimulationEvent::EffectApplied {
            target: bot_member_id,
            caster: TUTORIAL_PLAYER_ID,
            kind: ShotEffectKind::Freeze,
        }]);

        assert_eq!(harness.tutorial.task_deadline, None);

        harness.meet_condition(&[SimulationEvent::EffectApplied {
            target: bot_member_id,
            caster: TUTORIAL_PLAYER_ID,
            kind: ShotEffectKind::Compress,
        }]);
        harness.run_ticks(ability_constants::COMPRESS_EFFECT_TICKS);

        assert_eq!(harness.tutorial.task(), TutorialTaskKind::Freeze);
        assert_eq!(harness.bot_organism().last_hitter, None);
    }

    #[test]
    fn advance_task_ends_the_toxin_task_once_the_player_hits_or_kills_the_bot() {
        let mut hit_harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        let mut kill_harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);

        for harness in [&mut hit_harness, &mut kill_harness] {
            harness.start_task(TutorialTaskKind::Compress);
            harness.run_ticks(1);
            harness.start_task(TutorialTaskKind::Toxin);
        }

        let bot_member_id: MemberId = kill_harness.bot().member_id;
        hit_harness.bot_organism_mut().last_hitter = Some(TUTORIAL_PLAYER_ID);
        hit_harness.meet_condition(&[]);
        kill_harness.meet_condition(&[SimulationEvent::OrganismDied {
            member_id: bot_member_id,
            credited_to: Some(TUTORIAL_PLAYER_ID),
        }]);

        for harness in [&mut hit_harness, &mut kill_harness] {
            harness.run_ticks(ability_constants::TOXIN_ACTIVE_TICKS);

            assert_eq!(harness.tutorial.task(), TutorialTaskKind::Spore);
        }
    }

    #[test]
    fn advance_frame_pauses_halfway_through_a_spore_flight_until_the_spores_secrete() {
        let mut harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        harness.run_ticks(GROWTH_TICKS_BEFORE_SPORE);
        harness.start_task(TutorialTaskKind::Spore);

        harness.press(GameKeyKind::FourthAbility);
        harness.run_ticks(1);

        assert!(matches!(
            harness.player_organism().abilities.spore,
            SporePhase::Flying { .. }
        ));

        harness.run_ticks(ability_constants::SPORE_FLIGHT_TICKS / 2);
        let paused_tick: Tick = harness.tutorial.state().tick;
        harness.run_ticks(TICKS_RUN_WHILE_PAUSED);

        assert_eq!(harness.tutorial.state().tick, paused_tick);
        assert_eq!(harness.tutorial.stepping, TutorialSteppingKind::PausedForSecretion);

        harness.press(GameKeyKind::FourthAbility);
        harness.run_ticks(1);

        assert_eq!(harness.tutorial.state().tick, paused_tick.next());
        assert!(matches!(
            harness.player_organism().abilities.spore,
            SporePhase::Secreting { .. }
        ));
    }

    #[test]
    fn advance_frame_stops_a_multi_tick_frame_at_the_halfway_tick_of_a_spore_flight() {
        let mut harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        harness.run_ticks(GROWTH_TICKS_BEFORE_SPORE);
        harness.start_task(TutorialTaskKind::Spore);

        harness.press(GameKeyKind::FourthAbility);
        harness.run_ticks(1);
        let launch_tick: Tick = harness.tutorial.state().tick;
        harness.run_ticks(ability_constants::SPORE_FLIGHT_TICKS / 2 - 1);
        harness.run_frame_spanning_ticks(MULTI_TICK_FRAME_TICKS);

        assert_eq!(
            harness.tutorial.state().tick,
            launch_tick.plus(ability_constants::SPORE_FLIGHT_TICKS / 2),
        );
        assert_eq!(harness.tutorial.stepping, TutorialSteppingKind::PausedForSecretion);
    }

    #[test]
    fn advance_task_ends_the_spore_task_after_a_secretion_hits_the_bot() {
        let mut harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        harness.start_task(TutorialTaskKind::Compress);
        harness.run_ticks(1);
        harness.start_task(TutorialTaskKind::Spore);
        harness.bot_organism_mut().last_hitter = Some(TUTORIAL_PLAYER_ID);

        harness.meet_condition(&[]);

        assert_eq!(harness.tutorial.task_deadline, None);

        harness.player_organism_mut().abilities.spore = SporePhase::Secreting {
            ends_at: Tick(500),
            spores: Vec::new(),
        };
        harness.meet_condition(&[]);
        harness.run_ticks(ability_constants::SPORE_SECRETION_TICKS);

        assert_eq!(harness.tutorial.task(), TutorialTaskKind::Done);
    }

    #[test]
    fn advance_frame_places_every_organism_again_at_its_own_cursor_after_it_dies() {
        let mut harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        harness.start_task(TutorialTaskKind::Compress);
        harness.run_ticks(1);
        let bot: TutorialBot = harness.bot();
        let crosshair_point: WorldPoint = WorldPoint { x: 120, y: 90 };
        harness.controls.crosshair.snap_to(crosshair_point.to_subpixel_point());

        for member_id in [TUTORIAL_PLAYER_ID, bot.member_id] {
            harness.tutorial.local_game.state_mut().members.get_mut(&member_id).unwrap().organism = None;
        }

        harness.run_ticks(1);

        assert_eq!(harness.player_organism().cursor, crosshair_point);
        assert_eq!(harness.bot_organism().cursor, bot.cursor);
    }

    #[test]
    fn resize_keeps_the_crosshair_and_the_bot_at_their_place_in_the_world() {
        let mut harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        harness.start_task(TutorialTaskKind::Compress);
        harness.run_ticks(1);
        let bot_cursor: WorldPoint = harness.bot().cursor;

        harness.tutorial.resize(
            PixelSize {
                width_pixels: 1650,
                height_pixels: 1250,
            },
            &mut harness.controls,
        );

        assert_eq!(
            harness.controls.crosshair.position(),
            WorldPoint {
                x: WORLD_CENTER.x * 2,
                y: WORLD_CENTER.y * 2,
            }
            .to_subpixel_point(),
        );
        assert_eq!(
            harness.bot().cursor,
            WorldPoint {
                x: bot_cursor.x * 2,
                y: bot_cursor.y * 2,
            },
        );
        assert_eq!(harness.player_organism().cells.count(), 1);
    }
}
