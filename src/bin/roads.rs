//! **What the roads of a generated world are made of, and how long a haul
//! on them actually takes.**
//!
//!   cargo run --release --bin roads -- --seed 7 --nations 4
//!
//! Written to measure one change: a haul's *cost* has always read the road
//! class under every step of its path, while its *time* was kilometres
//! divided by one constant. This prints the surfaces that actually occur,
//! and every route's transit time both ways, so the size of that defect is
//! a number rather than an argument.

use scale_sim::econ::{Doctrine, Surface};
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Nations;
use scale_sim::settlement::Settlements;
use scale_sim::shipment::days_on_the_road;
use scale_sim::world::World;

fn main() {
    let mut seed = 7u64;
    let mut nations = 4usize;
    let mut towns = 4usize;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let v = it.next().and_then(|v| v.parse::<u64>().ok());
        match (a.as_str(), v) {
            ("--seed", Some(v)) => seed = v,
            ("--nations", Some(v)) => nations = v as usize,
            ("--towns", Some(v)) => towns = v as usize,
            _ => {
                eprintln!("usage: roads [--seed N] [--nations N] [--towns N]");
                return;
            }
        }
    }

    let world = World::generate(384, 216, seed);
    let polities = Polities::partition(&world, 24);
    let settlements = Settlements::place(&world, &polities, 3000);
    let network = Network::build(&world, &settlements, 500);
    let n = Nations::build(
        &world,
        &polities,
        &settlements,
        &network,
        nations,
        towns,
        Doctrine::Prudent,
    );
    let e = &n.economy;

    println!("seed {seed}, {nations} nations, {towns} towns each");
    println!("{} markets, {} roads\n", e.markets.len(), e.routes.len());

    // What the world is actually paved with.
    let all = [
        Surface::Highway,
        Surface::Road,
        Surface::Track,
        Surface::Open,
        Surface::Water,
    ];
    println!(
        "{:<14} {:>6} {:>8} {:>10}",
        "surface", "roads", "km", "pace"
    );
    for s in all {
        let roads: Vec<&_> = e.routes.iter().filter(|r| r.surface == s).collect();
        if roads.is_empty() {
            continue;
        }
        let km: f64 = roads.iter().map(|r| r.km).sum();
        println!(
            "{:<14} {:>6} {:>8.0} {:>10.2}",
            s.name(),
            roads.len(),
            km,
            s.pace()
        );
    }

    // Every pair, both ways, so the size of the change is visible.
    println!("\nwhere the two answers differ:");
    println!(
        "{:<22} {:>8} {:>9} {:>9} {:>7}",
        "route", "km", "was", "now", "worst"
    );
    let mut moved = 0usize;
    let mut pairs = 0usize;
    for a in 0..e.markets.len() {
        for b in 0..e.markets.len() {
            if a >= b {
                continue;
            }
            let km = e.routing.km(a, b);
            if !km.is_finite() {
                continue;
            }
            pairs += 1;
            let was = days_on_the_road(km);
            let now = e.routing.travel_days(a, b).floor() as u64;
            if was == now {
                continue;
            }
            moved += 1;
            // The worst surface anywhere on the path is usually the cause.
            let worst = e
                .routing
                .path_edges(a, b)
                .iter()
                .filter_map(|id| e.routes.iter().find(|r| r.id == *id))
                .map(|r| r.surface)
                .min_by(|x, y| x.pace().total_cmp(&y.pace()))
                .map(|s| s.name())
                .unwrap_or("-");
            println!(
                "{:<22} {:>8.0} {:>9} {:>9} {:>7}",
                format!(
                    "{} - {}",
                    short(&e.markets[a].name),
                    short(&e.markets[b].name)
                ),
                km,
                was,
                now,
                worst
            );
        }
    }
    println!("\n{moved} of {pairs} linked pairs now take longer than distance alone said");
}

fn short(s: &str) -> String {
    s.chars().take(9).collect()
}
