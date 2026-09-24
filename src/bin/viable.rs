//! **Whether the viable fixture pays its way.**
//!
//!   cargo run --release --bin viable -- --days 400
//!
//! Prints, per hundred days, what households received and from where, what
//! they paid out, what they hold, and anything left unpaid or unserved —
//! the readings that distinguish an economy living on wages from a fixture
//! that happens to be solvent.

use scale_sim::econ::{Commodity, Doctrine};
use scale_sim::money::Account;
use scale_sim::slice;
use std::collections::BTreeMap;

fn main() {
    let days: u64 = std::env::args()
        .skip_while(|a| a != "--days")
        .nth(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or(400);
    let mut e = slice::viable(Doctrine::Prudent);
    let towns = [slice::HARWICK, slice::SEATON];
    let purse = |e: &scale_sim::econ::Economy| -> f64 {
        towns
            .iter()
            .map(|&m| e.treasury.balance(Account::Households(m)))
            .sum()
    };
    let mut income: BTreeMap<String, f64> = BTreeMap::new();
    let mut paid: BTreeMap<String, f64> = BTreeMap::new();
    let mut unpaid: BTreeMap<String, f64> = BTreeMap::new();
    let (mut unserved, mut cannery) = (0.0, 0.0);
    let can = slice::site(&e, slice::Role::SeatonCannery);
    let mut opening = purse(&e);
    for day in 0..days {
        e.step();
        for t in e.treasury.today.iter() {
            if matches!(t.to, Account::Households(_)) {
                *income.entry(format!("{:?}", t.why)).or_default() += t.amount;
            }
            if matches!(t.from, Account::Households(_)) {
                *paid.entry(format!("{:?}", t.why)).or_default() += t.amount;
            }
        }
        for (k, v) in e.treasury.unpaid_why.iter() {
            *unpaid.entry((*k).to_string()).or_default() += v;
        }
        unserved += e.unserved_power;
        cannery += e.ledger.sites[can].ran;
        if (day + 1) % 100 == 0 {
            let inc: f64 = income.values().sum();
            let out: f64 = paid.values().sum();
            let wages = income.get("Payroll").copied().unwrap_or(0.0);
            println!("\ndays {}-{}", day + 1 - 100, day);
            println!(
                "  households' purse {:.3e} -> {:.3e}; in {:.3e} a day, wages {:.0}%; out {:.3e} a day",
                opening,
                purse(&e),
                inc / 100.0,
                100.0 * wages / inc.max(1e-9),
                out / 100.0
            );
            for (k, v) in &income {
                println!("    in  {k:<16} {:>12.0} a day", v / 100.0);
            }
            for (k, v) in &paid {
                println!("    out {k:<16} {:>12.0} a day", v / 100.0);
            }
            for (k, v) in &unpaid {
                if *v > 0.0 {
                    println!("    unpaid {k:<13} {:>12.0} a day", v / 100.0);
                }
            }
            println!(
                "  households hold {:.3e} at Harwick and {:.3e} at Seaton",
                e.treasury.balance(Account::Households(slice::HARWICK)),
                e.treasury.balance(Account::Households(slice::SEATON))
            );
            println!(
                "  unserved power {:.2} a day; cannery {:.1} a day; food {:.1} days of cover at Seaton, price/cost {:.2}",
                unserved / 100.0,
                cannery / 100.0,
                e.opening().map_or(0.0, |o| o.cover(slice::SEATON, Commodity::ProcessedFood)),
                e.price(slice::SEATON, Commodity::ProcessedFood)
                    / e.markets[slice::SEATON].cost[Commodity::ProcessedFood as usize]
            );
            income.clear();
            paid.clear();
            unpaid.clear();
            unserved = 0.0;
            cannery = 0.0;
            opening = purse(&e);
        }
    }
}
