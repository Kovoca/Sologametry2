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
    let mut z = 0i64;
    let mut plain = false;
    // **The inspection view**: everything, whether or not anybody
    // could see it. Named so it cannot be mistaken for a player's view.
    let mut omniscient = false;

    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => seed = it.next().and_then(|v| v.parse().ok()).unwrap_or(seed),
            "--rank" => rank = it.next().and_then(|v| v.parse().ok()).unwrap_or(3).max(1),
            "--which" => which = it.next().and_then(|v| v.parse().ok()).unwrap_or(4),
            "--where" => place = it.next().unwrap_or_else(|| "street".into()),
            "--z" => z = it.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "--plain" => plain = true,
            "--omniscient" => omniscient = true,
            "--help" | "-h" => {
                println!("usage: walk [--seed N] [--rank K] [--which 0-4] [--z N] [--plain] [--omniscient] [--where street|corner|lane|road|dual|motorway|shop|flats|house|works|edge]");
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

    // **How high, and how much it moves.** Absolute height from the
    // elevation field against a real ceiling, and relief from how fast the
    // field changes across the neighbouring cells — which is what decides
    // whether the ground under a town is flat or a hillside.
    let (elevation_m, relief_m) = ground_of(&world, cell);
    let loc = Locality::zoom(&world, cell);
    let ground_biome = loc.at(loc.size / 2, loc.size / 2).biome;
    let plan = Plan::lay_out_on(seed, cell, pop, 40, ground_biome).on_rock(world.geology.rock[cell])
        .on_ground(elevation_m, relief_m)
        .with_water_at(world.depth_to_water_m(cell));

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

    // **Show the superstore if the town has one.** A run of shop plots is
    // one building — a high street terrace, or a supermarket across three
    // — and it is the more interesting thing to stand in than whichever
    // lone corner shop happens to sit nearest the middle.
    if want == Lot::Shop {
        let mut best: Option<((usize, usize), i64)> = None;
        for y in 1..plan.height - 1 {
            for x in 1..plan.width - 1 {
                if plan.at(x, y) != Lot::Shop
                    || plan.at(x - 1, y) != Lot::Shop
                    || plan.at(x + 1, y) != Lot::Shop
                {
                    continue;
                }
                let d = (x as i64 - plan.width as i64 / 2).abs()
                    + (y as i64 - plan.height as i64 / 2).abs();
                if best.is_none_or(|(_, bd)| d < bd) {
                    best = Some(((x, y), d));
                }
            }
        }
        if let Some((p, _)) = best {
            found = p;
        }
    }

    let t = TILES_PER_PLOT as i64;
    let mut centre = (found.0 as i64 * t + t / 2, found.1 as i64 * t + t / 2);
    if want_corner {
        // Off the carriageway and onto the corner of the footway: the
        // nearest pavement tile to the diagonal of the junction.
        let probe = Ground::window(seed, &plan, centre, t as usize + 2, t as usize + 2);
        let mut best: Option<((i64, i64), i64)> = None;
        for vy in 0..probe.h {
            for vx in 0..probe.w {
                if probe.at(vx, vy) != Tile::Pavement {
                    continue;
                }
                let (gx, gy) = (probe.origin.0 + vx as i64, probe.origin.1 + vy as i64);
                // **Furthest from both centrelines is the corner.** The
                // sign was the wrong way round, which put the player back
                // in the middle of the junction — on the crown of the
                // road, which is the one place a pedestrian is not.
                let score = (gx - centre.0).abs().min((gy - centre.1).abs());
                if best.is_none_or(|(_, b)| score > b) {
                    best = Some(((gx, gy), score));
                }
            }
        }
        if let Some((p, _)) = best {
            centre = p;
        }
    }

    // **`--z` is relative to the ground you are standing on**, because
    // that is what a level means to somebody in the world: 0 is here, +1
    // is the floor above, -1 is the cellar. Absolute levels are the
    // engine's business — at 60 m above the sea, absolute 0 is twenty
    // levels underground.
    let here_z = scale_sim::ground::surface_z(seed, &plan, centre.0, centre.1);

    // A viewport, not the bubble: 80 x 34 looks square in a terminal.
    // A corner is worth looking at further, because what is interesting
    // about one is the buildings on it, and those are a plot away.
    let mut g = if want_corner {
        // **A junction needs two plots of height to be visible at all.**
        //
        // The plot grid is 32 m, so the crossing street's plot centre is a
        // full 32 m away — and a 44-row window centred on the corner
        // reaches 22 m, which clips the cross street off the picture
        // entirely. Somebody asking to stand on a corner got a straight
        // road with pavements and no corner in sight.
        Ground::window_on(seed, &plan, centre, 92, 72, z)
    } else if want == Lot::Shop {
        // **Wide enough to hold a superstore.** One runs across three
        // plots — 96 m, because 2,800-4,650 m² does not fit on one — and
        // an 80-column window cut it off at both ends.
        Ground::window_on(seed, &plan, centre, 104, 40, z)
    } else {
        Ground::window_on(seed, &plan, centre, 80, 34, z)
    };

    // **Stand on something you can stand on.**
    //
    // The spot picked out of the plan is the middle of a plot, which in a
    // supermarket is as likely to be a shelf as an aisle — and the player
    // came out standing inside the shelving. Nothing walks through
    // furniture, so step to the nearest tile that is actually floor.
    if !g
        .at((centre.0 - g.origin.0) as usize, (centre.1 - g.origin.1) as usize)
        .walkable()
    {
        let mut best: Option<((i64, i64), i64)> = None;
        for vy in 0..g.h {
            for vx in 0..g.w {
                if !g.at(vx, vy).walkable() {
                    continue;
                }
                let (gx, gy) = (g.origin.0 + vx as i64, g.origin.1 + vy as i64);
                let d = (gx - centre.0).abs() + (gy - centre.1).abs();
                if best.is_none_or(|(_, bd)| d < bd) {
                    best = Some(((gx, gy), d));
                }
            }
        }
        if let Some((p, _)) = best {
            centre = p;
        }
    }

    // **Park the lorry where a lorry goes.**
    //
    // It used to be dropped on the nearest road tile with only that one
    // tile checked, and an artic is seventeen metres — so standing in a
    // supermarket you got a lorry through the wall and halfway up an
    // aisle. A lorry stands on made ground with the whole of it out of
    // doors, and if there is a loading dock in sight it backs up to that,
    // because that is the entire point of a dock.
    if z == 0 {
        let artic = Vehicle::artic();
        if let Some(spot) = somewhere_to_park(&g, &artic) {
            g.park(&artic, spot);
        }
    }

    println!();
    println!(
        "{name} ({}) — standing on {:?} at tile {},{}, level {} — {:.0} m above the sea",
        // **Say how big the place is.** Somebody asking to stand in a
        // city centre and finding open country a plot away is owed the
        // information that they are in a market town of forty thousand
        // people in the mountains, not a city — the density field was
        // reaching every plot it should, and the town simply was not one.
        if pop >= 500_000.0 {
            format!("a city of {:.1}M", pop / 1.0e6)
        } else if pop >= 20_000.0 {
            format!("a town of {:.0}k", pop / 1.0e3)
        } else if pop >= 2_500.0 {
            format!("a small town of {pop:.0}")
        } else {
            format!("a village of {pop:.0}")
        },
        want,
        centre.0,
        centre.1,
        z,
        scale_sim::ground::elevation_at_level(seed, &plan, centre.0, centre.1, z),
    );
    println!(
        "  {} m by {} m of the {} m you could see on foot, generated from the",
        g.w, g.h, BUBBLE_ON_FOOT
    );
    println!("  seed and never stored (spec A1.5)");
    println!(
        "  the town stands in {ground_biome:?}, on {} — {:.0} m up, relief {:.0} m/km",
        plan.rock.name(),
        elevation_m,
        relief_m,
    );
    println!(
        "  water is {:.0} m down{}",
        plan.water_m,
        if plan.water_m < 3.0 {
            " — too near the surface for cellars"
        } else if plan.water_m > 30.0 {
            " — too deep for a hand-dug well"
        } else {
            ""
        }
    );
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
    print!(
        "{}",
        if plain {
            if omniscient { g.render_omniscient(Some(centre), false) } else { g.render(Some(centre)) }
        } else {
            if omniscient { g.render_omniscient(Some(centre), true) } else { g.render_in_colour(Some(centre)) }
        }
    );
    println!();
    println!("{}", scale_sim::ground::ground_legend_in(true, !plain));
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

/// Where a lorry would actually stand: the whole of it on made ground,
/// as close to a loading dock as there is one.
fn somewhere_to_park(g: &Ground, v: &Vehicle) -> Option<(i64, i64)> {
    let docks: Vec<(i64, i64)> = (0..g.w * g.h)
        .filter(|i| {
            g.tiles[*i] == Tile::Fitting(scale_sim::building::Fixture::LoadingBay)
        })
        .map(|i| ((i % g.w) as i64, (i / g.w) as i64))
        .collect();
    let (cx, cy) = (g.w as i64 / 2, g.h as i64 / 2);
    let mut best: Option<((i64, i64), i64)> = None;
    for y in 0..g.h as i64 {
        for x in 0..g.w as i64 {
            let at = (g.origin.0 + x, g.origin.1 + y);
            if !g.room_to_park(v, at) {
                continue;
            }
            // Nearest dock if there is one, otherwise nearest to the
            // viewer — a lorry on the far side of the window shows nothing.
            let d = docks
                .iter()
                .map(|(dx, dy)| (x - dx).abs() + (y - dy).abs())
                .min()
                .unwrap_or_else(|| (x - cx).abs() + (y - cy).abs());
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((at, d));
            }
        }
    }
    best.map(|(p, _)| p)
}

#[allow(dead_code)]
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
