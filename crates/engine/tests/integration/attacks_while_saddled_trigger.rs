//! "Attacks while saddled" trigger-gate coverage — Alacrian Jaguar and its
//! 27-card class, driven through the real declare-attackers / trigger pipeline.
//!
//! CR 702.171a: Saddle is an activated ability (sorcery speed).
//! CR 702.171b: the saddled designation lasts until end of turn or the
//!   permanent leaves the battlefield; it is a marker spells/abilities identify.
//! CR 702.171c: the creatures that saddled the permanent.
//! CR 508.1: attackers are declared as a turn-based action.
//! CR 603.4: the state gate is checked when the ability triggers AND rechecked
//!   as it resolves — if false at resolution the ability is removed.
//! Official ruling (2025-02-07): "attacks while saddled" fires only if the
//! creature is saddled when it's declared as an attacker.
//!
//! The gate lowers to `TriggerCondition::SourceMatchesFilter { Typed([IsSaddled]) }`.
//! A source destroyed in response after the trigger is on the stack resolves via
//! last known information (CR 608.2h + CR 113.7a): `LKISnapshot::is_saddled`
//! carries the exit-time designation so the recheck still passes.

use engine::game::layers::evaluate_layers;
use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::ability::{TargetRef, TriggerCondition};
use engine::types::actions::GameAction;
use engine::types::counter::CounterType;
use engine::types::game_state::{StackEntryKind, WaitingFor};
use engine::types::identifiers::ObjectId;
use engine::types::mana::ManaCost;
use engine::types::phase::Phase;
use engine::types::player::PlayerId;

use super::rules::AttackTarget;

// Verbatim Oracle text (Scryfall / card-data.json), not paraphrases — a
// paraphrase could take a different parser branch and mask the real behavior.
const ALACRIAN_JAGUAR: &str = "Vigilance\n\
Whenever this creature attacks while saddled, it gets +2/+2 until end of turn.\n\
Saddle 1 (Tap any number of other creatures you control with total power 1 or more: This Mount becomes saddled until end of turn. Saddle only as a sorcery.)";

const ORNERY_TUMBLEWAGG: &str = "At the beginning of combat on your turn, put a +1/+1 counter on target creature.\n\
Whenever this creature attacks while saddled, double the number of +1/+1 counters on target creature.\n\
Saddle 2 (Tap any number of other creatures you control with total power 2 or more: This Mount becomes saddled until end of turn. Saddle only as a sorcery.)";

const REMOVAL_INSTANT: &str = "Destroy target creature.";

fn effective_pt(runner: &mut GameRunner, id: ObjectId) -> (i32, i32) {
    runner.state_mut().layers_dirty.mark_full();
    evaluate_layers(runner.state_mut());
    let obj = &runner.state().objects[&id];
    (
        obj.power.expect("creature has power"),
        obj.toughness.expect("creature has toughness"),
    )
}

fn p1p1(runner: &GameRunner, id: ObjectId) -> u32 {
    runner.state().objects[&id]
        .counters
        .get(&CounterType::Plus1Plus1)
        .copied()
        .unwrap_or(0)
}

/// Saddle `mount` with `riders` through the real `SaddleMount` announce+pay
/// pipeline (CR 702.171a-c), then resolve the Saddle stack entry.
fn saddle_mount(runner: &mut GameRunner, mount: ObjectId, riders: Vec<ObjectId>) {
    runner
        .act(GameAction::SaddleMount {
            mount_id: mount,
            creature_ids: vec![],
        })
        .expect("entering SaddleMount should succeed at sorcery speed");
    runner
        .act(GameAction::SaddleMount {
            mount_id: mount,
            creature_ids: riders,
        })
        .expect("announcing the saddle should succeed");
    runner.advance_until_stack_empty();
    assert!(
        runner.state().objects[&mount].is_saddled,
        "mount must be saddled after Saddle resolves"
    );
}

/// Advance from the current main-phase priority to `DeclareAttackers`, handling
/// any at-the-beginning-of-combat trigger by choosing `aux_target` (used by the
/// Ornery Tumblewagg fixture, whose combat trigger targets a creature).
fn advance_to_declare_attackers(
    runner: &mut GameRunner,
    attacker: PlayerId,
    aux_target: Option<ObjectId>,
) {
    runner.state_mut().active_player = attacker;
    runner.state_mut().priority_player = attacker;
    runner.state_mut().waiting_for = WaitingFor::Priority { player: attacker };

    for _ in 0..40 {
        match runner.state().waiting_for.clone() {
            WaitingFor::DeclareAttackers { .. } => return,
            WaitingFor::OrderTriggers { triggers, .. } => {
                let order = (0..triggers.len()).collect();
                runner
                    .act(GameAction::OrderTriggers { order })
                    .expect("ordering combat triggers should succeed");
            }
            WaitingFor::TriggerTargetSelection { .. } => {
                let t = aux_target.expect("unexpected combat-trigger target selection");
                runner
                    .act(GameAction::ChooseTarget {
                        target: Some(TargetRef::Object(t)),
                    })
                    .expect("choosing combat-trigger target should succeed");
            }
            WaitingFor::Priority { .. } => {
                runner
                    .act(GameAction::PassPriority)
                    .expect("priority pass should advance toward declare attackers");
            }
            other => panic!("unexpected waiting_for advancing to declare attackers: {other:?}"),
        }
    }
    panic!("expected DeclareAttackers");
}

/// Handle target selection for the attacks-while-saddled trigger, choosing
/// `target`. Returns once priority reopens with the trigger on the stack.
fn choose_attack_trigger_target(runner: &mut GameRunner, target: ObjectId) {
    for _ in 0..16 {
        match runner.state().waiting_for.clone() {
            WaitingFor::OrderTriggers { triggers, .. } => {
                let order = (0..triggers.len()).collect();
                runner
                    .act(GameAction::OrderTriggers { order })
                    .expect("ordering attack triggers should succeed");
            }
            WaitingFor::TriggerTargetSelection { .. } => {
                runner
                    .act(GameAction::ChooseTarget {
                        target: Some(TargetRef::Object(target)),
                    })
                    .expect("choosing attack-trigger target should succeed");
                return;
            }
            _ => return,
        }
    }
    panic!("expected the attacks-while-saddled trigger to request a target");
}

fn stack_condition_for_source(
    runner: &GameRunner,
    source_id: ObjectId,
) -> Option<TriggerCondition> {
    runner.state().stack.iter().find_map(|entry| {
        if entry.source_id != source_id {
            return None;
        }
        match &entry.kind {
            StackEntryKind::TriggeredAbility { condition, .. } => condition.clone(),
            _ => None,
        }
    })
}

fn add_jaguar(scenario: &mut GameScenario, player: PlayerId, name: &str) -> ObjectId {
    // Synthetic 2/2 base so the +2/+2 lands at a clean 4/4 (the real card is 4/4);
    // the ability text is verbatim so the trigger takes the production branch.
    let mut b = scenario.add_creature(player, name, 2, 2);
    b.from_oracle_text_with_keywords(&["Vigilance", "Saddle"], ALACRIAN_JAGUAR);
    b.id()
}

/// Test 1 — saddled, attacks, the gate holds at trigger AND resolution, +2/+2.
#[test]
fn alacrian_jaguar_saddled_attack_pumps() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let jaguar = add_jaguar(&mut scenario, P0, "Alacrian Jaguar");
    let rider = scenario.add_creature(P0, "Rider", 1, 1).id();
    let mut runner = scenario.build();

    saddle_mount(&mut runner, jaguar, vec![rider]);
    assert_eq!(
        effective_pt(&mut runner, jaguar),
        (2, 2),
        "unpumped base P/T"
    );

    advance_to_declare_attackers(&mut runner, P0, None);
    runner
        .declare_attackers(&[(jaguar, AttackTarget::Player(P1))])
        .expect("saddled Mount should be a legal attacker");
    runner.advance_until_stack_empty();

    assert_eq!(
        effective_pt(&mut runner, jaguar),
        (4, 4),
        "saddled attacker must gain +2/+2 (CR 702.171b gate satisfied)"
    );
}

/// Test 2 — unsaddled: the trigger's condition is present (REVERT-FAILING
/// reach-guard) but false at trigger time, so no pump.
#[test]
fn alacrian_jaguar_unsaddled_attack_no_pump() {
    // Reach-guard: the parsed attacks trigger MUST carry a condition. Without the
    // elided-subject while-gate leaf the gate is dropped and this is `None`,
    // making the "stays 2/2" assertion below vacuous (an unconditional trigger
    // that simply never fired for another reason). This flips if the fix reverts.
    let parsed = engine::parser::oracle::parse_oracle_text(
        ALACRIAN_JAGUAR,
        "Alacrian Jaguar",
        &["Vigilance".to_string(), "Saddle".to_string()],
        &["Creature".to_string()],
        &["Cat".to_string()],
    );
    let attack_trigger = parsed
        .triggers
        .iter()
        .find(|t| t.mode == engine::types::triggers::TriggerMode::Attacks)
        .expect("Alacrian Jaguar has an attacks trigger");
    assert!(
        matches!(
            attack_trigger.condition.as_ref(),
            Some(TriggerCondition::SourceMatchesFilter { .. })
        ),
        "attacks-while-saddled trigger must carry a saddled SourceMatchesFilter gate, got {:?}",
        attack_trigger.condition
    );

    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let jaguar = add_jaguar(&mut scenario, P0, "Alacrian Jaguar");
    let mut runner = scenario.build();

    // No saddle activation — the Mount is NOT saddled.
    assert!(!runner.state().objects[&jaguar].is_saddled);

    advance_to_declare_attackers(&mut runner, P0, None);
    runner
        .declare_attackers(&[(jaguar, AttackTarget::Player(P1))])
        .expect("unsaddled Mount is still a legal attacker");
    runner.advance_until_stack_empty();

    assert_eq!(
        effective_pt(&mut runner, jaguar),
        (2, 2),
        "an unsaddled attacker must NOT gain +2/+2 — the gate is false at trigger time"
    );
}

/// Test 3 — two identical Mounts in one DeclareAttackers, only A saddled. The
/// gate is per-source, so only A pumps.
#[test]
fn only_saddled_mount_triggers_in_shared_attack() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let mount_a = add_jaguar(&mut scenario, P0, "Alacrian Jaguar A");
    let mount_b = add_jaguar(&mut scenario, P0, "Alacrian Jaguar B");
    let rider = scenario.add_creature(P0, "Rider", 1, 1).id();
    let mut runner = scenario.build();

    // Saddle only A.
    saddle_mount(&mut runner, mount_a, vec![rider]);
    assert!(!runner.state().objects[&mount_b].is_saddled);

    advance_to_declare_attackers(&mut runner, P0, None);
    runner
        .declare_attackers(&[
            (mount_a, AttackTarget::Player(P1)),
            (mount_b, AttackTarget::Player(P1)),
        ])
        .expect("both Mounts should be legal attackers");
    runner.advance_until_stack_empty();

    assert_eq!(
        effective_pt(&mut runner, mount_a),
        (4, 4),
        "the saddled Mount must gain +2/+2"
    );
    assert_eq!(
        effective_pt(&mut runner, mount_b),
        (2, 2),
        "the unsaddled Mount sharing the attack must NOT gain +2/+2"
    );
}

/// Test 4 — the LKI load-bearing case. A saddled Ornery Tumblewagg attacks, its
/// doubling trigger is placed on the stack targeting creature B, then the Mount
/// is DESTROYED in response through the real cast pipeline. The trigger still
/// resolves because the saddled recheck reads last known information
/// (CR 608.2h + CR 113.7a) via `LKISnapshot::is_saddled`.
#[test]
fn ornery_tumblewagg_dies_in_response_trigger_survives() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);

    let mount = {
        let mut b = scenario.add_creature(P0, "Ornery Tumblewagg", 2, 2);
        b.from_oracle_text_with_keywords(&["Saddle"], ORNERY_TUMBLEWAGG);
        b.id()
    };
    let rider = scenario.add_creature(P0, "Rider", 2, 2).id();
    // Target of the doubling trigger — starts with N=3 +1/+1 counters.
    let target = scenario.add_creature(P0, "Counter Bearer", 1, 1).id();
    scenario.with_counter(target, CounterType::Plus1Plus1, 3);
    // The removal instant that destroys the Mount in response.
    let removal = scenario
        .add_spell_to_hand_from_oracle(P0, "Doom Bolt", true, REMOVAL_INSTANT)
        .with_mana_cost(ManaCost::zero())
        .id();

    let mut runner = scenario.build();

    saddle_mount(&mut runner, mount, vec![rider]);

    // Advance to combat; the beginning-of-combat trigger targets the Mount
    // itself (kept off `target` so N stays deterministic at 3).
    advance_to_declare_attackers(&mut runner, P0, Some(mount));

    let n = p1p1(&runner, target);
    assert_eq!(n, 3, "target must start with N=3 +1/+1 counters");

    runner
        .declare_attackers(&[(mount, AttackTarget::Player(P1))])
        .expect("saddled Mount should be a legal attacker");
    choose_attack_trigger_target(&mut runner, target);

    // On-stack reach-guard: the saddled gate is NOT stripped (unlike event-only
    // attack qualifiers) — it must be present for the CR 603.4 resolution recheck.
    assert!(
        matches!(
            stack_condition_for_source(&runner, mount),
            Some(TriggerCondition::SourceMatchesFilter { .. })
        ),
        "the doubling trigger must carry its saddled gate on the stack, got {:?}",
        stack_condition_for_source(&runner, mount)
    );

    // Destroy the Mount in response — it leaves the battlefield BEFORE the
    // doubling trigger resolves. This is what exercises the LKI thread.
    runner.cast(removal).target_object(mount).resolve();
    assert_eq!(
        runner.state().objects[&mount].zone,
        engine::types::zones::Zone::Graveyard,
        "the Mount must have been destroyed in response"
    );
    runner.advance_until_stack_empty();

    assert_eq!(
        p1p1(&runner, target),
        2 * n,
        "the doubling trigger must still resolve via LKI after its saddled source \
         left the battlefield (CR 608.2h) — counters doubled from {n} to {}",
        2 * n
    );
    // The LKI snapshot recorded the exit-time saddled designation.
    assert_eq!(
        runner.state().lki_cache.get(&mount).map(|l| l.is_saddled),
        Some(true),
        "the destroyed Mount's LKI must retain is_saddled = true"
    );
}
