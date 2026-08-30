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
use scale_sim::townplan::{Lot, Plan, StreetClass, TILES_PER_PLOT};
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
                println!("usage: walk [--seed N] [--rank K] [--which 0-4] [--where street|corner|lane|road|dual|motorway|shop|flats|house|works|edge]");
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
        "flats" => Lot::Flats,
        "house" => Lot::House,
        "works" => Lot::Works,
        "edge" => Lot::Open,
        _ => Lot::Street,
    };
    // **A corner is a junction**, and you stand on the pavement at it —
    // which is a different question from standing in the middle of a
    // street, and the one somebody dropped into a city actually faces.
    let want_corner = place == "corner";
    // If they asked for a size of road, only streets of that size will do.
    let want_class = match place.as_str() {
        "lane" => Some(StreetClass::Lane),
        "road" => Some(StreetClass::Road),
        "dual" => Some(StreetClass::Dual),
        "motorway" => Some(StreetClass::Motorway),
        _ => None,
    };
    let mut found = (plan.width / 2, plan.height / 2);
    // Houses are worth seeing at the edge of town, where they stop being
    // terraces; everything else is worth seeing near the middle.
    let outward = want != Lot::House;
    'outer: for step in 0..plan.width {
        let r = if outward { step } else { plan.width - 1 - step };
        for y in 0..plan.height {
            for x in 0..plan.width {
                let d = (x as i64 - plan.width as i64 / 2)
                    .abs()
                    .max((y as i64 - plan.height as i64 / 2).abs());
                let junction = x > 0
                    && y > 0
                    && x + 1 < plan.width
                    && y + 1 < plan.height
                    && (plan.at(x, y - 1) == Lot::Street || plan.at(x, y + 1) == Lot::Street)
                    && (plan.at(x - 1, y) == Lot::Street || plan.at(x + 1, y) == Lot::Street);
                if d as usize == r
                    && plan.at(x, y) == want
                    && junction == want_corner
                    // A corner you can stand on. A motorway has no
                    // footway, so its junctions are not corners.
                    && (!want_corner
                        || plan.street_class(x, y).is_some_and(|c| {
                            c != scale_sim::townplan::StreetClass::Motorway
                        }))
                    && want_class.is_none_or(|c| plan.street_class(x, y) == Some(c))
                {
                    found = (x, y);
                    break 'outer;
                }
            }
        }
    }

    let t = TILES_PER_PLOT as i64;
    let mut centre = (found.0 as i64 * t + t / 2, found.1 as i64 * t + t / 2);
    if want_corner {
        // Off the carriageway and onto the corner of the footway: the
        // nearest pavement tile to the diagonal of the junction.
        let probe = Ground::window(seed, &plan, centre, t as usize, t as usize);
        let mut best: Option<((i64, i64), i64)> = None;
        for vy in 0..probe.h {
            for vx in 0..probe.w {
                if probe.at(vx, vy) != Tile::Pavement {
                    continue;
                }
                let (gx, gy) = (probe.origin.0 + vx as i64, probe.origin.1 + vy as i64);
                // Furthest from both centrelines is the corner itself.
                let score = -((gx - centre.0).abs().min((gy - centre.1).abs()));
                if best.is_none_or(|(_, b)| score > b) {
                    best = Some(((gx, gy), score));
                }
            }
        }
        if let Some((p, _)) = best {
            centre = p;
        }
    }

    // A viewport, not the bubble: 80 x 34 looks square in a terminal.
    let mut g = Ground::window(seed, &plan, centre, 80, 34);

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
        "  {} m by {} m of the {} m you could see on foot, generated from the",
        g.w, g.h, BUBBLE_ON_FOOT
    );
    println!("  seed and never stored (spec A1.5)");
    println!("  the town stands in {ground_biome:?}");
    println!();
    if let Some(c) = plan.street_class(found.0, found.1) {
        println!(
            "  a {}: {} lane{} each way of {:.2} m, {:.1} m of usable surface",
            c.name(),
            c.lanes_each_way(),
            if c.lanes_each_way() == 1 { "" } else { "s" },
            c.lane_width_m(),
            c.usable_m(),
        );
        println!();
        println!("  what it admits:");
        for (what, w) in [
            ("a car", 1.80),
            ("a van", Vehicle::van().width_m),
            ("an artic", Vehicle::artic().width_m),
            ("a battle tank", 3.90),
            ("a grid transformer", 4.50),
        ] {
            println!(
                "    {:<20} {:.2} m   {}",
                what,
                w,
                c.clearance_for(w).name()
            );
        }
        println!();
    }
    print!("{}", g.render(Some(centre)));
    println!();
    println!("  @ you   = road   : lane marking   ; hard shoulder   - pavement");
    println!("  # wall   / door   o window   . floor");
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
