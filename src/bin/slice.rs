//! Runs the vertical-slice scenario and prints what happens.
//!
//!   cargo run --release --bin slice
//!   cargo run --release --bin slice -- --grid minimal --days 60
//!
//! The default run cuts a transmission line on day 20 and repairs it on
//! day 45, so the whole cascade from spec Part D's acceptance test is
//! visible in one table.

use scale_sim::econ::{Commodity, Economy};
use scale_sim::slice::{self, GridPlan};

struct Args {
    days: u64,
    plan: GridPlan,
    fail_on: u64,
    repair_on: u64,
}

fn parse_args() -> Args {
    let mut args = Args {
        days: 70,
        plan: GridPlan::Minimal,
        fail_on: 20,
        repair_on: 45,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--days" => args.days = it.next().and_then(|v| v.parse().ok()).unwrap_or(70),
            "--grid" => {
                args.plan = match it.next().as_deref() {
                    Some("redundant") => GridPlan::Redundant,
                    Some("minimal") | None => GridPlan::Minimal,
                    Some(other) => {
                        eprintln!("bad --grid value: {other:?} (expected redundant or minimal)");
                        std::process::exit(2);
                    }
                }
            }
            "--fail-on" => args.fail_on = it.next().and_then(|v| v.parse().ok()).unwrap_or(20),
            "--repair-on" => {
                args.repair_on = it.next().and_then(|v| v.parse().ok()).unwrap_or(45)
            }
            "--help" | "-h" => {
                println!(
                    "usage: slice [--days N] [--grid redundant|minimal] \
                     [--fail-on DAY] [--repair-on DAY]"
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument: {other:?}");
                std::process::exit(2);
            }
        }
    }
    args
}

fn main() {
    let args = parse_args();
    let mut econ = slice::build(args.plan);

    let peak = 40.0; // rough daily peak load for the N-1 report
    println!(
        "grid: {} line(s), {:.0} MWh/day deliverable, N-1 {}",
        econ.grid.lines.len(),
        econ.grid.capacity(),
        if econ.grid.survives_n1(peak) {
            "satisfied"
        } else {
            "NOT satisfied — one failure blacks out industry"
        }
    );
    println!(
        "towns: {} ({:.0}k people), {} ({:.0}k people), joined by one road \
         at {:.0}/unit freight\n",
        econ.markets[0].name,
        econ.markets[0].population / 1000.0,
        econ.markets[1].name,
        econ.markets[1].population / 1000.0,
        econ.routes[0].freight_cost,
    );

    println!(
        " day │ food: Ashford   Bexley │ cover A  cover B │ haul │ power │ event"
    );
    println!("─────┼────────────────────────┼──────────────────┼──────┼───────┼──────────────");

    for day in 0..args.days {
        let mut note = String::new();
        if day == args.fail_on && econ.grid.fail_line("Kelling line A") {
            note = "line A fails".into();
        }
        if day == args.repair_on && econ.grid.restore_line("Kelling line A") {
            note = "line A repaired".into();
        }

        econ.step();

        let f = Commodity::ProcessedFood;
        let pa = econ.price(slice::ASHFORD, f);
        let pb = econ.price(slice::BEXLEY, f);
        let arb = econ.arbitrage(0, f);
        let shed = econ.unserved_power;

        // Only print days worth looking at: the start, anything eventful,
        // and a weekly sample.
        let interesting = !note.is_empty()
            || day < 3
            || day % 7 == 0
            || day + 1 == args.days;
        if !interesting {
            continue;
        }

        println!(
            "{:>4} │ {:>13.0} {:>8.0} │ {:>7.1} {:>8.1} │ {:>4} │ {:>5} │ {}",
            day,
            pa,
            pb,
            econ.markets[slice::ASHFORD].cover[f as usize],
            econ.markets[slice::BEXLEY].cover[f as usize],
            if arb > 0.0 { "yes" } else { "-" },
            if shed > 0.01 { "SHED" } else { "ok" },
            note,
        );
    }

    println!();
    summary(&econ);
}

fn summary(econ: &Economy) {
    println!("after {} days:", econ.ledger.day);
    for &c in &[
        Commodity::Grain,
        Commodity::Flour,
        Commodity::ProcessedFood,
        Commodity::RetailGoods,
    ] {
        println!(
            "  {:<7} {:>9.0} {} held   Ashford {:>7.0}   Bexley {:>7.0}",
            c.name(),
            econ.ledger.total(c),
            c.unit(),
            econ.price(slice::ASHFORD, c),
            econ.price(slice::BEXLEY, c),
        );
    }

    let hungry = econ.unmet_demand[Commodity::ProcessedFood as usize];
    if hungry > 0.01 {
        println!("  {hungry:.1} t of food demand went unmet today — people going without");
    }
    println!("  {} journal entries", econ.journal.len());
    econ.ledger.assert_conserved();
    println!("  conservation: OK");
}
