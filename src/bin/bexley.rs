//! **What Bexley takes in, what it wants, and what it actually pays.**
//!
//!   cargo run --release --bin bexley -- --days 400
//!
//! Two figures for Bexley's gap were reported — about 97,500 a day and
//! 3,233 a day — and they measured different things. This prints the four
//! quantities apart, per window:
//!
//! - **income**: what reached the town's households, by reason;
//! - **wanted**: the day's counter basket and the day's power bill at the
//!   town's own prices, whatever the purse holds;
//! - **paid**: what actually left the purse, by reason — a transfer
//!   records what moved, which `Treasury::pay` caps at the balance;
//! - **left unpaid or gone without**: the difference between the two.
//!
//! Bexley is kept as the fixture's **distressed town** on purpose: a place
//! with twenty-six thousand people, one shop and no works, living on the
//! profit of that shop. A world has to be able to contain one.

use scale_sim::econ::{Commodity, Doctrine};
use scale_sim::money::Account;
use scale_sim::slice;
use std::collections::BTreeMap;

const AT_THE_TILL: [Commodity; 4] = [
    Commodity::ProcessedFood,
    Commodity::Meat,
    Commodity::Remedies,
    Commodity::RetailGoods,
];

#[derive(Default)]
struct Window {
    days: u64,
    income: BTreeMap<String, f64>,
    paid: BTreeMap<String, f64>,
    wanted_till: f64,
    wanted_power: f64,
    opening: f64,
    closing: f64,
}

fn main() {
    let days: u64 = std::env::args()
        .skip_while(|a| a != "--days")
        .nth(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or(400);
    let town = slice::BEXLEY;
    let mut e = slice::build(Doctrine::Prudent);
    let windows: Vec<(u64, u64)> = vec![(0, 40), (40, 41), (days.saturating_sub(100), days)];
    let mut out: Vec<Window> = windows.iter().map(|_| Window::default()).collect();

    for day in 0..days {
        let before = e.treasury.balance(Account::Households(town));
        // What the day asks of the purse, read at the morning's prices.
        let till: f64 = AT_THE_TILL
            .iter()
            .map(|&c| e.markets[town].daily_household_demand(c) * e.price(town, c))
            .sum();
        let power = e.markets[town].daily_household_demand(Commodity::Electricity)
            * e.price(town, Commodity::Electricity);
        e.step();
        let after = e.treasury.balance(Account::Households(town));
        for (w, &(a, b)) in out.iter_mut().zip(windows.iter()) {
            if day < a || day >= b {
                continue;
            }
            if day == a {
                w.opening = before;
            }
            w.closing = after;
            w.days += 1;
            w.wanted_till += till;
            w.wanted_power += power;
            for t in &e.treasury.today {
                if t.to == Account::Households(town) {
                    *w.income.entry(format!("{:?}", t.why)).or_default() += t.amount;
                }
                if t.from == Account::Households(town) {
                    *w.paid.entry(format!("{:?}", t.why)).or_default() += t.amount;
                }
            }
        }
    }

    // **Which of the shortfall was money and which was an empty shelf.**
    // `unmet_demand` is wanted and not on any shelf; `went_without` is on
    // the shelf and unaffordable. Both are fixture-wide and daily, so they
    // are read on a fresh run over the first window only.
    let mut f = slice::build(Doctrine::Prudent);
    let mut unmet = [0.0f64; 4];
    let mut without = [0.0f64; 4];
    for _ in 0..40 {
        f.step();
        for (i, &c) in AT_THE_TILL.iter().enumerate() {
            unmet[i] += f.unmet_demand[c as usize] * f.price(town, c);
            without[i] += f.went_without[c as usize] * f.price(town, c);
        }
    }
    println!("fixture-wide, days 0-39, a day on average (both towns):");
    for (i, &c) in AT_THE_TILL.iter().enumerate() {
        let shop = slice::site(&f, slice::Role::BexleyStore);
        println!(
            "  {:<14} not on any shelf {:>10.1}   on a shelf, unaffordable {:>10.1}   Bexley shop holds {:>8.1} t",
            format!("{c:?}"),
            unmet[i] / 40.0,
            without[i] / 40.0,
            f.ledger.stock(shop, c)
        );
    }

    for (w, &(a, b)) in out.iter().zip(windows.iter()) {
        let n = w.days.max(1) as f64;
        let income: f64 = w.income.values().sum();
        let paid: f64 = w.paid.values().sum();
        let wanted = w.wanted_till + w.wanted_power;
        println!(
            "\nBexley households, days {a}-{} ({} days), a day on average",
            b - 1,
            w.days
        );
        println!(
            "  purse                 {:>12.1} -> {:>12.1}",
            w.opening, w.closing
        );
        for (k, v) in &w.income {
            println!("  in   {k:<16} {:>12.1}", v / n);
        }
        println!("  in   total            {:>12.1}", income / n);
        println!("  wanted: counter       {:>12.1}", w.wanted_till / n);
        println!("  wanted: power bill    {:>12.1}", w.wanted_power / n);
        println!("  wanted: total         {:>12.1}", wanted / n);
        for (k, v) in &w.paid {
            println!("  paid {k:<16} {:>12.1}", v / n);
        }
        println!("  paid total            {:>12.1}", paid / n);
        println!("  income - paid         {:>12.1}", (income - paid) / n);
        println!("  income - wanted       {:>12.1}", (income - wanted) / n);
        println!("  wanted and not paid   {:>12.1}", (wanted - paid) / n);
    }
}
