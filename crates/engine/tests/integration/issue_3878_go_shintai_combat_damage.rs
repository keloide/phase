//! Regression for issue #3878: Go-Shintai of Life's Origin must be able to
//! take combat damage as an enchantment creature.
//!
//! https://github.com/phase-rs/phase/issues/3878

use engine::game::combat::AttackTarget;
use engine::game::scenario::{GameScenario, P0, P1};
use engine::types::ability::TargetRef;
use engine::types::actions::GameAction;
use engine::types::events::GameEvent;
use engine::types::game_state::WaitingFor;
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const GO_SHINTAI_ORACLE: &str = "{W}{U}{B}{R}{G}, {T}: Return target enchantment card from your graveyard to the battlefield.\n\
    Whenever Go-Shintai of Life's Origin or another nontoken Shrine you control enters, create a 1/1 colorless Shrine enchantment creature token.";

#[test]
fn go_shintai_takes_combat_damage() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);

    let attacker = scenario.add_creature(P0, "Attacker", 3, 3).id();
    let shintai = scenario
        .add_creature_from_oracle(P1, "Go-Shintai of Life's Origin", 1, 1, GO_SHINTAI_ORACLE)
        .id();

    let mut runner = scenario.build();
    runner.advance_to_combat();
    let mut events = runner
        .declare_attackers(&[(attacker, AttackTarget::Player(P1))])
        .expect("attack")
        .events;
    for _ in 0..10 {
        if matches!(
            runner.state().waiting_for,
            WaitingFor::DeclareBlockers { .. }
        ) {
            break;
        }
        events.extend(
            runner
                .act(GameAction::PassPriority)
                .expect("reach blockers")
                .events,
        );
    }
    events.extend(
        runner
            .declare_blockers(&[(shintai, attacker)])
            .expect("block with Go-Shintai")
            .events,
    );
    let outcome = runner.combat_damage();
    events.extend_from_slice(outcome.events());
    // CR 510.2: both combatants deal damage before lethal state-based actions.
    for (recipient, dealer, amount) in [(shintai, attacker, 3), (attacker, shintai, 1)] {
        let damage: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                GameEvent::DamageDealt {
                    source_id,
                    target: TargetRef::Object(id),
                    amount,
                    is_combat,
                    ..
                } if *id == recipient => Some((*source_id, *amount, *is_combat)),
                _ => None,
            })
            .collect();
        assert_eq!(damage, vec![(dealer, amount, true)],
            "Go-Shintai must take combat damage from the blocked attacker and deal its own power back");
    }
    // CR 400.7: Go-Shintai's new graveyard object has cleared marked damage.
    assert_eq!(runner.state().objects[&shintai].zone, Zone::Graveyard);
    assert_eq!(runner.state().objects[&shintai].damage_marked, 0);
    assert_eq!(runner.state().objects[&attacker].zone, Zone::Battlefield);
    assert_eq!(runner.state().objects[&attacker].damage_marked, 1);
}
