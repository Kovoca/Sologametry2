//! Command-line world generator.
//!
//! Generates a planet and writes it out as PNG maps plus a text report, so
//! the terrain can be looked at and tuned long before there is a game to
//! walk around in. The debug overlays (elevation / temperature / rainfall)
//! matter more than they look: when terrain comes out wrong the biome map
//! alone rarely tells you which field caused it.
//!
//! Each run picks a random planet unless `--seed` pins one. The seed used is
//! always printed, so any world can be reproduced.
//!
//!   cargo run --release
//!   cargo run --release -- --seed 12345
//!   cargo run --release -- --seed 7 --size 512x288 --out out

use std::fs;
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use scale_sim::geology::{Geology, Rock};
use scale_sim::network::{Network, Road};
use scale_sim::polity::{Polities, UNCLAIMED};
use scale_sim::settlement::{Kind, Settlements};
use scale_sim::world::{Biome, Params, World};

/// Concentration at or above which a deposit is worth extracting. Used for
/// both the resource map and the report.
const WORKABLE: f32 = 0.45;

struct Args {
    seed: u64,
    width: usize,
    height: usize,
    out: String,
    params: Params,
    /// Roughly how many polities to seed. The actual count and every border
    /// emerge from the terrain.
    nations: usize,
    /// Roughly how many settlements to place across the world.
    cities: usize,
}

/// A fresh seed from the clock, for when the user hasn't pinned one.
fn random_seed() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    // Scramble so consecutive runs don't produce near-identical seeds.
    let mut z = nanos ^ 0x9E3779B97F4A7C15;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

fn parse_args() -> Args {
    let mut args = Args {
        seed: random_seed(),
        width: 768,
        height: 432,
        out: "out".to_string(),
        params: Params::default(),
        nations: 28,
        cities: 9000,
    };

    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => {
                let v = it.next().unwrap_or_default();
                args.seed = v.parse().unwrap_or_else(|_| {
                    eprintln!("bad --seed value: {v:?}");
                    std::process::exit(2);
                });
            }
            "--size" => {
                let v = it.next().unwrap_or_default();
                match v.split_once('x') {
                    Some((w, h)) => match (w.parse(), h.parse()) {
                        (Ok(w), Ok(h)) => {
                            args.width = w;
                            args.height = h;
                        }
                        _ => {
                            eprintln!("bad --size value: {v:?} (expected WxH, e.g. 384x216)");
                            std::process::exit(2);
                        }
                    },
                    None => {
                        eprintln!("bad --size value: {v:?} (expected WxH, e.g. 384x216)");
                        std::process::exit(2);
                    }
                }
            }
            "--out" => {
                args.out = it.next().unwrap_or_else(|| {
                    eprintln!("--out needs a directory");
                    std::process::exit(2);
                });
            }
            "--land" => {
                let v = it.next().unwrap_or_default();
                match v.parse::<f32>() {
                    Ok(f) if (0.05..=0.90).contains(&f) => args.params.target_land = f,
                    _ => {
                        eprintln!("bad --land value: {v:?} (expected 0.05..0.90, e.g. 0.34)");
                        std::process::exit(2);
                    }
                }
            }
            "--nations" => {
                let v = it.next().unwrap_or_default();
                match v.parse::<usize>() {
                    Ok(k) if (1..=400).contains(&k) => args.nations = k,
                    _ => {
                        eprintln!("bad --nations value: {v:?} (expected 1..400)");
                        std::process::exit(2);
                    }
                }
            }
            "--cities" => {
                let v = it.next().unwrap_or_default();
                match v.parse::<usize>() {
                    Ok(k) if (1..=20_000).contains(&k) => args.cities = k,
                    _ => {
                        eprintln!("bad --cities value: {v:?} (expected 1..20000)");
                        std::process::exit(2);
                    }
                }
            }
            "--wind" => {
                let v = it.next().unwrap_or_default();
                match v.as_str() {
                    "e" | "east" | "1" => args.params.prevailing_wind = 1,
                    "w" | "west" | "-1" => args.params.prevailing_wind = -1,
                    _ => {
                        eprintln!("bad --wind value: {v:?} (expected e or w)");
                        std::process::exit(2);
                    }
                }
            }
            "--help" | "-h" => {
                println!(
                    "usage: worldgen [--seed N] [--size WxH] [--land F] [--wind e|w] [--out DIR]\n\
                     \n\
                     --seed N      pick a planet (default: random; the seed used is printed)\n\
                     --size WxH    map size in tiles (default 768x432)\n\
                     --land F      land fraction 0.05..0.90 (default 0.34, Earth ~0.29)\n\
                     --wind e|w    prevailing wind direction (default e)\n\
                     --nations N   how many polities to seed, 1..400 (default 28);\n\
                     \x20             the real count and all borders emerge from terrain\n\
                     --cities N    roughly how many settlements, 1..20000 (default 9000)\n\
                     --out DIR     output directory (default out)"
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument: {other:?}");
                std::process::exit(2);
            }
        }
    }

    if args.width < 16 || args.height < 16 {
        eprintln!(
            "size too small: {}x{} (minimum 16x16)",
            args.width, args.height
        );
        std::process::exit(2);
    }

    args
}

fn main() {
    let args = parse_args();

    let start = Instant::now();
    let world = World::generate_with(args.width, args.height, args.seed, args.params);
    let gen_ms = start.elapsed().as_secs_f64() * 1000.0;

    // Political geography runs after the world, reading it. `World` stays
    // pure terrain.
    let polities = Polities::partition(&world, args.nations);
    let settlements = Settlements::place(&world, &polities, args.cities);
    let network = Network::build(&world, &settlements, 1400);

    let dir = Path::new(&args.out);
    fs::create_dir_all(dir).expect("create output directory");

    // Upscale small maps so pixels are visible; leave large maps near 1:1.
    let scale = (1600 / world.width).clamp(1, 6) as u32;

    write_biome_png(&world, &dir.join("world_biomes.png"), scale);
    write_ramp_png(
        &world.elevation.data,
        world.width,
        world.height,
        &dir.join("world_elevation.png"),
        scale,
        ramp_grey,
    );
    write_ramp_png(
        &world.temperature.data,
        world.width,
        world.height,
        &dir.join("world_temperature.png"),
        scale,
        ramp_heat,
    );
    write_ramp_png(
        &world.rainfall.data,
        world.width,
        world.height,
        &dir.join("world_rainfall.png"),
        scale,
        ramp_wet,
    );
    write_flow_png(&world, &dir.join("world_rivers.png"), scale);
    write_rock_png(&world, &dir.join("world_rock.png"), scale);
    write_land_ramp_png(
        &world,
        &world.geology.fertility.data,
        &dir.join("world_fertility.png"),
        scale,
        ramp_fertility,
    );
    write_resource_png(&world, &dir.join("world_resources.png"), scale);
    write_polity_png(&world, &polities, &dir.join("world_nations.png"), scale);
    write_settlement_png(
        &world,
        &polities,
        &settlements,
        &dir.join("world_settlements.png"),
        scale,
    );
    write_network_png(
        &world,
        &settlements,
        &network,
        &dir.join("world_routes.png"),
        scale,
    );
    write_ascii(&world, &dir.join("world.txt"));

    print_report(&world, gen_ms, &args.out);
    print_polities(&world, &polities);
    print_settlements(&settlements);
    print_network(&world, &network);
}

// --- PNG output ----------------------------------------------------------

fn write_biome_png(world: &World, path: &Path, scale: u32) {
    let (w, h) = (world.width as u32, world.height as u32);
    let mut img = image::RgbImage::new(w * scale, h * scale);

    const RIVER: [u8; 3] = [40, 90, 175];
    const LAKE: [u8; 3] = [50, 105, 180];

    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            let colour = if world.river[i] {
                RIVER
            } else if world.lake[i] {
                LAKE
            } else {
                world.biomes[i].colour()
            };
            for dy in 0..scale {
                for dx in 0..scale {
                    img.put_pixel(x * scale + dx, y * scale + dy, image::Rgb(colour));
                }
            }
        }
    }
    img.save(path).expect("write biome png");
}

/// River network on land: brightness by log flow accumulation, so trunk
/// rivers read heavier than headwater streams. Ocean stays dark.
fn write_flow_png(world: &World, path: &Path, scale: u32) {
    let (w, h) = (world.width, world.height);
    let max_accum = world.flow_accum.data.iter().copied().fold(1.0f32, f32::max);
    let ln_max = (max_accum + 1.0).ln();

    let mut img = image::RgbImage::new(w as u32 * scale, h as u32 * scale);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let colour = if world.biomes[i] == Biome::Ocean || world.biomes[i] == Biome::Shallows {
                [8, 12, 28]
            } else if world.lake[i] {
                [70, 120, 200]
            } else {
                let v = (world.flow_accum.data[i] + 1.0).ln() / ln_max;
                let c = (30.0 + v * 225.0) as u8;
                [c / 3, (c as f32 * 0.7) as u8, c]
            };
            for dy in 0..scale {
                for dx in 0..scale {
                    img.put_pixel(
                        x as u32 * scale + dx,
                        y as u32 * scale + dy,
                        image::Rgb(colour),
                    );
                }
            }
        }
    }
    img.save(path).expect("write flow png");
}

/// Write a scalar field as a PNG, auto-stretched to its own min/max so a
/// field with a narrow value range (rainfall, especially) is still legible.
fn write_ramp_png(
    data: &[f32],
    w: usize,
    h: usize,
    path: &Path,
    scale: u32,
    ramp: fn(f32) -> [u8; 3],
) {
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for &v in data {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    let range = (hi - lo).max(1e-12);

    let mut img = image::RgbImage::new(w as u32 * scale, h as u32 * scale);
    for y in 0..h {
        for x in 0..w {
            let v = (data[y * w + x] - lo) / range;
            let colour = ramp(v);
            for dy in 0..scale {
                for dx in 0..scale {
                    img.put_pixel(
                        x as u32 * scale + dx,
                        y as u32 * scale + dy,
                        image::Rgb(colour),
                    );
                }
            }
        }
    }
    img.save(path).expect("write ramp png");
}

/// Rock type across the land; ocean left dark.
fn write_rock_png(world: &World, path: &Path, scale: u32) {
    let (w, h) = (world.width, world.height);
    let mut img = image::RgbImage::new(w as u32 * scale, h as u32 * scale);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let colour = if world.elevation.data[i] < world.sea_level {
                [18, 26, 48]
            } else {
                world.geology.rock[i].colour()
            };
            put_block(&mut img, x, y, scale, colour);
        }
    }
    img.save(path).expect("write rock png");
}

/// The three extractive resources on one map: red = metal ore, grey = coal,
/// green = petroleum. Brightness is concentration; only workable
/// concentrations are drawn, so the map reads as deposits, not a gradient.
fn write_resource_png(world: &World, path: &Path, scale: u32) {
    let (w, h) = (world.width, world.height);
    let g = &world.geology;
    let mut img = image::RgbImage::new(w as u32 * scale, h as u32 * scale);

    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let colour = if world.elevation.data[i] < world.sea_level {
                [14, 20, 38]
            } else {
                let (o, c, p) = (g.ore.data[i], g.coal.data[i], g.petroleum.data[i]);
                let best = o.max(c).max(p);
                if best < WORKABLE {
                    [42, 44, 46] // barren land
                } else {
                    let v = ((best - WORKABLE) / (1.0 - WORKABLE)).clamp(0.0, 1.0);
                    let b = (90.0 + v * 165.0) as u8;
                    if best == o {
                        [b, (b as f32 * 0.30) as u8, (b as f32 * 0.25) as u8]
                    } else if best == c {
                        [b, b, b]
                    } else {
                        [(b as f32 * 0.25) as u8, b, (b as f32 * 0.45) as u8]
                    }
                }
            };
            put_block(&mut img, x, y, scale, colour);
        }
    }
    img.save(path).expect("write resource png");
}

/// Political territories, one colour per polity, with capitals marked and
/// borders darkened so the shape of each state reads at a glance.
fn write_polity_png(world: &World, pol: &Polities, path: &Path, scale: u32) {
    let (w, h) = (world.width, world.height);
    let mut img = image::RgbImage::new(w as u32 * scale, h as u32 * scale);

    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let id = pol.owner[i];

            let colour = if id == UNCLAIMED {
                if world.elevation.data[i] < world.sea_level {
                    [16, 24, 46] // sea
                } else {
                    [58, 58, 62] // unclaimed land: ice, high mountain
                }
            } else {
                let base = Polities::colour(id);
                // Darken a cell that touches another polity, so borders show.
                let border = [
                    (x + w - 1) % w + y * w,
                    (x + 1) % w + y * w,
                    x + y.saturating_sub(1) * w,
                    x + (y + 1).min(h - 1) * w,
                ]
                .iter()
                .any(|&j| pol.owner[j] != id && pol.owner[j] != UNCLAIMED);

                if border {
                    [base[0] / 2, base[1] / 2, base[2] / 2]
                } else {
                    base
                }
            };
            put_block(&mut img, x, y, scale, colour);
        }
    }

    // Capitals, drawn last so nothing overpaints them.
    for (_, p) in pol.ranked() {
        let (cx, cy) = (p.core % w, p.core / w);
        put_block(&mut img, cx, cy, scale, [250, 250, 250]);
    }

    img.save(path).expect("write polity png");
}

/// Settlements over a muted territory map: capitals gold, cities orange,
/// towns white, sized by population so the hierarchy reads at a glance.
fn write_settlement_png(world: &World, pol: &Polities, set: &Settlements, path: &Path, scale: u32) {
    let (w, h) = (world.width, world.height);
    let mut img = image::RgbImage::new(w as u32 * scale, h as u32 * scale);

    // Muted base so the settlements stand out.
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let colour = if world.elevation.data[i] < world.sea_level {
                [14, 20, 40]
            } else if pol.owner[i] == UNCLAIMED {
                [40, 42, 44]
            } else {
                let c = Polities::colour(pol.owner[i]);
                [c[0] / 4 + 18, c[1] / 4 + 18, c[2] / 4 + 18]
            };
            put_block(&mut img, x, y, scale, colour);
        }
    }

    // At 9,000 settlements on a 768-wide map nearly a tenth of the land is
    // a dot, and the picture turns to noise. The full set is still in the
    // data — this draws only the places worth naming, plus every capital.
    const DRAWN: usize = 900;
    let ranked = set.ranked();
    let mut ordered: Vec<_> = ranked
        .iter()
        .enumerate()
        .filter(|(rank, s)| *rank < DRAWN || s.kind == Kind::Capital)
        .map(|(_, s)| *s)
        .collect();

    // Smallest first, so the great cities paint over their neighbours.
    ordered.reverse();
    for s in ordered {
        let radius = match s.population {
            p if p >= 8_000_000 => 3,
            p if p >= 3_000_000 => 2,
            p if p >= 1_000_000 => 1,
            _ => 0,
        };
        let colour = s.kind.colour();
        let (cx, cy) = ((s.cell % w) as i32, (s.cell / w) as i32);
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy > radius * radius + 1 {
                    continue;
                }
                let y = cy + dy;
                if y < 0 || y >= h as i32 {
                    continue;
                }
                let x = (cx + dx).rem_euclid(w as i32) as usize;
                put_block(&mut img, x, y as usize, scale, colour);
            }
        }
    }
    img.save(path).expect("write settlement png");
}

/// Trade infrastructure over a dark land: navigable water in blue, roads
/// graded by traffic, chokepoints picked out in red.
fn write_network_png(world: &World, set: &Settlements, net: &Network, path: &Path, scale: u32) {
    let (w, h) = (world.width, world.height);
    let mut img = image::RgbImage::new(w as u32 * scale, h as u32 * scale);

    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let colour = if world.elevation.data[i] < world.sea_level {
                [12, 18, 36]
            } else if net.navigable[i] {
                [70, 145, 220]
            } else if let Some(c) = net.road[i].colour() {
                c
            } else {
                // Faint relief so routes read against the land they cross.
                let e = ((world.elevation.data[i] - world.sea_level)
                    / (1.0 - world.sea_level).max(1e-3))
                .clamp(0.0, 1.0);
                let v = (34.0 + e * 40.0) as u8;
                [v, v - 4, v - 10]
            };
            put_block(&mut img, x, y, scale, colour);
        }
    }

    // Chokepoints, then the great cities they serve.
    for &i in &net.chokepoints {
        put_block(&mut img, i % w, i / w, scale, [255, 70, 60]);
    }
    for s in set.ranked().into_iter().take(120) {
        put_block(&mut img, s.cell % w, s.cell / w, scale, [255, 255, 255]);
    }

    img.save(path).expect("write network png");
}

fn print_network(world: &World, net: &Network) {
    let land = (world.land_fraction() * world.biomes.len() as f32).max(1.0);
    let highway = net.count(Road::Highway);
    let road = net.count(Road::Road);
    let track = net.count(Road::Track);
    let paved = highway + road + track;

    println!(
        "routes    {paved} cells of road ({:.1}% of land)   {} navigable water cells",
        paved as f32 / land * 100.0,
        net.navigable_count()
    );
    println!(
        "  {highway} highway, {road} road, {track} track   {} chokepoints on the trunk network",
        net.chokepoints.len()
    );
    println!();
}

fn fmt_pop(p: u32) -> String {
    if p >= 1_000_000 {
        format!("{:.1}M", p as f64 / 1.0e6)
    } else if p >= 1_000 {
        format!("{:.0}k", p as f64 / 1.0e3)
    } else {
        format!("{p}")
    }
}

fn print_settlements(set: &Settlements) {
    let ranked = set.ranked();
    if ranked.is_empty() {
        println!("no settlements placed");
        return;
    }
    println!(
        "settlements   {} placed   {} capitals, {} cities, {} towns",
        ranked.len(),
        set.count_of(Kind::Capital),
        set.count_of(Kind::City),
        set.count_of(Kind::Town),
    );
    println!(
        "  urban population {:.2}B   largest {}   median {}",
        set.total_population() as f64 / 1.0e9,
        fmt_pop(ranked[0].population),
        fmt_pop(ranked[ranked.len() / 2].population),
    );

    println!("  rank        pop  kind      hinterland  site");
    for (rank, s) in ranked.iter().enumerate().take(10) {
        let mut site = Vec::new();
        if s.coastal {
            site.push("port");
        }
        if s.on_water {
            site.push("river");
        }
        println!(
            "  {:>4}  {:>9}  {:<8}  {:>10}  {}",
            rank + 1,
            fmt_pop(s.population),
            s.kind.name(),
            s.catchment,
            if site.is_empty() {
                "inland".into()
            } else {
                site.join("+")
            },
        );
    }
    let ports = ranked.iter().filter(|s| s.coastal).count();
    println!(
        "  {ports} of {} on the coast ({:.0}%)",
        ranked.len(),
        ports as f32 / ranked.len() as f32 * 100.0
    );
    println!();
}

fn print_polities(world: &World, pol: &Polities) {
    let ranked = pol.ranked();
    if ranked.is_empty() {
        println!("no polities formed");
        return;
    }
    let claimed: usize = ranked.iter().map(|(_, p)| p.cells).sum();
    let land = (world.land_fraction() * world.biomes.len() as f32).max(1.0);

    println!(
        "nations   {} formed   {:.0}% of land claimed",
        ranked.len(),
        claimed as f32 / land * 100.0
    );
    println!(
        "  largest holds {:.0}% of claimed land; top 3 hold {:.0}%; top 5 hold {:.0}%",
        pol.concentration(1) * 100.0,
        pol.concentration(3) * 100.0,
        pol.concentration(5) * 100.0,
    );

    println!("  rank    cells   food   ore  coal   oil  coast");
    for (rank, (_, p)) in ranked.iter().enumerate().take(10) {
        println!(
            "  {:>4}  {:>7}  {:>5.0}  {:>4}  {:>4}  {:>4}   {}",
            rank + 1,
            p.cells,
            p.food,
            p.ore_cells,
            p.coal_cells,
            p.petroleum_cells,
            if p.coastal { "yes" } else { "no" },
        );
    }
    if ranked.len() > 10 {
        println!("  ... and {} more", ranked.len() - 10);
    }

    let landlocked = ranked.iter().filter(|(_, p)| !p.coastal).count();
    let no_deposits = ranked
        .iter()
        .filter(|(_, p)| p.workable_deposits() == 0)
        .count();
    println!("  {landlocked} landlocked; {no_deposits} with no workable deposits");
    println!();
}

/// A scalar field over land only, stretched to its own land min/max. Ocean
/// is drawn flat so it cannot dominate the ramp.
fn write_land_ramp_png(
    world: &World,
    data: &[f32],
    path: &Path,
    scale: u32,
    ramp: fn(f32) -> [u8; 3],
) {
    let (w, h) = (world.width, world.height);
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for i in 0..w * h {
        if world.elevation.data[i] >= world.sea_level {
            lo = lo.min(data[i]);
            hi = hi.max(data[i]);
        }
    }
    let range = (hi - lo).max(1e-12);

    let mut img = image::RgbImage::new(w as u32 * scale, h as u32 * scale);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let colour = if world.elevation.data[i] < world.sea_level {
                [18, 26, 48]
            } else {
                ramp(((data[i] - lo) / range).clamp(0.0, 1.0))
            };
            put_block(&mut img, x, y, scale, colour);
        }
    }
    img.save(path).expect("write land ramp png");
}

#[inline]
fn put_block(img: &mut image::RgbImage, x: usize, y: usize, scale: u32, colour: [u8; 3]) {
    for dy in 0..scale {
        for dx in 0..scale {
            img.put_pixel(
                x as u32 * scale + dx,
                y as u32 * scale + dy,
                image::Rgb(colour),
            );
        }
    }
}

/// Barren tan through to deep green.
fn ramp_fertility(v: f32) -> [u8; 3] {
    let v = v.clamp(0.0, 1.0);
    [
        (200.0 - v * 165.0) as u8,
        (170.0 - v * 40.0) as u8,
        (110.0 - v * 45.0) as u8,
    ]
}

fn ramp_grey(v: f32) -> [u8; 3] {
    let c = (v.clamp(0.0, 1.0) * 255.0) as u8;
    [c, c, c]
}

fn ramp_heat(v: f32) -> [u8; 3] {
    let v = v.clamp(0.0, 1.0);
    [(v * 255.0) as u8, 40, ((1.0 - v) * 255.0) as u8]
}

fn ramp_wet(v: f32) -> [u8; 3] {
    let v = v.clamp(0.0, 1.0);
    [20, (60.0 + v * 120.0) as u8, (v * 255.0) as u8]
}

// --- Text output --------------------------------------------------------

fn write_ascii(world: &World, path: &Path) {
    let mut s = String::with_capacity((world.width + 1) * world.height);
    for y in 0..world.height {
        for x in 0..world.width {
            let i = y * world.width + x;
            let ch = if world.river[i] {
                '+'
            } else if world.lake[i] {
                'o'
            } else {
                world.biomes[i].glyph()
            };
            s.push(ch);
        }
        s.push('\n');
    }
    fs::write(path, s).expect("write ascii map");
}

fn print_report(world: &World, gen_ms: f64, out: &str) {
    println!(
        "seed {}   {}x{}   generated in {:.0} ms",
        world.seed, world.width, world.height, gen_ms
    );
    println!(
        "sea level {:.3}   land {:.1}%",
        world.sea_level,
        world.land_fraction() * 100.0
    );
    let land_cells = (world.land_fraction() * world.biomes.len() as f32).max(1.0);
    println!(
        "rivers {:.1}% of land   lakes {:.1}% of land",
        world.river_count() as f32 / land_cells * 100.0,
        world.lake_count() as f32 / land_cells * 100.0,
    );
    println!();

    let counts = world.biome_counts();
    let total = world.biomes.len() as f32;

    let mut rows: Vec<(Biome, usize)> = Biome::ALL
        .iter()
        .map(|&b| (b, counts[b as usize]))
        .collect();
    rows.sort_by_key(|a| std::cmp::Reverse(a.1));

    println!("biome distribution");
    for (biome, n) in rows {
        if n == 0 {
            continue;
        }
        let pct = n as f32 / total * 100.0;
        let bar = "#".repeat((pct / 2.0).round() as usize);
        println!(
            "  {} {:<10} {:>5.1}%  {}",
            biome.glyph(),
            biome.name(),
            pct,
            bar
        );
    }
    print_geology(world);

    println!("wrote 11 PNGs + world.txt to {out}/");
    println!("re-generate this exact world with:  --seed {}", world.seed);
}

/// Fraction of land within `reach` cells of any workable deposit.
/// Chebyshev distance by repeated dilation; X wraps, Y clamps.
fn land_within_reach(world: &World, reach: usize) -> f32 {
    let (w, h) = (world.width, world.height);
    let g = &world.geology;

    let mut near: Vec<bool> = (0..w * h)
        .map(|i| {
            g.ore.data[i] >= WORKABLE
                || g.coal.data[i] >= WORKABLE
                || g.petroleum.data[i] >= WORKABLE
        })
        .collect();

    for _ in 0..reach {
        let src = near.clone();
        for y in 0..h {
            let ym = y.saturating_sub(1);
            let yp = (y + 1).min(h - 1);
            for x in 0..w {
                if src[y * w + x] {
                    continue;
                }
                let xm = (x + w - 1) % w;
                let xp = (x + 1) % w;
                near[y * w + x] =
                    src[y * w + xm] || src[y * w + xp] || src[ym * w + x] || src[yp * w + x];
            }
        }
    }

    let mut land = 0usize;
    let mut served = 0usize;
    for i in 0..w * h {
        if world.elevation.data[i] >= world.sea_level {
            land += 1;
            if near[i] {
                served += 1;
            }
        }
    }
    served as f32 / land.max(1) as f32
}

fn print_geology(world: &World) {
    let g = &world.geology;
    let land: Vec<usize> = (0..world.biomes.len())
        .filter(|&i| world.elevation.data[i] >= world.sea_level)
        .collect();
    if land.is_empty() {
        return;
    }
    let land_n = land.len() as f32;

    println!("rock");
    for r in Rock::ALL {
        let n = land.iter().filter(|&&i| g.rock[i] == r).count();
        let pct = n as f32 / land_n * 100.0;
        println!(
            "  {:<13} {:>5.1}%  {}",
            r.name(),
            pct,
            "#".repeat((pct / 3.0).round() as usize)
        );
    }
    println!();

    let mean_fert: f32 = land.iter().map(|&i| g.fertility.data[i]).sum::<f32>() / land_n;
    let prime = land.iter().filter(|&&i| g.fertility.data[i] > 0.55).count();
    println!(
        "soil     mean fertility {:.2}   prime farmland {:.1}% of land",
        mean_fert,
        prime as f32 / land_n * 100.0
    );

    println!("deposits            % land   provinces   largest");
    let mut provinces_total = 0usize;
    for (name, f) in [
        ("metal ore", &g.ore),
        ("coal", &g.coal),
        ("petroleum", &g.petroleum),
    ] {
        let n = Geology::deposit_count(f, WORKABLE);
        let pct = n as f32 / land_n * 100.0;
        let prov = Geology::deposit_provinces(f, WORKABLE);
        provinces_total += prov.len();
        println!(
            "  {:<16} {:>5.2}%   {:>9}   {:>7}",
            name,
            pct,
            prov.len(),
            prov.first().copied().unwrap_or(0),
        );
    }

    // What matters for settlement is not the share of the surface that is
    // ore, but whether a place has any within working distance.
    let reach = 5; // cells; ~82 km at 16.4 km per cell
    let served = land_within_reach(world, reach);
    println!(
        "  {} provinces total; {:.0}% of land within {} cells of a deposit",
        provinces_total,
        served * 100.0,
        reach
    );
    println!();

    // Per landmass: does each significant body of land carry its own
    // resource base, or is a whole continent left with nothing to mine?
    println!("landmasses (>=50 cells)      cells    ore   coal    oil");
    let mut shown = 0;
    let mut complete = 0;
    let mut significant = 0;
    for m in &g.landmasses {
        if m.cells.len() < 50 {
            continue;
        }
        significant += 1;
        let count = |f: &scale_sim::field::Field| {
            m.cells.iter().filter(|&&i| f.data[i] >= WORKABLE).count()
        };
        let (o, c, p) = (count(&g.ore), count(&g.coal), count(&g.petroleum));
        if o > 0 && c > 0 && p > 0 {
            complete += 1;
        }
        if shown < 8 {
            println!(
                "  {:<24} {:>6}  {:>5}  {:>5}  {:>5}",
                format!("#{}", shown + 1),
                m.cells.len(),
                o,
                c,
                p
            );
            shown += 1;
        }
    }
    println!(
        "  {complete} of {significant} carry all three; \
         {} landmasses in total",
        g.landmasses.len()
    );
    println!();
}
