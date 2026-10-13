//! Non-regression for issue #735 — distinguish chosen-target power from a
//! genuine entering-object referent. Ent's Fury's own announced target binds
//! `ObjectScope::Target`; Tribute's entering creature keeps
//! `ObjectScope::CostPaidObject`. Genuine source/phase-subject references
//! (Amalia, Lily) retain `Source`.
//!
//! Cards covered:
//!   - Tribute to the World Tree: "Whenever a creature you control enters, draw
//!     a card if its power is 3 or greater. …" — "its" = the ENTERING creature.
//!   - Ent's Fury: "Put a +1/+1 counter on target creature you control if its
//!     power is 4 or greater. …" — "its" = the TARGET creature.
//!
//! CR references (verified against docs/MagicCompRules.txt):
//!   - CR 115.1a + CR 601.2c: Ent's Fury's targets are chosen as it is cast.
//!   - CR 608.2h: the power comparison reads the referenced object's current
//!     info once, when the effect is applied.
//!   - CR 608.2k: an effect referring to an untargeted object previously named
//!     by its cost or trigger condition still affects that object.

use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::game::triggers::{drain_order_triggers_with_identity, process_triggers};
use engine::game::zones::{create_object, move_to_zone};
use engine::types::card_type::CoreType;
use engine::types::counter::CounterType;
use engine::types::identifiers::{CardId, ObjectId};
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const TRIBUTE_ORACLE: &str = "Whenever a creature you control enters, draw a card if its power is 3 or greater. Otherwise, put two +1/+1 counters on it.";

const ENTS_FURY_ORACLE: &str = "Put a +1/+1 counter on target creature you control if its power is 4 or greater. Then that creature gets +1/+1 until end of turn and fights target creature you don't control.";

fn hand_size(runner: &GameRunner, player: engine::types::player::PlayerId) -> usize {
    runner.state().players[player.0 as usize].hand.len()
}

fn p1p1(runner: &GameRunner, id: ObjectId) -> u32 {
    runner.state().objects[&id]
        .counters
        .get(&CounterType::Plus1Plus1)
        .copied()
        .unwrap_or(0)
}

/// Create a creature in `owner`'s hand with the given power and move it to the
/// battlefield, firing "creature you control enters" triggers.
fn enter_creature(runner: &mut GameRunner, owner: engine::types::player::PlayerId, power: i32) {
    let cid = {
        let state = runner.state_mut();
        let card_id = CardId(state.next_object_id);
        let id = create_object(state, card_id, owner, "Bystander".to_string(), Zone::Hand);
        let obj = state.objects.get_mut(&id).unwrap();
        obj.card_types.core_types.push(CoreType::Creature);
        obj.base_card_types = obj.card_types.clone();
        obj.power = Some(power);
        obj.toughness = Some(power.max(1));
        obj.base_power = Some(power);
        obj.base_toughness = Some(power.max(1));
        id
    };
    let mut events = Vec::new();
    move_to_zone(runner.state_mut(), cid, Zone::Battlefield, &mut events);
    process_triggers(runner.state_mut(), &events);
    drain_order_triggers_with_identity(runner.state_mut());
    runner.advance_until_stack_empty();
}

fn setup_tribute() -> GameRunner {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let mut builder = scenario.add_creature(P0, "Tribute to the World Tree", 0, 0);
    builder.as_enchantment();
    builder.from_oracle_text(TRIBUTE_ORACLE);
    // Library padding so a draw does not deck P0.
    for _ in 0..10 {
        scenario.add_card_to_library_top(P0, "Forest");
    }
    scenario.build()
}

/// NON-REGRESSION (Tribute): an entering creature with power ≥ 3 draws a card —
/// "its power" binds the ENTERING creature (CostPaidObject), so a 3-power
/// creature satisfies `Power{CostPaidObject} GE 3` and P0 draws.
#[test]
fn tribute_entering_power_3_draws_a_card() {
    let mut runner = setup_tribute();
    let before = hand_size(&runner, P0);

    enter_creature(&mut runner, P0, 3);

    assert_eq!(
        hand_size(&runner, P0),
        before + 1,
        "an entering 3-power creature must make Tribute draw a card (its = entering creature)"
    );
}

/// NON-REGRESSION (Tribute): an entering creature with power < 3 does NOT draw;
/// the else branch puts two +1/+1 counters on the entering creature instead.
#[test]
fn tribute_entering_power_2_puts_counters_not_draw() {
    let mut runner = setup_tribute();
    let before = hand_size(&runner, P0);

    enter_creature(&mut runner, P0, 2);

    assert_eq!(
        hand_size(&runner, P0),
        before,
        "an entering 2-power creature must NOT draw (its power 2 < 3)"
    );
    // The entering creature ("Bystander") received two +1/+1 counters.
    let bystander = runner
        .state()
        .battlefield
        .iter()
        .find(|id| runner.state().objects[id].name == "Bystander")
        .copied()
        .expect("the entering creature is on the battlefield");
    assert_eq!(
        p1p1(&runner, bystander),
        2,
        "the else branch must put two +1/+1 counters on the entering creature"
    );
}

/// REGRESSION (Ent's Fury): "its power" reads the own announced target via
/// `Power{Target}`, while genuine cost/entering references retain CostPaidObject.
/// CR 608.2c + CR 608.2h: the first instruction reads the ally's current power 4,
/// so it receives one counter before the independent pump instruction.
/// CR 122.1a + CR 611.2a + CR 613.4c: counter +1/+1 and EOT pump +1/+1 make it 6/6.
/// CR 701.14a + CR 701.14d: it then fights the 3/3 with current powers as noncombat
/// damage. CostPaidObject remains a cost/trigger/effect referent, with no target
/// fallback.
#[test]
fn ents_fury_own_target_gets_counter_then_pump() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    // P0's own creature (the counter/pump/fight subject) at power 4.
    let ally = scenario.add_creature(P0, "Strong Ally", 4, 4).id();
    // An opponent creature to satisfy "fights target creature you don't control".
    let enemy = scenario.add_creature(P1, "Enemy", 3, 3).id();
    let spell = scenario
        .add_spell_to_hand_from_oracle(P0, "Ent's Fury", false, ENTS_FURY_ORACLE)
        .with_mana_cost(engine::types::mana::ManaCost::zero())
        .id();

    let mut runner = scenario.build();
    engine::game::layers::evaluate_layers(runner.state_mut());
    assert_eq!(
        runner.state().objects[&ally].power,
        Some(4),
        "precondition: the ally target has power 4"
    );

    // Two target slots: the subject (a creature you control) and the fight
    // opponent (a creature you don't control).
    let outcome = runner.cast(spell).target_objects(&[ally, enemy]).resolve();

    // CR 608.2c + CR 608.2h: the own target's initial power 4 satisfies the gate.
    assert_eq!(
        p1p1(&runner, ally),
        1,
        "Ent's Fury must put one +1/+1 counter on its own 4-power target"
    );
    // CR 122.1a + CR 611.2a + CR 613.4c: the counter and separate pump each add 1.
    assert_eq!(
        outcome.state().objects[&ally].power,
        Some(6),
        "Ent's Fury's counter and independent +1/+1 pump must both apply (4 -> 6)"
    );
    // CR 701.14a + CR 704.5g: the 6/6 target survives the 3/3's fight damage.
    assert_eq!(
        outcome.state().objects[&ally].zone,
        Zone::Battlefield,
        "the countered and pumped 6/6 target survives fighting a 3/3"
    );
}
