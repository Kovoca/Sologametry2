//! **Where three identical towns stop being identical.**
//!
//!   cargo run --release --bin symmetry
//!   cargo run --release --bin symmetry -- --throttle 0.30 --days 200
//!   cargo run --release --bin symmetry -- --hauliers --days 430
//!
//! `slice::symmetric` maps onto itself under rotation, so any spread is a
//! bug rather than a signal. This walks the day and names the first
//! quantity that diverges, and the first day it does — because the thing
//! a gate reports at day 200 is nearly always three steps downstream of
//! the thing that actually broke.
//!
//! It also prints where the country's money ended up, which is how the
//! most useful thing here was found and was not what it was built to look
//! for: after four hundred days this fixture holds 96.4% of its money
//! abroad and its households hold none of it.

use scale_sim::econ::{Commodity, Economy, SiteKind};
use scale_sim::logistics::Logistics;
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
    spread(v) <= scale * 1e-7
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
    // What each town's households paid out today and took in today, by what
    // for. A purse that diverges is the *stock*; these are the flows that
    // moved it, and one of them is the cause.
    let mut flows: std::collections::BTreeMap<String, Vec<f64>> = Default::default();
    for t in e.treasury.today.iter() {
        if let Account::Households(m) = t.from {
            flows
                .entry(format!("households paid {:?}", t.why))
                .or_insert_with(|| vec![0.0; towns])[m] += t.amount;
        }
        if let Account::Households(m) = t.to {
            flows
                .entry(format!("households got {:?}", t.why))
                .or_insert_with(|| vec![0.0; towns])[m] += t.amount;
        }
    }
    out.extend(flows);
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
            format!("{kind:?} staff"),
            (0..towns)
                .map(|m| {
                    (0..e.ledger.sites.len())
                        .filter(|&i| {
                            e.ledger.sites[i].market == m && e.ledger.sites[i].kind == kind
                        })
                        .map(|i| e.staff_today.get(i).copied().unwrap_or(0.0))
                        .sum()
                })
                .collect(),
        ));
        out.push((
            format!("{kind:?} stock"),
            (0..towns)
                .map(|m| {
                    (0..e.ledger.sites.len())
                        .filter(|&i| {
                            e.ledger.sites[i].market == m && e.ledger.sites[i].kind == kind
                        })
                        .map(|i| {
                            Commodity::ALL
                                .iter()
                                .map(|&c| e.ledger.stock(i, c))
                                .sum::<f64>()
                        })
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
    let mut hauliers = false;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--days" => days = it.next().and_then(|v| v.parse().ok()).unwrap_or(days),
            "--throttle" => throttle = it.next().and_then(|v| v.parse().ok()).unwrap_or(throttle),
            "--hauliers" => hauliers = true,
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
    if hauliers {
        e.logistics = Some(Logistics::found(&e));
    }

    println!("three identical towns, {days} days, mills at {throttle:.2} of rating\n");

    let mut reported: std::collections::BTreeSet<String> = Default::default();
    let mut first: Option<u64> = None;
    for d in 0..days {
        e.step();
        for (name, v) in readings(&e) {
            if noise(&v) || reported.contains(&name) {
                continue;
            }
            reported.insert(name.clone());
            first.get_or_insert(d);
            println!("day {d:>4}  {name:<26} {v:?}");
        }
    }
    // **And where the country's money actually is.** A purse holding a
    // fortieth of one day's shopping is not a spread, it is a fixture with
    // no money in it, and that has to be established before anything that
    // reads a purse can be judged in here.
    let mut held: std::collections::BTreeMap<&str, f64> = Default::default();
    for (a, v) in e.treasury.accounts() {
        let kind = match a {
            Account::Firm(_) => "firms",
            Account::Households(_) => "households",
            Account::ServiceSector(_) => "services",
            Account::State(_) => "the state",
            Account::Abroad => "abroad",
            _ => "other",
        };
        *held.entry(kind).or_default() += *v;
    }
    let all: f64 = held.values().sum();
    println!(
        "
where the money is after {days} days (total {all:.4e}):"
    );
    for (k, v) in &held {
        println!("  {k:<12} {v:>16.2}  {:>6.1}%", 100.0 * v / all);
    }

    if reported.is_empty() {
        println!("nothing diverged beyond float noise.");
        return;
    }

    // **And which firm did it**, on the day it first happened. A total
    // says the towns were paid differently; it cannot say by whom, and one
    // firm's last tranche falling short is invisible in a sum.
    let Some(when) = first else { return };
    let mut e2 = slice::symmetric(Doctrine::Prudent);
    if throttle < 1.0 {
        for s in 0..e2.ledger.sites.len() {
            if e2.ledger.sites[s].kind == SiteKind::Mill {
                e2.ledger.sites[s].throughput *= throttle;
            }
        }
    }
    if hauliers {
        e2.logistics = Some(Logistics::found(&e2));
    }
    for _ in 0..=when {
        e2.step();
    }
    let towns = e2.markets.len();
    println!("\nday {when}, what each firm paid each town, and what it holds:");
    for site in 0..e2.ledger.sites.len() {
        let paid: Vec<f64> = (0..towns)
            .map(|m| {
                e2.treasury
                    .today
                    .iter()
                    .filter(|t| t.from == Account::Firm(site) && t.to == Account::Households(m))
                    .map(|t| t.amount)
                    .sum()
            })
            .collect();
        if noise(&paid) && paid.iter().all(|&x| x == 0.0) {
            continue;
        }
        let flag = if noise(&paid) { "" } else { "  <-- ASYMMETRIC" };
        println!(
            "  {:<26} paid {paid:?}  holds {:.4}{flag}",
            e2.ledger.sites[site].name,
            e2.treasury.balance(Account::Firm(site))
        );
    }

    // And what each works actually did that day, which is what decides
    // what it had to pay out of.
    println!("\nday {when}, what each works ran and took in:");
    for site in 0..e2.ledger.sites.len() {
        let took: f64 = e2
            .treasury
            .today
            .iter()
            .filter(|t| t.to == Account::Firm(site))
            .map(|t| t.amount)
            .sum();
        let spent: f64 = e2
            .treasury
            .today
            .iter()
            .filter(|t| t.from == Account::Firm(site))
            .map(|t| t.amount)
            .sum();
        if e2.ledger.sites[site].kind == SiteKind::Shop {
            let mut by: std::collections::BTreeMap<String, f64> = Default::default();
            for t in e2
                .treasury
                .today
                .iter()
                .filter(|t| t.from == Account::Firm(site))
            {
                *by.entry(format!("{:?}->{:?}", t.why, t.to)).or_default() += t.amount;
            }
            println!("    {} paid: {by:?}", e2.ledger.sites[site].name);
        }
        println!(
            "  {:<26} ran {:>14.6}  took {took:>14.4}  spent {spent:>14.4}  \
             stock {:>14.4}",
            e2.ledger.sites[site].name,
            e2.ledger.sites[site].ran,
            Commodity::ALL
                .iter()
                .map(|&c| e2.ledger.stock(site, c))
                .sum::<f64>(),
        );
    }
}
