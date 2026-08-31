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
    /// **Standing plant biomass at its yearly peak and trough**, kg/km².
    /// The gap between them is the growing season made visible: a
    /// seasonal grassland halves, an equatorial one barely moves.
    pub standing_summer: Field,
    pub standing_winter: Field,
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

/// **The settling pass** — spec step 5E.
///
/// Seed the cell, then let it run until it stops moving. Plants grow
/// toward what the climate allows and are grazed; herbivores track the
/// forage with a lag; predators track the herbivores with a longer one.
/// Nothing here simulates a hunt. It only has to produce a plausible
/// standing state before civilisation and history begin.
///
/// **Plant biomass shifts and the animals do not.** Growth stops when the
/// month is too cold or too dry, so standing crop rises and falls through
/// the year — and the animals ride that out on a lag, which is what makes
/// a population stable rather than a mirror of this month's grass. Real
/// ungulate populations change by a few percent a year against a standing
/// crop that halves between summer and winter.
///
/// Real rates: a large ungulate herd can grow ~30% a year at best and
/// loses 8-15% to mortality; large carnivores manage ~15% and lose ~10%.
struct Settled {
    plant_summer: f32,
    plant_winter: f32,
    herbivore: f32,
    predator: f32,
}

/// Months of a plant's own turnover, and how fast each animal closes on
/// what its food supply will carry.
const PLANT_REGROWTH_PER_MONTH: f32 = 0.28;
const HERBIVORE_APPROACH_PER_YEAR: f32 = 0.30;
const PREDATOR_APPROACH_PER_YEAR: f32 = 0.15;
/// A large herbivore eats about 2.5% of its body mass in dry matter a
/// day; a large carnivore about 4% in meat.
const HERBIVORE_INTAKE_PER_YEAR: f32 = 0.025 * 365.0;
const CARNIVORE_INTAKE_PER_YEAR: f32 = 0.04 * 365.0;

fn settle(
    forage_per_year_kg: f32,
    mean_c: f32,
    season_range_c: f32,
    moisture: f32,
) -> Settled {
    // Standing crop a cell can hold: roughly half a year's production for
    // open country, which is what a grassland carries at peak.
    let capacity = forage_per_year_kg * 0.5;
    let mut plant = capacity * 0.5;
    let mut herbivore = 0.0f32;
    let mut predator = 0.0f32;
    let (mut peak, mut trough) = (0.0f32, f32::MAX);

    // A century of months is long past settling for populations that
    // move by tens of percent a year.
    for month in 0..1200 {
        let m = month % 12;
        // A sine year: coldest in month 0, warmest at 6.
        let phase = ((m as f32 / 12.0) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2).sin();
        let month_c = mean_c + phase * season_range_c * 0.5;
        // **Growth stops below freezing and where there is no water.**
        // That is what makes a growing season a season.
        //
        // Boreal conifers photosynthesise from about 0 °C and are near
        // full rate by ten. Requiring fourteen for full growth left taiga
        // and tundra at the bare floor — which is to say it left Canada
        // and Siberia with no vegetation at all.
        let warm = (month_c / 10.0).clamp(0.0, 1.0);
        let wet = (moisture / 0.65).clamp(0.0, 1.0);
        let growing = warm * wet;

        let grazed = (herbivore * HERBIVORE_INTAKE_PER_YEAR / 12.0).min(plant * 0.5);
        plant += PLANT_REGROWTH_PER_MONTH * growing * (capacity - plant) - grazed;
        // Standing crop dies back out of season whether or not it is eaten.
        plant -= plant * 0.06 * (1.0 - growing);
        plant = plant.max(capacity * 0.02);

        if month >= 1188 {
            peak = peak.max(plant);
            trough = trough.min(plant);
        }

        // What the year's forage will actually carry, approached slowly.
        let carries = forage_per_year_kg * UTILISATION / HERBIVORE_INTAKE_PER_YEAR;
        herbivore += (carries - herbivore) * HERBIVORE_APPROACH_PER_YEAR / 12.0;
        herbivore = herbivore.max(0.0);

        // Predators live on the herbivores' *production*, not their
        // standing mass: a herd yields roughly a fifth of itself a year in
        // calves and casualties.
        let prey_yield = herbivore * HERD_YIELD_PER_YEAR;
        let carries_c = prey_yield / CARNIVORE_INTAKE_PER_YEAR;
        predator += (carries_c - predator) * PREDATOR_APPROACH_PER_YEAR / 12.0;
        predator = predator.max(0.0);
    }

    Settled {
        plant_summer: peak,
        plant_winter: trough,
        herbivore,
        predator,
    }
}

/// **Share of a cell's yearly production that large herbivores take.**
///
/// Anchored on the Serengeti: ~900 g/m²/yr of production carrying ~5,000
/// kg/km² of large herbivores, each eating 2.5% of its mass a day, works
/// out at about 5%. Grazing studies quote 15-50% and that is not a
/// contradiction — they mean *aboveground* production, and roughly half
/// of NPP is roots while much of the rest is stem nobody can eat. Taking
/// the quoted figure against total productivity gave grassland 19,800
/// kg/km², four times what the Serengeti carries.
const UTILISATION: f32 = 0.055;

/// What a herd yields in a year as a share of its standing mass — calves
/// and casualties. Real large-ungulate recruitment is 20-30%.
const HERD_YIELD_PER_YEAR: f32 = 0.22;

pub fn generate(
    biomes: &[Biome],
    temperature_c: &Field,
    rainfall_mm: &Field,
    seasonality: &Field,
    soil_moisture: &Field,
    elev: &Field,
    sea_level: f32,
) -> Biota {
    let (w, h) = (elev.width, elev.height);
    let mut npp = Field::new(w, h);
    let mut game = Field::new(w, h);
    let mut predators = Field::new(w, h);
    let mut timber = Field::new(w, h);
    let mut standing_summer = Field::new(w, h);
    let mut standing_winter = Field::new(w, h);

    for i in 0..elev.data.len() {
        if elev.data[i] < sea_level {
            continue; // the sea has its own productivity; not modelled yet
        }
        let p = miami_npp(temperature_c.data[i], rainfall_mm.data[i]);
        npp.data[i] = p;

        let b = biomes[i];
        // A gram per square metre a year is a tonne per square kilometre.
        let forage_kg = p * grazeable_share(b) * 1000.0;
        let s = settle(
            forage_kg,
            temperature_c.data[i],
            seasonality.data[i],
            soil_moisture.data[i],
        );
        game.data[i] = s.herbivore;
        predators.data[i] = s.predator;
        standing_summer.data[i] = s.plant_summer;
        standing_winter.data[i] = s.plant_winter;

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
        standing_summer,
        standing_winter,
    }
}
