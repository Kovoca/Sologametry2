//! **Where the electricity comes from, and what that makes it cost.**
//!
//! The economy had one way to make power: burn coal. Which meant a country
//! with a great river and a country with none paid the same for
//! electricity, and the fields the world generator has been producing since
//! it was written — elevation, flow accumulation, latitude, volcanism —
//! had no consumer.
//!
//! The mechanism that makes a generation mix mean anything is **merit
//! order**, and it is how real wholesale markets actually clear: dispatch
//! the cheapest marginal cost first and let the last unit you need set the
//! price for everybody. Wind and hydro cost nothing to run once built and
//! coal burns fuel every hour, so a windy night is nearly free and a still
//! cold evening is dear — from the same plant, at the same capital cost.
//!
//! **And the price is not the interesting half.** Cheap power does not
//! lower the price of steel, because a blast furnace uses coal as a
//! *reductant* and only 250 kWh of electricity a tonne. What cheap power
//! changes is **which route is worth building**: an electric arc furnace on
//! scrap, an aluminium potline, a smelter of any kind. The endowment picks
//! the industry.
//!
//! Real levelised costs *(Lazard 2024, $/MWh)*: onshore wind 27-73, utility
//! solar 29-92, gas combined cycle 45-108, geothermal 61-102, coal 69-168,
//! nuclear 142-222. Hydro barely has one — enormous to build and then
//! almost free for a century.

use crate::world::World;

// =====================================================================
// what makes it
// =====================================================================

/// **How a megawatt-hour is produced**, and the differences that matter are
/// three: what it costs to build, what it costs to run, and how much of the
/// time it runs at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Source {
    /// Burns coal. Cheap to run where there is a seam and dear where there
    /// is not, and dirty everywhere.
    Coal,
    /// Combined cycle gas. Cheap to build, quick to start, and its cost is
    /// almost entirely the fuel — which is what makes it the marginal
    /// plant nearly everywhere and therefore what sets the price.
    Gas,
    /// **Enormous to build and then almost free for a century.** A dam is
    /// the closest thing there is to a permanent endowment.
    Hydro,
    /// Free to run and it blows when it blows.
    Wind,
    /// Free to run and not at night.
    Solar,
    /// Free to run, works day and night, and needs the geology.
    Geothermal,
    /// The capital cost of a small country and then almost nothing.
    Nuclear,
}

pub const ALL_SOURCES: [Source; 7] = {
    use Source::*;
    [Coal, Gas, Hydro, Wind, Solar, Geothermal, Nuclear]
};

impl Source {
    pub fn name(self) -> &'static str {
        match self {
            Source::Coal => "coal",
            Source::Gas => "gas",
            Source::Hydro => "hydro",
            Source::Wind => "wind",
            Source::Solar => "solar",
            Source::Geothermal => "geothermal",
            Source::Nuclear => "nuclear",
        }
    }

    /// **What it costs to run for an hour**, per MWh — fuel and variable
    /// upkeep, and nothing else. This is the number merit order sorts on
    /// and it is not the number anybody quotes.
    ///
    /// The gap between this and the levelised cost is the whole of the
    /// difference between a wind farm and a gas turbine: they may cost the
    /// same over thirty years and they behave completely differently on a
    /// Tuesday.
    pub fn marginal_cost(self) -> f64 {
        match self {
            // **Nothing to buy.** The wind was free and the dam is built.
            Source::Wind | Source::Solar => 0.0,
            Source::Hydro => 5.0,
            Source::Geothermal => 8.0,
            // Fuel is a tiny share of a nuclear plant's cost, which is
            // exactly why they are run flat out for ever.
            Source::Nuclear => 12.0,
            Source::Coal => 34.0,
            // Almost all fuel, which is why gas is usually the plant that
            // sets the price.
            Source::Gas => 42.0,
        }
    }

    /// **What it costs to build**, per kilowatt of capacity. Overnight
    /// cost, real US figures.
    pub fn capital_per_kw(self) -> f64 {
        match self {
            Source::Gas => 1_100.0,
            Source::Solar => 1_200.0,
            Source::Wind => 1_600.0,
            Source::Hydro => 3_500.0,
            Source::Coal => 4_000.0,
            Source::Geothermal => 4_500.0,
            Source::Nuclear => 8_000.0,
        }
    }

    /// **What share of the year it actually runs**, which is the number
    /// that turns a nameplate into electricity. Real US fleet averages.
    ///
    /// A megawatt of solar and a megawatt of nuclear are not the same
    /// thing: one produces four times the other over a year.
    pub fn capacity_factor(self) -> f64 {
        match self {
            Source::Nuclear => 0.93,
            Source::Geothermal => 0.69,
            Source::Gas => 0.57,
            Source::Coal => 0.42,
            Source::Hydro => 0.37,
            Source::Wind => 0.34,
            Source::Solar => 0.23,
        }
    }

    /// Whether it eats a commodity every hour it runs. **The dividing
    /// line**: a country with fuel-burning generation has a bill every day
    /// and a country with hydro has one enormous debt and then nothing.
    pub fn burns_fuel(self) -> bool {
        matches!(self, Source::Coal | Source::Gas | Source::Nuclear)
    }

    /// **Whether it can be told when to run.** Which is the other dividing
    /// line, and the reason a grid cannot be all wind: somebody has to be
    /// able to meet the evening.
    pub fn dispatchable(self) -> bool {
        !matches!(self, Source::Wind | Source::Solar)
    }

    /// **The levelised cost**, which is what everybody quotes and what
    /// nobody dispatches on. Capital spread over the energy it will
    /// actually make, plus what it costs to run.
    ///
    /// Real *(Lazard 2024, $/MWh)*: wind 27-73, solar 29-92, gas 45-108,
    /// geothermal 61-102, coal 69-168, nuclear 142-222.
    pub fn levelised_cost(self) -> f64 {
        // A thirty-year life, and a capital charge of about 8% a year,
        // which is what an ordinary utility discount rate comes to.
        let hours = 8_760.0 * self.capacity_factor() * 30.0;
        let capital = self.capital_per_kw() * 1_000.0 * 1.08 * 30.0 / 30.0;
        capital / (hours * 1_000.0 / 1_000.0).max(1.0) + self.marginal_cost()
    }
}

// =====================================================================
// what the ground will support
// =====================================================================

/// **What a place could generate, if somebody built it.**
///
/// Every one of these is read off a field the world generator has produced
/// since it was written and nothing has ever asked for.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Potential {
    /// Megawatts of hydro, from head and flow.
    pub hydro_mw: f64,
    /// The wind resource, 0 to 1, where 1 is a north-sea coast.
    pub wind: f64,
    /// Sunshine, 0 to 1, where 1 is the Mojave.
    pub solar: f64,
    /// Whether the geology is young enough to be hot near the surface.
    pub geothermal: f64,
    /// Whether there is coal under it, which `geology.rs` already knows.
    pub coal: f64,
}

/// **The hydro equation**, and it is the real one: `P = ρ g Q H η`.
///
/// A hundred metres of head with a hundred cubic metres a second is 88 MW,
/// which is a serious power station. Water density 1,000, gravity 9.81, and
/// a modern Francis turbine is about 90% efficient — so the whole thing
/// collapses to `8.83 kW per cubic metre per second per metre of head`.
pub fn hydro_megawatts(flow_m3s: f64, head_m: f64) -> f64 {
    1000.0 * 9.81 * flow_m3s.max(0.0) * head_m.max(0.0) * 0.90 / 1_000_000.0
}

/// **What the wind is likely to be**, derived rather than simulated.
///
/// There is no wind-speed field in the world generator — only a prevailing
/// direction — so this is an inference from the things that really do
/// govern a wind resource, and it is recorded as an inference:
///
/// - **Latitude.** The westerlies between about 35° and 60° are the windiest
///   band on earth, and the horse latitudes near 30° are the calmest.
/// - **Exposure.** Open water upwind means nothing to slow it: real
///   offshore capacity factors run 45-55% against 34% on land.
/// - **Height.** Wind speed rises with elevation and with getting clear of
///   the surface, which is why turbines got taller rather than wider.
/// - **Roughness.** Forest and broken ground take it out again.
pub fn wind_resource(latitude_deg: f64, coastal: bool, elevation_m: f64, rough: f64) -> f64 {
    let lat = latitude_deg.abs();
    // The westerly belt, peaking around 50 degrees. **Deliberately short of
    // 1.0**, so that exposure and height have somewhere to go: an exposed
    // Atlantic coast should beat an inland site at the same latitude, and
    // clamping the base at 1 made them identical.
    //
    // And **narrow**, because the two calm bands are as real as the windy
    // one: the horse latitudes near 30 degrees are where sailing ships were
    // becalmed for weeks, and the doldrums at the equator are worse. A
    // broad parabola made 30 degrees windier than the trades, which is
    // exactly backwards.
    let belt = 0.62 * (1.0 - ((lat - 50.0) / 20.0).powi(2));
    // The trade winds: steadier, gentler, and a narrow band of their own.
    let trades = 0.34 - ((lat - 15.0) / 12.0).powi(2) * 0.30;
    let base = belt.max(trades).max(0.08);
    let exposure = if coastal { 1.35 } else { 1.0 };
    let height = 1.0 + (elevation_m / 2_000.0).clamp(0.0, 0.6);
    (base * exposure * height * (1.0 - rough.clamp(0.0, 1.0) * 0.35)).clamp(0.0, 1.0)
}

/// **How much sun**, which is latitude and cloud.
///
/// Real annual irradiation: the US Southwest gets about 2,000 kWh/m², the
/// UK and Germany about 1,000 — a factor of two, and it is why the same
/// panel is worth twice as much in Arizona.
pub fn solar_resource(latitude_deg: f64, cloudiness: f64) -> f64 {
    let lat = latitude_deg.abs();
    let clear_sky = (1.0 - (lat / 90.0).powf(1.4)).clamp(0.0, 1.0);
    (clear_sky * (1.0 - cloudiness.clamp(0.0, 1.0) * 0.55)).clamp(0.0, 1.0)
}

/// Read a cell's potential off the world.
pub fn potential_at(world: &World, x: usize, y: usize) -> Potential {
    let i = y * world.width + x;
    let lat = latitude_of(world, y);
    let elev_m = world.elevation.data[i] as f64 * crate::world::MAX_LAND_M as f64;
    let coastal = near_the_sea(world, x, y);

    // **Hydro wants head and flow, and both are already generated.** Flow
    // accumulation is a count of upstream cells; a cell draining a large
    // basin in wet country carries a real river.
    let flow = world.flow_accum.data[i] as f64;
    let rain = world.rainfall.data[i] as f64;
    // Real: a catchment of a few thousand square kilometres in temperate
    // country carries tens of cubic metres a second.
    let m3s = flow * rain * 0.9;
    let head = local_relief(world, x, y);
    let hydro_mw = hydro_megawatts(m3s, head);

    // Cloud stands in for rainfall, which is what the world has.
    let cloud = (rain / 0.25).clamp(0.0, 1.0);
    let rough = forestedness(world, i);

    Potential {
        hydro_mw,
        wind: wind_resource(lat, coastal, elev_m, rough),
        solar: solar_resource(lat, cloud),
        geothermal: geothermal_at(world, i),
        coal: world.geology.coal.data[i] as f64,
    }
}

fn latitude_of(world: &World, y: usize) -> f64 {
    let t = y as f64 / (world.height.max(1) - 1).max(1) as f64;
    (0.5 - t) * 180.0
}

fn near_the_sea(world: &World, x: usize, y: usize) -> bool {
    let r = 2i32;
    for dy in -r..=r {
        for dx in -r..=r {
            let nx = ((x as i32 + dx).rem_euclid(world.width as i32)) as usize;
            let ny = (y as i32 + dy).clamp(0, world.height as i32 - 1) as usize;
            if world.elevation.data[ny * world.width + nx] <= world.sea_level {
                return true;
            }
        }
    }
    false
}

/// The drop available to a turbine, in metres. Uses the same local-relief
/// reasoning `ground.rs` already applies: a coarse cell is smoothed at
/// 16 km, so the difference between neighbours understates what a river
/// actually falls through inside it.
fn local_relief(world: &World, x: usize, y: usize) -> f64 {
    let i = y * world.width + x;
    let here = world.elevation.data[i];
    let mut lowest = here;
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            let nx = ((x as i32 + dx).rem_euclid(world.width as i32)) as usize;
            let ny = (y as i32 + dy).clamp(0, world.height as i32 - 1) as usize;
            lowest = lowest.min(world.elevation.data[ny * world.width + nx]);
        }
    }
    let gradient = (here - lowest) as f64 * crate::world::MAX_LAND_M as f64;
    // A dam does not use the regional gradient; it uses the fall it can
    // impound, which in hill country is far more than the coarse field
    // resolves.
    gradient.max(0.0)
}

/// **Young rock is hot rock.** Real geothermal generation is almost
/// entirely on volcanic and tectonically active ground: Iceland gets 30% of
/// its electricity from it, Kenya 47%, the Philippines 15%, and the United
/// States 0.4%.
fn geothermal_at(world: &World, i: usize) -> f64 {
    use crate::geology::Rock;
    match world.geology.rock[i] {
        Rock::Igneous => (world.geology.ore.data[i] as f64 * 0.5 + 0.5).clamp(0.0, 1.0),
        Rock::Metamorphic => 0.15,
        Rock::Sedimentary => 0.02,
    }
}

fn forestedness(world: &World, i: usize) -> f64 {
    (world.rainfall.data[i] as f64 / 0.2).clamp(0.0, 1.0) * 0.8
}

// =====================================================================
// merit order
// =====================================================================

/// A generating station somebody has actually built.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plant {
    pub source: Source,
    /// Nameplate, in megawatts.
    pub mw: f64,
    /// **What it can produce right now**, as a share of nameplate. For a
    /// dam this is how much water there is; for a wind farm it is whether
    /// the wind is blowing; for a coal station it is whether it is broken.
    pub available: f64,
}

impl Plant {
    pub fn new(source: Source, mw: f64) -> Self {
        Plant { source, mw, available: 1.0 }
    }

    /// What it will actually put out if it is called on.
    pub fn offered_mw(&self) -> f64 {
        self.mw * self.available.clamp(0.0, 1.0)
    }
}

/// **What the grid did this hour.**
#[derive(Clone, Debug, PartialEq)]
pub struct Dispatch {
    /// Who ran, and how hard.
    pub running: Vec<(Source, f64)>,
    /// **What the last unit needed cost**, which is what everybody is paid
    /// and what everybody pays. The single most important number in an
    /// electricity market and the least intuitive: a wind farm with no fuel
    /// bill is paid the same as the gas turbine that happened to be last.
    pub clearing_price: f64,
    /// Demand that nobody could meet.
    pub unserved_mw: f64,
    /// What it actually cost to produce, as against what was charged. The
    /// difference is the whole of a generator's return on its capital.
    pub production_cost: f64,
}

impl Dispatch {
    pub fn served_mw(&self) -> f64 {
        self.running.iter().map(|r| r.1).sum()
    }

    /// What the customers paid altogether.
    pub fn revenue(&self) -> f64 {
        self.served_mw() * self.clearing_price
    }

    /// **The share that came from something that burns nothing.** Which is
    /// what decides whether a hard winter costs the country money or
    /// merely costs it water.
    pub fn share_from_free_fuel(&self) -> f64 {
        let total = self.served_mw();
        if total <= 0.0 {
            return 0.0;
        }
        self.running
            .iter()
            .filter(|(s, _)| !s.burns_fuel())
            .map(|r| r.1)
            .sum::<f64>()
            / total
    }
}

/// **Dispatch the cheapest first and let the last one set the price.**
///
/// This is how a real wholesale electricity market clears, and it is the
/// mechanism that makes a generation mix mean something rather than being a
/// decoration. Everything with no fuel bill runs whenever it can, because a
/// wind farm that does not run earns nothing at all; the fuel-burners fill
/// in behind; and **the last megawatt needed sets the price for every
/// megawatt sold.**
///
/// Which produces the two facts everybody finds surprising: a windy night
/// can clear at nearly nothing, and a still cold evening clears at the cost
/// of the worst plant on the system — and the wind farm is paid that too.
pub fn dispatch(plants: &[Plant], demand_mw: f64) -> Dispatch {
    let mut order: Vec<&Plant> = plants.iter().collect();
    // Cheapest to run first, and a stable tiebreak because a seed has to
    // rebuild the same world.
    order.sort_by(|a, b| {
        a.source
            .marginal_cost()
            .total_cmp(&b.source.marginal_cost())
            .then(a.source.cmp(&b.source))
            .then(b.mw.total_cmp(&a.mw))
    });

    let mut left = demand_mw.max(0.0);
    let mut running: Vec<(Source, f64)> = Vec::new();
    let mut clearing = 0.0;
    let mut cost = 0.0;
    for p in order {
        if left <= 1e-9 {
            break;
        }
        let took = p.offered_mw().min(left);
        if took <= 1e-9 {
            continue;
        }
        left -= took;
        cost += took * p.source.marginal_cost();
        // **The last one called sets the price**, so this keeps being
        // overwritten until the demand is met.
        clearing = p.source.marginal_cost();
        match running.iter_mut().find(|r| r.0 == p.source) {
            Some(r) => r.1 += took,
            None => running.push((p.source, took)),
        }
    }

    // **Nothing left to call on**, and the price is whatever the market
    // rules say a shortage costs. Real markets set an administrative cap —
    // ERCOT's was $9,000/MWh in the 2021 Texas freeze and it stayed there
    // for four days, which bankrupted several retailers.
    if left > 1e-9 {
        clearing = SHORTAGE_PRICE;
    }

    Dispatch { running, clearing_price: clearing, unserved_mw: left, production_cost: cost }
}

/// What a megawatt-hour is deemed to cost when there is not one to be had.
/// Real: ERCOT's cap was $9,000 in February 2021 and it sat there for four
/// days.
pub const SHORTAGE_PRICE: f64 = 9_000.0;

/// **What it costs to keep the lights on over a year**, which is the number
/// a country actually feels — and which depends far more on what was built
/// than on what anything is worth per hour.
pub fn annual_cost(plants: &[Plant], demand_mw: f64) -> f64 {
    // A crude load shape: demand is not flat, and the peak is what the
    // dear plant is for. Real load factors run 55-65% for a national grid.
    let shape = [1.35, 1.15, 1.0, 0.9, 0.8, 0.72];
    let hours = 8_760.0 / shape.len() as f64;
    shape
        .iter()
        .map(|f| {
            let d = dispatch(plants, demand_mw * f);
            d.production_cost * hours
        })
        .sum()
}

/// **What a nation would build, given what it has.**
///
/// Nobody chooses a generation mix from a menu: they build what the ground
/// offers and fill the gap with whatever burns. A country with a great
/// river builds the dam because it is the cheapest thing available over
/// thirty years; a country without builds coal or gas and pays for the fuel
/// for ever.
pub fn what_they_would_build(have: &Potential, demand_mw: f64, has_gas: bool) -> Vec<Plant> {
    let mut built: Vec<Plant> = Vec::new();
    let mut firm = 0.0;

    // **The dam first, always, if there is a dam to be had.** Enormous
    // capital and then a century of almost free electricity, and it is
    // dispatchable, which wind is not.
    if have.hydro_mw > 1.0 {
        let mw = have.hydro_mw.min(demand_mw * 1.4);
        built.push(Plant::new(Source::Hydro, mw));
        firm += mw * Source::Hydro.capacity_factor();
    }

    // Geothermal where the rock is young. Also firm, also nearly free, and
    // it works at night — which is what makes Iceland and Kenya unusual.
    if have.geothermal > 0.55 {
        let mw = (demand_mw * 0.35 * have.geothermal).min(demand_mw);
        built.push(Plant::new(Source::Geothermal, mw));
        firm += mw * Source::Geothermal.capacity_factor();
    }

    // Then the things that are cheap and cannot be told when to run. They
    // displace fuel rather than capacity, which is exactly why a grid
    // cannot be built out of them alone.
    if have.wind > 0.40 {
        let mw = demand_mw * 0.9 * have.wind;
        built.push(Plant::new(Source::Wind, mw));
    }
    if have.solar > 0.55 {
        let mw = demand_mw * 0.7 * have.solar;
        built.push(Plant::new(Source::Solar, mw));
    }

    // **And somebody still has to be able to meet the evening.** What fills
    // the gap is whatever fuel is to hand: coal where there is a seam, gas
    // where there is a pipeline, and coal is dearer to build and cheaper to
    // run than gas.
    let gap = (demand_mw * 1.15 - firm).max(0.0);
    if gap > 0.0 {
        if have.coal > 0.35 {
            built.push(Plant::new(Source::Coal, gap / Source::Coal.capacity_factor() * 0.5));
        }
        if has_gas || have.coal <= 0.35 {
            built.push(Plant::new(Source::Gas, gap / Source::Gas.capacity_factor() * 0.8));
        }
    }
    built
}

// =====================================================================
// and what it means for what gets made
// =====================================================================

/// **What cheap power actually changes.**
///
/// Not the price of steel — a blast furnace uses coal as a *reductant* and
/// only 250 kWh of electricity a tonne, so halving the power price barely
/// touches it. What it changes is **which route is worth building**, and
/// that is a much bigger effect: the United States runs 70% electric arc
/// because it has an enormous scrap pool, the world runs 70% blast furnace
/// because it has ore and coal, and Brazil still makes pig iron on charcoal
/// because it has forest.
///
/// | route | electricity a tonne | what it really needs |
/// |---|---|---|
/// | blast furnace + BOF | **0.25 MWh** | 1.4 t ore, 0.8 t coal as reductant |
/// | electric arc furnace | **0.45 MWh** | ~1.1 t of scrap, no coke at all |
/// | charcoal blast furnace | ~0.2 MWh | 0.7 t charcoal, and 4-7 t of wood to make it |
/// | aluminium, primary | **14 MWh** | there is no non-electric route |
/// | aluminium, remelted | **0.7 MWh** | 5% of primary |
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    BlastFurnace,
    ElectricArc,
    CharcoalIron,
    PrimaryAluminium,
    SecondaryAluminium,
}

impl Route {
    pub fn name(self) -> &'static str {
        match self {
            Route::BlastFurnace => "blast furnace",
            Route::ElectricArc => "electric arc furnace",
            Route::CharcoalIron => "charcoal blast furnace",
            Route::PrimaryAluminium => "aluminium potline",
            Route::SecondaryAluminium => "aluminium remelt",
        }
    }

    /// Megawatt-hours of electricity per tonne of output.
    pub fn mwh_a_tonne(self) -> f64 {
        match self {
            Route::BlastFurnace => 0.25,
            Route::ElectricArc => 0.45,
            Route::CharcoalIron => 0.20,
            Route::PrimaryAluminium => 14.0,
            Route::SecondaryAluminium => 0.70,
        }
    }

    /// **How much of the cost is the electricity bill**, at an ordinary
    /// power price. Real: electricity is about 40% of the cost of primary
    /// aluminium and a few per cent of blast-furnace steel — which is the
    /// entire reason one chases cheap power round the world and the other
    /// does not.
    pub fn power_share_of_cost(self, price_per_mwh: f64) -> f64 {
        let other = match self {
            Route::BlastFurnace => 420.0,
            Route::ElectricArc => 330.0,
            Route::CharcoalIron => 380.0,
            Route::PrimaryAluminium => 1_150.0,
            Route::SecondaryAluminium => 1_100.0,
        };
        let power = self.mwh_a_tonne() * price_per_mwh;
        power / (power + other)
    }

    /// What a tonne costs by this route.
    pub fn cost_a_tonne(self, price_per_mwh: f64, inputs: f64) -> f64 {
        self.mwh_a_tonne() * price_per_mwh + inputs
    }
}

/// **Which way a country would make its steel.**
///
/// The endowment decides, and it is not the power price alone: an arc
/// furnace needs a scrap pool, which only a country that has already been
/// industrial for fifty years has. That is why the US could switch and a
/// developing country cannot.
/// **And what decides it is scrap, not the power price.**
///
/// Which follows from the table above rather than contradicting it: the two
/// steel routes differ by 0.2 MWh a tonne, so even at a punishing $120/MWh
/// that is $24 against something like $460 of ore, coal, scrap and
/// conversion. The power price is a rounding error in steel and is the
/// whole game in aluminium.
///
/// What actually differs, per tonne of crude steel:
///
/// | | inputs | conversion |
/// |---|---|---|
/// | blast furnace | 1.4 t ore + 0.8 t coal, ~260 | **~210**, an integrated works is enormous |
/// | electric arc | ~1.1 t scrap, ~385 | **~70**, a mini-mill is not |
/// | charcoal | 0.7 t charcoal + ore, ~315 | ~180 |
///
/// So an arc furnace has dearer inputs and far cheaper conversion, and the
/// balance tips on **how much scrap there is to be had** — which is why the
/// United States runs 70% electric arc after a century of accumulating
/// scrap, the world runs 70% blast furnace, and a country industrialising
/// today cannot simply pick the modern route because there is nothing in it
/// to melt.
pub fn steel_route(power_per_mwh: f64, coal: f64, scrap_available: f64, forest: f64) -> Route {
    // **No scrap, no arc furnace**, whatever the power costs.
    let arc = if scrap_available < 0.25 {
        f64::INFINITY
    } else {
        Route::ElectricArc.cost_a_tonne(power_per_mwh, 455.0 + 200.0 * (1.0 - scrap_available))
    };
    let blast = if coal > 0.3 {
        Route::BlastFurnace.cost_a_tonne(power_per_mwh, 470.0 + 240.0 * (1.0 - coal))
    } else {
        f64::INFINITY
    };
    let charcoal = if forest > 0.5 {
        Route::CharcoalIron.cost_a_tonne(power_per_mwh, 495.0 + 280.0 * (1.0 - forest))
    } else {
        f64::INFINITY
    };

    let mut best = (Route::BlastFurnace, blast);
    if arc < best.1 {
        best = (Route::ElectricArc, arc);
    }
    if charcoal < best.1 {
        best = (Route::CharcoalIron, charcoal);
    }
    best.0
}

/// **Whether it is worth smelting aluminium here at all.**
///
/// Almost nowhere is. Primary aluminium is 14 MWh a tonne and about 40% of
/// its cost is the power bill, so a potline goes where a forty-year
/// contract can be had at a price nobody else gets — Iceland, Quebec,
/// Norway, the Gulf. And it goes to the *grid*, not to the dam: losses over
/// a few hundred kilometres are about 5%, so what matters is the contract
/// and not the distance.
///
/// The load is its own argument for the contract: 300-700 MW, continuously,
/// and **it cannot be off for more than about four hours or the metal
/// freezes in the pots and the plant is destroyed.**
pub fn worth_a_potline(contract_price_per_mwh: f64) -> bool {
    // Real: primary smelting stops being viable much above $40/MWh, which
    // is why so few places have it.
    contract_price_per_mwh < 40.0
}

/// What a very large continuous load can negotiate.
///
/// It is not charity: a 500 MW customer that never varies is the cheapest
/// load a generator can serve, and it will sign for forty years.
pub fn industrial_contract(clearing: f64, mw: f64, years: u32) -> f64 {
    let size = (mw / 500.0).clamp(0.0, 1.0) * 0.28;
    let length = (years as f64 / 40.0).clamp(0.0, 1.0) * 0.15;
    clearing * (1.0 - size - length).max(0.35)
}
