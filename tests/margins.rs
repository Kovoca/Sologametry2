//! **A provider billed below its costs never gets back on its feet.**
//!
//! Builders and hospitals were billed the day's wages and the materials or
//! supplies used, and nothing more. Their power and the carriage on their
//! deliveries are costs of the work too, and had no income behind them —
//! so a provider knocked to nothing stayed there: it could not pay for its
//! power or its deliveries, was paid less than the day cost, and started
//! the next day short again.
//!
//! **The first fix was a margin, and the measurement said it was the wrong
//! fix.** A gross margin over wages and materials did get providers back
//! on their feet, by charging far more than the work cost. Bill every
//! operating cost the model has and they recover with no margin at all:
//!
//! | the bill covers                 | no margin     | operating margin |
//! |---------------------------------|---------------|------------------|
//! | wages and materials only        | 9 of 10 short | 2 of 10 short    |
//! | every operating cost incurred   | 0             | 0                |
//!
//! So this gate holds the bill to the whole cost; the margin itself is set
//! from its source, not by this test.
//!
//! **Part of that recovery is borrowed, and it is named.** The cost billed
//! is what was *incurred* — power and carriage owed as well as paid — and
//! what a firm fails to pay here is recorded for the day and then forgotten,
//! not owed. So a drained provider is paid for bills it never settled and
//! keeps the cash. When unpaid bills persist as obligations, it will have
//! to settle them out of that money, and this gate must be read again.

use scale_sim::econ::{Doctrine, Economy, SiteKind};
use scale_sim::money::{Account, Why};
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

fn a_nation() -> Economy {
    let world = World::generate(384, 216, 20260828);
    let pol = Polities::partition(&world, 30);
    let set = Settlements::place(&world, &pol, 5000);
    let net = Network::build(&world, &set, 1000);
    let id = pol.ranked()[2].0;
    Region::extract(&world, &pol, &set, &net, id, 5, Doctrine::Prudent)
        .expect("a nation")
        .economy
}

/// What `site` failed to pay anybody today, for anything.
fn left_unpaid_by(e: &Economy, site: usize) -> f64 {
    e.treasury
        .unpaid_by
        .iter()
        .filter(|((from, _, _), _)| *from == Account::Firm(site))
        .map(|(_, v)| v)
        .sum()
}

/// **There is a way back in.** Empty every builder and hospital in a
/// country — their money handed to the households of their town, so
/// nothing is created or destroyed — and require each of them, within two
/// months, to be paying everybody it owes in full: its staff, its
/// suppliers, its carriers and its power. Red with power and carriage left
/// out of the bill: two of ten still short with the margins, nine of ten
/// without them.
#[test]
fn a_provider_emptied_to_nothing_gets_back_on_its_feet() {
    let mut e = a_nation();
    for _ in 0..30 {
        e.step();
    }
    let providers: Vec<usize> = (0..e.ledger.sites.len())
        .filter(|&s| {
            matches!(
                e.ledger.sites[s].kind,
                SiteKind::Builders | SiteKind::Hospital
            )
        })
        .collect();
    assert!(
        providers
            .iter()
            .any(|&s| e.ledger.sites[s].kind == SiteKind::Builders)
            && providers
                .iter()
                .any(|&s| e.ledger.sites[s].kind == SiteKind::Hospital),
        "the fixture has no builders or no hospital to empty"
    );

    let day = e.ledger.day;
    for &s in &providers {
        let m = e.ledger.sites[s].market;
        let held = e.treasury.balance(Account::Firm(s));
        e.treasury.pay(
            day,
            Account::Firm(s),
            Account::Households(m),
            held,
            Why::Profit,
        );
        assert!(e.treasury.balance(Account::Firm(s)) < 1e-6);
    }

    // Forty-five days to find its feet, and then fifteen watched.
    for _ in 0..45 {
        e.step();
    }
    let mut short = vec![0.0; e.ledger.sites.len()];
    let mut worked = vec![0.0; e.ledger.sites.len()];
    for _ in 0..15 {
        e.step();
        for &s in &providers {
            short[s] += left_unpaid_by(&e, s);
            worked[s] += e.ledger.sites[s].ran;
        }
    }

    let still_short: Vec<String> = providers
        .iter()
        .filter(|&&s| short[s] > 1e-6)
        .map(|&s| {
            format!(
                "{:?} at {} left {:.3e} unpaid over the last fortnight",
                e.ledger.sites[s].kind, e.markets[e.ledger.sites[s].market].name, short[s]
            )
        })
        .collect();
    assert!(
        still_short.is_empty(),
        "two months after being emptied, {} of {} providers still cannot pay \
         everybody they owe:\n  {}",
        still_short.len(),
        providers.len(),
        still_short.join("\n  ")
    );
    // **And it recovered by working**, not by standing idle with nothing to
    // pay: a yard that does nothing owes nobody.
    assert!(
        providers.iter().all(|&s| worked[s] > 0.0),
        "a provider paid its way by doing no work"
    );
}
