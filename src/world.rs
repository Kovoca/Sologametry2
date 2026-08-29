//! Planetary world generation — the coarse pass (spec A1.3b).
//!
//!   1. Base elevation      -- continental fBm mask + ridged mountain ranges
//!   2. Smooth              -- diffuse land to make plains
//!   3. Provisional climate -- RNG-free latitude temp + moisture advection
//!   4. Erosion & rivers    -- flow routing carves channels into elevation
//!   5. Re-smooth
//!   6. Rainfall, final     -- advection against the *carved* elevation
//!   7. Temperature, final  -- latitude + altitude lapse, carved elevation
//!   8. Drainage / runoff
//!   9. Rivers & lakes      -- final flow routing, weighted by final rainfall
//!  10. Biomes              -- emergent from the finished fields
//!
//! Design principle (Tarn Adams, Principle 2): we never place a biome. We
//! model the underlying fields and let the biome fall out of their values.
//! Rain shadows, continental deserts and swamps appear because the fields
//! that produce them in reality are being simulated here too.
//!
//! Passes 6–7 run *after* pass 4 on purpose: climate must describe the
//! terrain water shaped, not the raw noise.

use crate::field::Field;
use crate::geology::{self, Geology};
use crate::hydrology;
use crate::noise::{fbm, ridged};
use crate::rng::Rng;

/// How hard rivers incise their beds in the coarse pass. Deliberately low —
/// 16 km cells, and the point is realistic drainage for the climate re-run,
/// not canyons.
const EROSION_STRENGTH: f32 = 0.06;

/// Roughly this fraction of land cells become river tiles — the ones with
/// the most upstream catchment. A percentile, so it holds across map sizes.
const RIVER_LAND_FRACTION: f32 = 0.05;

/// Tunable generation parameters. More knobs will land here as the pipeline
/// grows; keeping them in one struct means adding one later doesn't churn
/// every function signature.
#[derive(Clone, Copy, Debug)]
pub struct Params {
    /// Fraction of the map that should end up as land. Sea level is solved
    /// to hit this exactly, rather than guessed as an absolute height.
    /// Earth is ~0.29.
    pub target_land: f32,
    /// Prevailing wind: `+1` blows west-to-east, `-1` east-to-west.
    pub prevailing_wind: i32,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            target_land: 0.34,
            prevailing_wind: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// 1. Elevation
// ---------------------------------------------------------------------------

/// The continental mask decides *where* land is; ridged noise decides *how
/// high*. Keeping those jobs separate is what stops the interior becoming
/// one dome. A north/south falloff sinks the poles; the X axis wraps, so a
/// continent can run the full width of the map and around the back.
fn generate_elevation(width: usize, height: usize, rng: &mut Rng) -> Field {
    // Two independent low-frequency fields. `continents` places the land;
    // `carve` pushes some of it back under water. The product of two blobby
    // fields breaks the map into several distinct, irregular landmasses
    // instead of one or two big blobs.
    let mut continents = fbm(width, height, 5, 6.5, rng);
    continents.normalise();
    let mut carve = fbm(width, height, 3, 3.0, rng);
    carve.normalise();
    for (c, k) in continents.data.iter_mut().zip(&carve.data) {
        *c *= 0.5 + 0.5 * k;
    }
    continents.normalise();

    let rolling = fbm(width, height, 6, 5.0, rng);
    let ranges = ridged(width, height, 5, 3.2, rng);

    // Mountain belts only form where the ridge field is already strong, so
    // ranges cross the land rather than tiling the whole thing.
    let mut relief = Field::new(width, height);
    for i in 0..relief.data.len() {
        let belt = (((ranges.data[i] - 0.55) / 0.45).clamp(0.0, 1.0)).powf(1.5);
        relief.data[i] = rolling.data[i] * 0.45 + belt * 0.85;
    }
    relief.normalise();

    let mut elev = Field::new(width, height);
    for y in 0..height {
        // 0 at the equator, 1 at each pole.
        let lat = ((y as f32 / (height - 1) as f32) * 2.0 - 1.0).abs();
        let polar = (1.0 - ((lat - 0.74) / 0.26).clamp(0.0, 1.0).powf(1.5)).clamp(0.0, 1.0);

        for x in 0..width {
            let i = y * width + x;
            // `base` is also the ocean floor: it keeps varying so sea level
            // can be resolved by percentile. If every ocean tile clamped to
            // one flat value the percentile would land on it and the whole
            // map would read as land.
            let base = continents.data[i] * polar;
            let landness = (((base - 0.34) * 2.6).clamp(0.0, 1.0)).powf(0.85);
            elev.data[i] = base * 0.55 + landness * relief.data[i] * 0.90;
        }
    }
    elev.normalise();
    elev
}

// ---------------------------------------------------------------------------
// 2. Climate fields (each generated independently of the others)
// ---------------------------------------------------------------------------

/// Latitude temperature band alone: warm at the equator, cold at the poles,
/// no altitude or noise. Cheap and RNG-free — used as the provisional
/// climate that seeds erosion (spec A1.3b pass 3).
fn latitude_temp(w: usize, h: usize) -> Field {
    let mut temp = Field::new(w, h);
    for y in 0..h {
        let lat = ((y as f32 / (h - 1) as f32) * 2.0 - 1.0).abs();
        let band = (1.0 - lat.powf(1.45)).clamp(0.0, 1.0);
        for x in 0..w {
            temp.data[y * w + x] = band;
        }
    }
    temp
}

/// Final temperature: the latitude band, cooled by altitude (lapse rate),
/// roughened by a little local noise. Runs against the *carved* elevation.
fn generate_temperature(elev: &Field, sea: f32, rng: &mut Rng) -> Field {
    let (w, h) = (elev.width, elev.height);
    let variation = fbm(w, h, 4, 5.0, rng);
    let mut temp = latitude_temp(w, h);

    for i in 0..temp.data.len() {
        let land_height = (elev.data[i] - sea).max(0.0);
        let t = temp.data[i] - land_height * 0.85 + (variation.data[i] - 0.5) * 0.10;
        temp.data[i] = t.clamp(0.0, 1.0);
    }
    temp
}

/// Final rainfall: moisture advection against the carved elevation, plus a
/// little noise jitter so bands are not mechanically smooth.
fn generate_rainfall(elev: &Field, temp: &Field, sea: f32, wind: i32, rng: &mut Rng) -> Field {
    let (w, h) = (elev.width, elev.height);
    let mut rain = advect_rainfall(elev, temp, sea, wind);

    let jitter = fbm(w, h, 4, 6.0, rng);
    for i in 0..rain.data.len() {
        rain.data[i] = (rain.data[i] + (jitter.data[i] - 0.5) * 0.03).max(0.0);
    }
    rain
}

/// Moisture advection with orographic lift — the RNG-free core.
///
/// Wind carries moisture across the map, picking it up over ocean and
/// dropping it when terrain forces the air upward, so rain shadows form on
/// the lee side of mountains for free rather than us painting deserts.
///
/// Two details, both found by watching the earlier prototype fail:
///   - Land recycles a little moisture back into the air. Without it,
///     continental interiors collapse to pure desert.
///   - Moisture is mixed between adjacent rows every step. Without it, each
///     row is an independent scanline and the map streaks vertically.
///
/// RNG-free, so it also produces the provisional rainfall that weights
/// erosion (spec A1.3b pass 3).
fn advect_rainfall(elev: &Field, temp: &Field, sea: f32, wind: i32) -> Field {
    let (w, h) = (elev.width, elev.height);
    let mut rain = Field::new(w, h);

    let columns: Vec<usize> = if wind > 0 {
        (0..w).collect()
    } else {
        (0..w).rev().collect()
    };

    let mut moisture = vec![0.55f32; h];
    let mut scratch = vec![0.0f32; h];
    let mut prev_col = vec![0.0f32; h];

    // Two laps around the cylinder. The first primes the moisture columns;
    // the second is the one recorded, so the wrap-around column stops being
    // a dry seam.
    for lap in 0..2 {
        for (ci, &x) in columns.iter().enumerate() {
            let mut col_elev = vec![0.0f32; h];
            for y in 0..h {
                col_elev[y] = elev.data[y * w + x];
            }

            for y in 0..h {
                let i = y * w + x;
                let warmth = 0.45 + 0.55 * temp.data[i];
                let is_ocean = col_elev[y] < sea;

                moisture[y] = if is_ocean {
                    (moisture[y] + 0.30 * warmth).min(1.0)
                } else {
                    (moisture[y] + 0.045 * warmth).min(1.0)
                };

                let lift = if lap == 0 && ci == 0 {
                    0.0
                } else {
                    (col_elev[y] - prev_col[y]).max(0.0) * 7.0
                };

                let precip = (moisture[y] * (0.035 + lift)).min(moisture[y] * 0.9);
                if lap == 1 {
                    rain.data[i] = precip;
                }
                moisture[y] = (moisture[y] - precip * 0.55).max(0.02);
            }

            // Mix moisture between vertically adjacent rows (clamped at the
            // poles) so air masses are not independent scanlines.
            for y in 0..h {
                let up = moisture[y.saturating_sub(1)];
                let down = moisture[(y + 1).min(h - 1)];
                scratch[y] = (moisture[y] + up + down) / 3.0;
            }
            moisture.copy_from_slice(&scratch);
            prev_col.copy_from_slice(&col_elev);
        }
    }

    // Lateral smoothing so rainfall bands are not perfectly horizontal.
    for _ in 0..2 {
        let src = rain.data.clone();
        for y in 0..h {
            let ym = y.saturating_sub(1);
            let yp = (y + 1).min(h - 1);
            for x in 0..w {
                rain.data[y * w + x] =
                    (src[y * w + x] + src[ym * w + x] + src[yp * w + x]) / 3.0;
            }
        }
    }

    rain
}

/// How readily water leaves a tile: soil permeability biased by local slope.
/// Low drainage plus high rainfall is what produces wetland and swamp.
fn generate_drainage(elev: &Field, rng: &mut Rng) -> Field {
    let (w, h) = (elev.width, elev.height);
    let permeability = fbm(w, h, 5, 4.5, rng);

    let mut slope = Field::new(w, h);
    for y in 0..h {
        let ym = y.saturating_sub(1);
        let yp = (y + 1).min(h - 1);
        for x in 0..w {
            let xm = (x + w - 1) % w;
            let xp = (x + 1) % w;
            let gx = (elev.data[y * w + xp] - elev.data[y * w + xm]) * 0.5;
            let gy = (elev.data[yp * w + x] - elev.data[ym * w + x]) * 0.5;
            slope.data[y * w + x] = (gx * gx + gy * gy).sqrt();
        }
    }
    slope.normalise();

    let mut drainage = Field::new(w, h);
    for i in 0..drainage.data.len() {
        drainage.data[i] = permeability.data[i] * 0.6 + slope.data[i] * 0.4;
    }
    drainage.normalise();
    drainage
}

// ---------------------------------------------------------------------------
// Percentile rank over land tiles
// ---------------------------------------------------------------------------

/// Convert a field to a `0..1` percentile rank computed over land tiles
/// only. Absolute thresholds are fragile -- retuning the rainfall model once
/// turned the whole prototype world into desert because every value slid
/// under the cutoffs. Ranking against the other land tiles keeps the biome
/// thresholds meaningful no matter how the field happens to be scaled.
///
/// Deterministic: ties are broken by tile index.
fn rank_over_land(field: &Field, land: &[bool]) -> Field {
    let mut ranked = Field::new(field.width, field.height);

    let mut idx: Vec<usize> = (0..field.data.len()).filter(|&i| land[i]).collect();
    if idx.is_empty() {
        return ranked;
    }
    idx.sort_by(|&a, &b| field.data[a].total_cmp(&field.data[b]).then(a.cmp(&b)));

    let denom = (idx.len() - 1).max(1) as f32;
    for (rank, &i) in idx.iter().enumerate() {
        ranked.data[i] = rank as f32 / denom;
    }
    ranked
}

// ---------------------------------------------------------------------------
// 3. Biomes (emergent)
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Biome {
    Ocean,
    Shallows,
    Beach,
    Desert,
    Savanna,
    Grassland,
    Shrubland,
    Forest,
    Rainforest,
    Swamp,
    Taiga,
    Tundra,
    Mountain,
    Snowcap,
}

impl Biome {
    /// Every variant, in enum-discriminant order (so `ALL[b as usize] == b`).
    pub const ALL: [Biome; 14] = [
        Biome::Ocean,
        Biome::Shallows,
        Biome::Beach,
        Biome::Desert,
        Biome::Savanna,
        Biome::Grassland,
        Biome::Shrubland,
        Biome::Forest,
        Biome::Rainforest,
        Biome::Swamp,
        Biome::Taiga,
        Biome::Tundra,
        Biome::Mountain,
        Biome::Snowcap,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Biome::Ocean => "Ocean",
            Biome::Shallows => "Shallows",
            Biome::Beach => "Beach",
            Biome::Desert => "Desert",
            Biome::Savanna => "Savanna",
            Biome::Grassland => "Grassland",
            Biome::Shrubland => "Shrubland",
            Biome::Forest => "Forest",
            Biome::Rainforest => "Rainforest",
            Biome::Swamp => "Swamp",
            Biome::Taiga => "Taiga",
            Biome::Tundra => "Tundra",
            Biome::Mountain => "Mountain",
            Biome::Snowcap => "Snowcap",
        }
    }

    pub fn glyph(self) -> char {
        match self {
            Biome::Ocean => '.',
            Biome::Shallows => '~',
            Biome::Beach => ',',
            Biome::Desert => ':',
            Biome::Savanna => '"',
            Biome::Grassland => '"',
            Biome::Shrubland => ';',
            Biome::Forest => 't',
            Biome::Rainforest => 'T',
            Biome::Swamp => 'v',
            Biome::Taiga => 'f',
            Biome::Tundra => '-',
            Biome::Mountain => '^',
            Biome::Snowcap => 'A',
        }
    }

    /// Display colour, RGB 0-255.
    pub fn colour(self) -> [u8; 3] {
        match self {
            Biome::Ocean => [30, 60, 120],
            Biome::Shallows => [55, 100, 165],
            Biome::Beach => [215, 200, 150],
            Biome::Desert => [225, 200, 120],
            Biome::Savanna => [190, 190, 95],
            Biome::Grassland => [140, 180, 90],
            Biome::Shrubland => [150, 160, 95],
            Biome::Forest => [55, 125, 60],
            Biome::Rainforest => [25, 95, 50],
            Biome::Swamp => [75, 110, 80],
            Biome::Taiga => [60, 110, 95],
            Biome::Tundra => [170, 180, 175],
            Biome::Mountain => [120, 115, 110],
            Biome::Snowcap => [245, 245, 250],
        }
    }
}

// ---------------------------------------------------------------------------
// The generated world
// ---------------------------------------------------------------------------

pub struct World {
    pub width: usize,
    pub height: usize,
    pub seed: u64,
    pub sea_level: f32,

    // The raw fields are kept, not discarded: later pipeline stages read
    // them. Settlement placement needs water and biome, agriculture will
    // read temperature and rainfall directly.
    pub elevation: Field,
    pub temperature: Field,
    pub rainfall: Field,
    pub drainage: Field,

    /// Flow accumulation over the carved terrain — upstream catchment per
    /// cell. Kept for the debug overlay and for later navigable-water and
    /// settlement passes.
    pub flow_accum: Field,
    /// Per-cell: this land cell carries a river.
    pub river: Vec<bool>,
    /// Per-cell: this land cell holds a lake (inland standing water).
    pub lake: Vec<bool>,

    pub biomes: Vec<Biome>,

    /// Rock type, soil fertility, and mineral / fossil-fuel concentrations.
    pub geology: Geology,
}

impl World {
    /// Generate with the default parameters.
    pub fn generate(width: usize, height: usize, seed: u64) -> Self {
        Self::generate_with(width, height, seed, Params::default())
    }

    pub fn generate_with(width: usize, height: usize, seed: u64, params: Params) -> Self {
        let mut rng = Rng::new(seed);
        let wind = params.prevailing_wind;

        // 1. Base elevation.
        let mut elevation = generate_elevation(width, height, &mut rng);

        // 2. Smooth the land a little — raw ridged noise is too jagged, and
        // real continents have plains.
        let sea0 = elevation.quantile(1.0 - params.target_land);
        hydrology::diffuse_land(&mut elevation, sea0, 0.18, 1);

        // 3. Provisional climate (RNG-free) to weight erosion: wet mountains
        // must carve bigger valleys than dry ones.
        let sea_prov = elevation.quantile(1.0 - params.target_land);
        let temp_prov = latitude_temp(width, height);
        let rain_prov = advect_rainfall(&elevation, &temp_prov, sea_prov, wind);

        // 4-5. Route flow, incise river channels, re-smooth.
        let flow_prov = hydrology::route(&elevation, sea_prov, &rain_prov);

        // Natural endorheic basins: inland closed depressions in the terrain
        // *before* erosion cuts drainage paths through them. These stay
        // lakes no matter what the final routing does.
        let basin: Vec<bool> = (0..width * height)
            .map(|i| {
                elevation.data[i] >= sea_prov
                    && flow_prov.filled.data[i] - elevation.data[i] > 0.006
            })
            .collect();

        hydrology::erode(&mut elevation, &flow_prov, sea_prov, EROSION_STRENGTH);
        hydrology::diffuse_land(&mut elevation, sea_prov, 0.10, 1);
        elevation.normalise();

        // Re-resolve sea level on the carved terrain so land fraction still
        // hits the target.
        let sea_level = elevation.quantile(1.0 - params.target_land);

        // 6-7. Final climate against the carved elevation.
        let temperature = generate_temperature(&elevation, sea_level, &mut rng);
        let rainfall = generate_rainfall(&elevation, &temperature, sea_level, wind, &mut rng);

        // 8. Drainage / runoff.
        let drainage = generate_drainage(&elevation, &mut rng);

        // 9. Final flow routing on the carved terrain, weighted by final
        // rainfall — this is what the rivers and lakes are read from.
        let flow = hydrology::route(&elevation, sea_level, &rainfall);
        let (mut river, mut lake) =
            hydrology::extract_water(&elevation, &flow, sea_level, RIVER_LAND_FRACTION);

        // Fold in the natural basins found before erosion.
        for i in 0..width * height {
            if basin[i] && elevation.data[i] >= sea_level {
                lake[i] = true;
                river[i] = false;
            }
        }

        // Coherent water bodies, not scattered single-cell noise.
        hydrology::despeckle(&mut lake, width, height, 3);
        hydrology::despeckle(&mut river, width, height, 2);

        // Rank the climate fields against the other land tiles so the biome
        // thresholds below stay stable.
        let land: Vec<bool> = elevation.data.iter().map(|&e| e >= sea_level).collect();
        let rain_r = rank_over_land(&rainfall, &land);
        let drain_r = rank_over_land(&drainage, &land);
        let height_r = rank_over_land(&elevation, &land);

        // 10. Biomes emerge from the interplay of the fields above.
        let mut biomes = vec![Biome::Ocean; width * height];
        for i in 0..biomes.len() {
            let e = elevation.data[i];
            let t = temperature.data[i];
            let r = rain_r.data[i];
            let d = drain_r.data[i];
            let hr = height_r.data[i];

            biomes[i] = if e < sea_level - 0.04 {
                Biome::Ocean
            } else if e < sea_level {
                Biome::Shallows
            } else if e < sea_level + 0.015 {
                Biome::Beach
            } else if hr > 0.965 {
                if t < 0.45 {
                    Biome::Snowcap
                } else {
                    Biome::Mountain
                }
            } else if hr > 0.90 {
                Biome::Mountain
            } else if t < 0.16 {
                Biome::Tundra
            } else if t < 0.32 {
                if r > 0.30 {
                    Biome::Taiga
                } else {
                    Biome::Tundra
                }
            } else if d < 0.22 && r > 0.62 {
                // Poor drainage plus plenty of rain means standing water.
                Biome::Swamp
            } else if r < 0.16 {
                Biome::Desert
            } else if r < 0.34 {
                if t > 0.62 {
                    Biome::Savanna
                } else {
                    Biome::Shrubland
                }
            } else if r < 0.56 {
                Biome::Grassland
            } else if r < 0.80 {
                Biome::Forest
            } else if t > 0.60 {
                Biome::Rainforest
            } else {
                Biome::Forest
            };
        }

        // 11. Geology: rock provinces, mineral & fossil deposits, soil
        // fertility. Civ placement and the economy read these.
        let geology = geology::generate(
            &elevation, sea_level, &rain_r, &drain_r, &temperature, &river, &lake, &mut rng,
        );

        World {
            width,
            height,
            seed,
            sea_level,
            elevation,
            temperature,
            rainfall,
            drainage,
            flow_accum: flow.accum,
            river,
            lake,
            biomes,
            geology,
        }
    }

    /// Fraction of tiles that are dry land (everything above the shallows).
    pub fn land_fraction(&self) -> f32 {
        let land = self
            .biomes
            .iter()
            .filter(|b| !matches!(b, Biome::Ocean | Biome::Shallows))
            .count();
        land as f32 / self.biomes.len() as f32
    }

    /// Tile count per biome, indexed by `biome as usize`.
    pub fn biome_counts(&self) -> [usize; 14] {
        let mut counts = [0usize; 14];
        for b in &self.biomes {
            counts[*b as usize] += 1;
        }
        counts
    }

    /// Land cells carrying a river.
    pub fn river_count(&self) -> usize {
        self.river.iter().filter(|&&r| r).count()
    }

    /// Land cells holding a lake.
    pub fn lake_count(&self) -> usize {
        self.lake.iter().filter(|&&l| l).count()
    }

    /// Any water at cell `i`: ocean, shallows, lake, or river.
    pub fn is_water(&self, i: usize) -> bool {
        self.river[i]
            || self.lake[i]
            || matches!(self.biomes[i], Biome::Ocean | Biome::Shallows)
    }
}
