//! **Carriage dearer than the goods is not a reason to go without them.**
//!
//! `distribute` once refused any delivery whose freight came to more than
//! the goods were worth. Its comment claimed that was weaker than the
//! half-the-value rule a haulier applies and so could forbid nothing that
//! model allows; in combination with the consignment floor it was not —
//! `carriage_for` charges at least a quarter of a lorry however little is on
//! board, so a small order's freight is fixed while its value shrinks. What
//! it refused was a works topping up its daily ration down a long road, and
//! a refusal on day one bankrupted three cement works of five.
//!
//! The guard was removed on the evidence that no sub-tonne load appeared in
//! one fixture over four hundred days. **That run never offered the case**,
//! so it established nothing about it. This gate makes the case happen.

use scale_sim::econ::{Commodity, Doctrine, Event, Use};
use scale_sim::money::{Account, Why};
use scale_sim::slice::{self, Role};

/// **A small, affordable, necessary delivery is made even though the
/// carriage costs more than the flour.**
///
/// On the viable fixture the cannery is at Seaton and the mill is a
/// hundred and twenty kilometres up the road at Harwick. Turn the cannery
/// down to a trickle and empty its flour yard, and what it needs is a
/// fraction of a tonne: small. It holds its capital, so it can pay: it is
/// affordable. It has no flour at all, so without the delivery it cannot
/// run: necessary. The flour is worth a few tens and the lorry, floored at
/// a quarter-load, costs a couple of hundred.
///
/// It must arrive, be paid for — the mill for the flour and a carrier for
/// the haul — and the cannery must run on it the next day.
#[test]
fn a_small_necessary_delivery_is_made_when_the_carriage_costs_more_than_the_goods() {
    let mut e = slice::viable(Doctrine::Prudent);
    for _ in 0..60 {
        e.step();
    }
    let cannery = slice::site(&e, Role::SeatonCannery);
    let mill = slice::site(&e, Role::HarwickMill);
    let flour = Commodity::Flour;

    e.ledger.sites[cannery].throughput = 0.05;
    let yard = e.ledger.stock(cannery, flour);
    e.ledger.apply(
        &mut e.journal,
        Event::Consumed {
            site: cannery,
            commodity: flour,
            qty: yard,
            reason: Use::Input,
        },
    );
    let cash = e.treasury.balance(Account::Firm(cannery));

    let from = e.journal.entries().len();
    e.step();
    let (mut tonnes, mut goods, mut freight, mut milled) = (0.0, 0.0, 0.0, 0.0);
    for entry in &e.journal.entries()[from..] {
        match &entry.event {
            Event::Shipped {
                from,
                to,
                commodity,
                qty,
                paid,
                freight: f,
            } if *from == mill && *to == cannery && *commodity == flour => {
                tonnes += qty;
                goods += paid;
                freight += f;
            }
            Event::Produced {
                site,
                commodity,
                qty,
            } if *site == mill && *commodity == flour => milled += qty,
            _ => {}
        }
    }
    // There was flour to send: the mill made some that morning, before the
    // deliveries.
    assert!(
        milled > 1.0,
        "the mill made no flour, so this proves nothing"
    );
    let supplier_paid: f64 = e
        .treasury
        .today
        .iter()
        .filter(|t| t.from == Account::Firm(cannery) && t.to == Account::Firm(mill))
        .filter(|t| t.why == Why::Supply)
        .map(|t| t.amount)
        .sum();
    let carrier_paid: f64 = e
        .treasury
        .today
        .iter()
        .filter(|t| t.from == Account::Firm(cannery) && t.why == Why::Freight)
        .map(|t| t.amount)
        .sum();
    println!(
        "{tonnes:.3} t of flour worth {goods:.1} (paid {supplier_paid:.1}) carried for \
         {freight:.1} (paid {carrier_paid:.1}); the cannery held {cash:.0}"
    );
    assert!(tonnes > 0.0, "the cannery was left without flour");
    assert!(tonnes < 1.0, "{tonnes:.2} t is not a small delivery");
    assert!(
        freight > goods && freight > supplier_paid,
        "the carriage ({freight:.1}) does not exceed the goods ({goods:.1}), so this is not the case"
    );
    assert!(
        cash > goods + freight,
        "the cannery could not afford it, so this is not the case"
    );
    assert!(supplier_paid > 0.0, "the mill was not paid for its flour");
    assert!(
        carrier_paid >= freight * (1.0 - 1e-9),
        "the carrier was not paid for the haul"
    );

    e.step();
    assert!(
        e.ledger.sites[cannery].ran > 0.0,
        "the cannery did not run on the flour it was sent"
    );
    e.ledger.assert_conserved();
}
