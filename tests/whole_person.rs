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

use scale_sim::converse::{ask_of, Approach, Asked};
use scale_sim::coping::FunctionalState;
use scale_sim::id::{Arena, Id};
use scale_sim::memory::{EventKind, Place, Source, WorldEvent};
use scale_sim::mind::{Happening, Value};
use scale_sim::person::{Person, Trade};
use scale_sim::relations::Relationship;
use scale_sim::rng::Rng;

/// **The person who has a job is the person who has values.**
///
/// One object. Sabotage: take the mind off `Person` and this does not
/// compile, which is the strongest form this gate can take.
#[test]
fn the_man_with_a_trade_is_the_man_with_convictions() {
    let p = Person::new("Alder", Trade::ProductionWorker, 0, 100.0);

    // The economic half.
    assert_eq!(p.trade, Trade::ProductionWorker);
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
    let a = Person::new("Alder", Trade::ProductionWorker, 0, 100.0);
    let b = Person::new("Bramwell", Trade::ProductionWorker, 0, 100.0);

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
    let a = Person::new("Alder", Trade::ProductionWorker, 0, 100.0);
    let again = Person::new("Alder", Trade::ProductionWorker, 0, 100.0);
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
    let moved = Person::new("Alder", Trade::Sales, 3, 7.0);
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
        .map(|i| Person::new(format!("person {i}"), Trade::ProductionWorker, 0, 100.0))
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

// =====================================================================
// and the person answers out of their own head
// =====================================================================

/// Something that happened, which one man saw and another did not.
fn a_fight(day: u64, a: Id<Person>, b: Id<Person>) -> WorldEvent {
    WorldEvent {
        kind: EventKind::Assault,
        who: vec![a, b],
        actor: Some(b),
        place: Place(1),
        day,
        severity: -0.8,
        facts: Happening {
            severity: -0.8,
            deliberate: true,
            unexpected: 0.9,
            ..Default::default()
        },
    }
}

/// Put an event into one person's own memory, through the ordinary path.
fn saw_it(p: &mut Person, ev: &WorldEvent, of: Id<WorldEvent>, seed: u64) {
    let mut rng = Rng::new(seed);
    let perceived = p
        .memory
        .perceive(ev, Some(of), Source::Witnessed, 0.95, &mut rng)
        .expect("nothing perceived");
    let felt = p.mind.appraise(&p.mind.read(&perceived.facts));
    p.memory
        .encode(perceived, &p.mind, ev.day, &felt)
        .expect("nothing encoded");
}

/// **What a person knows is what is in their own head**, and no caller
/// can hand it to them.
///
/// `converse::ask` takes a `&Mind`, a `&Memory` and an
/// `Option<&Relationship>` alongside the `Id<Person>` handles, so that
/// those belonged to the person being addressed was a caller's promise.
/// Nothing stopped one man's memory being passed with another man's name.
/// `ask_of` takes the person, so there is nothing to get wrong.
#[test]
fn a_person_answers_from_their_own_memory() {
    let mut folk: Arena<Person> = Arena::new();
    let watched = folk.add(Person::new("Alder", Trade::ProductionWorker, 0, 100.0));
    let elsewhere = folk.add(Person::new("Bramwell", Trade::ProductionWorker, 0, 100.0));
    let asker = folk.add(Person::new("Cade", Trade::ProductionWorker, 0, 100.0));

    let mut happened: Arena<WorldEvent> = Arena::new();
    let ev = a_fight(10, watched, elsewhere);
    let of = happened.add(ev.clone());
    saw_it(folk.get_mut(watched).expect("there"), &ev, of, 3);

    let put_it_to = |who: Id<Person>, folk: &Arena<Person>| {
        ask_of(
            folk.get(who).expect("there"),
            who,
            asker,
            FunctionalState::Regulated,
            0.0,
            &Approach::a_friend(),
            Asked::About(of),
        )
    };

    let there = put_it_to(watched, &folk);
    let absent = put_it_to(elsewhere, &folk);

    assert!(
        there.because.knows,
        "the man who saw it does not know about it: {}",
        there.said
    );
    assert!(
        !absent.because.knows,
        "the man who was somewhere else knows about it anyway — his answer          is coming from outside his own head: {}",
        absent.said
    );
    assert_ne!(
        there.said, absent.said,
        "a witness and a man who was elsewhere gave the same answer"
    );
}

/// **What they make of you is their record of you**, and it is theirs.
///
/// Directed on purpose: Alice's opinion of Bob and Bob's of Alice are two
/// things that need not agree. Before the join, whose opinion reached the
/// conversation was whatever the caller passed.
#[test]
fn what_they_think_of_you_is_held_in_their_own_head() {
    let mut folk: Arena<Person> = Arena::new();
    let them = folk.add(Person::new("Alder", Trade::ProductionWorker, 0, 100.0));
    let asker = folk.add(Person::new("Cade", Trade::ProductionWorker, 0, 100.0));
    let stranger = folk.add(Person::new("Dunn", Trade::ProductionWorker, 0, 100.0));

    // A relationship is something that happens, not something issued.
    assert!(
        folk.get(them).expect("there").relations.is_empty(),
        "somebody was born already knowing people"
    );

    folk.get_mut(them)
        .expect("there")
        .relations
        .insert(asker, Relationship::strangers(them, asker));

    let p = folk.get(them).expect("there");
    assert!(
        p.relations.contains_key(&asker),
        "the record of the asker did not stay with the person"
    );
    assert!(
        !p.relations.contains_key(&stranger),
        "a record appeared for somebody never met"
    );

    // And it is *their* record: the asker holds nothing about them.
    assert!(
        folk.get(asker).expect("there").relations.is_empty(),
        "recording one side of a relationship wrote the other side too —          Alice's view of Bob and Bob's of Alice are two things"
    );
}
