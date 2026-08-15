//! Runtime coverage for Sovereign Okinec Ahau's member-driven attack trigger.
//!
//! Sovereign Okinec Ahau: "Whenever Sovereign Okinec Ahau attacks, for each
//! creature you control with power greater than that creature's base power, put
//! a number of +1/+1 counters on that creature equal to the difference."
//!
//! The scenario exercises the real Oracle parser and combat/trigger pipeline.
//! A pumped creature is the discriminating member: current power 4 versus base
//! power 2 produces two counters. An unpumped creature remains at zero. The
//! repeated body must rebind both the ParentTarget recipient and the difference
//! operands for each member.
//!
//! CR references (verified against docs/MagicCompRules.txt):
//! - CR 508.1a: the active player chooses which creatures attack.
//! - CR 603.2: the attack event automatically triggers the ability.
//! - CR 608.2c: the controller follows the instructions in order, including the
//!   per-member repeat and its counter instruction.
//! - CR 208.4b + CR 613.4b: base power is read before modifying counters.
//! - CR 122.1a: +1/+1 counters modify a creature's power and toughness.

use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::types::counter::CounterType;
use engine::types::identifiers::ObjectId;
use engine::types::phase::Phase;

use super::rules::run_combat;

const SOVEREIGN_ORACLE: &str = "Ward {2}\nWhenever Sovereign Okinec Ahau attacks, for each creature you control with power greater than that creature's base power, put a number of +1/+1 counters on that creature equal to the difference.";

fn plus_one_counters(runner: &GameRunner, id: ObjectId) -> u32 {
    runner
        .state()
        .objects
        .get(&id)
        .and_then(|object| object.counters.get(&CounterType::Plus1Plus1).copied())
        .unwrap_or(0)
}

#[test]
fn attacks_put_difference_counters_on_each_pumped_creature() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let sovereign = scenario
        .add_creature(P0, "Sovereign Okinec Ahau", 3, 4)
        .from_oracle_text(SOVEREIGN_ORACLE)
        .id();

    let pumped = {
        let mut creature = scenario.add_creature(P0, "Pumped Creature", 2, 2);
        creature.with_plus_counters(2);
        creature.id()
    };
    let unpumped = scenario.add_creature(P0, "Unpumped Creature", 2, 2).id();
    let mut runner = scenario.build();

    run_combat(&mut runner, vec![sovereign], vec![]);
    runner.advance_until_stack_empty();

    assert_eq!(
        plus_one_counters(&runner, pumped),
        4,
        "power 4 minus base power 2 must add two +1/+1 counters"
    );
    assert_eq!(
        plus_one_counters(&runner, unpumped),
        0,
        "a creature whose power equals base power is not in the repeated set"
    );
}

#[test]
fn sovereign_attack_does_not_count_itself_without_a_power_modifier() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let sovereign = scenario
        .add_creature(P0, "Sovereign Okinec Ahau", 3, 4)
        .from_oracle_text(SOVEREIGN_ORACLE)
        .id();
    let mut runner = scenario.build();

    run_combat(&mut runner, vec![sovereign], vec![]);
    runner.advance_until_stack_empty();

    assert_eq!(
        plus_one_counters(&runner, sovereign),
        0,
        "the source has no power/base-power difference and must not self-pump"
    );
}
