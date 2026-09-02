//! **Slice 2's entry contract.** `docs/mind-spec.md`.
//!
//! The twelve required tests, in order. What they guard is a boundary,
//! not an arithmetic: what somebody knows, how they came to know it, and
//! the impossibility of being charged twice for the same bad day.

use scale_sim::memory::{
    holds, testimony, Cue, EventKind, Memory, PerceivedWho, Place, Source, WorldEvent,
};
use scale_sim::mind::{
    Appraisal, ConcernKind, Emotion, Facet, Happening, Mind, Personality, Value,
};
use scale_sim::id::{Arena, Id};
use scale_sim::person::{Person, Trade};
use scale_sim::rng::Rng;

/// **Real person handles**, now that a mind can hold one. A fresh arena
/// each call is fine: `Id` is a slot and a generation, so the same index
/// gives the same handle every time.
fn who(n: u32) -> Id<Person> {
    let mut folk: Arena<Person> = Arena::new();
    let mut last = folk.add(Person::new("x", Trade::Labourer, 0, 0.0));
    for _ in 0..n {
        last = folk.add(Person::new("x", Trade::Labourer, 0, 0.0));
    }
    last
}


fn a_culture() -> Vec<(Value, i8)> {
    vec![(Value::Fairness, 25), (Value::Family, 30), (Value::Law, 20)]
}

fn a_mind(seed: u64) -> Mind {
    Mind::draw(&mut Rng::new(seed), &a_culture())
}

fn plain(seed: u64, set: &[(Facet, f32)]) -> Mind {
    let mut m = a_mind(seed);
    for f in Facet::ALL {
        m.person.set_baseline(f, 0.0);
    }
    for &(f, z) in set {
        m.person.set_baseline(f, z);
    }
    m.willpower = 0.0;
    m
}

/// The mine collapse from the specification's worked example.
fn a_collapse(day: u64) -> WorldEvent {
    WorldEvent {
        kind: EventKind::Collapse,
        who: vec![who(1), who(2)],
        actor: Some(who(9)), // the foreman who ordered work despite the warning
        place: Place(7),
        day,
        severity: -0.9,
        facts: Happening {
            severity: -0.9,
            to_me: 0.7,
            to_mine: 0.8,
            deliberate: true,
            control: 0.1,
            unexpected: 0.95,
            by_a_decision: true,
            bears_on: Some((Value::Law, false)),
            ..Default::default()
        },
    }
}

// --- 1 -----------------------------------------------------------------

/// **An unseen event creates no memory.**
///
/// A citizen two streets away never learns it happened. Without this the
/// population is omniscient and merely disagrees, which is not the same
/// thing at all.
#[test]
fn an_unseen_event_creates_no_memory() {
    let mut rng = Rng::new(1);
    let mind = plain(1, &[]);
    let mut mem = Memory::new();
    let ev = a_collapse(100);

    assert!(
        mem.perceive(&ev, None, Source::Witnessed, 0.0, &mut rng).is_none(),
        "somebody with no exposure to an event perceived it anyway"
    );
    assert_eq!(mem.traces.len(), 0);

    // And with exposure, they do.
    let p = mem
        .perceive(&ev, None, Source::Witnessed, 1.0, &mut rng)
        .expect("a witness standing there perceived nothing");
    let felt = mind.appraise(&mind.read(&p.facts));
    assert!(mem.encode(p, &mind, 100, &felt).is_some());
    assert_eq!(mem.traces.len(), 1);
}

// --- 2 -----------------------------------------------------------------

/// **Hearsay stores "A told me X", not "X happened".**
#[test]
fn hearsay_records_the_telling_and_not_the_fact() {
    let mind = plain(2, &[]);
    let mut mem = Memory::new();
    let claim = Memory::hear(
        EventKind::Theft,
        Some(PerceivedWho::Known(who(4))),
        Place(2),
        50,
        Source::Told { by: who(3) },
    );
    let felt = mind.appraise(&mind.read(&claim.facts));
    let id = mem.encode(claim, &mind, 50, &felt).expect("a claim was not kept");

    let t = &mem.traces[id];
    assert!(t.is_hearsay(), "being told something was recorded as seeing it");
    assert_eq!(t.source(), Source::Told { by: who(3) });
    assert!(
        t.what_was_perceived().of.is_none(),
        "a claim was filed as a perception of a real event"
    );
    assert!(
        testimony(t).contains("told me"),
        "the witness would swear to having seen it: {}",
        testimony(t)
    );
}

// --- 3 -----------------------------------------------------------------

/// **Two witnesses encode different versions of one event.**
///
/// Where two honest accounts begin to differ: not in the facts, but in
/// how clearly each was placed to take them in — and then in what each
/// made of what they got.
#[test]
fn two_witnesses_encode_different_versions() {
    let mut rng = Rng::new(3);
    let ev = a_collapse(100);
    let mem = Memory::new();

    // One standing over it; one who heard the noise from a gallery away.
    let close = mem
        .perceive(&ev, None, Source::Witnessed, 1.0, &mut rng)
        .unwrap();
    let far = mem
        .perceive(&ev, None, Source::Overheard, 0.35, &mut rng)
        .unwrap();

    assert_eq!(
        close.believed_actor.as_ref().and_then(|p| p.person()),
        Some(who(9)),
        "the man on the spot saw nobody"
    );
    assert_eq!(
        far.believed_actor, None,
        "somebody who only heard it knew exactly who was responsible"
    );
    assert!(
        far.confidence < close.confidence,
        "hearing a crash and watching it happen left the same certainty"
    );

    // And two minds make different things of the same reading.
    let anxious = plain(3, &[(Facet::Anxiety, 1.8), (Facet::Gloom, 1.5)]);
    let steady = plain(3, &[(Facet::Anxiety, -1.8), (Facet::Pride, 1.5)]);
    let a = anxious.read(&close.facts);
    let s = steady.read(&close.facts);
    assert_ne!(a.confirms_a_fear, s.confirms_a_fear);
}

// --- 4 -----------------------------------------------------------------

/// **Thirty routine meals consolidate; one exceptional meal does not.**
///
/// The mechanism is routine consolidation — semanticisation — not
/// suppression. What is dropped is the separate *episode*; everything
/// the repetition actually produced survives, which is why the count,
/// the span and the range are kept.
#[test]
fn routine_days_consolidate_and_the_exceptional_one_does_not() {
    let mut rng = Rng::new(4);
    let mind = plain(4, &[]);
    let mut mem = Memory::new();

    for day in 0..30u64 {
        let meal = WorldEvent {
            kind: EventKind::Meal,
            who: vec![who(1)],
            actor: None,
            place: Place(1),
            day,
            severity: 0.1,
            facts: Happening { severity: 0.1, to_me: 0.4, unexpected: 0.05, ..Default::default() },
        };
        let p = mem.perceive(&meal, None, Source::Witnessed, 1.0, &mut rng).unwrap();
        let felt = mind.appraise(&mind.read(&p.facts));
        mem.encode(p, &mind, day, &felt);
    }
    assert_eq!(
        mem.traces.len(),
        0,
        "thirty ordinary dinners became thirty permanent memories"
    );
    let r = &mem.routines[0];
    assert_eq!(r.count, 30, "the fact of the repetition was lost with the episodes");
    assert_eq!((r.first, r.last), (0, 29), "the span of the habit was lost");

    // The night somebody proposed over dinner stays its own memory.
    let proposal = WorldEvent {
        kind: EventKind::Meal,
        who: vec![who(1), who(2)],
        actor: Some(who(2)),
        place: Place(1),
        day: 30,
        severity: 0.95,
        facts: Happening {
            severity: 0.95,
            to_me: 1.0,
            deliberate: true,
            unexpected: 0.9,
            ..Default::default()
        },
    };
    let p = mem.perceive(&proposal, None, Source::Witnessed, 1.0, &mut rng).unwrap();
    let felt = mind.appraise(&mind.read(&p.facts));
    assert!(
        mem.encode(p, &mind, 30, &felt).is_some(),
        "the night somebody proposed was filed with the other dinners"
    );
    assert_eq!(mem.traces.len(), 1);
}

// --- 5 and 6 -----------------------------------------------------------

/// **Recall after a value change produces a different appraisal — and
/// cannot reapply the original stress.**
///
/// The two halves of the same guarantee. A `Trace` keeps what it meant at
/// the time as a `RememberedFeeling`, which is a different type from an
/// `Episode` and has no conversion to one anywhere. So recall has no
/// stored transaction to replay; it must appraise afresh, with today's
/// values — which is exactly why the meaning can change.
#[test]
fn recall_appraises_afresh_and_cannot_replay_the_original() {
    let mut rng = Rng::new(5);
    let mut mind = plain(5, &[(Facet::Anger, 1.2)]);
    holds(&mut mind, Value::Law, 5);
    let mut mem = Memory::new();

    let ev = a_collapse(100);
    let p = mem.perceive(&ev, None, Source::Witnessed, 1.0, &mut rng).unwrap();
    let at_the_time = mind.appraise(&mind.read(&p.facts));
    let id = mem.encode(p, &mind, 100, &at_the_time).unwrap();

    let then_outrage = at_the_time
        .iter()
        .find(|e| e.what == Emotion::Outrage)
        .map(|e| e.strength)
        .unwrap_or(0.0);

    // Years on, the same man has come to care a great deal about the law.
    holds(&mut mind, Value::Law, 48);
    let r = mem.recall(id, &mind, 100 + 365 * 4).expect("the memory was gone");

    let now_outrage = r
        .episodes
        .iter()
        .find(|e| e.what == Emotion::Outrage)
        .map(|e| e.strength)
        .unwrap_or(0.0);
    assert!(
        now_outrage > then_outrage + 0.1,
        "the same memory, appraised by a man who now holds the law dear, \
         produced the same outrage as before ({then_outrage:.2} then, \
         {now_outrage:.2} now)"
    );

    // **What he remembers feeling is not a feeling.** It comes back for
    // comparison and there is no route by which it can be applied.
    assert!(!r.then.is_empty(), "he remembers nothing of how it felt");
    let remembered_fear = r
        .then
        .iter()
        .find(|f| f.what == Emotion::Fear)
        .map(|f| f.how_strongly);
    if let Some(fear) = remembered_fear {
        assert!(fear > 0.0);
        // The type says so: `then` is `RememberedFeeling`, `episodes` is
        // `Episode`, and `mind.feel` takes only the latter. There is no
        // conversion, which is what makes double application impossible
        // rather than merely discouraged.
    }

    // The encoding is still there, unchanged, and is not an episode.
    // **It holds what he perceived, not what happened** — perception is
    // imperfect and the trace records his version, which is the whole
    // point of the boundary.
    let stored = mem.traces[id].as_encoded();
    assert!(
        (stored.severity - ev.facts.severity).abs() < 0.2,
        "the trace records something unrecognisable as the event"
    );
    assert!(
        mem.traces[id].what_was_perceived().source.firsthand(),
        "a thing he watched happen is not recorded as firsthand"
    );
}

/// **Recalling something repeatedly does not accumulate its stress.**
///
/// The failure this guards is quiet: a man who thinks about a bad day
/// every day would be charged for it every day and would break within the
/// year, from one event.
#[test]
fn thinking_about_it_often_does_not_charge_it_again() {
    let mut rng = Rng::new(6);
    let mut mind = plain(6, &[(Facet::StressVulnerability, 0.6)]);
    let mut mem = Memory::new();

    let ev = a_collapse(0);
    let p = mem.perceive(&ev, None, Source::Witnessed, 1.0, &mut rng).unwrap();
    let felt = mind.appraise(&mind.read(&p.facts));
    let id = mem.encode(p, &mind, 0, &felt).unwrap();
    mind.feel(felt);
    let after_the_event = mind.stress.load;

    // He goes over it every day for a year. Each recollection is a fresh
    // appraisal and costs what a fresh appraisal costs — but the day
    // itself is not re-billed, and time is passing.
    for day in 1..=365u64 {
        let r = mem.recall(id, &mind, day).unwrap();
        mind.feel(r.episodes);
        mind.a_day_passes(&mut rng);
        mem.a_day_passes();
    }

    assert!(
        mind.stress.load < after_the_event * 6.0,
        "a year of remembering one bad day multiplied its cost by {:.0}",
        mind.stress.load / after_the_event.max(1e-6)
    );
    assert!(
        mem.traces[id].recalls == 365,
        "the recollections were not counted"
    );
}

// --- 7 -----------------------------------------------------------------

/// **A place, a person, an anniversary or a similar event brings it
/// back.**
#[test]
fn a_place_a_person_or_an_anniversary_cues_recall() {
    let mut rng = Rng::new(7);
    let mind = plain(7, &[]);
    let mut mem = Memory::new();
    let ev = a_collapse(100);
    let p = mem.perceive(&ev, None, Source::Witnessed, 1.0, &mut rng).unwrap();
    let felt = mind.appraise(&mind.read(&p.facts));
    let id = mem.encode(p, &mind, 100, &felt).unwrap();

    assert_eq!(mem.cued_by(Cue::Place(Place(7)), 200), vec![id], "the mine itself");
    assert_eq!(mem.cued_by(Cue::Person(who(9)), 200), vec![id], "the foreman");
    assert_eq!(
        mem.cued_by(Cue::Similar(EventKind::Collapse), 200),
        vec![id],
        "timber cracking underground"
    );
    assert_eq!(
        mem.cued_by(Cue::Anniversary { day: 465 }, 465),
        vec![id],
        "a year to the day"
    );
    assert!(
        mem.cued_by(Cue::Place(Place(99)), 200).is_empty(),
        "a place he has never been reminded him of the collapse"
    );
    assert!(
        mem.cued_by(Cue::Anniversary { day: 250 }, 250).is_empty(),
        "an ordinary day in the middle of the year is an anniversary"
    );
}

// --- 10 ----------------------------------------------------------------

/// **Recollection changes accessibility and interpretation without
/// erasing provenance.**
///
/// Reminders demonstrably fold newly learned material into a recalled
/// memory *(Hupbach et al.)*, which is reconstruction rather than
/// playback. What must survive it is where the memory came from — or a
/// rumour becomes an eyewitness account by being thought about often
/// enough.
#[test]
fn recollection_reshapes_a_memory_without_rewriting_where_it_came_from() {
    let mut rng = Rng::new(10);
    let mind = plain(10, &[]);
    let mut mem = Memory::new();
    let ev = a_collapse(100);
    let p = mem.perceive(&ev, None, Source::Witnessed, 1.0, &mut rng).unwrap();
    let felt = mind.appraise(&mind.read(&p.facts));
    let id = mem.encode(p, &mind, 100, &felt).unwrap();

    let source_then = mem.traces[id].source();
    let seen_then = mem.traces[id].what_was_perceived().clone();

    // **Left alone for a decade first**, because a memory already as
    // available as it can be has nowhere to go, and what is being tested
    // is that going over something brings it back within reach.
    for _ in 0..3650 {
        mem.a_day_passes();
    }
    let reachable_then = mem.traces[id].accessibility;

    // Going over it makes it easier to reach...
    for d in 3800..3810 {
        mem.recall(id, &mind, d);
    }
    assert!(
        mem.traces[id].accessibility > reachable_then,
        "rehearsing a memory did not make it more available"
    );

    // ...and learning later that a different man gave the order changes
    // who is blamed, not what was seen.
    mem.reattribute(id, PerceivedWho::Believed { person: who(12), confidence: 0.7 }, 0.7);
    assert_eq!(
        mem.traces[id].blamed.as_ref().and_then(|p| p.person()),
        Some(who(12))
    );
    assert_eq!(
        mem.traces[id].source(),
        source_then,
        "the provenance of a memory was rewritten by thinking about it"
    );
    assert_eq!(
        mem.traces[id].what_was_perceived().believed_actor,
        seen_then.believed_actor,
        "what he originally saw was overwritten by what he later concluded"
    );

    // He can still say what he actually saw, which is the whole point.
    assert!(testimony(&mem.traces[id]).starts_with("I saw"));
}

// --- 11 ----------------------------------------------------------------

/// **A core memory emits a bounded plasticity signal; it does not
/// overwrite a facet.**
///
/// A memory that could set a trait directly would let one bad afternoon
/// replace somebody. What it does instead is ask, the caller decides, and
/// `Personality::adapt` clamps what a whole life can do.
#[test]
fn a_core_memory_asks_rather_than_writes() {
    let mut rng = Rng::new(11);
    let mind = plain(11, &[(Facet::Anxiety, 0.5)]);
    let mut mem = Memory::new();
    let ev = a_collapse(0);
    let p = mem.perceive(&ev, None, Source::Witnessed, 1.0, &mut rng).unwrap();
    let felt = mind.appraise(&mind.read(&p.facts));
    let id = mem.encode(p, &mind, 0, &felt).unwrap();

    assert!(
        mem.traces[id].core,
        "surviving a mine collapse was not a core memory"
    );
    let ask = mem.traces[id].plasticity();
    assert!(!ask.is_empty(), "a core memory asked nothing of the person");
    for (_, by) in &ask {
        assert!(
            by.abs() < 0.25,
            "one memory asked to move a facet by {by:.2}, which is a \
             rewrite and not an adaptation"
        );
    }

    // An ordinary day asks for nothing.
    let dull = WorldEvent {
        kind: EventKind::Shift,
        who: vec![],
        actor: None,
        place: Place(7),
        day: 1,
        severity: 0.0,
        facts: Happening { unexpected: 0.02, ..Default::default() },
    };
    let p = mem.perceive(&dull, None, Source::Witnessed, 1.0, &mut rng).unwrap();
    let felt = mind.appraise(&mind.read(&p.facts));
    if let Some(dull_id) = mem.encode(p, &mind, 1, &felt) {
        assert!(mem.traces[dull_id].plasticity().is_empty());
    }

    // And applying it is bounded by the personality, not by the memory.
    let mut person = Personality::draw(&mut Rng::new(11));
    let was = person.z(Facet::Anxiety);
    for _ in 0..200 {
        for &(f, by) in &ask {
            person.adapt(f, by);
        }
    }
    assert!(
        (person.z(Facet::Anxiety) - was).abs() <= 1.51,
        "two hundred recollections of one day replaced the man"
    );
}

// --- 12 ----------------------------------------------------------------

/// **A lie stays distinguishable from something witnessed, for ever.**
///
/// A lie creates a belief about the world; it does not modify the world.
/// And reputation is not one number — it is the aggregate of what
/// different people happen to believe, which is what permits factions,
/// investigations, propaganda and mistaken identity.
#[test]
fn a_lie_never_becomes_an_eyewitness_account() {
    let mut rng = Rng::new(12);
    let mind = plain(12, &[]);
    let mut mem = Memory::new();

    // He saw one thing...
    let ev = a_collapse(100);
    let p = mem.perceive(&ev, None, Source::Witnessed, 1.0, &mut rng).unwrap();
    let felt = mind.appraise(&mind.read(&p.facts));
    let seen = mem.encode(p, &mind, 100, &felt).unwrap();

    // ...and was told another, by somebody with a reason to lie.
    let lie = Memory::hear(
        EventKind::Theft,
        Some(PerceivedWho::Known(who(9))),
        Place(7),
        101,
        Source::Rumour { hops: 2 },
    );
    let felt = mind.appraise(&mind.read(&lie.facts));
    let heard = mem.encode(lie, &mind, 101, &felt).unwrap();

    // Ten years of thinking about both.
    for d in 102..(102 + 365 * 10) {
        if d % 30 == 0 {
            mem.recall(seen, &mind, d);
            mem.recall(heard, &mind, d);
        }
        mem.a_day_passes();
    }

    assert!(!mem.traces[seen].is_hearsay(), "what he saw became something he was told");
    assert!(mem.traces[heard].is_hearsay(), "a rumour became an eyewitness account");
    assert!(
        mem.traces[heard].confidence < mem.traces[seen].confidence,
        "a rumour ended up as certain as a thing he watched happen"
    );
    assert!(testimony(&mem.traces[seen]).starts_with("I saw"));
    assert!(testimony(&mem.traces[heard]).contains("going round"));
}

// --- 8 and 9 are slice 1's, and still hold ----------------------------

/// **The concern outlives the memory's activation**, which is where the
/// two slices meet: a cue reaches a memory, the memory produces a fresh
/// appraisal, and the standing concern is what makes it happen again next
/// year.
#[test]
fn a_cue_reaches_a_memory_and_the_concern_keeps_it_alive() {
    let mut rng = Rng::new(8);
    let mut mind = plain(8, &[(Facet::Anger, 1.4), (Facet::Vengefulness, 1.2)]);
    let mut mem = Memory::new();

    let ev = a_collapse(0);
    let p = mem.perceive(&ev, None, Source::Witnessed, 1.0, &mut rng).unwrap();
    let felt = mind.appraise(&mind.read(&p.facts));
    let id = mem.encode(p, &mind, 0, &felt).unwrap();
    mind.feel(felt);
    let grievance = mind.take_on(ConcernKind::Grievance, 0.85);

    // A year of ordinary days, then the anniversary.
    for d in 1..365u64 {
        mind.a_day_passes(&mut rng);
        mem.a_day_passes();
    }
    let quiet = mind.feeling_of(Emotion::Anger);

    let brought_back = mem.cued_by(Cue::Anniversary { day: 366 }, 366);
    assert_eq!(brought_back, vec![id], "a year to the day reminded him of nothing");
    let r = mem.recall(id, &mind, 366).unwrap();
    mind.feel(r.episodes);
    if let Some(e) = mind.cued(grievance, 1.0) {
        mind.feel(vec![e]);
    }
    assert!(
        mind.feeling_of(Emotion::Anger) > quiet,
        "the anniversary of the collapse was an ordinary day"
    );
}

/// A seed rebuilds the same memories.
#[test]
fn the_same_seed_remembers_the_same_things() {
    let build = || {
        let mut rng = Rng::new(99);
        let mind = plain(99, &[]);
        let mut mem = Memory::new();
        let ev = a_collapse(10);
        let p = mem.perceive(&ev, None, Source::Witnessed, 0.7, &mut rng).unwrap();
        let felt = mind.appraise(&mind.read(&p.facts));
        let id = mem.encode(p, &mind, 10, &felt).unwrap();
        (mem.traces[id].clone(), felt)
    };
    let (a, fa) = build();
    let (b, fb) = build();
    assert_eq!(a, b);
    assert_eq!(fa, fb);
}
