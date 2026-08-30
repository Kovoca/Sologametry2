//! Turning a piece of a generated planet into a running economy.
//!
//! This is where the two halves of the project meet. Everything up to now
//! produced a world — terrain, rivers, soil, ore, nations, cities, roads —
//! and everything in `econ` consumed a scenario typed by hand. This reads
//! the first and produces the second.
//!
//! Nothing here invents capacity. A region's farms are sized by the soil
//! its cities actually draw on, its mines exist only where the geology put
//! coal, its freight costs come from the real distances between its towns
//! and whether there is navigable water between them. If a region has no
//! coal it has no power station, and it will have to import — which is the
//! honest answer and the one that makes trade matter.

use crate::econ::{
    basket, recipe, Commodity, Doctrine, Economy, Grid, Journal, Ledger, Market, Response, Route,
    Site, SiteKind, DAYS_PER_YEAR, N_COMMODITIES,
};
use crate::geology::Geology;
use crate::network::Network;
use crate::polity::Polities;
use crate::settlement::{Kind, Settlements, NO_SETTLEMENT};
use crate::world::World;

/// Kilometres across one cell of the coarse map. Spec A1.1 puts a region
/// at 16.384 km; the generated grid is that resolution.
pub const KM_PER_CELL: f64 = 16.384;

/// Freight cost per tonne-kilometre, by mode. Sea and inland water are
/// roughly an order of magnitude cheaper than road (spec A.9), which is
/// the single fact that decides which of a region's towns are tightly
/// coupled and which can hold a price of their own.
const ROAD_COST_PER_TKM: f64 = 0.26;
const WATER_COST_PER_TKM: f64 = 0.04;

/// A region lifted out of a generated world, with the economy that its
/// geography can actually support.
pub struct Region {
    pub polity: u16,
    pub economy: Economy,
    /// Settlement index in `Settlements::list` for each market.
    pub settlement_of_market: Vec<usize>,
    /// What the extraction found, for reporting.
    pub notes: Vec<String>,
}

/// What a settlement's hinterland can supply.
struct Hinterland {
    food_capacity: f64,
}

/// What the nation as a whole has under it.
///
/// Scanned over the polity's whole territory, not over the hinterlands of
/// the few towns being modelled. A nation of nine hundred settlements
/// keeps its coalfields wherever the geology put them, and looking only at
/// the five largest cities' immediate catchments reports a country with a
/// working coal industry as having none.
struct Endowment {
    coal_cells: usize,
    ore_cells: usize,
    /// Cell of the richest coal ground, so the colliery can be sited at
    /// whichever modelled town is nearest it.
    best_coal: Option<usize>,
}

/// Deterministic settlement name from its position. Not in `settlement.rs`
/// because names are a presentation concern, and the map does not need
/// them to work.
pub fn place_name(cell: usize, seed: u64) -> String {
    const HEAD: [&str; 24] = [
        "Ash", "Bex", "Cald", "Dun", "Eller", "Fen", "Gart", "Hal", "Ing", "Kel", "Lang",
        "Mar", "Nor", "Ott", "Pen", "Quar", "Rhen", "Stan", "Thorn", "Ux", "Vale", "Wex",
        "Yar", "Zel",
    ];
    const TAIL: [&str; 16] = [
        "ford", "bury", "ton", "wich", "mouth", "dale", "cote", "hurst", "leigh", "stead",
        "wick", "combe", "field", "gate", "haven", "moor",
    ];
    let mut h = (cell as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ seed;
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    format!(
        "{}{}",
        HEAD[(h % HEAD.len() as u64) as usize],
        TAIL[((h >> 8) % TAIL.len() as u64) as usize]
    )
}

#[inline]
fn w_cells(world: &World) -> usize {
    world.width
}

fn cap(pairs: &[(Commodity, f64)]) -> [f64; N_COMMODITIES] {
    let mut b = basket();
    for &(c, v) in pairs {
        b[c as usize] = v;
    }
    b
}

/// Shortest distance between two cells on the cylinder, in kilometres.
fn distance_km(a: usize, b: usize, w: usize) -> f64 {
    let (ax, ay) = ((a % w) as f64, (a / w) as f64);
    let (bx, by) = ((b % w) as f64, (b / w) as f64);
    let mut dx = (ax - bx).abs();
    if dx > w as f64 * 0.5 {
        dx = w as f64 - dx;
    }
    let dy = ay - by;
    (dx * dx + dy * dy).sqrt() * KM_PER_CELL
}

/// What each settlement's hinterland yields. `catchment_of` already says
/// which settlement every stretch of country belongs to, so this is a
/// single pass over the map.
fn hinterlands(world: &World, set: &Settlements, g: &Geology) -> Vec<Hinterland> {
    let mut out: Vec<Hinterland> = (0..set.list.len())
        .map(|_| Hinterland {
            food_capacity: 0.0,
        })
        .collect();

    for i in 0..world.biomes.len() {
        let s = set.catchment_of[i];
        if s == NO_SETTLEMENT {
            continue;
        }
        out[s as usize].food_capacity += g.fertility.data[i] as f64;
    }
    out
}

/// Minerals under the whole of a polity's territory.
fn endowment(world: &World, pol: &Polities, polity: u16, workable: f32) -> Endowment {
    let g = &world.geology;
    let mut e = Endowment {
        coal_cells: 0,
        ore_cells: 0,
        best_coal: None,
    };
    let mut best = 0.0f32;

    for i in 0..world.biomes.len() {
        if pol.owner[i] != polity {
            continue;
        }
        if g.coal.data[i] >= workable {
            e.coal_cells += 1;
            if g.coal.data[i] > best {
                best = g.coal.data[i];
                e.best_coal = Some(i);
            }
        }
        if g.ore.data[i] >= workable {
            e.ore_cells += 1;
        }
    }
    e
}

impl Region {
    /// Build the economy of `polity`, using at most `max_markets` of its
    /// largest towns.
    ///
    /// Returns `None` if the polity holds no settlements worth modelling.
    pub fn extract(
        world: &World,
        polities: &Polities,
        settlements: &Settlements,
        network: &Network,
        polity: u16,
        max_markets: usize,
        doctrine: Doctrine,
    ) -> Option<Region> {
        const WORKABLE: f32 = 0.45;
        let g = &world.geology;
        let hinter = hinterlands(world, settlements, g);
        let endow = endowment(world, polities, polity, WORKABLE);
        let mut notes = Vec::new();

        // The polity's largest towns become its markets.
        let mut towns: Vec<usize> = (0..settlements.list.len())
            .filter(|&i| settlements.list[i].polity == polity)
            .collect();
        if towns.is_empty() {
            return None;
        }
        towns.sort_by(|&a, &b| {
            settlements.list[b]
                .population
                .cmp(&settlements.list[a].population)
                .then(a.cmp(&b))
        });
        towns.truncate(max_markets.max(1));

        let mut markets = Vec::new();
        let mut sites: Vec<Site> = Vec::new();
        let mut settlement_of_market = Vec::new();

        // Every town gets a market and a shop. Populations are the real
        // ones the settlement pass produced.
        for (m, &t) in towns.iter().enumerate() {
            let s = &settlements.list[t];
            let name = place_name(s.cell, world.seed);
            let pop = s.population as f64;

            let food_day = pop * Commodity::ProcessedFood.per_capita_annual() / 365.0;
            let goods_day = pop * Commodity::RetailGoods.per_capita_annual() / 365.0;

            markets.push(Market::new(name.clone(), pop));
            settlement_of_market.push(t);

            sites.push(Site {
                name: format!("{name} market"),
                kind: SiteKind::Shop,
                market: m,
                stock: cap(&[
                    (Commodity::ProcessedFood, food_day * 4.0),
                    (Commodity::RetailGoods, goods_day * 14.0),
                ]),
                capacity: cap(&[
                    (Commodity::ProcessedFood, food_day * 20.0),
                    (Commodity::RetailGoods, goods_day * 60.0),
                ]),
                recipe: None,
                throughput: 0.0,
                powered: true,
            });
        }

        let total_pop: f64 = markets.iter().map(|m| m.population).sum();
        let food_day = total_pop * Commodity::ProcessedFood.per_capita_annual() / 365.0;
        let goods_day = total_pop * Commodity::RetailGoods.per_capita_annual() / 365.0;

        // --- Farms, sized by the soil each town actually draws on ---
        //
        // A hinterland's fertility sum is a carrying-capacity proxy, not a
        // tonnage, so it is scaled to what the region has to eat and then
        // apportioned. A region whose land cannot feed it will come up
        // short here, and should.
        let region_fertility: f64 = towns.iter().map(|&t| hinter[t].food_capacity).sum();
        let grain_needed = food_day * 0.9 * 1.35; // cannery then mill ratios
        for (m, &t) in towns.iter().enumerate() {
            let share = if region_fertility > 0.0 {
                hinter[t].food_capacity / region_fertility
            } else {
                0.0
            };
            // Solved, not guessed. The harvest curve averages about 0.93 of
            // rated output over a year and weather another 0.97, so a farm
            // rated at bare demand delivers only ~0.90 of it. 1/0.90 is
            // 1.11, and the mill takes 1.35 t of grain per tonne of flour
            // against the 1.215 already in `grain_needed`, which together
            // put the balance point near 1.24. A couple of points above
            // that gives enough carryover to rebuild after a poor year
            // without the silos filling up and the price floored forever.
            // Sized against what is actually milled, which is set by what
            // people eat, not by the mills' rated capacity — that headroom
            // is never used, and multiplying the farms by it too left the
            // country with a permanent surplus and a floored price.
            //
            // 1/0.90 covers the average harvest curve and weather. The rest
            // is slack for the bad years: sized to the average, a country
            // that draws a poor season simply runs out, and no real farming
            // sector is planned that tightly. A nation that still cannot
            // feed itself ought to import, as it already does with fuel —
            // that is the honest fix and is not built yet.
            let rate = grain_needed * 1.25 * share;
            if rate < 0.5 {
                continue;
            }
            let name = &markets[m].name;
            sites.push(Site {
                name: format!("{name} farms"),
                kind: SiteKind::Farm,
                market: m,
                // Silo capacity is finite, and that is what makes weather
                // matter: a country able to store several years of grain
                // would never notice a bad harvest. Real cereal carryover
                // runs to a few months, so good years fill the silos and
                // stop, and poor ones draw them down where it can be felt.
                stock: cap(&[(Commodity::Grain, rate * 170.0)]),
                capacity: cap(&[(Commodity::Grain, rate * 260.0)]),
                recipe: Some(recipe::FARM),
                throughput: rate,
                powered: true,
            });
        }

        // --- Mills and canneries, one set per market, sized to that
        // market's own population ---
        //
        // Food processing sits near where the food is eaten. Putting a
        // nation's every mill and cannery in its capital would mean hauling
        // the whole population's bread hundreds of kilometres, and would
        // make that one city a single point of failure for everybody — an
        // artefact of the model rather than anything about the country.
        let total_mill = food_day * 1.12 * 0.9;
        let total_cannery = food_day * 1.12;
        for m in 0..towns.len() {
            let share = markets[m].population / total_pop.max(1.0);
            let mill_rate = total_mill * share;
            let cannery_rate = total_cannery * share;
            if cannery_rate < 0.5 {
                continue;
            }
            let name = markets[m].name.clone();
            sites.push(Site {
                name: format!("{name} mill"),
                kind: SiteKind::Mill,
                market: m,
                stock: cap(&[
                    (Commodity::Grain, mill_rate * 1.35 * 3.0),
                    (Commodity::Flour, mill_rate * 3.0),
                ]),
                capacity: cap(&[
                    (Commodity::Grain, mill_rate * 1.35 * 12.0),
                    (Commodity::Flour, mill_rate * 12.0),
                ]),
                recipe: Some(recipe::MILL),
                throughput: mill_rate,
                powered: true,
            });
            sites.push(Site {
                name: format!("{name} cannery"),
                kind: SiteKind::Factory,
                market: m,
                stock: cap(&[
                    (Commodity::Flour, cannery_rate * 0.9 * 3.0),
                    (Commodity::ProcessedFood, cannery_rate * 3.0),
                ]),
                capacity: cap(&[
                    (Commodity::Flour, cannery_rate * 0.9 * 12.0),
                    (Commodity::ProcessedFood, cannery_rate * 12.0),
                ]),
                recipe: Some(recipe::CANNERY),
                throughput: cannery_rate,
                powered: true,
            });
        }
        let mill_rate = total_mill;
        let cannery_rate = total_cannery;
        let capital_name = markets[0].name.clone();

        // --- Coal and generation, only where the geology allows ---
        //
        // Peak load has to be known before the mine can be sized, because
        // the mine exists to feed the station.
        let peak_power = cannery_rate * 0.35
            + mill_rate * 0.08
            + grain_needed * 1.12 * 0.05
            + goods_day * 0.02
            + 5.0;
        let coal_day = peak_power * 0.38; // station burns 0.38 t per MWh

        // Site the colliery at whichever modelled town is nearest the best
        // coal ground the nation holds.
        let coal_town = endow.best_coal.and_then(|coal_cell| {
            towns
                .iter()
                .enumerate()
                .min_by(|(_, &a), (_, &b)| {
                    distance_km(settlements.list[a].cell, coal_cell, w_cells(world))
                        .total_cmp(&distance_km(
                            settlements.list[b].cell,
                            coal_cell,
                            w_cells(world),
                        ))
                })
                .map(|(m, _)| (m, coal_cell))
        });

        match coal_town {
            Some((m, coal_cell)) if endow.coal_cells > 0 => {
                let name = markets[m].name.clone();
                let haul_km = distance_km(
                    settlements.list[towns[m]].cell,
                    coal_cell,
                    w_cells(world),
                );
                // Real coalfields feed mine-mouth power stations: moving
                // electricity is far cheaper than moving the coal.
                sites.push(Site {
                    name: format!("{name} colliery"),
                    kind: SiteKind::Mine,
                    market: m,
                    stock: cap(&[(Commodity::Coal, coal_day * 10.0)]),
                    capacity: cap(&[(Commodity::Coal, coal_day * 40.0)]),
                    recipe: Some(recipe::COAL_MINE),
                    throughput: coal_day * 1.1,
                    powered: true,
                });
                sites.push(Site {
                    name: format!("{name} power station"),
                    kind: SiteKind::PowerPlant,
                    market: m,
                    stock: cap(&[(Commodity::Coal, coal_day * 20.0)]),
                    capacity: cap(&[(Commodity::Coal, coal_day * 60.0), (Commodity::Electricity, 1e9)]),
                    recipe: Some(recipe::POWER_PLANT),
                    throughput: 1e9,
                    powered: true,
                });
                notes.push(format!(
                    "{} coalfield cells in the nation; nearest workings {:.0} km from {}, \
                     with a mine-mouth station on them",
                    endow.coal_cells, haul_km, name
                ));
            }
            _ => {
                // No coal of its own. It buys, at a port if it has one —
                // and thereby acquires a dependency somebody else can cut.
                let port = towns
                    .iter()
                    .position(|&t| settlements.list[t].coastal)
                    .unwrap_or(0);
                let name = markets[port].name.clone();
                sites.push(Site {
                    name: format!("{name} fuel terminal"),
                    kind: SiteKind::Mine,
                    market: port,
                    stock: cap(&[(Commodity::Coal, coal_day * 10.0)]),
                    capacity: cap(&[(Commodity::Coal, coal_day * 40.0)]),
                    recipe: Some(recipe::FUEL_IMPORTS),
                    throughput: coal_day * 1.1,
                    powered: true,
                });
                sites.push(Site {
                    name: format!("{name} power station"),
                    kind: SiteKind::PowerPlant,
                    market: port,
                    stock: cap(&[(Commodity::Coal, coal_day * 20.0)]),
                    capacity: cap(&[
                        (Commodity::Coal, coal_day * 60.0),
                        (Commodity::Electricity, 1e9),
                    ]),
                    recipe: Some(recipe::POWER_PLANT),
                    throughput: 1e9,
                    powered: true,
                });
                notes.push(format!(
                    "no workable coal in this nation — every tonne it burns is landed at {} \
                     {}. Cut that and the lights go out with nothing to mine instead.",
                    name,
                    if settlements.list[towns[port]].coastal {
                        "by sea"
                    } else {
                        "overland, from a landlocked country"
                    }
                ));
            }
        }
        if endow.ore_cells > 0 {
            notes.push(format!(
                "{} ore cells held — nothing in the economy uses metal yet",
                endow.ore_cells
            ));
        }

        // --- Depot for goods from outside the region ---
        sites.push(Site {
            name: format!("{capital_name} depot"),
            kind: SiteKind::Depot,
            market: 0,
            stock: cap(&[(Commodity::RetailGoods, goods_day * 10.0)]),
            capacity: cap(&[(Commodity::RetailGoods, goods_day * 40.0)]),
            recipe: Some(recipe::DEPOT),
            throughput: goods_day * 1.1,
            powered: true,
        });

        // --- Routes: every town connected to the largest, at a freight
        // cost taken from the real distance and whether there is water ---
        let w = world.width;
        let mut routes = Vec::new();
        for m in 1..towns.len() {
            let (a, b) = (settlements.list[towns[0]].cell, settlements.list[towns[m]].cell);
            let km = distance_km(a, b, w);

            // Two coastal or two river towns move goods by water, which is
            // an order of magnitude cheaper and is why such towns trade
            // freely while inland ones do not.
            let by_water = (settlements.list[towns[0]].coastal
                && settlements.list[towns[m]].coastal)
                || (network.navigable[a] && network.navigable[b]);
            let rate = if by_water { WATER_COST_PER_TKM } else { ROAD_COST_PER_TKM };

            routes.push(Route {
                name: format!(
                    "{} to {} ({:.0} km by {})",
                    markets[0].name,
                    markets[m].name,
                    km,
                    if by_water { "water" } else { "road" }
                ),
                a: 0,
                b: m,
                freight_cost: km * rate,
                capacity: food_day * 2.0,
                open: true,
            });
        }

        // The farming year runs six months out of step below the equator.
        let capital_cell = settlements.list[towns[0]].cell;
        let southern = capital_cell / world.width > world.height / 2;

        let mut economy = Economy {
            ledger: Ledger::new(sites),
            journal: Journal::new(),
            markets,
            routes,
            grid: Grid::for_doctrine(doctrine, peak_power),
            response: Response::for_doctrine(doctrine),
            southern,
            harvest_quality: 1.0,
            weather_seed: world.seed ^ (polity as u64).wrapping_mul(0x517C_C1B7_2722_0A95),
            unserved_power: 0.0,
            unmet_demand: basket(),
        };
        // Start mid-harvest rather than in the depths of winter, so a
        // short run is not looking at an unrepresentative slice of the
        // year.
        economy.ledger.day = (DAYS_PER_YEAR as f64 * 0.62) as u64;

        let held = polities.list[polity as usize].cells;
        notes.push(format!(
            "polity holds {held} cells and {} settlements; modelling its {} largest",
            (0..settlements.list.len())
                .filter(|&i| settlements.list[i].polity == polity)
                .count(),
            towns.len()
        ));

        Some(Region {
            polity,
            economy,
            settlement_of_market,
            notes,
        })
    }

    /// Total people fed by the modelled markets.
    pub fn population(&self) -> f64 {
        self.economy.markets.iter().map(|m| m.population).sum()
    }

    pub fn has_generation(&self) -> bool {
        self.economy
            .ledger
            .sites
            .iter()
            .any(|s| s.kind == SiteKind::PowerPlant)
    }
}

/// Kind of a settlement, for reporting.
pub fn settlement_kind(set: &Settlements, idx: usize) -> Kind {
    set.list[idx].kind
}
