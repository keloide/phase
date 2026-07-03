//! Anointed Peacekeeper — the ETB replacement (line 2) must, as the creature
//! enters, privately LOOK AT an opponent's hand and THEN establish a persisted
//! `Choose(CardName)`. Lines 3 & 4 (the spell tax and activated-ability tax
//! statics) already parse and read the persisted chosen name; this test proves
//! line 2 now composes the hand-look ahead of the persisted name choice.
//!
//! Drives the REAL apply() pipeline (cast creature → resolve as-enters
//! replacement → answer the name), not a hand-built state.
//!
//! Reverting the delta-4 composition drops the `RevealHand` entirely (the line
//! parses to a plain `Choose(CardName)`), so `private_look_ids` stays empty and
//! `looked_at_opponent_hand` below fails. Reverting delta 2 misroutes the hand
//! phrase; reverting delta 3 makes the look error `MissingParam` mid-entry.

use engine::types::ability::{ChoiceType, ChosenAttribute};
use engine::types::actions::GameAction;
use engine::types::game_state::{CastPaymentMode, WaitingFor};
use engine::types::phase::Phase;
use engine::types::player::PlayerId;
use std::sync::Arc;

use engine::game::scenario::GameScenario;

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

// Verbatim Oracle text (four abilities, newline-separated as printed).
const PEACEKEEPER: &str = "Vigilance\n\
As Anointed Peacekeeper enters the battlefield, look at an opponent's hand, then choose any card name.\n\
Spells your opponents cast with the chosen name cost {2} more to cast.\n\
Activated abilities of sources with the chosen name cost {2} more to activate unless they're mana abilities.";

#[test]
fn anointed_peacekeeper_etb_looks_at_opponent_hand_then_names_a_card() {
    let mut scenario = GameScenario::new_n_player(2, 7);
    scenario.at_phase(Phase::PreCombatMain);

    // The opponent (P1) holds the card the look must reveal.
    let opp_card = scenario.add_card_to_hand(P1, "Opposition Research");
    // The controller (P0) also holds a card — a private look at "an opponent's
    // hand" must never touch it (multi-authority reach guard).
    let own_card = scenario.add_card_to_hand(P0, "My Own Secret");

    let peacekeeper = {
        let mut b = scenario.add_creature_to_hand(P0, "Anointed Peacekeeper", 2, 2);
        b.from_oracle_text(PEACEKEEPER);
        b.id()
    };

    let mut runner = scenario.build();
    let card_id = runner.state().objects.get(&peacekeeper).unwrap().card_id;

    runner
        .act(GameAction::CastSpell {
            object_id: peacekeeper,
            card_id,
            targets: vec![],
            payment_mode: CastPaymentMode::Auto,
        })
        .expect("cast Anointed Peacekeeper");
    runner.advance_until_stack_empty();

    // Reach-guard: the entry paused on the persisted card-name choice sourced by
    // the Peacekeeper — proves the composed replacement reached the choose step.
    let WaitingFor::NamedChoice {
        choice_type,
        source_id,
        ..
    } = runner.state().waiting_for.clone()
    else {
        panic!(
            "as-enters replacement must pause on the card-name choice, got {}",
            runner.waiting_for_kind()
        );
    };
    assert!(
        matches!(choice_type, ChoiceType::CardName),
        "the persisted choice must be a card name"
    );
    assert_eq!(
        source_id,
        Some(peacekeeper),
        "the choice must be sourced by the Peacekeeper"
    );

    // (b) The private hand look actually resolved over the OPPONENT's hand — the
    // core revert-failing behavioral assertion for line 2.
    let looked = &runner.state().private_look_ids;
    assert!(
        looked.contains(&opp_card),
        "must have privately looked at the opponent's hand card, got {looked:?}"
    );
    assert!(
        !looked.contains(&own_card),
        "must never look at the controller's own hand"
    );
    assert_eq!(
        runner.state().private_look_player,
        Some(P0),
        "the looker is the ability controller"
    );

    // Answer the name (CR 607.2d card-name naming validates against known names).
    runner.state_mut().all_card_names = Arc::from(["Opposition Research".to_string()]);
    runner
        .act(GameAction::ChooseOption {
            choice: "Opposition Research".to_string(),
        })
        .expect("name the card");
    runner.advance_until_stack_empty();

    let pk = runner
        .state()
        .objects
        .get(&peacekeeper)
        .expect("Peacekeeper on battlefield");

    // (a) The chosen card name persists on the Peacekeeper (CR 607.2d / CR 613.1).
    assert!(
        pk.chosen_attributes
            .iter()
            .any(|a| matches!(a, ChosenAttribute::CardName(n) if n == "Opposition Research")),
        "Peacekeeper must persist the chosen card name, got {:?}",
        pk.chosen_attributes
    );

    // (c) The line-3 spell tax + line-4 activated-ability tax statics are attached
    // (they read the persisted chosen name). Pre-existing behavior, asserted here
    // as a reach guard that the full multi-line card parsed and installed both.
    assert!(
        pk.static_definitions.len() >= 2,
        "both cost-increase statics (spell tax + activated tax) must be present, got {}",
        pk.static_definitions.len()
    );
}
