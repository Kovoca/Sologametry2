//! Economic accounting: commodities, stockpiles, production, markets.
//!
//! Implements `docs/state-and-economy-spec.md` Part A, at the scale Part D
//! asks for — six commodities, four recipes, two markets.
//!
//! **The journal is the write path.** A stockpile cannot be modified
//! directly; it changes only by applying a journalled event. That is spec
//! A2.3, and it exists to prevent the class of bug where state quietly
//! evaporates at a handoff: every change has a cause, and conservation is
//! checkable by construction rather than by inspection.
//!
//! Rule R1 (conservation) is asserted every tick in debug builds. Goods
//! enter and leave the ledger through journalled deltas and by no other
//! route.

use std::fmt;

// ---------------------------------------------------------------------------
// Commodities
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Commodity {
    Grain,
    Flour,
    ProcessedFood,
    Electricity,
    Coal,
    RetailGoods,
}

pub const N_COMMODITIES: usize = 6;

impl Commodity {
    pub const ALL: [Commodity; N_COMMODITIES] = [
        Commodity::Grain,
        Commodity::Flour,
        Commodity::ProcessedFood,
        Commodity::Electricity,
        Commodity::Coal,
        Commodity::RetailGoods,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Commodity::Grain => "grain",
            Commodity::Flour => "flour",
            Commodity::ProcessedFood => "food",
            Commodity::Electricity => "power",
            Commodity::Coal => "coal",
            Commodity::RetailGoods => "goods",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            Commodity::Electricity => "MWh",
            Commodity::RetailGoods => "basket",
            _ => "t",
        }
    }

    /// Electricity cannot be stockpiled at this scale — it must be
    /// generated as it is consumed. This is why a grid failure is
    /// instantaneous where a food shortage takes days: there is no buffer
    /// to run down.
    pub fn storable(self) -> bool {
        self != Commodity::Electricity
    }

    /// Price elasticity of demand (negative). Staples barely respond to
    /// price, which is what turns a shortfall into a political event
    /// rather than a gentle reduction in consumption. Spec A.6.
    pub fn elasticity(self) -> f64 {
        match self {
            Commodity::ProcessedFood => -0.25,
            Commodity::Flour => -0.3,
            Commodity::Grain => -0.3,
            Commodity::Electricity => -0.1,
            Commodity::Coal => -0.4,
            Commodity::RetailGoods => -1.0,
        }
    }

    /// Annual household consumption per person, in this commodity's unit.
    /// Zero for things households do not buy directly. Spec A.1.
    pub fn per_capita_annual(self) -> f64 {
        match self {
            Commodity::ProcessedFood => 0.40,
            Commodity::Electricity => 3.0,
            Commodity::RetailGoods => 1.0,
            _ => 0.0,
        }
    }

    /// Days of cover a market tries to hold. Retail food really does run
    /// on three to five days, which is why shortages become visible within
    /// a week. Spec A.3.
    pub fn target_cover_days(self) -> f64 {
        match self {
            Commodity::Electricity => 0.0,
            Commodity::ProcessedFood => 4.0,
            Commodity::Flour => 10.0,
            Commodity::Grain => 30.0,
            Commodity::Coal => 20.0,
            Commodity::RetailGoods => 14.0,
        }
    }

    /// Reference cost of production per unit, in currency. A floor that
    /// price is measured against, not a fixed price.
    pub fn base_cost(self) -> f64 {
        match self {
            Commodity::Grain => 220.0,
            Commodity::Flour => 340.0,
            Commodity::ProcessedFood => 900.0,
            Commodity::Electricity => 60.0,
            Commodity::Coal => 90.0,
            Commodity::RetailGoods => 500.0,
        }
    }
}

impl fmt::Display for Commodity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A quantity of every commodity. Plain array — six commodities, indexed
/// by discriminant.
pub type Basket = [f64; N_COMMODITIES];

#[inline]
pub fn basket() -> Basket {
    [0.0; N_COMMODITIES]
}

// ---------------------------------------------------------------------------
// Journal — the only write path
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// Brought into existence: harvested, mined, manufactured, generated.
    Produced {
        site: usize,
        commodity: Commodity,
        qty: f64,
    },
    /// Destroyed or used up: eaten, burned, consumed as a recipe input.
    Consumed {
        site: usize,
        commodity: Commodity,
        qty: f64,
        reason: Use,
    },
    /// Moved between sites. Conserving by construction.
    Shipped {
        from: usize,
        to: usize,
        commodity: Commodity,
        qty: f64,
    },
    /// Could not be stored and was lost — a full shed, unsold electricity.
    Spoiled {
        site: usize,
        commodity: Commodity,
        qty: f64,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Use {
    /// Eaten or used by people.
    Household,
    /// Fed into a production recipe.
    Input,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub day: u64,
    pub event: Event,
}

#[derive(Default)]
pub struct Journal {
    entries: Vec<Entry>,
}

impl Journal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Entries from the last `days` days, most recent first.
    pub fn recent(&self, today: u64, days: u64) -> impl Iterator<Item = &Entry> {
        let cutoff = today.saturating_sub(days);
        self.entries.iter().rev().take_while(move |e| e.day >= cutoff)
    }
}

// ---------------------------------------------------------------------------
// Sites and the ledger
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SiteKind {
    Farm,
    /// Coal workings. Only exists where the geology put a deposit.
    Mine,
    Mill,
    Factory,
    PowerPlant,
    Shop,
    /// Where goods from outside the modelled region arrive.
    Depot,
}

pub struct Site {
    pub name: String,
    pub kind: SiteKind,
    /// Which market's prices this site trades at.
    pub market: usize,
    pub stock: Basket,
    pub capacity: Basket,
    /// Recipe this site runs, if it produces anything.
    pub recipe: Option<usize>,
    /// Rated output in batches per day. The plant's size — quite separate
    /// from how much it can store.
    pub throughput: f64,
    /// Whether the site currently has the power it needs. Set by the grid;
    /// a site without power cannot run its recipe.
    pub powered: bool,
}

/// Authoritative state. Spec A2: a materialised view of the journal, and
/// the only thing that is saved.
pub struct Ledger {
    pub day: u64,
    pub sites: Vec<Site>,
    /// Running totals, for the conservation check. Everything ever brought
    /// into existence, and everything ever destroyed.
    produced_total: Basket,
    consumed_total: Basket,
    spoiled_total: Basket,
    opening_total: Basket,
}

impl Ledger {
    pub fn new(sites: Vec<Site>) -> Self {
        let mut opening = basket();
        for s in &sites {
            for c in 0..N_COMMODITIES {
                opening[c] += s.stock[c];
            }
        }
        Ledger {
            day: 0,
            sites,
            produced_total: basket(),
            consumed_total: basket(),
            spoiled_total: basket(),
            opening_total: opening,
        }
    }

    /// Total of a commodity held across every site.
    pub fn total(&self, c: Commodity) -> f64 {
        self.sites.iter().map(|s| s.stock[c as usize]).sum()
    }

    /// **The only way stock changes.** Records the event and applies it.
    ///
    /// Taking `&mut Journal` rather than storing one means a caller cannot
    /// apply a change without it being recorded — there is no path that
    /// mutates a stockpile silently.
    pub fn apply(&mut self, journal: &mut Journal, event: Event) {
        match &event {
            Event::Produced {
                site,
                commodity,
                qty,
            } => {
                let c = *commodity as usize;
                self.sites[*site].stock[c] += qty;
                self.produced_total[c] += qty;
            }
            Event::Consumed {
                site,
                commodity,
                qty,
                ..
            } => {
                let c = *commodity as usize;
                self.sites[*site].stock[c] -= qty;
                self.consumed_total[c] += qty;
            }
            Event::Shipped {
                from,
                to,
                commodity,
                qty,
            } => {
                let c = *commodity as usize;
                self.sites[*from].stock[c] -= qty;
                self.sites[*to].stock[c] += qty;
            }
            Event::Spoiled {
                site,
                commodity,
                qty,
            } => {
                let c = *commodity as usize;
                self.sites[*site].stock[c] -= qty;
                self.spoiled_total[c] += qty;
            }
        }
        journal.entries.push(Entry {
            day: self.day,
            event,
        });
    }

    /// Rule R1. Everything held must equal everything that ever arrived
    /// minus everything that ever left. A failure here means something
    /// changed stock without going through the journal, which is the bug
    /// this whole structure exists to make impossible.
    pub fn assert_conserved(&self) {
        for (i, &c) in Commodity::ALL.iter().enumerate() {
            let held = self.total(c);
            let expected =
                self.opening_total[i] + self.produced_total[i]
                    - self.consumed_total[i]
                    - self.spoiled_total[i];
            // Tolerance is measured against total *flow*, not against what
            // happens to be in store. Rounding error accumulates with the
            // number and size of transactions, so a commodity that has
            // moved ten million tonnes and now sits at zero can be a
            // fraction of a tonne out without anything being wrong — while
            // the same absolute drift in a commodity that has barely moved
            // would be a genuine leak.
            let gross = self.opening_total[i]
                + self.produced_total[i]
                + self.consumed_total[i]
                + self.spoiled_total[i];
            let drift = (held - expected).abs();
            let scale = gross.abs().max(1.0);
            assert!(
                drift / scale < 1e-12,
                "conservation violated for {c}: holding {held} but journal says {expected} \
                 (drift {drift:e} against {gross:e} of gross flow)"
            );
        }
    }

    /// Stock a site holds, clamped at zero for float noise.
    #[inline]
    pub fn stock(&self, site: usize, c: Commodity) -> f64 {
        self.sites[site].stock[c as usize].max(0.0)
    }
}

// ---------------------------------------------------------------------------
// Recipes
// ---------------------------------------------------------------------------

/// Inputs + power + labour → outputs, per batch. Ratios from spec A.2.
pub struct Recipe {
    pub name: &'static str,
    pub inputs: &'static [(Commodity, f64)],
    pub outputs: &'static [(Commodity, f64)],
    /// MWh per batch.
    pub power: f64,
    /// Person-hours per batch.
    pub labour: f64,
    /// Needs irrigation water — the farm tier's critical dependency, and a
    /// different failure mode from the factory's.
    pub needs_water: bool,
}

pub const RECIPES: [Recipe; 7] = [
    Recipe {
        name: "farm",
        inputs: &[],
        outputs: &[(Commodity::Grain, 1.0)],
        power: 0.05,
        labour: 8.0,
        needs_water: true,
    },
    Recipe {
        name: "mill",
        inputs: &[(Commodity::Grain, 1.35)],
        outputs: &[(Commodity::Flour, 1.0)],
        power: 0.08,
        labour: 0.2,
        needs_water: false,
    },
    Recipe {
        name: "cannery",
        inputs: &[(Commodity::Flour, 0.9)],
        outputs: &[(Commodity::ProcessedFood, 1.0)],
        power: 0.35,
        labour: 1.2,
        needs_water: false,
    },
    Recipe {
        name: "power plant",
        inputs: &[(Commodity::Coal, 0.38)],
        outputs: &[(Commodity::Electricity, 1.0)],
        power: 0.0,
        labour: 0.02,
        needs_water: false,
    },
    // Goods arriving from outside the modelled region. Real economies
    // import; representing that as production at the boundary keeps
    // conservation honest instead of letting stock appear from nowhere.
    Recipe {
        name: "depot",
        inputs: &[],
        outputs: &[(Commodity::RetailGoods, 1.0)],
        power: 0.02,
        labour: 0.1,
        needs_water: false,
    },
    // Coal has to come out of the ground somewhere. The hand-built slice
    // simply gave its power station a heap of it, which is exactly the
    // sort of thing that stops being tenable once the region is a real
    // place with real geology.
    Recipe {
        name: "coal mine",
        inputs: &[],
        outputs: &[(Commodity::Coal, 1.0)],
        power: 0.03,
        labour: 1.5,
        needs_water: false,
    },
    // Fuel bought from outside the region, landed at a port or railhead.
    // A nation with no coal of its own does not simply go dark — it buys,
    // and in doing so acquires a dependency that can be cut. That
    // vulnerability is the interesting part, and it belongs to geography
    // rather than to anything a designer chose.
    Recipe {
        name: "fuel imports",
        inputs: &[],
        outputs: &[(Commodity::Coal, 1.0)],
        power: 0.01,
        labour: 0.2,
        needs_water: false,
    },
];

/// Indices into `RECIPES`, so scenarios read as places rather than numbers.
pub mod recipe {
    pub const FARM: usize = 0;
    pub const MILL: usize = 1;
    pub const CANNERY: usize = 2;
    pub const POWER_PLANT: usize = 3;
    pub const DEPOT: usize = 4;
    pub const COAL_MINE: usize = 5;
    pub const FUEL_IMPORTS: usize = 6;
}

// ---------------------------------------------------------------------------
// Markets
// ---------------------------------------------------------------------------

pub struct Market {
    pub name: String,
    /// People fed from this market.
    pub population: f64,
    pub price: Basket,
    /// Days of cover currently held, for reporting.
    pub cover: Basket,
}

impl Market {
    pub fn new(name: impl Into<String>, population: f64) -> Self {
        let mut price = basket();
        for (i, c) in Commodity::ALL.iter().enumerate() {
            price[i] = c.base_cost();
        }
        Market {
            name: name.into(),
            population,
            price,
            cover: basket(),
        }
    }

    /// Household demand per day, spec A.4. A floor, not a preference:
    /// below it, people go hungry.
    pub fn daily_household_demand(&self, c: Commodity) -> f64 {
        self.population * c.per_capita_annual() / 365.0
    }
}

// ---------------------------------------------------------------------------
// Power grid
// ---------------------------------------------------------------------------

/// What took a line out of service. The distinction matters enormously
/// for how long it stays out.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Cause {
    /// Conductor, tower, insulator — crews carry replacements.
    Conductor,
    /// The substation transformer. Nothing can be done without one, and
    /// one either sits in a store or is a year away.
    Transformer,
}

/// A transmission line. Spec B.2.
pub struct Line {
    pub name: String,
    /// MWh per day it can carry.
    pub capacity: f64,
    /// 0..1. Falls with age and load, restored by maintenance. Below the
    /// failure threshold it starts tripping.
    pub condition: f64,
    pub up: bool,
    /// Why it is down, if it is.
    pub cause: Option<Cause>,
}

pub struct Grid {
    pub lines: Vec<Line>,
    /// Transmission plus distribution losses. 2-5% and 4-6% respectively
    /// in reality; 8% combined is a fair single figure.
    pub loss: f64,
}

impl Grid {
    /// A grid sized for `peak` load, built to this doctrine's standard.
    pub fn for_doctrine(d: Doctrine, peak: f64) -> Self {
        let line = |name: &str| Line {
            name: name.into(),
            capacity: peak * 1.25,
            condition: 1.0,
            up: true,
            cause: None,
        };
        Grid {
            lines: if d.redundant_grid() {
                vec![line("main line"), line("reserve line")]
            } else {
                vec![line("main line")]
            },
            loss: 0.08,
        }
    }

    /// Capacity actually available right now.
    pub fn capacity(&self) -> f64 {
        self.lines
            .iter()
            .filter(|l| l.up)
            .map(|l| l.capacity)
            .sum::<f64>()
            * (1.0 - self.loss)
    }

    /// Whether the grid still carries peak load with its largest line out
    /// — the N-1 criterion (spec B.2), the concrete purchasable form of
    /// the prudent/negligent doctrine trait.
    pub fn survives_n1(&self, peak_demand: f64) -> bool {
        let largest = self
            .lines
            .iter()
            .filter(|l| l.up)
            .map(|l| l.capacity)
            .fold(0.0f64, f64::max);
        (self.capacity() - largest * (1.0 - self.loss)) >= peak_demand
    }

    /// Take a line out of service by name. Returns whether it was found.
    pub fn fail_line(&mut self, name: &str) -> bool {
        self.fail(name, Cause::Conductor)
    }

    /// Destroy the substation transformer feeding a line — the same
    /// blackout, a completely different recovery.
    pub fn fail_transformer(&mut self, name: &str) -> bool {
        self.fail(name, Cause::Transformer)
    }

    fn fail(&mut self, name: &str, cause: Cause) -> bool {
        for l in self.lines.iter_mut() {
            if l.name == name && l.up {
                l.up = false;
                l.cause = Some(cause);
                return true;
            }
        }
        false
    }

    pub fn restore_line(&mut self, name: &str) -> bool {
        for l in self.lines.iter_mut() {
            if l.name == name && !l.up {
                l.up = true;
                l.condition = 1.0;
                l.cause = None;
                return true;
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Trade routes
// ---------------------------------------------------------------------------

/// A physical link between two markets. Spec A.7 — the freight cost on
/// this route is what bounds the price gap between the markets it joins,
/// and cutting it is therefore an economic event, not just a nuisance.
pub struct Route {
    pub name: String,
    pub a: usize,
    pub b: usize,
    /// Currency per unit moved. Road freight is roughly 12x sea per
    /// tonne-km (spec A.9), which is why the mode matters so much.
    pub freight_cost: f64,
    /// Units per day the route can carry.
    pub capacity: f64,
    pub open: bool,
}

// ---------------------------------------------------------------------------
// Incidents and response
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    /// A transmission line is down. Crews carry emergency restoration
    /// towers and conductor; nothing has to be manufactured.
    Line(String),
    /// A substation transformer is destroyed. **The repair time depends
    /// entirely on whether a spare is in store**: with one, it is a swap
    /// measured in days; without one, it is a custom build with a lead
    /// time measured in months. That gap is why utilities hold spares at
    /// all, and it makes the size of the spares store the single most
    /// consequential prudence decision a state makes about its grid.
    Transformer(String),
}

/// Something broke, and what has happened about it since.
///
/// **Response is not automatic.** A fault sits unreported until somebody
/// notices it and has a working way to tell someone — that is the design
/// doc's rule, and it is why cutting comms is an attack in its own right.
/// After that a crew has to travel, which takes real days along a real
/// road, and during those days it can be helped or stopped.
#[derive(Clone, Debug)]
pub struct Incident {
    pub what: Fault,
    pub occurred: u64,
    pub reported: Option<u64>,
    pub dispatched: Option<u64>,
    /// Day the crew reaches the fault. While `dispatched` is set and this
    /// day has not arrived, the crew is on the road and interceptable.
    pub arrives: Option<u64>,
    /// Days of work once on site. Fixed at dispatch, because whether a
    /// spare part is on the shelf is known the moment the job is assigned.
    pub work_days: u64,
    pub resolved: Option<u64>,
}

impl Incident {
    pub fn in_transit(&self, day: u64) -> bool {
        self.dispatched.is_some()
            && self.resolved.is_none()
            && self.arrives.is_some_and(|a| day < a)
    }
}

/// The region's capacity to answer an incident. Funded by the state, so
/// these numbers are where the prudent/negligent doctrine shows up in
/// whether the lights come back on.
///
/// **Real restoration times.** A downed line is back in two to four days.
/// A destroyed transformer is a swap of about a week *if a spare is in
/// store*, and a twelve-to-eighteen-month wait if not — they are built to
/// order. Everything about grid resilience turns on that difference.
pub struct Response {
    /// Are communications working? With comms down, nothing gets reported.
    pub comms_up: bool,
    /// Crews the state maintains. Zero means nothing is ever repaired.
    pub crews: usize,
    /// Road distance from the depot that holds the crews to the things
    /// they maintain. What doctrine really decides is *where the depot is*
    /// — a well-run utility keeps one in the towns it serves, a neglected
    /// one runs everything from the regional capital.
    pub depot_km: f64,
    /// Road speed of a crew lorry, km/h.
    pub crew_speed_kmh: f64,
    /// Working hours in a day. Beyond this the crew stops for the night,
    /// which is what turns distance into whole days.
    pub working_hours: f64,
    /// Days of work once there. Trained crews with the right parts on the
    /// lorry finish in two; improvised ones take much longer.
    pub repair_days: u64,
    /// Spare transformers in store. Holding these is expensive and looks
    /// like waste right up until the day it does not.
    pub spare_transformers: usize,
    /// Days to have a new transformer built when the store is empty.
    /// Twelve to eighteen months in reality.
    pub transformer_lead_days: u64,
    pub incidents: Vec<Incident>,
}

/// How well the state that governs a region runs its infrastructure.
///
/// The prudent/negligent doctrine trait (spec C.3), expressed in the four
/// places it actually shows up: whether the grid was built with a spare
/// line, how many repair crews are kept, how far away their depot is, and
/// whether there is a spare transformer in store. One trait, four concrete
/// purchases, all of them readable on the ground by a player who looks.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Doctrine {
    /// Bought N-1 redundancy, keeps crews and a parts depot locally, holds
    /// a spare transformer against the day it is needed.
    Prudent,
    /// One line, one crew from the regional capital, nothing in store.
    Negligent,
}

impl Doctrine {
    /// How far the crews are kept from the things they maintain.
    ///
    /// A utility serving real towns keeps a depot among them — crews reach
    /// a fault in under an hour. A neglected one has closed the local
    /// depot and runs everything from the regional capital, which is
    /// further but still a morning's drive. Neither is days away; distance
    /// is not what makes a bad utility slow.
    pub fn depot_km(self) -> f64 {
        match self {
            Doctrine::Prudent => 25.0,
            Doctrine::Negligent => 240.0,
        }
    }

    /// Days of work once on site. A downed transmission line is two to
    /// four days end to end in reality: a trained crew arriving with the
    /// right conductor and an emergency tower manages two, a neglected
    /// utility takes twice that through wrong parts and second trips.
    /// Competence and stores, not mileage.
    pub fn repair_days(self) -> u64 {
        match self {
            Doctrine::Prudent => 2,
            Doctrine::Negligent => 4,
        }
    }

    pub fn crews(self) -> usize {
        match self {
            Doctrine::Prudent => 2,
            Doctrine::Negligent => 1,
        }
    }

    /// Spare transformers in store. The decision that matters most and
    /// shows least: money sitting idle for years, and then the difference
    /// between a week in the dark and a year.
    pub fn spares(self) -> usize {
        match self {
            Doctrine::Prudent => 1,
            Doctrine::Negligent => 0,
        }
    }

    /// Whether the grid is built to survive losing any single line.
    pub fn redundant_grid(self) -> bool {
        self == Doctrine::Prudent
    }
}

impl Response {
    /// The response capacity a state of this doctrine funds.
    pub fn for_doctrine(d: Doctrine) -> Self {
        Response {
            comms_up: true,
            crews: d.crews(),
            depot_km: d.depot_km(),
            crew_speed_kmh: 60.0,
            working_hours: 10.0,
            repair_days: d.repair_days(),
            spare_transformers: d.spares(),
            // Built to order. Twelve to eighteen months is the honest
            // figure for a region waiting its turn in a manufacturer's
            // queue; emergency procurement or borrowing one from a
            // neighbouring utility would beat it, and is the obvious next
            // thing to model.
            transformer_lead_days: 400,
            incidents: Vec::new(),
        }
    }

    /// Whole days to reach the fault. Anything a lorry can cover inside a
    /// working day arrives the same day — which is nearly everything
    /// inside a settled region, and why real outages are measured from
    /// when the crew starts work rather than when it sets off.
    pub fn travel_days(&self) -> u64 {
        let per_day = self.crew_speed_kmh * self.working_hours;
        if per_day <= 0.0 {
            return 0;
        }
        (self.depot_km / per_day).floor() as u64
    }

    pub fn crews_busy(&self, day: u64) -> usize {
        self.incidents
            .iter()
            .filter(|i| i.dispatched.is_some() && i.resolved.is_none_or(|r| r > day))
            .count()
    }

    /// Incidents still waiting for someone to notice them.
    pub fn unreported(&self) -> usize {
        self.incidents
            .iter()
            .filter(|i| i.reported.is_none() && i.resolved.is_none())
            .count()
    }
}

// ---------------------------------------------------------------------------
// The running economy
// ---------------------------------------------------------------------------

pub struct Economy {
    pub ledger: Ledger,
    pub journal: Journal,
    pub markets: Vec<Market>,
    pub routes: Vec<Route>,
    pub grid: Grid,
    pub response: Response,
    /// Electricity that could not be supplied today — the load shed.
    pub unserved_power: f64,
    /// Household demand that could not be met, per commodity. This is
    /// people going without, and it feeds C1's grievance conditions.
    pub unmet_demand: Basket,
}

impl Economy {
    /// Advance one day.
    pub fn step(&mut self) {
        self.unserved_power = 0.0;
        self.unmet_demand = basket();

        self.run_response();
        self.generate_power();
        self.allocate_power();
        self.produce();
        // Sell first, then reorder — a shop restocks against what it has
        // left at close of business, which is what makes the day's cover
        // figure mean "days of stock in hand".
        self.consume_households();
        self.distribute();
        self.trade();
        self.update_prices();
        self.discard_unused_power();

        self.ledger.day += 1;

        #[cfg(debug_assertions)]
        self.ledger.assert_conserved();
    }

    /// Notice faults, report them, dispatch crews, complete repairs.
    ///
    /// Each stage can fail independently, which is the point: a fault
    /// nobody reports is never fixed, a report with no crew to send goes
    /// nowhere, and a crew on the road has not arrived yet.
    fn run_response(&mut self) {
        let day = self.ledger.day;

        // Notice any line that is down and not already on the books.
        let downed: Vec<Fault> = self
            .grid
            .lines
            .iter()
            .filter(|l| !l.up)
            .map(|l| match l.cause {
                Some(Cause::Transformer) => Fault::Transformer(l.name.clone()),
                _ => Fault::Line(l.name.clone()),
            })
            .filter(|fault| {
                !self
                    .response
                    .incidents
                    .iter()
                    .any(|i| i.what == *fault && i.resolved.is_none())
            })
            .collect();
        for fault in downed {
            self.response.incidents.push(Incident {
                what: fault,
                occurred: day,
                reported: None,
                dispatched: None,
                arrives: None,
                work_days: 0,
                resolved: None,
            });
        }

        // Reporting needs a witness with a working way to tell someone.
        // With comms down the fault is simply not known about, however
        // obvious its effects.
        if self.response.comms_up {
            for inc in self.response.incidents.iter_mut() {
                if inc.reported.is_none() && inc.resolved.is_none() {
                    inc.reported = Some(day);
                }
            }
        }

        // Dispatch, oldest report first, as far as crews allow. The work
        // time is decided here, because whether a spare is on the shelf is
        // known the moment the job is assigned.
        let free = self.response.crews.saturating_sub(self.response.crews_busy(day));
        if free > 0 {
            let travel = self.response.travel_days();
            let mut sent = 0;
            let mut order: Vec<usize> = (0..self.response.incidents.len()).collect();
            order.sort_by_key(|&i| self.response.incidents[i].reported.unwrap_or(u64::MAX));

            for i in order {
                if sent >= free {
                    break;
                }
                if self.response.incidents[i].reported.is_none()
                    || self.response.incidents[i].dispatched.is_some()
                    || self.response.incidents[i].resolved.is_some()
                {
                    continue;
                }

                let work = match &self.response.incidents[i].what {
                    Fault::Line(_) => self.response.repair_days,
                    Fault::Transformer(_) => {
                        if self.response.spare_transformers > 0 {
                            self.response.spare_transformers -= 1;
                            self.response.repair_days + 5 // fit the spare
                        } else {
                            // Nothing in store: wait for one to be built.
                            self.response.transformer_lead_days
                        }
                    }
                };

                let inc = &mut self.response.incidents[i];
                inc.dispatched = Some(day);
                inc.arrives = Some(day + travel);
                inc.work_days = work;
                sent += 1;
            }
        }

        // Arrivals and completed work.
        let mut restored: Vec<String> = Vec::new();
        for inc in self.response.incidents.iter_mut() {
            if inc.resolved.is_some() {
                continue;
            }
            let Some(arrives) = inc.arrives else { continue };
            if day >= arrives + inc.work_days {
                inc.resolved = Some(day);
                match &inc.what {
                    Fault::Line(name) | Fault::Transformer(name) => restored.push(name.clone()),
                }
            }
        }
        for name in restored {
            self.grid.restore_line(&name);
        }
    }

    /// Power plants burn fuel and generate. Limited by what the grid can
    /// actually carry — generating into a severed line is pointless.
    fn generate_power(&mut self) {
        let carry = self.grid.capacity();
        let mut remaining = carry;

        for site in 0..self.ledger.sites.len() {
            if self.ledger.sites[site].kind != SiteKind::PowerPlant {
                continue;
            }
            let Some(r) = self.ledger.sites[site].recipe else {
                continue;
            };
            let recipe = &RECIPES[r];
            if remaining <= 0.0 {
                break;
            }

            // Batches are limited by fuel on hand and by what can be
            // delivered.
            let mut batches = remaining;
            for &(c, need) in recipe.inputs {
                let have = self.ledger.stock(site, c);
                batches = batches.min(have / need);
            }
            if batches <= 1e-9 {
                continue;
            }

            for &(c, need) in recipe.inputs {
                self.ledger.apply(
                    &mut self.journal,
                    Event::Consumed {
                        site,
                        commodity: c,
                        qty: need * batches,
                        reason: Use::Input,
                    },
                );
            }
            for &(c, out) in recipe.outputs {
                self.ledger.apply(
                    &mut self.journal,
                    Event::Produced {
                        site,
                        commodity: c,
                        qty: out * batches,
                    },
                );
            }
            remaining -= batches;
        }
    }

    /// Share the day's electricity out. Priority order is critical →
    /// industrial → household (spec B.2); anyone who misses out loses
    /// power for the day.
    fn allocate_power(&mut self) {
        let mut available: f64 = (0..self.ledger.sites.len())
            .filter(|&s| self.ledger.sites[s].kind == SiteKind::PowerPlant)
            .map(|s| self.ledger.stock(s, Commodity::Electricity))
            .sum();

        // What each consumer needs today.
        let mut wants: Vec<(usize, f64)> = Vec::new();
        for site in 0..self.ledger.sites.len() {
            let s = &self.ledger.sites[site];
            if s.kind == SiteKind::PowerPlant {
                continue;
            }
            let need = match s.recipe {
                Some(r) => RECIPES[r].power * self.site_capacity(site),
                None => 0.0,
            };
            if need > 0.0 {
                wants.push((site, need));
            }
        }
        // Industry before shops before homes; within a class, larger first,
        // so a shortage stops whole plants rather than crippling all of
        // them.
        wants.sort_by(|a, b| {
            let rank = |i: usize| match self.ledger.sites[i].kind {
                // The mine first: without coal nothing generates at all
                // tomorrow, so starving it to keep a factory running today
                // is how a grid talks itself into a blackout.
                SiteKind::Mine => 0,
                SiteKind::Factory | SiteKind::Mill => 1,
                SiteKind::Farm => 2,
                SiteKind::Shop => 3,
                _ => 4,
            };
            rank(a.0)
                .cmp(&rank(b.0))
                .then(b.1.total_cmp(&a.1))
                .then(a.0.cmp(&b.0))
        });

        for site in 0..self.ledger.sites.len() {
            self.ledger.sites[site].powered = self.ledger.sites[site].kind == SiteKind::PowerPlant;
        }

        for (site, need) in wants {
            if available >= need {
                available -= need;
                self.ledger.sites[site].powered = true;
            } else {
                self.unserved_power += need;
                self.ledger.sites[site].powered = false;
            }
        }
    }

    /// How many batches a site could run today if nothing were short.
    #[inline]
    fn site_capacity(&self, site: usize) -> f64 {
        self.ledger.sites[site].throughput
    }

    /// Everything that is not a power plant runs its recipe as far as
    /// inputs, power and storage allow.
    fn produce(&mut self) {
        for site in 0..self.ledger.sites.len() {
            let s = &self.ledger.sites[site];
            if s.kind == SiteKind::PowerPlant {
                continue;
            }
            let Some(r) = s.recipe else { continue };
            if !s.powered {
                continue; // no power, no production
            }
            let recipe = &RECIPES[r];

            let mut batches = self.site_capacity(site);
            for &(c, need) in recipe.inputs {
                batches = batches.min(self.ledger.stock(site, c) / need);
            }
            // Do not produce into a full shed.
            for &(c, out) in recipe.outputs {
                let room = (self.ledger.sites[site].capacity[c as usize]
                    - self.ledger.stock(site, c))
                .max(0.0);
                batches = batches.min(room / out);
            }
            if batches <= 1e-9 {
                continue;
            }

            // Power is drawn from the plants that generated it.
            let draw = recipe.power * batches;
            self.draw_power(draw);

            for &(c, need) in recipe.inputs {
                self.ledger.apply(
                    &mut self.journal,
                    Event::Consumed {
                        site,
                        commodity: c,
                        qty: need * batches,
                        reason: Use::Input,
                    },
                );
            }
            for &(c, out) in recipe.outputs {
                self.ledger.apply(
                    &mut self.journal,
                    Event::Produced {
                        site,
                        commodity: c,
                        qty: out * batches,
                    },
                );
            }
        }
    }

    /// Consume electricity from wherever it was generated.
    fn draw_power(&mut self, mut qty: f64) {
        for site in 0..self.ledger.sites.len() {
            if qty <= 1e-12 {
                break;
            }
            if self.ledger.sites[site].kind != SiteKind::PowerPlant {
                continue;
            }
            let have = self.ledger.stock(site, Commodity::Electricity);
            let take = have.min(qty);
            if take <= 0.0 {
                continue;
            }
            self.ledger.apply(
                &mut self.journal,
                Event::Consumed {
                    site,
                    commodity: Commodity::Electricity,
                    qty: take,
                    reason: Use::Input,
                },
            );
            qty -= take;
        }
    }

    /// For each market, every market reachable from it over open routes,
    /// itself included. Cutting a route splits a component, which is what
    /// makes severing the network an economic act rather than a delay.
    fn market_components(&self) -> Vec<Vec<usize>> {
        let n = self.markets.len();
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for r in self.routes.iter().filter(|r| r.open) {
            adj[r.a].push(r.b);
            adj[r.b].push(r.a);
        }

        (0..n)
            .map(|start| {
                let mut seen = vec![false; n];
                let mut stack = vec![start];
                seen[start] = true;
                let mut out = Vec::new();
                while let Some(m) = stack.pop() {
                    out.push(m);
                    for &j in &adj[m] {
                        if !seen[j] {
                            seen[j] = true;
                            stack.push(j);
                        }
                    }
                }
                out.sort_unstable();
                out
            })
            .collect()
    }

    /// Move finished goods from the works that made them to the shops that
    /// sell them, and inputs along the production chain.
    ///
    /// Shops pull toward their target days of cover (spec A.3) rather than
    /// taking everything available — that restocking pull is the
    /// stockbuilding demand of spec A.4, and it is what makes a shortage
    /// self-reinforcing when cover falls everywhere at once.
    fn distribute(&mut self) {
        // Which markets each market can be supplied from: everywhere the
        // open route network reaches, not merely its direct neighbours.
        // Goods transship — a town at the end of a chain is supplied
        // through the towns between, and only becomes isolated when the
        // network is actually severed. Restricting supply to one hop makes
        // outlying towns starve for want of a road that exists.
        let reach = self.market_components();

        for dst in 0..self.ledger.sites.len() {
            let kind = self.ledger.sites[dst].kind;
            let market = self.ledger.sites[dst].market;

            for &c in Commodity::ALL.iter() {
                if !c.storable() {
                    continue;
                }

                // How much this site wants: a shop stocks to cover, a
                // works keeps a few days of its own inputs.
                let want = match kind {
                    SiteKind::Shop => {
                        let daily = self.markets[market].daily_household_demand(c);
                        if daily <= 0.0 {
                            continue;
                        }
                        daily * c.target_cover_days()
                    }
                    _ => {
                        let Some(r) = self.ledger.sites[dst].recipe else {
                            continue;
                        };
                        let per = RECIPES[r]
                            .inputs
                            .iter()
                            .find(|&&(ic, _)| ic == c)
                            .map(|&(_, q)| q);
                        let Some(per) = per else { continue };
                        per * self.ledger.sites[dst].throughput * 3.0
                    }
                };

                let mut short = want - self.ledger.stock(dst, c);
                if short <= 1e-9 {
                    continue;
                }
                let room = (self.ledger.sites[dst].capacity[c as usize]
                    - self.ledger.stock(dst, c))
                .max(0.0);
                short = short.min(room);

                // Draw from producers of this commodity: this market
                // first, then anywhere an open route reaches. A town with
                // no works of its own is supplied down the road, which is
                // the ordinary case and is why cutting the road starves it.
                for src in 0..self.ledger.sites.len() {
                    if short <= 1e-9 {
                        break;
                    }
                    if src == dst || !reach[market].contains(&self.ledger.sites[src].market) {
                        continue;
                    }
                    let Some(r) = self.ledger.sites[src].recipe else {
                        continue;
                    };
                    if !RECIPES[r].outputs.iter().any(|&(oc, _)| oc == c) {
                        continue;
                    }
                    let qty = self.ledger.stock(src, c).min(short);
                    if qty <= 1e-9 {
                        continue;
                    }
                    self.ledger.apply(
                        &mut self.journal,
                        Event::Shipped {
                            from: src,
                            to: dst,
                            commodity: c,
                            qty,
                        },
                    );
                    short -= qty;
                }
            }
        }
    }

    /// People eat. Demand is a floor: what cannot be met is recorded as
    /// people going without, not quietly reduced.
    fn consume_households(&mut self) {
        for m in 0..self.markets.len() {
            for &c in Commodity::ALL.iter() {
                let want = self.markets[m].daily_household_demand(c);
                if want <= 0.0 || !c.storable() {
                    continue;
                }
                let mut left = want;
                for site in 0..self.ledger.sites.len() {
                    if left <= 1e-12 {
                        break;
                    }
                    let s = &self.ledger.sites[site];
                    if s.market != m || s.kind != SiteKind::Shop {
                        continue;
                    }
                    let take = self.ledger.stock(site, c).min(left);
                    if take <= 0.0 {
                        continue;
                    }
                    self.ledger.apply(
                        &mut self.journal,
                        Event::Consumed {
                            site,
                            commodity: c,
                            qty: take,
                            reason: Use::Household,
                        },
                    );
                    left -= take;
                }
                self.unmet_demand[c as usize] += left;
            }
        }
    }

    /// Move goods where the price gap beats the freight cost.
    ///
    /// This is spec A.7's governing relation made mechanical: the gap
    /// between two markets cannot stay wider than the cost of closing it,
    /// because closing it is profitable. Shut the route and the markets
    /// decouple.
    fn trade(&mut self) {
        for r in 0..self.routes.len() {
            if !self.routes[r].open {
                continue;
            }
            let (a, b) = (self.routes[r].a, self.routes[r].b);
            let freight = self.routes[r].freight_cost;
            let mut budget = self.routes[r].capacity;

            for &c in Commodity::ALL.iter() {
                if !c.storable() || budget <= 1e-9 {
                    continue;
                }
                let (pa, pb) = (self.markets[a].price[c as usize], self.markets[b].price[c as usize]);
                let (from_m, to_m, gap) = if pb - pa > freight {
                    (a, b, pb - pa - freight)
                } else if pa - pb > freight {
                    (b, a, pa - pb - freight)
                } else {
                    continue; // gap does not cover the haul
                };
                let _ = gap;

                // Ship from the surplus market's shops to the deficit's.
                let source: Vec<usize> = (0..self.ledger.sites.len())
                    .filter(|&s| {
                        self.ledger.sites[s].market == from_m
                            && self.ledger.sites[s].kind == SiteKind::Shop
                    })
                    .collect();
                let sink: Vec<usize> = (0..self.ledger.sites.len())
                    .filter(|&s| {
                        self.ledger.sites[s].market == to_m
                            && self.ledger.sites[s].kind == SiteKind::Shop
                    })
                    .collect();
                let (Some(&dst), false) = (sink.first(), source.is_empty()) else {
                    continue;
                };

                // Only genuine surplus moves. A trader who empties his home
                // market to chase a price has no home market — and without
                // this the two towns simply slosh stock back and forth.
                let keep =
                    self.markets[from_m].daily_household_demand(c) * c.target_cover_days();
                for src in source {
                    if budget <= 1e-9 {
                        break;
                    }
                    let spare = (self.ledger.stock(src, c) - keep).max(0.0);
                    let room = (self.ledger.sites[dst].capacity[c as usize]
                        - self.ledger.stock(dst, c))
                    .max(0.0);
                    let qty = spare.min(room).min(budget);
                    if qty <= 1e-9 {
                        continue;
                    }
                    self.ledger.apply(
                        &mut self.journal,
                        Event::Shipped {
                            from: src,
                            to: dst,
                            commodity: c,
                            qty,
                        },
                    );
                    budget -= qty;
                }
            }
        }
    }

    /// Price from stock cover against demand. Spec A.6.
    fn update_prices(&mut self) {
        for m in 0..self.markets.len() {
            for &c in Commodity::ALL.iter() {
                let demand = self.markets[m].daily_household_demand(c);
                if demand <= 0.0 {
                    self.markets[m].cover[c as usize] = f64::INFINITY;
                    continue;
                }
                let stock: f64 = (0..self.ledger.sites.len())
                    .filter(|&s| {
                        self.ledger.sites[s].market == m
                            && self.ledger.sites[s].kind == SiteKind::Shop
                    })
                    .map(|s| self.ledger.stock(s, c))
                    .sum();

                let cover = stock / demand;
                self.markets[m].cover[c as usize] = cover;

                // Elasticity relates a *proportional* shortfall to a
                // proportional price move: a 20% shortfall in a good with
                // elasticity -0.25 raises price about 80%, which is the
                // real relationship. Raising scarcity to the power of
                // 1/elasticity instead compounds to absurdity — an early
                // version of this priced food at four thousand times cost.
                let target = c.target_cover_days().max(0.5);
                let gap = (target - cover) / target;
                let multiplier =
                    (1.0 + gap / c.elasticity().abs()).clamp(0.4, 8.0);
                self.markets[m].price[c as usize] = c.base_cost() * multiplier;
            }
        }
    }

    /// Electricity that nobody drew is simply lost — it cannot be stored.
    fn discard_unused_power(&mut self) {
        for site in 0..self.ledger.sites.len() {
            if self.ledger.sites[site].kind != SiteKind::PowerPlant {
                continue;
            }
            let left = self.ledger.stock(site, Commodity::Electricity);
            if left > 1e-12 {
                self.ledger.apply(
                    &mut self.journal,
                    Event::Spoiled {
                        site,
                        commodity: Commodity::Electricity,
                        qty: left,
                    },
                );
            }
        }
    }

    /// Price of `c` in market `m`.
    pub fn price(&self, m: usize, c: Commodity) -> f64 {
        self.markets[m].price[c as usize]
    }

    /// Profit per unit from hauling `c` along `route`, after freight.
    /// Positive means a contract worth taking exists — and it exists
    /// because the arithmetic says so, not because anything generated it.
    pub fn arbitrage(&self, route: usize, c: Commodity) -> f64 {
        let r = &self.routes[route];
        let (pa, pb) = (self.price(r.a, c), self.price(r.b, c));
        (pb - pa).abs() - r.freight_cost
    }
}
