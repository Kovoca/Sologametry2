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
    basket, Commodity, Economy, Grid, Journal, Ledger, Market, Response, Route, Site, SiteKind,
    N_COMMODITIES,
};

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

/// **What a part of this fixture is for**, as against where it happens to
/// sit in the site vector.
///
/// `tests/shipment.rs` held its two shops as `ASHFORD_STORE = 6` and
/// `BEXLEY_STORE = 7`, which are **positions** — and they stopped meaning
/// the market hall and the general store the moment the fixture lost its
/// goods depot. Thirteen gates indexed past the end of the site list and
/// said nothing whatever about shipping.
///
/// Looking them up by name in the test was the first fix and is not
/// enough: a name can be duplicated or changed, and a `find` takes
/// whichever matched first. A role is an enum, so a typo cannot compile;
/// [`Slice::site`] asserts **exactly one** site answers to it; and the
/// table lives beside the constructor that authored the names, so renaming
/// a site moves the role with it rather than silently unhooking a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    AshfordFarm,
    AshfordMill,
    AshfordCannery,
    AshfordStockholder,
    /// Where Ashford's households shop.
    AshfordStore,
    /// Where Bexley's households shop — the only works in the town.
    BexleyStore,
    PowerStation,
    /// The viable fixture's cannery, at the port.
    SeatonCannery,
    /// The viable fixture's mill, beside the farm.
    HarwickMill,
    /// Where Harwick's households shop.
    HarwickHall,
    /// Where Seaton's households shop.
    SeatonHall,
    /// Lands the cannery's tinplate at Seaton's quay.
    SeatonStockholder,
}

impl Role {
    /// The authored name of the site holding this role.
    pub fn name(self) -> &'static str {
        match self {
            Role::AshfordFarm => "Ashford farm",
            Role::AshfordMill => "Ashford mill",
            Role::AshfordCannery => "Ashford cannery",
            Role::AshfordStockholder => "Ashford steel stockholder",
            Role::AshfordStore => "Ashford market hall",
            Role::BexleyStore => "Bexley general store",
            Role::PowerStation => "Kelling power station",
            Role::SeatonCannery => "Seaton cannery",
            Role::HarwickMill => "Harwick mill",
            Role::HarwickHall => "Harwick market hall",
            Role::SeatonHall => "Seaton market hall",
            Role::SeatonStockholder => "Seaton steel stockholder",
        }
    }
}

/// **Which site fills a role in the two-town fixture.**
///
/// Panics if none does or if more than one does. The second half is the
/// point: a duplicated name is not a near miss, it is two different things
/// answering to one handle, and a test that took the first would be
/// measuring whichever the constructor happened to push earlier.
pub fn site(e: &Economy, role: Role) -> usize {
    let want = role.name();
    let found: Vec<usize> = (0..e.ledger.sites.len())
        .filter(|&s| e.ledger.sites[s].name == want)
        .collect();
    match found.as_slice() {
        [one] => *one,
        [] => panic!("the fixture has no site filling {role:?} ({want:?})"),
        many => panic!("{} sites answer to {role:?} ({want:?})", many.len()),
    }
}

pub fn build(doctrine: Doctrine) -> Economy {
    use Commodity::*;

    // Daily food demand, and the chain that has to supply it.
    let food_per_day = (ASHFORD_POP + BEXLEY_POP) * ProcessedFood.per_capita_annual() / 365.0;
    let cannery_rate = food_per_day * 1.15; // a working margin
    let mill_rate = cannery_rate * 0.9; // cannery takes 0.9 t flour per t
    let farm_rate = mill_rate * 1.35 * 1.12; // mill ratio, plus slack for lean years

    let mut sites = vec![
        Site {
            address: None,
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
            address: None,
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
            address: None,
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
            address: None,
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
            address: None,
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
        // **No goods depot here either.** The same unfunded dependency
        // the symmetric fixture had: it landed retail goods every day and
        // had nothing to sell, and a country that imports all of its
        // manufactured goods cannot pay for them out of grain. This slice
        // is the food chain end to end, which is what its gates are about.
        Site {
            address: None,
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
            address: None,
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

    // **A power station with no colliery is living on an endowment**, and
    // this one ran out on day 253. Twenty thousand tonnes at about eighty a
    // day, and from then on the country was dark for good: electricity at
    // the 12,000 cap, the cannery stopped, and Bexley's income and spending
    // both nought — which read as a distressed town when it was the whole
    // fixture. `slice::symmetric` lost the same endowment earlier; this one
    // never did. Sized the same way, on what the station actually draws,
    // and appended so no site already here changes position.
    let load = cannery_rate * 0.35
        + mill_rate * 0.08
        + farm_rate * 0.05
        + 2.0
        + (ASHFORD_POP + BEXLEY_POP) * Electricity.per_capita_annual() / 365.0;
    let coal_per_day = load * 0.38;
    sites.push(Site {
        address: None,
        name: "Kelling colliery".into(),
        kind: SiteKind::Mine,
        market: ASHFORD,
        stock: cap(&[(Coal, coal_per_day * 30.0)]),
        capacity: cap(&[(Coal, coal_per_day * 120.0)]),
        recipe: Some(5),
        throughput: coal_per_day * 1.15,
        powered: true,
        ran: 0.0,
        fitted: None,
        cost_factor: 1.0,
    });

    // **Ashford stands on a frontier.** The country has no sea, and its
    // steel stockholder lands tinplate from abroad, so it needs a way across
    // the border — which it used to get from *no quay in reach* being read
    // as *no haul to pay* (`docs/status.md`, defect 12). Now it is said.
    // Bexley reaches it by the road, and pays the road.
    let mut ashford = Market::new("Ashford", ASHFORD_POP);
    ashford.frontier = true;
    let markets = vec![ashford, Market::new("Bexley", BEXLEY_POP)];

    // One road. The freight cost on it is what bounds the price gap
    // between the two towns (spec A.7) — cutting it is an economic event,
    // not just a nuisance.
    let routes = vec![Route {
        id: crate::quote::RouteId(1),
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
    let named_roads = routes.len() as u64 + 1;
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
        went_without: basket(),
        workforce: vec![crate::labour::Workforce::default(); markets_len],
        governments: Default::default(),
        logistics: None,
        treasury: crate::money::Treasury::new(),
        told_the_day: None,
        opening: None,
        shipments: crate::registry::Registry::new(),
        power_clearing: None,
        experiments: Default::default(),
        world_seed: 0,
        next_route_id: named_roads,
        routing: crate::quote::Routing::default(),
        hands: Default::default(),
        exported_today: Vec::new(),
        reservations: crate::quote::Reservations::new(),
        import_duty: Default::default(),
        exchange: crate::exchange::Exchange::at_par(),
        arrivals: Vec::new(),
        staff_today: Vec::new(),
        hospital_billed: Default::default(),
        obligations: Default::default(),
        bills_today: Default::default(),
        held_back_today: 0.0,
        payroll_met: Vec::new(),
        state_afford: Default::default(),
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
    // **And this slice had been in a permanent blackout too.**
    //
    // `peak` above is the works' draw added up by hand and it leaves the
    // households out altogether, so the grid carried **101.19 against a
    // call of 209.74** — 52% unserved, every day, for as long as this
    // fixture has existed. It is the same defect `symmetric` had and the
    // same fix: size it off the load it will actually see, with the 15-20%
    // reserve margin a real system plans.
    //
    // It did not invalidate what had been measured here, because it was
    // the same in every case — and this file's own note about that says
    // *it is exactly the kind of thing that invalidates the next one*,
    // which is what happened. Households were paying **6,227 a megawatt-
    // hour against a cost of 60 and a clearing price of 34.2**, because an
    // unserved grid prices at its administrative cap; the power bill was
    // 93% of everything they spent, and Bexley's whole deficit was its
    // share of it.
    let peak = economy.power_demand() * 1.20;
    economy.grid = Grid::for_doctrine(doctrine, peak);
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
    let tinplate = cannery_rate * 0.035;
    // What one town calls on the grid, and therefore what its station
    // burns: the same terms `peak` is built from below, plus the
    // households, times the 0.38 t of coal a megawatt-hour costs.
    let town_load = cannery_rate * 0.35
        + mill_rate * 0.08
        + farm_rate * 0.05
        + 2.0
        + POP * Electricity.per_capita_annual() / 365.0;
    let coal_per_day = town_load * 0.38;

    let mut sites = Vec::new();
    for m in 0..TOWNS {
        let town = ["Alpha", "Beta", "Gamma"][m];
        sites.push(Site {
            address: None,
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
            address: None,
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
            address: None,
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
        // **A power station with no colliery is living on an endowment.**
        // It opened on twenty thousand tonnes of coal with nothing to
        // refill it, so this fixture was never a steady state: the four
        // hundred days the gates run burn most of that pile. Sized on what
        // the station actually draws.
        sites.push(Site {
            address: None,
            name: format!("{town} colliery"),
            kind: SiteKind::Mine,
            market: m,
            stock: cap(&[(Coal, coal_per_day * 30.0)]),
            capacity: cap(&[(Coal, coal_per_day * 120.0)]),
            recipe: Some(5),
            throughput: coal_per_day * 1.15,
            powered: true,
            ran: 0.0,
            fitted: None,
            cost_factor: 1.0,
        });
        sites.push(Site {
            address: None,
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
            address: None,
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
        // **No goods depot.** A fixture that imports something it has no
        // industry for and no means to pay for is the same defect as a
        // power station with no colliery, and it is what emptied this
        // country: one town wants 82.2 t of retail goods a day, which
        // lands at about 75,200, and the most grain an export-agriculture
        // nation is *built* to sell -- three times its own milling need,
        // `region.rs`'s own ceiling -- earns about 27,500. A country that
        // imports all of its manufactured goods cannot pay for them by
        // exporting grain, and this one exports nothing whatever, so it
        // sent 96.4% of its money abroad inside four hundred days and
        // every gate that runs that long was reading a destitute country.
        //
        // What this fixture is for is allocation symmetry in a food
        // economy. Households still *want* retail goods and the want is
        // recorded as unmet, which is honest: there is no industry here
        // that could make them.
        sites.push(Site {
            address: None,
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

    // **Every town a frontier post**, and not one of them: each has its
    // own stockholder landing tinplate, so each needs a border crossing,
    // and giving it to one would break the symmetry this fixture exists for.
    let markets: Vec<Market> = (0..TOWNS)
        .map(|m| {
            let mut town = Market::new(["Alpha", "Beta", "Gamma"][m], POP);
            town.frontier = true;
            town
        })
        .collect();

    // A triangle: every town is exactly as far from every other, so no
    // town is better placed than any other.
    let mut routes = Vec::new();
    for a in 0..TOWNS {
        for b in (a + 1)..TOWNS {
            routes.push(Route {
                id: crate::quote::RouteId(routes.len() as u64 + 1),
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

    // Provisional: the real figure is taken off the built economy below,
    // because adding up the recipe draws by hand gets it wrong.
    let peak = (cannery_rate * 0.35 + mill_rate * 0.08 + farm_rate * 0.05 + 2.0) * TOWNS as f64;

    let named_roads = routes.len() as u64 + 1;
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
        went_without: basket(),
        workforce: vec![crate::labour::Workforce::default(); TOWNS],
        governments: Default::default(),
        logistics: None,
        treasury: crate::money::Treasury::new(),
        told_the_day: None,
        opening: None,
        shipments: crate::registry::Registry::new(),
        power_clearing: None,
        experiments: Default::default(),
        world_seed: 0,
        next_route_id: named_roads,
        routing: crate::quote::Routing::default(),
        hands: Default::default(),
        exported_today: Vec::new(),
        reservations: crate::quote::Reservations::new(),
        import_duty: Default::default(),
        exchange: crate::exchange::Exchange::at_par(),
        arrivals: Vec::new(),
        staff_today: Vec::new(),
        hospital_billed: Default::default(),
        obligations: Default::default(),
        bills_today: Default::default(),
        held_back_today: 0.0,
        payroll_met: Vec::new(),
        state_afford: Default::default(),
        building_stock: Vec::new(),
        building_condition: Vec::new(),
        services: None,
    };
    economy.issue_currency();
    // **Before anybody asks what a haul costs.** The table is rebuilt at
    // the top of every day, but a freshly built world is read before it
    // has had one.
    economy.resurvey();
    // **And size the grid against the load it will actually see.**
    //
    // Adding up the recipe draws by hand got it wrong by half: this
    // fixture ran at capacity 142 against a demand of 283, so every
    // measurement taken on it — the allocation work, the permutation
    // gates, the whole experiment matrix — was taken on a country in
    // permanent fifty per cent blackout. It did not invalidate them,
    // because it was the same in every case, and it is exactly the kind of
    // thing that invalidates the next one.
    //
    // **A real system plans a reserve margin of 15-20% above peak**, which
    // is what keeps the lights on when a unit trips or the weather turns.
    let peak = economy.power_demand() * 1.20;
    economy.grid = Grid::for_doctrine(doctrine, peak);

    economy
}

/// The viable fixture's inland works town.
pub const HARWICK: usize = 0;
/// The viable fixture's port.
pub const SEATON: usize = 1;

pub const HARWICK_POP: f64 = 40_000.0;
pub const SEATON_POP: f64 = 60_000.0;

/// **A small country that pays its way.**
///
/// `build`'s Bexley is kept as a distressed town on purpose — twenty-six
/// thousand people, one shop and no works, living on that shop's profit —
/// and a world has to be able to contain one. But a test that needs a
/// *functioning* economy cannot run on a fixture whose households live on
/// dividends: there payroll was 0.4% of household income, so whether a
/// change settled anything could not be told from whether the fixture
/// happened to be solvent.
///
/// So this one earns its living the way a country does. Its households
/// are paid wages — by the works, by the halls, and by a state and a
/// private service sector founded exactly as `region.rs` founds them over
/// a generated nation — and they spend them on what the works make.
///
/// - **Harwick** grows the grain, digs the coal and cans the food, and has
///   a market hall.
/// - **Seaton** is the port: it mills the grain and burns the coal, and
///   has an ocean quay and a market hall. The
///   cannery's tinplate is landed by a stockholder in Harwick whose import
///   parity carries the voyage, Seaton's dockers and the 120 km road up
///   from the quay — so an import pays for the road it comes up, though the
///   cargo itself still appears at the stockholder rather than riding the
///   road, which is a named gap in `region.rs` too.
pub fn viable(doctrine: Doctrine) -> Economy {
    use Commodity::*;

    let people = HARWICK_POP + SEATON_POP;
    let food_per_day = people * ProcessedFood.per_capita_annual() / 365.0;
    let cannery_rate = food_per_day * 1.15;
    // **A tenth of headroom over the cannery's whole rating**, because a
    // mill sized at exactly the draw it serves runs flat out and never
    // builds its customer a stock — a commodity settles at cost only if
    // somebody can build stock in it. A quarter was too much the other way:
    // flour sat in glut below the cost of the grain in it and the mill ran
    // out of cash.
    let mill_rate = cannery_rate * 0.9 * 1.1;
    // The farm is sized on what is milled, not on the mill's headroom, or
    // it grows more grain than anybody wants for ever.
    let farm_rate = cannery_rate * 0.9 * 1.35 * 1.12;
    let tinplate = cannery_rate * 0.035;
    let load = cannery_rate * 0.35
        + mill_rate * 0.08
        + farm_rate * 0.05
        + 2.0
        + people * Electricity.per_capita_annual() / 365.0;
    let coal_per_day = load * 0.38;

    let works = |name: &str,
                 kind: SiteKind,
                 market: usize,
                 stock: [f64; N_COMMODITIES],
                 capacity: [f64; N_COMMODITIES],
                 recipe: Option<usize>,
                 throughput: f64| Site {
        address: None,
        name: name.into(),
        kind,
        market,
        stock,
        capacity,
        recipe,
        throughput,
        powered: true,
        ran: 0.0,
        fitted: None,
        cost_factor: 1.0,
    };
    // **Each town sells the other something**, and the grain never has to
    // travel. Harwick grows it and mills it, digs the coal and burns it;
    // Seaton cans the flour and lands the tinplate at its own quay. So
    // Harwick pays Seaton for food, and Seaton pays Harwick for flour and
    // power.
    //
    // Two earlier layouts are why. With every works in Harwick, Seaton was a
    // second Bexley: its households went from 3.4M to 53,000 in four hundred
    // days. With the mill in Seaton and the farm in Harwick, grain had to
    // reach a mill whose yard holds twenty days in a town whose target is a
    // harvest year, so Seaton read grain as scarce at 913 against a cost of
    // 220 — and flour in Harwick sat at 2.7 times Seaton's price, because a
    // mill sized at exactly the cannery's draw has no headroom to build
    // anybody a stock. The cannery bought at the scarce price and sold at the
    // ordinary one.
    //
    // **The farms open the year holding most of last year's harvest.** Day
    // nought is the first of January and the crop peaks about day 226
    // (`harvest_curve`), with barely a trickle before day 180 — so a granary
    // holding four months, which is what `build` opens with, runs the mill
    // dry in June. That is `build`'s annual flour famine, and it is a fact
    // about the opening stock rather than the economy.
    //
    // **A farming district is many farms, not one.** Who owns a works
    // follows its headcount (`building::Ownership::for_size`), and a single
    // site standing for all of Harwick's farmland came to about 250 hands —
    // a corporation, whose profit is spread across the country by
    // population. So sixty per cent of what Harwick's fields earned was paid
    // to households in Seaton, while every Seaton firm kept its profit at
    // home, and Harwick's households ran dry in about four and a half years.
    // Six farms of forty-odd hands are partnerships, owned by the people who
    // work them.
    const FARMS: usize = 6;
    let mut sites: Vec<Site> = (1..=FARMS)
        .map(|n| {
            let share = farm_rate / FARMS as f64;
            works(
                &format!("Harwick farm {n}"),
                SiteKind::Farm,
                HARWICK,
                cap(&[(Grain, share * 220.0)]),
                cap(&[(Grain, share * 480.0)]),
                Some(0),
                share,
            )
        })
        .collect();
    sites.extend([
        works(
            "Harwick mill",
            SiteKind::Mill,
            HARWICK,
            cap(&[(Grain, mill_rate * 4.0), (Flour, mill_rate * 2.0)]),
            cap(&[(Grain, mill_rate * 20.0), (Flour, mill_rate * 15.0)]),
            Some(1),
            mill_rate,
        ),
        works(
            "Harwick colliery",
            SiteKind::Mine,
            HARWICK,
            cap(&[(Coal, coal_per_day * 30.0)]),
            cap(&[(Coal, coal_per_day * 120.0)]),
            Some(5),
            coal_per_day * 1.15,
        ),
        works(
            "Seaton cannery",
            SiteKind::Factory,
            SEATON,
            cap(&[
                (Flour, cannery_rate * 3.0),
                (ProcessedFood, cannery_rate * 2.0),
                (Steel, tinplate * 20.0),
            ]),
            cap(&[
                (Flour, cannery_rate * 15.0),
                (ProcessedFood, cannery_rate * 15.0),
                (Steel, tinplate * 60.0),
            ]),
            Some(2),
            cannery_rate,
        ),
        // **A pithead station**, the way coal-fired power was usually
        // built: next to the colliery, so the coal does not travel and the
        // electricity does. With the station at the port, the port took the
        // whole country's power bills as well as the cannery's margin, and
        // its households ended holding twenty-eight times Harwick's.
        works(
            "Harwick power station",
            SiteKind::PowerPlant,
            HARWICK,
            cap(&[(Coal, coal_per_day * 30.0)]),
            cap(&[(Coal, coal_per_day * 120.0), (Electricity, 1e9)]),
            Some(3),
            crate::econ::UNBOUNDED_THROUGHPUT,
        ),
        // **The tinplate comes ashore where it is used.** A stockholder at
        // a quay decides whether to land on its own town's price, so one at
        // a port with no steel demand of its own never lands a tonne however
        // short the works up-country are — a real defect
        // (`docs/status.md`, defect 14), which this layout does not reach.
        works(
            "Seaton steel stockholder",
            SiteKind::Depot,
            SEATON,
            cap(&[(Steel, tinplate * 30.0)]),
            cap(&[(Steel, tinplate * 90.0)]),
            Some(crate::econ::recipe::STEEL_IMPORTS),
            tinplate * 1.2,
        ),
        works(
            "Harwick market hall",
            SiteKind::Shop,
            HARWICK,
            cap(&[(ProcessedFood, food_per_day * 0.4 * 5.0)]),
            cap(&[(ProcessedFood, food_per_day * 0.4 * 25.0)]),
            None,
            0.0,
        ),
        works(
            "Seaton market hall",
            SiteKind::Shop,
            SEATON,
            cap(&[(ProcessedFood, food_per_day * 0.6 * 5.0)]),
            cap(&[(ProcessedFood, food_per_day * 0.6 * 25.0)]),
            None,
            0.0,
        ),
    ]);

    let mut seaton = Market::new("Seaton", SEATON_POP);
    seaton.port = true;
    seaton.berth = crate::world::Berth::Ocean;
    let markets = vec![Market::new("Harwick", HARWICK_POP), seaton];

    // **Road capacity follows the people it serves**: about twenty-four
    // tonnes a head a year moves by road *(UK: ~1.6bn t across 67M)*, and
    // carriage runs about 0.26 a tonne-kilometre here.
    let km = 120.0;
    let routes = vec![Route {
        id: crate::quote::RouteId(1),
        name: "Harwick–Seaton road".into(),
        a: HARWICK,
        b: SEATON,
        freight_cost: km * 0.26,
        sound_cost: km * 0.26,
        km,
        surface: crate::econ::Surface::Road,
        crossing: crate::econ::Crossing::Level,
        snowed_in: false,
        capacity: people * 24.0 / 365.0,
        moved: None,
        open: true,
    }];

    let markets_len = markets.len();
    let named_roads = routes.len() as u64 + 1;
    let mut economy = Economy {
        ledger: Ledger::new(sites),
        journal: Journal::new(),
        markets,
        routes,
        grid: Grid::for_doctrine(doctrine, load * 1.2),
        response: Response::for_doctrine(doctrine),
        road_condition: vec![1.0],
        maintenance_funding: vec![doctrine.maintenance_funding()],
        weather_seed: 0x5EED_C0FF_EE15_600D,
        unserved_power: 0.0,
        unmet_demand: basket(),
        went_without: basket(),
        workforce: vec![crate::labour::Workforce::default(); markets_len],
        governments: Default::default(),
        logistics: None,
        treasury: crate::money::Treasury::new(),
        told_the_day: None,
        opening: None,
        shipments: crate::registry::Registry::new(),
        power_clearing: None,
        experiments: Default::default(),
        world_seed: 0,
        next_route_id: named_roads,
        routing: crate::quote::Routing::default(),
        hands: Default::default(),
        exported_today: Vec::new(),
        reservations: crate::quote::Reservations::new(),
        import_duty: Default::default(),
        exchange: crate::exchange::Exchange::at_par(),
        arrivals: Vec::new(),
        staff_today: Vec::new(),
        hospital_billed: Default::default(),
        obligations: Default::default(),
        bills_today: Default::default(),
        held_back_today: 0.0,
        payroll_met: Vec::new(),
        state_afford: Default::default(),
        building_stock: Vec::new(),
        building_condition: Vec::new(),
        services: None,
    };
    // **A state and a private service sector**, founded exactly as
    // `region.rs` founds them over a generated nation: between them they
    // are most of the jobs in a developed country, and without them a
    // model of farms, mills and shops leaves nearly everybody with nowhere
    // to work.
    let capacity = match doctrine {
        Doctrine::Prudent => crate::state::Capacity::Developed,
        Doctrine::Negligent => crate::state::Capacity::Middling,
    };
    for n in economy.nations() {
        economy
            .governments
            .insert(n, crate::state::Government::govern(&economy, capacity, n));
    }
    economy.services = Some(crate::services::Services::provide(&economy));
    economy.issue_currency();
    economy.resurvey();
    let peak = economy.power_demand() * 1.20;
    economy.grid = Grid::for_doctrine(doctrine, peak);
    economy
}
