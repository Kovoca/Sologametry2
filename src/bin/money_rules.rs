//! **The four money rules, side by side on one small economy.**
//!
//! `cargo run --release --bin money_rules -- --days 730`
//!
//! Households budgeting before they shop, or shopping first; firms buying on
//! agreed trade credit, or for cash on delivery. Every one of the four
//! accounts for every delivery — paid, owed on the book, or refused before
//! it moved — so what differs between them is behaviour and not bookkeeping,
//! and the table reads production and consumption **beside** what is owed,
//! overdue and charged off. A recovery sustained by debts piling up that
//! nobody will pay is not a recovery, and the columns are there to show one.
//!
//! The fixture is `slice::viable`: two towns that each sell the other
//! something, small enough that a difference has somewhere to be seen.

use scale_sim::econ::{Commodity, Doctrine, Economy, Experiments, SiteKind};
use scale_sim::money::{Account, Why};
use scale_sim::slice;

struct Tally {
    label: String,
    ran_share: f64,
    food_bought: f64,
    goods_bought: f64,
    went_without_goods: f64,
    unemployment: f64,
    payroll_met: f64,
    households: f64,
    firms: f64,
    owed_firms: f64,
    overdue_firms: f64,
    owed_households: f64,
    overdue_households: f64,
    charged_off: f64,
    forgotten: f64,
    paid_abroad: f64,
    from_abroad: f64,
}

fn run(rules: Experiments, days: u64) -> Tally {
    let mut e: Economy = slice::viable(Doctrine::Prudent);
    e.experiments = rules;
    let mut ran_share = 0.0;
    let mut ran_days = 0.0f64;
    let mut food = 0.0;
    let mut goods = 0.0;
    let mut without_goods = 0.0;
    let mut unemployment = 0.0;
    let mut forgotten = 0.0;
    let mut paid_abroad = 0.0;
    let mut from_abroad = 0.0;
    for _ in 0..days {
        e.step();
        // How far the works ran against their rating, over works that have
        // a rating to run against.
        for s in e.ledger.sites.iter() {
            if s.kind == SiteKind::PowerPlant || s.throughput <= 0.0 || s.recipe.is_none() {
                continue;
            }
            ran_share += (s.ran / s.throughput).min(2.0);
            ran_days += 1.0;
        }
        for m in 0..e.markets.len() {
            food += e.markets[m].daily_household_demand(Commodity::ProcessedFood);
            goods += e.markets[m].daily_household_demand(Commodity::RetailGoods);
        }
        food -= e.went_without[Commodity::ProcessedFood as usize]
            + e.unmet_demand[Commodity::ProcessedFood as usize];
        goods -= e.went_without[Commodity::RetailGoods as usize]
            + e.unmet_demand[Commodity::RetailGoods as usize];
        without_goods += e.went_without[Commodity::RetailGoods as usize];
        let (idle, hands) = e.workforce.iter().fold((0.0, 0.0), |(i, h), w| {
            (i + w.unemployment * w.hands, h + w.hands)
        });
        unemployment += if hands > 0.0 { idle / hands } else { 0.0 };
        // What went unpaid and is owed by nobody — the defect every rule
        // here is meant to have closed. Capital crossing a border is a
        // record of what crossed, not a bill.
        forgotten += e
            .treasury
            .unpaid_by
            .iter()
            .filter(|((_, _, why), _)| *why != "capital")
            .map(|(_, v)| v)
            .sum::<f64>();
        for t in e.treasury.today.iter() {
            if t.why == Why::Capital {
                continue;
            }
            if t.to == Account::Abroad {
                paid_abroad += t.amount;
            }
            if t.from == Account::Abroad {
                from_abroad += t.amount;
            }
        }
    }
    let d = days as f64;
    let day = e.ledger.day;
    let kind = |a: Account| match a {
        Account::Firm(_) | Account::ServiceSector(_) => 0,
        Account::Households(_) => 1,
        _ => 2,
    };
    let mut owed = [0.0f64; 3];
    let mut overdue = [0.0f64; 3];
    for (_, i) in e.obligations.iter() {
        owed[kind(i.debtor)] += i.outstanding();
        overdue[kind(i.debtor)] += i.overdue_on(day);
    }
    let held = |f: &dyn Fn(&Account) -> bool| -> f64 {
        e.treasury
            .accounts()
            .filter(|(a, _)| f(a))
            .map(|(_, v)| *v)
            .sum()
    };
    let met: Vec<f64> = e.payroll_met.clone();
    Tally {
        label: rules.money_label(),
        ran_share: ran_share / ran_days.max(1.0),
        food_bought: food / d,
        goods_bought: goods / d,
        went_without_goods: without_goods / d,
        unemployment: unemployment / d,
        payroll_met: if met.is_empty() {
            1.0
        } else {
            met.iter().sum::<f64>() / met.len() as f64
        },
        households: held(&|a| matches!(a, Account::Households(_))),
        firms: held(&|a| matches!(a, Account::Firm(_))),
        owed_firms: owed[0],
        overdue_firms: overdue[0],
        owed_households: owed[1],
        overdue_households: overdue[1],
        charged_off: e.obligations.charged_off,
        forgotten,
        paid_abroad,
        from_abroad,
    }
}

fn main() {
    let mut days = 730u64;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--days" => days = it.next().and_then(|v| v.parse().ok()).unwrap_or(days),
            "--help" | "-h" => {
                println!("usage: money_rules [--days N]");
                return;
            }
            other => {
                eprintln!("unknown argument: {other:?}");
                std::process::exit(2);
            }
        }
    }
    println!("slice::viable, {days} days, under each of the four money rules\n");
    let rows: Vec<Tally> = Experiments::money_rules()
        .into_iter()
        .map(|r| run(r, days))
        .collect();
    let line = |name: &str, f: &dyn Fn(&Tally) -> String| {
        print!("  {name:<30}");
        for r in rows.iter() {
            print!(" {:>16}", f(r));
        }
        println!();
    };
    print!("  {:<30}", "");
    for r in rows.iter() {
        print!(" {:>16}", r.label.replace(" + ", "/"));
    }
    println!("\n");
    line("works ran, share of rating", &|r| format!("{:.1}%", r.ran_share * 100.0));
    line("food bought a day, t", &|r| format!("{:.1}", r.food_bought));
    line("goods bought a day, t", &|r| format!("{:.2}", r.goods_bought));
    line("goods gone without a day, t", &|r| {
        format!("{:.2}", r.went_without_goods)
    });
    line("unemployment, mean", &|r| format!("{:.1}%", r.unemployment * 100.0));
    line("payroll met at the end", &|r| format!("{:.1}%", r.payroll_met * 100.0));
    line("households hold", &|r| format!("{:.3e}", r.households));
    line("firms hold", &|r| format!("{:.3e}", r.firms));
    line("firms owe", &|r| format!("{:.3e}", r.owed_firms));
    line("  of it overdue", &|r| format!("{:.3e}", r.overdue_firms));
    line("households owe", &|r| format!("{:.3e}", r.owed_households));
    line("  of it overdue", &|r| format!("{:.3e}", r.overdue_households));
    line("charged off, whole run", &|r| format!("{:.3e}", r.charged_off));
    line("failed and forgotten", &|r| format!("{:.3e}", r.forgotten));
    line("paid abroad", &|r| format!("{:.3e}", r.paid_abroad));
    line("received from abroad", &|r| format!("{:.3e}", r.from_abroad));
}
