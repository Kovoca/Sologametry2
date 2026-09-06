//! **Slice 6's entry contract.** `docs/mind-spec.md`.
//!
//! The eighteen required tests. What they guard is that nothing crosses
//! from what a speaker *meant* into what a listener *got* except the
//! words and the manner — because **the danger here is telepathy
//! disguised as convenience**, and once only observable content and
//! delivery cues cross, praise, sarcasm, deception, failed jokes,
//! rejected apologies and honest misunderstanding all come out of one
//! pipeline without any of them being written as a case.

use scale_sim::id::{Arena, Id};
use scale_sim::memory::{EventKind, Memory, PerceivedWho, Source};
use scale_sim::mind::{Emotion, Facet, Mind, Reading, Value};
use scale_sim::needs::{Circumstances, Doing, Need};
use scale_sim::person::{Person, Trade};
use scale_sim::relations::{Aspect, Diagnosticity, Evidence, Relationship, TrustIn};
use scale_sim::rng::Rng;
use scale_sim::social::{
    anything_to_say, perform, social_skill, Apology, AssertedClaim, Channel, Content, Delivery,
    Exchange, Motive, Opportunity, SocialAct, SpeakerPlan, Strategy, Topic, WeightedMotive,
};
use scale_sim::witness::{Context, Cues};

fn a_village() -> (Arena<Person>, Vec<Id<Person>>) {
    let mut folk = Arena::new();
    let ids = ["Alice", "Bob", "Carol", "Dan"]
        .iter()
        .map(|n| folk.add(Person::new(*n, Trade::Labourer, 0, 100.0)))
        .collect();
    (folk, ids)
}

fn person(seed: u64, facets: &[(Facet, f32)]) -> Mind {
    let mut m = Mind::draw(&mut Rng::new(seed), &[(Value::Fairness, 25)]);
    for f in Facet::ALL {
        m.person.set_baseline(f, 0.0);
    }
    for &(f, z) in facets {
        m.person.set_baseline(f, z);
    }
    m.willpower = 0.0;
    m.empathy = 0.0;
    m
}

/// Everything got through: in the room, facing each other.
fn face_to_face() -> Cues {
    Cues { words: true, prosody: true, expression: true, gesture: true }
}

/// Every word, and no face.
fn from_behind() -> Cues {
    Cues { words: true, prosody: true, expression: false, gesture: false }
}

fn motives(list: &[(Motive, f64)]) -> Vec<WeightedMotive> {
    list.iter().map(|&(motive, weight)| WeightedMotive { motive, weight }).collect()
}

// --- 1 and 2 -----------------------------------------------------------

/// **Identical words with different private motives are observably
/// identical.**
///
/// A commander may sincerely admire a man *and* praise him publicly to
/// move the unit. If the act carried the motive, every listener would
/// know which — and there would be no flattery, no sarcasm and no
/// misunderstanding anywhere in the world.
#[test]
fn the_same_words_meant_differently_look_the_same() {
    let (_folk, id) = a_village();
    let mut rng = Rng::new(1);

    let sincere = SpeakerPlan {
        speaker: id[0],
        addressee: id[1],
        motives: motives(&[(Motive::Affection, 0.9), (Motive::Encouragement, 0.7)]),
        topic: Topic::TheirWork,
        strategy: Strategy::Encourage,
    };
    let calculating = SpeakerPlan {
        motives: motives(&[(Motive::StatusPerformance, 0.9), (Motive::Duty, 0.6)]),
        ..sincere.clone()
    };

    // Equal skill, so the delivery cues come out the same.
    let a = perform(&sincere, 1.0, false, &mut Rng::new(1));
    let b = perform(&calculating, 1.0, false, &mut Rng::new(1));

    assert_eq!(a.content, b.content, "the words differed by intention alone");
    assert_eq!(
        a.delivery.warmth, b.delivery.warmth,
        "the warmth in his voice reported his private reason"
    );
    assert_eq!(a.delivery.eagerness, b.delivery.eagerness);

    // **And there is no way to ask.** A `SocialAct` has no motives field
    // and no plan; the only route from one to the other is `perform`.
    let _: &SocialAct = &a;
    let _ = &mut rng;
}

/// **A listener works from what reached them and nothing else.**
///
/// `read_act` takes content, delivery and cues. It is not given the plan,
/// the speaker's mind, or the strategy — so it *cannot* consult them.
#[test]
fn a_listener_is_never_handed_the_plan() {
    let (_folk, id) = a_village();
    let listener = person(2, &[]);

    let flatterer = SpeakerPlan {
        speaker: id[0],
        addressee: id[1],
        motives: motives(&[(Motive::SeekingFavour, 0.9)]),
        topic: Topic::TheirWork,
        strategy: Strategy::Ingratiate,
    };
    // A *skilled* flatterer keeps it out of his voice entirely.
    let act = perform(&flatterer, 1.0, false, &mut Rng::new(2));
    assert_eq!(
        act.delivery.eagerness, 0.0,
        "a practised flatterer's angle showed in his voice"
    );

    let read = listener.read_act(&act.content, &act.delivery, &face_to_face(), Some((0.5, 0.5)));
    assert!(
        read.weight_of(Reading::SincerePraise) > read.weight_of(Reading::Flattery),
        "the listener saw straight through a man who gave nothing away"
    );
}

// --- 3, 4, 5, 6 --------------------------------------------------------

/// **Two listeners, one remark, two verdicts** — and sincere praise can
/// be taken for flattery.
#[test]
fn the_same_praise_lands_differently_on_different_people() {
    let (_folk, id) = a_village();
    let plan = SpeakerPlan {
        speaker: id[0],
        addressee: id[1],
        motives: motives(&[(Motive::Affection, 0.9)]),
        topic: Topic::TheirWork,
        strategy: Strategy::Encourage,
    };
    let act = perform(&plan, 0.9, true, &mut Rng::new(3));

    let trusting = person(3, &[(Facet::Trust, 2.0)]);
    let suspicious = person(3, &[(Facet::Trust, -2.0)]);

    let warm = trusting.read_act(&act.content, &act.delivery, &face_to_face(), Some((0.6, 0.6)));
    let cold = suspicious.read_act(&act.content, &act.delivery, &face_to_face(), Some((-0.5, 0.6)));

    assert!(
        warm.weight_of(Reading::SincerePraise) > cold.weight_of(Reading::SincerePraise),
        "a suspicious man took a kind word as warmly as a trusting one"
    );
    assert!(
        cold.weight_of(Reading::Flattery) > warm.weight_of(Reading::Flattery),
        "sincere praise cannot be mistaken for flattery"
    );
}

/// **Pleased and suspicious at the same time.**
///
/// Sincere praise and flattery are different informational signals and a
/// remark can plausibly be either. One enum on the listener's side would
/// have to choose, and the ordinary experience of being complimented by
/// somebody who wants something would be inexpressible.
#[test]
fn a_listener_can_be_pleased_and_suspicious_at_once() {
    let (_folk, id) = a_village();
    let plan = SpeakerPlan {
        speaker: id[0],
        addressee: id[1],
        motives: motives(&[(Motive::Affection, 0.6), (Motive::SeekingFavour, 0.7)]),
        topic: Topic::TheirWork,
        strategy: Strategy::Ingratiate,
    };
    // Clumsy enough that the angle shows.
    let act = perform(&plan, 0.35, false, &mut Rng::new(4));
    assert!(act.delivery.eagerness > 0.3, "his eagerness did not show at all");

    let m = person(4, &[]);
    let read = m.read_act(&act.content, &act.delivery, &face_to_face(), Some((0.3, 0.5)));
    assert!(read.weight_of(Reading::SincerePraise) > 0.15, "no pleasure at all");
    assert!(read.weight_of(Reading::Flattery) > 0.15, "no suspicion at all");

    let felt = m.appraise_reading(&read, 0.5);
    assert!(
        felt.iter().any(|e| matches!(e.what, Emotion::Joy | Emotion::Pride)),
        "being praised pleased him not at all: {felt:?}"
    );
    assert!(
        felt.iter().any(|e| e.what == Emotion::Anxiety),
        "a transparent angle made him not the least uneasy"
    );
}

/// **Deliberate mockery can be taken for friendly teasing**, by somebody
/// who trusts the speaker.
#[test]
fn mockery_can_be_mistaken_for_teasing() {
    let (_folk, id) = a_village();
    let cruel = SpeakerPlan {
        speaker: id[0],
        addressee: id[1],
        motives: motives(&[(Motive::Belittling, 0.9)]),
        topic: Topic::TheirWork,
        strategy: Strategy::Tease,
    };
    // Practised enough to keep the edge out of it.
    let act = perform(&cruel, 0.95, true, &mut Rng::new(5));

    let old_friend = person(5, &[(Facet::Trust, 1.5)]);
    let read = old_friend.read_act(&act.content, &act.delivery, &face_to_face(), Some((0.8, 0.9)));
    assert!(
        read.weight_of(Reading::FriendlyTeasing) > read.weight_of(Reading::Mockery),
        "a friend saw through a well-concealed sneer immediately"
    );
}

// --- 7 and 8 -----------------------------------------------------------

/// **Clumsy encouragement sounds like being patronised**, and skill does
/// not buy success.
#[test]
fn poor_execution_turns_encouragement_into_condescension() {
    let (_folk, id) = a_village();
    let plan = SpeakerPlan {
        speaker: id[0],
        addressee: id[1],
        motives: motives(&[(Motive::Encouragement, 0.9)]),
        topic: Topic::TheirWork,
        strategy: Strategy::Encourage,
    };
    let deft = perform(&plan, 0.95, false, &mut Rng::new(6));
    let clumsy = perform(&plan, 0.05, false, &mut Rng::new(6));

    let m = person(6, &[]);
    let good = m.read_act(&deft.content, &deft.delivery, &face_to_face(), Some((0.2, 0.4)));
    let bad = m.read_act(&clumsy.content, &clumsy.delivery, &face_to_face(), Some((0.2, 0.4)));

    assert!(
        bad.weight_of(Reading::Condescension) > good.weight_of(Reading::Condescension) + 0.1,
        "the same kind words said badly landed just as well"
    );
    assert!(
        good.weight_of(Reading::SincerePraise) > bad.weight_of(Reading::SincerePraise),
        "skill in the delivery bought nothing"
    );

    // **And skill still does not guarantee it.** The same deft words to
    // somebody who does not trust him.
    let mistrustful = person(6, &[(Facet::Trust, -2.0)]);
    let anyway = mistrustful.read_act(&deft.content, &deft.delivery, &face_to_face(), Some((-0.8, 0.7)));
    assert!(
        anyway.weight_of(Reading::Flattery) > 0.3,
        "a skilled speaker convinced a man who thinks him a liar"
    );
    assert!(social_skill(&person(6, &[(Facet::Gregariousness, 2.0)])) > social_skill(&m));
}

// --- 9 -----------------------------------------------------------------

/// **A mistake and a lie are different**, even when the words are the
/// same — and the listener gets the assertion, never the belief.
#[test]
fn a_mistake_and_a_lie_are_not_the_same_thing() {
    let (_folk, id) = a_village();
    let (speaker, listener) = (id[0], id[1]);

    // Sincerely wrong: he believes it and says it firmly.
    let mistaken = AssertedClaim::said(7, 0.95, 0.95);
    // Deliberately wrong: he doubts it and says it firmly anyway.
    let lie = AssertedClaim::said(7, 0.05, 0.95);

    assert!(!mistaken.was_deceptive(), "an honest error was filed as a lie");
    assert!(lie.was_deceptive(), "a flat lie was filed as an honest error");

    // **Observably identical**, which is the point.
    assert_eq!(mistaken.asserted, lie.asserted);
    assert_eq!(mistaken.proposition, lie.proposition);

    // And the listener cannot ask what was believed.
    assert_eq!(
        lie.privately_believed(listener, speaker),
        None,
        "the listener read the speaker's mind"
    );
    assert_eq!(lie.privately_believed(speaker, speaker), Some(0.05));

    // A hedged truth is not a lie either.
    let tactful = AssertedClaim::said(7, 0.9, 0.5);
    assert!(!tactful.was_deceptive());
}

// --- 10 and 14 ---------------------------------------------------------

/// **Gossip changes the listener's view of an absent third party — and
/// of the man telling it.**
///
/// And what is stored is the *telling*: accepted hearsay keeps the chain,
/// so a claim can be traced back and can turn out to be wrong.
#[test]
fn gossip_moves_two_relationships_and_keeps_its_chain() {
    let (_folk, id) = a_village();
    let (teller, absent) = (id[0], id[2]);
    let m = person(8, &[]);
    let mut mem = Memory::new();

    let heard = Memory::hear(
        EventKind::Theft,
        Some(PerceivedWho::Believed { person: absent, confidence: 0.6 }),
        scale_sim::memory::Place(1),
        20,
        Source::Told { by: teller },
    );
    let felt = m.appraise(&m.read(&heard.facts));
    let trace = mem.encode(heard, &m, 20, &felt).expect("a piece of gossip was not kept");

    // **The chain survives**, which is what makes it retractable.
    assert_eq!(mem.traces[trace].source(), Source::Told { by: teller });
    assert!(mem.traces[trace].is_hearsay());

    // The absent man is thought worse of...
    let mut on_absent = Relationship::strangers(id[1], absent);
    on_absent.saw(
        &Evidence {
            reliability: -0.8,
            reliability_in: Some(TrustIn::Money),
            reliability_about: Aspect::Integrity,
            // **Hearsay is weak evidence and must say so.** He did not
            // see it, so the quality is low.
            telling: Diagnosticity { quality: 0.35, ..Default::default() },
            ..Default::default()
        },
        20,
    );
    assert!(
        on_absent.trust_that(TrustIn::Money, Aspect::Integrity) < 0.0,
        "being told a man is a thief changed nothing"
    );

    // ...and the teller is judged for telling it.
    let mut on_teller = Relationship::strangers(id[1], teller);
    on_teller.saw(&Evidence { contact: 1.0, warmth: -0.2, ..Default::default() }, 20);
    assert!(on_teller.familiarity > 0.0);
    assert!(
        on_teller.affection() < 0.0,
        "carrying a story about a neighbour cost the teller nothing"
    );
}

// --- 11 and 12 ---------------------------------------------------------

/// **Saying sorry does not close a grievance.**
///
/// An apology is an observable act. Whether it repairs anything is the
/// listener's decision, and it turns on whether the responsibility is
/// believed. A shrug settles nothing.
#[test]
fn an_apology_is_performed_and_the_listener_decides() {
    let (_folk, id) = a_village();
    let mut wronged = Relationship::strangers(id[1], id[0]);
    wronged.saw(&Evidence { wrong: 0.8, ..Default::default() }, 10);
    let sore = wronged.resentment();
    assert!(sore > 0.5);

    let m = person(9, &[]);

    // A mumbled non-apology.
    let shrug = Content::Apology(Apology {
        acknowledgement: 0.4,
        responsibility: 0.1,
        remorse: 0.1,
        ..Default::default()
    });
    let read = m.read_act(&shrug, &Delivery { warmth: 0.1, ..Default::default() }, &face_to_face(), Some((-0.2, 0.5)));
    let credited = read.weight_of(Reading::AnApology);
    let mut after_a_shrug = wronged.clone();
    after_a_shrug.apologised(credited, 0.1, 0.0);
    assert!(
        after_a_shrug.resentment() > 0.4,
        "a shrug and a sorry settled it ({:.2})",
        after_a_shrug.resentment()
    );

    // A real one, taken as real.
    let proper = Content::Apology(Apology {
        acknowledgement: 0.95,
        responsibility: 0.95,
        remorse: 0.9,
        repair: 0.8,
        promise: 0.8,
        explanation: 0.3,
    });
    let read = m.read_act(&proper, &Delivery { warmth: 0.7, ..Default::default() }, &face_to_face(), Some((0.2, 0.6)));
    assert!(read.weight_of(Reading::AnApology) > 0.5, "a full apology read as hollow");
    let mut after_a_real_one = wronged.clone();
    after_a_real_one.apologised(read.weight_of(Reading::AnApology), 0.95, 0.8);
    assert!(
        after_a_real_one.resentment() < 0.25,
        "a full and credited apology settled nothing"
    );
}

/// **An accepted apology does not restore domain trust.**
///
/// Slice 5's rule stands: trust needs evidence of trustworthy behaviour,
/// and there has not been any.
#[test]
fn being_forgiven_is_not_being_trusted_again() {
    let (_folk, id) = a_village();
    let mut r = Relationship::strangers(id[1], id[0]);
    for d in 0..100 {
        r.saw(
            &Evidence {
                contact: 1.0,
                reliability: 0.8,
                reliability_in: Some(TrustIn::Money),
                reliability_about: Aspect::Integrity,
                telling: Diagnosticity::default(),
                ..Default::default()
            },
            d,
        );
    }
    r.saw(
        &Evidence {
            contact: 1.0,
            reliability: -1.0,
            reliability_in: Some(TrustIn::Money),
            reliability_about: Aspect::Integrity,
            telling: Diagnosticity::default(),
            wrong: 0.9,
            ..Default::default()
        },
        101,
    );
    let robbed_of_trust = r.trust_that(TrustIn::Money, Aspect::Integrity);
    assert!(robbed_of_trust < 0.3);

    r.apologised(0.95, 0.95, 0.9);
    assert!(r.resentment() < 0.2, "a full apology was not credited");
    assert_eq!(
        r.trust_that(TrustIn::Money, Aspect::Integrity),
        robbed_of_trust,
        "saying sorry made him safe with the takings again"
    );
}

// --- 13 ----------------------------------------------------------------

/// **Public and private are different acts**, and every listener forms
/// their own view.
///
/// One speech can produce gratitude in the man addressed, envy in his
/// rival, and suspicion in somebody who has seen it done before. The
/// addressee's reading must never be applied to the audience.
#[test]
fn an_audience_reads_it_for_themselves() {
    let (_folk, id) = a_village();
    let plan = SpeakerPlan {
        speaker: id[0],
        addressee: id[1],
        motives: motives(&[(Motive::Affection, 0.6), (Motive::StatusPerformance, 0.8)]),
        topic: Topic::TheirWork,
        strategy: Strategy::Encourage,
    };
    let public = perform(&plan, 0.6, true, &mut Rng::new(10));
    assert!(public.delivery.publicly, "a speech for the room was made in private");

    let addressed = person(10, &[(Facet::Trust, 1.0)]);
    let rival = person(11, &[(Facet::Envy, 2.0), (Facet::Trust, -1.0)]);
    let veteran = person(12, &[(Facet::Trust, -1.5)]);

    let a = addressed.read_act(&public.content, &public.delivery, &face_to_face(), Some((0.6, 0.7)));
    let b = rival.read_act(&public.content, &public.delivery, &face_to_face(), Some((-0.3, 0.5)));
    let c = veteran.read_act(&public.content, &public.delivery, &face_to_face(), Some((-0.4, 0.9)));

    assert!(a.weight_of(Reading::SincerePraise) > b.weight_of(Reading::SincerePraise));
    assert!(
        c.weight_of(Reading::Flattery) > a.weight_of(Reading::Flattery),
        "an old hand read the speech exactly as the man being praised did"
    );
    // Three separate readings, not one applied three times.
    assert!(a != b && b != c, "the audience inherited the addressee's verdict");
}

// --- 15 ----------------------------------------------------------------

/// **The speaker learns how it landed only by perceiving the response.**
///
/// A joke told to somebody who then turns away unheard is a joke the
/// teller thinks worked.
#[test]
fn a_speaker_does_not_know_whether_it_landed() {
    let (_folk, id) = a_village();
    let teller = person(13, &[]);
    let plan = SpeakerPlan {
        speaker: id[0],
        addressee: id[1],
        motives: motives(&[(Motive::Affection, 0.8)]),
        topic: Topic::TheirWork,
        strategy: Strategy::Tease,
    };
    let joke = perform(&plan, 0.8, false, &mut Rng::new(13));

    // It lands badly: the listener could not see the grin.
    let listener = person(14, &[(Facet::Trust, -1.0)]);
    let read = listener.read_act(&joke.content, &joke.delivery, &from_behind(), Some((0.0, 0.2)));
    assert!(
        read.weight_of(Reading::Mockery) > read.weight_of(Reading::FriendlyTeasing),
        "without the grin it still read as friendly"
    );

    // The listener says something back, coldly.
    let reply = Content::Remark { about: Topic::Themselves };
    let cold = Delivery { warmth: -0.6, edge: 0.5, ..Default::default() };

    // The teller is looking away and gets nothing.
    let nothing = Cues::default();
    let missed = teller.read_act(&reply, &cold, &nothing, Some((0.2, 0.3)));
    assert!(missed.understood.is_none(), "he heard a reply he could not hear");
    assert!(missed.confidence < 0.4, "he was sure of a response he never got");

    // Turning round, he finds out.
    let noticed = teller.read_act(&reply, &cold, &face_to_face(), Some((0.2, 0.3)));
    assert!(noticed.understood.is_some());
    assert!(
        noticed.stance < missed.stance,
        "seeing the man's face told him nothing about how it went"
    );
}

// --- 16 and 17 ---------------------------------------------------------

/// **Silence is a valid result.** An opportunity is not an obligation.
#[test]
fn nobody_has_to_say_anything() {
    let (_folk, id) = a_village();
    let here = Opportunity {
        with: vec![id[1]],
        audience: vec![],
        channel: Channel::FaceToFace,
        context: Context::Workplace(1),
        minutes: 30.0,
    };

    // Somebody solitary who has had plenty of company today.
    let mut content = person(15, &[(Facet::Gregariousness, -2.0)]);
    if let Some(n) = content.needs.as_mut() {
        n.did(
            Doing::TalkWithAFriend,
            4.0,
            Circumstances { with_somebody_known: true, ..Default::default() },
        );
    }
    assert!(
        !anything_to_say(&content, &here),
        "a private man with nothing to say struck up a conversation"
    );

    // Somebody starved of company will.
    let mut lonely = person(15, &[(Facet::Gregariousness, 2.0)]);
    for _ in 0..30 {
        if let Some(n) = lonely.needs.as_mut() {
            n.a_day_passes();
        }
    }
    assert!(anything_to_say(&lonely, &here), "a month alone and he says nothing");

    // No time is no conversation, whoever it is.
    let passing = Opportunity { minutes: 0.2, ..here };
    assert!(!anything_to_say(&lonely, &passing));
}

/// **An exchange has a budget**, so a conversation ends.
#[test]
fn a_conversation_runs_out_of_time_and_of_turns() {
    let (_folk, id) = a_village();
    let o = Opportunity {
        with: vec![id[1]],
        audience: vec![],
        channel: Channel::FaceToFace,
        context: Context::Tavern(1),
        minutes: 10.0,
    };
    let mut x = Exchange::opening(&o);
    let plan = SpeakerPlan {
        speaker: id[0],
        addressee: id[1],
        motives: motives(&[(Motive::Curiosity, 0.5)]),
        topic: Topic::Weather,
        strategy: Strategy::Disclose,
    };

    let mut said = 0;
    while !x.over() {
        let act = perform(&plan, 0.5, false, &mut Rng::new(16));
        if !x.take_a_turn(act, 1.0) {
            break;
        }
        said += 1;
    }
    assert!(said > 0 && said <= 8, "a ten-minute chat ran to {said} turns");
    assert!(x.over(), "the conversation never ended");

    // A minute in a doorway is not a conversation.
    let brief = Opportunity { minutes: 0.5, ..o };
    let mut short = Exchange::opening(&brief);
    let act = perform(&plan, 0.5, false, &mut Rng::new(16));
    assert!(!short.take_a_turn(act, 1.0), "a full exchange fitted into thirty seconds");
}

// --- 18 ----------------------------------------------------------------

/// **Social satisfaction follows the interpreted quality and the
/// relationship — not the number of things said.**
///
/// The join back to slice 4. An hour of being needled by somebody you
/// half trust is not an hour of friendship, however many words were in
/// it.
#[test]
fn talking_a_lot_is_not_the_same_as_good_company() {
    let (_folk, id) = a_village();
    let m = person(17, &[]);

    let mut close = Relationship::strangers(id[0], id[1]);
    for d in 0..60 {
        close.saw(&Evidence { contact: 1.0, warmth: 0.7, ..Default::default() }, d);
    }
    let mut prickly = Relationship::strangers(id[0], id[2]);
    for d in 0..60 {
        prickly.saw(&Evidence { contact: 1.0, warmth: -0.3, ..Default::default() }, d);
    }

    let warm_words = Content::Praise { about: Topic::TheirWork, strength: 0.7 };
    let good = m.read_act(
        &warm_words,
        &Delivery { warmth: 0.8, smiling: true, ..Default::default() },
        &face_to_face(),
        Some((0.7, 0.8)),
    );
    let barbed = Content::Barb { at: Topic::TheirWork, sharpness: 0.6 };
    let bad = m.read_act(
        &barbed,
        &Delivery { warmth: -0.2, edge: 0.4, ..Default::default() },
        &from_behind(),
        Some((-0.2, 0.5)),
    );

    let worth = |reading: &scale_sim::mind::ListenerReading, r: &Relationship| {
        scale_sim::relations::worth_of_company(r, reading.sincerity)
    };
    assert!(
        worth(&good, &close) > worth(&bad, &prickly) * 3.0,
        "an hour of needling from a man he half trusts was as good as an \
         hour with a friend"
    );

    // And the satisfaction that reaches the need follows that, not the
    // count of remarks.
    let mut needs = m.needs.clone().unwrap();
    for _ in 0..40 {
        needs.a_day_passes();
    }
    let got = needs.did(
        Doing::TalkWithAFriend,
        worth(&good, &close),
        Circumstances { with_somebody_known: true, ..Default::default() },
    );
    assert!(
        got.iter().any(|(n, _)| *n == Need::Friendship),
        "an hour with a close friend satisfied nothing"
    );
}

/// A seed says the same thing twice.
#[test]
fn the_same_speaker_performs_the_same_act() {
    let (_folk, id) = a_village();
    let plan = SpeakerPlan {
        speaker: id[0],
        addressee: id[1],
        motives: motives(&[(Motive::Affection, 0.5)]),
        topic: Topic::TheirWork,
        strategy: Strategy::Console,
    };
    let a = perform(&plan, 0.5, false, &mut Rng::new(99));
    let b = perform(&plan, 0.5, false, &mut Rng::new(99));
    assert_eq!(a, b);
}

// ---------------------------------------------------------------------
// what the compile_fail doctests are worth
// ---------------------------------------------------------------------

/// **The observable half of every sealed pair is reachable.**
///
/// A `compile_fail` doctest passes if the snippet fails for *any* reason,
/// including a typo, so on its own it proves very little. This is the
/// other half: for each thing `src/social.rs` proves unreachable, reach
/// the sibling beside it on the same type by the same path. If these
/// compile and those do not, the boundary is what is stopping them and
/// not a mistake in the test.
#[test]
fn the_observable_half_is_reachable() {
    let (_folk, id) = a_village();
    let plan = SpeakerPlan {
        speaker: id[0],
        addressee: id[1],
        motives: motives(&[(Motive::Deception, 0.9)]),
        topic: Topic::TheirWork,
        strategy: Strategy::Persuade,
    };
    let act = perform(&plan, 0.6, true, &mut Rng::new(77));

    // `act.motives` and `act.strategy` are proved unreachable; these are.
    let _: &Delivery = &act.delivery;
    let _: Topic = act.topic;
    let _: Content = act.content;

    // `claim.believed` is proved unreachable; `asserted` is the public
    // half of the same struct, and the private half is readable only
    // through the accessor that checks who is asking.
    if let Content::Claim(c) = act.content {
        let _: f64 = c.asserted;
        assert!(
            c.privately_believed(plan.speaker, plan.speaker).is_some(),
            "a speaker could not consult their own belief"
        );
        assert!(
            c.privately_believed(id[1], plan.speaker).is_none(),
            "the listener read what the speaker privately believed"
        );
    }

    // `ListenerReading` cannot be built here; it can be obtained from the
    // mind that did the reading, and read freely once it exists.
    let m = person(77, &[]);
    let r = m.read_act(&act.content, &act.delivery, &face_to_face(), Some((0.0, 0.5)));
    let _: f64 = r.confidence;
    assert!(!r.inferred.is_empty(), "a reading with nothing in it proves nothing");
}
