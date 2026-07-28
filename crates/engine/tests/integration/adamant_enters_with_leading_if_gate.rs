//! Runtime cast-pipeline coverage for the SENTENCE-INITIAL "if <condition>, "
//! enters-with gate (CR 614.1c), the position Adamant and Spell mastery cards
//! write their gate in.
//!
//! Before the fix, `parse_enters_with_counters` recognized a gate only in the
//! trailing " unless …" / " … if …" positions, so a leading gate was silently
//! swallowed and the replacement became UNCONDITIONAL — the Adamant Paladins
//! entered with a +1/+1 counter no matter what mana paid for them.
//!
//! Built via the `/card-test` recipe: `GameScenario` +
//! `GameRunner::cast(..).resolve()` + `CastOutcome` counter/life deltas, on
//! verbatim Oracle text. Every negative assertion is paired with a positive
//! reach-guard in the same test AND with a structural guard proving the card
//! parsed (a `Some` replacement condition, zero `Effect::Unimplemented`), so an
//! upstream parse failure cannot satisfy it vacuously.
//!
//! REVERT DISCRIMINATOR: `ardenvale_paladin_white_below_threshold_no_counter`
//! (R2). Neutralize `extract_enters_with_leading_if_gate` to always return
//! `NoLeadingIf` and the gate is dropped again — the counter applies
//! unconditionally and R2's `assert_counters(.., 0)` fails.

use engine::game::scenario::{CastOutcome, GameRunner, GameScenario, P0, P1};
use engine::types::ability::Effect;
use engine::types::counter::CounterType;
use engine::types::identifiers::ObjectId;
use engine::types::keywords::KeywordKind;
use engine::types::mana::{ManaColor, ManaCost, ManaCostShard, ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::zones::Zone;

/// Ardenvale Paladin {3}{W} 2/3 — verbatim Oracle text (`data/card-data.json`).
const ARDENVALE_PALADIN: &str = "Adamant — If at least three white mana was spent to cast this \
                                 spell, this creature enters with a +1/+1 counter on it.";

/// Embereth Paladin {3}{R} 3/1 — verbatim Oracle text, Haste line included.
const EMBERETH_PALADIN: &str = "Haste\nAdamant — If at least three red mana was spent to cast \
                                this spell, this creature enters with a +1/+1 counter on it.";

/// Dust Animus {1}{W} 1/1 — verbatim Oracle text. The Plot line is dropped
/// because plotting is irrelevant here and its reminder text would only add
/// unrelated parse surface; the leading-if line under test is verbatim.
const DUST_ANIMUS: &str = "Flying\nIf you control five or more untapped lands, this creature \
                           enters with two +1/+1 counters and a lifelink counter on it.";

/// Slaying Fire {2}{R} — verbatim Oracle text. Guards the Adamant ABILITY
/// RIDER, which converges on the same `OfColor` quantity shape as the
/// replacement gate.
const SLAYING_FIRE: &str = "Slaying Fire deals 3 damage to any target.\nAdamant — If at least \
                            three red mana was spent to cast this spell, it deals 4 damage \
                            instead.";

fn mana(kind: ManaType, n: usize) -> Vec<ManaUnit> {
    vec![ManaUnit::new(kind, ObjectId(0), false, vec![]); n]
}

/// Build a pool from a colored/colorless mix, in one place so each row reads as
/// its payment record.
fn pool(colored: &[(ManaType, usize)]) -> Vec<ManaUnit> {
    colored
        .iter()
        .flat_map(|(kind, n)| mana(*kind, *n))
        .collect()
}

/// Cast a 4-mana Paladin ({3}{<shard>}) out of an exactly-sized pool and return
/// the outcome plus its object id.
///
/// The pool always holds EXACTLY the four mana the cost needs, so the auto-payer
/// has no discretion: every unit in the pool is spent and `colors_spent_to_cast`
/// (CR 601.2h) is fully determined by the pool contents. That removes payment
/// nondeterminism from every assertion below.
fn cast_paladin(
    name: &str,
    oracle: &str,
    shard: ManaCostShard,
    power: i32,
    toughness: i32,
    payment: &[(ManaType, usize)],
) -> (CastOutcome, ObjectId) {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let paladin = scenario
        .add_creature_to_hand_from_oracle(P0, name, power, toughness, oracle)
        .with_mana_cost(ManaCost::Cost {
            generic: 3,
            shards: vec![shard],
        })
        .id();
    scenario.with_mana_pool(P0, pool(payment));
    let mut runner = scenario.build();

    assert_gate_is_attached(&runner, paladin, name);

    let outcome = runner.cast(paladin).resolve();
    assert_eq!(
        outcome.zone_of(paladin),
        Zone::Battlefield,
        "{name} must resolve onto the battlefield"
    );
    (outcome, paladin)
}

/// Structural reach-guard (the `/card-test` foot-gun #6 defence): the card
/// really parsed, the enters-with replacement really exists, and it really
/// carries a condition. Without this, a "0 counters" assertion would pass just
/// as well on a card whose replacement failed to parse at all.
fn assert_gate_is_attached(runner: &GameRunner, obj: ObjectId, name: &str) {
    let object = &runner.state().objects[&obj];
    assert!(
        !object
            .abilities
            .iter()
            .any(|a| matches!(&*a.effect, Effect::Unimplemented { .. })),
        "{name} must parse with zero Effect::Unimplemented, got {:?}",
        object.abilities
    );
    let def = object
        .replacement_definitions
        .first()
        .unwrap_or_else(|| panic!("{name} must publish an enters-with replacement"));
    assert!(
        def.condition.is_some(),
        "{name}'s enters-with replacement must carry the leading-if gate; \
         a None condition means the gate was swallowed and the counter is unconditional"
    );
}

// ---------------------------------------------------------------------------
// R1-R6 — the Adamant per-color threshold, CR 106.3 + CR 601.2h.
// ---------------------------------------------------------------------------

/// R1 — POSITIVE reach-guard for the whole white family. {W}{W}{W}{W} pays
/// {3}{W}: white spent = 4 >= 3, so the counter applies. Discriminates
/// `OfColor` from `DistinctColors` (which is 1 here, below the threshold).
#[test]
fn ardenvale_paladin_four_white_applies_counter() {
    let (outcome, paladin) = cast_paladin(
        "Ardenvale Paladin",
        ARDENVALE_PALADIN,
        ManaCostShard::White,
        2,
        3,
        &[(ManaType::White, 4)],
    );
    outcome.assert_counters(paladin, CounterType::Plus1Plus1, 1);
}

/// R2 — **THE PRIMARY REVERT DISCRIMINATOR.** One white + three colorless pays
/// {3}{W}: white spent = 1 < 3, so NO counter. Total mana spent is 4 >= 3, so
/// this row also discriminates `OfColor` from `CastManaSpentMetric::Total`.
///
/// Drop the leading-if peel and the replacement becomes unconditional → 1
/// counter → this assertion fails. Paired reach-guard: R1 above.
#[test]
fn ardenvale_paladin_white_below_threshold_no_counter() {
    let (outcome, paladin) = cast_paladin(
        "Ardenvale Paladin",
        ARDENVALE_PALADIN,
        ManaCostShard::White,
        2,
        3,
        &[(ManaType::White, 1), (ManaType::Colorless, 3)],
    );
    outcome.assert_counters(paladin, CounterType::Plus1Plus1, 0);
}

/// R6 — pins the comparator (GE, not GT) and the literal threshold 3. Exactly
/// three white → counter applies; exactly two white → it does not.
#[test]
fn ardenvale_paladin_threshold_is_greater_or_equal_three() {
    let (at_threshold, paladin) = cast_paladin(
        "Ardenvale Paladin",
        ARDENVALE_PALADIN,
        ManaCostShard::White,
        2,
        3,
        &[(ManaType::White, 3), (ManaType::Colorless, 1)],
    );
    at_threshold.assert_counters(paladin, CounterType::Plus1Plus1, 1);

    let (below, paladin) = cast_paladin(
        "Ardenvale Paladin",
        ARDENVALE_PALADIN,
        ManaCostShard::White,
        2,
        3,
        &[(ManaType::White, 2), (ManaType::Colorless, 2)],
    );
    below.assert_counters(paladin, CounterType::Plus1Plus1, 0);
}

/// R4 — POSITIVE reach-guard for the red family, and proof the color is read
/// per-card rather than hardcoded to the first card fixed (white). Three red +
/// one colorless pays {3}{R}: red spent = 3 >= 3 → counter.
#[test]
fn embereth_paladin_three_red_applies_counter() {
    let (outcome, paladin) = cast_paladin(
        "Embereth Paladin",
        EMBERETH_PALADIN,
        ManaCostShard::Red,
        3,
        1,
        &[(ManaType::Red, 3), (ManaType::Colorless, 1)],
    );
    outcome.assert_counters(paladin, CounterType::Plus1Plus1, 1);
}

/// R3 — the gate reads the card's OWN color. {W}{W}{W}{R} pays Embereth's
/// {3}{R}: red = 1 < 3 (no counter) even though WHITE = 3 would have passed
/// Ardenvale's gate, and total = 4 would have passed a `Total` gate.
/// Paired reach-guard: R4 above.
#[test]
fn embereth_paladin_reads_red_not_white() {
    let (outcome, paladin) = cast_paladin(
        "Embereth Paladin",
        EMBERETH_PALADIN,
        ManaCostShard::Red,
        3,
        1,
        &[(ManaType::White, 3), (ManaType::Red, 1)],
    );
    outcome.assert_counters(paladin, CounterType::Plus1Plus1, 0);
}

/// R5 — the row that separates `OfColor` from `DistinctColors` at a point where
/// `DistinctColors` PASSES. {W}{U}{B}{R} pays Embereth's {3}{R}: four distinct
/// colors (>= 3, so a `DistinctColors` gate would fire) but red = 1 < 3, so the
/// correct `OfColor` gate does not. Paired reach-guard: R4 above.
#[test]
fn embereth_paladin_four_distinct_colors_still_no_counter() {
    let (outcome, paladin) = cast_paladin(
        "Embereth Paladin",
        EMBERETH_PALADIN,
        ManaCostShard::Red,
        3,
        1,
        &[
            (ManaType::White, 1),
            (ManaType::Blue, 1),
            (ManaType::Black, 1),
            (ManaType::Red, 1),
        ],
    );
    outcome.assert_counters(paladin, CounterType::Plus1Plus1, 0);
}

// ---------------------------------------------------------------------------
// R7 — Dust Animus: a leading-if gate that is NOT a mana-spent threshold.
// Proves the peel makes the WHOLE `parse_inner_condition` grammar reachable
// from the sentence-initial position, not just the one new arm.
// ---------------------------------------------------------------------------

/// Cast Dust Animus ({1}{W}) out of an exact pool while controlling
/// `untapped_lands` untapped and `tapped_lands` tapped Plains. The lands are
/// never tapped for mana (the pool is pre-staged), so their tap state is
/// controlled purely by the fixture.
fn cast_dust_animus(untapped_lands: usize, tapped_lands: usize) -> (CastOutcome, ObjectId) {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let animus = scenario
        .add_creature_to_hand_from_oracle(P0, "Dust Animus", 1, 1, DUST_ANIMUS)
        .with_mana_cost(ManaCost::Cost {
            generic: 1,
            shards: vec![ManaCostShard::White],
        })
        .id();
    let mut to_tap = Vec::new();
    for _ in 0..untapped_lands {
        scenario.add_basic_land(P0, ManaColor::White);
    }
    for _ in 0..tapped_lands {
        to_tap.push(scenario.add_basic_land(P0, ManaColor::White));
    }
    scenario.with_mana_pool(P0, pool(&[(ManaType::White, 1), (ManaType::Colorless, 1)]));
    let mut runner = scenario.build();
    for land in to_tap {
        runner.state_mut().objects.get_mut(&land).unwrap().tapped = true;
    }

    assert_gate_is_attached(&runner, animus, "Dust Animus");

    let outcome = runner.cast(animus).resolve();
    assert_eq!(
        outcome.zone_of(animus),
        Zone::Battlefield,
        "Dust Animus must resolve onto the battlefield"
    );
    (outcome, animus)
}

/// R7a — POSITIVE reach-guard: five untapped lands satisfies the gate, so Dust
/// Animus keeps both counter payloads. This is the "the peel did not break the
/// already-green card" row (Dust Animus is `supported=true` today, but was
/// silently UNCONDITIONAL — the gate was swallowed).
#[test]
fn dust_animus_five_untapped_lands_applies_counters() {
    let (outcome, animus) = cast_dust_animus(5, 0);
    outcome.assert_counters(animus, CounterType::Plus1Plus1, 2);
    assert_eq!(
        outcome.counters(animus, CounterType::Keyword(KeywordKind::Lifelink)),
        1,
        "the lifelink counter rides the same gated payload"
    );
}

/// R7b — REVERT DISCRIMINATOR (second polarity). Six lands, one of them TAPPED,
/// leaves four untapped: below the "five or more untapped lands" threshold, so
/// no counters. Drop the peel and the payload applies unconditionally → 2 +1/+1
/// counters → this fails. Also discriminates `FilterProp::Untapped` from a bare
/// land count: a count-only reading would see six lands and fire.
/// Paired reach-guard: R7a above.
#[test]
fn dust_animus_only_four_untapped_lands_no_counters() {
    let (outcome, animus) = cast_dust_animus(4, 2);
    outcome.assert_counters(animus, CounterType::Plus1Plus1, 0);
    assert_eq!(
        outcome.counters(animus, CounterType::Keyword(KeywordKind::Lifelink)),
        0,
        "the whole gated payload is suppressed, not just the +1/+1 counters"
    );
}

// ---------------------------------------------------------------------------
// R8 — the Adamant ABILITY RIDER. The same grammar change re-routes 11 riders
// from `AbilityCondition::ManaColorSpent` to the generic
// `QuantityCheck { ManaSpentToCast { .., OfColor } }`. This is the mandatory
// runtime guard on that AST churn: the observable damage must not move.
// ---------------------------------------------------------------------------

fn cast_slaying_fire(payment: &[(ManaType, usize)]) -> CastOutcome {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let fire = scenario
        .add_spell_to_hand_from_oracle(P0, "Slaying Fire", true, SLAYING_FIRE)
        .with_mana_cost(ManaCost::Cost {
            generic: 2,
            shards: vec![ManaCostShard::Red],
        })
        .id();
    scenario.with_mana_pool(P0, pool(payment));
    let mut runner = scenario.build();
    assert!(
        !runner.state().objects[&fire]
            .abilities
            .iter()
            .any(|a| matches!(&*a.effect, Effect::Unimplemented { .. })),
        "Slaying Fire must parse with zero Effect::Unimplemented, got {:?}",
        runner.state().objects[&fire].abilities
    );
    runner.cast(fire).target_player(P1).resolve()
}

/// R8 positive: three red mana satisfies the Adamant rider → 4 damage, not 3.
#[test]
fn slaying_fire_three_red_deals_four() {
    let outcome = cast_slaying_fire(&[(ManaType::Red, 3)]);
    outcome.assert_life_delta(P1, -4);
}

/// R8 negative (paired with the positive above): one red + two colorless is
/// three TOTAL mana but only one RED, so the rider does not fire → 3 damage.
/// Discriminates `OfColor` from `Total` on the ability-rider path exactly as R2
/// does on the replacement path.
#[test]
fn slaying_fire_one_red_deals_three() {
    let outcome = cast_slaying_fire(&[(ManaType::Red, 1), (ManaType::Colorless, 2)]);
    outcome.assert_life_delta(P1, -3);
}
