//! **Where three identical towns stop being identical.**
//!
//!   cargo run --release --bin symmetry
//!   cargo run --release --bin symmetry -- --throttle 0.30 --days 200
//!
//! `slice::symmetric` maps onto itself under rotation, so any spread is a
//! bug rather than a signal. This walks the day and names the first
//! quantity that diverges, and the first day it does — because the thing
//! a gate reports at day 200 is nearly always three steps downstream of
//! the thing that actually broke.

use scale_sim::econ::{Commodity, Economy, SiteKind};
use scale_sim::money::Account;
use scale_sim::slice::{self, Doctrine};

fn spread(v: &[f64]) -> f64 {
    let (lo, hi) = v
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), &x| (a.min(x), b.max(x)));
    hi - lo
}

/// A spread that is float noise rather than a fact about the economy.
fn noise(v: &[f64]) -> bool {
    let scale = v.iter().fold(0.0f64, |a, &x| a.max(x.abs())).max(1.0);
    spread(v) <= scale * 1e-9
}

/// Days of cover a town holds, as `tests/equilibrium.rs` measures it.
fn cover(e: &Economy, m: usize, c: Commodity) -> f64 {
    let held: f64 = (0..e.ledger.sites.len())
        .filter(|&s| e.ledger.sites[s].market == m)
        .map(|s| e.ledger.stock(s, c))
        .sum();
    let d = e.daily_draw(m, c);
    if d > 1e-9 {
        held / d
    } else {
        0.0
    }
}

/// Every per-town reading worth watching, by name.
fn readings(e: &Economy) -> Vec<(String, Vec<f64>)> {
    let towns = e.markets.len();
    let mut out = Vec::new();
    out.push((
        "household purse".to_string(),
        (0..towns)
            .map(|m| e.treasury.balance(Account::Households(m)))
            .collect(),
    ));
    for &c in Commodity::ALL.iter() {
        let price: Vec<f64> = (0..towns).map(|m| e.markets[m].price[c as usize]).collect();
        if price.iter().any(|&p| p > 0.0) {
            out.push((format!("{c:?} price"), price));
            out.push((
                format!("{c:?} cost"),
                (0..towns).map(|m| e.markets[m].cost[c as usize]).collect(),
            ));
            out.push((
                format!("{c:?} cover"),
                (0..towns).map(|m| cover(e, m, c)).collect(),
            ));
        }
    }
    // Each kind of works, summed over the town it stands in, so the three
    // towns' copies are compared with each other and not with a neighbour.
    for kind in [
        SiteKind::Farm,
        SiteKind::Mill,
        SiteKind::Factory,
        SiteKind::PowerPlant,
        SiteKind::Depot,
        SiteKind::Shop,
    ] {
        out.push((
            format!("{kind:?} ran"),
            (0..towns)
                .map(|m| {
                    e.ledger
                        .sites
                        .iter()
                        .filter(|s| s.market == m && s.kind == kind)
                        .map(|s| s.ran)
                        .sum()
                })
                .collect(),
        ));
        out.push((
            format!("{kind:?} balance"),
            (0..towns)
                .map(|m| {
                    (0..e.ledger.sites.len())
                        .filter(|&i| {
                            e.ledger.sites[i].market == m && e.ledger.sites[i].kind == kind
                        })
                        .map(|i| e.treasury.balance(Account::Firm(i)))
                        .sum()
                })
                .collect(),
        ));
    }
    out
}

fn main() {
    let mut days = 200u64;
    let mut throttle = 1.0f64;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--days" => days = it.next().and_then(|v| v.parse().ok()).unwrap_or(days),
            "--throttle" => throttle = it.next().and_then(|v| v.parse().ok()).unwrap_or(throttle),
            _ => {}
        }
    }

    let mut e = slice::symmetric(Doctrine::Prudent);
    if throttle < 1.0 {
        for s in 0..e.ledger.sites.len() {
            if e.ledger.sites[s].kind == SiteKind::Mill {
                e.ledger.sites[s].throughput *= throttle;
            }
        }
    }

    println!("three identical towns, {days} days, mills at {throttle:.2} of rating\n");
    let mut reported: std::collections::BTreeSet<String> = Default::default();
    for d in 0..days {
        e.step();
        for (name, v) in readings(&e) {
            if noise(&v) || reported.contains(&name) {
                continue;
            }
            reported.insert(name.clone());
            println!("day {d:>4}  {name:<26} {v:?}");
        }
    }
    if reported.is_empty() {
        println!("nothing diverged beyond float noise.");
    }
}
