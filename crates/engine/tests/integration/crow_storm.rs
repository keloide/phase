//! Crow Storm's token name must flow from Oracle parsing through the normal
//! cast and Storm-copy pipelines.

use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::types::card_type::CoreType;
use engine::types::game_state::WaitingFor;
use engine::types::keywords::Keyword;
use engine::types::mana::ManaColor;
use engine::types::phase::Phase;
use engine::types::player::PlayerId;

const CROW_STORM_ORACLE: &str =
    "Create a 1/2 blue Bird creature token with flying named Storm Crow.\n\
Storm (When you cast this spell, copy it for each spell cast before it this turn.)";

const OSGOOD_TOKEN_ORACLE: &str =
    "Create a 2/2 blue Human Alien Shapeshifter creature token named Osgood, Operation Double with flying.";

const PRIOR_SPELL_ORACLE: &str = "You gain 1 life.";

fn spells_cast_by(runner: &GameRunner, player: PlayerId) -> usize {
    runner
        .state()
        .spells_cast_this_turn_by_player
        .get(&player)
        .map_or(0, |records| records.len())
}

#[test]
fn crow_storm_creates_correctly_named_tokens_for_original_and_storm_copy() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);

    let prior_spell = scenario
        .add_spell_to_hand_from_oracle(P0, "Prior Spell", true, PRIOR_SPELL_ORACLE)
        .id();
    let crow_storm = scenario
        .add_spell_to_hand(P0, "Crow Storm", false)
        .from_oracle_text_with_keywords(&["Storm"], CROW_STORM_ORACLE)
        .id();
    let mut runner = scenario.build();

    runner.state_mut().turn_number = 1;
    runner.state_mut().active_player = P0;
    runner.state_mut().priority_player = P0;
    runner.state_mut().waiting_for = WaitingFor::Priority { player: P0 };

    runner.cast(prior_spell).resolve();
    assert_eq!(
        spells_cast_by(&runner, P0),
        1,
        "the prior spell must be cast through the normal pipeline"
    );

    runner.cast(crow_storm).resolve();
    assert_eq!(
        spells_cast_by(&runner, P0),
        2,
        "Crow Storm itself is the second cast spell; its copies are not cast"
    );

    // CR 702.40a: Storm copies Crow Storm once for the one other spell cast
    // before it this turn, so the original and one copy each create a token.
    let token_ids: Vec<_> = runner
        .state()
        .battlefield
        .iter()
        .copied()
        .filter(|id| runner.state().objects[id].is_token)
        .collect();
    assert_eq!(
        token_ids.len(),
        2,
        "Crow Storm must create one token for the spell and one for its Storm copy"
    );

    // CR 111.3 + CR 111.4: the creating spell defines these characteristics,
    // including Storm Crow's name independently of its Bird subtype.
    for token_id in &token_ids {
        let token = &runner.state().objects[token_id];
        assert_eq!(token.controller, P0);
        assert_eq!(token.name, "Storm Crow");
        assert_eq!((token.power, token.toughness), (Some(1), Some(2)));
        assert_eq!(token.color, vec![ManaColor::Blue]);
        assert!(token.card_types.core_types.contains(&CoreType::Creature));
        assert!(
            token
                .card_types
                .subtypes
                .iter()
                .any(|subtype| subtype == "Bird"),
            "token must retain its Bird subtype: {:?}",
            token.card_types.subtypes
        );
        assert!(token.keywords.contains(&Keyword::Flying));
    }

    assert!(
        token_ids
            .iter()
            .all(|id| runner.state().objects[id].name != "Bird"),
        "the positive two-token assertion above prevents this default-name regression check from passing vacuously"
    );
}

#[test]
fn comma_bearing_token_name_survives_the_cast_pipeline_with_its_keyword_suffix() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let spell = scenario
        .add_spell_to_hand_from_oracle(P0, "Comma Token Spell", false, OSGOOD_TOKEN_ORACLE)
        .id();
    let mut runner = scenario.build();

    runner.state_mut().turn_number = 1;
    runner.state_mut().active_player = P0;
    runner.state_mut().priority_player = P0;
    runner.state_mut().waiting_for = WaitingFor::Priority { player: P0 };
    runner.cast(spell).resolve();

    let token_ids: Vec<_> = runner
        .state()
        .battlefield
        .iter()
        .copied()
        .filter(|id| runner.state().objects[id].is_token)
        .collect();
    assert_eq!(token_ids.len(), 1, "the spell must create its token");

    // CR 111.3 + CR 111.4: a comma is part of this token's name, while the
    // following `with flying` remains a separate defining characteristic.
    let token = &runner.state().objects[&token_ids[0]];
    assert_eq!(token.name, "Osgood, Operation Double");
    assert_eq!(token.color, vec![ManaColor::Blue]);
    assert_eq!((token.power, token.toughness), (Some(2), Some(2)));
    assert!(token.card_types.core_types.contains(&CoreType::Creature));
    assert!(
        ["Human", "Alien", "Shapeshifter"]
            .into_iter()
            .all(|subtype| token
                .card_types
                .subtypes
                .iter()
                .any(|actual| actual == subtype)),
        "token must retain each subtype: {:?}",
        token.card_types.subtypes
    );
    assert!(token.keywords.contains(&Keyword::Flying));
}
