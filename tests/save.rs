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
use scale_sim::save::{Journal, Reader, Save, SaveError, Store, Writer, FORMAT, MAGIC};
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
            &Save { world_seed: 7, day: 500, people: vec![c.clone()], journal: Journal::new() }
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

    let good = Save { world_seed: 1, day: 1, people: vec![], journal: Journal::new() }.to_bytes();
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
        Save { world_seed: 21, day: 0, people: vec![c.clone()], journal: Journal::new() }
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
/// version could alter something a witness already remembers.
#[test]
fn a_resolved_outcome_is_never_derived_again() {
    let mut j = Journal::new();
    let outcome = j.objective(1234, 77);
    assert_eq!(j.already(77), Some(outcome));

    // The world seed changes — a different build, a rebalanced world.
    // What already happened does not.
    assert_eq!(j.objective(999_999, 77), outcome);
    assert_eq!(j.len(), 1);

    // And it survives the round trip as history rather than as a rule.
    let bytes =
        Save { world_seed: 1234, day: 5, people: vec![], journal: j.clone() }.to_bytes();
    let back = Save::from_bytes(&bytes).unwrap();
    assert_eq!(back.journal.already(77), Some(outcome));
    assert_eq!(back.journal, j);
}

/// **Two witnesses cannot make two accidents.** The objective outcome
/// comes from the world's seed and the event, never from whoever
/// happened to be looking.
#[test]
fn two_witnesses_get_one_accident() {
    let mut alice = Journal::new();
    let mut bob = Journal::new();
    assert_eq!(alice.objective(4242, 900), bob.objective(4242, 900));

    // But what each of them makes of it is their own, and is not the
    // outcome — so two people can be wrong about it differently.
    let a = Journal::perception(1, 900);
    let b = Journal::perception(2, 900);
    assert_ne!(a, b, "everybody perceived it identically");
    assert_ne!(a, alice.objective(4242, 900), "a perception was just the fact");
}

/// **A perception is not written down.** It is derived, because it is
/// not a world fact — and a journal that recorded every witness's view
/// of every event would grow with attention rather than with history.
#[test]
fn perceptions_are_derived_and_history_is_recorded() {
    let mut j = Journal::new();
    for e in 0..50u64 {
        let _ = Journal::perception(7, e);
    }
    assert!(j.is_empty(), "merely looking at things wrote history");
    j.objective(1, 1);
    assert_eq!(j.len(), 1);
}

/// **A save costs what happened**, not how long anybody played.
#[test]
fn a_save_grows_with_history_and_not_with_time() {
    let quiet = Save {
        world_seed: 5,
        day: 365 * 200,
        people: vec![a_lived_life(5)],
        journal: Journal::new(),
    };
    let mut busy_journal = Journal::new();
    for e in 0..500u64 {
        busy_journal.objective(5, e);
    }
    let busy = Save { world_seed: 5, day: 10, people: vec![a_lived_life(5)], journal: busy_journal };

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
        Save { world_seed: 1, day: 0, people: vec![c.clone()], journal: Journal::new() }
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
    let bytes = Save { world_seed: 1, day: 1, people: vec![], journal: Journal::new() }.to_bytes();
    assert_eq!(&bytes[..8], MAGIC);
    assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), FORMAT);
}
