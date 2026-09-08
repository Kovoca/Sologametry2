//! **Slice 5's entry contract.** `docs/mind-spec.md`.
//!
//! The fourteen required tests. What they guard is that a relationship is
//! a **directed, compressed belief** — not an objective fact, not a
//! current emotion, and not a substitute for memory.

use scale_sim::id::{Arena, Id};
use scale_sim::memory::{Description, EventKind, Memory, PerceivedWho, Place, Source, WorldEvent};
use scale_sim::mind::{Emotion, Facet, Happening, Mind, Value};
use scale_sim::person::{Person, Trade};
use scale_sim::relations::{
    labels, worth_of_company, Aspect, Diagnosticity, Evidence, Label, Relationship, RespectFor,
    SocialFact, TrustIn,
};
use scale_sim::rng::Rng;

/// A world with people in it, so relationships have real endpoints.
fn a_village() -> (Arena<Person>, Vec<Id<Person>>) {
    let mut folk = Arena::new();
    let ids: Vec<Id<Person>> = ["Alice", "Bob", "Carol", "Dan"]
        .iter()
        .map(|n| folk.add(Person::new(*n, Trade::Labourer, 0, 100.0)))
        .collect();
    (folk, ids)
}

fn a_mind(seed: u64) -> Mind {
    let mut m = Mind::draw(&mut Rng::new(seed), &[(Value::Fairness, 25)]);
    for f in Facet::ALL {
        m.person.set_baseline(f, 0.0);
    }
    m
}

fn kindly() -> Evidence {
    Evidence {
        contact: 1.0,
        warmth: 0.6,
        ..Default::default()
    }
}

/// **A day on which he could have done otherwise and did not.** Anything
/// less shows nothing: two hundred uneventful days are not two hundred
/// observed honesty opportunities.
fn shown(reliability: f64, about: Aspect, domain: TrustIn) -> Evidence {
    Evidence {
        contact: 1.0,
        reliability,
        reliability_in: Some(domain),
        reliability_about: about,
        telling: Diagnosticity::default(),
        ..Default::default()
    }
}

// --- 1 -----------------------------------------------------------------

/// **Alice trusts Bob while Bob distrusts Alice.**
///
/// Two records, and they need not agree about anything. A single symmetric
/// edge cannot express the commonest fact about people: that regard is
/// rarely returned in equal measure.
#[test]
fn a_relationship_is_directed() {
    let (_folk, id) = a_village();
    let (alice, bob) = (id[0], id[1]);

    let mut alice_on_bob = Relationship::strangers(alice, bob);
    let mut bob_on_alice = Relationship::strangers(bob, alice);

    for d in 0..30 {
        alice_on_bob.saw(
            &Evidence {
                contact: 1.0,
                reliability: 0.8,
                telling: Diagnosticity::default(),
                ..Default::default()
            },
            d,
        );
        bob_on_alice.saw(
            &Evidence {
                contact: 1.0,
                reliability: -0.6,
                telling: Diagnosticity::default(),
                ..Default::default()
            },
            d,
        );
    }

    assert!(
        alice_on_bob.trust_in(TrustIn::General) > 0.4,
        "Alice does not trust Bob"
    );
    assert!(
        bob_on_alice.trust_in(TrustIn::General) < -0.3,
        "Bob trusts Alice"
    );
    assert_eq!(alice_on_bob.subject, alice);
    assert_eq!(alice_on_bob.object, bob);
}

// --- 2 and 3 -----------------------------------------------------------

/// **Knowing somebody well is not liking them**, and liking them is not
/// relying on them.
///
/// The two collapses that flatten a model. A colleague of ten years can be
/// perfectly familiar and no more than tolerated; a charming brother-in-law
/// can be loved and never lent money.
#[test]
fn familiarity_affection_and_trust_are_three_different_things() {
    let (_folk, id) = a_village();

    // A man he sees every day and does not warm to.
    let mut colleague = Relationship::strangers(id[0], id[1]);
    for d in 0..200 {
        colleague.saw(
            &Evidence {
                contact: 1.0,
                ..Default::default()
            },
            d,
        );
    }
    assert!(
        colleague.familiarity > 0.9,
        "two hundred days and still a stranger"
    );
    assert!(
        colleague.affection().abs() < 0.05,
        "seeing somebody daily made him fond of them ({:.2})",
        colleague.affection()
    );

    // A brother-in-law he is fond of and would not lend a penny.
    let mut charmer = Relationship::strangers(id[0], id[2]);
    for d in 0..60 {
        charmer.saw(&kindly(), d);
        charmer.saw(
            &Evidence {
                contact: 1.0,
                reliability: -0.5,
                reliability_in: Some(TrustIn::Money),
                telling: Diagnosticity::default(),
                ..Default::default()
            },
            d,
        );
    }
    assert!(charmer.affection() > 0.5, "he is not fond of the charmer");
    assert!(
        charmer.trust_in(TrustIn::Money) < -0.3,
        "he would lend the charmer money"
    );

    // **And trust has domains.** Brave beside you and hopeless with a
    // secret is one person, not a contradiction.
    let mut comrade = Relationship::strangers(id[0], id[3]);
    for d in 0..40 {
        comrade.saw(
            &Evidence {
                contact: 1.0,
                reliability: 0.9,
                reliability_in: Some(TrustIn::Danger),
                telling: Diagnosticity::default(),
                ..Default::default()
            },
            d,
        );
        comrade.saw(
            &Evidence {
                contact: 0.0,
                reliability: -0.8,
                reliability_in: Some(TrustIn::Secrets),
                telling: Diagnosticity::default(),
                ..Default::default()
            },
            d,
        );
    }
    assert!(
        comrade.trust_in(TrustIn::Danger) > 0.5,
        "he would not stand beside him"
    );
    assert!(
        comrade.trust_in(TrustIn::Secrets) < -0.4,
        "he would tell him anything"
    );
}

// --- 4 -----------------------------------------------------------------

/// **A respected enemy.**
///
/// Feared, disliked, and admired for being good at it. One "opinion"
/// number cannot say that, and it is an entirely ordinary thing to feel.
#[test]
fn an_enemy_can_be_respected_and_still_feared() {
    let (_folk, id) = a_village();
    let mut r = Relationship::strangers(id[0], id[1]);
    for d in 0..40 {
        r.saw(
            &Evidence {
                contact: 1.0,
                warmth: -0.5,
                menace: 0.7,
                esteem: 0.8,
                esteem_for: Some(RespectFor::Competence),
                ..Default::default()
            },
            d,
        );
    }
    r.saw(
        &Evidence {
            wrong: 0.8,
            ..Default::default()
        },
        41,
    );

    assert!(r.affection() < -0.3, "he likes his enemy");
    assert!(r.fear() > 0.4, "he does not fear a dangerous man");
    assert!(
        r.respect_for(RespectFor::Competence) > 0.4,
        "he cannot admit the man is good at it"
    );

    let l = labels(&r, &[]);
    assert!(l.contains(&Label::Feared));
    assert!(
        l.contains(&Label::Respected),
        "a respected enemy is not respected: {l:?}"
    );
    assert!(l.contains(&Label::Enemy));
}

// --- 5 -----------------------------------------------------------------

/// **Objective kinship survives hatred.**
///
/// A hated parent is still a parent. Keeping the fact out of the
/// dimensions is what makes an estrangement expressible at all.
#[test]
fn a_hated_parent_is_still_a_parent() {
    let (_folk, id) = a_village();
    let mut r = Relationship::strangers(id[0], id[1]);
    for d in 0..50 {
        r.saw(
            &Evidence {
                contact: 1.0,
                warmth: -0.9,
                menace: 0.4,
                ..Default::default()
            },
            d,
        );
    }
    r.saw(
        &Evidence {
            wrong: 0.9,
            ..Default::default()
        },
        51,
    );

    assert!(r.affection() < -0.5);
    assert!(r.resentment() > 0.5);

    let l = labels(&r, &[SocialFact::ParentOf]);
    assert!(
        l.contains(&Label::Kin),
        "hating his father stopped the man being his father: {l:?}"
    );
    assert!(l.contains(&Label::Grudge));

    // And a trusted subordinate is the same shape the other way up.
    let mut sub = Relationship::strangers(id[0], id[2]);
    for d in 0..40 {
        sub.saw(
            &Evidence {
                contact: 1.0,
                warmth: 0.5,
                reliability: 0.9,
                ..Default::default()
            },
            d,
        );
    }
    let l = labels(&sub, &[SocialFact::Commands]);
    assert!(l.contains(&Label::Commander) && l.contains(&Label::Friend));
}

// --- 6 -----------------------------------------------------------------

/// **Fear in the relationship generates episodes and is not one.**
///
/// Storing the emotion would mean being continuously terrified of a man
/// you have not seen for a year — the same mistake slice 1 corrected with
/// concerns, in a different place.
#[test]
fn a_disposition_produces_a_feeling_and_is_not_one() {
    let (_folk, id) = a_village();
    let mut mind = a_mind(6);
    let mut r = Relationship::strangers(id[0], id[1]);
    for d in 0..30 {
        r.saw(
            &Evidence {
                contact: 1.0,
                menace: 0.8,
                ..Default::default()
            },
            d,
        );
    }
    r.saw(
        &Evidence {
            wrong: 0.7,
            ..Default::default()
        },
        31,
    );

    // Not in the room: not afraid.
    assert!(
        mind.feeling_of(Emotion::Fear) < 0.01,
        "he is afraid of a man who is not there"
    );
    assert!(r.fear() > 0.4, "and yet he is not wary of him at all");

    // In the room: afraid.
    let felt = r.on_meeting(&mind, true);
    assert!(
        felt.iter().any(|e| e.what == Emotion::Fear),
        "meeting a man he fears frightened him not at all"
    );
    assert!(
        felt.iter().any(|e| e.what == Emotion::Anger),
        "and no old anger either"
    );
    mind.feel(felt);
    assert!(mind.feeling_of(Emotion::Fear) > 0.2);

    // And it fades, while the disposition does not.
    let mut rng = Rng::new(6);
    for _ in 0..20 {
        mind.a_day_passes(&mut rng);
    }
    assert!(
        mind.feeling_of(Emotion::Fear) < 0.05,
        "still in a panic three weeks later"
    );
    assert!(r.fear() > 0.4, "the wariness evaporated with the fright");
}

// --- 7 -----------------------------------------------------------------

/// **The same interaction changes each participant differently.**
#[test]
fn one_exchange_two_different_conclusions() {
    let (_folk, id) = a_village();
    let mut giver = Relationship::strangers(id[0], id[1]);
    let mut taker = Relationship::strangers(id[1], id[0]);

    // He does her a favour at some cost. She is grateful; he is not.
    giver.saw(
        &Evidence {
            contact: 1.0,
            warmth: 0.3,
            ..Default::default()
        },
        1,
    );
    taker.saw(
        &Evidence {
            contact: 1.0,
            warmth: 0.3,
            kindness: 0.9,
            owing: 0.5,
            ..Default::default()
        },
        1,
    );

    assert!(taker.gratitude() > 0.1, "she is not grateful");
    assert_eq!(
        giver.gratitude(),
        0.0,
        "he is grateful to her for accepting"
    );
    assert!(taker.obligation > 0.3, "she owes him nothing");
    assert_eq!(giver.obligation, 0.0);
    assert!(labels(&taker, &[]).contains(&Label::Beholden));
    assert!(!labels(&giver, &[]).contains(&Label::Beholden));
}

// --- 8 and 9 -----------------------------------------------------------

/// **False blame damages the wrong relationship, and correcting it
/// changes the attribution without rewriting the memory.**
///
/// The whole reason the mind holds a `PerceivedWho` rather than the
/// world's handle. Alice did it; Bob sincerely remembers Carol; Bob's
/// resentment lands on Carol, and it is entirely real.
#[test]
fn resentment_follows_the_belief_and_not_the_truth() {
    let (_folk, id) = a_village();
    let (alice, _bob, carol) = (id[0], id[1], id[2]);
    let mind = a_mind(8);
    let mut mem = Memory::new();
    let mut rng = Rng::new(8);

    // Alice did it. Bob only half saw.
    let ev = WorldEvent {
        kind: EventKind::Theft,
        who: vec![alice],
        actor: Some(alice),
        place: Place(1),
        day: 10,
        severity: -0.7,
        facts: Happening {
            severity: -0.7,
            to_me: 0.9,
            deliberate: true,
            by_a_decision: false,
            control: 0.1,
            unexpected: 0.8,
            ..Default::default()
        },
    };
    let p = mem
        .perceive(&ev, None, Source::Witnessed, 0.62, &mut rng)
        .unwrap();
    let felt = mind.appraise(&mind.read(&p.facts));
    let trace = mem.encode(p, &mind, 10, &felt).unwrap();

    // He is not certain — which is what makes him correctable.
    let believed = mem.traces[trace]
        .what_was_perceived()
        .believed_actor
        .clone();
    assert!(
        matches!(believed, Some(PerceivedWho::Believed { .. })),
        "a poor look produced a certain identification: {believed:?}"
    );

    // He decides it was Carol, and resents Carol for it.
    mem.reattribute(
        trace,
        PerceivedWho::Believed {
            person: carol,
            confidence: 0.7,
        },
        0.7,
    );
    let mut on_carol = Relationship::strangers(id[1], carol);
    let on_alice = Relationship::strangers(id[1], alice);
    on_carol.saw(
        &Evidence {
            wrong: 0.7,
            ..Default::default()
        },
        11,
    );

    assert!(
        on_carol.resentment() > 0.5,
        "he does not resent the woman he blames"
    );
    assert_eq!(
        on_alice.resentment(),
        0.0,
        "he resents the woman he never suspected"
    );

    // Cleared. The grievance closes; what he saw does not change.
    let seen_before = mem.traces[trace].what_was_perceived().clone();
    on_carol.cleared();
    mem.reattribute(
        trace,
        PerceivedWho::Unknown(Description("somebody".into())),
        0.2,
    );

    assert_eq!(
        on_carol.resentment(),
        0.0,
        "clearing her left the grudge standing"
    );
    assert_eq!(
        mem.traces[trace].what_was_perceived().believed_actor,
        seen_before.believed_actor,
        "being corrected rewrote what he originally saw"
    );
    assert_eq!(
        mem.traces[trace].source(),
        Source::Witnessed,
        "the provenance changed with the attribution"
    );
    // **Being cleared is not being liked again.** Affection and trust
    // have their own histories and are untouched.
    assert_eq!(on_carol.affection(), 0.0);
    assert_eq!(on_carol.trust_in(TrustIn::General), 0.0);
}

// --- 10 ----------------------------------------------------------------

/// **A dead person remains a valid relationship target.**
///
/// You go on loving, fearing, resenting and owing the dead. The handle
/// must therefore outlive the person, which is what the arena's
/// generation counter is *for* — the slot is never reused as somebody
/// else.
#[test]
fn the_dead_can_still_be_loved_and_resented() {
    let (mut folk, id) = a_village();
    let (mourner, gone) = (id[0], id[1]);

    let mut r = Relationship::strangers(mourner, gone);
    for d in 0..80 {
        r.saw(
            &Evidence {
                contact: 1.0,
                warmth: 0.8,
                kindness: 0.4,
                ..Default::default()
            },
            d,
        );
    }
    r.saw(
        &Evidence {
            wrong: 0.5,
            ..Default::default()
        },
        81,
    );
    let loved = r.affection();

    folk.remove(gone);
    assert!(!folk.holds(gone), "he did not die");

    // The relationship is untouched and still points where it pointed.
    assert_eq!(r.object, gone);
    assert_eq!(r.affection(), loved, "his feeling for her died with her");
    assert!(r.gratitude() > 0.0 && r.resentment() > 0.0);
    assert!(labels(&r, &[SocialFact::MarriedTo]).contains(&Label::Spouse));

    // And nobody else can become her.
    let newcomer = folk.add(Person::new("Erin", Trade::Labourer, 0, 100.0));
    assert_ne!(
        newcomer, gone,
        "somebody moved into the dead woman's identity"
    );
}

// --- 11 ----------------------------------------------------------------

/// **Labels coexist, and go away when the dimensions do.**
///
/// `friend = true` is never stored. A man can be a friend, a rival, a
/// creditor and feared at once — and stop being a friend without any of
/// the rest changing.
#[test]
fn labels_are_derived_plural_and_impermanent() {
    let (_folk, id) = a_village();
    let mut r = Relationship::strangers(id[0], id[1]);
    for d in 0..60 {
        r.saw(
            &Evidence {
                contact: 1.0,
                warmth: 0.7,
                reliability: 0.8,
                telling: Diagnosticity::default(),
                esteem: 0.7,
                esteem_for: Some(RespectFor::Competence),
                owing: -0.02,
                ..Default::default()
            },
            d,
        );
    }
    let l = labels(&r, &[SocialFact::EmployedBy]);
    assert!(
        l.contains(&Label::Friend),
        "sixty days of warmth is not a friendship: {l:?}"
    );
    assert!(l.contains(&Label::Respected));
    assert!(
        l.contains(&Label::Creditor),
        "the debt is not recorded: {l:?}"
    );
    assert!(l.contains(&Label::Employer));
    assert!(l.len() >= 4, "one relationship got one name: {l:?}");

    // A falling-out. The friendship goes; the debt and the employment do
    // not.
    for d in 60..90 {
        r.saw(
            &Evidence {
                contact: 1.0,
                warmth: -0.9,
                ..Default::default()
            },
            d,
        );
    }
    let after = labels(&r, &[SocialFact::EmployedBy]);
    assert!(
        !after.contains(&Label::Friend),
        "they fell out and are still friends"
    );
    assert!(
        after.contains(&Label::Creditor),
        "the debt was forgiven by falling out"
    );
    assert!(
        after.contains(&Label::Employer),
        "he stopped being employed by falling out"
    );
}

// --- 12 ----------------------------------------------------------------

/// **Friendship satisfaction depends on the relationship, not on
/// co-presence.**
///
/// The join with slice 4. `SomebodyKnown` is a question about the
/// *relationship*, not about the room — an hour with somebody you resent
/// is worth nothing.
#[test]
fn an_hour_with_somebody_is_worth_what_the_relationship_is_worth() {
    let (_folk, id) = a_village();

    let mut close = Relationship::strangers(id[0], id[1]);
    for d in 0..60 {
        close.saw(&kindly(), d);
    }
    let mut barely = Relationship::strangers(id[0], id[2]);
    for d in 0..6 {
        barely.saw(
            &Evidence {
                contact: 0.5,
                ..Default::default()
            },
            d,
        );
    }
    let mut sore = Relationship::strangers(id[0], id[3]);
    for d in 0..60 {
        sore.saw(&kindly(), d);
    }
    sore.saw(
        &Evidence {
            wrong: 0.9,
            ..Default::default()
        },
        61,
    );

    let good = worth_of_company(&close, 1.0);
    let thin = worth_of_company(&barely, 1.0);
    let bad = worth_of_company(&sore, 1.0);

    assert!(
        good > 0.4,
        "an hour with a close friend was worth {good:.2}"
    );
    assert!(
        thin < good / 2.0,
        "an hour with somebody he barely knows was as good"
    );
    assert!(
        bad < good,
        "an evening with a man who wronged him was as good as one with a \
         friend"
    );
}

// --- 13 ----------------------------------------------------------------

/// **A hundred trivial kindnesses do not add up to devotion.**
///
/// Saturation, or affection is farmable and everybody in the world ends
/// up adoring whoever passed them the salt most often.
#[test]
fn small_kindnesses_saturate_rather_than_accumulate() {
    let (_folk, id) = a_village();
    let mut r = Relationship::strangers(id[0], id[1]);
    let trivial = Evidence {
        contact: 0.3,
        warmth: 0.1,
        ..Default::default()
    };
    for d in 0..2000 {
        r.saw(&trivial, d);
    }
    assert!(
        r.affection() <= 0.15,
        "two thousand small courtesies produced {:.2} of devotion",
        r.affection()
    );

    // Whereas a few real ones go much further.
    let mut real = Relationship::strangers(id[0], id[2]);
    for d in 0..30 {
        real.saw(
            &Evidence {
                contact: 1.0,
                warmth: 0.9,
                ..Default::default()
            },
            d,
        );
    }
    assert!(
        real.affection() > r.affection() * 3.0,
        "thirty genuine kindnesses were worth less than two thousand nods"
    );
}

// --- 14 ----------------------------------------------------------------

/// **One betrayal damages trust by severity and domain — it does not
/// delete every other dimension.**
///
/// Somebody who lets you down over money is not thereby a stranger you
/// dislike who is bad at their job. And an apology repairs only if it is
/// believed: sincere, owning it, and costing something.
#[test]
fn a_betrayal_is_specific_and_an_apology_must_be_credible() {
    let (_folk, id) = a_village();
    let mut r = Relationship::strangers(id[0], id[1]);
    for d in 0..80 {
        r.saw(
            &Evidence {
                contact: 1.0,
                warmth: 0.7,
                reliability: 0.7,
                reliability_in: Some(TrustIn::Money),
                esteem: 0.6,
                esteem_for: Some(RespectFor::Competence),
                ..Default::default()
            },
            d,
        );
    }
    let (was_fond, was_able, was_known) = (
        r.affection(),
        r.respect_for(RespectFor::Competence),
        r.familiarity,
    );
    let trusted_with_money = r.trust_in(TrustIn::Money);

    // He takes the money and runs.
    r.saw(
        &Evidence {
            contact: 1.0,
            warmth: -0.6,
            reliability: -1.0,
            reliability_in: Some(TrustIn::Money),
            wrong: 0.8,
            ..Default::default()
        },
        81,
    );

    assert!(
        r.trust_in(TrustIn::Money) < trusted_with_money - 0.4,
        "he would still hand him the takings"
    );
    assert!(
        r.familiarity >= was_known - 0.01,
        "being robbed made them strangers again"
    );
    assert!(
        r.respect_for(RespectFor::Competence) > was_able - 0.2,
        "being robbed made the man bad at his trade"
    );
    assert!(
        r.affection() < was_fond,
        "and it cost him nothing in affection"
    );
    assert!(r.resentment() > 0.5);

    // A cheap apology closes nothing.
    let mut cheap = r.clone();
    cheap.apologised(0.3, 0.2, 0.0);
    assert!(cheap.resentment() > 0.4, "a shrug and a sorry settled it");

    // A real one repairs the grievance — and does not restore the trust,
    // which has its own evidence and needs new evidence to move.
    let mut proper = r.clone();
    proper.apologised(0.95, 0.9, 0.8);
    assert!(
        proper.resentment() < 0.2,
        "a full apology was worth nothing"
    );
    assert!(
        proper.trust_in(TrustIn::Money) < trusted_with_money - 0.4,
        "saying sorry made him trustworthy with money again"
    );
}

/// A seed builds the same feelings.
#[test]
fn the_same_history_gives_the_same_relationship() {
    let (_folk, id) = a_village();
    let build = || {
        let mut r = Relationship::strangers(id[0], id[1]);
        for d in 0..50 {
            r.saw(&kindly(), d);
        }
        r
    };
    assert_eq!(build(), build());
}

/// **How much is believed and how firmly are different things.**
///
/// One polite act and thirty years of unbroken civility both imply about
/// the same modest affection — and one rude afternoon should not do the
/// same damage to each. Repetition buys **confidence in a modest
/// conclusion**, not a larger conclusion.
#[test]
fn a_long_acquaintance_is_harder_to_overturn_than_a_first_impression() {
    let (_folk, id) = a_village();
    let civil = Evidence {
        contact: 0.4,
        warmth: 0.12,
        ..Default::default()
    };
    let rude = Evidence {
        contact: 0.4,
        warmth: -0.35,
        ..Default::default()
    };

    let mut one_meeting = Relationship::strangers(id[0], id[1]);
    one_meeting.saw(&civil, 0);

    let mut thirty_years = Relationship::strangers(id[0], id[2]);
    for d in 0..2000 {
        thirty_years.saw(&civil, d);
    }

    // Both conclusions are modest, and about the same size.
    assert!(
        (one_meeting.affection() - thirty_years.affection()).abs() < 0.06,
        "repetition inflated the conclusion instead of the confidence: \
         {:.2} against {:.2}",
        one_meeting.affection(),
        thirty_years.affection()
    );
    // The confidence is not.
    assert!(
        thirty_years.sureness_of_affection() > 3.0 * one_meeting.sureness_of_affection(),
        "thirty years left him no surer than one afternoon"
    );

    // Now one rude encounter each.
    let (a_was, b_was) = (one_meeting.affection(), thirty_years.affection());
    one_meeting.saw(&rude, 3000);
    thirty_years.saw(&rude, 3000);
    let a_moved = (one_meeting.affection() - a_was).abs();
    let b_moved = (thirty_years.affection() - b_was).abs();

    assert!(
        a_moved > 3.0 * b_moved,
        "one rude afternoon cost a stranger {a_moved:.3} and a man of \
         thirty years' acquaintance {b_moved:.3}"
    );
    assert!(
        thirty_years.affection() > 0.0,
        "one bad day undid thirty years of civility"
    );
    assert!(
        one_meeting.affection() < 0.0,
        "a stranger who was rude is still liked"
    );
}

/// **A betrayal is not another data point.**
///
/// A weighted mean alone makes a long history nearly immovable, which is
/// right for civility and wrong for treachery: one clear defection
/// reveals a disposition and *invalidates* the history rather than being
/// averaged against it. That asymmetry is why trust is hard to build and
/// easy to destroy.
#[test]
fn treachery_throws_the_evidence_away_and_rudeness_does_not() {
    let (_folk, id) = a_village();
    let steady = Evidence {
        contact: 1.0,
        reliability: 0.8,
        reliability_in: Some(TrustIn::Money),
        telling: Diagnosticity::default(),
        ..Default::default()
    };

    let mut r = Relationship::strangers(id[0], id[1]);
    for d in 0..200 {
        r.saw(&steady, d);
    }
    let earned = r.trust_in(TrustIn::Money);
    let sureness = r.sureness_of_trust(TrustIn::Money);
    assert!(
        earned > 0.6 && sureness > 0.8,
        "two hundred honest days built nothing"
    );

    // A small lapse barely registers against that history.
    let mut careless = r.clone();
    careless.saw(
        &Evidence {
            contact: 1.0,
            reliability: -0.3,
            reliability_in: Some(TrustIn::Money),
            telling: Diagnosticity::default(),
            ..Default::default()
        },
        201,
    );
    assert!(
        careless.trust_in(TrustIn::Money) > earned - 0.2,
        "one careless afternoon undid two hundred honest days"
    );

    // Outright theft does not average — it discards.
    let mut robbed = r.clone();
    robbed.saw(
        &Evidence {
            contact: 1.0,
            reliability: -1.0,
            reliability_in: Some(TrustIn::Money),
            telling: Diagnosticity::default(),
            ..Default::default()
        },
        201,
    );
    assert!(
        robbed.trust_in(TrustIn::Money) < 0.3,
        "he was robbed and would still hand over the takings ({:.2})",
        robbed.trust_in(TrustIn::Money)
    );
    assert!(
        robbed.sureness_of_trust(TrustIn::Money) < sureness,
        "he is as sure of the man as he ever was, having just been robbed"
    );
}

// ---------------------------------------------------------------------
// rupture, diagnosticity and epochs
// ---------------------------------------------------------------------

/// **Two hundred uneventful days are not two hundred observed honesty
/// opportunities.**
///
/// Mere time without theft is not the same as handing back money when
/// nobody would have known. A man who never had the chance has shown you
/// nothing.
#[test]
fn time_without_temptation_shows_nothing() {
    let (_folk, id) = a_village();

    let mut never_tempted = Relationship::strangers(id[0], id[1]);
    let mut proved = Relationship::strangers(id[0], id[2]);
    for d in 0..200 {
        never_tempted.saw(
            &Evidence {
                contact: 1.0,
                reliability: 0.8,
                reliability_in: Some(TrustIn::Money),
                reliability_about: Aspect::Integrity,
                telling: Diagnosticity {
                    opportunity: 0.0,
                    ..Default::default()
                },
                ..Default::default()
            },
            d,
        );
        proved.saw(&shown(0.8, Aspect::Integrity, TrustIn::Money), d);
    }

    assert_eq!(
        never_tempted.trust_that(TrustIn::Money, Aspect::Integrity),
        0.0,
        "two hundred days of no opportunity to steal proved him honest"
    );
    assert_eq!(never_tempted.sureness_of_trust(TrustIn::Money), 0.0);
    assert!(
        proved.trust_that(TrustIn::Money, Aspect::Integrity) > 0.6,
        "two hundred days of returning the money proved nothing"
    );
    // Familiarity is not the same question and does rise either way.
    assert!(never_tempted.familiarity > 0.9);
}

/// **Deliberate theft damages integrity; carelessness damages
/// competence.**
///
/// The asymmetry does not run one way for everything. Negative behaviour
/// is especially diagnostic for **morality** and positive behaviour for
/// **ability** *(Mende-Siedlecki et al.)*: one dishonest act says a great
/// deal about honesty, one failure says little about capability, and one
/// brilliant performance says a lot.
#[test]
fn what_an_act_is_evidence_of_depends_on_what_is_being_judged() {
    let (_folk, id) = a_village();
    let honest = shown(0.8, Aspect::Integrity, TrustIn::Money);
    let able = shown(0.7, Aspect::Competence, TrustIn::Money);

    let mut careless = Relationship::strangers(id[0], id[1]);
    let mut thief = Relationship::strangers(id[0], id[2]);
    for d in 0..120 {
        careless.saw(&honest, d);
        careless.saw(&able, d);
        thief.saw(&honest, d);
        thief.saw(&able, d);
    }

    // He makes a mess of the books, and not on purpose.
    careless.saw(
        &Evidence {
            contact: 1.0,
            reliability: -0.7,
            reliability_in: Some(TrustIn::Money),
            reliability_about: Aspect::Competence,
            telling: Diagnosticity {
                intentional: 0.0,
                ..Default::default()
            },
            ..Default::default()
        },
        121,
    );
    // He takes the money, knowing exactly what he is doing.
    thief.saw(&shown(-1.0, Aspect::Integrity, TrustIn::Money), 121);

    assert!(
        careless.trust_that(TrustIn::Money, Aspect::Integrity) > 0.6,
        "bad bookkeeping made him a thief"
    );
    assert!(
        careless.trust_that(TrustIn::Money, Aspect::Competence)
            < careless.trust_that(TrustIn::Money, Aspect::Integrity),
        "bad bookkeeping said nothing about his bookkeeping"
    );
    assert!(
        thief.trust_that(TrustIn::Money, Aspect::Integrity) < 0.3,
        "theft left his honesty intact"
    );
    assert!(
        thief.trust_that(TrustIn::Money, Aspect::Competence) > 0.5,
        "stealing the money made him bad at counting it"
    );
}

/// **A failure at something hard is weak evidence; a brilliant success is
/// strong.** The other half of the asymmetry, running the other way.
#[test]
fn one_triumph_outweighs_an_ordinary_failure() {
    let (_folk, id) = a_village();
    let mut r = Relationship::strangers(id[0], id[1]);
    for d in 0..20 {
        r.saw(&shown(0.2, Aspect::Competence, TrustIn::Craft), d);
    }
    let before = r.trust_that(TrustIn::Craft, Aspect::Competence);

    let mut after_a_failure = r.clone();
    after_a_failure.saw(&shown(-0.8, Aspect::Competence, TrustIn::Craft), 21);
    let mut after_a_triumph = r.clone();
    after_a_triumph.saw(&shown(0.95, Aspect::Competence, TrustIn::Craft), 21);

    let fell = before - after_a_failure.trust_that(TrustIn::Craft, Aspect::Competence);
    let rose = after_a_triumph.trust_that(TrustIn::Craft, Aspect::Competence) - before;
    assert!(
        rose > fell,
        "one botched job counted for more than one masterpiece ({fell:.3} against {rose:.3})"
    );
}

/// **Coercion is not character.**
///
/// A man who steals with a knife at his back has shown you something
/// about what he will do under duress and very little about what he is.
#[test]
fn a_coerced_act_says_less_about_the_man() {
    let (_folk, id) = a_village();
    let steady = shown(0.8, Aspect::Integrity, TrustIn::Money);

    let mut freely = Relationship::strangers(id[0], id[1]);
    let mut forced = Relationship::strangers(id[0], id[2]);
    for d in 0..150 {
        freely.saw(&steady, d);
        forced.saw(&steady, d);
    }
    freely.saw(&shown(-1.0, Aspect::Integrity, TrustIn::Money), 151);
    forced.saw(
        &Evidence {
            contact: 1.0,
            reliability: -1.0,
            reliability_in: Some(TrustIn::Money),
            reliability_about: Aspect::Integrity,
            // He did it, and it was not his idea.
            telling: Diagnosticity {
                responsibility: 0.2,
                intentional: 0.3,
                ..Default::default()
            },
            ..Default::default()
        },
        151,
    );

    assert!(
        forced.trust_that(TrustIn::Money, Aspect::Integrity)
            > freely.trust_that(TrustIn::Money, Aspect::Integrity) + 0.2,
        "a man with a knife at his back is judged like a man with a plan"
    );
    // It still lowers what you would predict of him under pressure.
    assert!(
        forced.trust_that(TrustIn::Money, Aspect::Integrity) < 0.8,
        "being robbed by him at knifepoint changed nothing at all"
    );
}

/// **Low expectation, high certainty.**
///
/// Two quantities cannot say what being robbed does. After a clear theft
/// a man may be *very sure* the thief is untrustworthy, and separately
/// have no idea what else the fellow might do.
#[test]
fn certainty_expectation_and_volatility_are_three_things() {
    let (_folk, id) = a_village();
    let mut r = Relationship::strangers(id[0], id[1]);
    for d in 0..200 {
        r.saw(&shown(0.8, Aspect::Integrity, TrustIn::Money), d);
    }
    let settled = r.volatility_of(TrustIn::Money, Aspect::Integrity);
    assert!(settled < 0.1, "a steady man read as unpredictable");

    r.saw(&shown(-1.0, Aspect::Integrity, TrustIn::Money), 201);

    assert!(
        r.trust_that(TrustIn::Money, Aspect::Integrity) < 0.3,
        "he would still hand over the takings"
    );
    assert!(
        r.sureness_of_trust(TrustIn::Money) > 0.3,
        "watching a man steal left him unsure of anything"
    );
    assert!(
        r.volatility_of(TrustIn::Money, Aspect::Integrity) > settled + 0.2,
        "the contradiction did not make the man harder to predict"
    );
}

/// **Disproving it restores what it discounted.**
///
/// The history is kept and not deleted, so a man cleared of a theft does
/// not have to earn two hundred days of trust over again.
#[test]
fn exoneration_gives_back_the_years() {
    let (_folk, id) = a_village();
    let mut r = Relationship::strangers(id[0], id[1]);
    for d in 0..200 {
        r.saw(&shown(0.8, Aspect::Integrity, TrustIn::Money), d);
    }
    let earned = r.trust_that(TrustIn::Money, Aspect::Integrity);

    r.saw(&shown(-1.0, Aspect::Integrity, TrustIn::Money), 201);
    assert!(r.trust_that(TrustIn::Money, Aspect::Integrity) < 0.3);

    // It was not him.
    r.disproved(TrustIn::Money, Aspect::Integrity);
    assert!(
        (r.trust_that(TrustIn::Money, Aspect::Integrity) - earned).abs() < 0.02,
        "clearing him left him having to earn two hundred days over again ({:.2} against {earned:.2})",
        r.trust_that(TrustIn::Money, Aspect::Integrity)
    );
}

/// **A betrayal over money does not erase unrelated competence.**
#[test]
fn stealing_the_money_does_not_make_him_a_coward() {
    let (_folk, id) = a_village();
    let mut r = Relationship::strangers(id[0], id[1]);
    for d in 0..80 {
        r.saw(&shown(0.9, Aspect::Competence, TrustIn::Danger), d);
        r.saw(&shown(0.8, Aspect::Integrity, TrustIn::Money), d);
    }
    let beside_him = r.trust_that(TrustIn::Danger, Aspect::Competence);
    r.saw(&shown(-1.0, Aspect::Integrity, TrustIn::Money), 81);

    assert!(r.trust_that(TrustIn::Money, Aspect::Integrity) < 0.3);
    assert_eq!(
        r.trust_that(TrustIn::Danger, Aspect::Competence),
        beside_him,
        "the thief became useless in a fight"
    );
}
