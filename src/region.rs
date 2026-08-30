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
    basket, recipe, Commodity, Crossing, Doctrine, Economy, Grid, Journal, Ledger, Market,
    Response, Route, Site, SiteKind, DAYS_PER_YEAR, N_COMMODITIES,
};
use crate::geology::Geology;
use crate::network::{Network, Road};
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

/// What it costs to move a tonne one kilometre over a given class of
/// ground. A nation's freight bill is not its map distance — it is what
/// the roads it actually built let it do.
fn rate_for(road: Road, navigable: bool) -> f64 {
    if navigable {
        return WATER_COST_PER_TKM;
    }
    match road {
        // A trunk route is what heavy freight is for. Local roads and
        // tracks cost more per tonne-kilometre; open country costs far
        // more, which is why goods follow roads even when the road is the
        // long way round.
        Road::Highway => ROAD_COST_PER_TKM * 0.75,
        Road::Road => ROAD_COST_PER_TKM,
        Road::Track => ROAD_COST_PER_TKM * 1.8,
        Road::None => ROAD_COST_PER_TKM * 5.0,
    }
}

/// Freight cost from `from` to every cell, following the road network.
///
/// Dijkstra over the map with each step priced by the road under it, so
/// the cost between two towns is what the country's actual roads charge
/// rather than the distance a crow would fly. Water is impassable to a
/// lorry; navigable rivers are cheap.
fn freight_field(world: &World, net: &Network, from: usize) -> Field4 {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    use crate::world::Biome;

    #[derive(Clone, Copy, PartialEq)]
    struct C(f64);
    impl Eq for C {}
    impl PartialOrd for C {
        fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
            Some(self.0.total_cmp(&o.0))
        }
    }
    impl Ord for C {
        fn cmp(&self, o: &Self) -> std::cmp::Ordering {
            self.0.total_cmp(&o.0)
        }
    }

    let (w, h) = (world.width, world.height);
    let mut best = vec![f64::INFINITY; w * h];
    // Kilometres along the cheapest path, which is not the same as the
    // shortest path: a longer run on a highway beats a short scramble over
    // a col, and reporting the road distance is what shows the detour.
    let mut km = vec![f64::INFINITY; w * h];
    // The high point of the cheapest path, and how cold it gets up there.
    // A route's summit is what decides whether it is a road or a pass, and
    // whether the pass survives February.
    let mut summit = vec![0.0f64; w * h];
    let mut coldest = vec![1.0f64; w * h];
    let mut heap: BinaryHeap<Reverse<(C, usize)>> = BinaryHeap::new();
    best[from] = 0.0;
    km[from] = 0.0;
    summit[from] = world.elevation.data[from] as f64;
    coldest[from] = world.temperature.data[from] as f64;
    heap.push(Reverse((C(0.0), from)));

    const D: [(i32, i32); 8] = [
        (-1, -1), (0, -1), (1, -1),
        (-1, 0), (1, 0),
        (-1, 1), (0, 1), (1, 1),
    ];

    while let Some(Reverse((C(d), i))) = heap.pop() {
        if d > best[i] {
            continue;
        }
        let (x, y) = ((i % w) as i32, (i / w) as i32);
        for (dx, dy) in D {
            let ny = y + dy;
            if ny < 0 || ny >= h as i32 {
                continue;
            }
            let nx = (x + dx).rem_euclid(w as i32) as usize;
            let j = ny as usize * w + nx;

            let navigable = net.navigable[j];
            if !navigable && matches!(world.biomes[j], Biome::Ocean | Biome::Shallows) {
                continue; // no road across water
            }
            let step_km = if dx != 0 && dy != 0 {
                KM_PER_CELL * std::f64::consts::SQRT_2
            } else {
                KM_PER_CELL
            };
            let nd = d + step_km * rate_for(net.road[j], navigable);
            if nd < best[j] {
                best[j] = nd;
                km[j] = km[i] + step_km;
                summit[j] = summit[i].max(world.elevation.data[j] as f64);
                coldest[j] = coldest[i].min(world.temperature.data[j] as f64);
                heap.push(Reverse((C(nd), j)));
            }
        }
    }
    Field4 {
        cost: best,
        km,
        summit,
        coldest,
    }
}

/// What a Dijkstra sweep learned about every destination.
struct Field4 {
    cost: Vec<f64>,
    km: Vec<f64>,
    summit: Vec<f64>,
    coldest: Vec<f64>,
}

/// Decide how a route gets over what is in its way.
///
/// This is the engineer's choice, and it turns on traffic. A col carrying
/// a few carts is left as a pass: cheap, because it follows the ground,
/// and shut every winter. A col carrying a nation's freight gets bored
/// through, at tens of millions a kilometre, precisely because a seasonal
/// hole in the trunk route is not survivable. Both exist in real mountain
/// country, side by side, for exactly this reason.
fn choose_crossing(
    summit: f64,
    sea_level: f64,
    coldest: f64,
    tonnes_per_day: f64,
    can_afford: bool,
) -> Crossing {
    // Height above the sea, as a share of the land's relief.
    let relief = ((summit - sea_level) / (1.0 - sea_level).max(1e-3)).clamp(0.0, 1.0);
    if relief < 0.55 {
        return Crossing::Level;
    }

    // Tunnelling is bought when the traffic justifies it and the state can
    // find the money. Real bored tunnel runs to tens of millions a
    // kilometre; a few kilometres of it is a national project.
    const TUNNEL_COST_PER_KM: f64 = 90.0; // millions
    let bore_km = 4.0 + 16.0 * relief;
    let capital = TUNNEL_COST_PER_KM * bore_km;

    let heavy = tonnes_per_day > 8_000.0;
    if heavy && can_afford {
        Crossing::Tunnel { capital }
    } else {
        Crossing::Pass {
            summit: relief,
            cold: coldest,
        }
    }
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

            // Hemisphere from the town's own latitude: a country can
            // straddle the equator, and a market's farming year follows
            // where it actually is.
            let southern = s.cell / world.width > world.height / 2;
            markets.push(Market::in_nation(name.clone(), pop, 0, southern));
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

        // --- Routes, following the roads the country actually built ---
        //
        // Two things were wrong with joining every town to the capital by a
        // straight line. Distance was as the crow flies, so a haul over a
        // mountain range cost the same as one across a plain; and the
        // topology was a star, so two neighbouring cities traded through a
        // capital that might be a thousand kilometres away. No nation's
        // settlements sit off its own road network.
        //
        // Costs now come from a Dijkstra over the generated network, priced
        // by the road under each step, and the towns are joined by a
        // minimum spanning tree over those costs — so the shape of the
        // economy is the shape of the roads.
        let mut routes = Vec::new();
        if towns.len() > 1 {
            let fields: Vec<Field4> = towns
                .iter()
                .map(|&t| freight_field(world, network, settlements.list[t].cell))
                .collect();
            let at = |b: usize| settlements.list[towns[b]].cell;
            let cost = |a: usize, b: usize| fields[a].cost[at(b)];
            let road_km = |a: usize, b: usize| fields[a].km[at(b)];

            // Prim's algorithm: grow one connected network from the capital
            // outward, always adding the town that is cheapest to reach
            // from what is already joined up.
            let mut joined = vec![false; towns.len()];
            joined[0] = true;
            for _ in 1..towns.len() {
                let mut best: Option<(usize, usize, f64)> = None;
                for a in 0..towns.len() {
                    if !joined[a] {
                        continue;
                    }
                    for b in 0..towns.len() {
                        if joined[b] {
                            continue;
                        }
                        let c = cost(a, b);
                        if !c.is_finite() {
                            continue; // no overland route at all
                        }
                        if best.is_none_or(|(_, _, bc)| c < bc) {
                            best = Some((a, b, c));
                        }
                    }
                }
                let Some((a, b, c)) = best else { break };
                joined[b] = true;

                let straight = distance_km(
                    settlements.list[towns[a]].cell,
                    settlements.list[towns[b]].cell,
                    world.width,
                );
                let along = road_km(a, b);
                let cell = settlements.list[towns[b]].cell;
                // Traffic on this link, roughly: the smaller end's daily
                // food demand stands for how much moves along it.
                let traffic = markets[a]
                    .daily_household_demand(Commodity::ProcessedFood)
                    .min(markets[b].daily_household_demand(Commodity::ProcessedFood));
                let crossing = choose_crossing(
                    fields[a].summit[cell],
                    world.sea_level as f64,
                    fields[a].coldest[cell],
                    traffic,
                    doctrine == Doctrine::Prudent,
                );

                // A tunnel is flat and straight; a pass is neither, and
                // heavy freight crawls over it.
                let cost = match crossing {
                    Crossing::Tunnel { .. } => c * 0.8,
                    Crossing::Pass { summit, .. } => c * (1.0 + 0.5 * summit),
                    Crossing::Level => c,
                };

                let how = match crossing {
                    Crossing::Level => String::new(),
                    Crossing::Pass { summit, .. } => {
                        format!(", over a pass at {:.0}% of relief", summit * 100.0)
                    }
                    Crossing::Tunnel { capital } => {
                        format!(", tunnelled at a cost of {capital:.0}M")
                    }
                };
                routes.push(Route {
                    name: format!(
                        "{} to {} ({:.0} km of road for a {:.0} km gap{how})",
                        markets[a].name, markets[b].name, along, straight
                    ),
                    a,
                    b,
                    freight_cost: cost,
                    sound_cost: cost,
                    crossing,
                    snowed_in: false,
                    capacity: food_day * 2.0,
                    open: true,
                });
            }

            let stranded = joined.iter().filter(|&&j| !j).count();
            if stranded > 0 {
                notes.push(format!(
                    "{stranded} of this nation's modelled towns have no overland route \
                     to the rest of it"
                ));
            }
        }

        let mut economy = Economy {
            ledger: Ledger::new(sites),
            journal: Journal::new(),
            markets,
            routes,
            grid: Grid::for_doctrine(doctrine, peak_power),
            response: Response::for_doctrine(doctrine),
            road_condition: vec![1.0],
            maintenance_funding: vec![doctrine.maintenance_funding()],
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

// ---------------------------------------------------------------------------
// A world of trading nations
// ---------------------------------------------------------------------------

/// Several nations in one economy, with lanes between them.
///
/// One ledger for the whole planet, because that is what makes
/// conservation mean anything across a border: a cargo leaving one country
/// is the same tonnes arriving in another, not a subtraction here and an
/// invention there.
pub struct Nations {
    pub economy: Economy,
    /// Polity id per nation index.
    pub polity: Vec<u16>,
    /// Market indices belonging to each nation.
    pub markets_of: Vec<Vec<usize>>,
    pub names: Vec<String>,
    pub notes: Vec<String>,
}

impl Nations {
    /// Build the `count` largest nations into a single trading world.
    pub fn build(
        world: &World,
        polities: &Polities,
        settlements: &Settlements,
        network: &Network,
        count: usize,
        markets_each: usize,
        doctrine: Doctrine,
    ) -> Nations {
        let ranked = polities.ranked();
        let mut economy: Option<Economy> = None;
        let mut polity_ids = Vec::new();
        let mut markets_of: Vec<Vec<usize>> = Vec::new();
        let mut names = Vec::new();
        let mut notes = Vec::new();
        let mut capitals: Vec<usize> = Vec::new(); // settlement cell of each capital
        let mut coastal: Vec<bool> = Vec::new();

        for (nation, &(id, _)) in ranked.iter().take(count).enumerate() {
            let Some(region) = Region::extract(
                world,
                polities,
                settlements,
                network,
                id,
                markets_each,
                doctrine,
            ) else {
                continue;
            };

            let cap_settlement = region.settlement_of_market[0];
            capitals.push(settlements.list[cap_settlement].cell);
            // A nation is maritime if its *territory* reaches the sea, not
            // if its largest cities happen to be ports. Most great cities
            // are inland; the country still ships through whatever harbour
            // it has, and judging by the capital alone leaves a planet of
            // landlocked states with no sea trade at all.
            coastal.push(has_coastline(world, polities, id));
            names.push(region.economy.markets[0].name.clone());
            polity_ids.push(id);
            for n in region.notes {
                notes.push(format!("{}: {n}", names[nation]));
            }

            match economy.as_mut() {
                None => {
                    let mut e = region.economy;
                    for m in e.markets.iter_mut() {
                        m.nation = nation as u16;
                    }
                    markets_of.push((0..e.markets.len()).collect());
                    economy = Some(e);
                }
                Some(host) => {
                    markets_of.push(absorb(host, region.economy, nation as u16));
                }
            }
        }

        let mut economy = economy.expect("no nations could be built");

        // --- Lanes between nations ---
        //
        // Capital to capital. Two coastal nations trade by sea at roughly a
        // tenth the cost per tonne-kilometre of road, which is why maritime
        // neighbours are effectively one market and landlocked ones are
        // not. Nations with no sea between them still trade overland, dearly.
        let w = world.width;
        for a in 0..capitals.len() {
            for b in (a + 1)..capitals.len() {
                let km = distance_km(capitals[a], capitals[b], w);
                let by_sea = coastal[a] && coastal[b];
                let rate = if by_sea {
                    WATER_COST_PER_TKM
                } else {
                    ROAD_COST_PER_TKM
                };
                // Beyond a certain distance an overland haul simply is not
                // done: there is no road across an ocean, and a landlocked
                // pair a hemisphere apart do not trade grain.
                if !by_sea && km > 4000.0 {
                    continue;
                }
                let (ma, mb) = (markets_of[a][0], markets_of[b][0]);
                let volume = economy.markets[ma]
                    .daily_household_demand(Commodity::ProcessedFood)
                    .min(
                        economy.markets[mb]
                            .daily_household_demand(Commodity::ProcessedFood),
                    );
                economy.routes.push(Route {
                    name: format!(
                        "{} - {} ({:.0} km by {})",
                        names[a],
                        names[b],
                        km,
                        if by_sea { "sea" } else { "land" }
                    ),
                    a: ma,
                    b: mb,
                    freight_cost: km * rate,
                    sound_cost: km * rate,
                    crossing: Crossing::Level,
                    snowed_in: false,
                    // International trade is a fraction of what a country
                    // moves internally, not a firehose.
                    capacity: volume * 0.5,
                    open: true,
                });
            }
        }

        let north = economy.markets.iter().filter(|m| !m.southern).count();
        notes.push(format!(
            "{} nations, {} markets ({} northern, {} southern), {} routes",
            names.len(),
            economy.markets.len(),
            north,
            economy.markets.len() - north,
            economy.routes.len(),
        ));

        Nations {
            economy,
            polity: polity_ids,
            markets_of,
            names,
            notes,
        }
    }

    pub fn population(&self) -> f64 {
        self.economy.markets.iter().map(|m| m.population).sum()
    }

    /// Nation index of a market.
    pub fn nation_of(&self, market: usize) -> u16 {
        self.economy.markets[market].nation
    }
}

/// Whether any of a polity's territory touches open water.
pub fn has_coastline(world: &World, pol: &Polities, polity: u16) -> bool {
    use crate::world::Biome;
    let (w, h) = (world.width, world.height);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if pol.owner[i] != polity {
                continue;
            }
            let xm = (x + w - 1) % w;
            let xp = (x + 1) % w;
            let candidates = [
                y * w + xm,
                y * w + xp,
                y.saturating_sub(1) * w + x,
                (y + 1).min(h - 1) * w + x,
            ];
            if candidates
                .iter()
                .any(|&j| matches!(world.biomes[j], Biome::Ocean | Biome::Shallows))
            {
                return true;
            }
        }
    }
    false
}

/// Fold one nation's economy into another's, renumbering as it goes.
///
/// Returns the host market indices the guest's markets became.
fn absorb(host: &mut Economy, guest: Economy, nation: u16) -> Vec<usize> {
    let market_base = host.markets.len();
    let site_base = host.ledger.sites.len();

    let mut mine = Vec::with_capacity(guest.markets.len());
    for (i, mut m) in guest.markets.into_iter().enumerate() {
        m.nation = nation;
        host.markets.push(m);
        mine.push(market_base + i);
    }
    for mut s in guest.ledger.sites {
        s.market += market_base;
        host.ledger.sites.push(s);
    }
    for mut r in guest.routes {
        r.a += market_base;
        r.b += market_base;
        host.routes.push(r);
    }
    // Each nation keeps its own grid and repair service; they are separate
    // states, and a blackout in one is not a blackout in the other.
    for l in guest.grid.lines {
        host.grid.lines.push(l);
    }
    // ...and its own roads, which it maintains or does not on its own
    // account. Indexed by nation, so `nation` doubles as the index.
    host.road_condition.push(*guest.road_condition.first().unwrap_or(&1.0));
    host.maintenance_funding
        .push(*guest.maintenance_funding.first().unwrap_or(&1.0));
    let _ = site_base;

    // The opening stock of the absorbed sites has to be counted, or the
    // conservation check will report every tonne they arrived with as
    // having appeared from nowhere.
    host.ledger.absorb_opening_stock(site_base);
    mine
}
