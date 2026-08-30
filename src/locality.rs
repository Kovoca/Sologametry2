//! The middle rung: one region cell, seen close enough to walk into.
//!
//! Dwarf Fortress nests a world tile inside a grid of mid-level tiles, and
//! you pick a block of *those* to actually play on. The ladder here had no
//! such middle — it went from a 16.4 km region cell straight to a 25 m
//! house plot, a factor of six hundred and fifty-five with nothing in
//! between, so there was no scale at which you could look at a valley.
//!
//! This is spec A1.3d, which is explicit about how it must work:
//!
//! > *Cell detail is generated per region on demand, deterministically
//! > from the world seed: interpolate the coarse fields, add
//! > higher-frequency variation.*
//!
//! Both halves matter. **Interpolating** means the rain shadow the coarse
//! pass computed still falls where it fell — a 16 km grid is ample to put
//! a mountain range in the way of the weather, and that must not be
//! re-rolled here. **Adding variation** means the ground has a stream and
//! a copse in it rather than being one flat colour per cell.
//!
//! Nothing is stored. A locality is a pure function of the world and its
//! coordinates, so walking away and coming back gives the same valley.
//!
//! ## The ladder, in powers of two
//!
//! | | metres | nests |
//! |---|---|---|
//! | region cell | 16,384 | the world map |
//! | **locality** | **1,024** | 16 x 16 to a region cell |
//! | plot | 32 | 32 x 32 to a locality |
//! | tile | 1 | 32 x 32 to a plot |
//!
//! 16 x 32 x 32 = 16,384. The same number all the way down, which is why
//! the region cell was 16.384 km in the first place.

use crate::world::{Biome, World};

/// Localities across one region cell.
pub const LOCALITIES_PER_CELL: usize = 16;

/// Metres across one locality.
pub const METRES_PER_LOCALITY: f64 = 1_024.0;

/// One patch of ground at locality resolution.
#[derive(Copy, Clone, Debug)]
pub struct Patch {
    pub elevation: f32,
    pub temperature: f32,
    pub rainfall: f32,
    pub biome: Biome,
    /// Water runs through here.
    pub river: bool,
}

/// One region cell, subdivided.
pub struct Locality {
    /// The region cell this came out of.
    pub cell: usize,
    pub size: usize,
    pub patches: Vec<Patch>,
    pub sea_level: f32,
}

impl Locality {
    pub fn at(&self, x: usize, y: usize) -> Patch {
        self.patches[y * self.size + x]
    }

    /// Zoom into one cell of the world map.
    pub fn zoom(world: &World, cell: usize) -> Self {
        let size = LOCALITIES_PER_CELL;
        let (cx, cy) = (cell % world.width, cell / world.width);
        let mut patches = Vec::with_capacity(size * size);

        // First pass: the ground itself.
        //
        // **The detail is variation within the parent, not a re-roll of
        // it.** At an amplitude larger than the gap between biome bands it
        // stops refining the coarse pass and starts overruling it, and a
        // mountain cell with a river through it came out as half desert
        // and half water. A sixteen-kilometre cell is one kind of country;
        // zooming in shows the folds in it.
        const RELIEF: f32 = 0.010;
        const DAMP: f32 = 0.04;
        let mut raw = Vec::with_capacity(size * size);
        for ly in 0..size {
            for lx in 0..size {
                let fx = (lx as f32 + 0.5) / size as f32 - 0.5;
                let fy = (ly as f32 + 0.5) / size as f32 - 0.5;
                let base = sample(world, &world.elevation.data, cx, cy, fx, fy);
                let elevation = base + detail(world.seed, cell, lx, ly, 0) * RELIEF;
                let temperature = sample(world, &world.temperature.data, cx, cy, fx, fy)
                    // Cooler up the hill, the same lapse the coarse pass
                    // uses, so a valley floor is warmer than its ridge.
                    - (elevation - base) * 0.6;
                let rainfall = (sample(world, &world.rainfall.data, cx, cy, fx, fy)
                    + detail(world.seed, cell, lx, ly, 1) * DAMP)
                    .clamp(0.0, 1.0);
                raw.push((elevation, temperature, rainfall));
            }
        }

        // **A river is a channel, not a lowland.**
        //
        // Marking every patch below a threshold put water over a tenth of
        // the cell in a scatter. A river is a connected line that comes in
        // from one neighbour and leaves towards another, wandering as it
        // goes — which is what it looks like when you zoom in on one, and
        // the thing that made my first attempt read wrong at a glance.
        let mut channel = vec![false; size * size];
        if world.river[cell] {
            // Which way the water comes and goes. Neighbours that also
            // carry the river are where it joins the map beyond this cell;
            // failing that it runs to the lowest side.
            let neigh = |dx: i32, dy: i32| -> (usize, f32, bool) {
                let nx = (cx as i32 + dx).rem_euclid(world.width as i32) as usize;
                let ny = (cy as i32 + dy).clamp(0, world.height as i32 - 1) as usize;
                let i = ny * world.width + nx;
                (i, world.elevation.data[i], world.river[i])
            };
            let dirs = [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)];
            let mut joined: Vec<(usize, f32)> = Vec::new();
            for (k, &(dx, dy)) in dirs.iter().enumerate() {
                let (_, e, r) = neigh(dx, dy);
                if r {
                    joined.push((k, e));
                }
            }
            // Deterministic: downhill first, so the water runs the right
            // way and the same way every time.
            joined.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
            let mut ends: Vec<usize> = joined.iter().map(|&(k, _)| k).collect();
            if ends.is_empty() {
                // A spring: it starts here and leaves by the low side.
                let mut all: Vec<(usize, f32)> = dirs
                    .iter()
                    .enumerate()
                    .map(|(k, &(dx, dy))| (k, neigh(dx, dy).1))
                    .collect();
                all.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
                ends.push(all[0].0);
            }
            let mid = (size / 2) as i32;
            let edge = |k: usize| -> (i32, i32) {
                match k {
                    0 => (mid, 0),
                    1 => (size as i32 - 1, mid),
                    2 => (mid, size as i32 - 1),
                    _ => (0, mid),
                }
            };
            let (sx, sy) = edge(ends[0]);
            let (ex, ey) = if ends.len() > 1 {
                edge(ends[1])
            } else {
                (mid, mid)
            };

            // Walk from one to the other, wandering a little each step the
            // way water does when the ground is not perfectly tilted.
            let steps = ((ex - sx).abs().max((ey - sy).abs())).max(1);
            let (mut x, mut y) = (sx as f32, sy as f32);
            let mut last: Option<(i32, i32)> = None;
            for t in 0..=steps {
                let f = t as f32 / steps as f32;
                let tx = sx as f32 + (ex - sx) as f32 * f;
                let ty = sy as f32 + (ey - sy) as f32 * f;
                let wander = 1.8 * (1.0 - (2.0 * f - 1.0).abs());
                x = tx + detail(world.seed, cell, t as usize, 0, 7) * wander;
                y = ty + detail(world.seed, cell, 0, t as usize, 8) * wander;
                // **Join it to the last point**, or the wander leaves
                // gaps and the river arrives in pieces. Water is
                // continuous; that is most of what makes it a river.
                let (ix, iy) = (x.round() as i32, y.round() as i32);
                let (px, py) = last.unwrap_or((ix, iy));
                let n = (ix - px).abs().max((iy - py).abs()).max(1);
                for k in 0..=n {
                    let jx = px + (ix - px) * k / n;
                    let jy = py + (iy - py) * k / n;
                    if jx >= 0 && jy >= 0 && (jx as usize) < size && (jy as usize) < size {
                        channel[jy as usize * size + jx as usize] = true;
                    }
                }
                last = Some((ix, iy));
            }
        }

        for i in 0..size * size {
            let (elevation, temperature, rainfall) = raw[i];
            let river = channel[i];

            // **The biome is not re-derived here, it is blended.**
            //
            // The coarse pass classifies by *percentile rank over land*
            // (see CLAUDE.md — absolute thresholds are fragile and this
            // was learned the hard way), so raw values mean nothing
            // against fixed bands. Re-classifying from absolutes turned a
            // mountain cell into flat desert.
            //
            // What you actually see at a kilometre is the *transition*
            // between two sixteen-kilometre cells: forest thinning into
            // grassland over a couple of miles rather than stopping at a
            // line. So a patch takes its parent's country, or its
            // neighbour's where it lies near that edge, dithered so the
            // boundary is ragged the way a treeline is.
            let (lx, ly) = (i % size, i / size);
            let fx = (lx as f32 + 0.5) / size as f32 - 0.5;
            let fy = (ly as f32 + 0.5) / size as f32 - 0.5;
            let pull = detail(world.seed, cell, lx, ly, 2) * 0.5 + 0.5; // 0..1
            let neighbour = |dx: i32, dy: i32| -> Biome {
                let nx = (cx as i32 + dx).rem_euclid(world.width as i32) as usize;
                let ny = (cy as i32 + dy).clamp(0, world.height as i32 - 1) as usize;
                world.biomes[ny * world.width + nx]
            };
            let mut biome = world.biomes[cell];
            // Nearer the edge than `pull` allows means the neighbour's
            // country has reached this far in.
            if fx.abs() > fy.abs() {
                if fx.abs() * 2.0 > pull + 0.55 {
                    biome = neighbour(fx.signum() as i32, 0);
                }
            } else if fy.abs() * 2.0 > pull + 0.55 {
                biome = neighbour(0, fy.signum() as i32);
            }
            // Water is water wherever the ground actually is under the sea.
            if river {
                biome = world.biomes[cell];
            }

            patches.push(Patch {
                elevation,
                temperature,
                rainfall,
                biome,
                river,
            });
        }

        Locality {
            cell,
            size,
            patches,
            sea_level: world.sea_level,
        }
    }

    /// Drawn out, one character to the patch.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for y in 0..self.size {
            for x in 0..self.size {
                let p = self.at(x, y);
                out.push(if p.river { '+' } else { glyph(p.biome) });
            }
            out.push('\n');
        }
        out
    }
}

/// Read a coarse field smoothly, so the cell boundary is not a step.
///
/// **This is the half that must not be re-rolled.** The rain shadow, the
/// temperature gradient and the shape of the range all came out of the
/// global pass; zooming in refines them and never overrules them, or the
/// weather stops making sense the moment you look closely.
fn sample(world: &World, field: &[f32], cx: usize, cy: usize, fx: f32, fy: f32) -> f32 {
    let w = world.width as i32;
    let h = world.height as i32;
    let get = |x: i32, y: i32| -> f32 {
        let yy = y.clamp(0, h - 1) as usize;
        let xx = x.rem_euclid(w) as usize;
        field[yy * world.width + xx]
    };
    let (x0, y0) = (cx as i32, cy as i32);
    let (sx, sy) = (fx.signum() as i32, fy.signum() as i32);
    let (ax, ay) = (fx.abs() * 2.0, fy.abs() * 2.0);
    let a = get(x0, y0);
    let b = get(x0 + sx, y0);
    let c = get(x0, y0 + sy);
    let d = get(x0 + sx, y0 + sy);
    let top = a + (b - a) * ax * 0.5;
    let bot = c + (d - c) * ax * 0.5;
    top + (bot - top) * ay * 0.5
}

/// Higher-frequency variation, deterministic from where it is.
///
/// The other half of A1.3d: without it every patch of a cell is identical
/// and the ground reads as a spreadsheet. Returns roughly -1..1.
fn detail(seed: u64, cell: usize, lx: usize, ly: usize, layer: u64) -> f32 {
    let mut h = seed
        ^ (cell as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (lx as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ (ly as u64).wrapping_mul(0x94D0_49BB_1331_11EB)
        ^ layer.wrapping_mul(0x2545_F491_4F6C_DD1D);
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    ((h >> 11) as f32 / (1u64 << 53) as f32) * 2.0 - 1.0
}

/// Same rules the coarse pass uses, so a zoomed valley is made of the same
/// stuff as the map it came out of.
fn classify(world: &World, elevation: f32, temperature: f32, rainfall: f32) -> Biome {
    if elevation < world.sea_level {
        return Biome::Ocean;
    }
    let above = (elevation - world.sea_level) / (1.0 - world.sea_level).max(1e-3);
    if above > 0.72 {
        return if temperature < 0.25 {
            Biome::Snowcap
        } else {
            Biome::Mountain
        };
    }
    if temperature < 0.18 {
        return Biome::Tundra;
    }
    if temperature < 0.35 {
        return if rainfall > 0.45 {
            Biome::Taiga
        } else {
            Biome::Tundra
        };
    }
    if rainfall < 0.18 {
        return Biome::Desert;
    }
    if rainfall < 0.30 {
        return if temperature > 0.65 {
            Biome::Savanna
        } else {
            Biome::Grassland
        };
    }
    if rainfall < 0.42 {
        return Biome::Shrubland;
    }
    if temperature > 0.72 && rainfall > 0.62 {
        return Biome::Rainforest;
    }
    if rainfall > 0.78 {
        return Biome::Swamp;
    }
    Biome::Forest
}

fn glyph(b: Biome) -> char {
    match b {
        Biome::Ocean => '~',
        Biome::Shallows => ':',
        Biome::Beach => ',',
        Biome::Desert => '.',
        Biome::Savanna => ';',
        Biome::Grassland => '"',
        Biome::Shrubland => '*',
        Biome::Forest => 'f',
        Biome::Rainforest => 'F',
        Biome::Swamp => 's',
        Biome::Taiga => 't',
        Biome::Tundra => '-',
        Biome::Mountain => '^',
        Biome::Snowcap => 'A',
    }
}
