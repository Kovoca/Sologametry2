//! Trade infrastructure: navigable waterways and the road network.
//!
//! Spec A4.9. Trade routes are not abstract lines between settlements —
//! they are physical infrastructure running over specific ground, and that
//! is what makes them attackable. You do not ambush the shipment; you drop
//! the bridge.
//!
//! The road network is built the way real ones grew: every place routes
//! toward the nearest larger place, so villages feed towns, towns feed
//! cities, and the shared stretches carry everyone's traffic. Nothing is
//! designated a highway — a highway is a road that ended up carrying a lot.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::settlement::Settlements;
use crate::world::{Biome, World};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Road {
    None,
    /// Local road: one settlement's traffic.
    Track,
    /// Regional road.
    Road,
    /// Trunk route carrying many settlements' traffic.
    Highway,
}

impl Road {
    pub fn colour(self) -> Option<[u8; 3]> {
        match self {
            Road::None => None,
            Road::Track => Some([120, 105, 90]),
            Road::Road => Some([190, 160, 110]),
            Road::Highway => Some([255, 215, 120]),
        }
    }
}

pub struct Network {
    pub width: usize,
    pub height: usize,
    /// People-equivalents of traffic crossing each cell.
    pub traffic: Vec<f64>,
    pub road: Vec<Road>,
    /// Fresh water a vessel can actually work: big rivers and lakes that
    /// reach the sea.
    pub navigable: Vec<bool>,
    /// Cells whose loss would sever a highway — a cut bridge, a blocked
    /// pass. These are the targets worth defending, and worth hitting.
    pub chokepoints: Vec<usize>,
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

/// All eight neighbours, with the step distance. Roads routed on four
/// neighbours come out visibly axis-aligned — real ones cut across country.
#[inline]
fn neighbours8(i: usize, w: usize, h: usize, mut f: impl FnMut(usize, f32)) {
    const D: [(i32, i32); 8] = [
        (-1, -1), (0, -1), (1, -1),
        (-1, 0), (1, 0),
        (-1, 1), (0, 1), (1, 1),
    ];
    let (x, y) = ((i % w) as i32, (i / w) as i32);
    for (dx, dy) in D {
        let ny = y + dy;
        if ny < 0 || ny >= h as i32 {
            continue;
        }
        let nx = (x + dx).rem_euclid(w as i32) as usize;
        let dist = if dx != 0 && dy != 0 {
            std::f32::consts::SQRT_2
        } else {
            1.0
        };
        f(ny as usize * w + nx, dist);
    }
}

/// Shortest distance between two columns on a cylinder.
#[inline]
fn dx_wrapped(a: usize, b: usize, w: usize) -> f32 {
    let d = (a as f32 - b as f32).abs();
    if d > w as f32 * 0.5 {
        w as f32 - d
    } else {
        d
    }
}

/// Cost of laying and running road over a cell. Ocean is impassable —
/// crossing water is a ferry or a port, not a road.
fn build_cost(world: &World) -> Vec<f32> {
    let n = world.width * world.height;
    let mut cost = vec![f32::INFINITY; n];
    for i in 0..n {
        cost[i] = match world.biomes[i] {
            Biome::Ocean | Biome::Shallows => continue,
            Biome::Snowcap => 30.0,
            Biome::Mountain => 12.0,
            Biome::Swamp => 6.0,
            Biome::Desert => 4.5,
            Biome::Rainforest => 3.5,
            Biome::Tundra => 3.5,
            Biome::Taiga => 2.4,
            Biome::Forest => 1.7,
            Biome::Shrubland | Biome::Savanna => 1.3,
            _ => 1.0,
        };
        // A river is cheap to follow and dear to cross. At this resolution
        // we cannot tell the two apart, so treat it as a corridor — which
        // is what the great trunk routes historically were.
        if world.river[i] {
            cost[i] *= 0.8;
        }
    }
    cost
}

/// Rivers a vessel can work: enough flow to float a barge, and a course
/// that reaches the sea. A big river draining into a closed basin is not a
/// trade route.
fn navigable_water(world: &World) -> Vec<bool> {
    let (w, h) = (world.width, world.height);
    let n = w * h;

    // Flow threshold: the top slice of river cells by upstream catchment.
    let mut flows: Vec<f32> = (0..n)
        .filter(|&i| world.river[i] || world.lake[i])
        .map(|i| world.flow_accum.data[i])
        .collect();
    if flows.is_empty() {
        return vec![false; n];
    }
    flows.sort_by(f32::total_cmp);
    let threshold = flows[flows.len() / 2];

    let big: Vec<bool> = (0..n)
        .map(|i| (world.river[i] || world.lake[i]) && world.flow_accum.data[i] >= threshold)
        .collect();

    // Keep only the stretches connected to the sea.
    let mut navigable = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    for i in 0..n {
        if !big[i] {
            continue;
        }
        let mut at_sea = false;
        neighbours(i, w, h, |j| {
            if matches!(world.biomes[j], Biome::Ocean | Biome::Shallows) {
                at_sea = true;
            }
        });
        if at_sea {
            navigable[i] = true;
            stack.push(i);
        }
    }
    while let Some(i) = stack.pop() {
        neighbours(i, w, h, |j| {
            if big[j] && !navigable[j] {
                navigable[j] = true;
                stack.push(j);
            }
        });
    }
    navigable
}

impl Network {
    /// Build the road network over `set`, using at most `hubs` settlements
    /// as route endpoints.
    ///
    /// Only the larger places get roads of their own; a hamlet is served by
    /// whatever passes nearby. Routing every one of nine thousand
    /// settlements would be slow and would draw a network no map could
    /// read.
    pub fn build(world: &World, set: &Settlements, hubs: usize) -> Self {
        let (w, h) = (world.width, world.height);
        let n = w * h;

        let navigable = navigable_water(world);
        let cost = build_cost(world);

        // Largest first: every place will route to the nearest place bigger
        // than itself, so the biggest is the root and traffic flows up the
        // hierarchy exactly as it does in reality.
        let ranked = set.ranked();
        let hubs: Vec<&crate::settlement::Settlement> =
            ranked.into_iter().take(hubs).collect();

        let mut traffic = vec![0.0f64; n];
        let mut visited = vec![u32::MAX; n];
        let mut best = vec![f32::INFINITY; n];
        let mut came_from = vec![usize::MAX; n];

        for (rank, s) in hubs.iter().enumerate() {
            if rank == 0 {
                continue; // the largest city has nowhere bigger to go
            }
            // Nearest larger settlement, straight-line. The road itself
            // still bends around the terrain.
            let mut target = usize::MAX;
            let mut best_d = f32::INFINITY;
            for bigger in hubs.iter().take(rank) {
                let dx = dx_wrapped(s.cell % w, bigger.cell % w, w);
                let dy = (s.cell / w) as f32 - (bigger.cell / w) as f32;
                let d = dx * dx + dy * dy;
                if d < best_d {
                    best_d = d;
                    target = bigger.cell;
                }
            }
            if target == usize::MAX {
                continue;
            }

            // Least-cost path, stopping the moment the target is reached.
            let stamp = rank as u32;
            let mut heap: BinaryHeap<Reverse<(Cost, usize)>> = BinaryHeap::new();
            best[s.cell] = 0.0;
            visited[s.cell] = stamp;
            came_from[s.cell] = usize::MAX;
            heap.push(Reverse((Cost(0.0), s.cell)));

            let mut reached = false;
            while let Some(Reverse((Cost(d), i))) = heap.pop() {
                if i == target {
                    reached = true;
                    break;
                }
                if visited[i] == stamp && d > best[i] {
                    continue;
                }
                neighbours8(i, w, h, |j, dist| {
                    if !cost[j].is_finite() {
                        return;
                    }
                    // Roads share: an existing route is cheaper to widen
                    // than to cut a new one alongside. This is what makes
                    // traffic converge onto trunk routes rather than
                    // spreading into a uniform mesh.
                    let mut step = cost[j] * dist;
                    if traffic[j] > 0.0 {
                        step *= 0.55;
                    }
                    let nd = d + step;
                    if visited[j] != stamp || nd < best[j] {
                        visited[j] = stamp;
                        best[j] = nd;
                        came_from[j] = i;
                        heap.push(Reverse((Cost(nd), j)));
                    }
                });
            }

            if !reached {
                continue; // different landmass, no overland route
            }
            let load = s.population as f64;
            let mut cur = target;
            while cur != usize::MAX {
                traffic[cur] += load;
                if cur == s.cell {
                    break;
                }
                cur = came_from[cur];
            }
        }

        // Classify by what each stretch actually ended up carrying,
        // against **real traffic thresholds** rather than against the rest
        // of this particular map.
        //
        // Ranking the map's own stretches and cutting at percentiles gives
        // every world the same 8% highway and 65% track no matter how rich
        // or empty it is, which is the identical mistake the geology pass
        // had to unlearn. Roads are not built because a stretch is busier
        // than average; they are built when the traffic crossing them
        // passes an absolute figure that pays for the work. A near-empty
        // country should be all dirt, and a dense one mostly paved, and
        // that only happens with fixed cuts.
        //
        // The figures are the standard ones from highway economics:
        //
        // - **Paving a dirt road pays at roughly 200-400 vehicles a day.**
        //   Below that the surface cannot be worn out fast enough to be
        //   worth the capital, and grading gravel is cheaper than laying
        //   pavement. This is the World Bank's long-standing rule of thumb
        //   and it is why most of the world's road length is unpaved.
        // - **A single carriageway carries to about 13,000 vehicles a day**
        //   before congestion justifies dualling; motorway standard is
        //   bought above that, at several times the cost per kilometre.
        //
        // Traffic here is people whose journeys cross the cell. Turning
        // that into vehicles: on an inter-urban corridor something like 2%
        // of the population served makes a trip on a given day, which puts
        // a corridor serving ten million at a couple of hundred thousand
        // vehicles — the right order for a real motorway.
        const TRIPS_PER_HEAD_PER_DAY: f64 = 0.02;
        const DUAL_CARRIAGEWAY_VPD: f64 = 13_000.0;
        const WORTH_PAVING_VPD: f64 = 300.0;
        let road: Vec<Road> = traffic
            .iter()
            .map(|&t| {
                let vpd = t * TRIPS_PER_HEAD_PER_DAY;
                if t <= 0.0 {
                    Road::None
                } else if vpd >= DUAL_CARRIAGEWAY_VPD {
                    Road::Highway
                } else if vpd >= WORTH_PAVING_VPD {
                    Road::Road
                } else {
                    Road::Track
                }
            })
            .collect();

        let chokepoints = find_chokepoints(world, &road, w, h);

        Network {
            width: w,
            height: h,
            traffic,
            road,
            navigable,
            chokepoints,
        }
    }

    pub fn count(&self, kind: Road) -> usize {
        self.road.iter().filter(|&&r| r == kind).count()
    }

    pub fn navigable_count(&self) -> usize {
        self.navigable.iter().filter(|&&v| v).count()
    }
}

/// Highway cells with no parallel route — the bridges, passes and causeways
/// where one demolition severs the network.
///
/// Approximated by local structure rather than a full articulation-point
/// search: a highway cell counts as a chokepoint when the ground beside it
/// is impassable or nearly so, which is exactly the bridge-and-pass case.
fn find_chokepoints(world: &World, road: &[Road], w: usize, h: usize) -> Vec<usize> {
    let mut out = Vec::new();
    for i in 0..w * h {
        if road[i] != Road::Highway {
            continue;
        }
        // How much of the surrounding ground could carry a detour?
        let mut open = 0;
        let mut total = 0;
        neighbours8(i, w, h, |j, _| {
            total += 1;
            let passable = !matches!(
                world.biomes[j],
                Biome::Ocean | Biome::Shallows | Biome::Snowcap | Biome::Mountain
            );
            if passable {
                open += 1;
            }
        });
        if total > 0 && open <= 3 {
            out.push(i);
        }
    }
    out
}
