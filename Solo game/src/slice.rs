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
            ran: 0.0,
            fitted: None,
        cost_factor: 1.0,
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
            ran: 0.0,
            fitted: None,
        cost_factor: 1.0,
    },
        Site {
            name: "Ashford cannery".into(),
            kind: SiteKind::Factory,
            market: ASHFORD,
            // A cannery holds weeks of tinplate: it does not spoil and the
            // works that rolls it is somebody else's country.
            stock: cap(&[
                (Flour, 300.0),
                (ProcessedFood, 200.0),
                (Steel, cannery_rate * 0.035 * 20.0),
            ]),
            capacity: cap(&[
                (Flour, 1500.0),
                (ProcessedFood, 1500.0),
                (Steel, cannery_rate * 0.035 * 60.0),
            ]),
            recipe: Some(2),
            throughput: cannery_rate,
            powered: true,
            ran: 0.0,
            fitted: None,
        cost_factor: 1.0,
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
            ran: 0.0,
            fitted: None,
        cost_factor: 1.0,
    },
        Site {
            // **Two towns do not smelt their own steel.** A slice this
            // size buys plate and bar from a stockholder, which is what
            // most of the world does — and it is the same dependency the
            // fuel terminal is.
            name: "Ashford steel stockholder".into(),
            kind: SiteKind::Depot,
            market: ASHFORD,
            stock: cap(&[(Steel, cannery_rate * 0.035 * 30.0)]),
            capacity: cap(&[(Steel, cannery_rate * 0.035 * 90.0)]),
            recipe: Some(crate::econ::recipe::STEEL_IMPORTS),
            throughput: cannery_rate * 0.035 * 1.2,
            powered: true,
            ran: 0.0,
            fitted: None,
        cost_factor: 1.0,
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
            ran: 0.0,
            fitted: None,
        cost_factor: 1.0,
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
            ran: 0.0,
            fitted: None,
        cost_factor: 1.0,
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
            ran: 0.0,
            fitted: None,
        cost_factor: 1.0,
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
        km: 173.0,
        surface: crate::econ::Surface::Road,
        crossing: crate::econ::Crossing::Level,
        snowed_in: false,
        capacity: 150.0,
        moved: None,
        open: true,
    }];

    // Peak load is dominated by the cannery.
    let peak = cannery_rate * 0.35 + mill_rate * 0.08 + farm_rate * 0.05 + 2.0;

    let markets_len = markets.len();
    let mut economy = Economy {
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
        workforce: vec![crate::labour::Workforce::default(); markets_len],
        government: None,
        logistics: None,
        treasury: crate::money::Treasury::new(),
        told_the_day: None,
            opening: None,
            shipments: crate::registry::Registry::new(),
            routing: crate::quote::Routing::default(),
            import_duty: Default::default(),
            arrivals: Vec::new(),
        staff_today: Vec::new(),
        payroll_met: Vec::new(),
        state_afford: 1.0,
        building_stock: Vec::new(),
        building_condition: Vec::new(),
        services: None,
    };
    // A hand-built slice needs money in it like anywhere else.
    economy.issue_currency();
    // **Before anybody asks what a haul costs.** The table is rebuilt at
    // the top of every day, but a freshly built world is read before it
    // has had one.
    economy.resurvey();
    economy
}

// =====================================================================
// A world with no reason for anything to differ
// =====================================================================

/// **Three towns identical in every respect.**
///
/// A diagnostic fixture, and the point of it is what it makes *provable*.
/// In a real country a spread in days of cover between two towns can be
/// perfectly correct — a mountain port 1,200 km from the grain belt is
/// supposed to hold less and pay more — so "cover is level" is a claim
/// about the *world* and not about the model, and asserting it in an
/// asymmetric fixture tests something nobody has established.
///
/// Here there is nothing to be different about. Same population, same
/// works at the same rates, same opening stock, same road cost and
/// distance to each of the other two, same season, one nation. Under
/// rotation of the three towns the world maps onto itself, so **any
/// mechanism that respects the economics must produce the same answer for
/// each of them.** A spread is then not a signal, it is a bug — an
/// ordering dependence, a first-come-first-served scan, or a feedback loop
/// closing inside a day.
///
/// Three rather than two because two markets can hide an asymmetry: with
/// one road, `a -> b` and `b -> a` are the same edge, and a rule that
/// favours whichever end is scanned first cannot show itself. With three
/// there is a genuine choice of supplier and a genuine choice of customer.
pub fn symmetric(doctrine: Doctrine) -> Economy {
    use Commodity::*;

    const TOWNS: usize = 3;
    const POP: f64 = 30_000.0;
    /// Far enough that a load sleeps on the road, so the transit layer is
    /// exercised rather than short-circuited.
    const KM: f64 = 800.0;

    let food_per_day = POP * ProcessedFood.per_capita_annual() / 365.0;
    let cannery_rate = food_per_day * 1.15;
    let mill_rate = cannery_rate * 0.9;
    let farm_rate = mill_rate * 1.35 * 1.12;
    let goods_per_day = POP * RetailGoods.per_capita_annual() / 365.0;
    let tinplate = cannery_rate * 0.035;

    let mut sites = Vec::new();
    for m in 0..TOWNS {
        let town = ["Alpha", "Beta", "Gamma"][m];
        sites.push(Site {
            name: format!("{town} farm"),
            kind: SiteKind::Farm,
            market: m,
            stock: cap(&[(Grain, farm_rate * 120.0)]),
            capacity: cap(&[(Grain, farm_rate * 480.0)]),
            recipe: Some(0),
            throughput: farm_rate,
            powered: true,
            ran: 0.0,
            fitted: None,
            cost_factor: 1.0,
        });
        sites.push(Site {
            name: format!("{town} mill"),
            kind: SiteKind::Mill,
            market: m,
            stock: cap(&[(Grain, 400.0), (Flour, 150.0)]),
            capacity: cap(&[(Grain, 2000.0), (Flour, 1500.0)]),
            recipe: Some(1),
            throughput: mill_rate,
            powered: true,
            ran: 0.0,
            fitted: None,
            cost_factor: 1.0,
        });
        sites.push(Site {
            name: format!("{town} cannery"),
            kind: SiteKind::Factory,
            market: m,
            stock: cap(&[
                (Flour, 300.0),
                (ProcessedFood, 200.0),
                (Steel, tinplate * 20.0),
            ]),
            capacity: cap(&[
                (Flour, 1500.0),
                (ProcessedFood, 1500.0),
                (Steel, tinplate * 60.0),
            ]),
            recipe: Some(2),
            throughput: cannery_rate,
            powered: true,
            ran: 0.0,
            fitted: None,
            cost_factor: 1.0,
        });
        sites.push(Site {
            name: format!("{town} power station"),
            kind: SiteKind::PowerPlant,
            market: m,
            stock: cap(&[(Coal, 20_000.0)]),
            capacity: cap(&[(Coal, 40_000.0), (Electricity, 1e9)]),
            recipe: Some(3),
            throughput: 1e9,
            powered: true,
            ran: 0.0,
            fitted: None,
            cost_factor: 1.0,
        });
        sites.push(Site {
            name: format!("{town} steel stockholder"),
            kind: SiteKind::Depot,
            market: m,
            stock: cap(&[(Steel, tinplate * 30.0)]),
            capacity: cap(&[(Steel, tinplate * 90.0)]),
            recipe: Some(crate::econ::recipe::STEEL_IMPORTS),
            throughput: tinplate * 1.2,
            powered: true,
            ran: 0.0,
            fitted: None,
            cost_factor: 1.0,
        });
        sites.push(Site {
            name: format!("{town} depot"),
            kind: SiteKind::Depot,
            market: m,
            stock: cap(&[(RetailGoods, 400.0)]),
            capacity: cap(&[(RetailGoods, 4000.0)]),
            recipe: Some(4),
            throughput: goods_per_day * 1.1,
            powered: true,
            ran: 0.0,
            fitted: None,
            cost_factor: 1.0,
        });
        sites.push(Site {
            name: format!("{town} market hall"),
            kind: SiteKind::Shop,
            market: m,
            stock: cap(&[(ProcessedFood, 150.0), (RetailGoods, 500.0)]),
            capacity: cap(&[(ProcessedFood, 900.0), (RetailGoods, 3000.0)]),
            recipe: None,
            throughput: 0.0,
            powered: true,
            ran: 0.0,
            fitted: None,
            cost_factor: 1.0,
        });
    }

    let markets: Vec<Market> = (0..TOWNS)
        .map(|m| Market::new(["Alpha", "Beta", "Gamma"][m], POP))
        .collect();

    // A triangle: every town is exactly as far from every other, so no
    // town is better placed than any other.
    let mut routes = Vec::new();
    for a in 0..TOWNS {
        for b in (a + 1)..TOWNS {
            routes.push(Route {
                name: format!("{}–{} road", markets[a].name, markets[b].name),
                a,
                b,
                freight_cost: 45.0,
                sound_cost: 45.0,
                km: KM,
                surface: crate::econ::Surface::Road,
                crossing: crate::econ::Crossing::Level,
                snowed_in: false,
                capacity: 500.0,
                moved: None,
                open: true,
            });
        }
    }

    let peak =
        (cannery_rate * 0.35 + mill_rate * 0.08 + farm_rate * 0.05 + 2.0) * TOWNS as f64;

    let mut economy = Economy {
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
        workforce: vec![crate::labour::Workforce::default(); TOWNS],
        government: None,
        logistics: None,
        treasury: crate::money::Treasury::new(),
        told_the_day: None,
        opening: None,
        shipments: crate::registry::Registry::new(),
            routing: crate::quote::Routing::default(),
            import_duty: Default::default(),
        arrivals: Vec::new(),
        staff_today: Vec::new(),
        payroll_met: Vec::new(),
        state_afford: 1.0,
        building_stock: Vec::new(),
        building_condition: Vec::new(),
        services: None,
    };
    economy.issue_currency();
    // **Before anybody asks what a haul costs.** The table is rebuilt at
    // the top of every day, but a freshly built world is read before it
    // has had one.
    economy.resurvey();
    economy
}
