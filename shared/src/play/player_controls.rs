use crate::geometry::SubpixelPoint;
use crate::play::{CrosshairController, KeyboardState};

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerControls {
    pub keyboard: KeyboardState,
    pub crosshair: CrosshairController,
}

impl PlayerControls {
    pub fn new() -> PlayerControls {
        PlayerControls {
            keyboard: KeyboardState::new(),
            crosshair: CrosshairController::new(SubpixelPoint { x: 0, y: 0 }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::AbilityPressSet;
    use crate::play::MovementDirection;

    #[test]
    fn new_holds_no_key_and_places_the_crosshair_at_the_origin() {
        let controls: PlayerControls = PlayerControls::new();

        assert_eq!(controls.keyboard.movement_direction(), MovementDirection { x: 0, y: 0 });
        assert_eq!(controls.keyboard.pending_ability_presses(), AbilityPressSet::NONE);
        assert_eq!(controls.crosshair.position(), SubpixelPoint { x: 0, y: 0 });
    }
}
