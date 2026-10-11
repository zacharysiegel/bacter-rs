use std::collections::BTreeSet;

use crate::ability::{AbilityPressSet, AimVector};

const MOVEMENT_KEYS: [GameKeyKind; 4] = [
    GameKeyKind::MoveLeft,
    GameKeyKind::MoveUp,
    GameKeyKind::MoveRight,
    GameKeyKind::MoveDown,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GameKeyKind {
    MoveLeft,
    MoveUp,
    MoveRight,
    MoveDown,
    FirstAbility,
    SecondAbility,
    ThirdAbility,
    FourthAbility,
    Respawn,
    Pause,
    Fullscreen,
}

impl GameKeyKind {
    /// `None` for keys which press no ability.
    pub fn ability_press(self) -> Option<AbilityPressSet> {
        match self {
            GameKeyKind::FirstAbility => Some(AbilityPressSet::FIRST),
            GameKeyKind::SecondAbility => Some(AbilityPressSet::SECOND),
            GameKeyKind::ThirdAbility => Some(AbilityPressSet::THIRD),
            GameKeyKind::FourthAbility => Some(AbilityPressSet::FOURTH),
            GameKeyKind::MoveLeft
            | GameKeyKind::MoveUp
            | GameKeyKind::MoveRight
            | GameKeyKind::MoveDown
            | GameKeyKind::Respawn
            | GameKeyKind::Pause
            | GameKeyKind::Fullscreen => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyTransitionKind {
    Pressed,
    Released,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyRepeatKind {
    Initial,
    Repeated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AbilityPressSample {
    pub ability_presses: AbilityPressSet,
    /// Present only with a first or second press.
    pub aim: Option<AimVector>,
}

impl AbilityPressSample {
    pub const NONE: AbilityPressSample = AbilityPressSample {
        ability_presses: AbilityPressSet::NONE,
        aim: None,
    };

    pub fn masked(self, enabled_presses: AbilityPressSet) -> AbilityPressSample {
        let ability_presses: AbilityPressSet = self.ability_presses.intersection(enabled_presses);

        AbilityPressSample {
            ability_presses,
            aim: self.aim.filter(|_| ability_presses.has_shot_slot_press()),
        }
    }
}

/// -1, 0 or 1 per axis; y grows downwards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovementDirection {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyboardState {
    held_keys: BTreeSet<GameKeyKind>,
    /// Keydown edges since the last sample.
    pending_sample: AbilityPressSample,
    /// Mouse offset from the on-screen crosshair.
    pointer_aim: AimVector,
}

impl KeyboardState {
    pub fn new() -> KeyboardState {
        KeyboardState {
            held_keys: BTreeSet::new(),
            pending_sample: AbilityPressSample::NONE,
            pointer_aim: AimVector { x: 0, y: 0 },
        }
    }

    pub fn key_changed(&mut self, key: GameKeyKind, transition: KeyTransitionKind, repeat: KeyRepeatKind) {
        match transition {
            KeyTransitionKind::Pressed => self.key_pressed(key, repeat),
            KeyTransitionKind::Released => {
                self.held_keys.remove(&key);
            }
        }
    }

    pub fn aim_changed(&mut self, aim: AimVector) {
        self.pointer_aim = aim;
    }

    /// For a lost window focus, after which no keyup arrives.
    pub fn release_all_keys(&mut self) {
        self.held_keys.clear();
    }

    pub fn is_held(&self, key: GameKeyKind) -> bool {
        self.held_keys.contains(&key)
    }

    pub fn is_any_movement_key_held(&self) -> bool {
        MOVEMENT_KEYS.iter().any(|key| self.is_held(*key))
    }

    pub fn movement_direction(&self) -> MovementDirection {
        MovementDirection {
            x: self.get_axis_direction(GameKeyKind::MoveLeft, GameKeyKind::MoveRight),
            y: self.get_axis_direction(GameKeyKind::MoveUp, GameKeyKind::MoveDown),
        }
    }

    pub fn pending_ability_presses(&self) -> AbilityPressSet {
        self.pending_sample.ability_presses
    }

    pub fn take_ability_presses(&mut self) -> AbilityPressSample {
        let sample: AbilityPressSample = self.pending_sample;
        self.pending_sample = AbilityPressSample::NONE;

        sample
    }

    pub fn discard_ability_presses(&mut self) {
        self.pending_sample = AbilityPressSample::NONE;
    }

    fn key_pressed(&mut self, key: GameKeyKind, repeat: KeyRepeatKind) {
        self.held_keys.insert(key);

        if repeat == KeyRepeatKind::Repeated {
            return;
        }

        let Some(ability_press) = key.ability_press() else {
            return;
        };

        self.record_ability_press(ability_press);
    }

    fn record_ability_press(&mut self, ability_press: AbilityPressSet) {
        self.pending_sample.ability_presses = self.pending_sample.ability_presses.with(ability_press);

        if ability_press.has_shot_slot_press() {
            self.pending_sample.aim = Some(self.pointer_aim);
        }
    }

    fn get_axis_direction(&self, negative_key: GameKeyKind, positive_key: GameKeyKind) -> i32 {
        i32::from(self.is_held(positive_key)) - i32::from(self.is_held(negative_key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AIM: AimVector = AimVector { x: 30, y: -40 };

    fn press(keyboard: &mut KeyboardState, key: GameKeyKind) {
        keyboard.key_changed(key, KeyTransitionKind::Pressed, KeyRepeatKind::Initial);
    }

    #[test]
    fn key_changed_records_a_keydown_edge_of_an_ability_key() {
        let mut keyboard: KeyboardState = KeyboardState::new();

        press(&mut keyboard, GameKeyKind::ThirdAbility);

        assert_eq!(
            keyboard.take_ability_presses(),
            AbilityPressSample {
                ability_presses: AbilityPressSet::THIRD,
                aim: None,
            },
        );
    }

    #[test]
    fn key_changed_ignores_a_repeated_keydown() {
        let mut keyboard: KeyboardState = KeyboardState::new();

        keyboard.key_changed(
            GameKeyKind::FourthAbility,
            KeyTransitionKind::Pressed,
            KeyRepeatKind::Repeated,
        );

        assert_eq!(keyboard.take_ability_presses(), AbilityPressSample::NONE);
        assert!(keyboard.is_held(GameKeyKind::FourthAbility));
    }

    #[test]
    fn take_ability_presses_returns_the_edges_since_the_last_sample_and_clears_them() {
        let mut keyboard: KeyboardState = KeyboardState::new();
        press(&mut keyboard, GameKeyKind::FirstAbility);
        keyboard.key_changed(
            GameKeyKind::FirstAbility,
            KeyTransitionKind::Released,
            KeyRepeatKind::Initial,
        );
        press(&mut keyboard, GameKeyKind::ThirdAbility);

        assert_eq!(
            keyboard.take_ability_presses().ability_presses,
            AbilityPressSet::FIRST.with(AbilityPressSet::THIRD),
        );
        assert_eq!(keyboard.take_ability_presses(), AbilityPressSample::NONE);
    }

    #[test]
    fn key_changed_takes_the_pointer_aim_with_a_shot_slot_press() {
        let mut keyboard: KeyboardState = KeyboardState::new();
        keyboard.aim_changed(AIM);

        press(&mut keyboard, GameKeyKind::SecondAbility);

        assert_eq!(
            keyboard.take_ability_presses(),
            AbilityPressSample {
                ability_presses: AbilityPressSet::SECOND,
                aim: Some(AIM),
            },
        );
    }

    #[test]
    fn discard_ability_presses_drops_the_edges_made_without_an_organism() {
        let mut keyboard: KeyboardState = KeyboardState::new();
        keyboard.aim_changed(AIM);
        press(&mut keyboard, GameKeyKind::FirstAbility);

        keyboard.discard_ability_presses();

        assert_eq!(keyboard.pending_ability_presses(), AbilityPressSet::NONE);
        assert_eq!(keyboard.take_ability_presses(), AbilityPressSample::NONE);
    }

    #[test]
    fn key_changed_presses_no_ability_for_other_keys() {
        let mut keyboard: KeyboardState = KeyboardState::new();

        for key in [
            GameKeyKind::MoveUp,
            GameKeyKind::Respawn,
            GameKeyKind::Pause,
            GameKeyKind::Fullscreen,
        ] {
            press(&mut keyboard, key);
        }

        assert_eq!(keyboard.take_ability_presses(), AbilityPressSample::NONE);
        assert!(keyboard.is_held(GameKeyKind::Fullscreen));
    }

    #[test]
    fn movement_direction_cancels_opposite_keys() {
        let mut keyboard: KeyboardState = KeyboardState::new();
        press(&mut keyboard, GameKeyKind::MoveLeft);
        press(&mut keyboard, GameKeyKind::MoveUp);

        assert_eq!(keyboard.movement_direction(), MovementDirection { x: -1, y: -1 });

        press(&mut keyboard, GameKeyKind::MoveRight);

        assert_eq!(keyboard.movement_direction(), MovementDirection { x: 0, y: -1 });
        assert!(keyboard.is_any_movement_key_held());
    }

    #[test]
    fn release_all_keys_stops_every_held_key() {
        let mut keyboard: KeyboardState = KeyboardState::new();
        press(&mut keyboard, GameKeyKind::MoveDown);

        keyboard.release_all_keys();

        assert_eq!(keyboard.movement_direction(), MovementDirection { x: 0, y: 0 });
        assert!(!keyboard.is_any_movement_key_held());
    }

    #[test]
    fn masked_keeps_the_enabled_presses_and_drops_an_aim_without_a_shot_slot_press() {
        let sample: AbilityPressSample = AbilityPressSample {
            ability_presses: AbilityPressSet::FIRST.with(AbilityPressSet::FOURTH),
            aim: Some(AIM),
        };

        assert_eq!(
            sample.masked(AbilityPressSet::FOURTH),
            AbilityPressSample {
                ability_presses: AbilityPressSet::FOURTH,
                aim: None,
            },
        );
        assert_eq!(sample.masked(AbilityPressSet::FIRST).aim, Some(AIM));
    }
}
