//! Generate a planet, pick one of its nations, and run that nation's
//! economy.
//!
//!   cargo run --release --bin region
//!   cargo run --release --bin region -- --seed 12345 --rank 3
//!   cargo run --release --bin region -- --doctrine prudent --fault transformer
//!
//! Unlike `--bin slice`, nothing here is hand-placed. The farms are sized
//! by the soil those cities actually draw on, the colliery exists only if
//! the geology put coal in reach, and the freight costs come from the real
//! distances between the towns and whether there is water between them.

use std::time::{SystemTime, UNIX_EPOCH};

use scale_sim::econ::{Commodity, SiteKind};
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::slice::Doctrine;
use scale_sim::world::World;

const FOOD: Commodity = Commodity::ProcessedFood;

struct Args {
    seed: u64,
    rank: usize,
    days: u64,
    markets: usize,
    doctrine: Doctrine,
    fault: Option<&'static str>,
    fail_on: u64,
}

fn random_seed() -> u64 {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mut z = n ^ 0x9E3779B97F4A7C15;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z ^ (z >> 31)
}

fn parse_args() -> Args {
    let mut a = Args {
        seed: random_seed(),
        rank: 1,
        days: 120,
        markets: 5,
        doctrine: Doctrine::Negligent,
        fault: None,
        fail_on: 30,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--seed" => a.seed = it.next().and_then(|v| v.parse().ok()).unwrap_or(a.seed),
            "--rank" => a.rank = it.next().and_then(|v| v.parse().ok()).unwrap_or(1).max(1),
            "--days" => a.days = it.next().and_then(|v| v.parse().ok()).unwrap_or(120),
            "--markets" => a.markets = it.next().and_then(|v| v.parse().ok()).unwrap_or(5).max(1),
            "--fail-on" => a.fail_on = it.next().and_then(|v| v.parse().ok()).unwrap_or(30),
            "--doctrine" => {
                a.doctrine = match it.next().as_deref() {
                    Some("prudent") => Doctrine::Prudent,
                    _ => Doctrine::Negligent,
                }
            }
            "--fault" => {
                a.fault = match it.next().as_deref() {
                    Some("line") => Some("line"),
                    Some("transformer") => Some("transformer"),
                    _ => None,
                }
            }
            "--help" | "-h" => {
                println!(
                    "usage: region [--seed N] [--rank K] [--markets N] [--days N]\n\
                     \x20             [--doctrine prudent|negligent] [--fault line|transformer]\n\
                     \n\
                     --rank K   which nation, by size (1 = largest)"
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument: {other:?}");
                std::process::exit(2);
            }
        }
    }
    a
}

fn main() {
    let args = parse_args();

    println!("generating world (seed {})...", args.seed);
    let world = World::generate(768, 432, args.seed);
    let polities = Polities::partition(&world, 30);
    let settlements = Settlements::place(&world, &polities, 9000);
    let network = Network::build(&world, &settlements, 1400);

    let ranked = polities.ranked();
    let Some(&(id, pol)) = ranked.get(args.rank - 1) else {
        eprintln!("no nation of rank {}", args.rank);
        std::process::exit(1);
    };

    let Some(mut region) = Region::extract(
        &world,
        &polities,
        &settlements,
        &network,
        id,
        args.markets,
        args.doctrine,
    ) else {
        eprintln!("nation {id} has no settlements to model");
        std::process::exit(1);
    };

    println!();
    println!(
        "nation #{} of {}: {} cells, {:.0} food capacity, {} workable deposits",
        args.rank,
        ranked.len(),
        pol.cells,
        pol.food,
        pol.workable_deposits(),
    );
    for n in &region.notes {
        println!("  {n}");
    }
    println!();

    println!("markets");
    for m in &region.economy.markets {
        println!("  {:<12} {:>10} people", m.name, fmt_pop(m.population));
    }
    println!("  {} in all", fmt_pop(region.population()));
    println!();

    println!("works");
    for s in &region.economy.ledger.sites {
        if s.recipe.is_some() {
            println!(
                "  {:<24} {:>10.0} /day",
                s.name,
                s.throughput.min(1e6)
            );
        }
    }
    println!();

    if !region.economy.routes.is_empty() {
        println!("routes");
        for r in &region.economy.routes {
            println!("  {:<44} {:>7.0} /t freight", r.name, r.freight_cost);
        }
        println!();
    }

    if !region.has_generation() {
        println!("this nation has no power station; expect nothing to work.\n");
    }

    // Run it.
    let e = &mut region.economy;
    println!(
        " day | season | harvest |  grain | grain cover | food | food cover | haul | event"
    );
    println!(
        "-----+--------+---------+--------+-------------+------+------------+------+---------"
    );

    let mut last = String::new();
    for day in 0..args.days {
        let mut note = String::new();
        if day == args.fail_on {
            match args.fault {
                Some("line") => {
                    e.grid.fail_line("main line");
                    note = "line down".into();
                }
                Some("transformer") => {
                    e.grid.fail_transformer("main line");
                    note = "transformer destroyed".into();
                }
                _ => {}
            }
        }
        e.step();

        if let Some(inc) = e.response.incidents.last() {
            let s = if inc.resolved.is_some() {
                "repaired"
            } else if inc.in_transit(e.ledger.day) {
                "crew travelling"
            } else if inc.dispatched.is_some() {
                "crew on site"
            } else if inc.reported.is_some() {
                "reported"
            } else {
                "unreported"
            };
            if s != last {
                note = if note.is_empty() { s.into() } else { format!("{note} - {s}") };
                last = s.to_string();
            }
        }

        // Any market where a haul is worth taking.
        let haul = (0..e.routes.len()).any(|r| e.arbitrage(r, FOOD) > 0.0);

        if !note.is_empty() || day < 2 || day % 20 == 0 || day + 1 == args.days {
            let grain = Commodity::Grain;
            println!(
                "{:>4} | {:<6} | {:>7.2} | {:>6.0} | {:>11.0} | {:>4.0} | {:>10.1} | {:>4} | {}{}",
                day,
                e.season().name(),
                e.harvest_today(),
                e.price(0, grain),
                e.markets[0].cover[grain as usize],
                e.price(0, FOOD),
                e.markets[0].cover[FOOD as usize],
                if haul { "yes" } else { "-" },
                if e.unserved_power > 0.01 { "SHED " } else { "" },
                note,
            );
        }
    }

    println!();
    println!("after {} days:", region.economy.ledger.day);
    for m in 0..region.economy.markets.len() {
        println!(
            "  {:<12} food {:>8.0}   cover {:>4.1} days",
            region.economy.markets[m].name,
            region.economy.price(m, FOOD),
            region.economy.markets[m].cover[FOOD as usize],
        );
    }
    let hungry = region.economy.unmet_demand[FOOD as usize];
    if hungry > 0.01 {
        println!("  {hungry:.0} t of food demand unmet today");
    }
    let mines = region
        .economy
        .ledger
        .sites
        .iter()
        .filter(|s| s.kind == SiteKind::Mine)
        .count();
    println!(
        "  {} collieries, {} journal entries",
        mines,
        region.economy.journal.len()
    );
    region.economy.ledger.assert_conserved();
    println!("  conservation: OK");
}

fn fmt_pop(p: f64) -> String {
    if p >= 1.0e6 {
        format!("{:.1}M", p / 1.0e6)
    } else if p >= 1.0e3 {
        format!("{:.0}k", p / 1.0e3)
    } else {
        format!("{p:.0}")
    }
}
