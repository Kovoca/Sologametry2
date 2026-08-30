//! The ground plan of one town: streets, houses, shops, works.
//!
//!   cargo run --release --bin town
//!   cargo run --release --bin town -- --seed 20260828 --rank 3 --which 4
//!
//! The rung between the world map and a building's interior. A market
//! used to be a point with a population hung off it; this is the place
//! that point stands for, laid out at 25 m to the plot — the resolution
//! CDDA's overmap works at, and the right one for "what is on this
//! street?"
//!
//! Generated, never stored (spec A1.5): the same seed and cell rebuild the
//! same town for ever.

use std::time::{SystemTime, UNIX_EPOCH};

use scale_sim::econ::Doctrine;
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::townplan::{Lot, Plan, HOUSEHOLD, METRES_PER_PLOT};
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
    let mut which = 4usize; // smallest of the five modelled towns
    let mut size = 72usize;
    // **A village has to be askable for**, because the world has none:
    // the two-thousandth settlement still holds a quarter of a million
    // people, which is a hole in `settlement.rs` and not in the layout.
    let mut pop_override: Option<f64> = None;

    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => seed = it.next().and_then(|v| v.parse().ok()).unwrap_or(seed),
            "--rank" => rank = it.next().and_then(|v| v.parse().ok()).unwrap_or(3).max(1),
            "--which" => which = it.next().and_then(|v| v.parse().ok()).unwrap_or(4),
            "--size" => size = it.next().and_then(|v| v.parse().ok()).unwrap_or(72).clamp(16, 200),
            "--pop" => pop_override = it.next().and_then(|v| v.parse().ok()),
            "--help" | "-h" => {
                println!("usage: town [--seed N] [--rank K] [--which 0-4] [--size N] [--pop N]");
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
    let Some(region) =
        Region::extract(&world, &polities, &settlements, &network, id, 5, Doctrine::Prudent)
    else {
        eprintln!("that nation has no settlements to model");
        std::process::exit(1);
    };

    let m = which.min(region.economy.markets.len() - 1);
    let name = &region.economy.markets[m].name;
    let pop = pop_override.unwrap_or(region.economy.markets[m].population);
    let cell = settlements.list[region.settlement_of_market[m]].cell;

    let plan = Plan::lay_out(seed, cell, pop, size);

    println!();
    println!(
        "{name} — {} people, {}, at {:.0} m to the plot",
        fmt_pop(pop),
        plan.pattern.name(),
        METRES_PER_PLOT
    );
    println!(
        "  {} plots across is {:.1} km of ground",
        size,
        size as f64 * METRES_PER_PLOT / 1000.0
    );
    println!();
    print!("{}", plan.render());
    println!();
    println!("  . lane  - road  = dual  # motorway   h house  H flats  S shop  W works  , park");
    println!();

    let houses = plan.count(Lot::House) + plan.count(Lot::Flats);
    println!("in this square:");
    println!(
        "  {} houses and {} blocks of flats, holding {} at {HOUSEHOLD} to a household",
        plan.count(Lot::House),
        plan.count(Lot::Flats),
        fmt_pop(plan.housed())
    );
    println!("  {} shops", plan.count(Lot::Shop));
    println!("  {} works", plan.count(Lot::Works));
    println!("  {} street plots", plan.count(Lot::Street));
    println!(
        "  {:.0} people per square kilometre where it is built up",
        plan.housed() / (size as f64 * METRES_PER_PLOT / 1000.0).powi(2)
    );
    println!();
    let per_hundred = if houses > 0 {
        plan.count(Lot::Shop) as f64 / houses as f64 * 100.0
    } else {
        0.0
    };
    println!(
        "  {per_hundred:.0} shops per hundred dwellings — a centre runs high, a country averages one"
    );
    println!();
    println!(
        "The middle {:.1} km of a town of {}. The estates and the industry are further out than this reaches.",
        size as f64 * METRES_PER_PLOT / 1000.0,
        fmt_pop(pop),
    );
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
