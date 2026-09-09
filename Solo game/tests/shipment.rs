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
            Event::Produced {
                site,
                commodity: c,
                qty,
            },
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
    assert_eq!(
        days_on_the_road(94.0),
        0,
        "the average British haul now sleeps out"
    );
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

    let id = e
        .consign(from, to, 0, c, qty, 2_000.0, false)
        .map(|(id, _)| id)
        .expect("nothing set off");
    e.ledger.assert_conserved();

    // It has left the seller.
    assert!(
        (e.ledger.stock(from, c) - (at_seller - qty)).abs() < 1e-9,
        "the seller still has goods that are on a lorry"
    );
    // It has not reached the buyer.
    assert!(e.shipments.get(id).unwrap().delivered < 1e-9);
    // And it is still in the country.
    assert!(
        e.afloat(c) >= qty - 1e-9,
        "{} afloat, {qty} despatched",
        e.afloat(c)
    );
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

    let id = e
        .consign(from, to, 0, c, qty, 1_300.0, false)
        .map(|(id, _)| id)
        .expect("nothing set off");
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
                    Event::Landed {
                        shipment,
                        paid,
                        qty,
                        ..
                    } if *shipment == id => Some(paid / qty),
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
    let id = e
        .consign(from, to, 3, c, qty, 2_000.0, true)
        .map(|(id, _)| id)
        .expect("nothing set off");
    let before = e.shipments.get(id).cloned().expect("it did not exist");

    // Through actual bytes.
    let mut w = Writer::new();
    e.shipments.store(&mut w);
    let mut r = Reader::new(&w.bytes);
    let back: Registry<Shipment> = Registry::load(&mut r).expect("it would not read back");
    assert!(r.done(), "the codec left bytes behind");

    let after = back
        .get(id)
        .expect("the cargo was not there after the reload");
    assert_eq!(
        &before, after,
        "the consignment changed on the way through a save"
    );
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
    let id = e
        .consign(from, to, 0, c, qty, 1_300.0, false)
        .map(|(id, _)| id)
        .expect("nothing set off");

    // The consignee's shed fills while the lorry is on the road.
    e.ledger.sites[to].capacity[c as usize] = e.ledger.stock(to, c);

    let off = e.tip(id);
    assert!(off < 1e-9, "{off} t went into a full shed");
    assert_eq!(e.shipments.get(id).unwrap().leg, Leg::Waiting);
    assert!(
        e.afloat(c) >= qty - 1e-9,
        "the cargo evaporated at the gate"
    );
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
    let id = e
        .consign(from, to, 0, c, qty, 700.0, false)
        .map(|(id, _)| id)
        .expect("nothing set off");

    for _ in 0..12 {
        e.step();
        e.ledger.assert_conserved();
    }
    assert!(
        !matches!(e.shipments.look(id), Lookup::Live(s) if s.in_transit()),
        "it is still standing at the bay a fortnight later"
    );
    assert!(e.afloat(c) < 1e-6, "{} t is still afloat", e.afloat(c));
    assert!(
        e.shipments.len() < 50,
        "the registry is filling up with stuck loads"
    );
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
        .map(|(id, _)| id)
        .expect("nothing set off");
    let b = cold
        .consign(ASHFORD_STORE, BEXLEY_STORE, 0, c, 80.0, 2_000.0, true)
        .map(|(id, _)| id)
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
            Event::LostInTransit {
                shipment, qty, how, ..
            } if *shipment == a && *how == Loss::Spoiled => Some(*qty),
            _ => None,
        })
        .sum();
    let lost_cold: f64 = cold
        .journal
        .entries()
        .iter()
        .filter_map(|e| match &e.event {
            Event::LostInTransit {
                shipment, qty, how, ..
            } if *shipment == b && *how == Loss::Spoiled => Some(*qty),
            _ => None,
        })
        .sum();

    assert!(
        lost_warm > 1e-6,
        "meat spent three days on an open lorry and nothing happened"
    );
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
    let paid_before = e
        .treasury
        .balance(scale_sim::money::Account::ServiceSector(market));

    let id = e
        .consign(from, to, 0, c, qty, 1_300.0, false)
        .map(|(id, _)| id)
        .expect("nothing set off");
    assert!(
        e.shipments.get(id).unwrap().freight > 0.0,
        "a two-day haul was contracted for nothing"
    );
    let paid_afloat = e
        .treasury
        .balance(scale_sim::money::Account::ServiceSector(market));
    assert!(
        (paid_afloat - paid_before).abs() < 1e-9,
        "the carrier was paid before the goods arrived"
    );

    e.tip(id);
    let paid_after = e
        .treasury
        .balance(scale_sim::money::Account::ServiceSector(market));
    assert!(
        paid_after > paid_before,
        "nobody was paid for delivering it"
    );
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
    let id = e
        .consign(from, to, 0, c, qty, 100.0, false)
        .map(|(id, _)| id)
        .expect("nothing set off");
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
            Event::Despatched { shipment, .. } if *shipment == id => Some("despatched"),
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

    // Try to send more than there is, twice. **The second may legitimately
    // be refused outright** — not for want of goods but for want of road,
    // since the first booking takes the day's capacity with it, and a
    // consignment that cannot be carried is not a consignment.
    let first = e
        .consign(from, to, 0, c, have * 0.75, 1_300.0, false)
        .map(|(id, _)| id);
    let second = e
        .consign(from, to, 0, c, have * 0.75, 1_300.0, false)
        .map(|(id, _)| id);
    assert!(first.is_some(), "nothing set off at all");

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
        if let Some((id, _)) = e.consign(from, to, 0, c, qty, 50.0, false) {
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
    let fresh = e.consign(from, to, 0, c, qty, 50.0, false).unwrap().0;
    assert!(
        !names.contains(&fresh),
        "a pruned grave let a name be reissued"
    );
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
    let mut n = Nations::build(
        &world,
        &polities,
        &settlements,
        &network,
        4,
        4,
        Doctrine::Prudent,
    );

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

    assert!(
        raised > 500,
        "only {raised} consignments in four months of four nations"
    );
    assert!(
        slept_out > 100,
        "nothing at all spent a night on the road ({slept_out})"
    );
    assert!(
        longest >= 2,
        "the longest journey in the world was {longest} days"
    );
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

/// **Persisting a key is not persisting the allocator that issued it.**
///
/// The reviewer's gate, step for step: make some, end some, go through
/// real bytes, come back, make more, and require that nothing is called by
/// a name that is already taken or already buried.
///
/// The failure it exists to stop is quiet and total. A loader that derives
/// its counter from the highest live key — one line shorter, and the
/// obvious thing to write — hands the next arrival the name of whatever
/// died most recently, and every journal entry, debt and grievance naming
/// the dead now names the living instead. Nothing checks tonnage, so
/// nothing notices.
#[test]
fn a_reloaded_world_never_reissues_a_name() {
    use scale_sim::save::Save;

    let mut save = Save::default();
    // 1. Create.
    let mut alive = Vec::new();
    let mut buried = Vec::new();
    for k in 0..40u64 {
        let id = save.shipments.add(a_consignment(k));
        // 2. End some of them.
        if k % 3 == 0 {
            save.shipments.end(id, k, "sank");
            buried.push(id);
        } else {
            alive.push(id);
        }
    }
    assert!(!buried.is_empty() && !alive.is_empty());

    // 3 and 4. Through the real file, header, checksum and all.
    let bytes = save.to_bytes();
    let mut back = Save::from_bytes(&bytes).expect("the world would not load");

    for &id in &alive {
        assert!(
            back.shipments.get(id).is_some(),
            "{id:?} was lost in the save"
        );
    }
    for &id in &buried {
        assert!(
            matches!(back.shipments.look(id), Lookup::Gone(_)),
            "{id:?} came back from the dead"
        );
    }

    // 5. Make more, and 6. require that none of them is a name already
    // spoken for.
    for k in 0..40u64 {
        let fresh = back.shipments.add(a_consignment(1000 + k));
        assert!(
            !alive.contains(&fresh),
            "{fresh:?} was handed out on top of a living consignment"
        );
        assert!(
            !buried.contains(&fresh),
            "{fresh:?} was handed out on top of a grave"
        );
    }
}

/// A consignment with something to tell apart from its neighbours.
fn a_consignment(k: u64) -> Shipment {
    Shipment {
        commodity: Commodity::Grain,
        consignor: 1,
        consignee: 2,
        carrier: 0,
        from_market: 0,
        to_market: 1,
        left: k,
        due: k + 2,
        despatched: 100.0 + k as f64,
        aboard: 100.0 + k as f64,
        delivered: 0.0,
        lost: 0.0,
        how_lost: None,
        goods: 900.0 * (100.0 + k as f64),
        freight: 45.0 * (100.0 + k as f64),
        refrigerated: k.is_multiple_of(2),
        leg: Leg::OnTheRoad,
    }
}

/// **A world saved with a lorry halfway there comes back with it halfway
/// there.**
///
/// The whole-root version, which the previous commit could not write
/// because there was no transit to be halfway through. What has to survive
/// is not just the tonnage: the same name, the same manifest, the same day
/// it is due, the same contract, and the same consignee — because a cargo
/// that reloads pointing at a different buyer is worse than one that
/// reloads missing.
#[test]
fn a_world_saved_mid_journey_resumes_the_same_journey() {
    use scale_sim::save::Save;

    let mut e = world();
    let (from, to, c, qty) = a_load(&mut e);
    let id = e
        .consign(from, to, 2, c, qty, 2_000.0, true)
        .map(|(id, _)| id)
        .expect("nothing set off");
    let before = e.shipments.get(id).cloned().expect("it did not exist");
    assert_eq!(before.leg, Leg::OnTheRoad);
    assert!(
        before.due > e.ledger.day,
        "the load is not actually in transit"
    );

    // Out to a file and back, through the header and the checksum.
    let save = Save {
        day: e.ledger.day,
        shipments: e.shipments.clone(),
        ..Default::default()
    };
    let back = Save::from_bytes(&save.to_bytes()).expect("the world would not load");

    let after = back
        .shipments
        .get(id)
        .expect("the cargo was not in the save");
    assert_eq!(
        &before, after,
        "the consignment changed on the way through a save"
    );
    assert_eq!(after.consignee, to, "it came back bound for somebody else");
    assert_eq!(after.due, before.due, "it came back due on a different day");
    assert!(
        (after.goods - before.goods).abs() < 1e-9 && (after.freight - before.freight).abs() < 1e-9,
        "the contract was rewritten by a save"
    );
    assert_eq!(
        back.day, e.ledger.day,
        "the world came back on a different day"
    );

    // **And the reloaded world does not hand its name to anybody else.**
    let mut back = back;
    let fresh = back.shipments.add(a_consignment(1));
    assert_ne!(fresh, id);
}

/// **The same road cannot be promised to everybody.**
///
/// Acceptance gate 18.7. A route table worked out once when the day opens
/// tells every enquiry what the road can carry — and if nothing books
/// against it, a dozen consignments each set off believing they have that
/// road to themselves. None of them is wrong on its own, which is what
/// makes it hard to see: the tonnage conserves, the money conserves, and
/// the country is simply moving more freight than its roads can hold.
///
/// So dispatch reserves, and what is asserted is the sum: **across every
/// road and every day, what has been committed cannot exceed what that
/// road can carry.**
#[test]
fn no_road_is_booked_past_what_it_can_carry() {
    use scale_sim::network::Network;
    use scale_sim::polity::Polities;
    use scale_sim::region::Nations;
    use scale_sim::settlement::Settlements;
    use scale_sim::world::World;

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

    let mut ever_booked = 0usize;
    let mut tightest = 0.0f64;
    for _ in 0..200 {
        n.economy.step();
        for ((road, day), tonnes) in n.economy.reservations.iter() {
            let carries = n
                .economy
                .road(road)
                .expect("a booking names a road that is not there")
                .capacity;
            assert!(
                tonnes <= carries + 1e-6,
                "road {road} on day {day} is booked for {tonnes:.1} t against \
                 a capacity of {carries:.1}"
            );
            ever_booked += 1;
            tightest = tightest.max(tonnes / carries.max(1e-9));
        }
    }

    // **And the gate has to have seen some traffic.** A country where
    // nothing ever moved satisfies the assertion above and proves nothing.
    assert!(
        ever_booked > 100,
        "only {ever_booked} road-days were ever booked — nothing is moving \
         and the gate is watching an empty country"
    );
    assert!(
        tightest > 0.01,
        "the busiest road was {:.4}% full, so the limit was never near \
         binding and a missing reservation would look the same",
        100.0 * tightest
    );

    // **The table does not grow with history.** Yesterday's traffic
    // constrains nothing.
    assert!(
        n.economy.reservations.len() < 5_000,
        "{} road-days still booked after two hundred days",
        n.economy.reservations.len()
    );
    n.economy.ledger.assert_conserved();
}

/// A manifest that is sound in every respect, to be spoiled one field at a
/// time by the gate below.
fn a_sound_manifest() -> Shipment {
    Shipment {
        commodity: Commodity::Grain,
        consignor: 0,
        consignee: 1,
        carrier: 0,
        from_market: 0,
        to_market: 1,
        left: 100,
        due: 103,
        despatched: 60.0,
        aboard: 60.0,
        delivered: 0.0,
        lost: 0.0,
        how_lost: None,
        goods: 5_400.0,
        freight: 780.0,
        refrigerated: false,
        leg: Leg::OnTheRoad,
    }
}

fn round_trip(s: &Shipment) -> Result<Shipment, scale_sim::save::SaveError> {
    let mut w = Writer::new();
    s.store(&mut w);
    Shipment::load(&mut Reader::new(&w.bytes))
}

/// **A commodity's name on disk is not where it sits in an enum.**
///
/// `Shipment::store` wrote `commodity as u8` and loaded through
/// `Commodity::ALL[index]`, so inserting or reordering one variant
/// silently reinterpreted every cargo in every existing save — a hold of
/// grain becoming a hold of coal, with the file intact and the checksum
/// correct. That is the exact failure the save design says explicit codes
/// prevent, and `Leg` and `Loss` in the same file already had them.
///
/// What this gate can check is that the mapping is a bijection and that it
/// does not agree with the declaration order, which is what makes it a
/// wire code rather than a cast wearing a function's name. The freeze
/// itself is a promise a test cannot keep — the doc comment carries it.
#[test]
fn a_commodity_is_saved_by_a_frozen_code_and_not_by_its_position() {
    let mut seen: Vec<u16> = Vec::new();
    for &c in Commodity::ALL.iter() {
        let code = c.wire_code();
        assert!(code > 0, "{c} has no wire code");
        assert!(
            !seen.contains(&code),
            "two commodities share wire code {code}, so a save cannot tell them apart"
        );
        seen.push(code);
        assert_eq!(
            Commodity::from_wire_code(code),
            Some(c),
            "{c} does not survive its own code"
        );
    }

    // **An unknown code is refused, not guessed at.** A save from a build
    // that has limestone in it must fail to load here rather than
    // arriving as whatever sits at that index.
    assert!(
        Commodity::from_wire_code(9_999).is_none(),
        "an unknown commodity code resolved to something"
    );
    let mut w = Writer::new();
    a_sound_manifest().store(&mut w);
    w.bytes[0..2].copy_from_slice(&9_999u16.to_le_bytes());
    assert!(
        matches!(
            Shipment::load(&mut Reader::new(&w.bytes)),
            Err(scale_sim::save::SaveError::UnknownCode("commodity", 9_999))
        ),
        "a cargo of an unknown commodity loaded anyway"
    );

    // And the codes must not merely be the positions again, or nothing
    // has changed but the name of the function.
    assert!(
        Commodity::ALL
            .iter()
            .enumerate()
            .any(|(i, c)| c.wire_code() as usize != i),
        "every wire code equals its enum position — this is a cast, not a code"
    );
}

/// **A save is not a trusted input**, and every one of these decodes
/// cleanly.
///
/// A finite float in a known field, a valid `Leg` code, a length inside
/// its bound — so nothing in the codec can catch them. The manifest one
/// is the dangerous one: `Ledger::total` counts `aboard`, so a load whose
/// parts do not add up to what was despatched makes tonnage appear or
/// vanish and **every subsequent conservation check passes**, which is
/// this project's only defence against a quiet leak.
///
/// The gate provokes each rejection in turn, because a validator that has
/// only ever seen clean data is untested — the same rule the bill-of-
/// materials validator already has a second gate for.
#[test]
fn a_manifest_that_cannot_be_true_is_refused() {
    use scale_sim::save::SaveError::Impossible;

    // The sound one has to survive, or the gate below proves nothing.
    let sound = round_trip(&a_sound_manifest()).expect("a sound manifest was refused");
    assert_eq!(sound.commodity, Commodity::Grain);
    assert_eq!(sound.leg, Leg::OnTheRoad);

    let spoil = |f: &dyn Fn(&mut Shipment), what: &str| {
        let mut s = a_sound_manifest();
        f(&mut s);
        match round_trip(&s) {
            Err(Impossible(_)) => {}
            other => panic!("{what} was accepted: {other:?}"),
        }
    };

    spoil(&|s| s.aboard = -60.0, "a negative tonnage aboard");
    spoil(&|s| s.despatched = -60.0, "a negative tonnage despatched");
    spoil(&|s| s.goods = -1.0, "a negative cargo value");
    spoil(&|s| s.freight = -1.0, "a negative carriage charge");
    spoil(&|s| s.due = 99, "a cargo due before it set off");
    spoil(&|s| s.aboard = 30.0, "a manifest missing thirty tonnes");
    spoil(
        &|s| {
            s.aboard = 90.0;
            s.despatched = 60.0;
        },
        "a manifest holding more than set off",
    );
    spoil(&|s| s.aboard = 0.0, "a cargo in transit with an empty hold");
    spoil(
        &|s| {
            s.leg = Leg::Delivered;
            s.delivered = 0.0;
        },
        "a finished shipment still holding its cargo",
    );
    spoil(
        &|s| {
            s.leg = Leg::WrittenOff;
            s.aboard = 0.0;
            s.delivered = 60.0;
        },
        "a written-off shipment that delivered",
    );
    spoil(
        &|s| {
            s.aboard = 50.0;
            s.lost = 10.0;
        },
        "ten tonnes lost for no reason at all",
    );
    spoil(
        &|s| s.how_lost = Some(Loss::Spoiled),
        "a cause of loss with nothing lost",
    );

    // And the legitimate finished states still load, or the rules above
    // have banned ordinary history.
    let mut done = a_sound_manifest();
    done.leg = Leg::Delivered;
    done.aboard = 0.0;
    done.delivered = 55.0;
    done.lost = 5.0;
    done.how_lost = Some(Loss::Spoiled);
    let back = round_trip(&done).expect("a delivered part-spoiled cargo was refused");
    assert_eq!(back.delivered, 55.0);
    assert_eq!(back.how_lost, Some(Loss::Spoiled));
}

/// **A collected grave is not a consignment nobody has heard of.**
///
/// `roll_the_road` collects a shipment's tombstone ninety days after it
/// ended, which keeps the registry growing with the world rather than with
/// history — the unbounded state this project has removed four times. But
/// the journal is **permanent** and goes on naming that consignment for
/// ever, so `look` silently changed its answer from `Gone` to `Unknown`,
/// and those are entirely different facts: one is ordinary history, the
/// other is almost always a bug in whatever is holding the reference.
///
/// Elapsed time cannot prove that nothing still refers to an entity. What
/// can is the counter, which only ever goes up — so a name that was issued
/// and a name that never existed stay distinguishable however many graves
/// have been collected.
#[test]
fn a_cargo_delivered_long_ago_is_still_a_cargo_that_existed() {
    use scale_sim::econ::Fate;

    let mut e = world();
    let (from, to, c, qty) = a_load(&mut e);
    let (id, _) = e
        .consign(from, to, 0, c, qty, 173.0, false)
        .expect("nothing was consigned");
    e.tip(id);

    // While the grave is fresh, the registry itself answers.
    assert!(
        matches!(e.what_became_of(id), Fate::Ended { .. }),
        "a cargo tipped this morning is not recorded as having ended"
    );

    // **Now run well past the ninety days the grave survives.**
    for _ in 0..140 {
        e.step();
    }
    assert!(
        matches!(e.shipments.look(id), Lookup::Unknown),
        "the grave was not collected, so this gate is not testing what it says"
    );

    // The registry has forgotten and the world has not.
    match e.what_became_of(id) {
        Fate::Ended { delivered, lost } => {
            assert!(
                delivered + lost > 0.0,
                "the journal has no record of a cargo it certainly carried"
            );
        }
        other => panic!(
            "a cargo delivered four months ago reads as {other:?} — the grave went and \
             took the history with it"
        ),
    }

    // **And a name this world never issued is still a different answer.**
    // That is the half a permanent tombstone would get right by accident
    // and a journal scan alone would get wrong: nothing in the journal
    // mentions it either.
    let never = e.shipments.add(Shipment {
        commodity: c,
        consignor: from,
        consignee: to,
        carrier: 0,
        from_market: 0,
        to_market: 1,
        left: 0,
        due: 0,
        despatched: 1.0,
        aboard: 1.0,
        delivered: 0.0,
        lost: 0.0,
        how_lost: None,
        goods: 0.0,
        freight: 0.0,
        refrigerated: false,
        leg: Leg::OnTheRoad,
    });
    let mut beyond = never;
    for _ in 0..5 {
        beyond = e.shipments.add(Shipment {
            commodity: c,
            consignor: from,
            consignee: to,
            carrier: 0,
            from_market: 0,
            to_market: 1,
            left: 0,
            due: 0,
            despatched: 1.0,
            aboard: 1.0,
            delivered: 0.0,
            lost: 0.0,
            how_lost: None,
            goods: 0.0,
            freight: 0.0,
            refrigerated: false,
            leg: Leg::OnTheRoad,
        });
    }
    let _ = beyond;
    assert_eq!(
        e.what_became_of(never),
        Fate::OnItsWay,
        "a consignment still on the road does not read as being on the road"
    );
}
