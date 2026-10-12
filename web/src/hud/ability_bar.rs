use leptos::prelude::*;

use shared::ability::ability_constants;
use shared::ability::{
    AbilityIconKind, AbilityPhase, FirstAbilityKind, Loadout, OrganismAbilities, SecondAbilityKind, ShotEffectKind,
    ShotPhase, SporePhase, ThirdAbilityKind,
};
use shared::game::{GameState, Tick};
use shared::member::{Member, MemberId};
use shared::organism::Organism;

use crate::hud::AbilityIconView;

pub const ABILITY_SLOT_COUNT: usize = 4;
/// The original's X, C, V and space, space drawn as an underscore.
pub const ABILITY_KEY_LABELS: [&str; ABILITY_SLOT_COUNT] = ["X", "C", "V", "_"];
const SLOT_SPACING_PIXELS: f32 = 100.0;
/// The slots are centred on the window centre.
const FIRST_SLOT_OFFSET_PIXELS: f32 = -SLOT_SPACING_PIXELS * (ABILITY_SLOT_COUNT - 1) as f32 / 2.0;
pub const DIAL_BACK_RADIUS_PIXELS: f32 = 30.0;
const DIAL_RADIUS_PIXELS: f32 = 29.0;
const SHOT_DIAL_RADIUS_PIXELS: f32 = 8.0;
/// Room outside the shot dial for its outline.
const SHOT_DIAL_OUTLINE_MARGIN_PIXELS: f32 = 1.0;

/// A dial's grey disc, full when ready.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AbilityDialKind {
    Full,
    /// Clockwise from twelve o'clock.
    Partial {
        filled_proportion: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectDotKind {
    /// The own Extend, Immortality or Neutralize.
    OwnEffect,
    /// Compressed, frozen, or the own Toxin field.
    EnemyEffect,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AbilitySlotView {
    pub icon: AbilityIconKind,
    pub key_label: &'static str,
    pub dial: AbilityDialKind,
    /// The shot a Compress or Freeze slot carries.
    pub shot_dial: Option<AbilityDialKind>,
    pub effect_dots: Vec<EffectDotKind>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AbilityBarView {
    pub slots: [AbilitySlotView; ABILITY_SLOT_COUNT],
}

impl AbilityBarView {
    /// `None` without an own organism.
    pub fn from_state(state: &GameState, own_member_id: MemberId) -> Option<AbilityBarView> {
        let own_member: &Member = state.members.get(&own_member_id)?;
        let loadout: &Loadout = own_member.loadout.as_ref()?;
        let organism: &Organism = own_member.organism.as_ref()?;
        let abilities: &OrganismAbilities = &organism.abilities;
        let tick: Tick = state.tick;

        Some(AbilityBarView {
            slots: [
                get_first_slot(abilities, loadout, tick),
                get_second_slot(abilities, loadout, tick),
                get_third_slot(abilities, loadout, tick),
                get_fourth_slot(abilities, tick),
            ],
        })
    }
}

#[component]
pub fn AbilityBar(ability_bar: Signal<Option<AbilityBarView>>) -> impl IntoView {
    move || {
        ability_bar.get().map(|ability_bar_view| {
            let slot_views: Vec<AnyView> = ability_bar_view
                .slots
                .into_iter()
                .enumerate()
                .map(|(slot_index, slot)| render_slot(slot_index, slot))
                .collect();

            view! { <div class="ability-bar">{slot_views}</div> }
        })
    }
}

/// An SVG path of the dial's filled part, centred on the origin; empty for nothing filled.
pub fn get_dial_path_data(dial: AbilityDialKind, radius: f32) -> String {
    let filled_proportion: f32 = match dial {
        AbilityDialKind::Full => 1.0,
        AbilityDialKind::Partial { filled_proportion } => filled_proportion,
    };

    if filled_proportion <= 0.0 {
        return String::new();
    }

    if filled_proportion >= 1.0 {
        return format!("M {radius} 0 A {radius} {radius} 0 1 1 -{radius} 0 A {radius} {radius} 0 1 1 {radius} 0 Z");
    }

    let end_angle: f32 = filled_proportion * std::f32::consts::TAU;
    let end_x: f32 = radius * end_angle.sin();
    let end_y: f32 = -radius * end_angle.cos();
    let large_arc_flag: u8 = if filled_proportion > 0.5 { 1 } else { 0 };

    format!("M 0 0 L 0 -{radius} A {radius} {radius} 0 {large_arc_flag} 1 {end_x:.3} {end_y:.3} Z")
}

/// A square SVG `viewBox` centred on the origin.
pub fn get_centred_view_box(half_width_pixels: f32) -> String {
    let width_pixels: String = get_centred_width(half_width_pixels);

    format!("-{half_width_pixels} -{half_width_pixels} {width_pixels} {width_pixels}")
}

pub fn get_centred_width(half_width_pixels: f32) -> String {
    (half_width_pixels * 2.0).to_string()
}

fn render_slot(slot_index: usize, slot: AbilitySlotView) -> AnyView {
    let slot_offset_pixels: f32 = FIRST_SLOT_OFFSET_PIXELS + SLOT_SPACING_PIXELS * slot_index as f32;
    let slot_style: String = format!("left: calc(50% + {slot_offset_pixels}px)");
    let dial_path_data: String = get_dial_path_data(slot.dial, DIAL_RADIUS_PIXELS);
    let shot_dial_view: Option<AnyView> = slot.shot_dial.map(render_shot_dial);
    let effect_dot_views: Vec<AnyView> = slot.effect_dots.into_iter().map(render_effect_dot).collect();
    let dial_view_box: String = get_centred_view_box(DIAL_BACK_RADIUS_PIXELS);
    let dial_width_pixels: String = get_centred_width(DIAL_BACK_RADIUS_PIXELS);

    view! {
        <div class="ability-slot" style=slot_style>
            <div class="ability-key-tab">{slot.key_label}</div>
            <svg class="ability-dial" viewBox=dial_view_box width=dial_width_pixels.clone() height=dial_width_pixels>
                <circle class="ability-dial-back" r=DIAL_BACK_RADIUS_PIXELS.to_string()></circle>
                <path class="ability-dial-fill" d=dial_path_data></path>
            </svg>
            <AbilityIconView icon=slot.icon />
            {shot_dial_view}
            {effect_dot_views}
        </div>
    }
    .into_any()
}

fn render_shot_dial(shot_dial: AbilityDialKind) -> AnyView {
    let shot_dial_path_data: String = get_dial_path_data(shot_dial, SHOT_DIAL_RADIUS_PIXELS);
    let shot_dial_half_width_pixels: f32 = SHOT_DIAL_RADIUS_PIXELS + SHOT_DIAL_OUTLINE_MARGIN_PIXELS;
    let shot_dial_view_box: String = get_centred_view_box(shot_dial_half_width_pixels);
    let shot_dial_width_pixels: String = get_centred_width(shot_dial_half_width_pixels);

    view! {
        <svg
            class="ability-shot-dial"
            viewBox=shot_dial_view_box
            width=shot_dial_width_pixels.clone()
            height=shot_dial_width_pixels
        >
            <path class="ability-shot-dial-fill" d=shot_dial_path_data></path>
        </svg>
    }
    .into_any()
}

fn render_effect_dot(effect_dot: EffectDotKind) -> AnyView {
    let effect_dot_class: &str = match effect_dot {
        EffectDotKind::OwnEffect => "ability-effect-dot ability-effect-dot-own",
        EffectDotKind::EnemyEffect => "ability-effect-dot ability-effect-dot-enemy",
    };

    view! { <div class=effect_dot_class></div> }.into_any()
}

fn get_first_slot(abilities: &OrganismAbilities, loadout: &Loadout, tick: Tick) -> AbilitySlotView {
    let (icon, carried_shot): (AbilityIconKind, Option<ShotEffectKind>) = match loadout.first {
        FirstAbilityKind::Extend => (AbilityIconKind::Extend, None),
        FirstAbilityKind::Compress => (AbilityIconKind::Compress, Some(ShotEffectKind::Compress)),
    };

    AbilitySlotView {
        icon,
        key_label: ABILITY_KEY_LABELS[0],
        dial: get_ability_dial(
            abilities.first,
            loadout.first.active_ticks(),
            loadout.first.cooldown_ticks(),
            tick,
        ),
        shot_dial: get_carried_shot_dial(abilities, carried_shot, tick),
        effect_dots: get_effect_dots(abilities.is_extended(loadout), abilities.is_compressed()),
    }
}

fn get_second_slot(abilities: &OrganismAbilities, loadout: &Loadout, tick: Tick) -> AbilitySlotView {
    let (icon, carried_shot): (AbilityIconKind, Option<ShotEffectKind>) = match loadout.second {
        SecondAbilityKind::Immortality => (AbilityIconKind::Immortality, None),
        SecondAbilityKind::Freeze => (AbilityIconKind::Freeze, Some(ShotEffectKind::Freeze)),
    };

    AbilitySlotView {
        icon,
        key_label: ABILITY_KEY_LABELS[1],
        dial: get_ability_dial(
            abilities.second,
            loadout.second.active_ticks(),
            loadout.second.cooldown_ticks(),
            tick,
        ),
        shot_dial: get_carried_shot_dial(abilities, carried_shot, tick),
        effect_dots: get_effect_dots(abilities.is_immortal(loadout), abilities.is_frozen()),
    }
}

fn get_third_slot(abilities: &OrganismAbilities, loadout: &Loadout, tick: Tick) -> AbilitySlotView {
    let icon: AbilityIconKind = match loadout.third {
        ThirdAbilityKind::Neutralize => AbilityIconKind::Neutralize,
        ThirdAbilityKind::Toxin => AbilityIconKind::Toxin,
    };

    AbilitySlotView {
        icon,
        key_label: ABILITY_KEY_LABELS[2],
        dial: get_ability_dial(
            abilities.third,
            loadout.third.active_ticks(),
            loadout.third.cooldown_ticks(),
            tick,
        ),
        shot_dial: None,
        effect_dots: get_effect_dots(
            abilities.is_neutralize_field_active(loadout),
            abilities.is_toxin_field_active(loadout),
        ),
    }
}

fn get_fourth_slot(abilities: &OrganismAbilities, tick: Tick) -> AbilitySlotView {
    let (icon, dial): (AbilityIconKind, AbilityDialKind) = match &abilities.spore {
        SporePhase::Ready => (AbilityIconKind::Spore, AbilityDialKind::Full),
        SporePhase::Flying { ends_at, .. } => (
            AbilityIconKind::Spore,
            get_remaining_dial(*ends_at, ability_constants::SPORE_FLIGHT_TICKS, tick),
        ),
        SporePhase::Secreting { ends_at, .. } => (
            AbilityIconKind::Secrete,
            get_remaining_dial(*ends_at, ability_constants::SPORE_SECRETION_TICKS, tick),
        ),
        SporePhase::Cooling { ready_at } => (
            AbilityIconKind::Spore,
            get_elapsed_dial(*ready_at, ability_constants::SPORE_COOLDOWN_TICKS, tick),
        ),
    };

    AbilitySlotView {
        icon,
        key_label: ABILITY_KEY_LABELS[3],
        dial,
        shot_dial: None,
        effect_dots: Vec::new(),
    }
}

/// Draining while Active, refilling while Cooling.
fn get_ability_dial(phase: AbilityPhase, active_ticks: u32, cooldown_ticks: u32, tick: Tick) -> AbilityDialKind {
    match phase {
        AbilityPhase::Ready => AbilityDialKind::Full,
        AbilityPhase::Active { ends_at } => get_remaining_dial(ends_at, active_ticks, tick),
        AbilityPhase::Cooling { ready_at } => get_elapsed_dial(ready_at, cooldown_ticks, tick),
    }
}

fn get_carried_shot_dial(
    abilities: &OrganismAbilities,
    carried_shot: Option<ShotEffectKind>,
    tick: Tick,
) -> Option<AbilityDialKind> {
    carried_shot.map(|shot_effect| get_shot_dial(&abilities.shots[shot_effect.slot_index()], tick))
}

fn get_shot_dial(shot: &ShotPhase, tick: Tick) -> AbilityDialKind {
    match shot {
        ShotPhase::Ready => AbilityDialKind::Full,
        ShotPhase::Flying { ends_at, .. } => get_remaining_dial(*ends_at, ability_constants::SHOT_FLIGHT_TICKS, tick),
        ShotPhase::Secreting { ends_at, .. } => {
            get_remaining_dial(*ends_at, ability_constants::SHOT_SECRETION_TICKS, tick)
        }
        ShotPhase::Cooling { ready_at } => get_elapsed_dial(*ready_at, ability_constants::SHOT_COOLDOWN_TICKS, tick),
    }
}

fn get_remaining_dial(ends_at: Tick, duration_ticks: u32, tick: Tick) -> AbilityDialKind {
    AbilityDialKind::Partial {
        filled_proportion: get_tick_proportion(ends_at.ticks_since(tick), duration_ticks),
    }
}

fn get_elapsed_dial(ready_at: Tick, duration_ticks: u32, tick: Tick) -> AbilityDialKind {
    AbilityDialKind::Partial {
        filled_proportion: 1.0 - get_tick_proportion(ready_at.ticks_since(tick), duration_ticks),
    }
}

fn get_tick_proportion(tick_count: u32, duration_ticks: u32) -> f32 {
    if duration_ticks == 0 {
        return 0.0;
    }

    (tick_count as f32 / duration_ticks as f32).clamp(0.0, 1.0)
}

fn get_effect_dots(has_own_effect: bool, has_enemy_effect: bool) -> Vec<EffectDotKind> {
    [
        (has_own_effect, EffectDotKind::OwnEffect),
        (has_enemy_effect, EffectDotKind::EnemyEffect),
    ]
    .into_iter()
    .filter(|(is_shown, _)| *is_shown)
    .map(|(_, effect_dot)| effect_dot)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::game::GameModeKind;
    use shared::game::test_fixture;
    use shared::geometry::WorldPoint;
    use shared::world::WorldShapeKind;

    const OWN_MEMBER_ID: MemberId = MemberId(0);

    fn create_state_with_own_organism(loadout: Loadout) -> GameState {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        let mut own_member: Member =
            test_fixture::create_participant_with_organism(OWN_MEMBER_ID, WorldPoint { x: 100, y: 100 });
        own_member.loadout = Some(loadout);
        test_fixture::insert_member(&mut state, own_member);
        state.tick = Tick(100);

        state
    }

    fn get_own_abilities_mut(state: &mut GameState) -> &mut OrganismAbilities {
        &mut test_fixture::get_organism_mut(state, OWN_MEMBER_ID).abilities
    }

    #[test]
    fn from_state_is_none_without_an_own_organism() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        test_fixture::insert_member(&mut state, test_fixture::create_participant(OWN_MEMBER_ID));

        assert_eq!(AbilityBarView::from_state(&state, OWN_MEMBER_ID), None);
    }

    #[test]
    fn from_state_shows_ready_abilities_as_full_dials_with_the_loadout_icons() {
        let state: GameState = create_state_with_own_organism(test_fixture::create_loadout());

        let ability_bar: AbilityBarView = AbilityBarView::from_state(&state, OWN_MEMBER_ID).unwrap();
        let icons: Vec<AbilityIconKind> = ability_bar.slots.iter().map(|slot| slot.icon).collect();
        let key_labels: Vec<&str> = ability_bar.slots.iter().map(|slot| slot.key_label).collect();

        assert_eq!(
            icons,
            vec![
                AbilityIconKind::Extend,
                AbilityIconKind::Immortality,
                AbilityIconKind::Neutralize,
                AbilityIconKind::Spore,
            ],
        );
        assert_eq!(key_labels, vec!["X", "C", "V", "_"]);
        assert!(ability_bar.slots.iter().all(|slot| slot.dial == AbilityDialKind::Full));
        assert!(ability_bar.slots.iter().all(|slot| slot.shot_dial.is_none()));
    }

    #[test]
    fn from_state_drains_an_active_ability_and_refills_a_cooling_one() {
        let mut state: GameState = create_state_with_own_organism(test_fixture::create_loadout());
        let extend_active_ticks: u32 = ability_constants::EXTEND_ACTIVE_TICKS;
        let immortality_cooldown_ticks: u32 = ability_constants::IMMORTALITY_COOLDOWN_TICKS;
        let abilities: &mut OrganismAbilities = get_own_abilities_mut(&mut state);
        abilities.first = AbilityPhase::Active {
            ends_at: Tick(100).plus(extend_active_ticks / 2),
        };
        abilities.second = AbilityPhase::Cooling {
            ready_at: Tick(100).plus(immortality_cooldown_ticks / 4),
        };

        let ability_bar: AbilityBarView = AbilityBarView::from_state(&state, OWN_MEMBER_ID).unwrap();

        assert_eq!(
            ability_bar.slots[0].dial,
            AbilityDialKind::Partial {
                filled_proportion: (extend_active_ticks / 2) as f32 / extend_active_ticks as f32,
            },
        );
        assert_eq!(
            ability_bar.slots[1].dial,
            AbilityDialKind::Partial {
                filled_proportion: 1.0 - (immortality_cooldown_ticks / 4) as f32 / immortality_cooldown_ticks as f32,
            },
        );
        assert_eq!(ability_bar.slots[0].effect_dots, vec![EffectDotKind::OwnEffect]);
        assert!(ability_bar.slots[1].effect_dots.is_empty());
    }

    #[test]
    fn from_state_gives_shot_carriers_a_shot_dial() {
        let mut loadout: Loadout = test_fixture::create_loadout();
        loadout.first = FirstAbilityKind::Compress;
        loadout.second = SecondAbilityKind::Freeze;
        let mut state: GameState = create_state_with_own_organism(loadout);
        get_own_abilities_mut(&mut state).shots[ShotEffectKind::Freeze.slot_index()] = ShotPhase::Flying {
            ends_at: Tick(100),
            shot: test_fixture::PROJECTILE,
        };

        let ability_bar: AbilityBarView = AbilityBarView::from_state(&state, OWN_MEMBER_ID).unwrap();

        assert_eq!(ability_bar.slots[0].icon, AbilityIconKind::Compress);
        assert_eq!(ability_bar.slots[0].shot_dial, Some(AbilityDialKind::Full));
        assert_eq!(
            ability_bar.slots[1].shot_dial,
            Some(AbilityDialKind::Partial { filled_proportion: 0.0 }),
        );
    }

    #[test]
    fn from_state_marks_an_enemy_effect_but_never_a_casters_own_timer() {
        let mut loadout: Loadout = test_fixture::create_loadout();
        loadout.first = FirstAbilityKind::Compress;
        let mut state: GameState = create_state_with_own_organism(loadout);
        let abilities: &mut OrganismAbilities = get_own_abilities_mut(&mut state);
        abilities.first = AbilityPhase::Active { ends_at: Tick(130) };
        abilities.frozen_until = Some(Tick(120));

        let ability_bar: AbilityBarView = AbilityBarView::from_state(&state, OWN_MEMBER_ID).unwrap();

        assert!(ability_bar.slots[0].effect_dots.is_empty());
        assert_eq!(ability_bar.slots[1].effect_dots, vec![EffectDotKind::EnemyEffect]);
    }

    #[test]
    fn from_state_shows_secretion_on_the_fourth_slot() {
        let mut state: GameState = create_state_with_own_organism(test_fixture::create_loadout());
        get_own_abilities_mut(&mut state).spore = SporePhase::Secreting {
            ends_at: Tick(100),
            spores: Vec::new(),
        };

        let ability_bar: AbilityBarView = AbilityBarView::from_state(&state, OWN_MEMBER_ID).unwrap();

        assert_eq!(ability_bar.slots[3].icon, AbilityIconKind::Secrete);
        assert_eq!(
            ability_bar.slots[3].dial,
            AbilityDialKind::Partial { filled_proportion: 0.0 },
        );
    }

    #[test]
    fn from_state_marks_an_own_neutralize_field_as_an_own_effect() {
        let mut state: GameState = create_state_with_own_organism(test_fixture::create_loadout());
        get_own_abilities_mut(&mut state).third = AbilityPhase::Active { ends_at: Tick(130) };

        let ability_bar: AbilityBarView = AbilityBarView::from_state(&state, OWN_MEMBER_ID).unwrap();

        assert_eq!(ability_bar.slots[2].effect_dots, vec![EffectDotKind::OwnEffect]);
    }

    #[test]
    fn from_state_marks_an_own_toxin_field_as_an_enemy_effect() {
        let mut loadout: Loadout = test_fixture::create_loadout();
        loadout.third = ThirdAbilityKind::Toxin;
        let mut state: GameState = create_state_with_own_organism(loadout);
        get_own_abilities_mut(&mut state).third = AbilityPhase::Active { ends_at: Tick(130) };

        let ability_bar: AbilityBarView = AbilityBarView::from_state(&state, OWN_MEMBER_ID).unwrap();

        assert_eq!(ability_bar.slots[2].icon, AbilityIconKind::Toxin);
        assert_eq!(ability_bar.slots[2].effect_dots, vec![EffectDotKind::EnemyEffect]);
    }

    #[test]
    fn from_state_drains_a_flying_spore_and_refills_a_cooling_one() {
        let mut state: GameState = create_state_with_own_organism(test_fixture::create_loadout());
        let spore_flight_ticks: u32 = ability_constants::SPORE_FLIGHT_TICKS;
        let spore_cooldown_ticks: u32 = ability_constants::SPORE_COOLDOWN_TICKS;
        get_own_abilities_mut(&mut state).spore = SporePhase::Flying {
            ends_at: Tick(100).plus(spore_flight_ticks / 2),
            spores: Vec::new(),
        };

        let flying_ability_bar: AbilityBarView = AbilityBarView::from_state(&state, OWN_MEMBER_ID).unwrap();

        get_own_abilities_mut(&mut state).spore = SporePhase::Cooling {
            ready_at: Tick(100).plus(spore_cooldown_ticks / 4),
        };

        let cooling_ability_bar: AbilityBarView = AbilityBarView::from_state(&state, OWN_MEMBER_ID).unwrap();

        assert_eq!(flying_ability_bar.slots[3].icon, AbilityIconKind::Spore);
        assert_eq!(
            flying_ability_bar.slots[3].dial,
            AbilityDialKind::Partial {
                filled_proportion: (spore_flight_ticks / 2) as f32 / spore_flight_ticks as f32,
            },
        );
        assert_eq!(cooling_ability_bar.slots[3].icon, AbilityIconKind::Spore);
        assert_eq!(
            cooling_ability_bar.slots[3].dial,
            AbilityDialKind::Partial {
                filled_proportion: 1.0 - (spore_cooldown_ticks / 4) as f32 / spore_cooldown_ticks as f32,
            },
        );
    }

    #[test]
    fn get_centred_view_box_spans_twice_the_half_width_around_the_origin() {
        assert_eq!(get_centred_view_box(30.0), "-30 -30 60 60");
        assert_eq!(get_centred_view_box(9.0), "-9 -9 18 18");
    }

    #[test]
    fn get_dial_path_data_draws_a_wedge_clockwise_from_twelve_o_clock() {
        assert_eq!(
            get_dial_path_data(AbilityDialKind::Partial { filled_proportion: 0.0 }, 29.0),
            "",
        );
        assert_eq!(
            get_dial_path_data(
                AbilityDialKind::Partial {
                    filled_proportion: 0.25
                },
                29.0
            ),
            "M 0 0 L 0 -29 A 29 29 0 0 1 29.000 0.000 Z",
        );
        assert_eq!(
            get_dial_path_data(
                AbilityDialKind::Partial {
                    filled_proportion: 0.75
                },
                8.0
            ),
            "M 0 0 L 0 -8 A 8 8 0 1 1 -8.000 -0.000 Z",
        );
        assert_eq!(
            get_dial_path_data(AbilityDialKind::Full, 8.0),
            "M 8 0 A 8 8 0 1 1 -8 0 A 8 8 0 1 1 8 0 Z",
        );
    }
}
