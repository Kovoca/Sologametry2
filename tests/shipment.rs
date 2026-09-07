//! **A cargo is somewhere, and it takes time to get anywhere.**
//!
//! Freight used to be one statement: tonnes off a shed here, tonnes on a
//! shelf there, in the same breath. These are the things that could not be
//! said while that was true.

use scale_sim::econ::{Commodity, Economy};
use scale_sim::econ::{Doctrine, Event};
use scale_sim::registry::{Lookup, Registry};
use scale_sim::save::{Reader, Store, Writer};
use scale_sim::shipment::{days_on_the_road, Leg, Loss, Shipment, KM_PER_DAY};
use scale_sim::slice;

const ASHFORD_STORE: usize = 6; // the market hall
const BEXLEY_STORE: usize = 7; // the general store

fn world() -> Economy {
    let mut e = slice::build(Doctrine::Prudent);
    // A few days so prices, cover and landed costs are real numbers rather
    // than whatever the fixture happened to be built with.
    for _ in 0..10 {
        e.step();
    }
    e
}

/// Somewhere that has some of something, and somewhere that wants it.
///
/// **Stock is brought into existence through the journal**, not written
/// into a site — this project has exactly one write path and a test is not
/// exempt from it. Setting `stock` by hand would leave the conservation
/// check comparing a holding against a journal that never saw the goods
/// arrive, so every assertion below would be measured against a broken
/// baseline.
fn a_load(e: &mut Economy) -> (usize, usize, Commodity, f64) {
    let c = Commodity::ProcessedFood;
    let from = ASHFORD_STORE;
    let to = BEXLEY_STORE;
    e.ledger.sites[from].capacity[c as usize] = 4_000.0;
    e.ledger.sites[to].capacity[c as usize] = 2_000.0;
    stock_up(e, from, c, 200.0);
    (from, to, c, 60.0)
}

/// Put a definite amount at a site, through the one write path.
fn stock_up(e: &mut Economy, site: usize, c: Commodity, want: f64) {
    let have = e.ledger.stock(site, c);
    if want > have {
        let qty = want - have;
        e.ledger.apply(
            &mut e.journal,
            Event::Produced { site, commodity: c, qty },
        );
    } else if have > want {
        let qty = have - want;
        e.ledger.apply(
            &mut e.journal,
            Event::Consumed {
                site,
                commodity: c,
                qty,
                reason: scale_sim::econ::Use::Input,
            },
        );
    }
}

/// **Most freight is same-day, and that is not a degenerate case.**
///
/// The average British road haul is about 94 km, which a lorry does and
/// comes home from before tea. A model that made every delivery an
/// overnight saga would be wrong about the common case in order to be
/// right about the rare one.
#[test]
fn a_short_haul_is_collected_and_tipped_the_same_day() {
    assert_eq!(days_on_the_road(94.0), 0, "the average British haul now sleeps out");
    assert_eq!(days_on_the_road(0.0), 0);
    assert_eq!(days_on_the_road(KM_PER_DAY - 1.0), 0);
    assert_eq!(days_on_the_road(KM_PER_DAY), 1);
    assert_eq!(days_on_the_road(1_300.0), 2, "a two-day haul is two days");
}

/// **Goods on the road are still in the country.**
///
/// The gate the whole design turns on. A cargo that has left the seller
/// and not reached the buyer has to be *somewhere*, or a haul that crosses
/// midnight destroys its load and creates it again in the morning — and
/// the conservation check, which has caught every leak in this model so
/// far, would not notice.
#[test]
fn a_cargo_on_the_road_has_not_stopped_existing() {
    let mut e = world();
    let (from, to, c, qty) = a_load(&mut e);
    let before = e.ledger.total(c);
    let at_seller = e.ledger.stock(from, c);

    let id = e.consign(from, to, 0, c, qty, 2_000.0, false).expect("nothing set off");
    e.ledger.assert_conserved();

    // It has left the seller.
    assert!(
        (e.ledger.stock(from, c) - (at_seller - qty)).abs() < 1e-9,
        "the seller still has goods that are on a lorry"
    );
    // It has not reached the buyer.
    assert!(e.shipments.get(id).unwrap().delivered < 1e-9);
    // And it is still in the country.
    assert!(e.afloat(c) >= qty - 1e-9, "{} afloat, {qty} despatched", e.afloat(c));
    assert!(
        (e.ledger.total(c) - before).abs() < 1e-6,
        "tonnage changed by putting it on a lorry: {before} -> {}",
        e.ledger.total(c)
    );
}

/// **A contract is struck before it is performed.**
///
/// What was agreed on Monday is what is settled on Thursday, whatever the
/// price did in between. That is most of what a forward price *is*, and it
/// cannot be said at all while buying and delivering are one instruction.
#[test]
fn the_price_moves_and_the_contract_does_not() {
    let mut e = world();
    let (from, to, c, qty) = a_load(&mut e);
    let from_market = e.ledger.sites[from].market;

    let id = e.consign(from, to, 0, c, qty, 1_300.0, false).expect("nothing set off");
    let agreed = e.shipments.get(id).unwrap().goods;
    let per_tonne = e.shipments.get(id).unwrap().landed_per_tonne();
    assert!(agreed > 0.0, "a consignment left with no contract price");

    // The market triples under the lorry.
    e.markets[from_market].landed[c as usize] *= 3.0;
    e.markets[from_market].price[c as usize] *= 3.0;
    for _ in 0..3 {
        e.step();
    }

    let done = match e.shipments.look(id) {
        Lookup::Live(s) => s.clone(),
        Lookup::Gone(_) => {
            // Delivered and buried, which is the ordinary outcome. What
            // matters is what the journal recorded when it landed.
            let landed: f64 = e
                .journal
                .entries()
                .iter()
                .filter_map(|entry| match &entry.event {
                    Event::Landed { shipment, paid, qty, .. }
                        if *shipment == id =>
                    {
                        Some(paid / qty)
                    }
                    _ => None,
                })
                .next()
                .expect("it was buried without ever landing");
            assert!(
                (landed - agreed / qty).abs() < 1e-6,
                "settled at {landed} a tonne against a contract of {}",
                agreed / qty
            );
            return;
        }
        Lookup::Unknown => panic!("the consignment was forgotten mid-journey"),
    };
    assert!(
        (done.goods - agreed).abs() < 1e-9,
        "the contract was rewritten in transit: {agreed} -> {}",
        done.goods
    );
    assert!((done.landed_per_tonne() - per_tonne).abs() < 1e-9);
}

/// **A cargo halfway there survives a save.**
///
/// The gate the previous commit could not write, because there was no
/// transit to be halfway through.
#[test]
fn a_lorry_on_the_road_is_the_same_lorry_after_a_reload() {
    let mut e = world();
    let (from, to, c, qty) = a_load(&mut e);
    let id = e.consign(from, to, 3, c, qty, 2_000.0, true).expect("nothing set off");
    let before = e.shipments.get(id).cloned().expect("it did not exist");

    // Through actual bytes.
    let mut w = Writer::new();
    e.shipments.store(&mut w);
    let mut r = Reader::new(&w.bytes);
    let back: Registry<Shipment> = Registry::load(&mut r).expect("it would not read back");
    assert!(r.done(), "the codec left bytes behind");

    let after = back.get(id).expect("the cargo was not there after the reload");
    assert_eq!(&before, after, "the consignment changed on the way through a save");
    assert_eq!(after.consignee, to);
    assert_eq!(after.leg, Leg::OnTheRoad);
    assert!(after.refrigerated, "the reefer came back as a flatbed");

    // **And the world it came back into does not reuse its name.**
    let mut back = back;
    let fresh = back.add(before.clone());
    assert_ne!(fresh, id);
}

/// **Room is checked on arrival, not only when the load was planned.**
///
/// Between the lorry leaving and the lorry arriving, somebody may have
/// filled the shed it was going into. The same rule `schedule.rs` had to
/// learn about finished work.
#[test]
fn a_lorry_cannot_tip_into_a_shed_that_filled_while_it_was_driving() {
    let mut e = world();
    let (from, to, c, qty) = a_load(&mut e);
    let id = e.consign(from, to, 0, c, qty, 1_300.0, false).expect("nothing set off");

    // The consignee's shed fills while the lorry is on the road.
    e.ledger.sites[to].capacity[c as usize] = e.ledger.stock(to, c);

    let off = e.tip(id);
    assert!(off < 1e-9, "{off} t went into a full shed");
    assert_eq!(e.shipments.get(id).unwrap().leg, Leg::Waiting);
    assert!(e.afloat(c) >= qty - 1e-9, "the cargo evaporated at the gate");
    e.ledger.assert_conserved();

    // Room appears, and the same lorry tips.
    e.ledger.sites[to].capacity[c as usize] += qty * 2.0;
    let off = e.tip(id);
    assert!((off - qty).abs() < 1e-6, "tipped {off} of {qty}");
    assert!(matches!(e.shipments.look(id), Lookup::Gone(t) if t.how.contains("delivered")));
}

/// **A load nobody can take is not left on a lorry for a month.**
///
/// Unbounded waiting is unbounded state, which this project has had to
/// remove three times. After a few days at a full bay a real carrier stops
/// waiting: the goods go into whatever store in that town will have them,
/// and if there is genuinely nowhere, they are written off.
#[test]
fn a_load_that_cannot_be_tipped_does_not_wait_for_ever() {
    let mut e = world();
    let (from, to, c, qty) = a_load(&mut e);
    // Nowhere in the whole destination market can take it.
    for i in 0..e.ledger.sites.len() {
        if e.ledger.sites[i].market == e.ledger.sites[to].market {
            e.ledger.sites[i].capacity[c as usize] = e.ledger.stock(i, c);
        }
    }
    let id = e.consign(from, to, 0, c, qty, 700.0, false).expect("nothing set off");

    for _ in 0..12 {
        e.step();
        e.ledger.assert_conserved();
    }
    assert!(
        !matches!(e.shipments.look(id), Lookup::Live(s) if s.in_transit()),
        "it is still standing at the bay a fortnight later"
    );
    assert!(e.afloat(c) < 1e-6, "{} t is still afloat", e.afloat(c));
    assert!(e.shipments.len() < 50, "the registry is filling up with stuck loads");
}

/// **A lorry is a store like any other, and refrigeration is the whole
/// difference.**
///
/// Nothing new is invented here: the model already knows how fast each
/// commodity leaks out of a store. What a journey adds is that the clock
/// runs while nobody can do anything about it — which is exactly why the
/// meat trade did not exist before the *Dunedin*.
#[test]
fn meat_rots_on_the_road_unless_the_lorry_is_cold() {
    let c = Commodity::Meat;
    let mut warm = world();
    let mut cold = world();
    for e in [&mut warm, &mut cold] {
        e.ledger.sites[ASHFORD_STORE].capacity[c as usize] = 200.0;
        e.ledger.sites[BEXLEY_STORE].capacity[c as usize] = 500.0;
        stock_up(e, ASHFORD_STORE, c, 100.0);
    }
    let a = warm
        .consign(ASHFORD_STORE, BEXLEY_STORE, 0, c, 80.0, 2_000.0, false)
        .expect("nothing set off");
    let b = cold
        .consign(ASHFORD_STORE, BEXLEY_STORE, 0, c, 80.0, 2_000.0, true)
        .expect("nothing set off");

    for _ in 0..3 {
        warm.step();
        cold.step();
    }
    let lost_warm: f64 = warm
        .journal
        .entries()
        .iter()
        .filter_map(|e| match &e.event {
            Event::LostInTransit { shipment, qty, how, .. }
                if *shipment == a && *how == Loss::Spoiled =>
            {
                Some(*qty)
            }
            _ => None,
        })
        .sum();
    let lost_cold: f64 = cold
        .journal
        .entries()
        .iter()
        .filter_map(|e| match &e.event {
            Event::LostInTransit { shipment, qty, how, .. }
                if *shipment == b && *how == Loss::Spoiled =>
            {
                Some(*qty)
            }
            _ => None,
        })
        .sum();

    assert!(lost_warm > 1e-6, "meat spent three days on an open lorry and nothing happened");
    assert!(
        lost_cold < lost_warm * 0.5,
        "refrigeration bought nothing: {lost_cold} against {lost_warm}"
    );
    warm.ledger.assert_conserved();
    cold.ledger.assert_conserved();
}

/// **The carrier is paid for what arrived, not for what set off.**
///
/// Which is also why a haulier's money comes in later than the work does,
/// and is a real reason small ones run out of it.
#[test]
fn nobody_is_paid_for_a_load_that_is_still_moving() {
    let mut e = world();
    let (from, to, c, qty) = a_load(&mut e);
    let market = e.ledger.sites[to].market;
    let paid_before = e.treasury.balance(scale_sim::money::Account::ServiceSector(market));

    let id = e.consign(from, to, 0, c, qty, 1_300.0, false).expect("nothing set off");
    assert!(
        e.shipments.get(id).unwrap().freight > 0.0,
        "a two-day haul was contracted for nothing"
    );
    let paid_afloat = e.treasury.balance(scale_sim::money::Account::ServiceSector(market));
    assert!(
        (paid_afloat - paid_before).abs() < 1e-9,
        "the carrier was paid before the goods arrived"
    );

    e.tip(id);
    let paid_after = e.treasury.balance(scale_sim::money::Account::ServiceSector(market));
    assert!(paid_after > paid_before, "nobody was paid for delivering it");
}

/// **One consignment, one name, and a grave when it is done with.**
///
/// The journal goes on naming a cargo long after it has been tipped, which
/// is why "never heard of it" and "delivered on day 214" are different
/// answers.
#[test]
fn a_consignment_keeps_one_name_and_leaves_a_grave() {
    let mut e = world();
    let (from, to, c, qty) = a_load(&mut e);
    let id = e.consign(from, to, 0, c, qty, 100.0, false).expect("nothing set off");
    let day = e.ledger.day;

    assert!(matches!(e.shipments.look(id), Lookup::Live(_)));
    e.tip(id);
    match e.shipments.look(id) {
        Lookup::Gone(t) => assert_eq!(t.day, day),
        other => panic!("a delivered cargo reads as {other:?}"),
    }

    // The journal still names it, both ends of the journey.
    let named: Vec<&str> = e
        .journal
        .entries()
        .iter()
        .filter_map(|entry| match &entry.event {
            Event::Despatched { shipment, .. } if *shipment == id => {
                Some("despatched")
            }
            Event::Landed { shipment, .. } if *shipment == id => Some("landed"),
            _ => None,
        })
        .collect();
    assert_eq!(named, vec!["despatched", "landed"]);
}

/// **A cargo that set off cannot be sold again.**
///
/// The same rule `fitted.rs` keeps for an alternator that is in a van and
/// in a stockroom at once: the state must be impossible, not merely
/// avoided. Here the ledger enforces it — the tonnes are off the seller's
/// books the moment the lorry pulls out.
#[test]
fn what_is_on_the_lorry_is_not_still_on_the_shelf() {
    let mut e = world();
    let (from, to, c, _) = a_load(&mut e);
    let have = e.ledger.stock(from, c);

    // Try to send more than there is, twice.
    let first = e.consign(from, to, 0, c, have * 0.75, 1_300.0, false);
    let second = e.consign(from, to, 0, c, have * 0.75, 1_300.0, false);
    assert!(first.is_some() && second.is_some());

    let sent: f64 = [first, second]
        .iter()
        .flatten()
        .map(|&id| e.shipments.get(id).unwrap().despatched)
        .sum();
    assert!(
        sent <= have + 1e-6,
        "sent {sent} t of something there were only {have} t of"
    );
    assert!(e.ledger.stock(from, c) >= -1e-9, "the seller went negative");
    e.ledger.assert_conserved();
}

/// **Graves are pruned, and the names still do not come back.**
///
/// A registry keeping a tombstone for every consignment ever delivered
/// would grow with history rather than with the world. Pruning is safe
/// here for exactly one reason, and it is the one the registry's own gate
/// asserts: the counter is written down rather than derived.
#[test]
fn a_year_of_deliveries_does_not_fill_the_registry() {
    let mut e = world();
    let mut names = Vec::new();
    for _ in 0..200 {
        let (from, to, c, qty) = a_load(&mut e);
        if let Some(id) = e.consign(from, to, 0, c, qty, 50.0, false) {
            e.tip(id);
            names.push(id);
        }
        e.step();
    }
    assert!(
        e.shipments.buried() < 150,
        "{} graves after two hundred deliveries",
        e.shipments.buried()
    );
    // The oldest are forgotten, which is the point — and a forgotten name
    // is still not handed to anybody else.
    let (from, to, c, qty) = a_load(&mut e);
    let fresh = e.consign(from, to, 0, c, qty, 50.0, false).unwrap();
    assert!(!names.contains(&fresh), "a pruned grave let a name be reissued");
    e.ledger.assert_conserved();
}

/// **The transit machinery is not dark.**
///
/// A feature the real economy never reaches is a feature that rots, and
/// the hand-built fixture above drives `consign` directly — so on its own
/// it proves the mechanism works and nothing about whether anything uses
/// it. This runs a generated planet and measures.
///
/// **Four fifths of consignments sleep on the road, and that is not a
/// contradiction of the 94 km average haul.** Local distribution — grain
/// to the mill, flour to the cannery, tins to the shops — never becomes a
/// consignment at all; it is `econ::distribute` pulling from down the
/// street. What a carrier gets is what could not be had locally, which is
/// the long end of the distribution by construction. The short hauls are
/// real and are simply somebody else's code.
#[test]
fn a_generated_world_actually_puts_things_on_the_road_overnight() {
    use scale_sim::network::Network;
    use scale_sim::polity::Polities;
    use scale_sim::region::Nations;
    use scale_sim::settlement::Settlements;
    use scale_sim::world::World;

    let world = World::generate(384, 216, 7);
    let polities = Polities::partition(&world, 24);
    let settlements = Settlements::place(&world, &polities, 3000);
    let network = Network::build(&world, &settlements, 500);
    let mut n =
        Nations::build(&world, &polities, &settlements, &network, 4, 4, Doctrine::Prudent);

    let mut slept_out = 0usize;
    let mut longest = 0u64;
    let mut peak_afloat = 0.0f64;
    let mut seen: std::collections::BTreeSet<u64> = Default::default();
    for _ in 0..120 {
        n.economy.step();
        for (k, s) in n.economy.shipments.iter() {
            if seen.insert(k.raw()) && s.due > s.left {
                slept_out += 1;
                longest = longest.max(s.due - s.left);
            }
        }
        for &c in Commodity::ALL.iter() {
            peak_afloat = peak_afloat.max(n.economy.afloat(c));
        }
    }
    let raised = n.economy.shipments.ever();

    assert!(raised > 500, "only {raised} consignments in four months of four nations");
    assert!(slept_out > 100, "nothing at all spent a night on the road ({slept_out})");
    assert!(longest >= 2, "the longest journey in the world was {longest} days");
    assert!(peak_afloat > 0.0, "no goods were ever in transit");

    // **And the registry does not grow with history.** Graves are pruned;
    // what is live is what is actually moving.
    assert!(
        (n.economy.shipments.len() as u64) < raised / 4,
        "{} live consignments against {raised} ever raised",
        n.economy.shipments.len()
    );
    n.economy.ledger.assert_conserved();
}
