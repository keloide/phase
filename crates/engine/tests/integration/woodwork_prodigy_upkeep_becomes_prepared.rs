//! Woodwork Prodigy (FRA): upkeep trigger prepares itself when unprepared.
//!
//! Verbatim front-face Oracle:
//!   "At the beginning of your upkeep, if this creature isn't prepared, it
//!    becomes prepared. (While it's prepared, you may cast a copy of its
//!    spell. Doing so unprepares it.)"
//!
//! Twin design driving one shared upkeep event: twin-a starts unprepared (its
//! trigger must fire and prepare it) while twin-b is prepared up front
//! through the real `prepare_object` path (its trigger must be suppressed by
//! the same event). The pair discriminates the
//! `Not(SourceMatchesFilter{creature + Prepared})` gate exactly — an
//! unconditional trigger would fire for both twins, a flipped polarity for
//! the wrong twin. CR 603.4 + CR 722.3a.
//!
//! Fixture-free like `fra_bloodline_recollector.rs`: both twins are built
//! with `from_oracle_text`, and the Prepare back face the Biblioplex gate
//! (`has_prepare_face`) requires is staged synthetically the same way.

use engine::game::game_object::BackFaceData;
use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::types::actions::GameAction;
use engine::types::card::LayoutKind;
use engine::types::events::GameEvent;
use engine::types::identifiers::ObjectId;
use engine::types::phase::Phase;

const WOODWORK_ORACLE: &str = "At the beginning of your upkeep, if this creature isn't prepared, it becomes prepared. (While it's prepared, you may cast a copy of its spell. Doing so unprepares it.)";

fn stack_entries_for(runner: &GameRunner, source: ObjectId) -> usize {
    runner
        .state()
        .stack
        .iter()
        .filter(|entry| entry.source_id == source)
        .count()
}

fn is_prepared(runner: &GameRunner, object: ObjectId) -> bool {
    runner
        .state()
        .objects
        .get(&object)
        .and_then(|o| o.prepared.as_ref())
        .is_some()
}

fn setup_twins() -> (GameRunner, ObjectId, ObjectId) {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::Untap);
    let unprepared = scenario
        .add_creature(P0, "Woodwork Prodigy", 2, 2)
        .from_oracle_text(WOODWORK_ORACLE)
        .id();
    let prepared = scenario
        .add_creature(P0, "Woodwork Prodigy", 2, 2)
        .from_oracle_text(WOODWORK_ORACLE)
        .id();
    let mut runner = scenario.build();

    // The Biblioplex gate (`has_prepare_face`, CR 722.3a reminder) no-ops
    // `prepare_object` on creatures without a Prepare back face, so stage one
    // on both twins exactly like `fra_bloodline_recollector.rs`.
    for twin in [unprepared, prepared] {
        runner.state_mut().objects.get_mut(&twin).unwrap().back_face = Some(BackFaceData {
            layout_kind: Some(LayoutKind::Prepare),
            ..BackFaceData::default()
        });
    }

    // Discriminating negative: the parsed Woodwork ability tree must contain
    // no Effect::Unimplemented. Before the prepared-gate fix the "if this
    // creature isn't prepared" clause was unparsed residue, so the trigger
    // either never fired or never gated.
    let parsed_json =
        serde_json::to_string(&runner.state().objects[&unprepared].trigger_definitions)
            .expect("serialize triggers");
    assert!(
        !parsed_json.contains("\"Unimplemented\""),
        "Woodwork Prodigy's parsed ability tree must contain no Effect::Unimplemented"
    );

    // Stage the suppression twin through the real prepare path (not a bare
    // field set), so its designation matches what resolution produces. Fail
    // loudly if staging itself did not take.
    {
        let mut events = Vec::new();
        engine::game::effects::prepare::prepare_object(runner.state_mut(), prepared, &mut events);
    }
    assert!(
        is_prepared(&runner, prepared),
        "staging must leave the suppression twin prepared"
    );
    assert!(
        !is_prepared(&runner, unprepared),
        "the firing twin must start unprepared"
    );
    (runner, unprepared, prepared)
}

/// CR 603.4 + CR 722.3a: at the shared upkeep event the unprepared twin's
/// trigger fires and prepares it (emitting `BecamePrepared`), while the
/// already-prepared twin's trigger is suppressed by the intervening-if.
#[test]
fn woodwork_prodigy_upkeep_prepares_unprepared_twin_only() {
    let (mut runner, unprepared, prepared) = setup_twins();

    // One shared upkeep event: exactly one Woodwork trigger (the unprepared
    // twin's) reaches the stack.
    runner.advance_to_upkeep();
    assert_eq!(
        stack_entries_for(&runner, unprepared),
        1,
        "the unprepared twin's upkeep trigger must be on the stack"
    );
    assert_eq!(
        stack_entries_for(&runner, prepared),
        0,
        "the prepared twin's trigger must be suppressed by the intervening-if"
    );

    // Resolve through priority, collecting events to confirm the
    // BecamePrepared emission for the firing twin only.
    let mut became_prepared = false;
    for _ in 0..8 {
        if runner.state().stack.is_empty() {
            break;
        }
        let result = runner
            .act(GameAction::PassPriority)
            .expect("passing priority resolves the upkeep trigger");
        became_prepared |= result.events.iter().any(
            |e| matches!(e, GameEvent::BecamePrepared { object_id } if *object_id == unprepared),
        );
    }
    assert!(
        runner.state().stack.is_empty(),
        "the upkeep trigger must fully resolve"
    );
    assert!(
        became_prepared,
        "a GameEvent::BecamePrepared for the unprepared twin must have been emitted"
    );
    assert!(
        is_prepared(&runner, unprepared),
        "the firing twin must be prepared after resolution"
    );
    assert!(
        is_prepared(&runner, prepared),
        "the suppressed twin must still be prepared — suppression read live state"
    );
}

/// CR 603.4 requires a live resolution-time recheck of the intervening-if:
/// if the firing twin becomes prepared after triggering but before
/// resolution, the ability must do nothing (no second `BecamePrepared`).
#[test]
fn woodwork_prodigy_intervening_if_is_rechecked_on_resolution() {
    let (mut runner, unprepared, prepared) = setup_twins();
    runner.advance_to_upkeep();
    assert_eq!(
        stack_entries_for(&runner, unprepared),
        1,
        "the unprepared twin's upkeep trigger must be on the stack"
    );

    // Prepare the firing twin before its trigger resolves.
    {
        let mut events = Vec::new();
        engine::game::effects::prepare::prepare_object(runner.state_mut(), unprepared, &mut events);
    }
    assert!(is_prepared(&runner, unprepared));

    let mut became_prepared_on_resolution = false;
    for _ in 0..8 {
        if runner.state().stack.is_empty() {
            break;
        }
        let result = runner
            .act(GameAction::PassPriority)
            .expect("passing priority resolves the upkeep trigger");
        became_prepared_on_resolution |= result.events.iter().any(
            |e| matches!(e, GameEvent::BecamePrepared { object_id } if *object_id == unprepared),
        );
    }
    assert!(
        runner.state().stack.is_empty(),
        "the upkeep trigger must fully resolve"
    );
    assert!(
        !became_prepared_on_resolution,
        "no BecamePrepared may be emitted on resolution once the twin is already prepared"
    );
    assert!(
        is_prepared(&runner, prepared),
        "the suppressed twin must still be prepared"
    );
}
