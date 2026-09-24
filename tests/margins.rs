//! **A provider charges more than it costs, or a bad week is for ever.**
//!
//! Builders and hospitals were billed exactly what the day used — wages,
//! and the materials or supplies at the price a firm pays — and nothing
//! more. Everything else they pay, the power and the carriage on their
//! deliveries, had no income behind it, and a firm that takes in exactly
//! what it pays out has no buffer: knock it to nothing and it stays there.
//! In the five-year soak both took in within a fraction of a per cent of
//! what they paid out, every year, in every world.
//!
//! Real firms price over their direct costs, and the gross margin is the
//! measured share left after direct labour and materials: **15.46%** of
//! sales for US engineering and construction firms and **39.10%** for
//! hospital chains *(Damodaran, Margins by Sector (US), data as of January
//! 2026)*.

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
/// suppliers, its carriers and its power.
///
/// At cost there is no way back. A provider that starts the day with
/// nothing cannot pay for its power or its deliveries; it is then paid its
/// wages and the materials it used, pays the wages, and starts the next
/// day with exactly the materials' worth — which is short of the materials
/// *and* the power *and* the carriage. So it stays short, every day, for
/// ever. A margin is what closes that gap.
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
