//! Planetary world generation, steps 1-3 of the design doc.
//!
//!   1. Elevation      -- continental fBm mask + ridged mountain ranges
//!   2. Climate fields -- temperature, rainfall, drainage, each independent
//!   3. Biomes         -- emergent from how those three fields interact
//!
//! Design principle (Tarn Adams, Principle 2): we never place a biome. We
//! model the underlying fields and let the biome fall out of their values.
//! Rain shadows, continental deserts and swamps appear because the fields
//! that produce them in reality are being simulated here too.

use crate::field::Field;
use crate::noise::{fbm, ridged};
use crate::rng::Rng;

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

/// Warm at the equator, cold at the poles, cooled further by altitude, then
/// roughened by a little local noise.
fn generate_temperature(elev: &Field, sea: f32, rng: &mut Rng) -> Field {
    let (w, h) = (elev.width, elev.height);
    let variation = fbm(w, h, 4, 5.0, rng);
    let mut temp = Field::new(w, h);

    for y in 0..h {
        let lat = ((y as f32 / (h - 1) as f32) * 2.0 - 1.0).abs();
        let band = 1.0 - lat.powf(1.45);

        for x in 0..w {
            let i = y * w + x;
            let land_height = (elev.data[i] - sea).max(0.0);
            let t = band - land_height * 0.85 + (variation.data[i] - 0.5) * 0.10;
            temp.data[i] = t.clamp(0.0, 1.0);
        }
    }
    temp
}

/// Moisture advection with orographic lift.
///
/// Wind carries moisture across the map, picking it up over ocean and
/// dropping it when terrain forces the air upward -- so rain shadows form on
/// the lee side of mountains for free, rather than us painting deserts.
///
/// Two details, both found by watching the earlier prototype fail:
///   - Land recycles a little moisture back into the air. Without it,
///     continental interiors collapse to pure desert.
///   - Moisture is mixed between adjacent rows every step. Without it, each
///     row is an independent scanline and the map streaks vertically.
fn generate_rainfall(elev: &Field, temp: &Field, sea: f32, wind: i32, rng: &mut Rng) -> Field {
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

    let jitter = fbm(w, h, 4, 6.0, rng);
    for i in 0..rain.data.len() {
        rain.data[i] = (rain.data[i] + (jitter.data[i] - 0.5) * 0.03).max(0.0);
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
    // them. Hydrology needs elevation, settlement placement needs water and
    // biome, agriculture will read temperature and rainfall directly.
    pub elevation: Field,
    pub temperature: Field,
    pub rainfall: Field,
    pub drainage: Field,

    pub biomes: Vec<Biome>,
}

impl World {
    /// Generate with the default parameters.
    pub fn generate(width: usize, height: usize, seed: u64) -> Self {
        Self::generate_with(width, height, seed, Params::default())
    }

    pub fn generate_with(width: usize, height: usize, seed: u64, params: Params) -> Self {
        let mut rng = Rng::new(seed);

        // 1. Elevation.
        let elevation = generate_elevation(width, height, &mut rng);

        // Resolve sea level by percentile so we hit the target land fraction
        // exactly, instead of guessing a height that shifts every time the
        // noise is retuned.
        let sea_level = elevation.quantile(1.0 - params.target_land);

        // 2. Climate fields, each generated independently.
        let temperature = generate_temperature(&elevation, sea_level, &mut rng);
        let rainfall =
            generate_rainfall(&elevation, &temperature, sea_level, params.prevailing_wind, &mut rng);
        let drainage = generate_drainage(&elevation, &mut rng);

        // Rank the climate fields against the other land tiles so the biome
        // thresholds below stay stable.
        let land: Vec<bool> = elevation.data.iter().map(|&e| e >= sea_level).collect();
        let rain_r = rank_over_land(&rainfall, &land);
        let drain_r = rank_over_land(&drainage, &land);
        let height_r = rank_over_land(&elevation, &land);

        // 3. Biomes emerge from the interplay of the fields above.
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

        World {
            width,
            height,
            seed,
            sea_level,
            elevation,
            temperature,
            rainfall,
            drainage,
            biomes,
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
}
