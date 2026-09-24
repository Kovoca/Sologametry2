//! **No way across the border is no trade across it** (`docs/status.md`,
//! defect 12).
//!
//! `inland_leg` returned 0.0 both where a town *was* the gateway and where
//! no gateway could be reached at all, so the most cut-off town in the
//! world landed its imports as though the dock were in its own high street.
//! There are three answers now — a genuinely zero-length leg, a costed
//! road, and no usable route — and imports and exports read the same one.
//! The gateway depends on how goods travel: a sea shipment needs a port, a
//! land-border shipment a frontier crossing and a road to it.

use scale_sim::econ::{Commodity, Doctrine, Economy, Event, Gateway};
use scale_sim::money::Account;
use scale_sim::slice::{self, Role};

fn close_every_road(e: &mut Economy) {
    for r in e.routes.iter_mut() {
        r.open = false;
    }
    e.resurvey();
}

/// **The three answers, and both directions read them.**
///
/// On the viable fixture Seaton has a quay and Harwick is a road away from
/// it. At the quay the leg is zero; up the road it costs the road, so
/// Harwick imports dearer and exports cheaper than Seaton; and with the road
/// shut Harwick has no way across the border at all — import parity
/// infinite, export parity minus infinity, neither worth doing at any
/// price.
///
/// The sabotage that matters is the old one: reading *unreachable* as
/// *zero*. Harwick then imports at Seaton's own parity with the road shut.
#[test]
fn a_town_with_no_way_across_the_border_cannot_trade_across_it() {
    let mut e = slice::viable(Doctrine::Prudent);
    e.step();
    let (h, s) = (slice::HARWICK, slice::SEATON);
    let steel = Commodity::Steel;

    assert_eq!(e.gateway(s), Some(Gateway::Quay));
    assert_eq!(e.gateway(h), None);
    assert_eq!(
        e.inland_leg(s),
        Some(0.0),
        "a quay town has a zero-length leg"
    );
    let road = e
        .inland_leg(h)
        .expect("Harwick reaches Seaton's quay by road");
    assert!(road > 0.0, "the road to the quay was free");
    assert!(e.import_parity(h, steel) > e.import_parity(s, steel));
    assert!(e.export_parity(h, steel) < e.export_parity(s, steel));

    close_every_road(&mut e);
    assert_eq!(
        e.inland_leg(h),
        None,
        "a town with its only road shut still reaches a quay"
    );
    assert_eq!(
        e.inland_leg(s),
        Some(0.0),
        "shutting the road moved Seaton's quay"
    );
    assert!(
        e.import_parity(h, steel).is_infinite() && e.import_parity(h, steel) > 0.0,
        "Harwick, with no way across the border, can import steel at {:.1} (Seaton {:.1})",
        e.import_parity(h, steel),
        e.import_parity(s, steel)
    );
    assert!(
        e.export_parity(h, steel).is_infinite() && e.export_parity(h, steel) < 0.0,
        "Harwick, with no way across the border, can export steel at {:.1}",
        e.export_parity(h, steel)
    );
    // At any price at all.
    e.markets[h].price[steel as usize] = 1e12;
    assert!(
        !e.worth_importing(h, steel),
        "a town cut off imports if the price is high enough"
    );
    e.markets[h].price[steel as usize] = 0.0;
    assert!(
        !e.worth_exporting(h, steel),
        "a town cut off exports if the price is low enough"
    );
}

/// **A merchant with no way across the border lands nothing.**
///
/// The behaviour, not just the arithmetic. The two-town fixture has no sea;
/// its steel stockholder at Ashford lands tinplate through a frontier post.
/// Take the post away and shut the road, and the stockholder has no gateway
/// by any mode: over thirty days it must land nothing and pay the outside
/// world nothing — where, with *unreachable* read as *free*, it went on
/// landing as though the border were in the yard.
///
/// And the control: the same fixture with its post keeps landing, so a
/// merchant that lands nothing is the missing gateway and not a quiet day.
#[test]
fn a_merchant_with_no_way_across_the_border_lands_nothing() {
    let landed = |frontier: bool| -> (f64, f64) {
        let mut e = slice::build(Doctrine::Prudent);
        let stockholder = slice::site(&e, Role::AshfordStockholder);
        e.markets[slice::ASHFORD].frontier = frontier;
        close_every_road(&mut e);
        // Low on tinplate, so a merchant with a way in would land some.
        let q = e.ledger.stock(stockholder, Commodity::Steel) * 0.9;
        e.ledger.apply(
            &mut e.journal,
            Event::Consumed {
                site: stockholder,
                commodity: Commodity::Steel,
                qty: q,
                reason: scale_sim::econ::Use::Input,
            },
        );
        let (mut tonnes, mut paid) = (0.0, 0.0);
        for _ in 0..30 {
            let from = e.journal.entries().len();
            e.step();
            for entry in &e.journal.entries()[from..] {
                if let Event::Produced {
                    site,
                    commodity,
                    qty,
                } = &entry.event
                {
                    if *site == stockholder && *commodity == Commodity::Steel {
                        tonnes += qty;
                    }
                }
            }
            paid += e
                .treasury
                .today
                .iter()
                .filter(|t| t.from == Account::Firm(stockholder) && t.to == Account::Abroad)
                .map(|t| t.amount)
                .sum::<f64>();
        }
        e.ledger.assert_conserved();
        // **And an unreachable parity must not reach the books.** It is
        // infinite, and a merchant that lands nothing has a day's landing
        // of nought: priced as a cost, nought times infinity is not a
        // number, and it spread to every price and every account in a
        // country until the treasury's own identity could not be checked.
        e.treasury.assert_conserved();
        for (m, market) in e.markets.iter().enumerate() {
            for &c in Commodity::ALL.iter() {
                assert!(
                    market.price[c as usize].is_finite() && market.cost[c as usize].is_finite(),
                    "{c:?} at {} is not a number: price {}, cost {}",
                    e.markets[m].name,
                    market.price[c as usize],
                    market.cost[c as usize]
                );
            }
        }
        (tonnes, paid)
    };
    let (open_t, open_paid) = landed(true);
    assert!(
        open_t > 0.0 && open_paid > 0.0,
        "with its frontier post the stockholder landed {open_t:.1} t, so the test proves nothing"
    );
    let (cut_t, cut_paid) = landed(false);
    assert_eq!(
        (cut_t, cut_paid),
        (0.0, 0.0),
        "with no gateway by any mode the stockholder landed {cut_t:.2} t and paid abroad \
         {cut_paid:.1}"
    );
}

/// **A frontier post is a way out as well as a way in.**
///
/// Exports used to need a quay while imports needed nothing at all, so a
/// country with no sea could buy and never sell — a one-way drain written
/// into the rule. Ashford's post now does both, and Bexley, a road away,
/// reaches it for both and pays the road both ways.
#[test]
fn a_frontier_post_is_a_way_out_as_well_as_a_way_in() {
    let mut e = slice::build(Doctrine::Prudent);
    e.step();
    let (a, b) = (slice::ASHFORD, slice::BEXLEY);
    let steel = Commodity::Steel;
    assert_eq!(e.gateway(a), Some(Gateway::Frontier));
    assert_eq!(e.inland_leg(a), Some(0.0));
    assert!(
        e.inland_leg(b).is_some_and(|x| x > 0.0),
        "Bexley reaches the post for nothing"
    );
    assert!(e.import_parity(b, steel) > e.import_parity(a, steel));
    assert!(e.export_parity(b, steel) < e.export_parity(a, steel));

    e.markets[a].price[steel as usize] = 0.0;
    assert!(
        e.worth_exporting(a, steel),
        "a frontier town with steel at nothing will not sell it abroad"
    );
    e.markets[a].price[steel as usize] = 1e12;
    assert!(
        e.worth_importing(a, steel),
        "a frontier town will not buy steel at any price"
    );
}
