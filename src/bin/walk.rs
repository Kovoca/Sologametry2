//! Stand somewhere in the world, at a metre to the tile.
//!
//!   cargo run --release --bin walk
//!   cargo run --release --bin walk -- --seed 20260828 --rank 3 --which 4
//!   cargo run --release --bin walk -- --where shop
//!
//! The bottom of the ladder. Everything above it — a nation's grain, a
//! town's streets, a shop's tills, a lorry's seventeen metres — arrives
//! here as ground somebody is standing on.

use std::time::{SystemTime, UNIX_EPOCH};

use scale_sim::econ::Doctrine;
use scale_sim::ground::{Ground, Tile, BUBBLE_ON_FOOT};
use scale_sim::locality::Locality;
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::townplan::{Lot, Plan, TILES_PER_PLOT};
use scale_sim::vehicle::Vehicle;
use scale_sim::world::World;

fn random_seed() -> u64 {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mut z = n ^ 0x9E3779B97F4A7C15;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z ^ (z >> 31)
}

fn main() {
    let mut seed = random_seed();
    let mut rank = 3usize;
    let mut which = 4usize;
    let mut place = String::from("street");

    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => seed = it.next().and_then(|v| v.parse().ok()).unwrap_or(seed),
            "--rank" => rank = it.next().and_then(|v| v.parse().ok()).unwrap_or(3).max(1),
            "--which" => which = it.next().and_then(|v| v.parse().ok()).unwrap_or(4),
            "--where" => place = it.next().unwrap_or_else(|| "street".into()),
            "--help" | "-h" => {
                println!("usage: walk [--seed N] [--rank K] [--which 0-4] [--where street|shop|edge]");
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
    let name = region.economy.markets[m].name.clone();
    let pop = region.economy.markets[m].population;
    let cell = settlements.list[region.settlement_of_market[m]].cell;

    let loc = Locality::zoom(&world, cell);
    let ground_biome = loc.at(loc.size / 2, loc.size / 2).biome;
    let plan = Plan::lay_out_on(seed, cell, pop, 40, ground_biome);

    // Find somewhere worth standing.
    let want = match place.as_str() {
        "shop" => Lot::Shop,
        "edge" => Lot::Open,
        _ => Lot::Street,
    };
    let mut found = (plan.width / 2, plan.height / 2);
    'outer: for r in 0..plan.width {
        for y in 0..plan.height {
            for x in 0..plan.width {
                let d = (x as i64 - plan.width as i64 / 2)
                    .abs()
                    .max((y as i64 - plan.height as i64 / 2).abs());
                if d as usize == r && plan.at(x, y) == want {
                    found = (x, y);
                    break 'outer;
                }
            }
        }
    }

    let t = TILES_PER_PLOT as i64;
    let centre = (
        found.0 as i64 * t + t / 2,
        found.1 as i64 * t + t / 2,
    );

    let mut g = Ground::around(seed, &plan, centre, BUBBLE_ON_FOOT / 2);

    // Park an artic on the nearest road, because a lorry is seventeen
    // metres of the street and you cannot see that any other way.
    if let Some(spot) = nearest(&g, Tile::Road) {
        g.park(&Vehicle::artic(), spot);
    }

    println!();
    println!(
        "{name} — standing on {:?} at tile {},{}",
        want, centre.0, centre.1
    );
    println!(
        "  {} m across, generated from the seed and never stored (spec A1.5)",
        g.w
    );
    println!("  the town stands in {ground_biome:?}");
    println!();
    print!("{}", g.render(Some(centre)));
    println!();
    println!("  @ you   = road   - pavement   # wall   / door   o window   . floor");
    println!("  $ till  S shelving  R racking  L loading bay");
    println!("  \" grass  T tree  * scrub  , sand  ^ rock  ~ water");
    println!("  E engine  @ seat  o wheel  = cargo  ! controls  b battery  a alternator");
    println!();
    let here = g.at(
        (centre.0 - g.origin.0) as usize,
        (centre.1 - g.origin.1) as usize,
    );
    println!(
        "You are standing on {:?}, which is {}.",
        here,
        if here.walkable() {
            "ground you can stand on"
        } else {
            "not somewhere you can be"
        }
    );
}

fn nearest(g: &Ground, want: Tile) -> Option<(i64, i64)> {
    let (cx, cy) = (g.w as i64 / 2, g.h as i64 / 2);
    let mut best: Option<((i64, i64), i64)> = None;
    for y in 0..g.h as i64 {
        for x in 0..g.w as i64 {
            if g.at(x as usize, y as usize) != want {
                continue;
            }
            let d = (x - cx).abs() + (y - cy).abs();
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some(((g.origin.0 + x, g.origin.1 + y), d));
            }
        }
    }
    best.map(|(p, _)| p)
}
