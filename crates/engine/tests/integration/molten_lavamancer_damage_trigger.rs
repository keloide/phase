//! Source-led damage triggers with a turn qualifier and an aggregate opponent
//! recipient. The damage source is an activated creature, so each activation
//! deals damage to two opponents in one resolution in a three-player game.

use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::ability::Effect;
use engine::types::phase::Phase;
use engine::types::player::PlayerId;

const P2: PlayerId = PlayerId(2);

const MOLTEN_LAVAMANCER: &str = "Prowess\nWhenever a source you control deals noncombat damage to one or more of your opponents during your turn, you create a 1/1 red Elemental creature token. This ability triggers only once each turn.";
const UNCAPPED_OBSERVER: &str = "Whenever a source you control deals noncombat damage to one or more of your opponents during your turn, you create a 1/1 red Elemental creature token.";
const UNKNOWN_TAIL_OBSERVER: &str = "Whenever a source you control deals noncombat damage to one or more of your opponents during an impossible phase, you create a 1/1 red Elemental creature token.";
const DAMAGE_SOURCE: &str = "{0}: This creature deals 1 damage to each opponent.";

fn fixture(
    observer_name: &str,
    observer_oracle: &str,
) -> (GameRunner, engine::types::identifiers::ObjectId) {
    let mut scenario = GameScenario::new_n_player(3, 42);
    scenario.at_phase(Phase::PreCombatMain);
    scenario.add_creature_from_oracle(P0, observer_name, 2, 2, observer_oracle);
    let source = scenario
        .add_creature_from_oracle(P0, "Damage Source", 2, 2, DAMAGE_SOURCE)
        .id();
    let runner = scenario.build();
    let abilities = &runner.state().objects[&source].abilities;
    assert_eq!(
        abilities.len(),
        1,
        "the damage source must have one activated ability"
    );
    assert!(
        !matches!(abilities[0].effect.as_ref(), Effect::Unimplemented { .. }),
        "the damage action must parse before testing its observers"
    );
    (runner, source)
}

fn life(runner: &GameRunner, player: engine::types::player::PlayerId) -> i32 {
    runner
        .state()
        .players
        .iter()
        .find(|candidate| candidate.id == player)
        .expect("player exists")
        .life
}

fn elementals(runner: &GameRunner) -> usize {
    runner
        .state()
        .battlefield
        .iter()
        .filter(|id| {
            let object = &runner.state().objects[id];
            object.is_token && object.controller == P0 && object.name == "Elemental"
        })
        .count()
}

/// CR 603.2c + CR 120.4b: the simultaneous damage to P1 and P2 is one
/// qualifying event for the aggregate recipient. A second activation is a
/// separate event and must create another token without an explicit cap.
#[test]
fn uncapped_observer_creates_one_elemental_per_two_opponent_damage_event() {
    let (mut runner, source) = fixture("Uncapped Observer", UNCAPPED_OBSERVER);
    let before = (life(&runner, P0), life(&runner, P1), life(&runner, P2));

    runner.activate(source, 0).resolve();
    assert_eq!(
        life(&runner, P0),
        before.0,
        "damage excludes the source controller"
    );
    assert_eq!(life(&runner, P1), before.1 - 1, "P1 must take damage");
    assert_eq!(life(&runner, P2), before.2 - 1, "P2 must take damage");
    assert_eq!(elementals(&runner), 1, "one event must make one token");

    runner.activate(source, 0).resolve();
    assert_eq!(life(&runner, P1), before.1 - 2);
    assert_eq!(life(&runner, P2), before.2 - 2);
    assert_eq!(elementals(&runner), 2, "two events must make two tokens");
}

/// CR 603.2 + CR 102.1: Molten Lavamancer observes its controller's turn and
/// its printed once-per-turn sentence caps a second qualifying event.
#[test]
fn molten_lavamancer_caps_own_turn_damage_and_ignores_other_turns() {
    let (mut runner, source) = fixture("Molten Lavamancer", MOLTEN_LAVAMANCER);
    let before = (life(&runner, P1), life(&runner, P2));

    runner.activate(source, 0).resolve();
    assert_eq!(life(&runner, P1), before.0 - 1);
    assert_eq!(life(&runner, P2), before.1 - 1);
    assert_eq!(elementals(&runner), 1);

    runner.activate(source, 0).resolve();
    assert_eq!(life(&runner, P1), before.0 - 2);
    assert_eq!(life(&runner, P2), before.1 - 2);
    assert_eq!(
        elementals(&runner),
        1,
        "printed cap prevents a second token"
    );

    runner.state_mut().active_player = P1;
    runner.activate(source, 0).resolve();
    assert_eq!(life(&runner, P1), before.0 - 3);
    assert_eq!(life(&runner, P2), before.1 - 3);
    assert_eq!(
        elementals(&runner),
        1,
        "opponent-turn damage must not trigger"
    );
}

/// CR 603.2: an unknown event qualifier cannot turn into a broad damage
/// trigger. The same source still damages both opponents, proving the event
/// reached the trigger pipeline.
#[test]
fn unknown_damage_timing_tail_does_not_fire_a_broad_trigger() {
    let (mut runner, source) = fixture("Unknown Tail Observer", UNKNOWN_TAIL_OBSERVER);
    let before = (life(&runner, P1), life(&runner, P2));

    runner.activate(source, 0).resolve();
    assert_eq!(life(&runner, P1), before.0 - 1);
    assert_eq!(life(&runner, P2), before.1 - 1);
    assert_eq!(elementals(&runner), 0, "unparsed timing must fail closed");
}
