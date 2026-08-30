//! Walk down the scale ladder, the way Dwarf Fortress does it.
//!
//!   cargo run --release --bin zoom
//!   cargo run --release --bin zoom -- --seed 20260828 --rank 3 --which 4
//!
//! World map, then the region cell that town sits in, then the town's own
//! ground plan. Each step is generated from the seed and the coordinates
//! and none of it is stored.

use std::time::{SystemTime, UNIX_EPOCH};

use scale_sim::econ::Doctrine;
use scale_sim::locality::{Locality, METRES_PER_LOCALITY};
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::{Region, KM_PER_CELL};
use scale_sim::settlement::Settlements;
use scale_sim::townplan::{Lot, Plan, METRES_PER_PLOT};
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
    let mut which = 4usize;

    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => seed = it.next().and_then(|v| v.parse().ok()).unwrap_or(seed),
            "--rank" => rank = it.next().and_then(|v| v.parse().ok()).unwrap_or(3).max(1),
            "--which" => which = it.next().and_then(|v| v.parse().ok()).unwrap_or(4),
            "--help" | "-h" => {
                println!("usage: zoom [--seed N] [--rank K] [--which 0-4]");
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
    let (cx, cy) = (cell % world.width, cell / world.width);

    // --- 1. the world map ---
    println!();
    println!("=== 1. THE WORLD — one character to {KM_PER_CELL:.1} km ===");
    println!("{name} is the X, at cell {cx},{cy}");
    println!();
    let span = 30i32;
    for y in (cy as i32 - span / 3)..=(cy as i32 + span / 3) {
        if y < 0 || y >= world.height as i32 {
            continue;
        }
        let mut line = String::new();
        for x in (cx as i32 - span)..=(cx as i32 + span) {
            let xx = x.rem_euclid(world.width as i32) as usize;
            let i = y as usize * world.width + xx;
            line.push(if i == cell {
                'X'
            } else if world.river[i] {
                '~'
            } else {
                world_glyph(world.biomes[i])
            });
        }
        println!("  {line}");
    }

    // --- 2. the region cell, zoomed ---
    let loc = Locality::zoom(&world, cell);
    println!();
    println!(
        "=== 2. THAT CELL — one character to {:.0} m, {} across ===",
        METRES_PER_LOCALITY, loc.size
    );
    println!(
        "{:.1} km of ground. This is the scale you would walk into.",
        loc.size as f64 * METRES_PER_LOCALITY / 1000.0
    );
    println!();
    for line in loc.render().lines() {
        println!("  {line}");
    }

    // --- 3. the town itself ---
    let size = 40;
    // The town stands on the middle of its own locality, so the ground
    // showing between the streets is the ground you would have walked in.
    let centre = loc.at(loc.size / 2, loc.size / 2);
    let plan = Plan::lay_out_on(seed, cell, pop, size, centre.biome);
    println!();
    println!(
        "=== 3. THE TOWN — one character to {:.0} m, {:.1} km across ===",
        METRES_PER_PLOT,
        size as f64 * METRES_PER_PLOT / 1000.0
    );
    println!("{name}, {} people", fmt_pop(pop));
    println!();
    for line in plan.render().lines() {
        println!("  {line}");
    }
    println!();
    println!(
        "  # street   h house   H flats   S shop   W works   , park   '{}' the {:?} it stands in",
        scale_sim::townplan::ground_glyph(plan.ground),
        plan.ground,
    );
    println!();
    println!(
        "  {} houses and {} blocks of flats; {} shops.",
        plan.count(Lot::House),
        plan.count(Lot::Flats),
        plan.count(Lot::Shop),
    );
    println!();
    println!("Below this: one plot is 32 x 32 tiles of a metre each, which is where");
    println!("a person stands and where a lorry takes up seventeen of them. Not built.");
}

fn world_glyph(b: scale_sim::world::Biome) -> char {
    use scale_sim::world::Biome::*;
    match b {
        Ocean => ' ',
        Shallows => ':',
        Beach => ',',
        Desert => '.',
        Savanna => ';',
        Grassland => '"',
        Shrubland => '*',
        Forest => 'f',
        Rainforest => 'F',
        Swamp => 's',
        Taiga => 't',
        Tundra => '-',
        Mountain => '^',
        Snowcap => 'A',
    }
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
