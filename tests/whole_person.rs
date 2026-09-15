//! **A person is one person.**
//!
//! Until this existed there were two halves of a human being in this
//! simulation and nothing that was both. `person::Person` had a trade, a
//! wage, a house, a skill and a household; `mind::Mind` had a
//! personality, values, concerns and needs — and **nothing owned a mind
//! at all**. `Mind::draw` was called by tests and one diagnostic binary,
//! and `populace.rs`, `labour.rs` and `game.rs` mentioned the type
//! nowhere.
//!
//! `converse::ask` takes `Id<Person>` handles for speaker and listener
//! *and* a `&Mind` as a separate argument, so the two halves being the
//! same individual was **a caller's promise rather than a type**. The
//! consequence is the one that matters: the person you could walk up to
//! and talk to did not exist. The one with a job and a house had no
//! values, no memory and no relationships; the one that could be lied to
//! was employed by nobody, housed by nobody and paid by nobody.

use scale_sim::mind::Value;
use scale_sim::person::{Person, Trade};

/// **The person who has a job is the person who has values.**
///
/// One object. Sabotage: take the mind off `Person` and this does not
/// compile, which is the strongest form this gate can take.
#[test]
fn the_man_with_a_trade_is_the_man_with_convictions() {
    let p = Person::new("Alder", Trade::Labourer, 0, 100.0);

    // The economic half.
    assert_eq!(p.trade, Trade::Labourer);
    assert_eq!(p.market, 0);

    // And the same object carries what he believes.
    let held: Vec<i8> = Value::ALL.iter().map(|&v| p.mind.conviction(v)).collect();
    assert_eq!(
        held.len(),
        Value::ALL.len(),
        "a person does not hold an opinion on every value"
    );
    assert!(
        held.iter().any(|&h| h != 0),
        "this person holds nothing at all — a mind was attached but never drawn"
    );
}

/// **Two people are two people.**
///
/// The point of giving everybody their own mind rather than a shared one:
/// if every person drew the same convictions there would be no room for a
/// liar among honest men, and the value gating social skills are to hang
/// off would have nothing to bite on.
#[test]
fn no_two_people_are_the_same_person() {
    let a = Person::new("Alder", Trade::Labourer, 0, 100.0);
    let b = Person::new("Bramwell", Trade::Labourer, 0, 100.0);

    let differs = Value::ALL
        .iter()
        .filter(|&&v| a.mind.conviction(v) != b.mind.conviction(v))
        .count();
    assert!(
        differs >= Value::ALL.len() / 2,
        "two people differ on only {differs} of {} values — they are \
         drawing from the same mind rather than their own",
        Value::ALL.len()
    );

    // And the spread is real rather than a rounding: somebody, somewhere,
    // holds something strongly.
    let strongest = Value::ALL
        .iter()
        .map(|&v| a.mind.conviction(v).abs().max(b.mind.conviction(v).abs()))
        .max()
        .unwrap_or(0);
    assert!(
        strongest > 15,
        "nobody holds anything more strongly than {strongest} — these are \
         not convictions, they are noise"
    );
}

/// **The same name rebuilds the same person**, mind and all.
///
/// This project's oldest rule reaching the newest field: a seed must
/// rebuild the same world for ever, and a person whose convictions were
/// drawn from a wandering RNG would come back a different human being.
#[test]
fn a_name_rebuilds_the_same_mind() {
    let a = Person::new("Alder", Trade::Labourer, 0, 100.0);
    let again = Person::new("Alder", Trade::Labourer, 0, 100.0);
    for &v in Value::ALL.iter() {
        assert_eq!(
            a.mind.conviction(v),
            again.mind.conviction(v),
            "the same name gave two different opinions about {}",
            v.name()
        );
    }

    // And it is the *name* that decides, not the trade or the town — or
    // moving somebody between markets would replace them.
    let moved = Person::new("Alder", Trade::Shopworker, 3, 7.0);
    for &v in Value::ALL.iter() {
        assert_eq!(
            a.mind.conviction(v),
            moved.mind.conviction(v),
            "changing somebody's trade changed what they believe about {}",
            v.name()
        );
    }
}

/// **A sampled town is full of people who each hold their own opinions.**
///
/// The unit gates above prove the type; this proves the population is not
/// accidentally uniform, which is what would happen if every person were
/// seeded off the same number.
#[test]
fn a_town_holds_a_spread_of_opinion() {
    let folk: Vec<Person> = (0..60)
        .map(|i| Person::new(format!("person {i}"), Trade::Labourer, 0, 100.0))
        .collect();

    // Truth is the one the social skills will gate on: DF will not let a
    // dwarf train as a liar unless they hold truth cheaply.
    let truths: Vec<i8> = folk.iter().map(|p| p.mind.conviction(Value::Truth)).collect();
    let low = truths.iter().filter(|&&t| t < -10).count();
    let high = truths.iter().filter(|&&t| t > 10).count();
    assert!(
        low > 0 && high > 0,
        "of sixty people, {low} hold truth cheaply and {high} hold it \
         dearly — a town with nobody at either end has no liars to catch \
         and nobody to be shocked by them"
    );

    // And nobody is a copy of anybody: at least most of the town differs.
    let mut seen: Vec<Vec<i8>> = folk
        .iter()
        .map(|p| Value::ALL.iter().map(|&v| p.mind.conviction(v)).collect())
        .collect();
    seen.sort();
    let before = seen.len();
    seen.dedup();
    assert_eq!(
        seen.len(),
        before,
        "two people in this town are identical in every conviction"
    );
}
