//! A town of people living in a real nation, for a year or several.
//!
//!   cargo run --release --bin people
//!   cargo run --release --bin people -- --seed N --rank 3 --years 3 --each 40
//!
//! `life` follows one man. This follows a sampled cohort in every town of
//! a nation at once, which is the only way to see whether the person model
//! and the workforce statistics are describing the same place.

use std::time::{SystemTime, UNIX_EPOCH};

use scale_sim::econ::{Commodity, Doctrine};
use scale_sim::network::Network;
use scale_sim::person::Trade;
use scale_sim::polity::Polities;
use scale_sim::populace::Populace;
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

fn random_seed() -> u64 {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mut z = n ^ 0x9E37_79B9_7F4A_7C15;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z ^ (z >> 31)
}

fn main() {
    let mut seed = random_seed();
    let mut rank = 3usize;
    let mut years = 2u64;
    let mut each = 40usize;

    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => seed = it.next().and_then(|v| v.parse().ok()).unwrap_or(seed),
            "--rank" => rank = it.next().and_then(|v| v.parse().ok()).unwrap_or(3).max(1),
            "--years" => years = it.next().and_then(|v| v.parse().ok()).unwrap_or(2).max(1),
            "--each" => each = it.next().and_then(|v| v.parse().ok()).unwrap_or(40).max(4),
            "--help" | "-h" => {
                println!("usage: people [--seed N] [--rank K] [--years N] [--each N]");
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument: {other:?}");
                std::process::exit(2);
            }
        }
    }

    println!("generating world (seed {seed})...");
    let world = World::generate(768, 432, seed);
    let polities = Polities::partition(&world, 30);
    let settlements = Settlements::place(&world, &polities, 9000);
    let network = Network::build(&world, &settlements, 1400);

    let Some(&(id, _)) = polities.ranked().get(rank - 1) else {
        eprintln!("no nation of rank {rank}");
        std::process::exit(1);
    };
    let Some(region) = Region::extract(
        &world,
        &polities,
        &settlements,
        &network,
        id,
        5,
        Doctrine::Prudent,
    ) else {
        eprintln!("that nation has no settlements to model");
        std::process::exit(1);
    };

    let mut econ = region.economy;
    let mut folk = Populace::seed(&econ, each, seed);
    let n = folk.people.len();
    let stands_for: f64 = folk.represents.iter().sum();

    println!();
    println!(
        "{n} people in {} towns, each standing for about {:.0} others — {:.1}M in all",
        econ.markets.len(),
        stands_for / n as f64,
        stands_for / 1.0e6
    );
    println!("running {years} year(s)...");

    let days = years * scale_sim::econ::DAYS_PER_YEAR;
    for day in 0..days {
        econ.step();
        folk.live_a_day(&mut econ, day);
    }

    println!();
    println!(
        "{:<18} {:>7} {:>8} {:>8} {:>9} {:>9} {:>8}",
        "town", "sample", "worked", "hungry", "homeless", "money", "idle*"
    );
    println!("{}", "-".repeat(74));
    for m in 0..econ.markets.len() {
        let c = folk.summary(m, days);
        if c.people == 0 {
            continue;
        }
        // What the *statistics* say about the same town, for comparison.
        let stat_idle = econ.workforce[m].unemployment;
        println!(
            "{:<18} {:>7} {:>7.0}% {:>7.0}% {:>8.0}% {:>9.0} {:>7.0}%",
            econ.markets[m].name,
            c.people,
            c.worked * 100.0,
            c.hungry * 100.0,
            c.homeless * 100.0,
            c.mean_money,
            stat_idle * 100.0,
        );
    }
    println!();
    println!("labourers only — the trade the workforce statistics actually count:");
    println!("{:<18} {:>10} {:>10}", "town", "worked", "idle");
    for m in 0..econ.markets.len() {
        let w = folk.worked_by(m, Trade::Labourer, days);
        if w.is_nan() {
            continue;
        }
        println!(
            "{:<18} {:>9.0}% {:>9.0}%",
            econ.markets[m].name,
            w * 100.0,
            econ.workforce[m].unemployment * 100.0
        );
    }

    println!();
    println!("  worked  = share of the cohort's days that were paid work");
    println!("  hungry  = share who went hungry at least once");
    println!("  idle*   = what the town's own workforce statistics say, for comparison");
    println!();
    println!();
    println!("  The whole-cohort figures are not comparable with idle*: the workforce");
    println!("  statistics count the trades the *works* employ, and a shop worker is");
    println!("  rostered separately and never appears in them. A town can carry idle");
    println!("  industrial hands and busy shops at once. The labourers-only table is");
    println!("  the like-for-like comparison.");

    // Who did well and who did not.
    let mut sorted: Vec<scale_sim::id::Id<scale_sim::person::Person>> =
        folk.people.ids().collect();
    sorted.sort_by(|&a, &b| {
        folk.people[b]
            .money
            .total_cmp(&folk.people[a].money)
            .then(a.cmp(&b))
    });
    let show = |i: scale_sim::id::Id<scale_sim::person::Person>| {
        let p = &folk.people[i];
        println!(
            "  {:<20} {:<11} {:<16} {:>8.0}  worked {:>4}d, hungry {:>3}d",
            p.name,
            trade_name(p.trade),
            econ.markets[p.market].name,
            p.money,
            p.days_worked,
            p.days_hungry,
        );
    };
    println!();
    println!("best off:");
    for &i in sorted.iter().take(3) {
        show(i);
    }
    println!("worst off:");
    for &i in sorted.iter().rev().take(3) {
        show(i);
    }

    if !folk.gone.is_empty() {
        println!();
        println!("{} died and were replaced:", folk.gone.len());
        for (name, day) in folk.gone.iter().take(6) {
            println!("  {name} on day {day}");
        }
    }

    println!();
    let bread = econ.price(0, Commodity::ProcessedFood);
    econ.ledger.assert_conserved();
    println!(
        "food in {} ended at {bread:.2} a tonne, and the ledger balances.",
        econ.markets[0].name,
    );
}

fn trade_name(t: Trade) -> &'static str {
    t.name()
}

#[allow(dead_code)]
fn unused(t: Trade) -> &'static str {
    match t {
        Trade::Haulier => "haulier",
        Trade::Labourer => "labourer",
        Trade::Shopworker => "shop work",
        Trade::Supervisor => "supervisor",
        Trade::Public => "public",
        _ => "other",
    }
}
