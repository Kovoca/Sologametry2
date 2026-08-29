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
        eprintln!("size too small: {}x{} (minimum 16x16)", args.width, args.height);
        std::process::exit(2);
    }

    args
}

fn main() {
    let args = parse_args();

    let start = Instant::now();
    let world = World::generate_with(args.width, args.height, args.seed, args.params);
    let gen_ms = start.elapsed().as_secs_f64() * 1000.0;

    let dir = Path::new(&args.out);
    fs::create_dir_all(dir).expect("create output directory");

    // Upscale small maps so pixels are visible; leave large maps near 1:1.
    let scale = (1600 / world.width).clamp(1, 6) as u32;

    write_biome_png(&world, &dir.join("world_biomes.png"), scale);
    write_ramp_png(&world.elevation.data, world.width, world.height,
        &dir.join("world_elevation.png"), scale, ramp_grey);
    write_ramp_png(&world.temperature.data, world.width, world.height,
        &dir.join("world_temperature.png"), scale, ramp_heat);
    write_ramp_png(&world.rainfall.data, world.width, world.height,
        &dir.join("world_rainfall.png"), scale, ramp_wet);
    write_flow_png(&world, &dir.join("world_rivers.png"), scale);
    write_rock_png(&world, &dir.join("world_rock.png"), scale);
    write_land_ramp_png(&world, &world.geology.fertility.data,
        &dir.join("world_fertility.png"), scale, ramp_fertility);
    write_resource_png(&world, &dir.join("world_resources.png"), scale);
    write_ascii(&world, &dir.join("world.txt"));

    print_report(&world, gen_ms, &args.out);
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
            let colour = if world.biomes[i] == Biome::Ocean
                || world.biomes[i] == Biome::Shallows
            {
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
                    img.put_pixel(x as u32 * scale + dx, y as u32 * scale + dy, image::Rgb(colour));
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
            img.put_pixel(x as u32 * scale + dx, y as u32 * scale + dy, image::Rgb(colour));
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

    let mut rows: Vec<(Biome, usize)> =
        Biome::ALL.iter().map(|&b| (b, counts[b as usize])).collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1));

    println!("biome distribution");
    for (biome, n) in rows {
        if n == 0 {
            continue;
        }
        let pct = n as f32 / total * 100.0;
        let bar = "#".repeat((pct / 2.0).round() as usize);
        println!("  {} {:<10} {:>5.1}%  {}", biome.glyph(), biome.name(), pct, bar);
    }
    print_geology(world);

    println!("wrote 8 PNGs + world.txt to {out}/");
    println!("re-generate this exact world with:  --seed {}", world.seed);
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

    let mean_fert: f32 =
        land.iter().map(|&i| g.fertility.data[i]).sum::<f32>() / land_n;
    let prime = land.iter().filter(|&&i| g.fertility.data[i] > 0.55).count();
    println!(
        "soil     mean fertility {:.2}   prime farmland {:.1}% of land",
        mean_fert,
        prime as f32 / land_n * 100.0
    );

    println!("deposits (% of land at workable concentration)");
    for (name, f) in [
        ("metal ore", &g.ore),
        ("coal", &g.coal),
        ("petroleum", &g.petroleum),
    ] {
        let n = Geology::deposit_count(f, WORKABLE);
        let pct = n as f32 / land_n * 100.0;
        println!(
            "  {:<11} {:>5.2}%  {}",
            name,
            pct,
            "#".repeat((pct * 2.0).round().min(40.0) as usize)
        );
    }
    println!();
}
