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
        let v = r.finite_f64()?;
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
