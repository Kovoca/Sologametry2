//! What grows and what lives on it — spec pipeline step 5.
//!
//! The design doc puts flora and fauna here deliberately: the later
//! ecology, disease and contamination systems need something real to
//! attach to, and so does the economy the moment anybody wants timber,
//! grazing or game.
//!
//! Nothing here is placed. Productivity comes out of temperature and
//! rainfall through a published model, standing biomass comes out of
//! productivity, and what you can hunt comes out of that — the same
//! "model the fields and let the result emerge" rule as the biomes.

use crate::field::Field;
use crate::world::Biome;

/// **Net primary productivity by the Miami model** *(Lieth, 1975)*, in
/// grams of dry matter per square metre per year.
///
/// A real published model rather than a curve picked to look right: plant
/// growth is limited by whichever of heat and water is scarcer, so the
/// answer is the lesser of the two limits. It is why a hot desert and a
/// wet tundra are both unproductive for opposite reasons.
///
/// For scale, real figures: tropical rainforest ~2,200, temperate forest
/// ~1,250, savanna ~900, temperate grassland ~600, tundra ~140, desert
/// ~90, and the global land mean about 700.
pub fn miami_npp(temp_c: f32, rain_mm: f32) -> f32 {
    let by_heat = 3000.0 / (1.0 + (1.315 - 0.119 * temp_c).exp());
    let by_water = 3000.0 * (1.0 - (-0.000_664 * rain_mm).exp());
    by_heat.min(by_water)
}

/// **How much of that growth an animal can actually eat.**
///
/// The thing that stops this being a multiple of productivity: a
/// rainforest is the most productive land on Earth and carries *less*
/// large-herbivore biomass than a savanna half as productive, because
/// forest production is locked up in wood forty metres overhead. What
/// feeds herbivores is the share at ground level.
///
/// Real large-herbivore standing biomass, kg/km²: savanna and grassland
/// 3,000-7,000 *(Serengeti ~5,000)*, temperate grassland 1,000-2,000,
/// temperate forest 500-1,500, rainforest 200-1,000, boreal 100-400,
/// tundra 50-200, desert 5-50.
fn grazeable_share(biome: Biome) -> f32 {
    use Biome::*;
    match biome {
        Savanna | Grassland => 1.00,
        Shrubland => 0.55,
        // Lichen and dwarf willow feed reindeer, but thinly.
        Tundra => 0.35,
        Swamp => 0.45,
        Forest | Taiga => 0.30,
        // Productive, and almost none of it within reach.
        Rainforest => 0.08,
        // Sparse, and much of what grows there is not worth eating.
        Desert => 0.15,
        Beach => 0.35,
        Mountain | Snowcap => 0.20,
        Ocean | Shallows => 0.0,
    }
}

/// How much standing timber a hectare carries, in cubic metres.
///
/// Real: a mature temperate forest holds 200-400 m³/ha, boreal 100-200,
/// tropical 200-500, and open country effectively none. This is what a
/// timber trade would draw on.
fn timber_share(biome: Biome) -> f32 {
    use Biome::*;
    match biome {
        Rainforest => 1.00,
        Forest => 0.85,
        Taiga => 0.45,
        Swamp => 0.35,
        Shrubland => 0.12,
        Savanna => 0.08,
        _ => 0.01,
    }
}

/// What lives on a cell, all in real units.
pub struct Biota {
    /// Net primary productivity, g dry matter / m² / yr.
    pub npp: Field,
    /// Standing large-herbivore biomass, kg/km² — what there is to hunt
    /// or graze.
    pub game: Field,
    /// Large-carnivore biomass, kg/km².
    pub predators: Field,
    /// Standing timber, m³/ha.
    pub timber: Field,
}

/// Kilograms of herbivore per km² per unit of grazeable productivity.
///
/// Anchored on the Serengeti: ~900 g/m²/yr of production at full
/// grazeable share carries about 5,000 kg/km² of large herbivores.
const HERBIVORE_PER_NPP: f32 = 5.6;

/// **Carnivores run at one to two percent of the herbivore biomass they
/// live on** *(Serengeti: ~100 kg/km² of large carnivore against ~5,000
/// of herbivore)*. Two trophic steps is a hundredfold loss, which is why
/// predators are rare everywhere and not merely rare where it is cold.
const CARNIVORE_OF_HERBIVORE: f32 = 0.02;

/// Cubic metres of standing timber per hectare at full forest cover.
const TIMBER_AT_FULL_COVER: f32 = 380.0;

pub fn generate(
    biomes: &[Biome],
    temperature_c: &Field,
    rainfall_mm: &Field,
    elev: &Field,
    sea_level: f32,
) -> Biota {
    let (w, h) = (elev.width, elev.height);
    let mut npp = Field::new(w, h);
    let mut game = Field::new(w, h);
    let mut predators = Field::new(w, h);
    let mut timber = Field::new(w, h);

    for i in 0..elev.data.len() {
        if elev.data[i] < sea_level {
            continue; // the sea has its own productivity; not modelled yet
        }
        let p = miami_npp(temperature_c.data[i], rainfall_mm.data[i]);
        npp.data[i] = p;

        let b = biomes[i];
        let herbivore = p * grazeable_share(b) * HERBIVORE_PER_NPP;
        game.data[i] = herbivore;
        predators.data[i] = herbivore * CARNIVORE_OF_HERBIVORE;

        // Timber tracks productivity as well as forest type: a boreal
        // forest and a tropical one are both forest and are not the same
        // standing volume.
        // **Standing timber is accumulated, not annual.** A boreal
        // forest grows slowly and stands for centuries, so it carries
        // 100-200 m³/ha on a fraction of the tropics' productivity.
        // Scaling stock straight off productivity gave taiga 42 m³/ha,
        // which is scrub.
        let stocking = 0.5 + 0.5 * (p / 2200.0).min(1.0).sqrt();
        timber.data[i] = timber_share(b) * TIMBER_AT_FULL_COVER * stocking;
    }

    Biota {
        npp,
        game,
        predators,
        timber,
    }
}
