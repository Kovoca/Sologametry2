//! Command-line world generator.
//!
//! Generates a planet and writes it out as PNG maps plus a text report, so
//! the terrain can be looked at and tuned long before there is a game to
//! walk around in. The debug overlays (elevation / temperature / rainfall)
//! matter more than they look: when terrain comes out wrong the biome map
//! alone rarely tells you which field caused it.
//!
//!   cargo run --release
//!   cargo run --release -- --seed 12345
//!   cargo run --release -- --seed 7 --size 512x288 --out out

use std::fs;
use std::path::Path;
use std::time::Instant;

use scale_sim::world::{Biome, Params, World};

struct Args {
    seed: u64,
    width: usize,
    height: usize,
    out: String,
    params: Params,
}

fn parse_args() -> Args {
    let mut args = Args {
        seed: 20260828,
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
                     --seed N      pick a planet (same seed = same world)\n\
                     --size WxH    map size in tiles (default 512x288)\n\
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
    write_ascii(&world, &dir.join("world.txt"));

    print_report(&world, gen_ms, &args.out);
}

// --- PNG output ----------------------------------------------------------

fn write_biome_png(world: &World, path: &Path, scale: u32) {
    let (w, h) = (world.width as u32, world.height as u32);
    let mut img = image::RgbImage::new(w * scale, h * scale);

    for y in 0..h {
        for x in 0..w {
            let colour = world.biomes[(y * w + x) as usize].colour();
            for dy in 0..scale {
                for dx in 0..scale {
                    img.put_pixel(x * scale + dx, y * scale + dy, image::Rgb(colour));
                }
            }
        }
    }
    img.save(path).expect("write biome png");
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
            s.push(world.biomes[y * world.width + x].glyph());
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
    println!();
    println!("wrote 4 PNGs + world.txt to {out}/");
}
