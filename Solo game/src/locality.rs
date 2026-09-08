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
            let mut last: Option<(i32, i32)> = None;
            for t in 0..=steps {
                let f = t as f32 / steps as f32;
                let tx = sx as f32 + (ex - sx) as f32 * f;
                let ty = sy as f32 + (ey - sy) as f32 * f;
                // Nil at both ends, so the channel meets its neighbours
                // where they said it would and wanders in between.
                let wander = 1.8 * (1.0 - (2.0 * f - 1.0).abs());
                let x = tx + detail(world.seed, cell, t as usize, 0, 7) * wander;
                let y = ty + detail(world.seed, cell, 0, t as usize, 8) * wander;
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
        self.draw(false)
    }

    /// The same, in colour: the country by what it is made of, and a river
    /// blue, so a watercourse can be picked out of a wooded valley.
    pub fn render_in_colour(&self) -> String {
        self.draw(true)
    }

    fn draw(&self, colour: bool) -> String {
        use crate::ground::Colour;
        let mut out = String::new();
        let mut last: Option<Colour> = None;
        for y in 0..self.size {
            for x in 0..self.size {
                let p = self.at(x, y);
                let (g, c) = if p.river {
                    ('+', Colour::LightBlue)
                } else {
                    (glyph(p.biome), crate::townplan::ground_colour(p.biome))
                };
                if colour && last != Some(c) {
                    out.push_str(c.ansi());
                    last = Some(c);
                }
                out.push(g);
            }
            out.push('\n');
        }
        if colour {
            out.push_str("\x1b[0m");
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

// **There is deliberately no `classify` here.** Re-deriving a biome from
// absolute thresholds is the bug this module already unlearned: the
// coarse pass classifies by percentile rank over land, so fixed cuts
// turned a mountain cell into flat desert. A zoomed patch takes its
// parent's country or a neighbour's near the edge. Restoring a
// classifier would restore the fault with it.

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

// ---------------------------------------------------------------------
// the villages, which are not in any list
// ---------------------------------------------------------------------

/// **A place too small to be worth storing.**
///
/// There are millions of them — geographic databases hold something like
/// four million populated places on Earth against fewer than ten thousand
/// urban areas over seventy thousand people — so keeping a list of them
/// is precisely the unbounded state this project has had to remove three
/// times. They are a pure function of where they are, like the ground
/// they stand on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Village {
    /// Which locality of the cell, 0..LOCALITIES_PER_CELL².
    pub locality: usize,
    pub population: u32,
}

/// **One settlement per thirteen square kilometres** is about what
/// long-settled farming country carries: England has upwards of ten
/// thousand villages in a hundred and thirty thousand square kilometres.
/// A region cell is 16.384 km on a side, so 268 km², so about twenty
/// places — and most of them are hamlets.
pub const KM2_PER_SETTLED_PLACE: f64 = 13.0;

fn hash3(a: u64, b: u64, c: u64) -> u64 {
    let mut h = a.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ b.wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ c.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h.wrapping_mul(0x94D0_49BB_1331_11EB) ^ (h >> 31)
}

fn unit(h: u64) -> f64 {
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// **The villages of one region cell**, generated and never stored.
///
/// `people` is what the countryside of this cell holds — the caller
/// works that out from the land, because carrying capacity is a
/// question about soil and water and this is a question about where
/// somebody put a house.
///
/// The sizes come out in the real proportions: mostly hamlets, some
/// villages, occasionally something with a church and a shop. Nothing is
/// placed by hand and nothing is remembered.
pub fn villages_in(seed: u64, cell: usize, people: f64) -> Vec<Village> {
    if people < 20.0 {
        return Vec::new();
    }
    let cell_km2 = 16.384f64 * 16.384;
    let want = (cell_km2 / KM2_PER_SETTLED_PLACE).round().max(1.0) as usize;
    let slots = LOCALITIES_PER_CELL * LOCALITIES_PER_CELL;

    // Which localities are settled at all. Deterministic, so walking away
    // and back finds the same hamlets in the same fields.
    let mut chosen: Vec<(usize, f64)> = Vec::new();
    for i in 0..slots {
        let h = hash3(seed, cell as u64, i as u64);
        chosen.push((i, unit(h)));
    }
    chosen.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    chosen.truncate(want.min(slots));

    // **Small places are the rule and the big one is the exception.** A
    // steep share so that one place in the cell is the village with the
    // church and the rest are farmsteads and hamlets, which is what
    // country like this actually looks like.
    let mut share: Vec<f64> = chosen
        .iter()
        .enumerate()
        .map(|(rank, _)| 1.0 / ((rank + 1) as f64).powf(1.15))
        .collect();
    let total: f64 = share.iter().sum::<f64>().max(1e-9);
    for v in share.iter_mut() {
        *v /= total;
    }

    let mut out: Vec<Village> = chosen
        .iter()
        .zip(share)
        .map(|((locality, _), s)| Village {
            locality: *locality,
            population: (people * s).round().max(1.0) as u32,
        })
        .filter(|v| v.population >= 5)
        .collect();
    out.sort_by_key(|v| v.locality);
    out
}
