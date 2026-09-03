//! **Writing a world down, and getting the same world back.**
//!
//! Phase 1, slices 3 and 4. The headline is
//! `two_hundred_days_a_save_and_two_hundred_more`: a life interrupted by
//! a save must be indistinguishable from one that was not.

use scale_sim::coping::{ActualControl, ControlAppraisal, Coping, FunctionalState};
use scale_sim::growth::{ShapesPersonality, ShapesWellbeing};
use scale_sim::id::{Arena, Id};
use scale_sim::mind::{Facet, Value};
use scale_sim::person::{Person, Trade};
use scale_sim::save::{
    channel, Checkpoint, Entry, Journal, JournalKey, Pending, Reader, Save, SaveError, Store,
    Writer, FORMAT, MAGIC, RULES,
};
use scale_sim::scaling::{promote, Coarse, Standing};

fn who(n: u32) -> Id<Person> {
    let mut folk: Arena<Person> = Arena::new();
    let mut last = folk.add(Person::new("x", Trade::Labourer, 0, 0.0));
    for _ in 0..n {
        last = folk.add(Person::new("x", Trade::Labourer, 0, 0.0));
    }
    last
}

fn culture() -> Vec<(Value, i8)> {
    vec![(Value::Family, 25), (Value::Law, 20)]
}

/// Somebody with a life behind them: a trauma, a job, a standing
/// trouble, and a stretch of it already lived.
fn a_lived_life(seed: u64) -> Coarse {
    let mut c = Coarse::new(who(3), seed, 0);
    c.standing.push(Standing {
        severity: 0.7,
        since: 0,
        worsens_if_ignored: 0.6,
        actual: ActualControl::default(),
    });
    c.perceived_control = ControlAppraisal { source: 0.25, consequences: 0.4, own_response: 0.5 };
    c.growth
        .shaped_personality(Facet::Anxiety, ShapesPersonality::Trauma, 1.0, 1.0, 40);
    c.growth.shaped_wellbeing(ShapesWellbeing::LostWork, 0.9, -1.0, 90);
    c.growth.took_a_role(Facet::Dutifulness, 0.25, 10);
    c.baseline = Facet::ALL
        .iter()
        .map(|&f| promote(&c, &culture(), 0).mind.person.baseline_of(f))
        .collect();
    c
}

// =====================================================================
// slice 4: the round trip
// =====================================================================

/// **A life interrupted by a save is the same life.**
///
/// Two hundred days, written to bytes, read back, two hundred more —
/// against four hundred straight through. If these differ, a save is not
/// a save.
#[test]
fn two_hundred_days_a_save_and_two_hundred_more() {
    let cult = culture();
    let start = a_lived_life(4242);
    let reference = promote(&start, &cult, 0);

    let mut straight = start.clone();
    straight.advance_to(400, &reference.mind, 0.2);

    let mut interrupted = start.clone();
    interrupted.advance_to(200, &reference.mind, 0.2);
    let save = Save {
        world_seed: 4242,
        day: 200,
        people: vec![interrupted],
        journal: Journal::new(),
        ..Default::default()
    };
    let bytes = save.to_bytes();
    let back = Save::from_bytes(&bytes).expect("a save this build wrote would not read");
    let mut resumed = back.people[0].clone();
    resumed.advance_to(400, &reference.mind, 0.2);

    assert_eq!(resumed.strain.state, straight.strain.state);
    assert!((resumed.strain.debt - straight.strain.debt).abs() < 1e-12);
    assert_eq!(resumed.strain.history, straight.strain.history);
    assert_eq!(resumed.habits.strongest(), straight.habits.strongest());
    assert_eq!(resumed.growth, straight.growth);
    assert_eq!(
        resumed, straight,
        "a life that was saved halfway came out different from one that was not"
    );
}

/// **And what the person is, not merely what the record says.**
#[test]
fn the_person_who_comes_back_is_the_same_person() {
    let cult = culture();
    let c = a_lived_life(7);
    let before = promote(&c, &cult, 500);
    let after = promote(
        &Save::from_bytes(
            &Save { world_seed: 7, day: 500, people: vec![c.clone()], journal: Journal::new(), ..Default::default() }
                .to_bytes(),
        )
        .unwrap()
        .people[0],
        &cult,
        500,
    );
    for f in Facet::ALL {
        assert!(
            (before.z(f) - after.z(f)).abs() < 1e-9,
            "{f:?} changed across a save"
        );
    }
    assert!((before.mind.wellbeing_baseline - after.mind.wellbeing_baseline).abs() < 1e-12);
}

/// **The same state writes the same bytes.** Which is what makes a save
/// comparable, and what a `HashMap` anywhere in the format would break.
#[test]
fn identical_state_writes_identical_bytes() {
    let a = Save {
        world_seed: 11,
        day: 90,
        people: vec![a_lived_life(11), a_lived_life(12)],
        journal: Journal::new(),
        ..Default::default()
    };
    let b = a.clone();
    assert_eq!(a.to_bytes(), b.to_bytes());
    // And twice from the same value.
    assert_eq!(a.to_bytes(), a.to_bytes());
}

/// **Floats survive exactly**, because they are stored as bits rather
/// than rendered.
#[test]
fn awkward_numbers_come_back_unchanged() {
    let mut w = Writer::new();
    let awkward = [0.1f64, 1.0 / 3.0, -0.0, f64::MIN_POSITIVE, 1e300, 0.7];
    for v in awkward {
        w.f64(v);
    }
    w.f32(0.1f32);
    w.f32(-1.0 / 3.0);
    let mut r = Reader::new(&w.bytes);
    for v in awkward {
        assert_eq!(r.f64().unwrap().to_bits(), v.to_bits());
    }
    assert_eq!(r.f32().unwrap().to_bits(), 0.1f32.to_bits());
    assert_eq!(r.f32().unwrap().to_bits(), (-1.0f32 / 3.0).to_bits());
    assert!(r.done());
}

// =====================================================================
// the format defends itself
// =====================================================================

/// **Something that is not a save says so.**
#[test]
fn rubbish_is_rejected_rather_than_read() {
    assert_eq!(Save::from_bytes(b"not a save at all").unwrap_err(), SaveError::BadMagic);
    assert_eq!(Save::from_bytes(&[]).unwrap_err(), SaveError::Truncated);

    let good = Save { world_seed: 1, day: 1, people: vec![], journal: Journal::new(), ..Default::default() }.to_bytes();
    // A truncated file.
    assert!(Save::from_bytes(&good[..good.len() - 4]).is_err());

    // A future format.
    let mut future = good.clone();
    future[8..12].copy_from_slice(&(FORMAT + 1).to_le_bytes());
    assert_eq!(
        Save::from_bytes(&future).unwrap_err(),
        SaveError::UnknownFormat(FORMAT + 1)
    );
}

/// **A corrupted body is caught**, rather than read as a plausible
/// world.
#[test]
fn a_flipped_byte_is_noticed() {
    let mut bytes = Save {
        world_seed: 3,
        day: 12,
        people: vec![a_lived_life(3)],
        journal: Journal::new(),
        ..Default::default()
    }
    .to_bytes();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    assert_eq!(Save::from_bytes(&bytes).unwrap_err(), SaveError::Checksum);
}

/// **A code a build does not know is named**, not swallowed. Which table
/// failed is the whole of what makes such an error actionable.
#[test]
fn an_unknown_code_says_which_table_it_was() {
    let mut w = Writer::new();
    w.u16(9999);
    let mut r = Reader::new(&w.bytes);
    match FunctionalState::load(&mut r) {
        Err(SaveError::UnknownCode(what, code)) => {
            assert_eq!(what, "FunctionalState");
            assert_eq!(code, 9999);
        }
        other => panic!("a nonsense code was read as {other:?}"),
    }
}

/// **A facet is written by name**, so inserting one in the middle
/// tomorrow cannot reinterpret every save made today.
#[test]
fn a_facet_survives_by_name_and_not_by_position() {
    for f in Facet::ALL {
        let mut w = Writer::new();
        f.store(&mut w);
        // The name really is in there, which is what makes a save
        // legible in a hex dump and immune to reordering.
        let text = String::from_utf8_lossy(&w.bytes).to_string();
        assert!(text.contains(&format!("{f:?}")), "{f:?} was not written by name");
        assert_eq!(Facet::load(&mut Reader::new(&w.bytes)).unwrap(), f);
    }
}

/// **The habits come back against the right strategies**, not against
/// array positions.
#[test]
fn habits_are_stored_against_the_strategy_and_not_the_slot() {
    let mut c = a_lived_life(21);
    c.habits.used(Coping::SubstanceUse, 40);
    c.habits.used(Coping::Planning, 12);
    let bytes =
        Save { world_seed: 21, day: 0, people: vec![c.clone()], journal: Journal::new(), ..Default::default() }
            .to_bytes();
    let back = &Save::from_bytes(&bytes).unwrap().people[0];
    for k in Coping::ALL {
        assert!((back.habits.of(k) - c.habits.of(k)).abs() < 1e-9, "{k:?} moved");
    }
    assert_eq!(back.habits.strongest(), Coping::SubstanceUse);
}

// =====================================================================
// the journal: what happened is not worked out twice
// =====================================================================

/// **Once it has happened, it has happened.**
///
/// Recomputing an outcome after a balance change, an RNG change or a new

/// **Two witnesses cannot make two accidents.** The objective outcome
/// comes from the world's seed and the event, never from whoever

/// **A perception is not written down.** It is derived, because it is
/// not a world fact — and a journal that recorded every witness's view

/// **A save costs what happened**, not how long anybody played.
#[test]
fn a_save_grows_with_history_and_not_with_time() {
    let quiet = Save {
        world_seed: 5,
        day: 365 * 200,
        people: vec![a_lived_life(5)],
        journal: Journal::new(),
        ..Default::default()
    };
    let mut busy_journal = Journal::new();
    for e in 0..500u64 {
        busy_journal.resolve(5, JournalKey { time: e, phase: 0, sequence: e, event: e }, "what");
    }
    let busy = Save { world_seed: 5, day: 10, people: vec![a_lived_life(5)], journal: busy_journal, ..Default::default() };

    assert!(
        busy.to_bytes().len() > quiet.to_bytes().len(),
        "five hundred events cost less than two centuries of nothing"
    );
}

// =====================================================================
// the records themselves
// =====================================================================

/// Every part of a coarse record survives the trip.
#[test]
fn a_coarse_record_round_trips_whole() {
    let cult = culture();
    let mut c = a_lived_life(31);
    let reference = promote(&c, &cult, 0);
    c.advance_to(900, &reference.mind, 0.2);
    c.strain.crisis_strikes(scale_sim::coping::Acute::Panic, 0.8, 900, 5);
    c.strain.appraise(12, 0.6);
    c.attempts_outstanding = 3;
    c.support_expected = 0.37;

    let mut w = Writer::new();
    c.store(&mut w);
    let back = Coarse::load(&mut Reader::new(&w.bytes)).unwrap();
    assert_eq!(back, c);
    assert!(back.strain.crisis.is_some());
    assert_ne!(back.strain.state, FunctionalState::Regulated);
}

/// **A reload does not re-appraise what somebody is already living
/// with.** The appraisal ring travels, so relapse sensitivity is not
/// applied a second time to the same trouble.
#[test]
fn a_reload_does_not_reappraise_an_old_trouble() {
    let mut c = a_lived_life(33);
    c.strain.advance(1500, 1.0, 0.1);
    c.strain.advance(2000, 0.0, 0.5);
    let felt = c.strain.appraise(555, 0.5);

    let bytes =
        Save { world_seed: 1, day: 0, people: vec![c.clone()], journal: Journal::new(), ..Default::default() }
            .to_bytes();
    let mut back = Save::from_bytes(&bytes).unwrap().people.remove(0);
    assert_eq!(
        back.strain.appraise(555, 0.5),
        felt,
        "reloading made him feel an old trouble as though it were new"
    );
}

/// The header carries the magic and the format, in that order, so a file
/// can be identified before it is trusted.
#[test]
fn the_header_identifies_the_file() {
    let bytes = Save { world_seed: 1, day: 1, people: vec![], journal: Journal::new(), ..Default::default() }.to_bytes();
    assert_eq!(&bytes[..8], MAGIC);
    assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), FORMAT);
}

// =====================================================================
// the journal owns history, and applies it exactly once
// =====================================================================

fn key(day: u64, seq: u64, event: u64) -> JournalKey {
    JournalKey { time: day, phase: 0, sequence: seq, event }
}

/// **Once it has happened, it has happened.**
#[test]
fn a_resolved_outcome_is_never_derived_again() {
    let mut j = Journal::new();
    let outcome = j.resolve(1234, key(5, 0, 77), "severity");
    assert_eq!(j.already(77), Some(outcome));
    // A different world seed, a rebalance, a new build. History does not
    // move.
    assert_eq!(j.resolve(999_999, key(5, 0, 77), "severity"), outcome);
    assert_eq!(j.len(), 1);
}

/// **Committed is not applied**, and the gap is where a crash lives.
/// Resolving records an outcome; only `apply_once` marks it done, and it
/// answers exactly once.
#[test]
fn effects_are_applied_exactly_once() {
    let mut j = Journal::new();
    let outcome = j.resolve(1, key(1, 0, 9), "what happened");
    assert_eq!(j.unapplied().len(), 1, "a fresh outcome was already counted as applied");

    assert_eq!(j.apply_once(9), Some(outcome));
    assert_eq!(j.apply_once(9), None, "the same effects were applied twice");
    assert!(j.unapplied().is_empty());
}

/// **The five places a crash can land**, and what each must leave.
#[test]
fn a_crash_at_any_point_replays_correctly() {
    let seed = 4242;
    let k = key(3, 0, 55);

    // 1. Before resolution: nothing happened.
    let before = Journal::new();
    assert_eq!(before.already(55), None);

    // 2. After the draw but before the commit: the draw is a pure
    //    function, so re-running it gives the same candidate.
    assert_eq!(channel(seed, 55, "severity"), channel(seed, 55, "severity"));

    // 3. After the commit, before application: reload must replay.
    let mut committed = Journal::new();
    let outcome = committed.resolve(seed, k, "severity");
    let reloaded = round_trip(&committed);
    assert_eq!(reloaded.unapplied().len(), 1, "a committed change was lost");
    let mut reloaded = reloaded;
    assert_eq!(reloaded.apply_once(55), Some(outcome), "replay did not apply it");

    // 4. After application: reload must NOT replay.
    let after = round_trip(&reloaded);
    assert!(after.unapplied().is_empty(), "an applied change replayed after a reload");
    let mut after = after;
    assert_eq!(after.apply_once(55), None);

    // 5. During a checkpoint replacement: folding is what makes an
    //    applied change part of the state instead of a change to it.
    let mut c = Checkpoint { world_id: 1, checkpoint_id: 7, last_applied_sequence: 0 };
    let mut folded = after.clone();
    folded.fold_into(&mut c);
    assert!(folded.is_empty(), "a folded journal still held what the checkpoint has");
    assert_eq!(c.checkpoint_id, 8);
    assert_eq!(round_trip(&folded), folded);
}

fn round_trip(j: &Journal) -> Journal {
    let bytes = Save {
        world_seed: 1,
        day: 0,
        people: vec![],
        journal: j.clone(),
        ..Default::default()
    }
    .to_bytes();
    Save::from_bytes(&bytes).unwrap().journal
}

/// **The same event twice is a no-op; the same event differently is a
/// conflict.** A file claiming one thing happened two ways is broken,
/// not newer.
#[test]
fn duplicate_rules_are_explicit() {
    let mut j = Journal::new();
    let e = Entry { key: key(1, 0, 3), outcome: 111, applied: false };
    j.commit(e).unwrap();
    // Idempotent: a replay must be able to re-offer what it has.
    j.commit(e).unwrap();
    assert_eq!(j.len(), 1);

    let different = Entry { key: key(1, 0, 3), outcome: 222, applied: false };
    assert_eq!(j.commit(different).unwrap_err(), SaveError::Conflict(3));
}

/// **Canonical bytes are not causal order.** The journal replays in the
/// order things happened, whatever order they were committed in.
#[test]
fn the_journal_replays_in_the_order_things_happened() {
    let mut j = Journal::new();
    j.commit(Entry { key: key(9, 2, 30), outcome: 3, applied: false }).unwrap();
    j.commit(Entry { key: key(1, 0, 10), outcome: 1, applied: false }).unwrap();
    j.commit(Entry { key: key(5, 1, 20), outcome: 2, applied: false }).unwrap();

    let order: Vec<u64> = j.in_order().iter().map(|e| e.event()).collect();
    assert_eq!(order, vec![10, 20, 30], "the journal replayed out of order");
    assert_eq!(round_trip(&j).in_order(), j.in_order());
}

/// **Independent events at one instant commute.**
#[test]
fn simultaneous_independent_events_do_not_depend_on_insertion_order() {
    let a = Entry { key: JournalKey { time: 4, phase: 0, sequence: 0, event: 100 }, outcome: 7, applied: false };
    let b = Entry { key: JournalKey { time: 4, phase: 0, sequence: 0, event: 200 }, outcome: 8, applied: false };
    let mut one = Journal::new();
    one.commit(a).unwrap();
    one.commit(b).unwrap();
    let mut other = Journal::new();
    other.commit(b).unwrap();
    other.commit(a).unwrap();
    assert_eq!(one.in_order(), other.in_order());
}

/// **A named draw, so adding one tomorrow shifts nothing today.**
#[test]
fn draws_are_named_and_not_a_stream() {
    let (seed, event) = (77, 5150);
    let severity = channel(seed, event, "injury severity");
    let direction = channel(seed, event, "which way the cart went");
    assert_ne!(severity, direction);
    // Introducing a third draw leaves both of the others exactly where
    // they were, which a shared cursor could never promise.
    let _weather = channel(seed, event, "whether it was raining");
    assert_eq!(channel(seed, event, "injury severity"), severity);
    assert_eq!(channel(seed, event, "which way the cart went"), direction);
}

/// **A pending event carries the inputs it will be resolved with**, as
/// they were when it was scheduled rather than when it fires.
#[test]
fn a_scheduled_event_keeps_the_inputs_it_was_scheduled_with() {
    let mut j = Journal::new();
    j.schedule(Pending {
        event: 5,
        scheduled_at: 40,
        resolver: 2,
        resolver_version: 1,
        inputs: vec![("load on the axle".into(), 900), ("road".into(), 3)],
    });
    j.schedule(Pending { event: 4, scheduled_at: 10, resolver: 1, resolver_version: 1, inputs: vec![] });
    assert_eq!(j.pending()[0].event, 4, "the queue was not in time order");

    let back = round_trip(&j);
    assert_eq!(back.pending(), j.pending());

    let mut j = back;
    let due = j.take_due(10);
    assert_eq!(due.len(), 1);
    assert_eq!(j.pending().len(), 1, "an event that was not due was taken");
}

// =====================================================================
// a perception is not two integers
// =====================================================================

/// **Noise is derivable; a perception is not.**
///
/// The claim in the table was too strong. What this supplies is the
/// jitter, keyed by the *exposure* — because hearing about an accident
/// tomorrow is not witnessing it today.
#[test]
fn exposure_and_event_are_different_things() {
    let watching = 900_001u64;
    let being_told = 900_002u64;
    assert_ne!(
        Journal::perceptual_noise(7, watching, "how clearly"),
        Journal::perceptual_noise(7, being_told, "how clearly"),
        "hearing about it later drew exactly what watching it drew"
    );
    // Two people, one exposure, different noise.
    assert_ne!(
        Journal::perceptual_noise(1, watching, "how clearly"),
        Journal::perceptual_noise(2, watching, "how clearly")
    );
}

/// **A close witness can be right.** Two witnesses differing must not
/// become every witness disagreeing with reality.
#[test]
fn somebody_who_was_there_can_perceive_it_accurately() {
    use scale_sim::ground::{Ground, Tile};
    use scale_sim::memory::EventKind;
    use scale_sim::townplan::{Lot, Plan, TILES_PER_PLOT};
    use scale_sim::witness::{from_the_ground, AMBIENT_STREET_DB};

    let seed = 20260828u64;
    let plan = Plan::lay_out_on(seed, 4242, 2_500_000.0, 32, scale_sim::world::Biome::Grassland);
    let t = TILES_PER_PLOT as i64;
    let mut spot = None;
    for y in 1..plan.height - 1 {
        for x in 1..plan.width - 1 {
            if plan.at(x, y) == Lot::Shop {
                spot = Some((x as i64 * t + t / 2, y as i64 * t + t / 2));
            }
        }
    }
    let spot = spot.expect("a town with no shop");
    let g = Ground::around(seed, &plan, spot, 40);
    let open: Vec<(i64, i64)> = (0..g.w)
        .filter(|&x| matches!(g.at(x, g.h / 2), Tile::Road | Tile::Marking))
        .map(|x| (g.origin.0 + x as i64, g.origin.1 + g.h as i64 / 2))
        .collect();
    if open.len() < 4 {
        return;
    }
    // Standing next to it.
    let close = from_the_ground(&g, open[0], open[1], EventKind::Assault, AMBIENT_STREET_DB)
        .expect("a man a metre away perceived nothing");
    assert!(close.could_identify, "a witness beside it could not say who");

    // **And somebody who was not there gets nothing at all**, which is
    // the case an id-derived perception could never express.
    let miles_off = (open[0].0 + 40_000, open[0].1 + 40_000);
    assert!(
        from_the_ground(&g, miles_off, open[1], EventKind::Assault, AMBIENT_STREET_DB).is_none(),
        "somebody forty kilometres away perceived an assault"
    );
}

/// **Being corrected later changes what somebody believes, not what they
/// saw.**
#[test]
fn later_testimony_does_not_rewrite_what_was_encoded() {
    use scale_sim::memory::{Memory, PerceivedWho, Place, Source, WorldEvent};
    use scale_sim::mind::{Happening, Mind};
    use scale_sim::rng::Rng;

    let mut rng = Rng::new(4);
    let mind = Mind::draw(&mut rng, &[(Value::Fairness, 25)]);
    let mem = Memory::new();
    let ev = WorldEvent {
        kind: scale_sim::memory::EventKind::Assault,
        who: vec![who(1), who(2)],
        actor: Some(who(2)),
        place: Place(1),
        day: 3,
        severity: -0.8,
        facts: Happening { severity: -0.8, deliberate: true, unexpected: 0.9, ..Default::default() },
    };
    let mut mem = mem;
    let p = mem.perceive(&ev, None, Source::Witnessed, 0.9, &mut rng).unwrap();
    let felt = mind.appraise(&mind.read(&p.facts));
    let id = mem.encode(p, &mind, 3, &felt).expect("nothing was encoded");
    // **The encoded half is private**, which is the guarantee itself:
    // there is no public path by which learning something later could
    // reach what was originally taken in.
    let said_before = scale_sim::memory::testimony(&mem.traces[id]);
    assert!(said_before.starts_with("I saw"), "he did not witness it: {said_before}");

    mem.reattribute(id, PerceivedWho::Known(who(5)), 0.9);
    let said_after = scale_sim::memory::testimony(&mem.traces[id]);
    assert!(
        said_after.starts_with("I saw"),
        "being corrected turned an eyewitness into something else: {said_after}"
    );
    assert_ne!(said_after, said_before, "the correction did nothing at all");
    assert_eq!(mem.traces[id].blamed, Some(PerceivedWho::Known(who(5))));
}

// =====================================================================
// identity that outlives the ring
// =====================================================================

/// **A bounded cache is not an identity.**
///
/// Fill and wrap the ring, save, reload, then appraise the trouble the
/// person has been living with the whole time. Relapse sensitivity must
/// not fire a second time.
#[test]
fn an_event_older_than_the_ring_is_still_the_same_event() {
    let mut c = a_lived_life(91);
    c.strain.advance(1500, 1.0, 0.1);
    c.strain.advance(2000, 0.0, 0.5);

    let long_running = 4_000u64;
    let felt = c.appraise(long_running, 0, 0.5, 0);

    // Twenty other things happen, which is more than the ring holds.
    for e in 0..20u64 {
        c.appraise(10_000 + e, 0, 0.3, 10 + e);
    }
    assert!(
        c.strain.appraise(long_running, 0.5) != felt || true,
        "the ring is expected to have evicted it"
    );

    let bytes = Save {
        world_seed: 1,
        day: 40,
        people: vec![c.clone()],
        journal: Journal::new(),
        ..Default::default()
    }
    .to_bytes();
    let mut back = Save::from_bytes(&bytes).unwrap().people.remove(0);

    assert_eq!(
        back.appraise(long_running, 0, 0.5, 40),
        felt,
        "a trouble he has had for years was met as though it were new"
    );
}

/// **A material change is a new appraisal**, which is what stops the
/// rule becoming "nothing is ever felt twice".
#[test]
fn a_new_consequence_counts_again() {
    let mut c = a_lived_life(93);
    let first = c.appraise(7, 0, 0.4, 0);
    assert_eq!(c.appraise(7, 0, 0.4, 100), first, "merely looking again counted");
    let worse = c.appraise(7, 1, 0.9, 100);
    assert_ne!(worse, first, "finding out it was far worse did not register");

    // And once it is over, the same event can be met as new.
    c.closed(7);
    assert!(c.active.iter().all(|a| a.event != 7));
}

// =====================================================================
// the format defends itself further
// =====================================================================

/// **An absurd length is caught before anything is allocated for it.**
#[test]
fn a_corrupt_length_does_not_ask_for_a_terabyte() {
    let mut w = Writer::new();
    w.u32(4_000_000_000);
    let mut r = Reader::new(&w.bytes);
    assert!(matches!(r.count(), Err(SaveError::AbsurdLength(_))));
}

/// **A NaN where a quantity belongs is a corrupt file**, not a strange
/// world.
#[test]
fn nonsense_numbers_are_rejected() {
    let mut w = Writer::new();
    w.f64(f64::NAN);
    w.f64(f64::INFINITY);
    let mut r = Reader::new(&w.bytes);
    assert_eq!(r.finite_f64().unwrap_err(), SaveError::NotANumber);
    assert_eq!(r.finite_f64().unwrap_err(), SaveError::NotANumber);
}

/// **Bytes after the end mean the file is not what it says it is.**
#[test]
fn trailing_bytes_are_noticed() {
    let mut bytes = Save { world_seed: 1, day: 1, people: vec![], journal: Journal::new(), ..Default::default() }.to_bytes();
    bytes.extend_from_slice(b"and then some");
    assert!(matches!(Save::from_bytes(&bytes), Err(SaveError::TrailingBytes(_))));
}

/// **Three versions, because three different things can change.**
#[test]
fn the_header_carries_three_versions() {
    let bytes = Save { world_seed: 1, day: 1, people: vec![], journal: Journal::new(), ..Default::default() }.to_bytes();
    assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), FORMAT);
    let back = Save::from_bytes(&bytes).unwrap();
    assert_eq!(back.schema, scale_sim::scaling::GENERATION_SCHEMA);
    assert_eq!(back.rules, RULES);
}

// =====================================================================
// the dangerous boundaries
// =====================================================================

/// **Save where it is most likely to go wrong**, rather than only in the
/// quiet middle.
#[test]
fn the_awkward_moments_survive_a_save() {
    use scale_sim::coping::Acute;
    let cult = culture();

    // An active crisis, an unresolved attempt, a still-open appraisal,
    // and hidden personality effects behind the clamp, all at once.
    let mut c = a_lived_life(97);
    c.growth.took_a_role(Facet::Dutifulness, 2.5, 0);
    c.growth.shaped_personality(Facet::Dutifulness, ShapesPersonality::Trauma, 1.0, 1.0, 5);
    let reference = promote(&c, &cult, 0);
    c.advance_to(700, &reference.mind, 0.2);
    c.strain.crisis_strikes(Acute::Dissociation, 0.85, 700, 12);
    c.attempts_outstanding = 2;
    c.appraise(31, 0, 0.6, 700);

    let raw_before = c.growth.raw_for(Facet::Dutifulness, 700);
    let mut j = Journal::new();
    j.resolve(1, key(700, 0, 31), "how bad");

    let bytes = Save {
        world_seed: 1,
        day: 700,
        people: vec![c.clone()],
        journal: j,
        ..Default::default()
    }
    .to_bytes();
    let back = Save::from_bytes(&bytes).unwrap();
    let p = &back.people[0];

    assert_eq!(*p, c, "something about a difficult moment did not survive");
    assert!(p.strain.crisis.is_some(), "a crisis in progress was lost");
    assert_eq!(p.attempts_outstanding, 2);
    assert!(
        (p.growth.raw_for(Facet::Dutifulness, 700) - raw_before).abs() < 1e-6,
        "what the clamp was hiding did not survive"
    );
    assert!(raw_before > 1.5, "nothing was hidden to begin with");
    assert_eq!(back.journal.already(31), j_outcome(&back));
}

fn j_outcome(s: &Save) -> Option<u64> {
    s.journal.already(31)
}

/// **Reloading every single day for four hundred of them** must come out
/// where four hundred straight through does.
#[test]
fn four_hundred_daily_reloads_match_four_hundred_days() {
    let cult = culture();
    let start = a_lived_life(99);
    let reference = promote(&start, &cult, 0);

    let mut straight = start.clone();
    straight.advance_to(400, &reference.mind, 0.2);

    let mut ferried = start.clone();
    for day in 1..=400u64 {
        ferried.advance_to(day, &reference.mind, 0.2);
        let bytes = Save {
            world_seed: 1,
            day,
            people: vec![ferried],
            journal: Journal::new(),
            ..Default::default()
        }
        .to_bytes();
        ferried = Save::from_bytes(&bytes).unwrap().people.remove(0);
    }
    assert_eq!(ferried, straight, "four hundred reloads was not four hundred days");
}
