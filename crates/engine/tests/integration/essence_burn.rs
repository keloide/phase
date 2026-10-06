//! Essence Burn exercises generic color-list grammar and target-bound death replacements.
use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::game::scenario_db::GameScenarioDbExt;
use engine::types::ability::{AbilityDefinition, Effect, TargetFilter, TargetRef};
use engine::types::actions::GameAction;
use engine::types::counter::CounterType;
use engine::types::game_state::{CastPaymentMode, WaitingFor};
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaColor, ManaCost, ManaCostShard, ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::player::PlayerId;
use engine::types::zones::Zone;

const ESSENCE: &str = "Essence Burn deals 5 damage to target black or green creature or planeswalker. If that permanent would die this turn, exile it instead.";
const RITES: &str =
    "As an additional cost to cast this spell, sacrifice a creature.\nDraw two cards.";
const GUST: &str = "Choose target spell or permanent that's red or green. Its owner puts it on their choice of the top or bottom of their library.";

fn cost(generic: u32, shard: ManaCostShard) -> ManaCost {
    ManaCost::Cost {
        generic,
        shards: vec![shard],
    }
}
fn mana(color: ManaType) -> ManaUnit {
    ManaUnit::new(color, ObjectId(0), false, vec![])
}
fn board() -> GameScenario {
    let mut s = GameScenario::new();
    s.at_phase(Phase::PreCombatMain);
    s.with_mana_pool(
        P0,
        (0..12)
            .flat_map(|_| {
                [
                    mana(ManaType::Red),
                    mana(ManaType::Black),
                    mana(ManaType::Blue),
                ]
            })
            .collect(),
    );
    s.with_library_top(P0, &["One", "Two", "Three", "Four", "Five", "Six"]);
    s.with_library_top(P1, &["One", "Two", "Three", "Four"]);
    s
}
fn burn(s: &mut GameScenario) -> ObjectId {
    s.add_spell_to_hand_from_oracle(P0, "Essence Burn", true, ESSENCE)
        .with_mana_cost(cost(1, ManaCostShard::Red))
        .id()
}
fn creature(
    s: &mut GameScenario,
    player: PlayerId,
    name: &str,
    color: ManaColor,
    toughness: i32,
) -> ObjectId {
    s.add_creature(player, name, 2, toughness)
        .with_color(vec![color])
        .id()
}
fn walker(s: &mut GameScenario, name: &str, color: ManaColor, loyalty: u32) -> ObjectId {
    s.add_planeswalker_from_oracle(P1, name, "Test", loyalty, "")
        .with_color(vec![color])
        .id()
}
fn rites(s: &mut GameScenario) -> ObjectId {
    s.add_spell_to_hand_from_oracle(P0, "Village Rites", true, RITES)
        .with_mana_cost(cost(0, ManaCostShard::Black))
        .id()
}
fn announce(r: &mut GameRunner, spell: ObjectId) {
    let card_id = r.state().objects[&spell].card_id;
    r.act(GameAction::CastSpell {
        object_id: spell,
        card_id,
        targets: vec![],
        payment_mode: CastPaymentMode::Auto,
    })
    .expect("production CastSpell reaches target selection");
}
fn legal(r: &GameRunner) -> Vec<TargetRef> {
    let WaitingFor::TargetSelection { target_slots, .. } = &r.state().waiting_for else {
        panic!("expected TargetSelection, got {:?}", r.state().waiting_for);
    };
    assert_eq!(target_slots.len(), 1);
    assert!(
        !target_slots[0].legal_targets.is_empty(),
        "positive target reach guard"
    );
    target_slots[0].legal_targets.clone()
}
fn reject(r: &mut GameRunner, id: ObjectId) {
    let err = r
        .act(GameAction::ChooseTarget {
            target: TargetRef::Object(id),
        })
        .expect_err("illegal target must reject");
    assert!(
        format!("{err:?}").contains("Illegal target selected"),
        "{err:?}"
    );
    assert!(matches!(
        r.state().waiting_for,
        WaitingFor::TargetSelection { .. }
    ));
}
fn gaps(def: &AbilityDefinition) -> bool {
    matches!(def.effect.as_ref(), Effect::Unimplemented { .. })
        || def.sub_ability.as_deref().is_some_and(gaps)
        || def.else_ability.as_deref().is_some_and(gaps)
}

#[test]
fn essence_burn_exiles_lethal_black_creature() {
    let mut s = board();
    let spell = burn(&mut s);
    let victim = creature(&mut s, P1, "Victim", ManaColor::Black, 3);
    let bystander = creature(&mut s, P1, "Bystander", ManaColor::Black, 3);
    let mut r = s.build();
    let out = r.cast(spell).target_object(victim).resolve();
    // CR 608.2c + CR 700.4: the whole instruction installs the rider before lethal SBA.
    assert_eq!(
        out.zone_of(victim),
        Zone::Exile,
        "Essence Burn must exile its lethal black creature target"
    );
    out.assert_zone(&[spell], Zone::Graveyard);
    out.assert_zone(&[bystander], Zone::Battlefield);
    assert_eq!(out.damage_marked(bystander), 0);
    out.assert_stack_size(0);
}

#[test]
fn essence_burn_target_legality() {
    let mut s = board();
    let spell = burn(&mut s);
    let black = creature(&mut s, P0, "Black", ManaColor::Black, 6);
    let green = creature(&mut s, P1, "Green", ManaColor::Green, 6);
    let black_pw = walker(&mut s, "Black walker", ManaColor::Black, 7);
    let green_pw = walker(&mut s, "Green walker", ManaColor::Green, 7);
    let artifact = s
        .add_creature(P1, "Black artifact", 2, 6)
        .as_artifact()
        .with_color(vec![ManaColor::Black])
        .id();
    let enchantment = s
        .add_creature(P1, "Black enchantment", 2, 6)
        .as_enchantment()
        .with_color(vec![ManaColor::Black])
        .id();
    let land = s
        .add_creature(P1, "Black land", 2, 6)
        .as_land()
        .with_color(vec![ManaColor::Black])
        .id();
    let red = creature(&mut s, P1, "Red", ManaColor::Red, 6);
    let red_pw = walker(&mut s, "Red walker", ManaColor::Red, 7);
    let multicolor = s
        .add_creature(P1, "Black red", 2, 6)
        .with_color(vec![ManaColor::Black, ManaColor::Red])
        .id();
    let colorless = s.add_creature(P1, "Colorless", 2, 6).id();
    let mut r = s.build();
    announce(&mut r, spell);
    let targets = legal(&r);
    for id in [black, green, black_pw, green_pw, multicolor] {
        assert!(
            targets.contains(&TargetRef::Object(id)),
            "positive color/type combination"
        );
    }
    // CR 109.2 + CR 105.2: colors qualify BOTH concrete noun alternatives.
    assert!(
        !targets.contains(&TargetRef::Object(artifact)),
        "Essence Burn must reject a black noncreature nonplaneswalker target"
    );
    for id in [artifact, enchantment, land, red, red_pw, colorless] {
        assert!(!targets.contains(&TargetRef::Object(id)));
        reject(&mut r, id);
    }
    r.act(GameAction::ChooseTarget {
        target: TargetRef::Object(black),
    })
    .expect("legal selection");
    r.advance_until_stack_empty();
    assert_eq!(r.state().objects[&black].damage_marked, 5);
    assert_eq!(r.state().objects[&black].zone, Zone::Battlefield);
    assert_eq!(r.state().objects[&spell].zone, Zone::Graveyard);
    assert!(r.state().stack.is_empty());
}

#[test]
fn essence_burn_exiles_lethal_green_creature() {
    let mut s = board();
    let spell = burn(&mut s);
    let victim = creature(&mut s, P1, "Green", ManaColor::Green, 3);
    let mut r = s.build();
    let out = r.cast(spell).target_object(victim).resolve();
    out.assert_zone(&[victim], Zone::Exile);
    out.assert_zone(&[spell], Zone::Graveyard);
    out.assert_stack_size(0);
}
#[test]
fn essence_burn_exiles_lethal_green_planeswalker() {
    let mut s = board();
    let spell = burn(&mut s);
    let victim = walker(&mut s, "Green", ManaColor::Green, 5);
    let other = walker(&mut s, "Other", ManaColor::Green, 7);
    let mut r = s.build();
    let out = r.cast(spell).target_object(victim).resolve();
    out.assert_zone(&[victim], Zone::Exile);
    out.assert_zone(&[spell], Zone::Graveyard);
    out.assert_zone(&[other], Zone::Battlefield);
    out.assert_counters(other, CounterType::Loyalty, 7);
}
#[test]
fn essence_burn_nonlethal_creature_and_planeswalker_survive() {
    let mut s = board();
    let first = burn(&mut s);
    let second = burn(&mut s);
    let host = creature(&mut s, P1, "Black", ManaColor::Black, 6);
    let pw = walker(&mut s, "Black", ManaColor::Black, 7);
    let mut r = s.build();
    let out = r.cast(first).target_object(host).resolve();
    out.assert_zone(&[host], Zone::Battlefield);
    assert_eq!(out.damage_marked(host), 5);
    let out = r.cast(second).target_object(pw).resolve();
    out.assert_zone(&[pw], Zone::Battlefield);
    out.assert_counters(pw, CounterType::Loyalty, 2);
}
#[test]
fn essence_burn_replaces_later_same_turn_sacrifice() {
    let mut s = board();
    let spell = burn(&mut s);
    let first = rites(&mut s);
    let second = rites(&mut s);
    let host = creature(&mut s, P0, "Host", ManaColor::Black, 6);
    let other = creature(&mut s, P0, "Other", ManaColor::Black, 6);
    let mut r = s.build();
    let out = r.cast(spell).target_object(host).resolve();
    out.assert_zone(&[host], Zone::Battlefield);
    assert_eq!(out.damage_marked(host), 5);
    let out = r.cast(first).sacrifice_with(&[other]).resolve();
    out.assert_zone(&[other], Zone::Graveyard);
    out.assert_hand_drawn(P0, 2);
    out.assert_zone(&[host], Zone::Battlefield);
    let out = r.cast(second).sacrifice_with(&[host]).resolve();
    // CR 700.4: any battlefield-to-graveyard move is death, including a later cost.
    assert_eq!(
        out.zone_of(host),
        Zone::Exile,
        "Essence Burn must replace its target's later same-turn sacrifice"
    );
    out.assert_hand_drawn(P0, 2);
    out.assert_zone(&[second], Zone::Graveyard);
}
#[test]
fn essence_burn_expires_at_cleanup() {
    let mut s = board();
    let spell = burn(&mut s);
    let counterpart_spell = burn(&mut s);
    let sacrifice = rites(&mut s);
    let counterpart_sacrifice = rites(&mut s);
    let host = creature(&mut s, P0, "Host", ManaColor::Black, 6);
    let counterpart = creature(&mut s, P0, "Same turn counterpart", ManaColor::Green, 6);
    let mut r = s.build();
    let out = r.cast(spell).target_object(host).resolve();
    out.assert_zone(&[host], Zone::Battlefield);
    assert_eq!(out.damage_marked(host), 5);
    let out = r
        .cast(counterpart_spell)
        .target_object(counterpart)
        .resolve();
    assert_eq!(out.damage_marked(counterpart), 5);
    let out = r
        .cast(counterpart_sacrifice)
        .sacrifice_with(&[counterpart])
        .resolve();
    out.assert_zone(&[counterpart], Zone::Exile);
    out.assert_hand_drawn(P0, 2);
    out.assert_zone(&[host], Zone::Battlefield);
    let turn = r.state().turn_number;
    // CR 514.2: cross real turn transitions; both marked damage and the grant expire.
    for _ in 0..128 {
        if r.state().turn_number > turn
            && r.state().active_player == P0
            && r.state().phase == Phase::PreCombatMain
        {
            break;
        }
        match r.state().waiting_for {
            WaitingFor::Priority { .. } => {
                r.act(GameAction::PassPriority).expect("turn priority");
            }
            WaitingFor::DeclareAttackers { .. } => {
                r.declare_attackers(&[]).expect("empty combat");
            }
            WaitingFor::DeclareBlockers { .. } => {
                r.declare_blockers(&[]).expect("empty blocks");
            }
            _ => panic!("unexpected turn prompt {:?}", r.state().waiting_for),
        }
    }
    assert!(
        r.state().turn_number > turn,
        "actual cleanup/turn advancement"
    );
    assert_eq!(r.state().phase, Phase::PreCombatMain);
    assert_eq!(r.state().objects[&host].damage_marked, 0);
    // Seed payment only after the real cleanup emptied the previous pool.
    r.state_mut()
        .players
        .iter_mut()
        .find(|p| p.id == P0)
        .unwrap()
        .mana_pool
        .add(mana(ManaType::Black));
    let out = r.cast(sacrifice).sacrifice_with(&[host]).resolve();
    out.assert_zone(&[host], Zone::Graveyard);
    out.assert_hand_drawn(P0, 2);
}
#[test]
fn essence_burn_distinct_casts_keep_distinct_hosts() {
    let mut s = board();
    let a = burn(&mut s);
    let b = burn(&mut s);
    let ra = rites(&mut s);
    let rb = rites(&mut s);
    let first = creature(&mut s, P0, "First", ManaColor::Black, 6);
    let second = creature(&mut s, P0, "Second", ManaColor::Green, 6);
    let mut r = s.build();
    assert_eq!(
        r.cast(a)
            .target_object(first)
            .resolve()
            .damage_marked(first),
        5
    );
    assert_eq!(
        r.cast(b)
            .target_object(second)
            .resolve()
            .damage_marked(second),
        5
    );
    let out = r.cast(ra).sacrifice_with(&[first]).resolve();
    out.assert_zone(&[first], Zone::Exile);
    out.assert_hand_drawn(P0, 2);
    out.assert_zone(&[second], Zone::Battlefield);
    let out = r.cast(rb).sacrifice_with(&[second]).resolve();
    out.assert_zone(&[second], Zone::Exile);
    out.assert_hand_drawn(P0, 2);
}
#[test]
fn essence_burn_no_legal_target_cannot_commit() {
    let mut s = board();
    let spell = burn(&mut s);
    let red = creature(&mut s, P1, "Red", ManaColor::Red, 6);
    let mut r = s.build();
    assert!(r.cast(spell).target_object(red).try_resolve().is_err());
    assert!(r.state().stack.is_empty());
    assert_eq!(r.state().objects[&spell].zone, Zone::Hand);
    // Positive same-board counterpart proves ordinary announcement can reach the prompt.
    let mut s = board();
    let spell = burn(&mut s);
    let green = creature(&mut s, P1, "Green", ManaColor::Green, 6);
    let mut r = s.build();
    let out = r.cast(spell).target_object(green).resolve();
    assert_eq!(out.damage_marked(green), 5);
}
#[test]
fn essence_burn_vanished_target_does_not_rebind() {
    let mut s = board();
    let spell = burn(&mut s);
    let victim = creature(&mut s, P1, "Victim", ManaColor::Black, 6);
    let other = creature(&mut s, P1, "Other", ManaColor::Black, 6);
    let bounce = s
        .add_spell_to_hand_from_oracle(
            P0,
            "Unsummon",
            true,
            "Return target creature to its owner's hand.",
        )
        .with_mana_cost(cost(0, ManaCostShard::Blue))
        .id();
    let mut r = s.build();
    let mut committed = r.cast(spell).target_object(victim).commit();
    assert_eq!(committed.state().objects[&spell].zone, Zone::Stack);
    let out = committed.cast(bounce).target_object(victim).resolve();
    out.assert_zone(&[victim], Zone::Hand);
    out.assert_zone(&[spell, bounce], Zone::Graveyard);
    out.assert_zone(&[other], Zone::Battlefield);
    assert_eq!(out.damage_marked(other), 0);
}
#[test]
fn essence_burn_bounce_reentry_drops_old_grant() {
    let mut s = board();
    let spell = burn(&mut s);
    let sacrifice = rites(&mut s);
    let host = s
        .add_creature(P0, "Host", 2, 6)
        .with_mana_cost(cost(0, ManaCostShard::Black))
        .id();
    let bounce = s
        .add_spell_to_hand_from_oracle(
            P0,
            "Unsummon",
            true,
            "Return target creature to its owner's hand.",
        )
        .with_mana_cost(cost(0, ManaCostShard::Blue))
        .id();
    let mut r = s.build();
    let out = r.cast(spell).target_object(host).resolve();
    assert_eq!(out.damage_marked(host), 5);
    out.assert_zone(&[host], Zone::Battlefield);
    r.cast(bounce)
        .target_object(host)
        .resolve()
        .assert_zone(&[host], Zone::Hand);
    // CR 400.7: a normally recast permanent is a new object with no old grant.
    let out = r.cast(host).resolve();
    out.assert_zone(&[host], Zone::Battlefield);
    assert_eq!(out.damage_marked(host), 0);
    let out = r.cast(sacrifice).sacrifice_with(&[host]).resolve();
    out.assert_zone(&[host], Zone::Graveyard);
    out.assert_hand_drawn(P0, 2);
}

#[test]
fn essence_burn_inline_parse_is_complete() {
    let parsed =
        engine::parser::parse_oracle_text(ESSENCE, "Essence Burn", &[], &["Instant".into()], &[]);
    assert_eq!(parsed.abilities.len(), 1);
    let def = &parsed.abilities[0];
    assert!(matches!(def.effect.as_ref(), Effect::DealDamage { .. }));
    assert!(matches!(
        def.sub_ability
            .as_deref()
            .expect("positive rider")
            .effect
            .as_ref(),
        Effect::AddTargetReplacement { .. }
    ));
    assert!(!gaps(def));
    assert!(
        parsed.parse_warnings.is_empty(),
        "{:?}",
        parsed.parse_warnings
    );
}

#[test]
fn essence_burn_generated_fixture_exiles_lethal_creature() {
    let db = crate::support::shared_card_db().expect("generated integration fixture is required");
    let mut s = board();
    let spell = s.add_real_card(P0, "Essence Burn", Zone::Hand, db);
    let victim = creature(&mut s, P1, "Victim", ManaColor::Black, 3);
    let mut r = s.build();
    let out = r.cast(spell).target_object(victim).resolve();
    out.assert_zone(&[victim], Zone::Exile);
    out.assert_zone(&[spell], Zone::Graveyard);
    out.assert_stack_size(0);
}

#[test]
fn aether_gust_shared_color_target_legality() {
    let parsed =
        engine::parser::parse_oracle_text(GUST, "Aether Gust", &[], &["Instant".into()], &[]);
    assert_eq!(parsed.abilities.len(), 1);
    let Effect::TargetOnly { target } = parsed.abilities[0].effect.as_ref() else {
        panic!("positive spell/permanent target root");
    };
    assert!(matches!(target, TargetFilter::Or { .. }));
    assert!(!gaps(&parsed.abilities[0]));
    assert!(
        parsed.parse_warnings.is_empty(),
        "{:?}",
        parsed.parse_warnings
    );
    // CR 601.2c + CR 105.2: each pending spell uses its verified printed cost/color.
    for (name, oracle, shard, color, mana_color) in [
        ("Shock", "Shock deals 2 damage to any target.", ManaCostShard::Red, ManaColor::Red, ManaType::Red),
        ("Giant Growth", "Target creature gets +3/+3 until end of turn.", ManaCostShard::Green, ManaColor::Green, ManaType::Green),
        ("Opt", "Scry 1. (Look at the top card of your library. You may put that card on the bottom.)\nDraw a card.", ManaCostShard::Blue, ManaColor::Blue, ManaType::Blue),
    ] {
        let mut s = board(); s.with_mana_pool(P0, vec![mana(mana_color), mana(ManaType::Blue), mana(ManaType::Red)]);
        let red = creature(&mut s, P1, "Red", ManaColor::Red, 6);
        let green = creature(&mut s, P1, "Green", ManaColor::Green, 6);
        let blue = creature(&mut s, P1, "Blue", ManaColor::Blue, 6);
        let pending = s.add_spell_to_hand_from_oracle(P0, name, true, oracle).with_mana_cost(cost(0, shard)).id();
        let gust = s.add_spell_to_hand_from_oracle(P0, "Aether Gust", true, GUST).with_mana_cost(cost(1, ManaCostShard::Blue)).id();
        let mut r = s.build();
        {
            let committed = if name == "Opt" { r.cast(pending).commit() } else { r.cast(pending).target_object(red).commit() };
            assert_eq!(committed.state().objects[&pending].zone, Zone::Stack);
        }
        assert_eq!(r.state().objects[&pending].zone, Zone::Stack);
        assert_eq!(r.state().objects[&pending].color, vec![color]);
        announce(&mut r, gust); let targets = legal(&r);
        assert!(targets.contains(&TargetRef::Object(red))); assert!(targets.contains(&TargetRef::Object(green)));
        assert_eq!(targets.contains(&TargetRef::Object(pending)), color != ManaColor::Blue, "pending {name} color legality");
        assert!(!targets.contains(&TargetRef::Object(blue))); reject(&mut r, blue);
        if color == ManaColor::Blue { reject(&mut r, pending); }
        assert_eq!(r.state().objects[&pending].zone, Zone::Stack);
        // Stop at the live target prompt; library placement/owner choice is not asserted.
        assert!(matches!(r.state().waiting_for, WaitingFor::TargetSelection { .. }));
    }
}
