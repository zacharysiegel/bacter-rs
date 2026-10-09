use crate::game::Tick;
use crate::geometry::{SubpixelPoint, SubpixelVector, WorldPoint};
use crate::member::{Appearance, TeamKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loadout {
    pub appearance: Appearance,
    pub first: FirstAbilityKind,
    pub second: SecondAbilityKind,
    pub third: ThirdAbilityKind,
}

impl Loadout {
    pub fn with_team_color(self, team: Option<TeamKind>) -> Loadout {
        Loadout {
            appearance: self.appearance.with_team_color(team),
            first: self.first,
            second: self.second,
            third: self.third,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstAbilityKind {
    Extend,
    Compress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondAbilityKind {
    Immortality,
    Freeze,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThirdAbilityKind {
    Neutralize,
    Toxin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbilityPhase {
    Ready,
    Active { ends_at: Tick },
    Cooling { ready_at: Tick },
}

impl AbilityPhase {
    pub fn is_active(self) -> bool {
        matches!(self, AbilityPhase::Active { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SporePhase {
    Ready,
    Flying { ends_at: Tick, spores: Vec<Projectile> },
    Secreting { ends_at: Tick, spores: Vec<Projectile> },
    Cooling { ready_at: Tick },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotPhase {
    Ready,
    Flying { ends_at: Tick, shot: Projectile },
    Secreting { ends_at: Tick, center: SubpixelPoint },
    Cooling { ready_at: Tick },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Projectile {
    pub position: SubpixelPoint,
    pub velocity: SubpixelVector,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrganismAbilities {
    pub first: AbilityPhase,
    pub second: AbilityPhase,
    pub third: AbilityPhase,
    pub third_center: Option<WorldPoint>,
    pub spore: SporePhase,
    /// Slot 0 carries compress, slot 1 carries freeze.
    pub shots: [ShotPhase; 2],
    pub compressed_until: Option<Tick>,
    pub frozen_until: Option<Tick>,
}

impl OrganismAbilities {
    pub fn all_ready() -> OrganismAbilities {
        OrganismAbilities {
            first: AbilityPhase::Ready,
            second: AbilityPhase::Ready,
            third: AbilityPhase::Ready,
            third_center: None,
            spore: SporePhase::Ready,
            shots: [ShotPhase::Ready, ShotPhase::Ready],
            compressed_until: None,
            frozen_until: None,
        }
    }

    pub fn is_extended(&self, loadout: &Loadout) -> bool {
        self.first.is_active() && loadout.first == FirstAbilityKind::Extend
    }

    pub fn is_immortal(&self, loadout: &Loadout) -> bool {
        self.second.is_active() && loadout.second == SecondAbilityKind::Immortality
    }

    pub fn is_neutralize_field_active(&self, loadout: &Loadout) -> bool {
        self.third.is_active() && loadout.third == ThirdAbilityKind::Neutralize
    }

    pub fn is_toxin_field_active(&self, loadout: &Loadout) -> bool {
        self.third.is_active() && loadout.third == ThirdAbilityKind::Toxin
    }

    pub fn is_compressed(&self) -> bool {
        self.compressed_until.is_some()
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen_until.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AbilityPressSet {
    bits: u8,
}

impl AbilityPressSet {
    pub const NONE: AbilityPressSet = AbilityPressSet { bits: 0 };
    pub const FIRST: AbilityPressSet = AbilityPressSet { bits: 1 };
    pub const SECOND: AbilityPressSet = AbilityPressSet { bits: 1 << 1 };
    pub const THIRD: AbilityPressSet = AbilityPressSet { bits: 1 << 2 };
    pub const FOURTH: AbilityPressSet = AbilityPressSet { bits: 1 << 3 };
    const KNOWN_BITS: u8 = 0b1111;

    /// `None` when any bit other than the four press bits is set.
    pub fn from_bits(bits: u8) -> Option<AbilityPressSet> {
        if bits & !AbilityPressSet::KNOWN_BITS != 0 {
            return None;
        }

        Some(AbilityPressSet { bits })
    }

    pub fn bits(self) -> u8 {
        self.bits
    }
}

/// Mouse offset from the on-screen crosshair, in CSS px.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AimVector {
    pub x: i16,
    pub y: i16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::member::{OrganismColorKind, SkinKind};

    fn create_loadout(first: FirstAbilityKind, second: SecondAbilityKind, third: ThirdAbilityKind) -> Loadout {
        Loadout {
            appearance: Appearance {
                color: OrganismColorKind::Leaf,
                skin: SkinKind::Grid,
            },
            first,
            second,
            third,
        }
    }

    fn create_active_abilities() -> OrganismAbilities {
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();
        abilities.first = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.second = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.third = AbilityPhase::Active { ends_at: Tick(50) };

        abilities
    }

    #[test]
    fn is_extended_requires_the_extend_loadout() {
        let abilities: OrganismAbilities = create_active_abilities();
        let extend_loadout: Loadout = create_loadout(
            FirstAbilityKind::Extend,
            SecondAbilityKind::Immortality,
            ThirdAbilityKind::Neutralize,
        );
        let compress_loadout: Loadout = create_loadout(
            FirstAbilityKind::Compress,
            SecondAbilityKind::Immortality,
            ThirdAbilityKind::Neutralize,
        );

        assert!(abilities.is_extended(&extend_loadout));
        assert!(!abilities.is_extended(&compress_loadout));
        assert!(!OrganismAbilities::all_ready().is_extended(&extend_loadout));
    }

    #[test]
    fn is_immortal_requires_the_immortality_loadout() {
        let abilities: OrganismAbilities = create_active_abilities();
        let immortality_loadout: Loadout = create_loadout(
            FirstAbilityKind::Extend,
            SecondAbilityKind::Immortality,
            ThirdAbilityKind::Neutralize,
        );
        let freeze_loadout: Loadout = create_loadout(
            FirstAbilityKind::Extend,
            SecondAbilityKind::Freeze,
            ThirdAbilityKind::Neutralize,
        );

        assert!(abilities.is_immortal(&immortality_loadout));
        assert!(!abilities.is_immortal(&freeze_loadout));
    }

    #[test]
    fn is_neutralize_field_active_and_is_toxin_field_active_follow_the_third_ability_kind() {
        let abilities: OrganismAbilities = create_active_abilities();
        let neutralize_loadout: Loadout = create_loadout(
            FirstAbilityKind::Extend,
            SecondAbilityKind::Immortality,
            ThirdAbilityKind::Neutralize,
        );
        let toxin_loadout: Loadout = create_loadout(
            FirstAbilityKind::Extend,
            SecondAbilityKind::Immortality,
            ThirdAbilityKind::Toxin,
        );

        assert!(abilities.is_neutralize_field_active(&neutralize_loadout));
        assert!(!abilities.is_toxin_field_active(&neutralize_loadout));
        assert!(abilities.is_toxin_field_active(&toxin_loadout));
        assert!(!abilities.is_neutralize_field_active(&toxin_loadout));
    }

    #[test]
    fn is_compressed_and_is_frozen_hold_while_a_deadline_is_set() {
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();

        assert!(!abilities.is_compressed());
        assert!(!abilities.is_frozen());

        abilities.compressed_until = Some(Tick(10));
        abilities.frozen_until = Some(Tick(10));

        assert!(abilities.is_compressed());
        assert!(abilities.is_frozen());
    }

    #[test]
    fn from_bits_rejects_unknown_bits() {
        assert_eq!(
            AbilityPressSet::from_bits(0b1111).map(AbilityPressSet::bits),
            Some(0b1111),
        );
        assert_eq!(AbilityPressSet::from_bits(0b1_0000), None);
        assert_eq!(AbilityPressSet::from_bits(0), Some(AbilityPressSet::NONE));
    }

    #[test]
    fn with_team_color_forces_only_the_color() {
        let loadout: Loadout = create_loadout(
            FirstAbilityKind::Compress,
            SecondAbilityKind::Freeze,
            ThirdAbilityKind::Toxin,
        );
        let team_loadout: Loadout = loadout.with_team_color(Some(TeamKind::Pink));

        assert_eq!(team_loadout.appearance.color, OrganismColorKind::Petal);
        assert_eq!(team_loadout.appearance.skin, SkinKind::Grid);
        assert_eq!(team_loadout.first, FirstAbilityKind::Compress);
        assert_eq!(team_loadout.second, SecondAbilityKind::Freeze);
        assert_eq!(team_loadout.third, ThirdAbilityKind::Toxin);
        assert_eq!(loadout.with_team_color(None), loadout);
    }
}
