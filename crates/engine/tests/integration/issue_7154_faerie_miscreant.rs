//! Regression for issue #7154 — Faerie Miscreant's named-card intervening-if.
//!
//! The exact Oracle condition is checked when Faerie Miscreant enters and again
//! while its trigger resolves (CR 603.4). These tests use the real cast,
//! battlefield-entry, trigger-collection, and resolution pipeline.

use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::game::zones::move_to_zone;
use engine::types::ability::{FilterProp, TargetFilter, TriggerCondition, TypeFilter};
use engine::types::actions::GameAction;
use engine::types::game_state::StackEntryKind;
use engine::types::mana::ManaCost;
use engine::types::phase::Phase;
use engine::types::zones::Zone;
use engine::types::ObjectId;

const FAERIE_MISCREANT: &str =
    "Flying\nWhen this creature enters, if you control another creature named Faerie Miscreant, draw a card.";

fn has_named_companion_condition(runner: &GameRunner, source: ObjectId) -> bool {
    runner.state().objects[&source]
        .trigger_definitions
        .as_slice()
        .iter()
        .any(|entry| matches!(
            &entry.definition.condition,
            Some(TriggerCondition::ControlsType {
                filter: TargetFilter::Typed(filter),
            }) if filter.type_filters.contains(&TypeFilter::Creature)
                && filter.properties.iter().any(|property| matches!(
                    property,
                    FilterProp::Named { name } if name == "faerie miscreant"
                ))
                && filter.properties.iter().any(|property| matches!(property, FilterProp::Another))
        ))
}

fn faerie_on_stack(runner: &GameRunner, source: ObjectId) -> bool {
    runner.state().stack.iter().any(|entry| {
        matches!(
            &entry.kind,
            StackEntryKind::TriggeredAbility { source_id, .. } if *source_id == source
        )
    })
}

#[test]
fn draws_when_another_faerie_miscreant_is_present() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_library_top(P0, &["Drawn card"]);
    scenario.add_creature(P0, "Faerie Miscreant", 1, 1);
    let faerie = scenario
        .add_creature_to_hand_from_oracle(P0, "Faerie Miscreant", 1, 1, FAERIE_MISCREANT)
        .with_mana_cost(ManaCost::zero())
        .id();

    let mut runner = scenario.build();
    assert!(
        has_named_companion_condition(&runner, faerie),
        "reach-guard: exact Oracle must synthesize the named companion condition"
    );

    let outcome = runner.cast(faerie).resolve();
    outcome.assert_hand_drawn(P0, 1);
}

#[test]
fn does_not_draw_without_another_faerie_miscreant() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_library_top(P0, &["Drawn card"]);
    let faerie = scenario
        .add_creature_to_hand_from_oracle(P0, "Faerie Miscreant", 1, 1, FAERIE_MISCREANT)
        .with_mana_cost(ManaCost::zero())
        .id();

    let mut runner = scenario.build();
    assert!(
        has_named_companion_condition(&runner, faerie),
        "reach-guard: the negative starts from the parsed named companion condition"
    );

    let outcome = runner.cast(faerie).resolve();
    outcome.assert_hand_drawn(P0, 0);
}

#[test]
fn companion_removed_after_trigger_fires_prevents_resolution_draw() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_library_top(P0, &["Drawn card"]);
    let companion = scenario.add_creature(P0, "Faerie Miscreant", 1, 1).id();
    let faerie = scenario
        .add_creature_to_hand_from_oracle(P0, "Faerie Miscreant", 1, 1, FAERIE_MISCREANT)
        .with_mana_cost(ManaCost::zero())
        .id();

    let mut runner = scenario.build();
    runner.cast(faerie).commit();

    let mut fired = false;
    for _ in 0..12 {
        let entered = runner.state().objects[&faerie].zone == Zone::Battlefield;
        if entered && faerie_on_stack(&runner, faerie) {
            fired = true;
            break;
        }
        runner
            .act(GameAction::PassPriority)
            .expect("advance creature spell to its triggered ability");
    }
    assert!(
        fired,
        "reach-guard: the condition held on entry and Faerie Miscreant's trigger reached the stack"
    );

    let hand_after_entry = runner.state().players[P0.0 as usize].hand.len();
    let mut events = Vec::new();
    move_to_zone(runner.state_mut(), companion, Zone::Graveyard, &mut events);

    for _ in 0..12 {
        if runner.state().stack.is_empty() {
            break;
        }
        runner
            .act(GameAction::PassPriority)
            .expect("resolve Faerie Miscreant's pending trigger");
    }
    assert!(runner.state().stack.is_empty(), "trigger must settle");
    assert_eq!(
        runner.state().players[P0.0 as usize].hand.len(),
        hand_after_entry,
        "the live intervening-if is false after the companion leaves, so the trigger must not draw"
    );
}
