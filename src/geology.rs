//! Geology, mineral & fossil deposits, and soil fertility.
//!
//! Spec A1.3b pass 11 / review Part 6. Civilisation placement and the whole
//! economy read these, so they are generated before any settlement exists.
//!
//! Like the climate fields, nothing here is placed by hand: rock type falls
//! out of elevation and structure, deposits fall out of rock type and
//! history, fertility falls out of rock, climate, slope and water.

use crate::field::Field;
use crate::noise::fbm;
use crate::rng::Rng;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Rock {
    /// Cooled magma — hard, ore-bearing, poor soil parent.
    Igneous,
    /// Heat- and pressure-altered rock of uplifted belts — the richest ores.
    Metamorphic,
    /// Layered deposits of lowlands and old sea beds — soft, fossil fuels,
    /// the best farmland parent material.
    Sedimentary,
}

impl Rock {
    pub const ALL: [Rock; 3] = [Rock::Igneous, Rock::Metamorphic, Rock::Sedimentary];

    pub fn name(self) -> &'static str {
        match self {
            Rock::Igneous => "Igneous",
            Rock::Metamorphic => "Metamorphic",
            Rock::Sedimentary => "Sedimentary",
        }
    }

    pub fn colour(self) -> [u8; 3] {
        match self {
            Rock::Igneous => [150, 95, 90],
            Rock::Metamorphic => [130, 110, 150],
            Rock::Sedimentary => [205, 180, 120],
        }
    }
}

/// The geology layer of a world.
pub struct Geology {
    pub rock: Vec<Rock>,
    /// Soil fertility, 0 (barren) .. 1 (prime farmland).
    pub fertility: Field,
    /// Metal ore concentration, 0..1 (sparse).
    pub ore: Field,
    /// Coal concentration, 0..1 (sparse).
    pub coal: Field,
    /// Oil & gas concentration, 0..1 (sparse).
    pub petroleum: Field,
}

impl Geology {
    /// Cells whose concentration of `field` clears `t` — a "workable
    /// deposit". Coarse-pass stand-in for discrete deposits.
    pub fn deposit_count(field: &Field, t: f32) -> usize {
        field.data.iter().filter(|&&v| v >= t).count()
    }

    /// Contiguous runs of workable cells — the distinct *deposit provinces*
    /// on the map. Returns each province's size in cells, largest first.
    ///
    /// This is the number that matters for settlement: what decides whether
    /// a region can support its own industry is how many separate places
    /// have ore, not what share of the planet's surface does.
    pub fn deposit_provinces(field: &Field, t: f32) -> Vec<usize> {
        let (w, h) = (field.width, field.height);
        let n = w * h;
        let mut seen = vec![false; n];
        let mut sizes = Vec::new();
        let mut stack = Vec::new();

        for start in 0..n {
            if seen[start] || field.data[start] < t {
                continue;
            }
            let mut size = 0usize;
            stack.push(start);
            seen[start] = true;
            while let Some(i) = stack.pop() {
                size += 1;
                let (x, y) = (i % w, i / w);
                let mut visit = |j: usize| {
                    if !seen[j] && field.data[j] >= t {
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
            sizes.push(size);
        }
        sizes.sort_unstable_by(|a, b| b.cmp(a));
        sizes
    }
}

#[inline]
fn bell(x: f32, mu: f32, sigma: f32) -> f32 {
    let z = (x - mu) / sigma;
    (-z * z).exp()
}

/// Value at quantile `q` of `scores`, considering only the cells in `idx`.
/// Deterministic: `total_cmp` with an index tiebreak.
fn land_quantile(scores: &[f32], idx: &[usize], q: f32) -> f32 {
    if idx.is_empty() {
        return f32::INFINITY;
    }
    let mut v: Vec<f32> = idx.iter().map(|&i| scores[i]).collect();
    v.sort_by(|a, b| a.total_cmp(b));
    let k = (((v.len() - 1) as f32) * q.clamp(0.0, 1.0)).round() as usize;
    v[k]
}

/// Rescale a concentration field over land so the 99th-percentile land cell
/// sits at 1.0. Anchoring on a high percentile rather than the raw maximum
/// keeps the scale stable — one freak outlier cell would otherwise squash
/// every real deposit on the map — while still letting a genuinely
/// ore-poor or ore-rich world come out that way.
fn normalise_over_land(f: &mut Field, land: &[usize]) {
    if land.is_empty() {
        return;
    }
    let anchor = land_quantile(&f.data, land, 0.995).max(1e-6);
    for &i in land {
        // Squared after scaling: concentration falls away sharply from a
        // deposit's core, so workable ground stays a small share of the map
        // instead of a broad smear.
        let v = (f.data[i] / anchor).clamp(0.0, 1.0);
        f.data[i] = v * v;
    }
}

/// Rescale `scores` over the given cells to span 0..1. Preserves the shape
/// of the distribution, so a world whose terrain genuinely favours one rock
/// type still comes out that way.
fn stretch_over(scores: &mut [f32], idx: &[usize]) {
    if idx.is_empty() {
        return;
    }
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for &i in idx {
        lo = lo.min(scores[i]);
        hi = hi.max(scores[i]);
    }
    let range = (hi - lo).max(1e-6);
    for &i in idx {
        scores[i] = (scores[i] - lo) / range;
    }
}

/// Local slope magnitude, normalised 0..1. X wraps, Y clamps.
fn slope_field(elev: &Field) -> Field {
    let (w, h) = (elev.width, elev.height);
    let mut s = Field::new(w, h);
    for y in 0..h {
        let ym = y.saturating_sub(1);
        let yp = (y + 1).min(h - 1);
        for x in 0..w {
            let xm = (x + w - 1) % w;
            let xp = (x + 1) % w;
            let gx = (elev.data[y * w + xp] - elev.data[y * w + xm]) * 0.5;
            let gy = (elev.data[yp * w + x] - elev.data[ym * w + x]) * 0.5;
            s.data[y * w + x] = (gx * gx + gy * gy).sqrt();
        }
    }
    s.normalise();
    s
}

/// Proximity to fresh surface water: 1.0 on a river/lake cell, decaying over
/// two cells. Drives alluvial fertility.
fn water_proximity(river: &[bool], lake: &[bool], w: usize, h: usize) -> Field {
    let mut p = Field::new(w, h);
    for i in 0..w * h {
        if river[i] || lake[i] {
            p.data[i] = 1.0;
        }
    }
    for _ in 0..2 {
        let src = p.data.clone();
        for y in 0..h {
            let ym = y.saturating_sub(1);
            let yp = (y + 1).min(h - 1);
            for x in 0..w {
                let xm = (x + w - 1) % w;
                let xp = (x + 1) % w;
                let i = y * w + x;
                let n = src[y * w + xm]
                    .max(src[y * w + xp])
                    .max(src[ym * w + x])
                    .max(src[yp * w + x]);
                p.data[i] = src[i].max(n * 0.55);
            }
        }
    }
    p
}

#[allow(clippy::too_many_arguments)]
pub fn generate(
    elev: &Field,
    sea_level: f32,
    rain_rank: &Field,
    drain_rank: &Field,
    temperature: &Field,
    river: &[bool],
    lake: &[bool],
    rng: &mut Rng,
) -> Geology {
    let (w, h) = (elev.width, elev.height);
    let n = w * h;
    let land_span = (1.0 - sea_level).max(1e-3);

    let slope = slope_field(elev);
    let water = water_proximity(river, lake, w, h);

    // Independent crustal noise: which province, and where the veins run.
    let mut province = fbm(w, h, 3, 2.4, rng);
    province.normalise();
    // Three scales of structure. Regional belts say *where* a mineral
    // district is; the fine field says which specific fields inside it are
    // actually worth working. Without the fine scale a deposit smears
    // across a whole rock province and one nation owns all the ore.
    let mut vein_a = fbm(w, h, 5, 7.0, rng);
    vein_a.normalise();
    let mut vein_b = fbm(w, h, 6, 12.0, rng);
    vein_b.normalise();
    let mut vein_fine = fbm(w, h, 5, 26.0, rng);
    vein_fine.normalise();
    let mut basin_noise = fbm(w, h, 4, 3.0, rng);
    basin_noise.normalise();

    // --- Rock type ---
    //
    // Two scores per land cell, then classify by percentile over land
    // rather than by absolute cutoffs — the same lesson as the biome
    // thresholds. Absolute cutoffs made one rock type swallow the map as
    // soon as the terrain statistics shifted.
    let land: Vec<usize> = (0..n).filter(|&i| elev.data[i] >= sea_level).collect();

    let mut sed_score = vec![0.0f32; n];
    let mut ign_score = vec![0.0f32; n];
    for &i in &land {
        let altitude = ((elev.data[i] - sea_level) / land_span).clamp(0.0, 1.0);
        let flat = 1.0 - slope.data[i];
        // Sedimentary where basins collect: low, flat, and inside a
        // depositional province.
        sed_score[i] = 0.34 * (1.0 - altitude) + 0.30 * flat + 0.36 * basin_noise.data[i];
        // Igneous where the crust is young and undeformed; metamorphic
        // takes the uplifted, high-slope belts.
        ign_score[i] = 0.62 * province.data[i] + 0.38 * (1.0 - slope.data[i]) * (1.0 - altitude);
    }

    // Stretch each score to 0..1 over land, then cut at fixed thresholds.
    // Stretching keeps the cuts meaningful whatever scale the raw scores
    // landed on; fixed cuts (rather than percentiles) let the mix genuinely
    // differ between worlds — a shield world really can come out mostly
    // igneous, where a percentile split would force the same ratios every
    // time.
    stretch_over(&mut sed_score, &land);
    let mut rock = vec![Rock::Sedimentary; n];
    let non_sed: Vec<usize> = land
        .iter()
        .copied()
        .filter(|&i| sed_score[i] < 0.68)
        .collect();
    stretch_over(&mut ign_score, &non_sed);

    for &i in &land {
        rock[i] = if sed_score[i] >= 0.68 {
            Rock::Sedimentary
        } else if ign_score[i] >= 0.52 {
            Rock::Igneous
        } else {
            Rock::Metamorphic
        };
    }

    let mut fertility = Field::new(w, h);
    let mut ore = Field::new(w, h);
    let mut coal = Field::new(w, h);
    let mut petroleum = Field::new(w, h);

    for i in 0..n {
        if elev.data[i] < sea_level {
            continue; // sea floor: left as default, not classified
        }

        // 0 at the coast, ~1 at the highest peak.
        let altitude = ((elev.data[i] - sea_level) / land_span).clamp(0.0, 1.0);

        // --- Metal ore: richest in igneous/metamorphic uplands, but not
        // absent elsewhere. Sedimentary basins carry their own deposits
        // (banded iron, sedimentary copper, placer gold), and lowland ore
        // is common enough that confining it to the highlands would leave
        // most of the planet with nothing to mine.
        let host = match rock[i] {
            Rock::Metamorphic => 1.0,
            Rock::Igneous => 0.85,
            Rock::Sedimentary => 0.42,
        };
        let veins = (vein_a.data[i] * vein_fine.data[i]).powf(1.5);
        ore.data[i] = host * (0.55 + 0.45 * altitude.sqrt()) * veins;

        // --- Coal: sedimentary lowlands with a wet, temperate past ---
        let coal_host = if rock[i] == Rock::Sedimentary { 1.0 } else { 0.12 };
        let peat = bell(rain_rank.data[i], 0.68, 0.24) * bell(temperature.data[i], 0.55, 0.28);
        coal.data[i] = coal_host
            * peat
            * (1.0 - altitude).powf(1.5)
            * (vein_a.data[i] * vein_fine.data[i]).powf(1.4);

        // --- Petroleum: sedimentary former shallow seas near the coast ---
        let pet_host = if rock[i] == Rock::Sedimentary { 1.0 } else { 0.1 };
        let shelf = bell(altitude, 0.06, 0.09); // coastal lowland shelf
        petroleum.data[i] = pet_host
            * shelf
            * province.data[i].powf(1.5)
            * (vein_b.data[i] * vein_fine.data[i]).powf(1.4);

        // --- Fertility ---
        let parent = match rock[i] {
            Rock::Sedimentary => 0.62,
            Rock::Metamorphic => 0.42,
            Rock::Igneous => 0.30,
        };
        let rain_term = bell(rain_rank.data[i], 0.55, 0.30);
        let temp_term = bell(temperature.data[i], 0.60, 0.30);
        let slope_pen = (1.0 - slope.data[i]).powf(0.7);
        let drain_term = bell(drain_rank.data[i], 0.5, 0.32);

        let mut f = parent
            * (0.35 + 0.65 * rain_term)
            * (0.45 + 0.55 * temp_term)
            * slope_pen
            * (0.6 + 0.4 * drain_term);

        // Alluvial soils: floodplains and lake shores are the best farmland.
        f += 0.40 * water.data[i] * slope_pen;

        // Frozen ground grows nothing.
        if temperature.data[i] < 0.14 {
            f *= 0.15;
        }

        fertility.data[i] = f.clamp(0.0, 1.0);
    }

    // Put every deposit field on a common scale so "workable" means the
    // same thing regardless of how the raw products happened to fall out.
    normalise_over_land(&mut ore, &land);
    normalise_over_land(&mut coal, &land);
    normalise_over_land(&mut petroleum, &land);

    Geology {
        rock,
        fertility,
        ore,
        coal,
        petroleum,
    }
}
