//! Natural political fragmentation.
//!
//! Partitions the land into the territories a planet's geography *wants*.
//! This is not the history simulation and it places nothing by hand — it
//! answers the prior question the history sim needs answered: given this
//! terrain, where do people settle, and where do the borders fall?
//!
//! Three steps:
//!   1. Score every land cell for habitability (food, water, minerals).
//!   2. Seed cores at the best sites, spaced apart.
//!   3. Grow each core outward at a cost set by the terrain, until the
//!      territories meet. Borders land where expansion got expensive —
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
    /// Cell index of the founding site — the capital.
    pub core: usize,
    /// Founding advantage: how rich and open the country around the capital
    /// is, relative to the other capitals on this planet. Above 1 expands
    /// further for the same effort; below 1 is hemmed in. This is what
    /// makes some states great powers and others perpetual minors.
    pub reach: f32,
    /// Territory size in cells.
    pub cells: usize,
    /// Total soil fertility held: the crude carrying-capacity proxy, and
    /// the best single predictor of which of these becomes a great power.
    pub food: f32,
    pub ore_cells: usize,
    pub coal_cells: usize,
    pub petroleum_cells: usize,
    /// Holds at least one coastal cell — can trade and project by sea.
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

/// How attractive a cell is to settle. Food first — every real settlement
/// pattern is built on what can be eaten locally — then fresh water, then
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
/// dear across mountains, desert, ice, and open water — which is where
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

/// Pick founding sites: the best-scoring cells, each at least `spacing`
/// cells from the ones already chosen. Greedy, deterministic.
///
/// Returns fewer than `target` when the land cannot hold that many — a
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

    // Spread the target count over the habitable area, then leave headroom
    // so terrain — not the spacing rule — is what actually limits the count.
    let spacing = ((candidates.len() as f32 / target as f32).sqrt() * 0.62).max(2.0);
    let sp2 = spacing * spacing;

    let mut cores: Vec<usize> = Vec::with_capacity(target);
    for &i in &candidates {
        if cores.len() >= target {
            break;
        }
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

/// How much country each core has to grow into, relative to the others.
///
/// A capital in a wide fertile basin becomes a great power; one wedged onto
/// a mountainous island stays small however good the site itself is. Sum the
/// habitability of the surrounding country, normalise across all cores, and
/// sharpen — real state sizes differ by orders of magnitude, not by a few
/// percent, and a flat spacing rule gives every state the same size.
fn core_reach(world: &World, score: &[f32], cores: &[usize], radius: f32, seed: u64) -> Vec<f32> {
    let (w, h) = (world.width, world.height);
    let r = radius.max(1.0) as i32;

    let mut raw: Vec<f32> = cores
        .iter()
        .map(|&c| {
            let (cx, cy) = ((c % w) as i32, (c / w) as i32);
            let mut sum = 0.0;
            for dy in -r..=r {
                let y = cy + dy;
                if y < 0 || y >= h as i32 {
                    continue;
                }
                for dx in -r..=r {
                    if dx * dx + dy * dy > r * r {
                        continue;
                    }
                    let x = (cx + dx).rem_euclid(w as i32) as usize;
                    sum += score[y as usize * w + x];
                }
            }
            sum
        })
        .collect();

    let mean = (raw.iter().sum::<f32>() / raw.len().max(1) as f32).max(1e-6);

    // A little jitter so two equally-placed capitals do not end up
    // identical — history is not purely geography.
    let mut rng = Rng::new(seed ^ 0x9E37_79B9_7F4A_7C15);
    for v in &mut raw {
        let luck = 0.85 + 0.30 * rng.next_f32();
        // Exponent and clamp are the dial between "one hegemon and a lot of
        // scraps" and "everyone the same size". These give a spread of
        // roughly an order of magnitude between largest and smallest, which
        // is about what real state areas look like.
        // Territory scales roughly with the *square* of reach, so this
        // clamp is tighter than it looks — 0.7..1.45 already spreads areas
        // by about 4x, on top of the variation terrain alone produces.
        *v = ((*v / mean) * luck).powf(0.95).clamp(0.7, 1.45);
    }
    raw
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
        // up owned, and a planet with three states on it has no frontier —
        // which is not how thinly-settled worlds work.
        let land_cells = (0..n)
            .filter(|&i| world.elevation.data[i] >= world.sea_level)
            .count();
        let spacing = (land_cells as f32 / cores.len().max(1) as f32).sqrt();
        let budget = spacing * 4.5;

        let reach = core_reach(world, &score, &cores, spacing * 0.9, world.seed);

        let mut owner = vec![UNCLAIMED; n];
        let mut best = vec![f32::INFINITY; n];
        let mut heap: BinaryHeap<Reverse<(Cost, usize, u16)>> = BinaryHeap::new();

        for (id, &c) in cores.iter().enumerate() {
            best[c] = 0.0;
            heap.push(Reverse((Cost(0.0), c, id as u16)));
        }

        // Multi-source Dijkstra. Every cell falls to whichever core can
        // reach it most cheaply — so a polity's reach is set by the terrain
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
