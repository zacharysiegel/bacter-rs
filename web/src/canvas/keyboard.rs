use shared::ability::AimVector;
use shared::play::{GameKeyKind, KeyRepeatKind};
use shared::render::ViewPoint;

/// Whether the browser may also act on a key the game uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyDefaultActionKind {
    Allowed,
    Prevented,
}

/// `KeyboardEvent.code`, so the keys sit where the original's did on any layout.
pub fn get_game_key(code: &str) -> Option<GameKeyKind> {
    match code {
        "KeyA" | "ArrowLeft" => Some(GameKeyKind::MoveLeft),
        "KeyW" | "ArrowUp" => Some(GameKeyKind::MoveUp),
        "KeyD" | "ArrowRight" => Some(GameKeyKind::MoveRight),
        "KeyS" | "ArrowDown" => Some(GameKeyKind::MoveDown),
        "KeyX" => Some(GameKeyKind::FirstAbility),
        "KeyC" => Some(GameKeyKind::SecondAbility),
        "KeyV" => Some(GameKeyKind::ThirdAbility),
        "Space" => Some(GameKeyKind::FourthAbility),
        "KeyR" => Some(GameKeyKind::Respawn),
        "Escape" => Some(GameKeyKind::Pause),
        "F11" => Some(GameKeyKind::Fullscreen),
        _ => None,
    }
}

pub fn get_key_repeat(is_repeat: bool) -> KeyRepeatKind {
    if is_repeat {
        KeyRepeatKind::Repeated
    } else {
        KeyRepeatKind::Initial
    }
}

/// Arrows and space would scroll or press a focused button; F11 stays the browser's fullscreen key.
pub fn get_key_default_action(key: GameKeyKind) -> KeyDefaultActionKind {
    match key {
        GameKeyKind::MoveLeft
        | GameKeyKind::MoveUp
        | GameKeyKind::MoveRight
        | GameKeyKind::MoveDown
        | GameKeyKind::FirstAbility
        | GameKeyKind::SecondAbility
        | GameKeyKind::ThirdAbility
        | GameKeyKind::FourthAbility => KeyDefaultActionKind::Prevented,
        GameKeyKind::Respawn | GameKeyKind::Pause | GameKeyKind::Fullscreen => KeyDefaultActionKind::Allowed,
    }
}

/// The pointer's offset from the crosshair on screen, in CSS pixels, saturated to the wire range.
pub fn get_aim_vector(pointer: ViewPoint, crosshair: ViewPoint) -> AimVector {
    AimVector {
        x: get_saturated_aim_coordinate(pointer.x - crosshair.x),
        y: get_saturated_aim_coordinate(pointer.y - crosshair.y),
    }
}

fn get_saturated_aim_coordinate(offset_pixels: f32) -> i16 {
    offset_pixels.round().clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_game_key_maps_the_original_keys_and_arrows() {
        assert_eq!(get_game_key("KeyW"), Some(GameKeyKind::MoveUp));
        assert_eq!(get_game_key("ArrowUp"), Some(GameKeyKind::MoveUp));
        assert_eq!(get_game_key("KeyA"), Some(GameKeyKind::MoveLeft));
        assert_eq!(get_game_key("ArrowDown"), Some(GameKeyKind::MoveDown));
        assert_eq!(get_game_key("KeyX"), Some(GameKeyKind::FirstAbility));
        assert_eq!(get_game_key("KeyC"), Some(GameKeyKind::SecondAbility));
        assert_eq!(get_game_key("KeyV"), Some(GameKeyKind::ThirdAbility));
        assert_eq!(get_game_key("Space"), Some(GameKeyKind::FourthAbility));
        assert_eq!(get_game_key("KeyR"), Some(GameKeyKind::Respawn));
        assert_eq!(get_game_key("Escape"), Some(GameKeyKind::Pause));
        assert_eq!(get_game_key("F11"), Some(GameKeyKind::Fullscreen));
        assert_eq!(get_game_key("KeyQ"), None);
    }

    #[test]
    fn get_key_repeat_tells_an_auto_repeat_apart() {
        assert_eq!(get_key_repeat(true), KeyRepeatKind::Repeated);
        assert_eq!(get_key_repeat(false), KeyRepeatKind::Initial);
    }

    #[test]
    fn get_key_default_action_keeps_the_browser_fullscreen_key() {
        assert_eq!(
            get_key_default_action(GameKeyKind::FourthAbility),
            KeyDefaultActionKind::Prevented,
        );
        assert_eq!(
            get_key_default_action(GameKeyKind::MoveLeft),
            KeyDefaultActionKind::Prevented,
        );
        assert_eq!(
            get_key_default_action(GameKeyKind::Fullscreen),
            KeyDefaultActionKind::Allowed,
        );
    }

    #[test]
    fn get_aim_vector_is_the_rounded_offset_from_the_crosshair() {
        assert_eq!(
            get_aim_vector(ViewPoint { x: 410.6, y: 290.0 }, ViewPoint { x: 400.0, y: 300.0 }),
            AimVector { x: 11, y: -10 },
        );
        assert_eq!(
            get_aim_vector(ViewPoint { x: 1.0e9, y: -1.0e9 }, ViewPoint { x: 0.0, y: 0.0 }),
            AimVector {
                x: i16::MAX,
                y: i16::MIN,
            },
        );
    }
}
