//! **Durable identity.** Spec Phase 1, first piece.
//!
//! Everything in this simulation is identified by a bare `usize` index
//! into a `Vec`, which works exactly as long as nothing ever moves.

use scale_sim::id::{Arena, Id};

#[derive(Debug, PartialEq)]
struct Mill(&'static str);

#[derive(Debug, PartialEq)]
struct Market(&'static str);

/// **A handle survives its neighbours moving.**
///
/// With a `Vec` index, removing anything re-points every index past it —
/// so a mill holding "market 4" silently becomes a mill trading at what
/// used to be market 5. Nothing here removes a site yet, which is the
/// only reason it has never bitten; the moment anything does, every
/// cross-reference in the economy is off by one and the ledger still
/// balances, because the tonnage went *somewhere*.
#[test]
fn removing_one_thing_does_not_repoint_the_others() {
    let mut mills: Arena<Mill> = Arena::new();
    let a = mills.add(Mill("Ashford"));
    let b = mills.add(Mill("Barrow"));
    let c = mills.add(Mill("Cowley"));

    assert_eq!(mills.remove(b), Some(Mill("Barrow")));

    // The handles either side still name what they always named.
    assert_eq!(mills[a], Mill("Ashford"));
    assert_eq!(mills[c], Mill("Cowley"));
    assert_eq!(mills.len(), 2);
}

/// **A stale handle is caught, not silently honoured.**
///
/// This is the whole reason for the generation counter. Reusing a slot is
/// what an arena is *for* — it keeps the storage compact — and it is
/// exactly what turns a dangling reference into a wrong answer: a mill
/// holding an identifier for a market that has gone would otherwise start
/// trading at whatever moved into the slot, and every conservation check
/// in the model would still pass, because the flour went somewhere.
#[test]
fn a_reused_slot_is_a_different_thing() {
    let mut markets: Arena<Market> = Arena::new();
    let gone = markets.add(Market("Penhurst"));
    markets.remove(gone);

    let fresh = markets.add(Market("Yarhaven"));

    // Same slot, and that is the point: the storage is reused.
    assert_eq!(gone.slot(), fresh.slot(), "the arena is not reusing slots");
    // But not the same identity.
    assert_ne!(gone, fresh);
    assert!(
        !markets.holds(gone),
        "a handle to a removed market still resolves"
    );
    assert!(markets.holds(fresh));
    assert_eq!(markets.get(gone), None);
    assert_eq!(markets[fresh], Market("Yarhaven"));
}

/// **A market identifier cannot be used as a mill identifier.**
///
/// Not testable as a failing compile from in here, so it is asserted the
/// other way round: the two are separate types carrying the same bits, so
/// the only way across is the explicit unpack the loader uses — one
/// function, in one place, where the re-assertion of type can be looked
/// at.
#[test]
fn the_bits_are_the_same_and_the_types_are_not() {
    let mut mills: Arena<Mill> = Arena::new();
    let mut markets: Arena<Market> = Arena::new();
    let m = mills.add(Mill("Ashford"));
    let k = markets.add(Market("Penhurst"));

    // Same slot, same generation, same bits — different types.
    assert_eq!(m.bits(), k.bits());
    // And `mills.get(k)` does not compile, which is the entire point.

    // A round trip through the save format keeps the identity.
    let back: Id<Mill> = Id::from_bits(m.bits());
    assert_eq!(back, m);
    assert_eq!(mills[back], Mill("Ashford"));
}

/// **Deterministic, because a seed has to rebuild the same world.**
///
/// Slots are handed out in order and reused oldest-first. If reuse were
/// last-in-first-out — the obvious implementation — two runs that removed
/// things in a different order would hand out different identifiers, and
/// a save would stop matching the world that wrote it.
#[test]
fn identifiers_are_handed_out_the_same_way_every_time() {
    let build = || {
        let mut a: Arena<Mill> = Arena::new();
        let one = a.add(Mill("one"));
        let two = a.add(Mill("two"));
        let three = a.add(Mill("three"));
        a.remove(one);
        a.remove(two);
        let four = a.add(Mill("four"));
        let five = a.add(Mill("five"));
        (three.bits(), four.bits(), five.bits())
    };
    assert_eq!(build(), build());

    // Oldest free slot first: "four" takes the slot "one" vacated.
    let (_, four, five) = build();
    assert_eq!(Id::<Mill>::from_bits(four).slot(), 0);
    assert_eq!(Id::<Mill>::from_bits(five).slot(), 1);
}

/// Iteration skips the holes and hands back handles that resolve.
#[test]
fn iterating_gives_handles_that_work() {
    let mut a: Arena<Mill> = Arena::new();
    let one = a.add(Mill("one"));
    a.add(Mill("two"));
    a.remove(one);
    a.add(Mill("three"));

    let names: Vec<&str> = a.values().map(|m| m.0).collect();
    assert_eq!(names, vec!["three", "two"], "slot order, holes filled");

    for (id, mill) in a.iter() {
        assert_eq!(a[id].0, mill.0, "a handle from iteration does not resolve");
    }
    assert_eq!(a.ids().count(), a.len());
}
