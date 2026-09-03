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

impl Settlement {
    /// How big it is, which is not the same question as what it is.
    pub fn band(&self) -> Band {
        Band::of(self.population)
    }
}

impl Settlements {
    /// Everybody on the planet, stored or not.
    pub fn world_population() -> f64 {
        WORLD_POPULATION
    }

    /// What the stored places hold between them.
    pub fn stored_population(&self) -> f64 {
        self.list.iter().map(|s| s.population as f64).sum()
    }

    /// **Everybody else**, who live in villages and hamlets and out on
    /// the land — and who are not in this list because there are
    /// millions of such places. They are generated where they stand.
    pub fn countryside_population(&self) -> f64 {
        (WORLD_POPULATION - self.stored_population()).max(0.0)
    }
}

/// World population, spread across the land by what it can feed. Earth at
/// the Information Age baseline is about 8 billion.
const WORLD_POPULATION: f64 = 8.0e9;

/// **What a hand-dug well reaches without trouble**, in metres. Below
/// this the water is simply there for the digging.
const HAND_DUG_EASILY_M: f32 = 10.0;

/// **And what it reaches at all.** Real hand-dug wells run 10 to 30
/// metres; past that the water needs drilling, which arrived in the
/// nineteenth century and is far too late to have founded anywhere.
const HAND_DUG_LIMIT_M: f32 = 30.0;

/// **How big the nth-largest place on Earth actually is.**
///
/// Not a law, because no single power law fits: Zipf holds tolerably
/// within one country and the world's top is far flatter than it
/// predicts — strict Zipf off Tokyo's 37 million would put the hundredth
/// city at 370,000 when it is nearer six million. So these are the real
/// figures at real ranks, interpolated between in log-log space, and the
/// curve says what it is rather than pretending to be an equation.
///
/// The previous model gave every stored settlement a share of the whole
/// urban population by weight, which came out with an implied exponent
/// near **0.51** against a real one near 1: far too flat, so the median
/// settlement on the planet was a city of 360,000 and there were two
/// small towns and no villages at all.
const RANK_SIZE: [(f64, f64); 10] = [
    (1.0, 37.0e6),
    (10.0, 20.0e6),
    (50.0, 9.0e6),
    (100.0, 6.0e6),
    (500.0, 1.5e6),
    (1_000.0, 800.0e3),
    (2_000.0, 400.0e3),
    (5_000.0, 140.0e3),
    (10_000.0, 60.0e3),
    (50_000.0, 10.0e3),
];

/// What the nth-largest settlement holds, interpolated between the real
/// anchors above.
pub fn population_at_rank(rank: usize) -> f64 {
    let r = (rank.max(1)) as f64;
    if r <= RANK_SIZE[0].0 {
        return RANK_SIZE[0].1;
    }
    for pair in RANK_SIZE.windows(2) {
        let ((r0, p0), (r1, p1)) = (pair[0], pair[1]);
        if r <= r1 {
            let t = (r.ln() - r0.ln()) / (r1.ln() - r0.ln());
            return (p0.ln() + t * (p1.ln() - p0.ln())).exp();
        }
    }
    // Past the last anchor, continue the final slope rather than
    // stopping dead.
    let ((r0, p0), (r1, p1)) = (RANK_SIZE[8], RANK_SIZE[9]);
    let slope = (p1.ln() - p0.ln()) / (r1.ln() - r0.ln());
    (p1.ln() + slope * (r.ln() - r1.ln())).exp()
}

/// **The size bands, which are not the same thing as status.**
///
/// A "city" in Britain is a rank granted by charter, not a headcount:
/// St Davids has 1,600 people and is a city, and Reading has 175,000 and
/// is not. So `Kind` carries what a place *is to its country* and this
/// carries how big it is, and the two are allowed to disagree.
///
/// The bands are the ordinary English ones. Statistical thresholds
/// disagree wildly and are worth knowing about: the line for "urban" is
/// 200 in Norway and Sweden, 2,000 in France and Germany, 2,500 in the
/// United States, 5,000 in India and **50,000 in Japan**.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Band {
    Hamlet,
    Village,
    LargeVillage,
    SmallTown,
    Town,
    LargeTown,
    City,
}

impl Band {
    pub fn of(population: u32) -> Band {
        match population {
            0..=99 => Band::Hamlet,
            100..=999 => Band::Village,
            1_000..=2_499 => Band::LargeVillage,
            2_500..=9_999 => Band::SmallTown,
            10_000..=49_999 => Band::Town,
            50_000..=99_999 => Band::LargeTown,
            _ => Band::City,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Band::Hamlet => "hamlet",
            Band::Village => "village",
            Band::LargeVillage => "large village",
            Band::SmallTown => "small town",
            Band::Town => "town",
            Band::LargeTown => "large town",
            Band::City => "city",
        }
    }
}

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

        // **Water is not one consideration among four. It is the
        // condition.**
        //
        // Before piped supply and treatment — which is to say for all but
        // the last century and a half — a settlement had to sit on water
        // it could reach, and that is why nearly every old city is on a
        // river. Scored additively, a fertile mineral-rich coast with no
        // fresh water came out a fine site, which it is not: it is not a
        // site at all.
        //
        // And the reachable water is not only what is on the surface.
        // **A hand-dug well goes 10 to 30 metres** — the figure this
        // project already records — so ordinary well country, which is
        // where most of the world's villages are, counts too. Past that
        // depth you need drilling, which is a nineteenth-century arrival
        // and far too late to have founded anywhere.
        //
        // `world.water_table` has been generated since it was written and
        // nothing in here had ever asked it.
        let depth = world.depth_to_water_m(i) as f32;
        let well = if depth <= HAND_DUG_EASILY_M {
            1.0
        } else if depth >= HAND_DUG_LIMIT_M {
            0.0
        } else {
            1.0 - (depth - HAND_DUG_EASILY_M) / (HAND_DUG_LIMIT_M - HAND_DUG_EASILY_M)
        };
        let reachable = water.max(0.85 * well);
        if reachable < 0.12 {
            // No water anybody could have dug for. Nobody founded a town
            // here, whatever else the ground is worth.
            continue;
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

        // **A mine is worked wherever the ore is**, which is the other
        // half of why places are where they are. Potosí sits at 4,090
        // metres, Kalgoorlie in desert, Kiruna inside the Arctic Circle —
        // none of them good country, all of them settled, because what
        // is under the ground was worth the trouble. So extraction pays
        // only a softened penalty for rough ground where farming pays
        // the full one.
        let softened = rough + (1.0 - rough) * 0.65;

        let farming = g.fertility.data[i] * rough;
        let trade = coast;
        let extraction = minerals * softened;

        score[i] = (0.30 * farming + 0.22 * trade + 0.24 * extraction + 0.24 * reachable)
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
                // A port is a town with working access to the sea, not one
                // whose own cell happens to be beach. At sixteen kilometres
                // a cell, a city two cells from open water is on the coast
                // by any measure that matters — and getting this wrong
                // makes a planet with no seaports at all, which then has no
                // sea freight and so no cheap trade between continents.
                let coastal = matches!(world.biomes[cell], Biome::Beach) || {
                    let mut found = false;
                    let mut frontier = vec![cell];
                    let mut seen = vec![cell];
                    for _ in 0..2 {
                        let mut next = Vec::new();
                        for &i in &frontier {
                            neighbours(i, w, h, |j| {
                                if matches!(world.biomes[j], Biome::Shallows | Biome::Ocean) {
                                    found = true;
                                } else if !seen.contains(&j) {
                                    seen.push(j);
                                    next.push(j);
                                }
                            });
                        }
                        if found {
                            break;
                        }
                        frontier = next;
                    }
                    found
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

        // **The land decides which places are big; the curve decides how
        // big.** Weight — catchment, water, coast, being a capital — is a
        // good mechanism for ranking sites and a poor one for sizing
        // them: normalising it over a total gave every place roughly the
        // mean and produced a planet of identical cities.
        let mut order: Vec<usize> = (0..list.len()).collect();
        order.sort_by(|&a, &b| {
            weight[b].total_cmp(&weight[a]).then(list[a].cell.cmp(&list[b].cell))
        });
        for (rank, &idx) in order.iter().enumerate() {
            list[idx].population = population_at_rank(rank + 1).round() as u32;
        }

        // **And the stored places are not the whole world.** They hold
        // what the curve gives them; everybody else lives in villages and
        // hamlets and out on the land, which are generated where they
        // stand rather than kept in a list — there are millions of them,
        // and a list of millions is the unbounded state this project has
        // had to remove three times already.
        let stored: f64 = list.iter().map(|s| s.population as f64).sum();
        if stored > WORLD_POPULATION * 0.6 {
            let scale = WORLD_POPULATION * 0.6 / stored;
            for s in list.iter_mut() {
                s.population = (s.population as f64 * scale).round() as u32;
            }
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
