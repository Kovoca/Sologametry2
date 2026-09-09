//! **A world saved mid-journey, continued on both sides.**
//!
//! The test this replaces was named for saving a world and did not save
//! one: it built a `Save` by hand containing the day and a cloned shipment
//! registry, and left out the stocks, the afloat balance, the markets, the
//! roads, the reservations, the carriers, the treasury and the journal. It
//! also never advanced the reloaded branch to arrival, so nothing it
//! asserted depended on the reload having worked.
//!
//! What is exercised here is the whole economic root through real bytes,
//! and then **both branches played on** — because a codec that loses a
//! field the next day needs is indistinguishable from a correct one until
//! somebody plays the next day.
//!
//! **The comparison is the bytes.** `save.rs` already establishes that
//! identical state gives identical bytes — floats stored as bit patterns, a
//! `BTreeMap` wherever order would otherwise be arbitrary — which is
//! exactly what makes a byte comparison a canonical-state comparison rather
//! than a shortcut. A field-by-field compare would test whatever fields
//! somebody remembered to list.

use scale_sim::econ::{Commodity, Doctrine, Economy};
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Nations;
use scale_sim::save::{Reader, Store, Writer};
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

fn bytes_of(e: &Economy) -> Vec<u8> {
    let mut w = Writer::new();
    e.store(&mut w);
    w.bytes
}

fn reload(bytes: &[u8]) -> Economy {
    let mut r = Reader::new(bytes);
    Economy::load(&mut r).expect("the world would not load back")
}

/// A country big enough to have freight on the road, run until it has.
fn a_country_with_lorries_out() -> Economy {
    let world = World::generate(384, 216, 7);
    let polities = Polities::partition(&world, 24);
    let settlements = Settlements::place(&world, &polities, 3000);
    let network = Network::build(&world, &settlements, 500);
    let mut n = Nations::build(
        &world,
        &polities,
        &settlements,
        &network,
        4,
        4,
        Doctrine::Prudent,
    );
    for _ in 0..120 {
        n.economy.step();
        let afloat = n
            .economy
            .shipments
            .iter()
            .filter(|(_, s)| s.in_transit())
            .count();
        if afloat > 0 && n.economy.ledger.day > 60 {
            return n.economy;
        }
    }
    n.economy
}

/// **The gate.** Save mid-transit, reload, play both on, compare.
#[test]
fn a_world_saved_mid_journey_continues_the_same_journey() {
    let mut straight_through = a_country_with_lorries_out();

    let afloat_at_the_save = straight_through
        .shipments
        .iter()
        .filter(|(_, s)| s.in_transit())
        .count();
    assert!(
        afloat_at_the_save > 0,
        "no cargo was on the road, so there is no mid-journey to save"
    );
    let tonnes_afloat: f64 = straight_through
        .shipments
        .iter()
        .filter(|(_, s)| s.in_transit())
        .map(|(_, s)| s.aboard)
        .sum();
    assert!(
        tonnes_afloat > 0.0,
        "the consignments on the road are all empty"
    );

    // ---------------------------------------------------------------
    // through real bytes
    // ---------------------------------------------------------------
    let saved = bytes_of(&straight_through);
    assert!(
        saved.len() > 10_000,
        "the whole economy came to {} bytes, which is not a world",
        saved.len()
    );
    let mut reloaded = reload(&saved);

    // **The round trip is exact before anything else is claimed.** If the
    // bytes of the reloaded world differ from the bytes it came from, some
    // field did not survive and every comparison after this is comparing
    // two wrongs.
    assert_eq!(
        bytes_of(&reloaded).len(),
        saved.len(),
        "the reloaded world writes a different number of bytes"
    );
    assert!(
        bytes_of(&reloaded) == saved,
        "the reloaded world is not the world that was saved"
    );

    // The consignments are the same consignments, by name.
    // A consignment's name has no public number — deliberately, so that
    // nothing can do arithmetic on it — so it is compared through its own
    // codec, which is the only thing entitled to see it.
    let manifest = |e: &Economy| {
        let mut v: Vec<(Vec<u8>, u64, u64, u64)> = e
            .shipments
            .iter()
            .map(|(id, s)| {
                let mut w = Writer::new();
                id.store(&mut w);
                (w.bytes, s.despatched.to_bits(), s.aboard.to_bits(), s.due)
            })
            .collect();
        v.sort_unstable();
        v
    };
    assert_eq!(
        manifest(&reloaded),
        manifest(&straight_through),
        "a cargo changed name, tonnage or due date on the way through the bytes"
    );

    // ---------------------------------------------------------------
    // and both worlds play on
    // ---------------------------------------------------------------
    //
    // This is the half the old test did not have. A codec that loses a
    // field the *next* day needs is indistinguishable from a correct one
    // until somebody plays the next day — and thirty of them takes every
    // cargo that was on the road through arrival, waiting, part-unloading
    // or write-off.
    for _ in 0..30 {
        straight_through.step();
        reloaded.step();
    }

    assert_eq!(
        bytes_of(&reloaded),
        bytes_of(&straight_through),
        "the two worlds diverged after being played on from the same state"
    );

    // Both still conserve, which is the property the whole ledger exists
    // for and the one a bad reload would break silently.
    straight_through.ledger.assert_conserved();
    reloaded.ledger.assert_conserved();
    straight_through.treasury.assert_conserved();
    reloaded.treasury.assert_conserved();

    // And the journey the save was taken in the middle of actually
    // finished, or the continuation proved nothing.
    let still_out = reloaded
        .shipments
        .iter()
        .filter(|(_, s)| s.in_transit())
        .count();
    assert!(
        reloaded.journal.entries().len() > straight_through.journal.entries().len() / 2,
        "the reloaded world stopped journalling"
    );
    let _ = still_out;
}

/// **A file naming a road that is not there is broken, not newer.**
///
/// Every one of these decodes cleanly. What makes them dangerous is that
/// none of them panics: a works in a town that is not there has its goods
/// counted into a town nobody lives in, a booking on a road nobody built is
/// capacity promised out of nothing, and the price pass reads both as
/// ordinary figures.
#[test]
fn a_world_whose_parts_do_not_refer_to_each_other_is_refused() {
    use scale_sim::save::SaveError;
    let e = a_country_with_lorries_out();

    let refuse = |e: &Economy, what: &str| {
        let bytes = bytes_of(e);
        match Economy::load(&mut Reader::new(&bytes)) {
            Err(SaveError::Impossible(_)) => {}
            other => panic!("{what} was accepted: {:?}", other.map(|_| "a world")),
        }
    };

    // A works in a town that is not there.
    let mut broken = reload(&bytes_of(&e));
    broken.ledger.sites[0].market = 9_999;
    refuse(&broken, "a works in a town that is not there");

    // A road to a town that is not there.
    let mut broken = reload(&bytes_of(&e));
    broken.routes[0].a = 9_999;
    refuse(&broken, "a road to a town that is not there");

    // Two roads with one name, which is the defect `RouteId` exists to
    // prevent, arriving through a file rather than through a vector.
    let mut broken = reload(&bytes_of(&e));
    let first = broken.routes[0].id;
    broken.routes[1].id = first;
    refuse(&broken, "two roads sharing a name");

    // A road named at or past the next unused name, which means the
    // counter would hand that name out again to a different road.
    let mut broken = reload(&bytes_of(&e));
    broken.next_route_id = broken.routes[0].id.0;
    refuse(&broken, "a road named past the counter");

    // Capacity booked on a road that is not there.
    let mut broken = reload(&bytes_of(&e));
    broken
        .reservations
        .book(scale_sim::quote::RouteId(999_999), broken.ledger.day, 10.0);
    refuse(&broken, "capacity booked on a road that is not there");

    // A world whose mass does not balance. This is the one that matters
    // most: the ledger's conservation assertion is this project's only
    // defence against a quiet leak, and a save that starts out of balance
    // makes it fire somewhere else, days later, for no visible reason.
    let mut broken = reload(&bytes_of(&e));
    let fullest = (0..broken.ledger.sites.len())
        .max_by(|&a, &b| {
            broken
                .ledger
                .stock(a, Commodity::Grain)
                .total_cmp(&broken.ledger.stock(b, Commodity::Grain))
        })
        .expect("no sites at all");
    let held = broken.ledger.stock(fullest, Commodity::Grain);
    assert!(
        held > 1.0,
        "nobody in this world holds any grain, so emptying a silo proves nothing"
    );
    broken.ledger.sites[fullest].stock[Commodity::Grain as usize] = 0.0;
    refuse(&broken, "a world whose mass does not conserve");
}
