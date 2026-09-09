//! **Writing the economy down.**
//!
//! One file, deliberately, when the convention elsewhere is to put a codec
//! beside its type: **the wire codes are a single frozen namespace** and
//! keeping them together is what makes "these numbers never change"
//! reviewable in one place rather than a promise spread over six files.
//!
//! Every rule `save.rs` states applies here, and two of them decide most
//! of what follows:
//!
//! - **A variant's position is not its encoding.** Every enum carries an
//!   explicit code, so inserting a variant tomorrow cannot reinterpret
//!   every save made today. This project has now had that defect twice —
//!   a shipment's commodity, and `ALL_MATERIALS` missing `Water` — and both
//!   times what caught it was an exhaustive match refusing to compile.
//! - **A basket is not eighteen numbers in a row.** It is
//!   `[f64; N_COMMODITIES]`, so writing it positionally would break every
//!   save the day a nineteenth commodity is added, which the resource work
//!   is going to do. It is written as `(wire code, value)` pairs, so a save
//!   made before limestone existed loads into a world that has it and every
//!   figure lands on the commodity it was measured for.
//!
//! What is **not** here, and why: the economy's `routing` is an all-pairs
//! table derived from the roads and rebuilt by `resurvey` every morning. It
//! is a cache, not state. Writing it down would store a value that has to
//! agree with the roads and can silently stop agreeing, so a load rebuilds
//! it instead.

use crate::econ::{
    Basket, Commodity, Crossing, Fault, Level, Market, Opening, Route, Site, SiteKind, Surface,
};
use crate::save::{Reader, SaveError, Store, Writer};

// =====================================================================
// the basket
// =====================================================================

/// **A basket is a reading per commodity, not eighteen numbers.**
///
/// See the module header. The count goes down as well as the pairs, so a
/// reader knows when to stop without trusting `N_COMMODITIES` to have been
/// the same on both sides.
pub fn store_basket(w: &mut Writer, b: &Basket) {
    w.len(Commodity::ALL.len());
    for &c in Commodity::ALL.iter() {
        w.u16(c.wire_code());
        w.f64(b[c as usize]);
    }
}

pub fn load_basket(r: &mut Reader) -> Result<Basket, SaveError> {
    let n = r.count()?;
    let mut b: Basket = [0.0; crate::econ::N_COMMODITIES];
    for _ in 0..n {
        let code = r.u16()?;
        // **An infinity in a basket is a real reading, and a NaN is not.**
        //
        // Electricity declares an infinite days of cover on purpose: none
        // of it is ever held, so "how long would the store last" has no
        // finite answer and the price pass is required to notice. Refusing
        // it here would make every real world unloadable — and it very
        // nearly did. A NaN is still refused, because unlike an infinity
        // it is not a measurement of anything.
        let v = r.f64()?;
        if v.is_nan() {
            return Err(SaveError::NotANumber);
        }
        // **A commodity this build does not have is not an error.** A save
        // from a world with limestone in it, read by a build without it,
        // carries a figure for something that cannot be represented — and
        // refusing the whole world over one unknown column would make
        // every future commodity a breaking change. The figure is dropped
        // and the rest of the basket is exact.
        if let Some(c) = Commodity::from_wire_code(code) {
            b[c as usize] = v;
        }
    }
    Ok(b)
}

// =====================================================================
// enums — every code frozen, every match exhaustive
// =====================================================================

impl Store for SiteKind {
    fn store(&self, w: &mut Writer) {
        w.u16(match self {
            SiteKind::Farm => 1,
            SiteKind::Pasture => 2,
            SiteKind::Butcher => 3,
            SiteKind::Mill => 4,
            SiteKind::Factory => 5,
            SiteKind::Mine => 20,
            SiteKind::IronMine => 21,
            SiteKind::OilField => 22,
            SiteKind::Forestry => 23,
            SiteKind::Steelworks => 40,
            SiteKind::Works => 41,
            SiteKind::Cracker => 42,
            SiteKind::MachineWorks => 43,
            SiteKind::CementWorks => 44,
            SiteKind::ChemicalWorks => 45,
            SiteKind::Pharma => 46,
            SiteKind::Builders => 60,
            SiteKind::Hospital => 61,
            SiteKind::PowerPlant => 62,
            SiteKind::Shop => 63,
            SiteKind::Depot => 64,
        });
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(match r.u16()? {
            1 => SiteKind::Farm,
            2 => SiteKind::Pasture,
            3 => SiteKind::Butcher,
            4 => SiteKind::Mill,
            5 => SiteKind::Factory,
            20 => SiteKind::Mine,
            21 => SiteKind::IronMine,
            22 => SiteKind::OilField,
            23 => SiteKind::Forestry,
            40 => SiteKind::Steelworks,
            41 => SiteKind::Works,
            42 => SiteKind::Cracker,
            43 => SiteKind::MachineWorks,
            44 => SiteKind::CementWorks,
            45 => SiteKind::ChemicalWorks,
            46 => SiteKind::Pharma,
            60 => SiteKind::Builders,
            61 => SiteKind::Hospital,
            62 => SiteKind::PowerPlant,
            63 => SiteKind::Shop,
            64 => SiteKind::Depot,
            n => return Err(SaveError::UnknownCode("site kind", n as u32)),
        })
    }
}

impl Store for Surface {
    fn store(&self, w: &mut Writer) {
        w.u8(match self {
            Surface::Water => 1,
            Surface::Highway => 2,
            Surface::Road => 3,
            Surface::Track => 4,
            Surface::Open => 5,
        });
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(match r.u8()? {
            1 => Surface::Water,
            2 => Surface::Highway,
            3 => Surface::Road,
            4 => Surface::Track,
            5 => Surface::Open,
            n => return Err(SaveError::UnknownCode("road surface", n as u32)),
        })
    }
}

impl Store for Level {
    fn store(&self, w: &mut Writer) {
        w.u8(match self {
            Level::Service => 1,
            Level::Transformer => 2,
            Level::Feeder => 3,
            Level::Substation => 4,
            Level::Transmission => 5,
        });
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(match r.u8()? {
            1 => Level::Service,
            2 => Level::Transformer,
            3 => Level::Feeder,
            4 => Level::Substation,
            5 => Level::Transmission,
            n => return Err(SaveError::UnknownCode("grid level", n as u32)),
        })
    }
}

impl Store for crate::econ::Cause {
    fn store(&self, w: &mut Writer) {
        w.u8(match self {
            crate::econ::Cause::Conductor => 1,
            crate::econ::Cause::Transformer => 2,
        });
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(match r.u8()? {
            1 => crate::econ::Cause::Conductor,
            2 => crate::econ::Cause::Transformer,
            n => return Err(SaveError::UnknownCode("outage cause", n as u32)),
        })
    }
}

impl Store for Fault {
    fn store(&self, w: &mut Writer) {
        match self {
            Fault::Line(name) => {
                w.u8(1);
                w.str(name);
            }
            Fault::Transformer(name) => {
                w.u8(2);
                w.str(name);
            }
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(match r.u8()? {
            1 => Fault::Line(r.str()?),
            2 => Fault::Transformer(r.str()?),
            n => return Err(SaveError::UnknownCode("fault", n as u32)),
        })
    }
}

impl Store for Crossing {
    fn store(&self, w: &mut Writer) {
        match self {
            Crossing::Level => w.u8(1),
            Crossing::Pass { summit, cold } => {
                w.u8(2);
                w.f64(*summit);
                w.f64(*cold);
            }
            Crossing::Tunnel { capital } => {
                w.u8(3);
                w.f64(*capital);
            }
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(match r.u8()? {
            1 => Crossing::Level,
            2 => Crossing::Pass {
                summit: r.finite_f64()?,
                cold: r.finite_f64()?,
            },
            3 => Crossing::Tunnel {
                capital: r.finite_f64()?,
            },
            n => return Err(SaveError::UnknownCode("mountain crossing", n as u32)),
        })
    }
}

impl Store for crate::state::Capacity {
    fn store(&self, w: &mut Writer) {
        w.u8(match self {
            crate::state::Capacity::Developed => 1,
            crate::state::Capacity::Middling => 2,
            crate::state::Capacity::Weak => 3,
        });
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(match r.u8()? {
            1 => crate::state::Capacity::Developed,
            2 => crate::state::Capacity::Middling,
            3 => crate::state::Capacity::Weak,
            n => return Err(SaveError::UnknownCode("state capacity", n as u32)),
        })
    }
}

impl Store for crate::quote::RouteId {
    fn store(&self, w: &mut Writer) {
        w.u64(self.0);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(crate::quote::RouteId(r.u64()?))
    }
}

// =====================================================================
// the physical world: sites, markets, roads
// =====================================================================

impl Store for Route {
    fn store(&self, w: &mut Writer) {
        self.id.store(w);
        w.str(&self.name);
        w.len(self.a);
        w.len(self.b);
        w.f64(self.freight_cost);
        w.f64(self.sound_cost);
        w.f64(self.km);
        self.surface.store(w);
        self.crossing.store(w);
        w.bool(self.snowed_in);
        w.f64(self.capacity);
        w.bool(self.open);
        match &self.moved {
            None => w.u8(0),
            Some((c, to, t)) => {
                w.u8(1);
                w.u16(c.wire_code());
                w.len(*to);
                w.f64(*t);
            }
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let id = crate::quote::RouteId::load(r)?;
        if id.0 == 0 {
            return Err(SaveError::Impossible("a road with no name"));
        }
        let route = Route {
            id,
            name: r.str()?,
            a: r.read_len()?,
            b: r.read_len()?,
            freight_cost: r.finite_f64()?,
            sound_cost: r.finite_f64()?,
            km: r.finite_f64()?,
            surface: Surface::load(r)?,
            crossing: Crossing::load(r)?,
            snowed_in: r.bool()?,
            capacity: r.finite_f64()?,
            open: r.bool()?,
            moved: match r.u8()? {
                0 => None,
                1 => {
                    let code = r.u16()?;
                    let c = Commodity::from_wire_code(code)
                        .ok_or(SaveError::UnknownCode("commodity", code as u32))?;
                    Some((c, r.read_len()?, r.finite_f64()?))
                }
                n => return Err(SaveError::UnknownCode("route traffic tag", n as u32)),
            },
        };
        if route.a == route.b {
            return Err(SaveError::Impossible("a road from a town to itself"));
        }
        if route.km < 0.0 || route.capacity < 0.0 || route.freight_cost < 0.0 {
            return Err(SaveError::Impossible("a road with a negative measurement"));
        }
        Ok(route)
    }
}

impl Store for Opening {
    fn store(&self, w: &mut Writer) {
        w.u64(self.day);
        for baskets in [&self.price, &self.cover, &self.landed, &self.stock] {
            w.len(baskets.len());
            for b in baskets.iter() {
                store_basket(w, b);
            }
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let day = r.u64()?;
        let read = |r: &mut Reader| -> Result<Vec<Basket>, SaveError> {
            let n = r.count()?;
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push(load_basket(r)?);
            }
            Ok(v)
        };
        Ok(Opening {
            day,
            price: read(r)?,
            cover: read(r)?,
            landed: read(r)?,
            stock: read(r)?,
        })
    }
}

impl Store for Market {
    fn store(&self, w: &mut Writer) {
        w.str(&self.name);
        w.u16(self.nation);
        w.f64(self.population);
        w.bool(self.southern);
        w.f64(self.harvest_quality);
        for b in [
            &self.price,
            &self.cost,
            &self.landed,
            &self.cover,
            &self.expected_cover,
        ] {
            store_basket(w, b);
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let name = r.str()?;
        let nation = r.u16()?;
        let population = r.finite_f64()?;
        let southern = r.bool()?;
        let harvest_quality = r.finite_f64()?;
        if population < 0.0 {
            return Err(SaveError::Impossible("a market with a negative population"));
        }
        Ok(Market {
            name,
            nation,
            population,
            southern,
            harvest_quality,
            price: load_basket(r)?,
            cost: load_basket(r)?,
            landed: load_basket(r)?,
            cover: load_basket(r)?,
            expected_cover: load_basket(r)?,
        })
    }
}

impl Store for Site {
    fn store(&self, w: &mut Writer) {
        w.str(&self.name);
        self.kind.store(w);
        w.len(self.market);
        store_basket(w, &self.stock);
        store_basket(w, &self.capacity);
        match self.recipe {
            None => w.u8(0),
            Some(i) => {
                w.u8(1);
                w.len(i);
            }
        }
        w.f64(self.throughput);
        w.bool(self.powered);
        w.f64(self.ran);
        w.f64(self.cost_factor);
        // **A site's fixture list is written down as a fixture list**, and
        // that is worth being explicit about: `building::Building` is a
        // count of tills and shelving rather than a persistent building
        // instance, which `building.rs` records as a known gap. It is
        // stored because the staff numbers come off it and a shop that
        // reloaded without its tills would employ nobody.
        match &self.fitted {
            None => w.u8(0),
            Some(b) => {
                w.u8(1);
                b.store(w);
            }
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let name = r.str()?;
        let kind = SiteKind::load(r)?;
        let market = r.read_len()?;
        let stock = load_basket(r)?;
        let capacity = load_basket(r)?;
        let recipe = match r.u8()? {
            0 => None,
            1 => Some(r.read_len()?),
            n => return Err(SaveError::UnknownCode("site recipe tag", n as u32)),
        };
        let throughput = r.finite_f64()?;
        let powered = r.bool()?;
        let ran = r.finite_f64()?;
        let cost_factor = r.finite_f64()?;
        let fitted = match r.u8()? {
            0 => None,
            1 => Some(crate::building::Building::load(r)?),
            n => return Err(SaveError::UnknownCode("site fixture tag", n as u32)),
        };
        if throughput < 0.0 || ran < 0.0 || cost_factor < 0.0 {
            return Err(SaveError::Impossible("a site with a negative rate or cost"));
        }
        if let Some(i) = recipe {
            if i >= crate::econ::RECIPES.len() {
                return Err(SaveError::Impossible("a site whose recipe does not exist"));
            }
        }
        // **What is in the shed cannot exceed the shed.** A stock above its
        // own capacity is a contradiction the ledger's allocation code
        // would then try to reconcile by delivering nothing for ever.
        for &c in Commodity::ALL.iter() {
            let i = c as usize;
            if stock[i] < 0.0 {
                return Err(SaveError::Impossible("a site holding a negative stock"));
            }
            if stock[i] > capacity[i] * 1.000_001 + 1e-6 {
                return Err(SaveError::Impossible("a site holding more than its store"));
            }
        }
        Ok(Site {
            name,
            kind,
            market,
            stock,
            capacity,
            recipe,
            throughput,
            powered,
            ran,
            cost_factor,
            fitted,
        })
    }
}

// =====================================================================
// what a shop is fitted out with
// =====================================================================

impl Store for crate::building::Fixture {
    fn store(&self, w: &mut Writer) {
        use crate::building::Fixture as F;
        w.u8(match self {
            F::Till => 1,
            F::Shelving => 2,
            F::StockRack => 3,
            F::LoadingBay => 4,
            F::Counter => 5,
            F::ChillCabinet => 6,
            F::ColdStore => 7,
        });
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        use crate::building::Fixture as F;
        Ok(match r.u8()? {
            1 => F::Till,
            2 => F::Shelving,
            3 => F::StockRack,
            4 => F::LoadingBay,
            5 => F::Counter,
            6 => F::ChillCabinet,
            7 => F::ColdStore,
            n => return Err(SaveError::UnknownCode("shop fixture", n as u32)),
        })
    }
}

impl Store for crate::building::Building {
    fn store(&self, w: &mut Writer) {
        w.len(self.fixtures.len());
        for (f, n) in self.fixtures.iter() {
            f.store(w);
            w.f64(*n);
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let n = r.count()?;
        let mut fixtures = Vec::with_capacity(n);
        for _ in 0..n {
            let f = crate::building::Fixture::load(r)?;
            let count = r.finite_f64()?;
            if count < 0.0 {
                return Err(SaveError::Impossible("a negative number of fixtures"));
            }
            fixtures.push((f, count));
        }
        Ok(crate::building::Building { fixtures })
    }
}

// =====================================================================
// the grid, and who mends it
// =====================================================================

impl Store for crate::econ::Line {
    fn store(&self, w: &mut Writer) {
        w.str(&self.name);
        self.level.store(w);
        match self.serves {
            None => w.u8(0),
            Some(i) => {
                w.u8(1);
                w.len(i);
            }
        }
        w.len(self.feeds.len());
        for &f in self.feeds.iter() {
            w.len(f);
        }
        match self.parent {
            None => w.u8(0),
            Some(i) => {
                w.u8(1);
                w.len(i);
            }
        }
        w.bool(self.ring_fed);
        w.f64(self.capacity);
        w.f64(self.condition);
        w.bool(self.up);
        match &self.cause {
            None => w.u8(0),
            Some(c) => {
                w.u8(1);
                c.store(w);
            }
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let name = r.str()?;
        let level = Level::load(r)?;
        let serves = match r.u8()? {
            0 => None,
            1 => Some(r.read_len()?),
            n => return Err(SaveError::UnknownCode("line serves tag", n as u32)),
        };
        let n = r.count()?;
        let mut feeds = Vec::with_capacity(n);
        for _ in 0..n {
            feeds.push(r.read_len()?);
        }
        let parent = match r.u8()? {
            0 => None,
            1 => Some(r.read_len()?),
            n => return Err(SaveError::UnknownCode("line parent tag", n as u32)),
        };
        let ring_fed = r.bool()?;
        let capacity = r.finite_f64()?;
        let condition = r.finite_f64()?;
        let up = r.bool()?;
        let cause = match r.u8()? {
            0 => None,
            1 => Some(crate::econ::Cause::load(r)?),
            n => return Err(SaveError::UnknownCode("line cause tag", n as u32)),
        };
        if capacity < 0.0 || !(0.0..=1.0).contains(&condition) {
            return Err(SaveError::Impossible("a line with an impossible rating"));
        }
        // **A line that is up cannot have failed for a reason**, and one
        // that is down without a cause is a fault nobody can send a crew
        // to. Both are contradictions the repair model would read as an
        // outage that never ends.
        if up != cause.is_none() {
            return Err(SaveError::Impossible(
                "a line and its fault disagree about whether it is up",
            ));
        }
        Ok(crate::econ::Line {
            name,
            level,
            serves,
            feeds,
            parent,
            ring_fed,
            capacity,
            condition,
            up,
            cause,
        })
    }
}

impl Store for crate::econ::Grid {
    fn store(&self, w: &mut Writer) {
        w.len(self.lines.len());
        for l in self.lines.iter() {
            l.store(w);
        }
        w.f64(self.loss);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let n = r.count()?;
        let mut lines = Vec::with_capacity(n);
        for _ in 0..n {
            lines.push(crate::econ::Line::load(r)?);
        }
        let loss = r.finite_f64()?;
        // **Every parent and every feed has to be a line that exists.** A
        // dangling parent is a substation whose outage takes nothing with
        // it, which is the whole of what makes the grid a hierarchy rather
        // than a list — and an external review named exactly this as the
        // risk in folding two national networks into one state.
        for l in lines.iter() {
            if let Some(p) = l.parent {
                if p >= lines.len() {
                    return Err(SaveError::Impossible(
                        "a line fed by a line that is not there",
                    ));
                }
            }
            for &f in l.feeds.iter() {
                if f >= lines.len() {
                    return Err(SaveError::Impossible(
                        "a line feeding a line that is not there",
                    ));
                }
            }
        }
        if !(0.0..1.0).contains(&loss) {
            return Err(SaveError::Impossible("a grid losing all of its power"));
        }
        Ok(crate::econ::Grid { lines, loss })
    }
}

impl Store for crate::econ::Utility {
    fn store(&self, w: &mut Writer) {
        w.str(&self.name);
        w.len(self.serves.len());
        for &m in self.serves.iter() {
            w.len(m);
        }
        w.len(self.spares);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let name = r.str()?;
        let n = r.count()?;
        let mut serves = Vec::with_capacity(n);
        for _ in 0..n {
            serves.push(r.read_len()?);
        }
        Ok(crate::econ::Utility {
            name,
            serves,
            spares: r.read_len()?,
        })
    }
}

impl Store for crate::econ::Incident {
    fn store(&self, w: &mut Writer) {
        self.what.store(w);
        w.u64(self.occurred);
        for d in [self.reported, self.dispatched, self.arrives, self.resolved] {
            match d {
                None => w.u8(0),
                Some(day) => {
                    w.u8(1);
                    w.u64(day);
                }
            }
        }
        w.u64(self.work_days);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let what = Fault::load(r)?;
        let occurred = r.u64()?;
        let day = |r: &mut Reader| -> Result<Option<u64>, SaveError> {
            Ok(match r.u8()? {
                0 => None,
                1 => Some(r.u64()?),
                n => return Err(SaveError::UnknownCode("incident date tag", n as u32)),
            })
        };
        let reported = day(r)?;
        let dispatched = day(r)?;
        let arrives = day(r)?;
        let resolved = day(r)?;
        // **A repair happens in an order.** Reported before sent for, sent
        // for before arrived, and nothing before it broke. A save that
        // says otherwise has a crew arriving at a fault nobody had told
        // them about.
        let mut last = occurred;
        for d in [reported, dispatched, arrives, resolved]
            .into_iter()
            .flatten()
        {
            if d < last {
                return Err(SaveError::Impossible(
                    "a repair whose steps happened out of order",
                ));
            }
            last = d;
        }
        Ok(crate::econ::Incident {
            what,
            occurred,
            reported,
            dispatched,
            arrives,
            work_days: r.u64()?,
            resolved,
        })
    }
}

impl Store for crate::econ::Response {
    fn store(&self, w: &mut Writer) {
        w.bool(self.comms_up);
        w.len(self.crews);
        w.f64(self.depot_km);
        w.f64(self.crew_speed_kmh);
        w.f64(self.working_hours);
        w.u64(self.repair_days);
        w.len(self.spare_transformers);
        w.u64(self.transformer_lead_days);
        w.len(self.utilities.len());
        for u in self.utilities.iter() {
            u.store(w);
        }
        w.len(self.incidents.len());
        for i in self.incidents.iter() {
            i.store(w);
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let comms_up = r.bool()?;
        let crews = r.read_len()?;
        let depot_km = r.finite_f64()?;
        let crew_speed_kmh = r.finite_f64()?;
        let working_hours = r.finite_f64()?;
        let repair_days = r.u64()?;
        let spare_transformers = r.read_len()?;
        let transformer_lead_days = r.u64()?;
        let n = r.count()?;
        let mut utilities = Vec::with_capacity(n);
        for _ in 0..n {
            utilities.push(crate::econ::Utility::load(r)?);
        }
        let n = r.count()?;
        let mut incidents = Vec::with_capacity(n);
        for _ in 0..n {
            incidents.push(crate::econ::Incident::load(r)?);
        }
        if crew_speed_kmh < 0.0 || working_hours < 0.0 || depot_km < 0.0 {
            return Err(SaveError::Impossible("a repair crew with a negative rate"));
        }
        Ok(crate::econ::Response {
            comms_up,
            crews,
            depot_km,
            crew_speed_kmh,
            working_hours,
            repair_days,
            spare_transformers,
            utilities,
            transformer_lead_days,
            incidents,
        })
    }
}

// =====================================================================
// who works, who governs, who hauls
// =====================================================================

impl Store for crate::labour::Workforce {
    fn store(&self, w: &mut Writer) {
        for v in [
            self.supervisory_posts,
            self.posts,
            self.working,
            self.hands,
            self.unemployment,
            self.wage_index,
            self.food_anchor,
        ] {
            w.f64(v);
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let w = crate::labour::Workforce {
            supervisory_posts: r.finite_f64()?,
            posts: r.finite_f64()?,
            working: r.finite_f64()?,
            hands: r.finite_f64()?,
            unemployment: r.finite_f64()?,
            wage_index: r.finite_f64()?,
            food_anchor: r.finite_f64()?,
        };
        if w.posts < 0.0 || w.hands < 0.0 || w.working < 0.0 {
            return Err(SaveError::Impossible("a workforce of negative people"));
        }
        if !(0.0..=1.0).contains(&w.unemployment) {
            return Err(SaveError::Impossible(
                "an unemployment rate outside nought to one",
            ));
        }
        Ok(w)
    }
}

impl Store for crate::state::Government {
    fn store(&self, w: &mut Writer) {
        self.capacity.store(w);
        w.f64(self.revenue);
        for v in self.funded.iter() {
            w.f64(*v);
        }
        w.len(self.posts.len());
        for p in self.posts.iter() {
            w.f64(*p);
        }
        w.f64(self.supplied);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let capacity = crate::state::Capacity::load(r)?;
        let revenue = r.finite_f64()?;
        let mut funded = [0.0f64; 6];
        for v in funded.iter_mut() {
            *v = r.finite_f64()?;
        }
        let n = r.count()?;
        let mut posts = Vec::with_capacity(n);
        for _ in 0..n {
            posts.push(r.finite_f64()?);
        }
        if posts.iter().any(|&p| p < 0.0) {
            return Err(SaveError::Impossible("a negative number of public posts"));
        }
        Ok(crate::state::Government {
            capacity,
            revenue,
            funded,
            posts,
            supplied: r.finite_f64()?,
        })
    }
}

impl Store for crate::services::Services {
    fn store(&self, w: &mut Writer) {
        w.len(self.posts.len());
        for p in self.posts.iter() {
            for v in p.iter() {
                w.f64(*v);
            }
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let n = r.count()?;
        let mut posts = Vec::with_capacity(n);
        for _ in 0..n {
            let mut row = [0.0f64; 4];
            for v in row.iter_mut() {
                *v = r.finite_f64()?;
                if *v < 0.0 {
                    return Err(SaveError::Impossible("a negative number of service posts"));
                }
            }
            posts.push(row);
        }
        Ok(crate::services::Services { posts })
    }
}

impl Store for crate::logistics::Carrier {
    fn store(&self, w: &mut Writer) {
        w.str(&self.name);
        w.len(self.home);
        for v in [
            self.vehicles,
            self.capacity_t_km,
            self.rate,
            self.hauled_t,
            self.worked_t_km,
            self.revenue,
            self.lifetime_t,
        ] {
            w.f64(v);
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let c = crate::logistics::Carrier {
            name: r.str()?,
            home: r.read_len()?,
            vehicles: r.finite_f64()?,
            capacity_t_km: r.finite_f64()?,
            rate: r.finite_f64()?,
            hauled_t: r.finite_f64()?,
            worked_t_km: r.finite_f64()?,
            revenue: r.finite_f64()?,
            lifetime_t: r.finite_f64()?,
        };
        if c.vehicles < 0.0 || c.capacity_t_km < 0.0 || c.lifetime_t < 0.0 {
            return Err(SaveError::Impossible("a haulier with a negative fleet"));
        }
        Ok(c)
    }
}

// =====================================================================
// what the roads have been promised
// =====================================================================

/// **The bookings, keyed by the road and not by where it sits.**
///
/// This is the field `RouteId` was built for. Written down against a vector
/// position it would reload into a world whose roads were built in a
/// different order and name a different stretch of tarmac, with the tonnage
/// and the money both conserving.
impl Store for crate::quote::Reservations {
    fn store(&self, w: &mut Writer) {
        w.len(self.iter().count());
        for ((road, day), tonnes) in self.iter() {
            road.store(w);
            w.u64(day);
            w.f64(tonnes);
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let n = r.count()?;
        let mut booked = crate::quote::Reservations::new();
        for _ in 0..n {
            let road = crate::quote::RouteId::load(r)?;
            let day = r.u64()?;
            let tonnes = r.finite_f64()?;
            if tonnes < 0.0 {
                return Err(SaveError::Impossible("a negative booking on a road"));
            }
            booked.book(road, day, tonnes);
        }
        Ok(booked)
    }
}

// =====================================================================
// the economy
// =====================================================================

/// **The whole economic root, and what is deliberately left out of it.**
///
/// Three fields are not written down, and each for the same reason: it is
/// derived, so storing it would keep a second copy that has to agree with
/// the first and can silently stop agreeing.
///
/// - `routing` — the all-pairs freight table, rebuilt by `resurvey`.
/// - `logistics.cost` / `logistics.km` — the haulier's own distance
///   matrices, rebuilt by `Logistics::survey`. The **carriers** are stored,
///   because their accumulated revenue, tonnage and work are history rather
///   than a cache.
/// - `treasury.flows` — a reporting tally of the day, keyed by
///   `&'static str`, rebuilt as the day is played.
///
/// Both rebuilds happen inside `load`, so what comes back is a working
/// economy rather than one that needs a caller to remember two calls.
impl Store for crate::econ::Economy {
    fn store(&self, w: &mut Writer) {
        self.ledger.store(w);
        self.journal.store(w);

        w.len(self.markets.len());
        for m in self.markets.iter() {
            m.store(w);
        }
        w.len(self.routes.len());
        for r in self.routes.iter() {
            r.store(w);
        }
        w.u64(self.next_route_id);

        self.grid.store(w);
        self.response.store(w);
        w.u64(self.weather_seed);

        w.len(self.road_condition.len());
        for v in self.road_condition.iter() {
            w.f64(*v);
        }
        w.len(self.maintenance_funding.len());
        for v in self.maintenance_funding.iter() {
            w.f64(*v);
        }
        w.f64(self.unserved_power);
        store_basket(w, &self.unmet_demand);

        w.len(self.workforce.len());
        for f in self.workforce.iter() {
            f.store(w);
        }

        match &self.government {
            None => w.u8(0),
            Some(g) => {
                w.u8(1);
                g.store(w);
            }
        }
        match &self.services {
            None => w.u8(0),
            Some(s) => {
                w.u8(1);
                s.store(w);
            }
        }
        // Only the carriers. The matrices are rebuilt.
        match &self.logistics {
            None => w.u8(0),
            Some(l) => {
                w.u8(1);
                w.len(l.carriers.len());
                for c in l.carriers.iter() {
                    c.store(w);
                }
            }
        }

        self.treasury.store(w);
        match self.told_the_day {
            None => w.u8(0),
            Some(d) => {
                w.u8(1);
                w.u64(d);
            }
        }
        match &self.opening {
            None => w.u8(0),
            Some(o) => {
                w.u8(1);
                o.store(w);
            }
        }
        match self.power_clearing {
            None => w.u8(0),
            Some(p) => {
                w.u8(1);
                w.f64(p);
            }
        }
        // Four switches, written as four bits of one byte rather than four
        // bytes, and read back the same way. They are scaffolding and will
        // come out; the codec says so rather than pretending otherwise.
        let x = &self.experiments;
        w.u8((x.carrier_landed_cost as u8)
            | ((x.marginal_source_pricing as u8) << 1)
            | ((x.cheapest_delivered_supplier as u8) << 2)
            | ((x.market_wide_trade as u8) << 3));

        w.len(self.import_duty.len());
        for (nation, rate) in self.import_duty.iter() {
            w.u16(*nation);
            w.f64(*rate);
        }
        self.reservations.store(w);
        self.shipments.store(w);

        // The day's own working figures. A save taken mid-day has these
        // part-filled, and a load that zeroed them would let the rest of
        // the day be played twice over.
        w.len(self.arrivals.len());
        for (qty, paid) in self.arrivals.iter() {
            store_basket(w, qty);
            store_basket(w, paid);
        }
        for v in [
            &self.staff_today,
            &self.payroll_met,
            &self.building_stock,
            &self.building_condition,
        ] {
            w.len(v.len());
            for x in v.iter() {
                w.f64(*x);
            }
        }
        w.f64(self.state_afford);
    }

    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let ledger = crate::econ::Ledger::load(r)?;
        let journal = crate::econ::Journal::load(r)?;

        let n = r.count()?;
        let mut markets = Vec::with_capacity(n);
        for _ in 0..n {
            markets.push(Market::load(r)?);
        }
        let n = r.count()?;
        let mut routes = Vec::with_capacity(n);
        for _ in 0..n {
            routes.push(Route::load(r)?);
        }
        let next_route_id = r.u64()?;

        let grid = crate::econ::Grid::load(r)?;
        let response = crate::econ::Response::load(r)?;
        let weather_seed = r.u64()?;

        let n = r.count()?;
        let mut road_condition = Vec::with_capacity(n);
        for _ in 0..n {
            road_condition.push(r.finite_f64()?);
        }
        let n = r.count()?;
        let mut maintenance_funding = Vec::with_capacity(n);
        for _ in 0..n {
            maintenance_funding.push(r.finite_f64()?);
        }
        let unserved_power = r.finite_f64()?;
        let unmet_demand = load_basket(r)?;

        let n = r.count()?;
        let mut workforce = Vec::with_capacity(n);
        for _ in 0..n {
            workforce.push(crate::labour::Workforce::load(r)?);
        }

        let government = match r.u8()? {
            0 => None,
            1 => Some(crate::state::Government::load(r)?),
            n => return Err(SaveError::UnknownCode("government tag", n as u32)),
        };
        let services = match r.u8()? {
            0 => None,
            1 => Some(crate::services::Services::load(r)?),
            n => return Err(SaveError::UnknownCode("services tag", n as u32)),
        };
        let carriers = match r.u8()? {
            0 => None,
            1 => {
                let n = r.count()?;
                let mut cs = Vec::with_capacity(n);
                for _ in 0..n {
                    cs.push(crate::logistics::Carrier::load(r)?);
                }
                Some(cs)
            }
            n => return Err(SaveError::UnknownCode("logistics tag", n as u32)),
        };

        let treasury = crate::money::Treasury::load(r)?;
        let told_the_day = match r.u8()? {
            0 => None,
            1 => Some(r.u64()?),
            n => return Err(SaveError::UnknownCode("told-the-day tag", n as u32)),
        };
        let opening = match r.u8()? {
            0 => None,
            1 => Some(Opening::load(r)?),
            n => return Err(SaveError::UnknownCode("opening snapshot tag", n as u32)),
        };
        let power_clearing = match r.u8()? {
            0 => None,
            1 => Some(r.finite_f64()?),
            n => return Err(SaveError::UnknownCode("power clearing tag", n as u32)),
        };
        let bits = r.u8()?;
        if bits & 0xF0 != 0 {
            return Err(SaveError::UnknownCode("experiment switches", bits as u32));
        }
        let experiments = crate::econ::Experiments {
            carrier_landed_cost: bits & 1 != 0,
            marginal_source_pricing: bits & 2 != 0,
            cheapest_delivered_supplier: bits & 4 != 0,
            market_wide_trade: bits & 8 != 0,
        };

        let n = r.count()?;
        let mut import_duty = std::collections::BTreeMap::new();
        for _ in 0..n {
            let nation = r.u16()?;
            let rate = r.finite_f64()?;
            if rate < 0.0 {
                return Err(SaveError::Impossible("a negative import duty"));
            }
            if import_duty.insert(nation, rate).is_some() {
                return Err(SaveError::Impossible("one nation charged duty twice"));
            }
        }
        let reservations = crate::quote::Reservations::load(r)?;
        let shipments = crate::registry::Registry::<crate::shipment::Shipment>::load(r)?;

        let n = r.count()?;
        let mut arrivals = Vec::with_capacity(n);
        for _ in 0..n {
            let qty = load_basket(r)?;
            let paid = load_basket(r)?;
            arrivals.push((qty, paid));
        }
        let read_row = |r: &mut Reader| -> Result<Vec<f64>, SaveError> {
            let n = r.count()?;
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push(r.finite_f64()?);
            }
            Ok(v)
        };
        let staff_today = read_row(r)?;
        let payroll_met = read_row(r)?;
        let building_stock = read_row(r)?;
        let building_condition = read_row(r)?;
        let state_afford = r.finite_f64()?;

        // -------------------------------------------------------------
        // every reference has to point at something that is there
        // -------------------------------------------------------------
        //
        // **This is where a dangling index becomes a wrong answer rather
        // than a crash.** A site in a market that does not exist has its
        // goods counted into a town that is not there; a booking on a road
        // nobody built is capacity promised out of nothing; a shipment
        // consigned from a works that has gone delivers goods from
        // nowhere. None of them panics — each one quietly makes a figure
        // wrong somewhere else, which is the failure mode this whole file
        // exists to refuse.
        let n_markets = markets.len();
        let n_sites = ledger.sites.len();
        for s in ledger.sites.iter() {
            if s.market >= n_markets {
                return Err(SaveError::Impossible("a works in a town that is not there"));
            }
        }
        for road in routes.iter() {
            if road.a >= n_markets || road.b >= n_markets {
                return Err(SaveError::Impossible("a road to a town that is not there"));
            }
            if road.id.0 >= next_route_id {
                return Err(SaveError::Impossible(
                    "a road named at or past the next unused name",
                ));
            }
            if let Some((_, to, _)) = road.moved {
                if to >= n_markets {
                    return Err(SaveError::Impossible(
                        "a road carrying goods to a town that is not there",
                    ));
                }
            }
        }
        // **Two roads with one name is the defect `RouteId` exists to
        // prevent**, arriving through a file rather than through a vector.
        let mut names: Vec<u64> = routes.iter().map(|x| x.id.0).collect();
        names.sort_unstable();
        let unique = names.len();
        names.dedup();
        if names.len() != unique {
            return Err(SaveError::Impossible("two roads with the same name"));
        }
        let named: std::collections::BTreeSet<crate::quote::RouteId> =
            routes.iter().map(|x| x.id).collect();
        for ((road, _), _) in reservations.iter() {
            if !named.contains(&road) {
                return Err(SaveError::Impossible(
                    "capacity booked on a road that is not there",
                ));
            }
        }
        for (_, s) in shipments.iter() {
            if s.consignor >= n_sites || s.consignee >= n_sites || s.carrier >= n_sites {
                return Err(SaveError::Impossible(
                    "a consignment between works that are not there",
                ));
            }
            if s.from_market >= n_markets || s.to_market >= n_markets {
                return Err(SaveError::Impossible(
                    "a consignment between towns that are not there",
                ));
            }
        }
        // **Road condition is per nation, not per road.** A country
        // maintains its network or does not, on its own account, so the
        // index is the nation — which is worth stating because the first
        // version of this check compared it against the number of roads
        // and would have refused every world with more than one country in
        // it.
        let nations = markets
            .iter()
            .map(|m| m.nation as usize)
            .max()
            .map_or(0, |n| n + 1);
        if road_condition.len() != nations || maintenance_funding.len() != nations {
            return Err(SaveError::Impossible(
                "a road-maintenance figure for a country that is not there",
            ));
        }
        if workforce.len() != n_markets {
            return Err(SaveError::Impossible(
                "a workforce for a town that is not there",
            ));
        }
        if let Some(o) = &opening {
            if o.price.len() != n_markets {
                return Err(SaveError::Impossible(
                    "an opening snapshot of a different country",
                ));
            }
        }

        let mut economy = crate::econ::Economy {
            ledger,
            journal,
            markets,
            routes,
            next_route_id,
            grid,
            response,
            weather_seed,
            road_condition,
            maintenance_funding,
            unserved_power,
            unmet_demand,
            workforce,
            government,
            services,
            logistics: None,
            treasury,
            told_the_day,
            opening,
            power_clearing,
            experiments,
            routing: crate::quote::Routing::default(),
            reservations,
            import_duty,
            shipments,
            arrivals,
            staff_today,
            payroll_met,
            state_afford,
            building_stock,
            building_condition,
        };

        // **The derived tables, rebuilt rather than trusted.** A load hands
        // back a working economy, not one that needs the caller to remember
        // two more calls.
        economy.resurvey();
        if let Some(carriers) = carriers {
            let mut freight = crate::logistics::Logistics::found(&economy);
            freight.carriers = carriers;
            freight.survey(&economy);
            economy.logistics = Some(freight);
        }
        Ok(economy)
    }
}
