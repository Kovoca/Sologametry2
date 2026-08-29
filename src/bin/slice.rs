//! Runs the vertical-slice scenario and prints what happens.
//!
//!   cargo run --release --bin slice
//!   cargo run --release --bin slice -- --doctrine prudent
//!   cargo run --release --bin slice -- --fault transformer --days 200
//!
//! Something is broken on the chosen day. Nothing is repaired on a
//! schedule: a fault has to be noticed, reported over working comms,
//! assigned to a crew, and travelled to before any work starts — so how
//! long the lights are out is a consequence of how the region is run.

use scale_sim::econ::{Commodity, Economy, Fault};
use scale_sim::slice::{self, Doctrine};

#[derive(Copy, Clone, PartialEq, Eq)]
enum FaultKind {
    Line,
    Transformer,
}

struct Args {
    days: u64,
    doctrine: Doctrine,
    fault: FaultKind,
    fail_on: u64,
    comms_out: bool,
}

fn parse_args() -> Args {
    let mut args = Args {
        days: 90,
        doctrine: Doctrine::Negligent,
        fault: FaultKind::Line,
        fail_on: 20,
        comms_out: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--days" => args.days = it.next().and_then(|v| v.parse().ok()).unwrap_or(90),
            "--fail-on" => args.fail_on = it.next().and_then(|v| v.parse().ok()).unwrap_or(20),
            "--comms-out" => args.comms_out = true,
            "--doctrine" => {
                args.doctrine = match it.next().as_deref() {
                    Some("prudent") => Doctrine::Prudent,
                    Some("negligent") | None => Doctrine::Negligent,
                    Some(o) => {
                        eprintln!("bad --doctrine {o:?} (expected prudent or negligent)");
                        std::process::exit(2);
                    }
                }
            }
            "--fault" => {
                args.fault = match it.next().as_deref() {
                    Some("transformer") => FaultKind::Transformer,
                    Some("line") | None => FaultKind::Line,
                    Some(o) => {
                        eprintln!("bad --fault {o:?} (expected line or transformer)");
                        std::process::exit(2);
                    }
                }
            }
            "--help" | "-h" => {
                println!(
                    "usage: slice [--days N] [--doctrine prudent|negligent]\n\
                     \x20            [--fault line|transformer] [--fail-on DAY] [--comms-out]"
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
    let mut econ = slice::build(args.doctrine);
    econ.response.comms_up = !args.comms_out;

    let peak = 40.0;
    println!(
        "state: {:?} — {} line(s), N-1 {}, {} crew(s) at a depot {:.0} km away, \
         {} spare transformer(s)",
        args.doctrine,
        econ.grid.lines.len(),
        if econ.grid.survives_n1(peak) { "satisfied" } else { "NOT satisfied" },
        econ.response.crews,
        econ.response.depot_km,
        econ.response.spare_transformers,
    );
    println!(
        "       depot is {}",
        match econ.response.travel_days() {
            0 => "close enough that crews arrive the same day".to_string(),
            n => format!("far enough that crews spend {n} day(s) on the road"),
        }
    );
    if args.comms_out {
        println!("comms are down — nothing can be reported");
    }
    println!(
        "towns: {} ({:.0}k), {} ({:.0}k), one road at {:.0}/unit freight\n",
        econ.markets[0].name,
        econ.markets[0].population / 1000.0,
        econ.markets[1].name,
        econ.markets[1].population / 1000.0,
        econ.routes[0].freight_cost,
    );

    println!(" day | food: Ashford   Bexley | cover A  cover B | haul | power | event");
    println!("-----+------------------------+------------------+------+-------+-----------------");

    let f = Commodity::ProcessedFood;
    let mut last_state = String::new();

    for day in 0..args.days {
        let mut note = String::new();
        if day == args.fail_on {
            match args.fault {
                FaultKind::Line => {
                    econ.grid.fail_line("Kelling line A");
                    note = "line A down".into();
                }
                FaultKind::Transformer => {
                    econ.grid.fail_transformer("Kelling line A");
                    note = "transformer destroyed".into();
                }
            }
        }

        econ.step();

        // Report the moment the response chain changes state.
        if let Some(inc) = econ.response.incidents.last() {
            let state = if inc.resolved.is_some() {
                "repaired"
            } else if inc.in_transit(econ.ledger.day) {
                "crew travelling"
            } else if inc.dispatched.is_some() {
                "crew on site"
            } else if inc.reported.is_some() {
                "reported, waiting"
            } else {
                "unreported"
            };
            if state != last_state {
                if note.is_empty() {
                    note = state.to_string();
                } else {
                    note = format!("{note} — {state}");
                }
                last_state = state.to_string();
            }
        }

        let interesting =
            !note.is_empty() || day < 2 || day % 14 == 0 || day + 1 == args.days;
        if !interesting {
            continue;
        }

        println!(
            "{:>4} | {:>13.0} {:>8.0} | {:>7.1} {:>8.1} | {:>4} | {:>5} | {}",
            day,
            econ.price(slice::ASHFORD, f),
            econ.price(slice::BEXLEY, f),
            econ.markets[slice::ASHFORD].cover[f as usize],
            econ.markets[slice::BEXLEY].cover[f as usize],
            if econ.arbitrage(0, f) > 0.0 { "yes" } else { "-" },
            if econ.unserved_power > 0.01 { "SHED" } else { "ok" },
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

    for inc in &econ.response.incidents {
        let what = match &inc.what {
            Fault::Line(n) => format!("line {n}"),
            Fault::Transformer(n) => format!("transformer at {n}"),
        };
        match (inc.reported, inc.resolved) {
            (None, _) => println!("  {what}: still unreported after {} days", econ.ledger.day - inc.occurred),
            (Some(_), None) => println!("  {what}: reported, still out"),
            (Some(_), Some(done)) => println!(
                "  {what}: out for {} days ({} to notice and reach it, {} to fix)",
                done - inc.occurred,
                inc.arrives.unwrap_or(done) - inc.occurred,
                inc.work_days,
            ),
        }
    }

    let hungry = econ.unmet_demand[Commodity::ProcessedFood as usize];
    if hungry > 0.01 {
        println!("  {hungry:.1} t of food demand went unmet today");
    }
    println!("  {} journal entries", econ.journal.len());
    econ.ledger.assert_conserved();
    println!("  conservation: OK");
}
