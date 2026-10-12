//! Appetite for Brains must offer and exile only a revealed card with mana
//! value four or greater.

use engine::game::scenario::{GameScenario, P0, P1};
use engine::types::actions::GameAction;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaCost, ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const APPETITE_FOR_BRAINS: &str = "Target opponent reveals their hand. You choose a card from it with mana value 4 or greater and exile that card.";

fn add_appetite_for_brains(scenario: &mut GameScenario) -> ObjectId {
    scenario
        .add_spell_to_hand_from_oracle(P0, "Appetite for Brains", false, APPETITE_FOR_BRAINS)
        .with_mana_cost(ManaCost::Cost {
            shards: vec![engine::types::mana::ManaCostShard::Black],
            generic: 0,
        })
        .id()
}

fn add_black_mana(scenario: &mut GameScenario) {
    scenario.with_mana_pool(
        P0,
        vec![ManaUnit::new(ManaType::Black, ObjectId(0), false, vec![])],
    );
}

#[test]
fn appetite_for_brains_skips_choice_when_no_revealed_card_meets_mana_value() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    add_black_mana(&mut scenario);
    let spell = add_appetite_for_brains(&mut scenario);
    let mv_three = scenario
        .add_creature_to_hand(P1, "Three-Mana Card", 0, 0)
        .with_mana_cost(ManaCost::generic(3))
        .id();

    let mut runner = scenario.build();
    runner.cast(spell).target_player(P1).resolve();

    assert!(
        !matches!(runner.state().waiting_for, WaitingFor::RevealChoice { .. }),
        "a mana-value-three card must not create a reveal choice: {:?}",
        runner.state().waiting_for
    );
    runner.advance_until_stack_empty();

    assert!(
        runner.state().stack.is_empty(),
        "the spell must resolve fully"
    );
    assert_eq!(
        runner.state().objects[&mv_three].zone,
        Zone::Hand,
        "a mana-value-three card must stay in the revealed hand"
    );
}

#[test]
fn appetite_for_brains_offers_and_exiles_only_mana_value_four_or_greater() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    add_black_mana(&mut scenario);
    let spell = add_appetite_for_brains(&mut scenario);
    let mv_three = scenario
        .add_creature_to_hand(P1, "Three-Mana Card", 0, 0)
        .with_mana_cost(ManaCost::generic(3))
        .id();
    let mv_four = scenario
        .add_creature_to_hand(P1, "Four-Mana Card", 0, 0)
        .with_mana_cost(ManaCost::generic(4))
        .id();

    let mut runner = scenario.build();
    runner.cast(spell).target_player(P1).resolve();

    let eligible = match &runner.state().waiting_for {
        WaitingFor::RevealChoice { cards, .. } => cards,
        other => panic!("expected RevealChoice after reveal, got {other:?}"),
    };
    assert!(
        eligible.contains(&mv_four),
        "the mana-value-four card must be choosable: {eligible:?}"
    );
    assert!(
        !eligible.contains(&mv_three),
        "the mana-value-three card must not be choosable: {eligible:?}"
    );

    runner
        .act(GameAction::SelectCards {
            cards: vec![mv_four],
        })
        .expect("the mana-value-four card must be a legal selection");
    runner.advance_until_stack_empty();

    assert_eq!(
        runner.state().objects[&mv_four].zone,
        Zone::Exile,
        "the chosen mana-value-four card must be exiled"
    );
    assert_eq!(
        runner.state().objects[&mv_three].zone,
        Zone::Hand,
        "the unchosen mana-value-three card must remain in hand"
    );
}
