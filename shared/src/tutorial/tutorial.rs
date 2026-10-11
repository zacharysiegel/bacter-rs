use crate::ability::{AbilityPressSet, FirstAbilityKind, SecondAbilityKind, ThirdAbilityKind};
use crate::game::tick;

pub const FULLSCREEN_TASK_TICKS: u32 = tick::get_ticks_from_milliseconds(3500);
pub const SURVIVE_TASK_TICKS: u32 = tick::get_ticks_from_milliseconds(4500);
pub const SHOOT_TASK_TICKS: u32 = tick::get_ticks_from_milliseconds(10_000);

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
