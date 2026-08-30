//! The vertical slice scenario.
//!
//! Two towns joined by one road. Ashford has the farm, mill, cannery and
//! the power station that runs them; Bexley across the road has a shop and
//! no industry of its own, so its food comes down that road. Everything
//! the acceptance test in `docs/state-and-economy-spec.md` Part D needs,
//! and nothing more.
//!
//! Hand-placed on purpose. This is the smallest world in which the
//! connective systems — production, distribution, prices, freight, the
//! grid — can be watched interacting, and it is meant to be replaced by
//! generated regions once they demonstrably work.
//!
//! **Sized from demand, not by eye.** Ashford and Bexley hold 68,000
//! people between them; at the real figure of 0.40 t of processed food per
//! person per year that is 74.5 t a day, and the chain behind it is sized
//! to match with a working margin.

/// `Doctrine` describes how a state runs its infrastructure, not anything
/// about this scenario, so it lives with `Response` in `econ`. Re-exported
/// here because this is where callers first meet it.
pub use crate::econ::Doctrine;

use crate::econ::{
    basket, Commodity, Economy, Grid, Journal, Ledger, Market, Response, Route, Site,
    SiteKind, N_COMMODITIES,
};

pub mod site {
    pub const FARM: usize = 0;
    pub const MILL: usize = 1;
    pub const CANNERY: usize = 2;
    pub const POWER_PLANT: usize = 3;
    pub const DEPOT: usize = 4;
    pub const ASHFORD_SHOP: usize = 5;
    pub const BEXLEY_SHOP: usize = 6;
}

pub const ASHFORD: usize = 0;
pub const BEXLEY: usize = 1;

pub const ASHFORD_POP: f64 = 42_000.0;
pub const BEXLEY_POP: f64 = 26_000.0;

fn cap(pairs: &[(Commodity, f64)]) -> [f64; N_COMMODITIES] {
    let mut b = basket();
    for &(c, v) in pairs {
        b[c as usize] = v;
    }
    b
}

pub fn build(doctrine: Doctrine) -> Economy {
    use Commodity::*;

    // Daily food demand, and the chain that has to supply it.
    let food_per_day = (ASHFORD_POP + BEXLEY_POP) * ProcessedFood.per_capita_annual() / 365.0;
    let cannery_rate = food_per_day * 1.15; // a working margin
    let mill_rate = cannery_rate * 0.9; // cannery takes 0.9 t flour per t
    let farm_rate = mill_rate * 1.35 * 1.12; // mill ratio, plus slack for lean years
    let goods_per_day = (ASHFORD_POP + BEXLEY_POP) * RetailGoods.per_capita_annual() / 365.0;

    let sites = vec![
        Site {
            name: "Ashford farm".into(),
            kind: SiteKind::Farm,
            market: ASHFORD,
            stock: cap(&[(Grain, farm_rate * 120.0)]),
            capacity: cap(&[(Grain, farm_rate * 480.0)]),
            recipe: Some(0),
            throughput: farm_rate,
            powered: true,
        },
        Site {
            name: "Ashford mill".into(),
            kind: SiteKind::Mill,
            market: ASHFORD,
            stock: cap(&[(Grain, 400.0), (Flour, 150.0)]),
            capacity: cap(&[(Grain, 2000.0), (Flour, 1500.0)]),
            recipe: Some(1),
            throughput: mill_rate,
            powered: true,
        },
        Site {
            name: "Ashford cannery".into(),
            kind: SiteKind::Factory,
            market: ASHFORD,
            stock: cap(&[(Flour, 300.0), (ProcessedFood, 200.0)]),
            capacity: cap(&[(Flour, 1500.0), (ProcessedFood, 1500.0)]),
            recipe: Some(2),
            throughput: cannery_rate,
            powered: true,
        },
        Site {
            name: "Kelling power station".into(),
            kind: SiteKind::PowerPlant,
            market: ASHFORD,
            stock: cap(&[(Coal, 20_000.0)]),
            capacity: cap(&[(Coal, 40_000.0), (Electricity, 1e9)]),
            recipe: Some(3),
            throughput: 1e9,
            powered: true,
        },
        Site {
            name: "Ashford depot".into(),
            kind: SiteKind::Depot,
            market: ASHFORD,
            stock: cap(&[(RetailGoods, 400.0)]),
            capacity: cap(&[(RetailGoods, 4000.0)]),
            recipe: Some(4),
            throughput: goods_per_day * 1.1,
            powered: true,
        },
        Site {
            name: "Ashford market hall".into(),
            kind: SiteKind::Shop,
            market: ASHFORD,
            stock: cap(&[(ProcessedFood, 180.0), (RetailGoods, 600.0)]),
            capacity: cap(&[(ProcessedFood, 900.0), (RetailGoods, 3000.0)]),
            recipe: None,
            throughput: 0.0,
            powered: true,
        },
        Site {
            name: "Bexley general store".into(),
            kind: SiteKind::Shop,
            market: BEXLEY,
            stock: cap(&[(ProcessedFood, 110.0), (RetailGoods, 380.0)]),
            capacity: cap(&[(ProcessedFood, 600.0), (RetailGoods, 2000.0)]),
            recipe: None,
            throughput: 0.0,
            powered: true,
        },
    ];

    let markets = vec![
        Market::new("Ashford", ASHFORD_POP),
        Market::new("Bexley", BEXLEY_POP),
    ];

    // One road. The freight cost on it is what bounds the price gap
    // between the two towns (spec A.7) — cutting it is an economic event,
    // not just a nuisance.
    let routes = vec![Route {
        name: "Ashford–Bexley road".into(),
        a: ASHFORD,
        b: BEXLEY,
        freight_cost: 45.0,
        sound_cost: 45.0,
        capacity: 150.0,
        open: true,
    }];

    // Peak load is dominated by the cannery.
    let peak = cannery_rate * 0.35 + mill_rate * 0.08 + farm_rate * 0.05 + 2.0;

    Economy {
        ledger: Ledger::new(sites),
        journal: Journal::new(),
        markets,
        routes,
        grid: Grid::for_doctrine(doctrine, peak),
        response: Response::for_doctrine(doctrine),
        road_condition: vec![1.0],
        maintenance_funding: vec![doctrine.maintenance_funding()],
        weather_seed: 0x5EED_C0FF_EE15_600D,
        unserved_power: 0.0,
        unmet_demand: basket(),
    }
}
