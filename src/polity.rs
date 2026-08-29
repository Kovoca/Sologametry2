//! Natural political fragmentation.
//!
//! Partitions the land into the territories a planet's geography *wants*.
//! This is not the history simulation and it places nothing by hand â€” it
//! answers the prior question the history sim needs answered: given this
//! terrain, where do people settle, and where do the borders fall?
//!
//! Three steps:
//!   1. Score every land cell for habitability (food, water, minerals).
//!   2. Seed cores at the best sites, spaced apart.
//!   3. Grow each core outward at a cost set by the terrain, until the
//!      territories meet. Borders land where expansion got expensive â€”
//!      mountain crests, deserts, straits.
//!
//! Runs *after* `World::generate` on purpose. `World` stays pure geography;
//! this reads it. Consolidation into great powers is the history sim's job,
//! not this pass's.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::rng::Rng;
use crate::world::{Biome, World};

/// No owner.
pub const UNCLAIMED: u16 = u16::MAX;

/// Concentration at which a deposit is worth working. Matches the reporting
/// threshold in the CLI.
const WORKABLE: f32 = 0.45;

#[derive(Clone, Debug)]
pub struct Polity {
    /// Cell index of the founding site â€” the capital.
    pub core: usize,
    /// State capacity — how effectively this polity projects power, above
    /// or below the average. The part of state size that is history rather
    /// than geography.
    pub reach: f32,
    /// Territory size in cells.
    pub cells: usize,
    /// Total soil fertility held: the crude carrying-capacity proxy, and
    /// the best single predictor of which of these becomes a great power.
    pub food: f32,
    pub ore_cells: usize,
    pub coal_cells: usize,
    pub petroleum_cells: usize,
    /// Holds at least one coastal cell â€” can trade and project by sea.
    pub coastal: bool,
}

impl Polity {
    pub fn workable_deposits(&self) -> usize {
        self.ore_cells + self.coal_cells + self.petroleum_cells
    }
}

pub struct Polities {
    pub width: usize,
    pub height: usize,
    /// Owning polity per cell, or `UNCLAIMED`.
    pub owner: Vec<u16>,
    pub list: Vec<Polity>,
}

/// Total order over `f32` for the frontier queue. `total_cmp` so NaN cannot
/// wedge it and the result is identical on every platform.
#[derive(Clone, Copy, PartialEq)]
struct Cost(f32);
impl Eq for Cost {}
impl PartialOrd for Cost {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.0.total_cmp(&other.0))
    }
}
impl Ord for Cost {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

#[inline]
fn neighbours(i: usize, w: usize, h: usize, mut f: impl FnMut(usize)) {
    let (x, y) = (i % w, i / w);
    f(y * w + (x + w - 1) % w);
    f(y * w + (x + 1) % w);
    if y > 0 {
        f((y - 1) * w + x);
    }
    if y + 1 < h {
        f((y + 1) * w + x);
    }
}

/// How attractive a cell is to settle. Food first â€” every real settlement
/// pattern is built on what can be eaten locally â€” then fresh water, then
/// the sea, then minerals.
fn habitability(world: &World) -> Vec<f32> {
    let (w, h) = (world.width, world.height);
    let n = w * h;
    let g = &world.geology;

    // Fresh water and coastal access, spread a couple of cells so a site
    // beside a river scores nearly as well as one on it.
    let mut water = vec![0.0f32; n];
    let mut coast = vec![0.0f32; n];
    for i in 0..n {
        if world.river[i] || world.lake[i] {
            water[i] = 1.0;
        }
        if world.biomes[i] == Biome::Shallows || world.biomes[i] == Biome::Beach {
            coast[i] = 1.0;
        }
    }
    for _ in 0..2 {
        let (ws, cs) = (water.clone(), coast.clone());
        for i in 0..n {
            let (mut bw, mut bc) = (ws[i], cs[i]);
            neighbours(i, w, h, |j| {
                bw = bw.max(ws[j] * 0.6);
                bc = bc.max(cs[j] * 0.6);
            });
            water[i] = bw;
            coast[i] = bc;
        }
    }

    let mut score = vec![0.0f32; n];
    for i in 0..n {
        if world.elevation.data[i] < world.sea_level {
            continue;
        }
        // Nobody founds a capital on a glacier or in open water.
        if matches!(world.biomes[i], Biome::Snowcap | Biome::Mountain) {
            continue;
        }
        let minerals = (g.ore.data[i].max(g.coal.data[i]).max(g.petroleum.data[i]))
            .min(1.0);
        let temp = world.temperature.data[i];
        // Habitable band: hard freeze and blazing desert both suppress.
        let climate = if temp < 0.18 {
            temp / 0.18
        } else {
            1.0
        };

        score[i] = (0.44 * g.fertility.data[i]
            + 0.28 * water[i]
            + 0.16 * coast[i]
            + 0.12 * minerals)
            * climate;
    }
    score
}

/// Cost of expanding into a cell. Cheap on flat, watered, temperate ground;
/// dear across mountains, desert, ice, and open water â€” which is where
/// borders end up sitting.
fn expansion_cost(world: &World) -> Vec<f32> {
    let n = world.width * world.height;
    let mut cost = vec![0.0f32; n];

    for i in 0..n {
        let b = world.biomes[i];
        cost[i] = match b {
            // Deep water stops land expansion; shallows and beach let a
            // polity across a strait but not an ocean.
            Biome::Ocean => 260.0,
            Biome::Shallows => 26.0,
            Biome::Beach => 1.0,
            Biome::Snowcap => 45.0,
            Biome::Mountain => 14.0,
            Biome::Desert => 9.0,
            Biome::Tundra => 7.0,
            Biome::Taiga => 3.0,
            Biome::Swamp => 3.5,
            Biome::Rainforest => 3.0,
            Biome::Shrubland | Biome::Savanna => 1.8,
            Biome::Forest => 1.6,
            Biome::Grassland => 1.0,
        };
        // Rivers are corridors, not just borders: they carried every early
        // expansion that mattered.
        if world.river[i] {
            cost[i] *= 0.55;
        }
    }
    cost
}

/// Regional average of a per-cell field â€” block means, smoothly
/// interpolated. Ocean is excluded so a coastal region is not dragged down
/// by the water beside it.
fn regional_average(world: &World, field: &[f32], block: usize) -> Vec<f32> {
    let (w, h) = (world.width, world.height);
    let block = block.max(1);
    let (bw, bh) = (w.div_ceil(block), h.div_ceil(block));

    let mut sum = vec![0.0f32; bw * bh];
    let mut count = vec![0u32; bw * bh];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if world.elevation.data[i] < world.sea_level {
                continue;
            }
            let b = (y / block) * bw + (x / block);
            sum[b] += field[i];
            count[b] += 1;
        }
    }
    let coarse: Vec<f32> = sum
        .iter()
        .zip(&count)
        .map(|(&s, &c)| if c > 0 { s / c as f32 } else { 0.0 })
        .collect();

    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        let fy = (y as f32 + 0.5) / block as f32 - 0.5;
        let y0f = fy.floor();
        let ty = (fy - y0f).clamp(0.0, 1.0);
        let y0 = (y0f as i32).clamp(0, bh as i32 - 1) as usize;
        let y1 = (y0 + 1).min(bh - 1);

        for x in 0..w {
            let fx = (x as f32 + 0.5) / block as f32 - 0.5;
            let x0f = fx.floor();
            let tx = fx - x0f;
            let x0 = (x0f as i32).rem_euclid(bw as i32) as usize;
            let x1 = (x0 + 1) % bw;

            let top = coarse[y0 * bw + x0] * (1.0 - tx) + coarse[y0 * bw + x1] * tx;
            let bot = coarse[y1 * bw + x0] * (1.0 - tx) + coarse[y1 * bw + x1] * tx;
            out[y * w + x] = top * (1.0 - ty) + bot * ty;
        }
    }
    out
}

/// Pick founding sites: the best-scoring cells, each far enough from the
/// ones already chosen. Greedy, deterministic.
///
/// **Spacing varies with how good the country is.** Rich regions carry more
/// people, more people means more competing centres of power, and competing
/// centres means many small states â€” Europe, the Indian subcontinent, the
/// Chinese warring states. Marginal country supports few centres, so one
/// state ends up spanning enormous distances almost unopposed â€” Russia,
/// Canada, the Sahara. Spacing the cores evenly instead makes every state
/// the same size, which is the one shape real political maps never take.
///
/// Returns fewer than `target` when the land cannot hold that many â€” a
/// fragmented archipelago genuinely supports fewer separated cores than a
/// supercontinent of the same area, and that is the honest answer.
fn choose_cores(world: &World, score: &[f32], target: usize) -> Vec<usize> {
    let (w, h) = (world.width, world.height);

    let mut candidates: Vec<usize> = (0..w * h).filter(|&i| score[i] > 0.02).collect();
    if candidates.is_empty() || target == 0 {
        return Vec::new();
    }
    // Deterministic: score descending, index as tiebreak.
    candidates.sort_by(|&a, &b| score[b].total_cmp(&score[a]).then(a.cmp(&b)));

    // Regional prosperity, on a scale much larger than a single good valley.
    let regional = regional_average(world, score, (w / 16).max(4));
    let mut sorted: Vec<f32> = candidates.iter().map(|&i| regional[i]).collect();
    sorted.sort_by(f32::total_cmp);
    let lo = sorted[sorted.len() / 10];
    let hi = sorted[sorted.len() * 9 / 10];
    let span = (hi - lo).max(1e-6);

    // Rich regions pack cores tightly; poor regions hold them apart.
    let multiplier = |i: usize| {
        let prosperity = ((regional[i] - lo) / span).clamp(0.0, 1.0);
        1.85 - 1.25 * prosperity
    };

    // Scale the base spacing so the *whole map* holds about `target` cores.
    //
    // Without this the rich regions swallow every core before the greedy
    // pass ever reaches marginal country â€” a tight local spacing means high
    // local capacity, and capacity is consumed in score order. The result is
    // all the cores crowded into the good land and then Voronoi-ing out into
    // equal-sized states, which is exactly the uniformity this is meant to
    // break. Summing 1/multiplierÂ² over the land gives the map's capacity at
    // unit spacing; solving for `target` yields the scale.
    let inv_sq: f32 = candidates.iter().map(|&i| 1.0 / multiplier(i).powi(2)).sum();
    // 0.72 is the packing efficiency of a Poisson-disc sample; without it
    // the greedy pass falls short of the requested count.
    let base = ((inv_sq / target as f32).sqrt() * 0.72).max(1.5);

    let mut cores: Vec<usize> = Vec::with_capacity(target);
    for &i in &candidates {
        if cores.len() >= target {
            break;
        }
        let sp = base * multiplier(i);
        let sp2 = sp * sp;

        let (xi, yi) = ((i % w) as f32, (i / w) as f32);
        let far_enough = cores.iter().all(|&c| {
            let (xc, yc) = ((c % w) as f32, (c / w) as f32);
            // X wraps: take the shorter way round the cylinder.
            let mut dx = (xi - xc).abs();
            if dx > w as f32 * 0.5 {
                dx = w as f32 - dx;
            }
            let dy = yi - yc;
            dx * dx + dy * dy >= sp2
        });
        if far_enough {
            cores.push(i);
        }
    }
    cores
}

/// How effectively each state projects power, relative to the others.
///
/// Deliberately **not** derived from the quality of the surrounding
/// country. Geography already decides state size twice over â€” through core
/// spacing, which packs rivals close together in rich regions, and through
/// the terrain expansion cost. Feeding habitability in here a third time
/// cancels the first: a capital in marginal country would get a low reach
/// and stay small, when the whole point is that it has no rivals for a
/// thousand miles and should sprawl.
///
/// So this is state capacity â€” the part that is history rather than
/// geography. Skewed, because most polities are unremarkable and a few are
/// exceptionally well-run. Territory scales with roughly the square of
/// reach, so the range is tighter than it looks.
fn core_reach(count: usize, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed ^ 0x9E37_79B9_7F4A_7C15);
    (0..count)
        .map(|_| {
            let r = rng.next_f32();
            0.78 + 0.72 * r * r
        })
        .collect()
}

impl Polities {
    /// Partition the world into roughly `target` polities. The actual count,
    /// and every border, emerges from the terrain.
    pub fn partition(world: &World, target: usize) -> Self {
        let (w, h) = (world.width, world.height);
        let n = w * h;

        let score = habitability(world);
        let cost = expansion_cost(world);

        let cores = choose_cores(world, &score, target);

        // How far a polity can hold territory before distance and terrain
        // defeat it. Without a limit every ice cap and desert interior ends
        // up owned, and a planet with three states on it has no frontier â€”
        // which is not how thinly-settled worlds work.
        let land_cells = (0..n)
            .filter(|&i| world.elevation.data[i] >= world.sea_level)
            .count();
        let spacing = (land_cells as f32 / cores.len().max(1) as f32).sqrt();
        let budget = spacing * 4.5;

        let reach = core_reach(cores.len(), world.seed);

        let mut owner = vec![UNCLAIMED; n];
        let mut best = vec![f32::INFINITY; n];
        let mut heap: BinaryHeap<Reverse<(Cost, usize, u16)>> = BinaryHeap::new();

        for (id, &c) in cores.iter().enumerate() {
            best[c] = 0.0;
            heap.push(Reverse((Cost(0.0), c, id as u16)));
        }

        // Multi-source Dijkstra. Every cell falls to whichever core can
        // reach it most cheaply â€” so a polity's reach is set by the terrain
        // between it and its neighbours, and borders settle on the ridges,
        // deserts and straits where two expansions cost the same.
        while let Some(Reverse((Cost(d), i, id))) = heap.pop() {
            if d > best[i] {
                continue; // stale entry
            }
            owner[i] = id;
            // A well-founded state covers ground more cheaply, so it wins
            // the contested middle and pushes its border further out.
            let ease = reach[id as usize];
            neighbours(i, w, h, |j| {
                let nd = d + cost[j] / ease;
                if nd < best[j] && nd <= budget {
                    best[j] = nd;
                    heap.push(Reverse((Cost(nd), j, id)));
                }
            });
        }

        // Water is never territory, whatever the flood reached across.
        for i in 0..n {
            if matches!(world.biomes[i], Biome::Ocean | Biome::Shallows) {
                owner[i] = UNCLAIMED;
            }
        }

        // Tally what each ended up holding.
        let g = &world.geology;
        let mut list: Vec<Polity> = cores
            .iter()
            .enumerate()
            .map(|(id, &core)| Polity {
                core,
                reach: reach[id],
                cells: 0,
                food: 0.0,
                ore_cells: 0,
                coal_cells: 0,
                petroleum_cells: 0,
                coastal: false,
            })
            .collect();

        for i in 0..n {
            let id = owner[i];
            if id == UNCLAIMED {
                continue;
            }
            let p = &mut list[id as usize];
            p.cells += 1;
            p.food += g.fertility.data[i];
            if g.ore.data[i] >= WORKABLE {
                p.ore_cells += 1;
            }
            if g.coal.data[i] >= WORKABLE {
                p.coal_cells += 1;
            }
            if g.petroleum.data[i] >= WORKABLE {
                p.petroleum_cells += 1;
            }
            if !p.coastal && world.biomes[i] == Biome::Beach {
                p.coastal = true;
            }
        }

        Polities {
            width: w,
            height: h,
            owner,
            list,
        }
    }

    /// Polities that actually hold territory, largest first, as
    /// `(id, &Polity)`.
    pub fn ranked(&self) -> Vec<(u16, &Polity)> {
        let mut v: Vec<(u16, &Polity)> = self
            .list
            .iter()
            .enumerate()
            .filter(|(_, p)| p.cells > 0)
            .map(|(i, p)| (i as u16, p))
            .collect();
        v.sort_by(|a, b| b.1.cells.cmp(&a.1.cells).then(a.0.cmp(&b.0)));
        v
    }

    /// Share of all claimed land held by the largest `k` polities. The
    /// number that says whether this is a world of great powers or of
    /// equals: high means a few states dominate, low means no one does.
    pub fn concentration(&self, k: usize) -> f32 {
        let ranked = self.ranked();
        let total: usize = ranked.iter().map(|(_, p)| p.cells).sum();
        if total == 0 {
            return 0.0;
        }
        let top: usize = ranked.iter().take(k).map(|(_, p)| p.cells).sum();
        top as f32 / total as f32
    }

    /// A stable, well-separated display colour per polity id.
    pub fn colour(id: u16) -> [u8; 3] {
        // Golden-ratio hue walk: consecutive ids land far apart on the
        // wheel, so neighbouring territories never share a shade.
        let hue = (id as f32 * 0.61803399).fract();
        let sat = 0.52 + ((id % 3) as f32) * 0.12;
        let val = 0.72 + ((id % 2) as f32) * 0.16;
        hsv_to_rgb(hue, sat, val)
    }
}

fn hsv_to_rgb(hue: f32, s: f32, v: f32) -> [u8; 3] {
    let i = (hue * 6.0).floor();
    let f = hue * 6.0 - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match (i as i32).rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8,
    ]
}
