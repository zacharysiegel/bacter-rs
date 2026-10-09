use crate::ability::ability_constants;
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

impl FirstAbilityKind {
    /// Extend's own duration, or how long a compress caster's `first` stays Active after a hit.
    pub fn active_ticks(self) -> u32 {
        match self {
            FirstAbilityKind::Extend => ability_constants::EXTEND_ACTIVE_TICKS,
            FirstAbilityKind::Compress => ability_constants::COMPRESS_EFFECT_TICKS,
        }
    }

    pub fn cooldown_ticks(self) -> u32 {
        match self {
            FirstAbilityKind::Extend => ability_constants::EXTEND_COOLDOWN_TICKS,
            FirstAbilityKind::Compress => ability_constants::COMPRESS_COOLDOWN_TICKS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondAbilityKind {
    Immortality,
    Freeze,
}

impl SecondAbilityKind {
    /// Immortality's own duration, or how long a freeze caster's `second` stays Active after a hit.
    pub fn active_ticks(self) -> u32 {
        match self {
            SecondAbilityKind::Immortality => ability_constants::IMMORTALITY_ACTIVE_TICKS,
            SecondAbilityKind::Freeze => ability_constants::FREEZE_EFFECT_TICKS,
        }
    }

    pub fn cooldown_ticks(self) -> u32 {
        match self {
            SecondAbilityKind::Immortality => ability_constants::IMMORTALITY_COOLDOWN_TICKS,
            SecondAbilityKind::Freeze => ability_constants::FREEZE_COOLDOWN_TICKS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThirdAbilityKind {
    Neutralize,
    Toxin,
}

impl ThirdAbilityKind {
    pub fn active_ticks(self) -> u32 {
        match self {
            ThirdAbilityKind::Neutralize => ability_constants::NEUTRALIZE_ACTIVE_TICKS,
            ThirdAbilityKind::Toxin => ability_constants::TOXIN_ACTIVE_TICKS,
        }
    }

    pub fn cooldown_ticks(self) -> u32 {
        match self {
            ThirdAbilityKind::Neutralize => ability_constants::NEUTRALIZE_COOLDOWN_TICKS,
            ThirdAbilityKind::Toxin => ability_constants::TOXIN_COOLDOWN_TICKS,
        }
    }
}

/// The effect a shot carries; each has its own shot slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotEffectKind {
    Compress,
    Freeze,
}

impl ShotEffectKind {
    pub fn slot_index(self) -> usize {
        match self {
            ShotEffectKind::Compress => 0,
            ShotEffectKind::Freeze => 1,
        }
    }

    pub fn effect_ticks(self) -> u32 {
        match self {
            ShotEffectKind::Compress => ability_constants::COMPRESS_EFFECT_TICKS,
            ShotEffectKind::Freeze => ability_constants::FREEZE_EFFECT_TICKS,
        }
    }
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

    pub fn is_ready(self) -> bool {
        self == AbilityPhase::Ready
    }

    /// An ended Active phase cools for `cooldown_ticks` from `tick`; an ended Cooling phase becomes Ready.
    pub fn expire(&mut self, tick: Tick, cooldown_ticks: u32) {
        let next_phase: Option<AbilityPhase> = match *self {
            AbilityPhase::Active { ends_at } if ends_at <= tick => Some(AbilityPhase::Cooling {
                ready_at: tick.plus(cooldown_ticks),
            }),
            AbilityPhase::Cooling { ready_at } if ready_at <= tick => Some(AbilityPhase::Ready),
            AbilityPhase::Ready | AbilityPhase::Active { .. } | AbilityPhase::Cooling { .. } => None,
        };

        if let Some(next_phase) = next_phase {
            *self = next_phase;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SporePhase {
    Ready,
    Flying { ends_at: Tick, spores: Vec<Projectile> },
    Secreting { ends_at: Tick, spores: Vec<Projectile> },
    Cooling { ready_at: Tick },
}

impl SporePhase {
    /// An ended flight or secretion drops its spores and cools; an ended Cooling phase becomes Ready.
    pub fn expire(&mut self, tick: Tick) {
        let next_phase: Option<SporePhase> = match self {
            SporePhase::Flying { ends_at, .. } | SporePhase::Secreting { ends_at, .. } if *ends_at <= tick => {
                Some(SporePhase::Cooling {
                    ready_at: tick.plus(ability_constants::SPORE_COOLDOWN_TICKS),
                })
            }
            SporePhase::Cooling { ready_at } if *ready_at <= tick => Some(SporePhase::Ready),
            SporePhase::Ready
            | SporePhase::Flying { .. }
            | SporePhase::Secreting { .. }
            | SporePhase::Cooling { .. } => None,
        };

        if let Some(next_phase) = next_phase {
            *self = next_phase;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotPhase {
    Ready,
    Flying { ends_at: Tick, shot: Projectile },
    Secreting { ends_at: Tick, center: SubpixelPoint },
    Cooling { ready_at: Tick },
}

impl ShotPhase {
    /// An ended flight or secretion cools; an ended Cooling phase becomes Ready.
    pub fn expire(&mut self, tick: Tick) {
        let next_phase: Option<ShotPhase> = match *self {
            ShotPhase::Flying { ends_at, .. } | ShotPhase::Secreting { ends_at, .. } if ends_at <= tick => {
                Some(ShotPhase::Cooling {
                    ready_at: tick.plus(ability_constants::SHOT_COOLDOWN_TICKS),
                })
            }
            ShotPhase::Cooling { ready_at } if ready_at <= tick => Some(ShotPhase::Ready),
            ShotPhase::Ready | ShotPhase::Flying { .. } | ShotPhase::Secreting { .. } | ShotPhase::Cooling { .. } => {
                None
            }
        };

        if let Some(next_phase) = next_phase {
            *self = next_phase;
        }
    }
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

    pub fn get_neutralize_field_center(&self, loadout: &Loadout) -> Option<WorldPoint> {
        self.third_center.filter(|_| self.is_neutralize_field_active(loadout))
    }

    pub fn get_toxin_field_center(&self, loadout: &Loadout) -> Option<WorldPoint> {
        self.third_center.filter(|_| self.is_toxin_field_active(loadout))
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

    pub fn contains(self, press: AbilityPressSet) -> bool {
        self.bits & press.bits == press.bits
    }

    pub fn with(self, press: AbilityPressSet) -> AbilityPressSet {
        AbilityPressSet {
            bits: self.bits | press.bits,
        }
    }
}

/// Mouse offset from the on-screen crosshair, in CSS px.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AimVector {
    pub x: i16,
    pub y: i16,
}

impl AimVector {
    pub fn is_zero(self) -> bool {
        self.x == 0 && self.y == 0
    }
}

pub fn is_inside_field(field_center: WorldPoint, point: WorldPoint) -> bool {
    field_center.distance_squared(point) <= ability_constants::FIELD_RADIUS_SQUARED_PIXELS
}

pub fn is_inside_spore_secretion(spore_position: SubpixelPoint, point: SubpixelPoint) -> bool {
    spore_position.distance_squared(point) <= ability_constants::SPORE_SECRETION_RADIUS_SQUARED_SUBPIXELS
}

pub fn is_inside_shot_secretion(shot_center: SubpixelPoint, point: SubpixelPoint) -> bool {
    shot_center.distance_squared(point) <= ability_constants::SHOT_SECRETION_RADIUS_SQUARED_SUBPIXELS
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

    #[test]
    fn contains_and_with_combine_presses() {
        let presses: AbilityPressSet = AbilityPressSet::FIRST.with(AbilityPressSet::FOURTH);

        assert!(presses.contains(AbilityPressSet::FIRST));
        assert!(presses.contains(AbilityPressSet::FOURTH));
        assert!(!presses.contains(AbilityPressSet::SECOND));
        assert_eq!(presses.bits(), 0b1001);
    }

    #[test]
    fn active_ticks_and_cooldown_ticks_follow_each_kind() {
        assert_eq!(FirstAbilityKind::Extend.active_ticks(), 64);
        assert_eq!(FirstAbilityKind::Compress.active_ticks(), 50);
        assert_eq!(FirstAbilityKind::Compress.cooldown_ticks(), 57);
        assert_eq!(SecondAbilityKind::Immortality.active_ticks(), 50);
        assert_eq!(SecondAbilityKind::Freeze.active_ticks(), 57);
        assert_eq!(SecondAbilityKind::Freeze.cooldown_ticks(), 86);
        assert_eq!(ThirdAbilityKind::Neutralize.cooldown_ticks(), 93);
        assert_eq!(ThirdAbilityKind::Toxin.active_ticks(), 57);
    }

    #[test]
    fn slot_index_gives_compress_slot_zero_and_freeze_slot_one() {
        assert_eq!(ShotEffectKind::Compress.slot_index(), 0);
        assert_eq!(ShotEffectKind::Freeze.slot_index(), 1);
        assert_eq!(ShotEffectKind::Freeze.effect_ticks(), 57);
    }

    #[test]
    fn is_ready_holds_only_for_ready() {
        assert!(AbilityPhase::Ready.is_ready());
        assert!(!AbilityPhase::Cooling { ready_at: Tick(3) }.is_ready());
        assert!(!AbilityPhase::Active { ends_at: Tick(3) }.is_ready());
    }

    #[test]
    fn is_zero_holds_only_for_the_zero_vector() {
        assert!(AimVector { x: 0, y: 0 }.is_zero());
        assert!(!AimVector { x: 0, y: -1 }.is_zero());
    }

    #[test]
    fn ability_phase_expire_cools_then_readies_on_the_deadline() {
        let mut phase: AbilityPhase = AbilityPhase::Active { ends_at: Tick(64) };

        phase.expire(Tick(63), 57);
        assert_eq!(phase, AbilityPhase::Active { ends_at: Tick(64) });

        phase.expire(Tick(64), 57);
        assert_eq!(phase, AbilityPhase::Cooling { ready_at: Tick(121) });

        phase.expire(Tick(120), 57);
        assert_eq!(phase, AbilityPhase::Cooling { ready_at: Tick(121) });

        phase.expire(Tick(121), 57);
        assert_eq!(phase, AbilityPhase::Ready);
    }

    #[test]
    fn spore_phase_expire_drops_the_spores_and_cools() {
        let spore: Projectile = Projectile {
            position: SubpixelPoint { x: 0, y: 0 },
            velocity: SubpixelVector { x: 1, y: 0 },
        };
        let mut flying_phase: SporePhase = SporePhase::Flying {
            ends_at: Tick(24),
            spores: vec![spore],
        };
        let mut secreting_phase: SporePhase = SporePhase::Secreting {
            ends_at: Tick(30),
            spores: vec![spore],
        };

        flying_phase.expire(Tick(24));
        secreting_phase.expire(Tick(30));

        assert_eq!(flying_phase, SporePhase::Cooling { ready_at: Tick(131) });
        assert_eq!(secreting_phase, SporePhase::Cooling { ready_at: Tick(137) });

        flying_phase.expire(Tick(131));
        assert_eq!(flying_phase, SporePhase::Ready);
    }

    #[test]
    fn shot_phase_expire_cools_a_flight_or_a_secretion() {
        let mut flying_phase: ShotPhase = ShotPhase::Flying {
            ends_at: Tick(21),
            shot: Projectile {
                position: SubpixelPoint { x: 0, y: 0 },
                velocity: SubpixelVector { x: 0, y: 1 },
            },
        };
        let mut secreting_phase: ShotPhase = ShotPhase::Secreting {
            ends_at: Tick(11),
            center: SubpixelPoint { x: 0, y: 0 },
        };

        flying_phase.expire(Tick(20));
        assert!(matches!(flying_phase, ShotPhase::Flying { .. }));

        flying_phase.expire(Tick(21));
        secreting_phase.expire(Tick(11));

        assert_eq!(flying_phase, ShotPhase::Cooling { ready_at: Tick(50) });
        assert_eq!(secreting_phase, ShotPhase::Cooling { ready_at: Tick(40) });

        secreting_phase.expire(Tick(40));
        assert_eq!(secreting_phase, ShotPhase::Ready);
    }

    #[test]
    fn get_field_centers_follow_the_third_ability_kind() {
        let mut abilities: OrganismAbilities = create_active_abilities();
        abilities.third_center = Some(WorldPoint { x: 5, y: 6 });
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

        assert_eq!(
            abilities.get_neutralize_field_center(&neutralize_loadout),
            Some(WorldPoint { x: 5, y: 6 }),
        );
        assert_eq!(abilities.get_toxin_field_center(&neutralize_loadout), None);
        assert_eq!(
            abilities.get_toxin_field_center(&toxin_loadout),
            Some(WorldPoint { x: 5, y: 6 }),
        );

        abilities.third = AbilityPhase::Cooling { ready_at: Tick(90) };

        assert_eq!(abilities.get_toxin_field_center(&toxin_loadout), None);
    }

    #[test]
    fn is_inside_field_includes_the_radius() {
        let field_center: WorldPoint = WorldPoint { x: 100, y: 100 };

        assert!(is_inside_field(field_center, WorldPoint { x: 160, y: 100 }));
        assert!(!is_inside_field(field_center, WorldPoint { x: 161, y: 100 }));
    }

    #[test]
    fn is_inside_secretion_includes_the_radius() {
        let center: SubpixelPoint = SubpixelPoint { x: 0, y: 0 };

        assert!(is_inside_spore_secretion(center, SubpixelPoint { x: 25_197, y: 0 }));
        assert!(!is_inside_spore_secretion(center, SubpixelPoint { x: 25_198, y: 0 }));
        assert!(is_inside_shot_secretion(center, SubpixelPoint { x: 12_598, y: 0 }));
        assert!(!is_inside_shot_secretion(center, SubpixelPoint { x: 12_599, y: 0 }));
    }
}
