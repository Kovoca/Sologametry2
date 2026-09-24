//! **From the works to the counter, and who was paid on the way.**
//!
//! The missing evidence for the counter change. Households buying only
//! what they can pay for was shipped with gates on the till; nothing yet
//! showed a tonne being grown, milled, canned, carried and bought over a
//! counter with every hand-off paid for, and nothing showed what a purse
//! too thin to buy everything leaves behind. This walks both on
//! `slice::viable` — a small country whose households are paid wages by
//! works that sell to customers — so what it shows is an economy working
//! rather than a fixture that happens to be solvent.
//!
//! **What it does not establish, and it is the original defect.** Goods
//! still move between firms whether or not the buyer can pay: `distribute`
//! delivers first and `Treasury::pay` settles what the buyer holds, with the
//! rest on the unpaid counter. Even this viable fixture leaves some of its
//! grain unpaid for in the lean months, and the first gate names exactly
//! which. Requiring cash on delivery was built and measured — it closes the
//! gap here and collapses every generated world — and is kept on the
//! `pay-on-delivery` branch with the measurement; `docs/status.md`, tracked
//! gap 2, says what it is waiting for. `credit.rs`, the other authorised
//! form, is not wired in either.

use scale_sim::econ::{Commodity, Doctrine, Economy, Event, Use};
use scale_sim::money::{Account, Why};
use scale_sim::slice::{self, Role};
use std::collections::BTreeSet;

fn run(e: &mut Economy, days: u64) {
    for _ in 0..days {
        e.step();
    }
}

fn is_station(e: &Economy, a: Account) -> bool {
    matches!(a, Account::Firm(s) if e.ledger.sites[s].kind == scale_sim::econ::SiteKind::PowerPlant)
}

/// What went unpaid today for **goods handed from one firm to another** —
/// supply, a consignment's goods, or the carriage on either — as distinct
/// from a power bill, which is metered rather than delivered. By payer and
/// payee.
fn goods_unpaid_today(e: &Economy) -> Vec<(Account, Account, f64)> {
    e.treasury
        .unpaid_by
        .iter()
        .filter(|((from, to, why), v)| {
            **v > 1e-6
                && matches!(from, Account::Firm(_))
                && match *why {
                    "supply" => !is_station(e, *to),
                    "purchases" | "freight" => true,
                    _ => false,
                }
        })
        .map(|((from, to, _), v)| (*from, *to, *v))
        .collect()
}

/// **The fixture earns its living, and what goes unpaid in it is named.**
///
/// The two-town fixture's households lived on dividends — payroll was
/// 0.4% of their income — so it could not tell a working economy from a
/// solvent one. Over nine hundred days this asks the viable fixture the
/// things that tell them apart: wages are a large part of what households
/// receive; **each town's** households hold their money rather than
/// draining into the other's; and the lights stay on.
///
/// *Nine hundred, because four hundred cannot see a drain.* With Harwick's
/// farmland as one corporate farm, whose profit is spread across both towns
/// by population, Harwick's households go from 3.9M to 0.14M between day
/// 100 and day 1,000 — and are still above seven tenths of where they
/// started at day 500.
///
/// *Wages are not the 61% of household income they are in life*
/// *(compensation of employees, US personal income, BEA 2023)*: here they
/// run about 35-40%, because firms pay no rent, interest or depreciation and
/// all of it falls into profit (CLAUDE.md, "Nobody imports a haircut"). The
/// bar is a quarter, which the distressed fixture misses by sixty times.
///
/// **And the goods that go unpaid are the mill's grain, and nothing else.**
/// Before the harvest grain is dear and flour is not, so the mill mills at
/// a loss, empties its till and goes on taking grain from the farms — about
/// 17,000 a day through the spring. Nothing stops it, because nothing yet
/// requires a buyer to be able to pay (see the module comment). This names
/// the one path so a second one cannot open unnoticed, and it is not a
/// claim that one is acceptable.
#[test]
fn the_viable_fixture_earns_its_living_and_names_what_goes_unpaid() {
    let mut e = slice::viable(Doctrine::Prudent);
    run(&mut e, 100);
    let mill = slice::site(&e, Role::HarwickMill);
    let towns = [slice::HARWICK, slice::SEATON];
    let before: Vec<f64> = towns
        .iter()
        .map(|&m| e.treasury.balance(Account::Households(m)))
        .collect();
    let (mut wages, mut income, mut supplied, mut unpaid) = (0.0, 0.0, 0.0, 0.0);
    for _ in 0..900 {
        e.step();
        assert_eq!(e.unserved_power, 0.0, "day {}: load unserved", e.ledger.day);
        for (from, to, v) in goods_unpaid_today(&e) {
            let farm = matches!(to, Account::Firm(s)
                if e.ledger.sites[s].kind == scale_sim::econ::SiteKind::Farm);
            assert!(
                from == Account::Firm(mill) && farm,
                "day {}: {v:.1} of goods went unpaid by {from:?} to {to:?}, which is not the \
                 mill's grain",
                e.ledger.day
            );
            unpaid += v;
        }
        for t in e.treasury.today.iter() {
            if let Account::Households(_) = t.to {
                income += t.amount;
                if t.why == Why::Payroll {
                    wages += t.amount;
                }
            }
            if t.why == Why::Supply && matches!(t.from, Account::Firm(_)) && !is_station(&e, t.to) {
                supplied += t.amount;
            }
        }
    }
    let share = wages / income;
    println!(
        "wages {:.1}% of household income; goods between firms paid {supplied:.3e}, \
         unpaid {unpaid:.3e} ({:.1}%)",
        share * 100.0,
        100.0 * unpaid / (supplied + unpaid).max(1e-9)
    );
    assert!(
        share > 0.25,
        "wages are {:.1}% of what households receive",
        share * 100.0
    );
    for (k, &m) in towns.iter().enumerate() {
        let after = e.treasury.balance(Account::Households(m));
        assert!(
            after > before[k] * 0.7,
            "{}'s households went {:.0} -> {after:.0} in nine hundred days",
            e.markets[m].name,
            before[k]
        );
    }
    e.ledger.assert_conserved();
}

/// **Every hand-off from the field to the counter is paid for, the day it
/// happens.**
///
/// Grain is grown and milled in Harwick; the flour goes down the road to
/// Seaton's cannery; tinplate is landed from abroad at Seaton's quay and
/// sold to the cannery; the food comes back up the road to Harwick's hall
/// and across the street to Seaton's; households buy it over both counters.
/// Followed through the journal and the treasury a day at a time, and for
/// every delivery between firms the buyer paid the seller that day, and
/// paid a carrier for every one that crossed between towns.
///
/// Days 60 to 90, which is before the lean months: the gate above says what
/// the mill leaves unpaid later in the year.
#[test]
fn every_hand_off_from_the_field_to_the_counter_is_paid_for() {
    let mut e = slice::viable(Doctrine::Prudent);
    run(&mut e, 60);
    let mill = slice::site(&e, Role::HarwickMill);
    let cannery = slice::site(&e, Role::SeatonCannery);
    let stockholder = slice::site(&e, Role::SeatonStockholder);
    let halls = [
        slice::site(&e, Role::HarwickHall),
        slice::site(&e, Role::SeatonHall),
    ];
    let mut moved: BTreeSet<(usize, usize, usize)> = BTreeSet::new();
    let (mut milled, mut canned, mut landed, mut abroad) = (0.0, 0.0, 0.0, 0.0);
    let mut bought = [0.0f64; 2];
    for _ in 0..30 {
        let from = e.journal.entries().len();
        e.step();
        let today = &e.journal.entries()[from..];
        for entry in today {
            match &entry.event {
                Event::Produced {
                    site,
                    commodity,
                    qty,
                } => {
                    if *site == mill && *commodity == Commodity::Flour {
                        milled += qty;
                    }
                    if *site == cannery && *commodity == Commodity::ProcessedFood {
                        canned += qty;
                    }
                    if *site == stockholder && *commodity == Commodity::Steel {
                        landed += qty;
                    }
                }
                Event::Shipped {
                    from,
                    to,
                    commodity,
                    qty,
                    freight,
                    ..
                } if *qty > 1e-9 => {
                    moved.insert((*from, *to, *commodity as usize));
                    let supplier_paid = e.treasury.today.iter().any(|t| {
                        t.from == Account::Firm(*to)
                            && t.to == Account::Firm(*from)
                            && t.why == Why::Supply
                            && t.amount > 0.0
                    });
                    assert!(
                        supplier_paid,
                        "day {}: {qty:.2} t of {commodity:?} went from {} to {} and the \
                         supplier was not paid",
                        e.ledger.day, e.ledger.sites[*from].name, e.ledger.sites[*to].name
                    );
                    let crosses = e.ledger.sites[*from].market != e.ledger.sites[*to].market;
                    if crosses {
                        assert!(*freight > 0.0, "a haul between towns was carried free");
                        let carrier_paid = e.treasury.today.iter().any(|t| {
                            t.from == Account::Firm(*to) && t.why == Why::Freight && t.amount > 0.0
                        });
                        assert!(
                            carrier_paid,
                            "day {}: nobody paid the carrier",
                            e.ledger.day
                        );
                    }
                }
                Event::Consumed {
                    site,
                    commodity,
                    qty,
                    reason: Use::Household,
                } if *commodity == Commodity::ProcessedFood => {
                    if let Some(k) = halls.iter().position(|h| h == site) {
                        bought[k] += qty;
                    }
                }
                _ => {}
            }
        }
        abroad += e
            .treasury
            .today
            .iter()
            .filter(|t| t.from == Account::Firm(stockholder) && t.to == Account::Abroad)
            .map(|t| t.amount)
            .sum::<f64>();
        let unpaid = goods_unpaid_today(&e);
        assert!(
            unpaid.is_empty(),
            "day {}: goods went unpaid: {unpaid:?}",
            e.ledger.day
        );
    }
    let crossed = |from: usize, to_town: usize, c: Commodity| {
        moved
            .iter()
            .any(|&(f, t, k)| f == from && e.ledger.sites[t].market == to_town && k == c as usize)
    };
    assert!(
        milled > 0.0 && canned > 0.0,
        "the mill or the cannery made nothing"
    );
    assert!(
        landed > 0.0 && abroad > 0.0,
        "no tinplate was landed and paid for abroad"
    );
    assert!(
        crossed(mill, slice::SEATON, Commodity::Flour),
        "no flour went down the road to the cannery"
    );
    assert!(
        crossed(stockholder, slice::SEATON, Commodity::Steel),
        "the landed tinplate never reached the cannery"
    );
    assert!(
        crossed(cannery, slice::HARWICK, Commodity::ProcessedFood),
        "no food went up the road to Harwick"
    );
    assert!(
        bought[0] > 0.0 && bought[1] > 0.0,
        "a hall sold no food: {bought:?}"
    );
    e.ledger.assert_conserved();
}

/// **A purse too thin for the basket buys less, and what it does not buy
/// is still on the shelf and still wanted.**
///
/// Seaton's households are left a quarter of one day's counter basket and
/// the day is played. Nothing is inferred: what was wanted is the town's own
/// daily demand, what was bought is read off the journal at Seaton's hall,
/// and what is left is the hall's stock — and they are checked against each
/// other, so the tonnes add up:
///
/// ```text
/// wanted = bought + gone without
/// on the shelf before + delivered = bought + on the shelf after + spoiled
/// ```
///
/// The shortfall is **unaffordable, not unstocked**: food is still on the
/// shelf at the end of the day. And nothing is taken at the counter on
/// credit — no purchase at the hall is left unpaid.
///
/// **Not asserted, because it is not true yet: services.** A household's
/// service bill — the posts `services.rs` counts against population — is
/// raised after the counter has spent what the purse could stand, and is
/// not cut to what is left. The test prints it.
#[test]
fn a_thin_purse_buys_less_and_leaves_the_rest_on_the_shelf() {
    let mut e = slice::viable(Doctrine::Prudent);
    run(&mut e, 60);
    let hall = slice::site(&e, Role::SeatonHall);
    let food = Commodity::ProcessedFood;
    let wanted = e.markets[slice::SEATON].daily_household_demand(food);
    // The basket the counter costs: what is sold over a counter *and* is on
    // a shelf here. This fixture's halls sell food only, and meat,
    // remedies and goods nobody stocks sit at their scarcity ceiling — so
    // counting them made "a quarter of a basket" enough for all the food.
    let basket: f64 = Commodity::ALL
        .iter()
        .filter(|&&c| c.till_elasticity().is_some() && e.ledger.stock(hall, c) > 0.0)
        .map(|&c| e.markets[slice::SEATON].daily_household_demand(c) * e.price(slice::SEATON, c))
        .sum();
    // Leave a quarter of a day's basket. The money goes abroad rather than
    // vanishing, so the books still balance.
    let purse = e.treasury.balance(Account::Households(slice::SEATON));
    e.treasury.pay(
        e.ledger.day,
        Account::Households(slice::SEATON),
        Account::Abroad,
        purse - basket * 0.25,
        Why::Trade,
    );
    let shelf_before = e.ledger.stock(hall, food);
    let from = e.journal.entries().len();
    e.step();
    let (mut bought, mut delivered, mut spoiled) = (0.0, 0.0, 0.0);
    for entry in &e.journal.entries()[from..] {
        match &entry.event {
            Event::Consumed {
                site,
                commodity,
                qty,
                reason: Use::Household,
            } if *site == hall && *commodity == food => bought += qty,
            Event::Shipped {
                to, commodity, qty, ..
            }
            | Event::Landed {
                to, commodity, qty, ..
            } if *to == hall && *commodity == food => delivered += qty,
            Event::Spoiled {
                site,
                commodity,
                qty,
            } if *site == hall && *commodity == food => spoiled += qty,
            _ => {}
        }
    }
    let shelf_after = e.ledger.stock(hall, food);
    let gone_without = wanted - bought;
    let services: f64 = e
        .treasury
        .unpaid_by
        .iter()
        .filter(|((f, t, _), _)| {
            *f == Account::Households(slice::SEATON) && matches!(t, Account::ServiceSector(_))
        })
        .map(|(_, v)| v)
        .sum();
    println!(
        "Seaton, one day on a quarter of a basket: wanted {wanted:.1} t of food, bought \
         {bought:.1}, went without {gone_without:.1}; shelf {shelf_before:.1} -> \
         {shelf_after:.1} with {delivered:.1} delivered and {spoiled:.2} spoiled; \
         service bills unpaid {services:.0}"
    );
    assert!(bought > 0.0, "a quarter of a basket bought no food at all");
    assert!(
        gone_without > wanted * 0.05,
        "a quarter of a basket bought nearly everything"
    );
    assert!(
        shelf_after > 0.0,
        "the shelf ran dry, so the shortfall is not about money"
    );
    let books = shelf_before + delivered - bought - spoiled - shelf_after;
    assert!(
        books.abs() < 1e-6,
        "the hall's food does not add up by {books:.6} t"
    );
    let at_the_counter = e
        .treasury
        .unpaid_by
        .get(&(
            Account::Households(slice::SEATON),
            Account::Firm(hall),
            "purchases",
        ))
        .copied()
        .unwrap_or(0.0);
    assert!(
        at_the_counter <= 1e-6,
        "{at_the_counter:.1} of food was taken at the counter and not paid for"
    );
    e.ledger.assert_conserved();
}
