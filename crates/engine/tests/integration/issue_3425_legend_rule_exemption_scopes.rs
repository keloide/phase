//! Legend-rule scope regressions, including Hall of Echoes's resolved exemption.

use engine::game::sba::{check_state_based_actions, legend_rule_exempt};
use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::ability::{ContinuousModification, Duration, TargetRef};
use engine::types::actions::GameAction;
use engine::types::card_type::{CoreType, Supertype};
use engine::types::counter::CounterType;
use engine::types::events::GameEvent;
use engine::types::game_state::{
    CastPaymentMode, GameState, ShardChoice, StackEntryKind, WaitingFor,
};
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaCost, ManaCostShard, ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::player::PlayerId;
use engine::types::statics::StaticMode;
use engine::types::zones::Zone;

const HALL: &str = "Hall of Echoes";
const HALL_ORACLE: &str = "{T}: Add {C}.\n{5}: This land becomes a copy of target creature you control until end of turn. The \"legend rule\" doesn't apply to permanents you control this turn.";
const ISAMARU: &str = "Isamaru, Hound of Konda";
const MOX: &str = "Mox Amber";
const MOX_ORACLE: &str =
    "{T}: Add one mana of any color among legendary creatures and planeswalkers you control.";
const ACT_OF_AGGRESSION: &str = "({R/P} can be paid with either {R} or 2 life.)\nGain control of target creature an opponent controls until end of turn. Untap that creature. It gains haste until end of turn.";
const BEAST_WITHIN: &str =
    "Destroy target permanent. Its controller creates a 3/3 green Beast creature token.";
const HALL_COPY_ABILITY: usize = 1;

fn add_funded_hall(scenario: &mut GameScenario) -> ObjectId {
    scenario.with_mana_pool(
        P0,
        vec![ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]); 5],
    );
    scenario.add_land_from_oracle(P0, HALL, HALL_ORACLE).id()
}

fn add_isamaru(scenario: &mut GameScenario, player: PlayerId) -> ObjectId {
    scenario
        .add_creature(player, ISAMARU, 2, 2)
        .as_legendary()
        .with_subtypes(vec!["Dog"])
        .with_mana_cost(ManaCost::Cost {
            generic: 0,
            shards: vec![ManaCostShard::White],
        })
        .id()
}

fn add_mox_pair(scenario: &mut GameScenario) -> (ObjectId, ObjectId) {
    let incumbent = scenario
        .add_artifact_from_oracle(P0, MOX, MOX_ORACLE)
        .as_legendary()
        .with_mana_cost(ManaCost::generic(0))
        .id();
    let incoming = scenario
        .add_artifact_to_hand_from_oracle(P0, MOX, MOX_ORACLE)
        .as_legendary()
        .with_mana_cost(ManaCost::generic(0))
        .id();
    (incumbent, incoming)
}

fn assert_isamaru_on_battlefield(state: &GameState, id: ObjectId) {
    assert!(state.battlefield.contains(&id));
    let object = &state.objects[&id];
    assert_eq!(object.name, ISAMARU);
    assert_eq!(object.card_types.core_types, [CoreType::Creature]);
    assert!(object.card_types.supertypes.contains(&Supertype::Legendary));
}

fn choose_legend(
    runner: &mut GameRunner,
    controller: PlayerId,
    name: &str,
    keep: ObjectId,
    other: ObjectId,
) {
    // CR 704.5j: the controller chooses one member of the same-name pair.
    let WaitingFor::ChooseLegend {
        player,
        legend_name,
        candidates,
    } = &runner.state().waiting_for
    else {
        panic!(
            "expected legend choice, got {:?}",
            runner.state().waiting_for
        );
    };
    assert_eq!(*player, controller);
    assert_eq!(legend_name, name);
    assert_eq!(candidates.len(), 2);
    assert!(candidates.contains(&keep) && candidates.contains(&other));
    runner
        .act(GameAction::ChooseLegend { keep })
        .expect("choose a legend to keep");
}

#[test]
fn duplicate_legendaries_without_exemption_prompt_choose_legend() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);

    scenario
        .add_creature(P0, "Thalia", 2, 1)
        .as_legendary()
        .id();
    scenario
        .add_creature(P0, "Thalia", 2, 1)
        .as_legendary()
        .id();

    let mut runner = scenario.build();
    let mut events = Vec::<GameEvent>::new();
    check_state_based_actions(runner.state_mut(), &mut events);

    assert!(
        matches!(runner.state().waiting_for, WaitingFor::ChooseLegend { .. }),
        "duplicate legendaries must trigger the legend-rule SBA choice"
    );
}

// Scoped exemption runtime coverage lives in `game/sba.rs` unit tests
// (`sba_legend_rule_suppressed_for_bare_tokens_scope`,
// `sba_legend_rule_suppressed_for_commanders_scope`).

#[test]
fn hall_of_echoes_exempts_live_permanents_for_its_activator() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let hall = add_funded_hall(&mut scenario);
    let donor = add_isamaru(&mut scenario, P0);
    let opposing_isamaru = add_isamaru(&mut scenario, P1);
    let (incumbent_mox, incoming_mox) = add_mox_pair(&mut scenario);
    scenario.with_counter(donor, CounterType::Plus1Plus1, 1);
    scenario.with_mana_pool(
        P1,
        vec![ManaUnit::new(ManaType::Red, ObjectId(0), false, vec![]); 5],
    );
    let takeover = scenario
        .add_spell_to_hand_from_oracle(P1, "Act of Aggression", true, ACT_OF_AGGRESSION)
        .with_mana_cost(ManaCost::Cost {
            generic: 3,
            shards: vec![ManaCostShard::PhyrexianRed; 2],
        })
        .id();
    let mut runner = scenario.build();
    runner.state_mut().objects.get_mut(&donor).unwrap().tapped = true;

    // CR 608.2c + CR 117.5 + CR 704.5j: both instructions finish before the
    // SBA check, so the copy and the original coexist at priority.
    let activation = runner
        .activate(hall, HALL_COPY_ABILITY)
        .target_object(donor)
        .resolve();
    let state = activation.state();
    for id in [hall, donor] {
        assert_isamaru_on_battlefield(state, id);
        assert_eq!(state.objects[&id].controller, P0);
        assert!(legend_rule_exempt(state, id));
    }
    assert!(state.stack.is_empty());
    assert_eq!(state.waiting_for, WaitingFor::Priority { player: P0 });

    // CR 707.2: Hall takes Isamaru's printed power and toughness and keeps its
    // own status and counters.
    let copy = &state.objects[&hall];
    let original = &state.objects[&donor];
    assert_eq!(copy.card_types, original.card_types);
    assert_eq!((copy.power, copy.toughness), (Some(2), Some(2)));
    assert!(original.tapped);
    assert_eq!(original.counters.get(&CounterType::Plus1Plus1), Some(&1));
    assert!(!copy.tapped);
    assert!(copy.counters.is_empty());

    // CR 611.2c + CR 613.11: the live rule reaches a legendary artifact that
    // enters after resolution, through the SBA query independently of abilities.
    let cast = runner.cast(incoming_mox).resolve();
    cast.assert_zone(&[incumbent_mox, incoming_mox], Zone::Battlefield);
    let state = cast.state();
    assert!(state.stack.is_empty());
    assert_eq!(state.waiting_for, WaitingFor::Priority { player: P0 });
    for id in [incumbent_mox, incoming_mox] {
        let artifact = &state.objects[&id];
        assert_eq!(artifact.name, MOX);
        assert_eq!(artifact.controller, P0);
        assert_eq!(artifact.card_types.core_types, [CoreType::Artifact]);
        assert!(artifact
            .card_types
            .supertypes
            .contains(&Supertype::Legendary));
        assert!(legend_rule_exempt(state, id));
        assert!(artifact
            .static_definitions
            .iter_all()
            .all(|definition| definition.mode != StaticMode::LegendRuleDoesntApply));
    }

    runner.act(GameAction::PassPriority).expect("pass to P1");
    assert_eq!(
        runner.state().waiting_for,
        WaitingFor::Priority { player: P1 }
    );
    let card_id = runner.state().objects[&takeover].card_id;
    runner
        .act(GameAction::CastSpell {
            object_id: takeover,
            card_id,
            targets: vec![],
            payment_mode: CastPaymentMode::Auto,
        })
        .expect("cast Act of Aggression");
    runner
        .act(GameAction::ChooseTarget {
            target: Some(TargetRef::Object(hall)),
        })
        .expect("choose the copied Hall");
    // CR 107.4f: pay both Phyrexian symbols with red mana.
    assert!(matches!(
        &runner.state().waiting_for,
        WaitingFor::PhyrexianPayment { player: P1, spell_object, shards, .. }
            if *spell_object == takeover && shards.len() == 2
    ));
    runner
        .act(GameAction::SubmitPhyrexianChoices {
            choices: vec![ShardChoice::PayMana; 2],
        })
        .expect("pay for Act of Aggression");
    assert_eq!(
        runner
            .state()
            .stack
            .back()
            .expect("Act of Aggression on stack")
            .id,
        takeover
    );
    runner.resolve_top();

    // CR 109.5 + CR 611.2c: Act of Aggression moves Hall out of P0's affected
    // population. The rule still reads "you" as the Hall ability's activator.
    assert_eq!(runner.state().objects[&takeover].zone, Zone::Graveyard);
    assert_eq!(runner.state().objects[&hall].controller, P1);
    choose_legend(&mut runner, P1, ISAMARU, opposing_isamaru, hall);

    // CR 704.5j: the losing permanent goes to its owner's graveyard. The rule
    // effect's stated duration (CR 611.2a) continues after Hall leaves.
    let state = runner.state();
    assert!(state.players[0].graveyard.contains(&hall));
    assert_eq!(state.objects[&hall].zone, Zone::Graveyard);
    assert_isamaru_on_battlefield(state, opposing_isamaru);
    assert!(state.stack.is_empty());
    assert_eq!(state.waiting_for, WaitingFor::Priority { player: P0 });
    for id in [donor, incumbent_mox, incoming_mox] {
        assert!(state.battlefield.contains(&id));
        assert!(legend_rule_exempt(state, id));
    }
}

#[test]
fn hall_of_echoes_copy_and_exemption_expire_together_in_cleanup() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PostCombatMain);
    let hall = add_funded_hall(&mut scenario);
    let donor = add_isamaru(&mut scenario, P0);
    let mut runner = scenario.build();

    runner
        .activate(hall, HALL_COPY_ABILITY)
        .target_object(donor)
        .resolve();
    runner.advance_to_end_step();
    let state = runner.state();
    assert_eq!(state.phase, Phase::End);
    assert_eq!(state.waiting_for, WaitingFor::Priority { player: P0 });
    assert!(state.stack.is_empty());
    let previous_turn = state.turn_number;

    // CR 514.2: both continuous effects are active through end-step priority.
    for id in [hall, donor] {
        assert_isamaru_on_battlefield(state, id);
        assert!(legend_rule_exempt(state, id));
    }
    let effects: Vec<_> = state
        .transient_continuous_effects
        .iter()
        .filter(|effect| effect.source_id == hall)
        .collect();
    assert_eq!(effects.len(), 2, "Hall's copy and exemption");
    assert!(effects
        .iter()
        .all(|effect| effect.duration == Duration::UntilEndOfTurn));
    assert!(effects
        .iter()
        .any(|effect| effect.modifications.iter().any(|modification| {
            matches!(modification, ContinuousModification::CopyValues { .. })
        })));
    assert!(effects
        .iter()
        .any(|effect| effect.modifications.iter().any(|modification| {
            matches!(
                modification,
                ContinuousModification::AddStaticMode {
                    mode: StaticMode::LegendRuleDoesntApply
                }
            )
        })));

    let first_pass = runner
        .act(GameAction::PassPriority)
        .expect("P0 passes in the end step");
    assert_eq!(first_pass.waiting_for, WaitingFor::Priority { player: P1 });
    let second_pass = runner
        .act(GameAction::PassPriority)
        .expect("P1 passes into cleanup");
    assert!(first_pass
        .events
        .iter()
        .chain(&second_pass.events)
        .any(|event| {
            matches!(
                event,
                GameEvent::PhaseChanged {
                    phase: Phase::Cleanup
                }
            )
        }));

    // CR 514.2 + CR 704.5j: when the exemption ends, Hall has already returned
    // to its land characteristics and the two permanents remain on the battlefield.
    let state = runner.state();
    assert_eq!(state.turn_number, previous_turn + 1);
    assert_eq!(state.phase, Phase::Upkeep);
    assert_eq!(state.waiting_for, WaitingFor::Priority { player: P1 });
    assert!(state.stack.is_empty());
    assert_isamaru_on_battlefield(state, donor);
    assert!(state.battlefield.contains(&hall));
    assert_eq!(state.objects[&hall].name, HALL);
    assert_eq!(state.objects[&hall].card_types.core_types, [CoreType::Land]);
    assert!(!legend_rule_exempt(state, hall));
    assert!(!legend_rule_exempt(state, donor));
    assert!(state
        .transient_continuous_effects
        .iter()
        .all(|effect| effect.source_id != hall));
}

#[test]
fn hall_of_echoes_losing_its_only_target_prevents_both_instructions() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let hall = add_funded_hall(&mut scenario);
    let donor = add_isamaru(&mut scenario, P0);
    let alternate_donor = scenario.add_creature(P0, "Grizzly Bears", 2, 2).id();
    let (incumbent_mox, incoming_mox) = add_mox_pair(&mut scenario);
    scenario.with_mana_pool(
        P1,
        vec![ManaUnit::new(ManaType::Green, ObjectId(0), false, vec![]); 3],
    );
    let removal = scenario
        .add_spell_to_hand_from_oracle(P1, "Beast Within", true, BEAST_WITHIN)
        .with_mana_cost(ManaCost::Cost {
            generic: 2,
            shards: vec![ManaCostShard::Green],
        })
        .id();
    let mut runner = scenario.build();

    // CR 115.1c: the copy donor is the single mandatory target of the activation.
    let announced = runner
        .act(GameAction::ActivateAbility {
            source_id: hall,
            ability_index: HALL_COPY_ABILITY,
            payment_mode: CastPaymentMode::Auto,
        })
        .expect("announce Hall's copy ability");
    let WaitingFor::TargetSelection {
        player: P0,
        target_slots,
        ..
    } = announced.waiting_for
    else {
        panic!(
            "expected Hall's target selection, got {:?}",
            announced.waiting_for
        );
    };
    assert_eq!(target_slots.len(), 1);
    let legal_targets = &target_slots[0].legal_targets;
    assert_eq!(legal_targets.len(), 2);
    assert!(legal_targets.contains(&TargetRef::Object(donor)));
    assert!(legal_targets.contains(&TargetRef::Object(alternate_donor)));
    assert!(!target_slots[0].optional);
    runner
        .act(GameAction::ChooseTarget {
            target: Some(TargetRef::Object(donor)),
        })
        .expect("choose Isamaru and pay for Hall's activation");
    let state = runner.state();
    assert_eq!(state.waiting_for, WaitingFor::Priority { player: P0 });
    assert_eq!(state.players[0].mana_pool.total(), 0);
    assert_eq!(state.stack.len(), 1);
    let StackEntryKind::ActivatedAbility { source_id, ability } = &state.stack[0].kind else {
        panic!("expected Hall's activation on the stack");
    };
    assert_eq!(*source_id, hall);
    assert_eq!(ability.targets, [TargetRef::Object(donor)]);

    runner
        .act(GameAction::PassPriority)
        .expect("pass to P1 with Hall's ability pending");
    assert_eq!(
        runner.state().waiting_for,
        WaitingFor::Priority { player: P1 }
    );
    runner.cast(removal).target_object(donor).commit();
    assert_eq!(runner.state().stack.len(), 2);
    runner.resolve_top();
    assert_eq!(runner.state().objects[&donor].zone, Zone::Graveyard);
    assert_eq!(runner.state().stack.len(), 1);

    // CR 608.2b: the invalid sole target prevents the entire ability from
    // resolving. A newly cast duplicate must still receive a legend choice.
    runner.advance_until_stack_empty();
    let state = runner.state();
    assert!(state.stack.is_empty());
    assert_eq!(state.waiting_for, WaitingFor::Priority { player: P0 });
    assert_eq!(state.objects[&hall].zone, Zone::Battlefield);
    assert_eq!(state.objects[&hall].name, HALL);
    assert_eq!(state.objects[&hall].card_types.core_types, [CoreType::Land]);

    let cast = runner.cast(incoming_mox).resolve();
    cast.assert_zone(&[incumbent_mox, incoming_mox], Zone::Battlefield);
    choose_legend(&mut runner, P0, MOX, incumbent_mox, incoming_mox);
    let state = runner.state();
    assert_eq!(state.objects[&incumbent_mox].zone, Zone::Battlefield);
    assert_eq!(state.objects[&incoming_mox].zone, Zone::Graveyard);
    assert!(state.stack.is_empty());
    assert_eq!(state.waiting_for, WaitingFor::Priority { player: P0 });
}
