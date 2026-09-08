//! **Walking up to somebody and talking to them.**
//!
//! What is guarded here is that the answer comes out of *their* life:
//! what they saw, how they came by it, what they make of you, and what
//! they are carrying. Nothing is looked up in the world's own record.

use scale_sim::converse::{
    ask, courtesy_exchange, has_anything_to_say, what_was_seen, Approach, Asked, Courtesy, Occasion,
};
use scale_sim::coping::FunctionalState;
use scale_sim::id::{Arena, Id};
use scale_sim::memory::{EventKind, Memory, PerceivedWho, Place, Source, WorldEvent};
use scale_sim::mind::{Facet, Happening, Mind, Value};
use scale_sim::person::{Person, Trade};
use scale_sim::relations::{Evidence, Relationship, TrustIn};
use scale_sim::rng::Rng;

fn who(n: u32) -> Id<Person> {
    let mut a: Arena<Person> = Arena::new();
    let mut last = a.add(Person::new("x", Trade::Labourer, 0, 0.0));
    for _ in 0..n {
        last = a.add(Person::new("x", Trade::Labourer, 0, 0.0));
    }
    last
}

fn a_mind(seed: u64) -> Mind {
    let mut m = Mind::draw(&mut Rng::new(seed), &[(Value::Fairness, 25)]);
    for f in Facet::ALL {
        m.person.set_baseline(f, 0.0);
    }
    m.mood.valence = 0.0;
    m
}

fn a_fight(day: u64) -> WorldEvent {
    WorldEvent {
        kind: EventKind::Assault,
        who: vec![who(1), who(2)],
        actor: Some(who(2)),
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

/// Somebody who saw it, and the id of the event they saw.
///
/// **The world's record of the event is a separate thing from theirs.**
/// The id is what lets a question find whatever this person has of it —
/// and passing no id at all is how the model represents having been
/// told about something that never happened.
fn a_witness(seed: u64, how: Source, clarity: f64) -> (Mind, Memory, Id<WorldEvent>) {
    let mind = a_mind(seed);
    let mut mem = Memory::new();
    let mut rng = Rng::new(seed);
    let mut happened: Arena<WorldEvent> = Arena::new();
    let ev = a_fight(10);
    let of = happened.add(ev.clone());
    let p = mem
        .perceive(&ev, Some(of), how, clarity, &mut rng)
        .expect("nothing perceived");
    let felt = mind.appraise(&mind.read(&p.facts));
    mem.encode(p, &mind, 10, &felt).expect("nothing encoded");
    (mind, mem, of)
}

// =====================================================================
// they answer from their own knowledge
// =====================================================================

/// **Somebody who was not there cannot be made to know.** There is no
/// route from the world's record into a mouth.
#[test]
fn a_man_who_was_not_there_has_nothing_to_tell_you() {
    let (_, _, of) = a_witness(1, Source::Witnessed, 0.9);
    let elsewhere = a_mind(2);
    let nothing = Memory::new();

    assert!(!has_anything_to_say(&nothing, of));
    let a = ask(
        &elsewhere,
        &nothing,
        None,
        FunctionalState::Regulated,
        0.0,
        &Approach::a_friend(),
        Asked::About(of),
        who(9),
        who(2),
    );
    assert!(!a.because.knows);
    assert!(a.said.contains("not there"), "{}", a.said);
}

/// **A rumour answers differently from having been there**, and going
/// over it often does not promote it.
#[test]
fn how_somebody_came_to_know_shows_in_what_they_say() {
    let (saw, saw_mem, of) = a_witness(3, Source::Witnessed, 0.95);
    let (heard, heard_mem, of2) = a_witness(3, Source::Rumour { hops: 2 }, 0.5);

    let first = ask(
        &saw,
        &saw_mem,
        None,
        FunctionalState::Regulated,
        0.0,
        &Approach::a_friend(),
        Asked::About(of),
        who(9),
        who(1),
    );
    let second = ask(
        &heard,
        &heard_mem,
        None,
        FunctionalState::Regulated,
        0.0,
        &Approach::a_friend(),
        Asked::About(of2),
        who(9),
        who(1),
    );

    assert_eq!(first.because.how, Some(Source::Witnessed));
    assert_eq!(second.because.how, Some(Source::Rumour { hops: 2 }));
    assert_ne!(
        first.said, second.said,
        "an eyewitness and a rumour said the same thing"
    );
    assert!(first.said.contains("saw"), "{}", first.said);
}

/// **What they say can be wrong, and stays wrong** until somebody
/// corrects them — and correcting them changes the blame, not what they
/// saw.
#[test]
fn they_can_be_mistaken_and_go_on_being_mistaken() {
    let (mind, mut mem, of) = a_witness(5, Source::Witnessed, 0.9);
    let trace = mem.traces.ids().next().expect("no trace");
    mem.reattribute(trace, PerceivedWho::Known(who(7)), 0.8);

    let a = ask(
        &mind,
        &mem,
        None,
        FunctionalState::Regulated,
        0.0,
        &Approach::a_friend(),
        Asked::About(of),
        who(9),
        who(1),
    );
    assert_eq!(a.because.blames, Some(PerceivedWho::Known(who(7))));
    // He still says he saw it, because he did.
    assert_eq!(a.because.how, Some(Source::Witnessed));
}

// =====================================================================
// they answer according to what they make of you
// =====================================================================

/// **A man who does not trust you tells you less**, and that is
/// discretion rather than deception: he knows perfectly well.
#[test]
fn distrust_withholds_what_it_does_not_erase() {
    let (mind, mem, of) = a_witness(11, Source::Witnessed, 0.95);
    let mut wary = Relationship::strangers(who(1), who(9));
    for day in 0..40u64 {
        wary.saw(
            &Evidence {
                contact: 0.5,
                reliability: -0.9,
                reliability_in: Some(TrustIn::General),
                ..Default::default()
            },
            day,
        );
    }
    assert!(
        wary.trust_in(TrustIn::General) < -0.3,
        "he does not distrust him enough: {}",
        wary.trust_in(TrustIn::General)
    );

    let open = ask(
        &mind,
        &mem,
        None,
        FunctionalState::Regulated,
        0.0,
        &Approach::a_friend(),
        Asked::About(of),
        who(9),
        who(1),
    );
    let closed = ask(
        &mind,
        &mem,
        Some(&wary),
        FunctionalState::Regulated,
        0.0,
        &Approach::a_friend(),
        Asked::About(of),
        who(9),
        who(1),
    );

    assert_ne!(open.said, closed.said);
    // He still knows it. The knowledge did not go anywhere.
    assert!(closed.because.knows, "distrust deleted his memory");
    assert!(closed.act.delivery.warmth < open.act.delivery.warmth);
}

/// **An open grievance is not a mood.** He is civil to a stranger and
/// short with the man he has something against.
#[test]
fn somebody_you_have_wronged_is_colder_to_you() {
    let mind = a_mind(13);
    let mem = Memory::new();
    let mut sore = Relationship::strangers(who(1), who(9));
    sore.saw(
        &Evidence {
            wrong: 0.8,
            ..Default::default()
        },
        1,
    );
    assert!(sore.resentment() > 0.3, "the wrong left no grievance");

    let stranger = ask(
        &mind,
        &mem,
        None,
        FunctionalState::Regulated,
        0.5,
        &Approach::a_friend(),
        Asked::Greeting,
        who(9),
        who(1),
    );
    let wronged = ask(
        &mind,
        &mem,
        Some(&sore),
        FunctionalState::Regulated,
        0.5,
        &Approach::a_friend(),
        Asked::Greeting,
        who(9),
        who(1),
    );
    assert!(wronged.act.delivery.warmth < stranger.act.delivery.warmth);
    assert_ne!(wronged.said, stranger.said);
}

// =====================================================================
// they answer according to what they are carrying
// =====================================================================

/// **The same man is worse company in a bad year**, and it is not a
/// personality trait.
#[test]
fn what_somebody_is_carrying_shows_when_you_talk_to_them() {
    let mind = a_mind(17);
    let mem = Memory::new();
    let says = |state| {
        ask(
            &mind,
            &mem,
            None,
            state,
            0.0,
            &Approach::a_friend(),
            Asked::HowTheyAre,
            who(9),
            who(1),
        )
    };
    let well = says(FunctionalState::Regulated);
    let worn = says(FunctionalState::Depleted);
    let gone = says(FunctionalState::Impaired);

    assert_ne!(well.said, worn.said);
    assert_ne!(worn.said, gone.said);
    assert!(well.act.delivery.warmth > gone.act.delivery.warmth);
    // The hesitation is the state showing through, not a speech habit.
    assert!(gone.act.delivery.hesitation > well.act.delivery.hesitation);
}

/// **Somebody lonely is glad of the company**, which is a `needs`
/// question rather than a dialogue one.
#[test]
fn loneliness_changes_what_a_greeting_is_worth() {
    let mind = a_mind(19);
    let mem = Memory::new();
    let alone = ask(
        &mind,
        &mem,
        None,
        FunctionalState::Regulated,
        0.9,
        &Approach::a_friend(),
        Asked::Greeting,
        who(9),
        who(1),
    );
    let content = ask(
        &mind,
        &mem,
        None,
        FunctionalState::Regulated,
        0.0,
        &Approach::a_friend(),
        Asked::Greeting,
        who(9),
        who(1),
    );
    assert_ne!(alone.said, content.said);

    assert!(
        scale_sim::converse::worth_of_it(0.6, 0.9, 30.0)
            > scale_sim::converse::worth_of_it(0.6, 0.0, 30.0),
        "an hour with somebody was worth the same to a lonely man and a busy one"
    );
}

/// **A private man is shorter with a stranger**, which is character
/// rather than either distrust or a bad week.
#[test]
fn reticence_is_not_distrust() {
    let mut private = a_mind(23);
    private.person.set_baseline(Facet::Privacy, 2.0);
    let mut open = a_mind(23);
    open.person.set_baseline(Facet::Privacy, -2.0);
    let mem = Memory::new();

    let a = ask(
        &private,
        &mem,
        None,
        FunctionalState::Regulated,
        0.0,
        &Approach::default(),
        Asked::Greeting,
        who(9),
        who(1),
    );
    let b = ask(
        &open,
        &mem,
        None,
        FunctionalState::Regulated,
        0.0,
        &Approach::default(),
        Asked::Greeting,
        who(9),
        who(1),
    );
    assert!(a.act.delivery.warmth < b.act.delivery.warmth);
}

// =====================================================================
// the boundary
// =====================================================================

/// **The player gets no privileged channel.** What comes back is a
/// `SocialAct` — the same thing any listener gets — so it is read the
/// same way and can be misread the same way.
#[test]
fn there_is_no_special_line_to_the_player() {
    let (mind, mem, of) = a_witness(29, Source::Witnessed, 0.9);
    let a = ask(
        &mind,
        &mem,
        None,
        FunctionalState::Regulated,
        0.0,
        &Approach::a_friend(),
        Asked::About(of),
        who(9),
        who(1),
    );

    // A reader can pick it up, and reads it with their own head.
    let reader = a_mind(31);
    let cues = scale_sim::witness::Cues {
        words: true,
        prosody: true,
        expression: true,
        gesture: true,
    };
    let read = reader.read_act(&a.act.content, &a.act.delivery, &cues, Some((0.0, 0.2)));
    assert!(read.understood.is_some());
    assert!(
        !read.inferred.is_empty(),
        "the answer carried nothing to be read"
    );
}

/// The trace search is a search of *their* memory, not the world's.
#[test]
fn what_was_seen_looks_in_the_persons_own_record() {
    let (_, mem, of) = a_witness(37, Source::Witnessed, 0.9);
    assert!(what_was_seen(&mem, of).is_some());
    assert!(what_was_seen(&Memory::new(), of).is_none());
}

// =====================================================================
// how you came to be talking to them at all
// =====================================================================

/// **Walking up to a stranger going about their business is awkward**,
/// and it is awkward for reasons that are about neither person: no
/// standing reason to be talking, they were doing something else, and
/// the place is not one where this is done.
#[test]
fn approaching_a_stranger_in_the_street_is_awkward() {
    let cold = Approach::default();
    let known = Approach::a_friend();
    assert!(
        cold.awkwardness() > known.awkwardness() * 3.0,
        "stopping a stranger cost about what greeting a friend did"
    );

    let mind = a_mind(41);
    let mem = Memory::new();
    let say = |a: &Approach| {
        ask(
            &mind,
            &mem,
            None,
            FunctionalState::Regulated,
            0.0,
            a,
            Asked::Greeting,
            who(9),
            who(1),
        )
    };
    assert!(say(&cold).act.delivery.warmth < say(&known).act.delivery.warmth);
    assert_ne!(say(&cold).said, say(&known).said);
}

/// **A counter is a place for being approached**, which is most of what
/// a counter is. The same stranger gets a civil answer there and a short
/// one in the street — and the man has not changed at all.
#[test]
fn a_shopkeeper_is_civil_because_of_where_he_is_standing() {
    let street = Approach::default();
    let counter = Approach::at_a_counter();
    assert!(counter.awkwardness() < street.awkwardness());

    let mind = a_mind(43);
    let mem = Memory::new();
    let at = |a: &Approach| {
        ask(
            &mind,
            &mem,
            None,
            FunctionalState::Regulated,
            0.0,
            a,
            Asked::Greeting,
            who(9),
            who(1),
        )
        .act
        .delivery
        .warmth
    };
    assert!(
        at(&counter) > at(&street),
        "the setting made no difference to being stopped by a stranger"
    );
}

/// **Interrupting somebody costs more than catching them idle**, and it
/// is a smaller term than being a stranger.
#[test]
fn being_busy_is_a_real_cost_and_not_the_main_one() {
    let idle = Approach {
        they_are_busy: 0.0,
        ..Default::default()
    };
    let busy = Approach {
        they_are_busy: 1.0,
        ..Default::default()
    };
    assert!(busy.awkwardness() > idle.awkwardness());

    let stranger_idle = idle.awkwardness();
    let friend_busy = Approach {
        they_are_busy: 1.0,
        ..Approach::a_friend()
    }
    .awkwardness();
    assert!(
        stranger_idle > friend_busy,
        "being a stranger mattered less than being interrupted"
    );
}

/// **A private man feels an intrusion more.** The approach and the
/// person compound; neither alone is the story.
#[test]
fn reticence_and_intrusion_compound() {
    let mem = Memory::new();
    let mut private = a_mind(47);
    private.person.set_baseline(Facet::Privacy, 2.0);
    let mut open = a_mind(47);
    open.person.set_baseline(Facet::Privacy, -2.0);

    let cold = Approach::default();
    let warm = Approach::a_friend();
    let w = |m: &Mind, a: &Approach| {
        ask(
            m,
            &mem,
            None,
            FunctionalState::Regulated,
            0.0,
            a,
            Asked::Greeting,
            who(9),
            who(1),
        )
        .act
        .delivery
        .warmth
    };
    let private_cost = w(&private, &warm) - w(&private, &cold);
    let open_cost = w(&open, &warm) - w(&open, &cold);
    assert!(
        private_cost > open_cost,
        "an intrusion cost a private man no more than an open one: {private_cost:.2} vs {open_cost:.2}"
    );
}

/// **And it costs the person who was approached something**, which is
/// most of why people do not do it.
#[test]
fn being_stopped_by_a_stranger_costs_the_one_who_was_stopped() {
    let busy_stranger = Approach {
        they_are_busy: 0.9,
        ..Default::default()
    };
    assert!(scale_sim::converse::cost_to_them(&busy_stranger, 10.0) > 0.0);
    assert!(
        scale_sim::converse::cost_to_them(&busy_stranger, 10.0)
            > scale_sim::converse::cost_to_them(&Approach::a_friend(), 10.0),
        "stopping a busy stranger cost them what greeting a friend did"
    );
}

// =====================================================================
// most talking is not a conversation
// =====================================================================

/// **A held door is not a conversation**, and it is not awkward either.
///
/// The reason is the act. A stranger who has just done you a small
/// kindness has every reason to be spoken to; the same stranger stopped
/// in the street to be asked something has none.
#[test]
fn an_occasioned_exchange_is_not_awkward() {
    let stopped = Approach::default();
    let door = Approach::a_courtesy(Courtesy::HeldDoor);

    // Same stranger, same street, same busy man.
    assert_eq!(stopped.familiarity, door.familiarity);
    assert!(
        door.awkwardness() < stopped.awkwardness() * 0.35,
        "holding a door was nearly as awkward as stopping somebody: {:.2} vs {:.2}",
        door.awkwardness(),
        stopped.awkwardness()
    );

    let mind = a_mind(53);
    let mem = Memory::new();
    let w = |a: &Approach| {
        ask(
            &mind,
            &mem,
            None,
            FunctionalState::Regulated,
            0.0,
            a,
            Asked::Greeting,
            who(9),
            who(1),
        )
        .act
        .delivery
        .warmth
    };
    assert!(w(&door) > w(&stopped));
}

/// **And it is over.** Two turns, complete in itself, leading nowhere —
/// which is what stops a model having everybody chat all day.
#[test]
fn a_courtesy_is_two_turns_and_finished() {
    for c in [
        Courtesy::HeldDoor,
        Courtesy::GaveWay,
        Courtesy::ReturnedDropped,
        Courtesy::Helped,
    ] {
        let o = Occasion::Courtesy(c);
        assert_eq!(o.turns(), 2);
        assert!(o.is_complete_in_itself());
        let (thanks, reply) = courtesy_exchange(c);
        assert!(!thanks.is_empty() && !reply.is_empty());
        assert_ne!(thanks, reply);
    }
    // A deliberate approach is the one that has to carry its own reason,
    // and it is the longest.
    assert_eq!(Occasion::Deliberate.reason_to_speak(), 0.0);
    assert!(Occasion::Deliberate.turns() > Occasion::Courtesy(Courtesy::HeldDoor).turns());
    assert!(!Occasion::Deliberate.is_complete_in_itself());
}

/// **A transaction carries its own words too.** A shopkeeper being
/// spoken to is not being intruded upon.
#[test]
fn buying_something_is_a_reason_to_be_speaking() {
    assert!(Occasion::Transaction.reason_to_speak() > 0.9);
    assert!(Approach::at_a_counter().awkwardness() < 0.1);
}
