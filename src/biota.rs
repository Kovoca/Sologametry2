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
        Forest | Taiga => 0.22,
        // Productive, and almost none of it within reach — and what is
        // within reach in a closed forest is largely not worth eating.
        Rainforest => 0.04,
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
    /// **Growing-season water a crop actually gets**, in mm.
    pub crop_water: Field,
    /// Mean temperature over the growing months, °C.
    pub season_c: Field,
    /// How well drained the root zone is in its worst month, 0 to 1.
    pub aeration: Field,
    /// **What a farmer here would actually plant**, and what it yields in
    /// tonnes of grain-equivalent a hectare.
    pub crop: Vec<Crop>,
    pub crop_yield: Field,
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
pub struct Settled {
    /// Standing plant biomass at its yearly peak and trough, kg/km².
    pub plant_summer: f32,
    pub plant_winter: f32,
    /// Standing large-herbivore and large-carnivore biomass, kg/km².
    pub herbivore: f32,
    pub predator: f32,
    /// **Water a crop actually gets through the growing season**, in mm.
    /// This is evapotranspiration that really happened, not rainfall —
    /// which is what a yield is built on.
    pub crop_water_mm: f32,
    /// Mean temperature over the growing months, °C — which decides what
    /// will grow, not merely how much.
    pub season_c: f32,
    /// **How well drained the root zone is in its worst month**, 0 to 1.
    /// Kept apart from the water figure because whether it matters is a
    /// property of the crop: rice is grown in standing water.
    pub aeration: f32,
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

/// **Available water capacity, millimetres per metre of soil.**
///
/// Real: sand holds ~100 mm/m, loam 150-200, clay a lot but gives much of
/// it up grudgingly. 150 is the honest single figure until substrate
/// exists to distinguish them.
const WATER_HELD_PER_METRE_MM: f32 = 150.0;

/// **How much of the soil roots actually reach.** Most grasses and crops
/// work the top 0.5-1.5 m; water below the root zone is present and
/// inaccessible, which is a real distinction and not a rounding.
const ROOTABLE_M: f32 = 1.2;

/// **Degree-day snowmelt**, mm per °C per day. Real snowpack melts at
/// 2-6 mm/°C/day.
const MELT_MM_PER_DEGREE_DAY: f32 = 3.5;

/// **How far water climbs out of the water table into the root zone.**
///
/// Capillary rise is why a shallow water table is a *resource* and not
/// only a hazard: it feeds a crop from below through a dry season, which
/// is what makes a floodplain or an oasis productive in country that has
/// no business growing anything. Real heights depend on texture — sand
/// 0.3-1 m, loam 1-2, clay 2-4 — and 1.5 m is the honest single figure
/// until substrate distinguishes them.
const CAPILLARY_RISE_M: f32 = 1.5;

/// **The water table moves through the year**, falling as the dry season
/// draws it down and recovering on recharge. Real seasonal swings: half a
/// metre to two in shallow alluvium, one to five in temperate aquifers,
/// and five to fifteen between pre- and post-monsoon in a strongly
/// seasonal climate.
///
/// **And it lags the rain by a month or two**, because water has to work
/// its way down. That lag is exactly why a floodplain still has water
/// under it well into a dry season.
const WATER_TABLE_LAG_MONTHS: f32 = 1.5;

/// Run one cell to equilibrium. Public so the calibration tests can hold
/// the climate fixed and vary one thing at a time, which is the only way
/// to see what soil depth is buying: in the world at large, deep soil
/// correlates with floodplains and the comparison confounds.
pub fn settle(
    forage_per_year_kg: f32,
    mean_c: f32,
    season_range_c: f32,
    climatic_moisture: f32,
    soil_depth_m: f32,
    rain_mm_per_year: f32,
    summer_share: f32,
    water_table_m: f32,
) -> Settled {
    // **Soil depth turns a climate index into water a plant can drink.**
    // A deep soil banks the spring and pays it out through summer; a thin
    // one is dry a fortnight after rain, on identical rainfall.
    // How far the table swings between its wettest and driest month. A
    // climate that delivers its rain in one season moves it a long way;
    // an equable one barely moves it at all.
    let concentration = ((summer_share - 0.5).abs() * 2.0).clamp(0.0, 1.0);
    let swing_m = (0.5 + 5.0 * concentration).min(water_table_m * 0.8);

    // **Roots need air as much as water**, and what kills them is the
    // *wettest* month, not the average. Where the table stands inside the
    // root zone the soil is waterlogged and roots drown in it, which is
    // why field drainage is installed to hold it below about a metre.
    // Rice is the exception and is not modelled.
    let wettest_table_m = (water_table_m - swing_m * 0.5).max(0.0);
    let aerated_m = (wettest_table_m - 0.3).max(0.0);
    let rootable = soil_depth_m.min(ROOTABLE_M).min(aerated_m);
    let capacity_mm = rootable * WATER_HELD_PER_METRE_MM;
    let mut storage_mm = capacity_mm * 0.5;
    let mut snow_mm = 0.0f32;

    // Thornthwaite's annual heat index, which sets the shape of the
    // monthly PET curve. Months below freezing contribute nothing.
    let month_temp = |m: i32| {
        let phase =
            ((m as f32 / 12.0) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2).sin();
        mean_c + phase * season_range_c * 0.5
    };
    let heat_index: f32 = (0..12)
        .map(|m| {
            let t = month_temp(m);
            if t > 0.0 {
                (t / 5.0).powf(1.514)
            } else {
                0.0
            }
        })
        .sum();
    let a = 6.75e-7 * heat_index.powi(3) - 7.71e-5 * heat_index.powi(2)
        + 1.792e-2 * heat_index
        + 0.49239;
    // Standing crop a cell can hold: roughly half a year's production for
    // open country, which is what a grassland carries at peak.
    let capacity = forage_per_year_kg * 0.5;
    let mut plant = capacity * 0.5;
    let mut herbivore = 0.0f32;
    let mut predator = 0.0f32;
    let (mut peak, mut trough) = (0.0f32, f32::MAX);
    let mut standing_mean = capacity * 0.5;
    let mut month_et = [0.0f32; 12];
    let mut season_c_sum = 0.0f32;
    let mut season_months = 0.0f32;
    let mut worst_aeration = 1.0f32;

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

        // --- The month's water balance ---
        //
        // **Precipitation below freezing is snow**, and it feeds nothing
        // until it thaws. Without a snow store a boreal cell was watering
        // its plants in January and dry in June, which is backwards.
        // **Rain arrives in a season.** Spread evenly there is never a
        // surplus big enough to fill a deep soil, so depth buys nothing —
        // which is exactly what the first version showed.
        let wet_half = phase > 0.0; // warm half of the year
        let month_rain = rain_mm_per_year / 6.0
            * if wet_half { summer_share } else { 1.0 - summer_share };
        if month_c <= 0.0 {
            snow_mm += month_rain;
        }
        let melt = if month_c > 0.0 {
            (month_c * MELT_MM_PER_DEGREE_DAY * 30.0).min(snow_mm)
        } else {
            0.0
        };
        snow_mm -= melt;
        let arriving = melt + if month_c > 0.0 { month_rain } else { 0.0 };

        // Thornthwaite monthly PET, in mm.
        let pet = if month_c > 0.0 && heat_index > 0.0 {
            16.0 * (10.0 * month_c / heat_index).powf(a)
        } else {
            0.0
        };

        // --- Where the water table is standing this month ---
        //
        // It follows the rain, a month or two behind, because water has to
        // work its way down.
        let lag = ((m as f32 - WATER_TABLE_LAG_MONTHS) / 12.0) * std::f32::consts::TAU
            - std::f32::consts::FRAC_PI_2;
        let wet_season_phase = if summer_share >= 0.5 { lag.sin() } else { -lag.sin() };
        let table_m = (water_table_m - swing_m * 0.5 * wet_season_phase).max(0.0);

        // **Groundwater feeds the crop when it is close enough to reach.**
        // Capillary rise carries water up out of the table into the root
        // zone, which is the whole reason a floodplain in dry country
        // grows anything at all — and it was doing nothing here, so the
        // table could only ever be a hazard.
        let reach = rootable + CAPILLARY_RISE_M;
        let from_below = if table_m < reach {
            // Full contribution when the table is at the root zone,
            // tailing off to nothing at the top of the capillary fringe.
            let closeness = ((reach - table_m) / CAPILLARY_RISE_M).clamp(0.0, 1.0);
            pet * closeness * 0.8
        } else {
            0.0
        };

        // **A drowned crop does not grow, however much water it has.**
        // Reducing the *storage* was not enough on its own: capillary
        // supply more than made up for it, so a table 30 cm down came out
        // as the best land on the map when it is in fact a marsh. What
        // waterlogging does is suffocate roots — real yield losses run
        // 20-50% from a few days of it — and field drainage exists to hold
        // the table around a metre down for exactly this reason.
        let aeration = ((table_m - 0.25) / 0.75).clamp(0.0, 1.0);

        let available = storage_mm + arriving + from_below;
        let actual_et = pet.min(available);
        storage_mm = (available - actual_et - from_below).clamp(0.0, capacity_mm);

        // **How well watered the plants were this month**: what they got
        // against what they wanted. A saturated soil scores 1 and a soil
        // that could not meet demand scores the shortfall.
        let wet = if pet > 0.0 {
            (actual_et / pet).clamp(0.0, 1.0)
        } else {
            (climatic_moisture / 0.65).clamp(0.0, 1.0)
        };
        let growing = warm * wet * aeration;

        let grazed = (herbivore * HERBIVORE_INTAKE_PER_YEAR / 12.0).min(plant * 0.5);
        plant += PLANT_REGROWTH_PER_MONTH * growing * (capacity - plant) - grazed;
        // **Standing crop cures and falls whether or not it is eaten**,
        // and how fast depends on how badly the month went. Real
        // grassland loses 60-80% of its peak by the end of a dry or cold
        // season; a flat 6% a month lost 22% over four dry months, which
        // made deep soil and thin soil look alike because neither had
        // anything to lose.
        plant -= plant * (0.03 + 0.22 * (1.0 - growing));
        plant = plant.max(capacity * 0.02);

        if month >= 1188 {
            peak = peak.max(plant);
            trough = trough.min(plant);
            // **What a crop drinks over its season** — its season, not
            // the year. Summing every month above 5 °C counted a whole
            // year of evapotranspiration by natural vegetation, which in
            // the tropics is twelve months, and pinned a tenth of the
            // planet at the theoretical maximum yield. A cereal occupies
            // the ground for 120-180 days and uses 350-650 mm in that
            // time; the best five months of the year is that season.
            if month_c > 5.0 {
                // **Unpenalised**: whether waterlogging matters depends
                // on the crop, and rice does not care. The aeration
                // factor travels separately so the crop can decide.
                month_et[m as usize] = actual_et;
                season_c_sum += month_c;
                season_months += 1.0;
                worst_aeration = worst_aeration.min(aeration);
            }
        }

        // **What the grass that actually grew will carry**, not what the
        // climate could have produced. Reading the potential meant a herd
        // was the same size on a hand's depth of soil as on a metre of
        // it, which is the one thing soil depth is supposed to change.
        // A twelve-month running mean, so the herd is sized on the year
        // and not on this month's grass.
        standing_mean += (plant - standing_mean) / 12.0;
        let carries = standing_mean * HERD_SHARE_OF_STANDING_CROP;
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

    // The five best months of the year: one crop's season.
    month_et.sort_by(|a, b| b.total_cmp(a));
    let crop_water: f32 = month_et[..5].iter().sum();

    Settled {
        plant_summer: peak,
        plant_winter: trough,
        herbivore,
        predator,
        crop_water_mm: crop_water,
        season_c: if season_months > 0.0 {
            season_c_sum / season_months
        } else {
            mean_c
        },
        aeration: worst_aeration,
    }
}

/// **Herbivore biomass as a share of the standing crop it lives on.**
///
/// Anchored on the Serengeti: ~2,000 kg/ha of standing grass carrying
/// ~5,000 kg/km² of large herbivores, which is about 2.5%.
///
/// Measured against *standing crop* rather than annual regrowth, because
/// regrowth in this model is gap-closing: a cell that loses less to the
/// dry season also regrows less, so reading the herd off production made
/// deep soil carry **fewer** animals than thin. What a herd eats is the
/// grass that is standing there.
const HERD_SHARE_OF_STANDING_CROP: f32 = 0.025;

/// What a herd yields in a year as a share of its standing mass — calves
/// and casualties. Real large-ungulate recruitment is 20-30%.
const HERD_YIELD_PER_YEAR: f32 = 0.22;

pub fn generate(
    biomes: &[Biome],
    temperature_c: &Field,
    rainfall_mm: &Field,
    seasonality: &Field,
    climatic_moisture: &Field,
    soil_depth: &Field,
    rain_season: &Field,
    water_table_m: &Field,
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
    let mut crop_water = Field::new(w, h);
    let mut season_c = Field::new(w, h);
    let mut aeration = Field::new(w, h);
    let mut crop = vec![Crop::Wheat; w * h];
    let mut crop_yield = Field::new(w, h);

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
            climatic_moisture.data[i],
            soil_depth.data[i],
            rainfall_mm.data[i],
            rain_season.data[i],
            water_table_m.data[i],
        );
        game.data[i] = s.herbivore;
        predators.data[i] = s.predator;
        standing_summer.data[i] = s.plant_summer;
        standing_winter.data[i] = s.plant_winter;
        crop_water.data[i] = s.crop_water_mm;
        season_c.data[i] = s.season_c;
        aeration.data[i] = s.aeration;
        let (c, y) = best_crop(s.season_c, s.crop_water_mm, s.aeration);
        crop[i] = c;
        crop_yield.data[i] = y;

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
        crop_water,
        season_c,
        aeration,
        crop,
        crop_yield,
    }
}

/// **People grow what grows.**
///
/// Assuming one crop everywhere is what made a waterlogged floodplain
/// yield nothing — when floodplain under rice is the most productive
/// farmland on Earth and feeds billions. A place too dry for wheat grows
/// sorghum; too cold, barley; too hot and wet, rice. The land is not
/// unproductive, it is *differently* productive, and pretending otherwise
/// starves people who in reality eat perfectly well.
///
/// All figures real: season temperature window, water-use efficiency in
/// kg of grain per hectare per millimetre, the bare-soil evaporation lost
/// before the crop gets any, and a rainfed ceiling.
///
/// **C4 crops — maize, sorghum, millet — convert water half again as
/// efficiently as wheat**, which is exactly why they hold the hot dry
/// parts of the world.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Crop {
    /// Cold and short-season: 5-20 °C, and it will take 250 mm.
    Barley,
    /// The temperate staple.
    Wheat,
    /// C4, hot, thirsty, and the highest rainfed yield there is.
    Maize,
    /// C4, hot and dry — grows on rain that would fail wheat outright.
    Sorghum,
    /// **Grown in standing water**, so waterlogging is not a hazard but
    /// the method.
    Rice,
    /// Cool, short season, and enormous yields by dry matter.
    Potato,
}

struct CropNeeds {
    /// Season mean temperature the crop will take at all, and the band it
    /// is happiest in.
    t_min: f32,
    t_opt_lo: f32,
    t_opt_hi: f32,
    t_max: f32,
    /// kg of grain per hectare per millimetre of season water.
    wue: f32,
    /// Lost to bare-soil evaporation before the crop gets any.
    loss_mm: f32,
    /// Best a rainfed crop does.
    ceiling: f32,
    /// Whether standing water in the root zone is a problem.
    drowns: bool,
}

impl Crop {
    pub const ALL: [Crop; 6] = [
        Crop::Barley,
        Crop::Wheat,
        Crop::Maize,
        Crop::Sorghum,
        Crop::Rice,
        Crop::Potato,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Crop::Barley => "barley",
            Crop::Wheat => "wheat",
            Crop::Maize => "maize",
            Crop::Sorghum => "sorghum",
            Crop::Rice => "rice",
            Crop::Potato => "potatoes",
        }
    }

    fn needs(self) -> CropNeeds {
        match self {
            // Hardier and thirstier-tolerant than wheat, and lower
            // yielding: which is why it is grown where wheat will not go
            // and not where it will.
            Crop::Barley => CropNeeds {
                t_min: 2.0, t_opt_lo: 8.0, t_opt_hi: 18.0, t_max: 26.0,
                wue: 19.0, loss_mm: 70.0, ceiling: 6.0, drowns: true,
            },
            Crop::Wheat => CropNeeds {
                t_min: 5.0, t_opt_lo: 12.0, t_opt_hi: 22.0, t_max: 30.0,
                wue: 22.0, loss_mm: 80.0, ceiling: 10.0, drowns: true,
            },
            // C4: more grain per drop, and it wants heat.
            Crop::Maize => CropNeeds {
                t_min: 12.0, t_opt_lo: 20.0, t_opt_hi: 30.0, t_max: 38.0,
                wue: 30.0, loss_mm: 90.0, ceiling: 12.0, drowns: true,
            },
            // C4 and drought-hardy: a crop off 300 mm that would leave
            // wheat with nothing.
            Crop::Sorghum => CropNeeds {
                t_min: 15.0, t_opt_lo: 22.0, t_opt_hi: 32.0, t_max: 42.0,
                wue: 26.0, loss_mm: 55.0, ceiling: 6.0, drowns: true,
            },
            // Thirsty and low-efficiency per drop, but it is grown in the
            // one place nothing else will grow at all.
            Crop::Rice => CropNeeds {
                t_min: 16.0, t_opt_lo: 22.0, t_opt_hi: 32.0, t_max: 40.0,
                wue: 14.0, loss_mm: 60.0, ceiling: 9.0, drowns: false,
            },
            // **By dry matter a potato crop is enormous** — 40 t/ha fresh
            // at ~20% dry matter — and it still is not the staple, which
            // is the thing to get right. A potato is 80% water: it will
            // not store through a year, will not survive a long haul, and
            // cannot be a strategic reserve. Rated on raw tonnage it took
            // a quarter of the planet and left wheat with none, against a
            // real cropland share of 1.4% for potatoes and 15% for wheat.
            // What is rated here is *storable, shippable* food.
            Crop::Potato => CropNeeds {
                t_min: 3.0, t_opt_lo: 10.0, t_opt_hi: 20.0, t_max: 27.0,
                wue: 20.0, loss_mm: 75.0, ceiling: 6.5, drowns: true,
            },
        }
    }

    /// What this crop yields here, in tonnes of grain-equivalent a
    /// hectare. Nil if the season is outside what it will take.
    pub fn yield_t_per_ha(self, season_c: f32, water_mm: f32, aeration: f32) -> f32 {
        let n = self.needs();
        if season_c < n.t_min || season_c > n.t_max {
            return 0.0;
        }
        // Full inside the optimum band, tailing off to the limits.
        let heat = if season_c < n.t_opt_lo {
            (season_c - n.t_min) / (n.t_opt_lo - n.t_min)
        } else if season_c > n.t_opt_hi {
            (n.t_max - season_c) / (n.t_max - n.t_opt_hi)
        } else {
            1.0
        };
        let water = if n.drowns { water_mm * aeration } else { water_mm };
        let kg = n.wue * (water - n.loss_mm).max(0.0);
        let drowning = if n.drowns { aeration } else { 1.0 };
        (kg / 1000.0).min(n.ceiling) * heat.clamp(0.0, 1.0) * drowning
    }
}

/// **What a farmer here would actually plant, and what it yields.**
pub fn best_crop(season_c: f32, water_mm: f32, aeration: f32) -> (Crop, f32) {
    let mut best = (Crop::Wheat, 0.0f32);
    for c in Crop::ALL {
        let y = c.yield_t_per_ha(season_c, water_mm, aeration);
        // Ties break by the order in ALL, so a seed stays reproducible.
        if y > best.1 {
            best = (c, y);
        }
    }
    best
}

/// **What a hectare yields, from the water the crop actually got.**
///
/// The French-Schultz relation, which is what dryland agronomy actually
/// uses: yield is water-use efficiency times growing-season water, less
/// what the soil surface evaporates before the crop can reach it.
///
/// Real figures: wheat converts about **20 kg of grain per hectare per
/// millimetre** of growing-season water, and about **110 mm** is lost to
/// bare-soil evaporation before any of it becomes grain. Which gives 1.8
/// t/ha on 200 mm, 5.8 on 400 and 8.8 on 550 — against a real spread of
/// under 1 t/ha on marginal ground, 3.5 as a world average and 8 in
/// France and the UK.
pub fn crop_yield_t_per_ha(crop_water_mm: f32) -> f32 {
    // **Modern parameters, because this is a modern world.** The classic
    // French-Schultz figures — 20 kg/ha/mm and 110 mm of loss — describe
    // dryland wheat with few inputs. Modern varieties with fertiliser
    // reach 22-25, and stubble retention and no-till cut evaporation loss
    // to 60-80 mm by keeping the surface covered.
    const KG_PER_HA_PER_MM: f32 = 22.0;
    const EVAPORATED_BEFORE_THE_CROP_MM: f32 = 80.0;
    /// Nothing rainfed beats this; irrigation is a separate question.
    const BEST_RAINFED_T_PER_HA: f32 = 10.0;
    let kg = KG_PER_HA_PER_MM * (crop_water_mm - EVAPORATED_BEFORE_THE_CROP_MM).max(0.0);
    (kg / 1000.0).min(BEST_RAINFED_T_PER_HA)
}
