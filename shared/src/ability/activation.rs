use crate::ability::{Loadout, OrganismAbilities};
use crate::game::{GameState, Tick};

pub fn run_timer_expiry_phase(state: &mut GameState, tick: Tick) {
    for member in state.members.values_mut() {
        let Some(loadout) = member.loadout else {
            continue;
        };

        let Some(organism) = member.organism.as_mut() else {
            continue;
        };

        expire_timers(&mut organism.abilities, &loadout, tick);
    }
}

pub fn expire_timers(abilities: &mut OrganismAbilities, loadout: &Loadout, tick: Tick) {
    abilities.first.expire(tick, loadout.first.cooldown_ticks());
    abilities.second.expire(tick, loadout.second.cooldown_ticks());
    abilities.third.expire(tick, loadout.third.cooldown_ticks());

    if !abilities.third.is_active() {
        abilities.third_center = None;
    }

    abilities.spore.expire(tick);

    for shot_phase in &mut abilities.shots {
        shot_phase.expire(tick);
    }

    abilities.compressed_until = abilities.compressed_until.filter(|compressed_until| *compressed_until > tick);
    abilities.frozen_until = abilities.frozen_until.filter(|frozen_until| *frozen_until > tick);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::{AbilityPhase, ThirdAbilityKind};
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::WorldPoint;
    use crate::member::MemberId;
    use crate::world::WorldShapeKind;

    const CASTER_ID: MemberId = MemberId(0);
    const TARGET_ID: MemberId = MemberId(1);

    fn create_state_with_organisms(mode: GameModeKind, positions: &[WorldPoint]) -> GameState {
        let mut state: GameState = test_fixture::create_state(mode, WorldShapeKind::Rectangle, 800);

        for (index, position) in positions.iter().enumerate() {
            let member_id: MemberId = MemberId(u32::try_from(index).unwrap());
            state.members.insert(
                member_id,
                test_fixture::create_participant_with_organism(member_id, *position),
            );
        }

        state
    }

    fn get_abilities(state: &GameState, member_id: MemberId) -> &OrganismAbilities {
        &test_fixture::get_organism(state, member_id).abilities
    }

    #[test]
    fn expire_timers_uses_the_cooldown_of_the_chosen_kind() {
        let mut loadout: Loadout = test_fixture::create_loadout();
        loadout.third = ThirdAbilityKind::Toxin;
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();
        abilities.first = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.second = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.third = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.third_center = Some(WorldPoint { x: 4, y: 4 });

        expire_timers(&mut abilities, &loadout, Tick(50));

        assert_eq!(abilities.first, AbilityPhase::Cooling { ready_at: Tick(107) });
        assert_eq!(abilities.second, AbilityPhase::Cooling { ready_at: Tick(136) });
        assert_eq!(abilities.third, AbilityPhase::Cooling { ready_at: Tick(136) });
        assert_eq!(abilities.third_center, None);
    }

    #[test]
    fn expire_timers_clears_received_effects_on_their_deadline() {
        let loadout: Loadout = test_fixture::create_loadout();
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();
        abilities.compressed_until = Some(Tick(60));
        abilities.frozen_until = Some(Tick(61));

        expire_timers(&mut abilities, &loadout, Tick(60));

        assert_eq!(abilities.compressed_until, None);
        assert_eq!(abilities.frozen_until, Some(Tick(61)));
    }

    #[test]
    fn expire_timers_keeps_an_active_field_centre() {
        let loadout: Loadout = test_fixture::create_loadout();
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();
        abilities.third = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.third_center = Some(WorldPoint { x: 4, y: 4 });

        expire_timers(&mut abilities, &loadout, Tick(49));

        assert_eq!(abilities.third_center, Some(WorldPoint { x: 4, y: 4 }));
    }

    #[test]
    fn run_timer_expiry_phase_advances_every_organism() {
        let mut state: GameState = create_state_with_organisms(
            GameModeKind::FreeForAll,
            &[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 300, y: 100 }],
        );

        for member_id in [CASTER_ID, TARGET_ID] {
            test_fixture::get_organism_mut(&mut state, member_id).abilities.second =
                AbilityPhase::Active { ends_at: Tick(5) };
        }

        run_timer_expiry_phase(&mut state, Tick(5));

        for member_id in [CASTER_ID, TARGET_ID] {
            assert_eq!(
                get_abilities(&state, member_id).second,
                AbilityPhase::Cooling { ready_at: Tick(91) },
            );
        }
    }
}
