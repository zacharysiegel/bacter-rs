mod helpers;

use std::collections::BTreeSet;

use shared::ability::{AbilityPhase, OrganismAbilities, ShotPhase, SporePhase};
use shared::game;
use shared::game::{GameState, InputBundle};
use shared::organism::Organism;
use shared::protocol::{GameStateSerialOut, InputBundleSerialOut};
use shared::replay::ReplayLog;
use shared::round::RoundPhase;

use crate::helpers::replay_fixture;

const EXPECTED_PHASE_NAMES: [&str; 18] = [
    "ability active",
    "ability cooling",
    "ability ready",
    "compressed",
    "field center",
    "frozen",
    "round playing",
    "round post-round",
    "round pre-round",
    "round waiting",
    "shot cooling",
    "shot flying",
    "shot ready",
    "shot secreting",
    "spore cooling",
    "spore flying",
    "spore ready",
    "spore secreting",
];

fn get_ability_phase_name(ability_phase: &AbilityPhase) -> &'static str {
    match ability_phase {
        AbilityPhase::Ready => "ability ready",
        AbilityPhase::Active { .. } => "ability active",
        AbilityPhase::Cooling { .. } => "ability cooling",
    }
}

fn get_spore_phase_name(spore_phase: &SporePhase) -> &'static str {
    match spore_phase {
        SporePhase::Ready => "spore ready",
        SporePhase::Flying { .. } => "spore flying",
        SporePhase::Secreting { .. } => "spore secreting",
        SporePhase::Cooling { .. } => "spore cooling",
    }
}

fn get_shot_phase_name(shot_phase: &ShotPhase) -> &'static str {
    match shot_phase {
        ShotPhase::Ready => "shot ready",
        ShotPhase::Flying { .. } => "shot flying",
        ShotPhase::Secreting { .. } => "shot secreting",
        ShotPhase::Cooling { .. } => "shot cooling",
    }
}

fn get_round_phase_name(round_phase: &RoundPhase) -> &'static str {
    match round_phase {
        RoundPhase::Waiting => "round waiting",
        RoundPhase::PreRound => "round pre-round",
        RoundPhase::Playing => "round playing",
        RoundPhase::PostRound => "round post-round",
    }
}

fn get_ability_names(abilities: &OrganismAbilities) -> Vec<&'static str> {
    let mut phase_names: Vec<&'static str> = vec![
        get_ability_phase_name(&abilities.first),
        get_ability_phase_name(&abilities.second),
        get_ability_phase_name(&abilities.third),
        get_spore_phase_name(&abilities.spore),
        get_shot_phase_name(&abilities.shots[0]),
        get_shot_phase_name(&abilities.shots[1]),
    ];

    if abilities.third_center.is_some() {
        phase_names.push("field center");
    }

    if abilities.compressed_until.is_some() {
        phase_names.push("compressed");
    }

    if abilities.frozen_until.is_some() {
        phase_names.push("frozen");
    }

    phase_names
}

fn get_phase_names(state: &GameState) -> Vec<&'static str> {
    let mut phase_names: Vec<&'static str> =
        state.round.iter().map(|round| get_round_phase_name(&round.phase)).collect();

    for member in state.members.values() {
        let Some(organism): Option<&Organism> = member.organism.as_ref() else {
            continue;
        };

        phase_names.extend(get_ability_names(&organism.abilities));
    }

    phase_names
}

#[test]
fn try_from_restores_every_scripted_state_and_bundle() {
    let replay_log: ReplayLog = replay_fixture::read_scripted_game_log();
    let mut state: GameState = GameState::new(replay_log.header.settings.clone(), replay_log.header.seed);
    let mut observed_phase_names: BTreeSet<&'static str> = BTreeSet::new();

    for bundle in &replay_log.input_bundles {
        assert_eq!(
            &InputBundle::try_from(InputBundleSerialOut::from(bundle)).unwrap(),
            bundle,
        );

        game::step(&mut state, bundle).unwrap();

        assert_eq!(GameState::try_from(GameStateSerialOut::from(&state)).unwrap(), state);

        observed_phase_names.extend(get_phase_names(&state));
    }

    assert_eq!(observed_phase_names, BTreeSet::from(EXPECTED_PHASE_NAMES));
}
