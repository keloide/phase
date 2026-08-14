//! Regression coverage for Raubahn, Bull of Ala Mhigo.
//!
//! The card's Ward payload is dynamic: it asks for life equal to Raubahn's
//! power when the Ward trigger resolves.  Its attack trigger also has an
//! optional Equipment target followed by a required attacking-creature target.

use engine::game::scenario::{GameScenario, P0, P1};
use engine::parser::oracle::parse_oracle_text;
use engine::types::ability::{
    AbilityCost, ControllerRef, Effect, FilterProp, MultiTargetSpec, ObjectScope, QuantityExpr,
    QuantityRef, TargetFilter, TypeFilter,
};
use engine::types::actions::GameAction;
use engine::types::game_state::WaitingFor;
use engine::types::keywords::{Keyword, WardCost};
use engine::types::phase::Phase;

const RAUBAHN_ORACLE: &str = "Ward—Pay life equal to Raubahn's power.\nWhenever Raubahn attacks, attach up to one target Equipment you control to target attacking creature.";

fn assert_no_unimplemented(effect: &Effect, context: &str) {
    assert!(
        !matches!(effect, Effect::Unimplemented { .. }),
        "{context} must not be Unimplemented: {effect:?}"
    );
}

#[test]
fn raubahn_full_oracle_text_parses_ward_and_attack_attachment() {
    let parsed = parse_oracle_text(
        RAUBAHN_ORACLE,
        "Raubahn, Bull of Ala Mhigo",
        &["Ward".to_string()],
        &["Legendary".to_string(), "Creature".to_string()],
        &["Human".to_string(), "Warrior".to_string()],
    );

    assert!(
        parsed
            .extracted_keywords
            .iter()
            .any(|keyword| matches!(keyword, Keyword::Ward(WardCost::PayLifeEqualToPower))),
        "Raubahn must retain its dynamic Ward cost: {:?}",
        parsed.extracted_keywords
    );
    let attack_trigger = parsed
        .triggers
        .iter()
        .find_map(|trigger| {
            trigger
                .execute
                .as_deref()
                .filter(|ability| matches!(ability.effect.as_ref(), Effect::Attach { .. }))
        })
        .expect("Raubahn's attack trigger must have an execute ability");
    assert_no_unimplemented(&attack_trigger.effect, "Raubahn's attack trigger");
    let Effect::Attach { attachment, target } = attack_trigger.effect.as_ref() else {
        panic!(
            "Raubahn's attack trigger must attach an Equipment: {:?}",
            attack_trigger.effect
        );
    };
    assert_eq!(
        attack_trigger.multi_target,
        Some(MultiTargetSpec::up_to(QuantityExpr::Fixed { value: 1 })),
        "the Equipment target must be optional up to one"
    );
    let TargetFilter::Typed(equipment) = attachment else {
        panic!("Equipment target must be typed: {attachment:?}");
    };
    assert_eq!(equipment.controller, Some(ControllerRef::You));
    assert!(
        equipment
            .type_filters
            .iter()
            .any(|filter| matches!(filter, TypeFilter::Subtype(subtype) if subtype == "Equipment")),
        "attachment target must require the Equipment subtype: {equipment:?}"
    );
    let TargetFilter::Typed(attacker) = target else {
        panic!("attacking-creature target must be typed: {target:?}");
    };
    assert!(
        attacker
            .type_filters
            .iter()
            .any(|filter| matches!(filter, TypeFilter::Creature)),
        "host target must require a creature: {attacker:?}"
    );
    assert!(
        attacker
            .properties
            .iter()
            .any(|property| matches!(property, FilterProp::Attacking { defender: None })),
        "host target must require an attacking creature: {attacker:?}"
    );
}

#[test]
fn raubahn_ward_uses_power_at_resolution_and_charges_the_opponent() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_life(P1, 20);
    let raubahn = scenario
        .add_creature_from_oracle(P0, "Raubahn, Bull of Ala Mhigo", 2, 2, RAUBAHN_ORACLE)
        .id();
    let murder = scenario
        .add_spell_to_hand_from_oracle(P1, "Murder", true, "Destroy target creature.")
        .id();
    let mut runner = scenario.build();
    {
        let state = runner.state_mut();
        state.active_player = P1;
        state.priority_player = P1;
        state.waiting_for = WaitingFor::Priority { player: P1 };
    }

    runner.cast(murder).target_objects(&[raubahn]).commit();

    // CR 702.21b: A Ward cost that uses the source's power resolves that value
    // when the Ward trigger resolves, rather than when the target is chosen.
    {
        let object = runner
            .state_mut()
            .objects
            .get_mut(&raubahn)
            .expect("Raubahn remains on the battlefield");
        object.base_power = Some(3);
        object.power = Some(3);
    }
    runner.advance_until_stack_empty();

    let WaitingFor::UnlessPayment { player, cost, .. } = &runner.state().waiting_for else {
        panic!(
            "Raubahn's Ward must prompt the opponent for life payment, got {:?}",
            runner.state().waiting_for
        );
    };
    assert_eq!(*player, P1);
    assert!(matches!(
        cost,
        AbilityCost::PayLife {
            amount: QuantityExpr::Ref {
                qty: QuantityRef::Power {
                    scope: ObjectScope::Source,
                },
            },
        }
    ));

    runner
        .act(GameAction::PayUnlessCost { pay: true })
        .expect("the opponent pays Raubahn's Ward cost");
    assert_eq!(
        runner.state().players[P1.0 as usize].life,
        17,
        "Ward must charge the opponent Raubahn's current 3 power"
    );
    assert!(
        runner.state().stack.iter().any(|entry| entry.id == murder),
        "paying Ward must leave the targeted spell on the stack"
    );
}
