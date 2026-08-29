//! Settlement placement.
//!
//! Where people actually live, inside the territories `polity` drew. Runs
//! after both, reading them.
//!
//! Sites are chosen for the reasons real ones were — food, fresh water, a
//! harbour, minerals — and a settlement's size comes from the land it
//! draws on, not from a die roll. A city on a wide fertile plain grows
//! large because there is a large plain feeding it; one wedged in a
//! mountain valley stays a town however good the site itself is. That is
//! what produces a realistic scatter of a few great cities and a long tail
//! of small places, without a rank-size rule being imposed.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::polity::{Polities, UNCLAIMED};
use crate::world::{Biome, World};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Seat of government. One per polity.
    Capital,
    City,
    Town,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Capital => "Capital",
            Kind::City => "City",
            Kind::Town => "Town",
        }
    }

    pub fn colour(self) -> [u8; 3] {
        match self {
            Kind::Capital => [255, 240, 140],
            Kind::City => [255, 130, 90],
            Kind::Town => [235, 235, 235],
        }
    }
}

#[derive(Clone, Debug)]
pub struct Settlement {
    pub cell: usize,
    pub polity: u16,
    pub kind: Kind,
    pub population: u32,
    /// Land this settlement is the nearest one to — its hinterland. Size
    /// here is what makes a city large.
    pub catchment: usize,
    pub on_water: bool,
    pub coastal: bool,
}

pub struct Settlements {
    pub width: usize,
    pub height: usize,
    pub list: Vec<Settlement>,
    /// Nearest settlement per land cell, or `u32::MAX`.
    pub catchment_of: Vec<u32>,
}

pub const NO_SETTLEMENT: u32 = u32::MAX;

/// World population, spread across the land by what it can feed. Earth at
/// the Information Age baseline is about 8 billion.
const WORLD_POPULATION: f64 = 8.0e9;

/// Share of people living in a named settlement rather than dispersed
/// across the countryside. Roughly Earth's current urban fraction.
const URBAN_SHARE: f64 = 0.57;

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

/// What makes a spot worth building on. Deliberately weighted differently
/// from the polity habitability score: a *state* forms around broad fertile
/// country, but a *town* forms at a specific advantage — a ford, a harbour,
/// a mine.
fn site_score(world: &World) -> Vec<f32> {
    let (w, h) = (world.width, world.height);
    let n = w * h;
    let g = &world.geology;

    let mut score = vec![0.0f32; n];
    for i in 0..n {
        if world.elevation.data[i] < world.sea_level {
            continue;
        }
        if matches!(world.biomes[i], Biome::Snowcap) {
            continue;
        }

        let mut water = 0.0f32;
        let mut coast = 0.0f32;
        neighbours(i, w, h, |j| {
            if world.river[j] || world.lake[j] {
                water = water.max(0.85);
            }
            if matches!(world.biomes[j], Biome::Shallows | Biome::Beach) {
                coast = coast.max(0.9);
            }
        });
        if world.river[i] || world.lake[i] {
            water = 1.0;
        }
        if world.biomes[i] == Biome::Beach {
            coast = 1.0;
        }

        let minerals = g.ore.data[i].max(g.coal.data[i]).max(g.petroleum.data[i]);
        let rough = match world.biomes[i] {
            Biome::Mountain => 0.25,
            Biome::Swamp => 0.55,
            Biome::Tundra => 0.5,
            Biome::Desert => 0.6,
            _ => 1.0,
        };
        let cold = (world.temperature.data[i] / 0.20).clamp(0.15, 1.0);

        score[i] = (0.34 * g.fertility.data[i]
            + 0.28 * water
            + 0.22 * coast
            + 0.16 * minerals)
            * rough
            * cold;
    }
    score
}

/// Travel cost between adjacent cells, for working out which settlement a
/// stretch of country belongs to. Water is impassable here — a hinterland
/// is reached overland.
fn travel_cost(world: &World) -> Vec<f32> {
    let n = world.width * world.height;
    let mut cost = vec![f32::INFINITY; n];
    for i in 0..n {
        cost[i] = match world.biomes[i] {
            Biome::Ocean | Biome::Shallows => continue,
            Biome::Mountain => 9.0,
            Biome::Snowcap => 20.0,
            Biome::Desert => 5.0,
            Biome::Tundra => 4.0,
            Biome::Swamp => 3.0,
            Biome::Rainforest => 2.6,
            Biome::Taiga => 2.2,
            Biome::Forest => 1.5,
            Biome::Shrubland | Biome::Savanna => 1.3,
            _ => 1.0,
        };
        if world.river[i] {
            cost[i] *= 0.5;
        }
    }
    cost
}

impl Settlements {
    /// Place roughly `target` settlements across the world.
    ///
    /// Every polity gets a capital regardless of size; the rest are shared
    /// out in proportion to how much land there is to feed them.
    pub fn place(world: &World, pol: &Polities, target: usize) -> Self {
        let (w, h) = (world.width, world.height);
        let n = w * h;

        let score = site_score(world);
        let ranked = pol.ranked();
        if ranked.is_empty() {
            return Settlements {
                width: w,
                height: h,
                list: Vec::new(),
                catchment_of: vec![NO_SETTLEMENT; n],
            };
        }

        // Cells belonging to each polity, best site first.
        let mut by_polity: Vec<Vec<usize>> = vec![Vec::new(); pol.list.len()];
        for i in 0..n {
            let id = pol.owner[i];
            if id != UNCLAIMED && score[i] > 0.02 {
                by_polity[id as usize].push(i);
            }
        }
        for cells in &mut by_polity {
            cells.sort_by(|&a, &b| score[b].total_cmp(&score[a]).then(a.cmp(&b)));
        }

        // Share the budget out by carrying capacity, not by area — empty
        // steppe supports few towns however much of it there is.
        let total_food: f32 = ranked.iter().map(|(_, p)| p.food).sum::<f32>().max(1e-6);
        let spare = target.saturating_sub(ranked.len());

        let mut list: Vec<Settlement> = Vec::with_capacity(target);
        for (id, p) in &ranked {
            let cells = &by_polity[*id as usize];
            if cells.is_empty() {
                continue;
            }
            let extra = ((p.food / total_food) * spare as f32).round() as usize;
            let want = 1 + extra;

            // Spacing so towns do not pile into the same good valley.
            let spacing = ((cells.len() as f32 / want as f32).sqrt() * 0.75).max(2.0);
            let sp2 = spacing * spacing;

            let mut placed: Vec<usize> = Vec::with_capacity(want);
            for &i in cells {
                if placed.len() >= want {
                    break;
                }
                let (xi, yi) = ((i % w) as f32, (i / w) as f32);
                let clear = placed.iter().all(|&c| {
                    let (xc, yc) = ((c % w) as f32, (c / w) as f32);
                    let mut dx = (xi - xc).abs();
                    if dx > w as f32 * 0.5 {
                        dx = w as f32 - dx;
                    }
                    let dy = yi - yc;
                    dx * dx + dy * dy >= sp2
                });
                if clear {
                    placed.push(i);
                }
            }

            // The capital sits at the polity's founding site, which is
            // already the best ground it had.
            if !placed.contains(&p.core) {
                placed.insert(0, p.core);
            } else {
                placed.retain(|&c| c != p.core);
                placed.insert(0, p.core);
            }

            for (rank, cell) in placed.into_iter().enumerate() {
                let coastal = matches!(world.biomes[cell], Biome::Beach)
                    || {
                        let mut c = false;
                        neighbours(cell, w, h, |j| {
                            if matches!(world.biomes[j], Biome::Shallows | Biome::Beach) {
                                c = true;
                            }
                        });
                        c
                    };
                list.push(Settlement {
                    cell,
                    polity: *id,
                    kind: if rank == 0 { Kind::Capital } else { Kind::Town },
                    population: 0,
                    catchment: 0,
                    on_water: world.river[cell] || world.lake[cell],
                    coastal,
                });
            }
        }

        // --- Hinterlands: every stretch of country belongs to whichever
        // settlement is cheapest to reach overland. A settlement's size then
        // follows from how much land it draws on. ---
        let cost = travel_cost(world);
        let mut catchment_of = vec![NO_SETTLEMENT; n];
        let mut best = vec![f32::INFINITY; n];
        let mut heap: BinaryHeap<Reverse<(Cost, usize, u32)>> = BinaryHeap::new();

        for (idx, s) in list.iter().enumerate() {
            best[s.cell] = 0.0;
            heap.push(Reverse((Cost(0.0), s.cell, idx as u32)));
        }
        while let Some(Reverse((Cost(d), i, idx))) = heap.pop() {
            if d > best[i] {
                continue;
            }
            catchment_of[i] = idx;
            neighbours(i, w, h, |j| {
                if !cost[j].is_finite() {
                    return;
                }
                let nd = d + cost[j];
                if nd < best[j] {
                    best[j] = nd;
                    heap.push(Reverse((Cost(nd), j, idx)));
                }
            });
        }

        // Population: the food a settlement's hinterland yields, with a
        // premium for the advantages that concentrate people — a harbour, a
        // river crossing, being the seat of government.
        let g = &world.geology;
        let mut weight = vec![0.0f64; list.len()];
        for i in 0..n {
            let idx = catchment_of[i];
            if idx == NO_SETTLEMENT {
                continue;
            }
            list[idx as usize].catchment += 1;
            weight[idx as usize] += g.fertility.data[i] as f64;
        }
        // Cap the hinterland before it is exponentiated. A settlement in
        // empty country inherits every remote cell that routes to it, and
        // with a superlinear exponent that one freak catchment swallows the
        // whole planet's urban population. Real cities do not scale without
        // limit either — beyond some distance the countryside supplies
        // somewhere closer.
        // Saturating rather than a hard cap: a hard clamp flattens every
        // large city to the same size, which is just as wrong in the other
        // direction. This leaves ordinary hinterlands essentially untouched
        // and only bends the runaway ones.
        {
            let mut sorted: Vec<f64> = weight.iter().copied().filter(|&v| v > 0.0).collect();
            sorted.sort_by(f64::total_cmp);
            if !sorted.is_empty() {
                let median = sorted[sorted.len() / 2].max(1e-9);
                let limit = median * 30.0;
                for v in weight.iter_mut() {
                    *v = *v / (1.0 + *v / limit);
                }
            }
        }

        for (idx, s) in list.iter().enumerate() {
            let mut m = 1.0;
            if s.coastal {
                m *= 1.55; // ports concentrate trade
            }
            if s.on_water {
                m *= 1.25;
            }
            if s.kind == Kind::Capital {
                m *= 1.7;
            }
            // Strongly superlinear. Real settlement sizes span four orders
            // of magnitude — a handful of megacities above twenty million,
            // then thousands of towns in the tens of thousands. A mild
            // exponent gives every place roughly the mean, which reads as a
            // planet of identical mid-sized cities and nothing else.
            // Tuned against Earth: the largest city should come out around
            // 150x the median, not 500x.
            weight[idx] = weight[idx].powf(1.6) * m;
        }

        let total_weight: f64 = weight.iter().sum::<f64>().max(1e-9);
        let urban = WORLD_POPULATION * URBAN_SHARE;
        for (idx, s) in list.iter_mut().enumerate() {
            s.population = ((weight[idx] / total_weight) * urban).round() as u32;
        }

        // Promote the genuinely large places. An absolute headcount, so a
        // small polity's biggest town stays a town — being the largest
        // place in a poor country does not make somewhere a city.
        for s in list.iter_mut() {
            if s.kind != Kind::Capital && s.population >= 2_000_000 {
                s.kind = Kind::City;
            }
        }

        Settlements {
            width: w,
            height: h,
            list,
            catchment_of,
        }
    }

    /// Settlements largest first.
    pub fn ranked(&self) -> Vec<&Settlement> {
        let mut v: Vec<&Settlement> = self.list.iter().collect();
        v.sort_by(|a, b| b.population.cmp(&a.population).then(a.cell.cmp(&b.cell)));
        v
    }

    pub fn total_population(&self) -> u64 {
        self.list.iter().map(|s| s.population as u64).sum()
    }

    pub fn count_of(&self, kind: Kind) -> usize {
        self.list.iter().filter(|s| s.kind == kind).count()
    }
}
