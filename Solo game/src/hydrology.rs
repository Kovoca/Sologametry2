//! Hydrology: depression filling, flow routing, flow accumulation, and
//! gentle stream-power erosion.
//!
//! This is spec A1.3b passes 4–5 and 9: rivers carve the elevation field
//! *before* the final climate pass runs, so rainfall and temperature
//! describe the terrain water actually shaped rather than raw noise.
//!
//! Resolution note: this runs on the coarse region grid (~16 km per cell).
//! The goal is a realistic drainage network and believable valleys for the
//! climate re-run — not deep canyons, which belong to the per-cell
//! refinement pass that does not exist yet.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::field::Field;

const SQRT2: f32 = std::f32::consts::SQRT_2;

/// Total order over `f32` for the priority queue (`total_cmp`, so NaN can't
/// wedge it and the result is identical on every platform).
#[derive(Clone, Copy, PartialEq)]
struct Elev(f32);
impl Eq for Elev {}
impl PartialOrd for Elev {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // Delegates to `Ord`, which is the canonical form and the only one
        // that cannot drift apart from it. `Ord::cmp` is the total order
        // over the float; a second copy of it here is a second thing to
        // get wrong.
        Some(self.cmp(other))
    }
}
impl Ord for Elev {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// Visit the up-to-8 neighbours of `(x, y)`. X wraps (cylinder); Y clamps
/// — the poles are edges, not seams. `dist` is 1.0 or √2.
#[inline]
fn for_neighbours(x: usize, y: usize, w: usize, h: usize, mut f: impl FnMut(usize, f32)) {
    const D: [(i32, i32); 8] = [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];
    for (dx, dy) in D {
        let ny = y as i32 + dy;
        if ny < 0 || ny >= h as i32 {
            continue;
        }
        let nx = (x as i32 + dx).rem_euclid(w as i32) as usize;
        let dist = if dx != 0 && dy != 0 { SQRT2 } else { 1.0 };
        f(ny as usize * w + nx, dist);
    }
}

/// The routed drainage of an elevation field.
pub struct Flow {
    /// Depression-filled surface. Where this sits above the real ground,
    /// the cell holds standing water (a lake).
    pub filled: Field,
    /// Downstream cell for each cell. `receiver[i] == i` marks a sink
    /// (ocean, or the single lowest cell on a waterless world).
    pub receiver: Vec<usize>,
    /// Priority-flood pop order — lower is closer to an outlet. Doubles as a
    /// topological order: a cell is always popped after its receiver.
    pub order: Vec<u32>,
    /// Flow accumulation: each cell's own weight plus everything upstream.
    pub accum: Field,
}

/// Fill depressions (Barnes/Priority-Flood), route flow by steepest descent
/// on the filled surface, and accumulate `weight` downstream.
pub fn route(elev: &Field, sea_level: f32, weight: &Field) -> Flow {
    let (w, h) = (elev.width, elev.height);
    let n = w * h;

    // --- Priority-Flood: raise every cell to the lowest lip it must cross
    // to reach an outlet. Flat pools form where that lip is above ground. ---
    let mut filled = elev.data.clone();
    let mut closed = vec![false; n];
    let mut order = vec![u32::MAX; n];
    let mut heap: BinaryHeap<Reverse<(Elev, usize)>> = BinaryHeap::new();

    for i in 0..n {
        if elev.data[i] < sea_level {
            closed[i] = true;
            heap.push(Reverse((Elev(filled[i]), i)));
        }
    }
    // Waterless world: seed the single lowest cell so routing still resolves.
    if heap.is_empty() {
        let lo = (0..n)
            .min_by(|&a, &b| elev.data[a].total_cmp(&elev.data[b]))
            .unwrap_or(0);
        closed[lo] = true;
        heap.push(Reverse((Elev(filled[lo]), lo)));
    }

    let mut counter = 0u32;
    while let Some(Reverse((Elev(level), i))) = heap.pop() {
        order[i] = counter;
        counter += 1;
        for_neighbours(i % w, i / w, w, h, |j, _| {
            if closed[j] {
                return;
            }
            closed[j] = true;
            if filled[j] < level {
                filled[j] = level;
            }
            heap.push(Reverse((Elev(filled[j]), j)));
        });
    }
    let filled = Field {
        width: w,
        height: h,
        data: filled,
    };

    // --- Receivers: steepest descent on the filled surface. On flats
    // (equal filled elevation) route toward the smaller flood order, which
    // points back toward the outlet. ---
    let mut receiver = vec![0usize; n];
    for i in 0..n {
        if elev.data[i] < sea_level {
            receiver[i] = i; // ocean is a sink
            continue;
        }
        let ei = filled.data[i];
        let mut best = i;
        let mut best_drop = 0.0f32;
        let mut best_order = order[i];
        for_neighbours(i % w, i / w, w, h, |j, dist| {
            let drop = (ei - filled.data[j]) / dist;
            if drop > best_drop + 1e-9 {
                best_drop = drop;
                best = j;
                best_order = order[j];
            } else if best_drop <= 1e-9 && drop.abs() <= 1e-9 && order[j] < best_order {
                best = j;
                best_order = order[j];
            }
        });
        receiver[i] = best;
    }

    // --- Accumulation: push weight downstream, upstream cells first.
    // `order` descending is a valid processing order (receiver popped
    // before its donors). ---
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| order[b].cmp(&order[a]));
    let mut accum = weight.data.clone();
    for &i in &idx {
        let r = receiver[i];
        if r != i {
            accum[r] += accum[i];
        }
    }
    let accum = Field {
        width: w,
        height: h,
        data: accum,
    };

    Flow {
        filled,
        receiver,
        order,
        accum,
    }
}

/// Gentle stream-power incision: lower high-flow land cells, clamped so a
/// cell never drops below the cell it drains into. Processes outlet-first
/// so each clamp sees its receiver's already-eroded height.
pub fn erode(elev: &mut Field, flow: &Flow, sea_level: f32, strength: f32) {
    let n = elev.data.len();
    let max_accum = flow.accum.data.iter().copied().fold(1.0f32, f32::max);

    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| flow.order[a].cmp(&flow.order[b]));

    for &i in &idx {
        if elev.data[i] < sea_level {
            continue;
        }
        let r = flow.receiver[i];
        if r == i {
            continue;
        }
        let slope = (elev.data[i] - elev.data[r]).max(0.0);
        let a = (flow.accum.data[i] / max_accum).sqrt();
        let incision = strength * a * slope.sqrt();
        let floor = elev.data[r] + 1e-4;
        elev.data[i] = (elev.data[i] - incision).max(floor).max(0.0);
    }
}

/// Diffuse (average toward neighbours) land cells only; ocean is held
/// fixed. Creates plains and takes the sharpest edges off raw ridged noise.
pub fn diffuse_land(elev: &mut Field, sea_level: f32, rate: f32, iters: usize) {
    let (w, h) = (elev.width, elev.height);
    for _ in 0..iters {
        let src = elev.data.clone();
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if src[i] < sea_level {
                    continue;
                }
                let mut sum = 0.0f32;
                let mut count = 0.0f32;
                for_neighbours(x, y, w, h, |j, _| {
                    sum += src[j];
                    count += 1.0;
                });
                let avg = sum / count.max(1.0);
                elev.data[i] = src[i] * (1.0 - rate) + avg * rate;
            }
        }
    }
}

/// Drop connected components of a boolean mask smaller than `min_size`
/// cells (4-connected, X wrapping). Turns scattered single-cell noise into
/// coherent water bodies.
pub fn despeckle(mask: &mut [bool], w: usize, h: usize, min_size: usize) {
    let n = w * h;
    let mut seen = vec![false; n];
    let mut stack = Vec::new();
    let mut comp = Vec::new();

    for start in 0..n {
        if !mask[start] || seen[start] {
            continue;
        }
        comp.clear();
        stack.push(start);
        seen[start] = true;
        while let Some(i) = stack.pop() {
            comp.push(i);
            let (x, y) = (i % w, i / w);
            let mut visit = |j: usize| {
                if mask[j] && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            };
            visit(y * w + (x + w - 1) % w);
            visit(y * w + (x + 1) % w);
            if y > 0 {
                visit((y - 1) * w + x);
            }
            if y + 1 < h {
                visit((y + 1) * w + x);
            }
        }
        if comp.len() < min_size {
            for &i in &comp {
                mask[i] = false;
            }
        }
    }
}

/// River and lake masks from a routed flow field.
///
/// - **River:** a land cell in the top `river_land_fraction` of land cells
///   by flow accumulation *and* with a real downhill neighbour, so water
///   genuinely moves through it. Using a percentile keeps river coverage
///   stable across map resolutions.
/// - **Lake:** standing water — an inland terminal sink, a cell the
///   depression fill raised above the real ground, or a high-flow cell with
///   nowhere lower to drain (it pools).
pub fn extract_water(
    elev: &Field,
    flow: &Flow,
    sea_level: f32,
    river_land_fraction: f32,
) -> (Vec<bool>, Vec<bool>) {
    let (w, h) = (elev.width, elev.height);
    let n = w * h;

    // Accumulation threshold: the (1 - fraction) quantile over land cells.
    let mut land_accum: Vec<f32> = (0..n)
        .filter(|&i| elev.data[i] >= sea_level)
        .map(|i| flow.accum.data[i])
        .collect();
    if land_accum.is_empty() {
        return (vec![false; n], vec![false; n]);
    }
    land_accum.sort_by(f32::total_cmp);
    let q = ((land_accum.len() - 1) as f32 * (1.0 - river_land_fraction)).round() as usize;
    let threshold = land_accum[q].max(1.0);

    let has_lower_neighbour = |i: usize| {
        let (x, y) = (i % w, i / w);
        let mut lower = false;
        for_neighbours(x, y, w, h, |j, _| {
            if elev.data[j] < elev.data[i] - 1e-5 {
                lower = true;
            }
        });
        lower
    };

    let mut river = vec![false; n];
    let mut lake = vec![false; n];
    for i in 0..n {
        if elev.data[i] < sea_level {
            continue;
        }
        let submerged = flow.filled.data[i] - elev.data[i] > 0.0004;
        let terminal_sink = flow.receiver[i] == i;
        let big_flow = flow.accum.data[i] >= threshold;
        let drains = has_lower_neighbour(i);

        if terminal_sink || submerged || (big_flow && !drains) {
            lake[i] = true;
        } else if big_flow && drains {
            river[i] = true;
        }
    }
    (river, lake)
}
