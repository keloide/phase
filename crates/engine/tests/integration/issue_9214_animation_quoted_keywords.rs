//! Issue #9214: an animation's keyword preceding a quoted ability must reach
//! the permanent through the real activation and continuous effect pipeline.

use engine::game::layers::evaluate_layers;
use engine::game::scenario::{GameScenario, P0};
use engine::parser::parse_oracle_text;
use engine::types::ability::{ContinuousModification, Effect};
use engine::types::card_type::CoreType;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::keywords::Keyword;
use engine::types::mana::{ManaColor, ManaType, ManaUnit};
use engine::types::phase::Phase;

const HIVE_ORACLE: &str = "If you control two or more other lands, this land enters tapped.\n\
{T}: Add {B}.\n\
{3}{B}: Until end of turn, this land becomes a 3/3 black Beholder creature with menace and \"Whenever this creature attacks, exile target card from defending player's graveyard.\" It's still a land.";

const ARGOTHIAN_ORACLE: &str = "Put two +1/+1 counters on each of X target lands you control. They each become 0/0 Elemental creatures with reach, haste, and \"When this creature leaves the battlefield, conjure a card named Forest onto the battlefield tapped.\" They're still lands.";

const GODDRIC_ORACLE: &str = "Haste\nCelebration — As long as two or more nonland permanents entered the battlefield under your control this turn, Goddric is a Dragon with base power and toughness 4/4, flying, and \"{R}: Dragons you control get +1/+0 until end of turn.\" (It loses all other creature types.)";

const HONEST_WORK_ORACLE: &str = "Enchant creature an opponent controls\nWhen this Aura enters, tap enchanted creature and remove all counters from it.\nEnchanted creature loses all abilities and is a Citizen with base power and toughness 1/1 and \"{T}: Add {C}\" named Humble Merchant. (It loses all other creature types and names.)";

fn has_keyword_modification(value: &serde_json::Value, keyword: &str) -> bool {
    match value {
        serde_json::Value::Object(fields) => {
            (fields.get("type").and_then(serde_json::Value::as_str) == Some("AddKeyword")
                && fields.get("keyword").and_then(serde_json::Value::as_str) == Some(keyword))
                || fields
                    .values()
                    .any(|value| has_keyword_modification(value, keyword))
        }
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| has_keyword_modification(value, keyword)),
        _ => false,
    }
}

#[test]
fn production_oracle_paths_keep_quoted_animation_keywords() {
    for (name, oracle, types, keywords) in [
        (
            "Hive of the Eye Tyrant",
            HIVE_ORACLE,
            "Land",
            &["Menace"][..],
        ),
        (
            "Argothian Uprooting",
            ARGOTHIAN_ORACLE,
            "Sorcery",
            &["Reach", "Haste"][..],
        ),
    ] {
        let parsed = parse_oracle_text(oracle, name, &[], &[types.to_string()], &[]);
        let tree = serde_json::to_value(&parsed).expect("parsed abilities serialize");
        for keyword in keywords {
            assert!(
                has_keyword_modification(&tree, keyword),
                "{name} must grant {keyword} through the production parser: {parsed:?}"
            );
        }
    }

    let goddric = parse_oracle_text(
        GODDRIC_ORACLE,
        "Goddric, Cloaked Reveler",
        &["Haste".to_string()],
        &["Creature".to_string()],
        &["Human".to_string()],
    );
    let static_grant = goddric
        .statics
        .iter()
        .find(|definition| {
            definition.modifications.iter().any(|modification| {
                matches!(
                    modification,
                    ContinuousModification::AddKeyword {
                        keyword: Keyword::Flying
                    }
                )
            })
        })
        .expect("Goddric's conditional animation must grant flying");
    assert!(static_grant
        .modifications
        .contains(&ContinuousModification::SetPower { value: 4 }));
    assert!(static_grant
        .modifications
        .contains(&ContinuousModification::SetToughness { value: 4 }));

    let honest = parse_oracle_text(
        HONEST_WORK_ORACLE,
        "Honest Work",
        &[],
        &["Enchantment".to_string()],
        &["Aura".to_string()],
    );
    let static_grant = honest.statics.iter().find(|definition| definition.modifications.iter().any(|modification| matches!(modification, ContinuousModification::SetTextName { name } if name == "Humble Merchant"))).expect("Honest Work's quote-only grant and name must reach a continuous static");
    assert!(static_grant
        .modifications
        .contains(&ContinuousModification::SetPower { value: 1 }));
    assert!(static_grant
        .modifications
        .contains(&ContinuousModification::SetToughness { value: 1 }));
    assert!(static_grant
        .modifications
        .contains(&ContinuousModification::AddSubtype {
            subtype: "Citizen".to_string()
        }));
    assert!(static_grant.modifications.iter().any(|modification| matches!(modification, ContinuousModification::GrantAbility { definition } if matches!(definition.effect.as_ref(), Effect::Mana { .. }))));
}

/// CR 613.1d + CR 613.1f + CR 613.4b: the animation changes the activating
/// land's type, abilities, and base P/T, leaving a second land untouched.
#[test]
fn hive_activation_grants_menace_and_keeps_land_type() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let hive = scenario
        .add_land_from_oracle(P0, "Hive of the Eye Tyrant", HIVE_ORACLE)
        .id();
    let other = scenario.add_land_from_oracle(P0, "Other Land", "").id();
    scenario.with_mana_pool(
        P0,
        vec![
            ManaUnit::new(ManaType::Black, ObjectId(0), false, vec![]),
            ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]),
            ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]),
            ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]),
        ],
    );
    let mut runner = scenario.build();
    assert!(!runner.state().objects[&hive].has_keyword(&Keyword::Menace));
    assert_eq!(
        runner.state().objects[&hive].card_types.core_types,
        vec![CoreType::Land]
    );
    assert!(runner.state().objects[&hive].trigger_definitions.is_empty());

    let animate = runner.state().objects[&hive]
        .abilities
        .iter()
        .position(|ability| {
            ability
                .description
                .as_deref()
                .is_some_and(|text| text.contains("becomes a 3/3"))
        })
        .expect("Hive's printed animation ability must parse");
    let outcome = runner.activate(hive, animate).resolve();
    assert!(matches!(
        outcome.final_waiting_for(),
        WaitingFor::Priority { .. }
    ));
    assert!(
        outcome.state().stack.is_empty(),
        "activation must resolve before layer assertions"
    );
    evaluate_layers(runner.state_mut());

    let animated = &runner.state().objects[&hive];
    assert!(animated.has_keyword(&Keyword::Menace));
    assert_eq!(animated.color, vec![ManaColor::Black]);
    assert_eq!((animated.power, animated.toughness), (Some(3), Some(3)));
    assert!(animated.card_types.core_types.contains(&CoreType::Creature));
    assert!(animated.card_types.core_types.contains(&CoreType::Land));
    assert!(animated
        .card_types
        .subtypes
        .iter()
        .any(|subtype| subtype == "Beholder"));
    assert!(
        animated
            .trigger_definitions
            .iter_unchecked()
            .any(|trigger| {
                trigger
                    .definition
                    .description
                    .as_deref()
                    .is_some_and(|text| text.contains("attacks"))
            }),
        "the quoted attack trigger must be granted"
    );

    let other = &runner.state().objects[&other];
    assert!(!other.has_keyword(&Keyword::Menace));
    assert_eq!(other.card_types.core_types, vec![CoreType::Land]);
}

#[test]
fn malformed_keyword_tail_declines_the_whole_animation() {
    let good = r#"{3}{B}: Until end of turn, this land becomes a 3/3 black Beholder creature with menace and "Whenever this creature attacks, exile target card from defending player's graveyard." It's still a land."#;
    let bad_clauses = [
        r#"{3}{B}: Until end of turn, this land becomes a 3/3 black Beholder creature with menace and gibberish and "Whenever this creature attacks, exile target card from defending player's graveyard." It's still a land."#,
        r#"{3}{B}: Until end of turn, this land becomes a 3/3 black Beholder creature with vanishing 3 if that creature doesn't have vanishing and "Whenever this creature attacks, exile target card from defending player's graveyard." It's still a land."#,
    ];
    let parse =
        |text| parse_oracle_text(text, "Animation Test Land", &[], &["Land".to_string()], &[]);
    let good = parse(good);
    assert_eq!(
        good.abilities.len(),
        1,
        "positive route reach guard: {good:?}"
    );
    assert!(
        !matches!(
            good.abilities[0].effect.as_ref(),
            Effect::Unimplemented { .. }
        ),
        "valid animation must parse"
    );
    for clause in bad_clauses {
        let bad = parse(clause);
        assert_eq!(
            bad.abilities.len(),
            1,
            "invalid keyword must stay visible: {bad:?}"
        );
        assert!(
            matches!(
                bad.abilities[0].effect.as_ref(),
                Effect::Unimplemented { .. }
            ),
            "invalid keyword must not leave a type-only animation: {bad:?}"
        );
    }
}
