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

    let mut plain = false;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => seed = it.next().and_then(|v| v.parse().ok()).unwrap_or(seed),
            "--rank" => rank = it.next().and_then(|v| v.parse().ok()).unwrap_or(3).max(1),
            "--which" => which = it.next().and_then(|v| v.parse().ok()).unwrap_or(4),
            "--plain" => plain = true,
            "--help" | "-h" => {
                println!("usage: zoom [--seed N] [--rank K] [--which 0-4] [--plain]");
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

    let (_elevation_m, _relief_m) = ground_of(&world, cell);
    // --- 2. the region cell, zoomed ---
    // **How high, and how much it moves.** Absolute height from the
    // elevation field against a real ceiling, and relief from how fast the
    // field changes across the neighbouring cells — which is what decides
    // whether the ground under a town is flat or a hillside.
    let (elevation_m, relief_m) = ground_of(&world, cell);
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
    for line in if plain { loc.render() } else { loc.render_in_colour() }.lines() {
        println!("  {line}");
    }

    // --- 3. the town itself ---
    let size = 40;
    // The town stands on the middle of its own locality, so the ground
    // showing between the streets is the ground you would have walked in.
    let centre = loc.at(loc.size / 2, loc.size / 2);
    let plan = Plan::lay_out_on(seed, cell, pop, size, centre.biome).on_rock(world.geology.rock[cell])
        .on_ground(elevation_m, relief_m)
        .with_water_at(world.depth_to_water_m(cell));
    println!();
    println!(
        "=== 3. THE TOWN — one character to {:.0} m, {:.1} km across ===",
        METRES_PER_PLOT,
        size as f64 * METRES_PER_PLOT / 1000.0
    );
    println!("{name}, {} people", fmt_pop(pop));
    println!();
    for line in if plain { plan.render() } else { plan.render_in_colour() }.lines() {
        println!("  {line}");
    }
    println!();
    println!("{}", scale_sim::townplan::plan_legend_in(plan.ground, !plain));
    println!();
    println!(
        "  {} houses and {} blocks of flats; {} shops.",
        plan.count(Lot::House),
        plan.count(Lot::Flats),
        plan.count(Lot::Shop),
    );
    println!();
    println!("Below this: one plot is 32 x 32 tiles of a metre each, which is where");
    println!("a person stands and a lorry takes up seventeen of them.  --bin walk");
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

/// **Height and relief at a cell, in metres.**
///
/// Absolute height comes from the elevation field against a real ceiling
/// *(Everest, 8,848 m)*.
///
/// **Relief is not the regional gradient**, which was the first thing I
/// got wrong: the coarse field is smoothed at 16 km, so the difference
/// between neighbouring cells gave a town at 5,380 m in mountain country
/// a relief of 11 m/km. A mountain cell contains peaks and valleys the
/// coarse field never resolved. Local relief is a property of the
/// landform, so it comes from the biome — real figures for height range
/// within a kilometre:
///
/// | | m/km |
/// |---|---|
/// | marsh, beach, floodplain | 2-10 |
/// | plains — steppe, savanna, desert | 10-20 |
/// | rolling country — forest, scrub | 30-60 |
/// | mountain | 300-600 |
/// | high peaks | 500-900 |
///
/// The regional gradient is then *added*, because a mountainside that is
/// also on a steep regional slope is steeper still.
fn ground_of(world: &World, cell: usize) -> (f64, f64) {
    use scale_sim::world::Biome::*;
    let (w, h) = (world.width, world.height);
    let (cx, cy) = (cell % w, cell / w);
    let at = |x: i64, y: i64| -> f64 {
        let xx = x.rem_euclid(w as i64) as usize;
        let yy = y.clamp(0, h as i64 - 1) as usize;
        world.elevation.data[yy * w + xx] as f64
    };
    let here = at(cx as i64, cy as i64);
    let sea = world.sea_level as f64;
    let above = ((here - sea) / (1.0 - sea).max(1e-3)).max(0.0);
    let elevation_m = above * scale_sim::world::MAX_LAND_M;

    let landform = match world.biomes[cell] {
        Ocean | Shallows | Swamp | Beach => 4.0,
        Desert | Savanna | Grassland => 14.0,
        Tundra => 25.0,
        Rainforest => 35.0,
        Forest | Shrubland | Taiga => 45.0,
        Mountain => 420.0,
        Snowcap => 650.0,
    };

    // The steepest neighbour, as metres per kilometre of regional slope.
    let mut drop = 0.0f64;
    for (dx, dy) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
        drop = drop.max((here - at(cx as i64 + dx, cy as i64 + dy)).abs());
    }
    let regional = drop / (1.0 - sea).max(1e-3) * scale_sim::world::MAX_LAND_M
        / scale_sim::region::KM_PER_CELL;

    (elevation_m, (landform + regional).clamp(2.0, 900.0))
}
