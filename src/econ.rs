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
    /// Animals on the hoof, in tonnes of live weight. **Alive, so it does
    /// not spoil** — which is exactly why it was moved live for most of
    /// history, and why a stockyard sat next to every city.
    Livestock,
    /// Butchered meat, in tonnes of retail cuts. **The whole reason a
    /// cold chain exists.**
    Meat,
    /// **Iron ore**, in tonnes. Mined only where the geology put metal —
    /// the field `geology.rs` has generated since it was written and
    /// nothing had ever asked for.
    IronOre,
    /// **Crude steel**, in tonnes. The intermediate everything physical
    /// passes through: a tin can, a plough, a lorry, a substation.
    ///
    /// **Coal here is not fuel, it is the reductant.** A blast furnace
    /// uses carbon to strip the oxygen off iron oxide, which is why the
    /// real recipe is 1.4 t of ore and 0.8 t of coal but only 200-300 kWh
    /// of *electricity* per tonne. A country with unlimited power and no
    /// coal still cannot make primary steel — which is the whole reason
    /// steel is hard to decarbonise, and a dependency worth having.
    Steel,
}

pub const N_COMMODITIES: usize = 10;

impl Commodity {
    pub const ALL: [Commodity; N_COMMODITIES] = [
        Commodity::Grain,
        Commodity::Flour,
        Commodity::ProcessedFood,
        Commodity::Electricity,
        Commodity::Coal,
        Commodity::RetailGoods,
        Commodity::Livestock,
        Commodity::Meat,
        Commodity::IronOre,
        Commodity::Steel,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Commodity::Grain => "grain",
            Commodity::Flour => "flour",
            Commodity::ProcessedFood => "food",
            Commodity::Electricity => "power",
            Commodity::Coal => "coal",
            Commodity::RetailGoods => "goods",
            Commodity::Livestock => "stock",
            Commodity::Meat => "meat",
            Commodity::IronOre => "ore",
            Commodity::Steel => "steel",
        }
    }

    /// **How long it keeps before it is no longer food**, in days.
    ///
    /// Real: fresh meat at ambient temperature is finished in **one to two
    /// days**; chilled and vacuum-packed it keeps **four to six weeks**;
    /// frozen, six months to a year. Grain keeps for years in a dry silo,
    /// flour about one, and a can indefinitely — which is what canning is
    /// *for*.
    ///
    /// **This is the whole reason refrigeration matters.** Before
    /// refrigerated shipping — the *Dunedin* carried frozen lamb from New
    /// Zealand to London in 1882 — meat was eaten where it was killed or
    /// it was salted. A cold chain turns a local product into a traded
    /// one.
    pub fn shelf_life_days(self, refrigerated: bool) -> Option<f64> {
        match self {
            Commodity::Meat => Some(if refrigerated { 30.0 } else { 2.0 }),
            Commodity::Grain => Some(730.0),
            Commodity::Flour => Some(365.0),
            Commodity::ProcessedFood => Some(1_825.0),
            // Livestock is alive, and electricity, coal and dry goods do
            // not rot.
            _ => None,
        }
    }

    /// **What share of a stockpile is lost each day.**
    ///
    /// *Not* one over the shelf life, which was the first thing tried and
    /// is wrong: a shelf life says how long something stays *good*, while
    /// a loss rate says how fast a store leaks. Treating them as the same
    /// destroyed **40% of a nation's grain a year** and starved a country
    /// that had a full silo.
    ///
    /// Real figures. Grain in a decent silo loses **1-2% a year** to
    /// insects, rodents and moisture — 10-20% where storage is poor, which
    /// is a real and enormous problem in hot countries. Flour keeps less
    /// well. A can loses nothing worth counting.
    ///
    /// **Meat is the outlier and the reason any of this exists**: at
    /// ambient temperature it is finished in a day or two, and chilled it
    /// keeps four to six weeks. That gap is what a cold chain buys.
    pub fn spoilage_per_day(self, refrigerated: bool) -> f64 {
        match self {
            Commodity::Meat => {
                if refrigerated {
                    1.0 / 30.0
                } else {
                    0.5
                }
            }
            Commodity::Grain => 0.02 / 365.0,
            Commodity::Flour => 0.05 / 365.0,
            Commodity::ProcessedFood => 0.005 / 365.0,
            _ => 0.0,
        }
    }

    /// Whether keeping it needs power, and therefore whether a blackout
    /// destroys it rather than merely inconveniencing it.
    pub fn needs_cold(self) -> bool {
        self == Commodity::Meat
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
            // A staple in poor countries and a luxury in rich ones; it
            // gives more readily than bread does.
            Commodity::Meat => -0.6,
            Commodity::Livestock => -0.5,
            // **Industrial demand does not respond to price the way a
            // household does.** A steelworks needs 1.4 tonnes of ore per
            // tonne of steel whatever ore costs — there is no substitute
            // and no doing without, which is why raw-material prices swing
            // so violently on small changes in supply.
            Commodity::IronOre => -0.15,
            Commodity::Steel => -0.35,
        }
    }

    /// Annual household consumption per person, in this commodity's unit.
    /// Zero for things households do not buy directly. Spec A.1.
    pub fn per_capita_annual(self) -> f64 {
        match self {
            Commodity::ProcessedFood => 0.40,
            // **Real world average meat consumption is ~43 kg a head a
            // year**, against roughly 150 kg of cereals — a fifth of the
            // diet by weight and rather more of its cost.
            Commodity::Meat => 0.043,
            // **Residential only, and that is the whole point.** Real
            // world electricity is ~3.5 MWh a head a year *in total*, of
            // which households take about 0.9 — industry is ~42% and
            // commerce most of the rest. 3.0 stood in for the entire
            // economy's demand back when no industry was modelled, and the
            // moment factories and a steelworks existed the country
            // counted them twice: household demand alone came to 271,000
            // MWh a day against industry's 4,000, so the station burnt
            // every tonne raised to supply it and the steelworks on the
            // same coalfield never smelted anything.
            Commodity::Electricity => 0.9,
            Commodity::RetailGoods => 1.0,
            // Nobody buys a tonne of crude steel. It reaches a household
            // inside a cooker, a car and a tin, which is what makes it an
            // *industrial* demand and why pricing it off household
            // purchases would give it no price at all.
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
            // Grain is harvested once and eaten for twelve months, so a
            // working stock is months rather than weeks. Judging it against
            // a few weeks' cover prices it as a catastrophe every spring.
            Commodity::Grain => 150.0,
            Commodity::Coal => 20.0,
            Commodity::RetailGoods => 14.0,
            // **A butcher holds days, not weeks**, and that is not a
            // choice — it is the shelf life. This is the number that makes
            // a cold chain worth building.
            Commodity::Meat => 3.0,
            // Stock is held on the hoof and keeps itself.
            Commodity::Livestock => 30.0,
            // Bulk raw material, stockpiled at the works. Real steelworks
            // hold weeks of ore and coal against a shipping interruption,
            // which is exactly what a strategic stockpile is.
            Commodity::IronOre => 30.0,
            Commodity::Steel => 25.0,
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
            // Real: beef runs about 4,000-6,000 a tonne wholesale against
            // grain's 200-250, which is the tenfold-plus premium meat
            // carries for the feed and the years that went into it.
            Commodity::Meat => 4_200.0,
            Commodity::Livestock => 1_600.0,
            // Real: iron ore runs $80-120 a tonne delivered, crude steel
            // $500-700. The gap between them is the whole of a steel
            // industry.
            Commodity::IronOre => 90.0,
            Commodity::Steel => 450.0,
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
    /// Grazing. Not a farm: different labour, different water dependency,
    /// and it can use ground no plough would touch.
    Pasture,
    /// **Where live weight becomes meat**, and the first works whose
    /// output a blackout destroys rather than merely delays.
    Butcher,
    /// Coal workings. Only exists where the geology put a deposit.
    Mine,
    Mill,
    Factory,
    /// **Where the metal comes out of the ground.** Only exists where the
    /// geology put ore, exactly like the colliery.
    IronMine,
    /// **Ore and coal in, steel out.** Sited on a coalfield, on an
    /// orefield, or on tidewater where both can be landed — which is the
    /// real history of the industry in one line.
    Steelworks,
    /// **Where steel becomes things**: tools, parts, tins, machines.
    Works,
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
    /// Batches actually run today, against `throughput` rated.
    ///
    /// A works standing idle for want of power, inputs or somewhere to put
    /// the output still has its people; how long it keeps them is a
    /// different question, and this is the number that decides it.
    pub ran: f64,
    /// How the place is fitted out, where that matters.
    ///
    /// A shop's tills and shelving and loading bays decide how many people
    /// work in it — the biggest single employer in the economy, and one
    /// that used to employ nobody at all because a shop was a stockpile
    /// with a name.
    pub fitted: Option<crate::building::Building>,
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

    /// Count the opening stock of sites added after construction.
    ///
    /// Needed when several economies are folded into one: the incomers
    /// arrive holding goods, and without this the conservation check
    /// reports every tonne of it as having appeared from nowhere.
    pub fn absorb_opening_stock(&mut self, from_site: usize) {
        for s in &self.sites[from_site..] {
            for c in 0..N_COMMODITIES {
                self.opening_total[c] += s.stock[c];
            }
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

pub const RECIPES: [Recipe; 16] = [
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
        // **A can is made of steel**, and canning was born of the tinplate
        // industry rather than alongside it. 35 kg of steel per tonne of
        // canned food puts metal packaging at ~14 kg a head a year, which
        // is what developed countries actually get through.
        inputs: &[(Commodity::Flour, 0.9), (Commodity::Steel, 0.035)],
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
    // Grain bought from outside the region and landed at a port.
    //
    // **A nation whose land cannot feed it does not simply starve.** It
    // buys, as it already does with fuel, and in doing so acquires the
    // oldest dependency there is: the grain fleet. Every empire that
    // outgrew its own fields has lived on one, and every one of them has
    // been strangled by somebody who understood that.
    Recipe {
        name: "grain imports",
        inputs: &[],
        outputs: &[(Commodity::Grain, 1.0)],
        power: 0.02,
        labour: 0.05,
        needs_water: false,
    },
    // **Stock on grass.** Very low power and very high labour per tonne
    // against arable — real extensive grazing runs one stockman to
    // several hundred head — and it needs water daily, which is the same
    // dependency a farm has and a different one from a factory's.
    Recipe {
        name: "pasture",
        inputs: &[],
        outputs: &[(Commodity::Livestock, 1.0)],
        power: 0.02,
        labour: 60.0,
        needs_water: true,
    },
    // **Live weight to retail meat.** Real dressing: a 450 kg beast gives
    // about 56% as carcass and 70% of that boned out, so roughly 2.6
    // tonnes on the hoof for a tonne on the counter.
    //
    // The power is mostly chilling — a real meat plant runs 150-250 kWh a
    // tonne — which is why this is the one works whose output is destroyed
    // by a blackout rather than merely delayed by it.
    Recipe {
        name: "butcher",
        inputs: &[(Commodity::Livestock, 2.6)],
        outputs: &[(Commodity::Meat, 1.0)],
        power: 0.25,
        labour: 3.0,
        needs_water: false,
    },
    // **Meat landed from outside the region**, which is a thing that only
    // exists because of refrigerated shipping. The *Dunedin* carried
    // frozen lamb from New Zealand to London in 1882 and created this
    // trade; before it, a country short of meat ate less meat.
    //
    // It draws power for the same reason a butcher does — the cold store
    // on the quay — so a blackout at the port is a blackout in the
    // nation's meat supply.
    Recipe {
        name: "meat imports",
        inputs: &[],
        outputs: &[(Commodity::Meat, 1.0)],
        power: 0.10,
        labour: 0.05,
        needs_water: false,
    },
    // -----------------------------------------------------------------
    // Ore, steel, and the things made of it.
    //
    // Until this existed, goods appeared at a depot from nowhere and the
    // metal `geology.rs` had been placing since it was written had no
    // consumer at all.
    // -----------------------------------------------------------------
    Recipe {
        name: "iron mine",
        inputs: &[],
        outputs: &[(Commodity::IronOre, 1.0)],
        power: 0.08,
        // **Ore mining barely employs anybody.** The Pilbara moves ~900 Mt
        // a year with about 60,000 people — 15,000 tonnes each — which is
        // why an ore province can be enormously valuable and still not be
        // a place many people live.
        labour: 0.4,
        needs_water: false,
    },
    // A nation with no orefield buys ore, exactly as one with no coalfield
    // buys fuel. Japan and Korea run world-class steel industries on
    // entirely imported ore and coal, so this is not a poor country's
    // recipe — it is the normal one.
    Recipe {
        name: "ore imports",
        inputs: &[],
        outputs: &[(Commodity::IronOre, 1.0)],
        power: 0.02,
        labour: 0.15,
        needs_water: false,
    },
    Recipe {
        name: "steelworks",
        // **Real BF-BOF figures: 1.4 t of ore and 0.8 t of coal per tonne
        // of crude steel.** The coal is doing chemistry, not just heating.
        inputs: &[(Commodity::IronOre, 1.4), (Commodity::Coal, 0.8)],
        outputs: &[(Commodity::Steel, 1.0)],
        // **Only 200-300 kWh a tonne of *electricity*** — the 24 GJ of
        // total energy is mostly the coal itself. Which is why a steelworks
        // is a huge consumer of fuel and a modest one of grid power.
        power: 0.25,
        // Basic materials are capital-intensive; it is *fabrication* that
        // employs people. A modern mill runs 0.5-2 person-hours a tonne.
        labour: 1.5,
        needs_water: false,
    },
    Recipe {
        name: "factory",
        // Backed out of the real figure rather than chosen: **world crude
        // steel is ~230 kg a head a year**, and households take a tonne of
        // goods each, so a tonne of manufactured output carries 0.23 t of
        // steel.
        inputs: &[(Commodity::Steel, 0.23)],
        outputs: &[(Commodity::RetailGoods, 1.0)],
        power: 1.0,
        // **And this is where manufacturing employment actually is.** A
        // car is 1.5 t and takes something like a hundred person-hours
        // once its parts are counted; at ~55 hours a tonne the sector
        // lands near its real 7.6% of the workforce, against the
        // steelworks' 1.5. Fabrication is the labour, not the metal.
        labour: 55.0,
        needs_water: false,
    },
    // **A steel stockholder.** Most economies do not smelt their own —
    // there are about fifty countries with a steel industry and two
    // hundred without — and a small one buys plate and bar from a service
    // centre like any other input. The same shape as fuel and ore
    // imports, and the same dependency.
    Recipe {
        name: "steel imports",
        inputs: &[],
        outputs: &[(Commodity::Steel, 1.0)],
        power: 0.02,
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
    pub const GRAIN_IMPORTS: usize = 7;
    pub const PASTURE: usize = 8;
    pub const BUTCHER: usize = 9;
    pub const MEAT_IMPORTS: usize = 10;
    pub const IRON_MINE: usize = 11;
    pub const ORE_IMPORTS: usize = 12;
    pub const STEELWORKS: usize = 13;
    pub const FACTORY: usize = 14;
    pub const STEEL_IMPORTS: usize = 15;
}

// ---------------------------------------------------------------------------
// Markets
// ---------------------------------------------------------------------------

pub struct Market {
    pub name: String,
    /// Which nation this market belongs to. Weather is drawn per nation,
    /// and a lane between two nations is a different thing from a road
    /// inside one.
    pub nation: u16,
    /// People fed from this market.
    pub population: f64,
    /// Southern hemisphere, so its farming year runs six months out of
    /// step with a northern one. This is what makes seasonal trade between
    /// hemispheres possible: one country's lean season is another's
    /// harvest.
    pub southern: bool,
    /// This year's growing conditions here, as a multiplier on the
    /// harvest. Weather is regional, so markets of the same nation share a
    /// draw.
    pub harvest_quality: f64,
    pub price: Basket,
    /// Days of cover currently held, for reporting.
    pub cover: Basket,
    /// Cover as the market *sees* it: a slow average rather than today's
    /// reading. See `update_prices`.
    pub expected_cover: Basket,
}

impl Market {
    pub fn new(name: impl Into<String>, population: f64) -> Self {
        Self::in_nation(name, population, 0, false)
    }

    pub fn in_nation(
        name: impl Into<String>,
        population: f64,
        nation: u16,
        southern: bool,
    ) -> Self {
        let mut price = basket();
        for (i, c) in Commodity::ALL.iter().enumerate() {
            price[i] = c.base_cost();
        }
        let mut expected = basket();
        for (i, c) in Commodity::ALL.iter().enumerate() {
            expected[i] = c.target_cover_days();
        }
        Market {
            name: name.into(),
            nation,
            population,
            southern,
            harvest_quality: 1.0,
            price,
            cover: basket(),
            expected_cover: expected,
        }
    }

    /// Household demand per day, spec A.4. A floor, not a preference:
    /// below it, people go hungry.
    pub fn daily_household_demand(&self, c: Commodity) -> f64 {
        self.population * c.per_capita_annual() / 365.0
    }

    /// Season here today.
    pub fn season(&self, day: u64) -> Season {
        Season::on(day, self.southern)
    }

    /// Today's harvest multiplier here: where the year is, times how the
    /// year has turned out.
    pub fn harvest(&self, day: u64) -> f64 {
        harvest_curve(day, self.southern) * self.harvest_quality
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
/// **Where in the grid a thing sits, which is what decides how many
/// people a fault takes out.**
///
/// A grid is not one wire. The whole point of the hierarchy is that the
/// blast radius of a failure is set by where it happens, and the numbers
/// are not close: real customers affected by a single failure run from
/// one to tens of thousands.
///
/// | | customers off | typical repair |
/// |---|---|---|
/// | service drop | **1** | 2-6 hours |
/// | distribution transformer | 5-50 | 4-8 hours, or weeks if it must be replaced |
/// | feeder | 500-3,000 | 2-6 hours |
/// | primary substation | 10,000-50,000 | hours to days |
/// | transmission circuit | **usually none** | days |
///
/// **Transmission is built N-1**, meaning the network is designed to lose
/// any single circuit without dropping a customer — which is why a
/// pylon coming down is a news item and not a blackout. That is the fact
/// the old model had exactly backwards: it pooled everything into one or
/// two lines, so *any* fault was national.
///
/// Real reliability, for scale: a customer in Britain is off supply about
/// **35 minutes a year** across roughly one interruption; Germany manages
/// 12 minutes, the United States about 90 excluding major storms. Almost
/// all of it is distribution, not transmission and not generation.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Level {
    /// The wire into one building.
    Service,
    /// A pole or pad transformer: a street of them.
    Transformer,
    /// An 11 kV feeder: a district.
    Feeder,
    /// A primary substation: a town.
    Substation,
    /// A transmission circuit. Normally redundant.
    Transmission,
}

impl Level {
    pub fn name(self) -> &'static str {
        match self {
            Level::Service => "service connection",
            Level::Transformer => "distribution transformer",
            Level::Feeder => "feeder",
            Level::Substation => "primary substation",
            Level::Transmission => "transmission circuit",
        }
    }

    /// **How long it takes to put right, in days**, once a crew is on it.
    /// Travel, reporting and spares are handled separately — this is the
    /// work itself.
    pub fn repair_days(self) -> f64 {
        match self {
            Level::Service => 0.2,
            Level::Transformer => 0.4,
            Level::Feeder => 0.25,
            Level::Substation => 1.0,
            Level::Transmission => 3.0,
        }
    }
}

pub struct Line {
    pub name: String,
    /// Where in the grid it sits.
    pub level: Level,
    /// Which market it serves, if it serves only one. `None` for
    /// transmission, which serves everybody.
    pub serves: Option<usize>,
    /// Which sites it feeds. Empty means "everything downstream of the
    /// pool", which is what transmission does.
    pub feeds: Vec<usize>,
    /// What is immediately upstream. **A substation going out takes its
    /// feeders with it** — that is what makes the hierarchy a hierarchy
    /// rather than a list.
    pub parent: Option<usize>,
    /// **Whether it can be back-fed from another direction.**
    ///
    /// Urban distribution is built as an open ring: on a fault the
    /// operator switches and supply is back in minutes, long before
    /// anybody repairs anything. Rural distribution is radial — one wire,
    /// one path — so the village waits for a crew. That difference is why
    /// the same fault is an hour in a city and most of a day in the
    /// country, and it is one of the most real things about a grid.
    pub ring_fed: bool,
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
            level: Level::Transmission,
            serves: None,
            feeds: Vec::new(),
            parent: None,
            // Transmission is meshed and built N-1: lose any single
            // circuit and nobody notices. A pylon down is a news item,
            // not a blackout.
            ring_fed: true,
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

    /// **Hang the distribution network under the transmission.**
    ///
    /// A feeder to each town and a service connection to each works, which
    /// is what gives a fault somewhere to happen that is not national.
    /// Before this the grid was one pool with one or two lines in it, so
    /// the only failure the model could express was "the country goes
    /// dark" — and a line to a house going down took out everybody.
    pub fn wire_up(
        &mut self,
        markets: &[(String, f64)],
        sites: &[(usize, usize, String)],
    ) {
        /// **One primary substation to about thirty thousand people**, and
        /// six or so feeders off each — real distribution planning. A
        /// substation serves 10,000-50,000 customers and a feeder 500-3,000.
        const PEOPLE_PER_SUBSTATION: f64 = 30_000.0;
        const FEEDERS_PER_SUBSTATION: usize = 6;
        /// A city of sixteen million would otherwise want five hundred
        /// substations. The model samples the structure rather than
        /// enumerating it; what matters is that a fault has a size.
        const MOST_SUBSTATIONS: usize = 4;
        /// **Ring-fed above this**, radial below. Real: dense networks are
        /// built as open rings so an operator can switch round a fault;
        /// the countryside gets one wire.
        const RING_FED_ABOVE: f64 = 50_000.0;

        for (m, (name, population)) in markets.iter().enumerate() {
            let n = ((population / PEOPLE_PER_SUBSTATION).ceil() as usize)
                .clamp(1, MOST_SUBSTATIONS);
            for k in 0..n {
                let sub = self.lines.len();
                self.lines.push(Line {
                    name: format!("{name} substation {}", k + 1),
                    level: Level::Substation,
                    serves: Some(m),
                    feeds: Vec::new(),
                    parent: None,
                    // A primary substation carries two transformers, so it
                    // survives losing one of them.
                    ring_fed: true,
                    capacity: 0.0,
                    condition: 1.0,
                    up: true,
                    cause: None,
                });
                for j in 0..FEEDERS_PER_SUBSTATION {
                    // **A feeder takes a share of the town, not all of
                    // it.** Six feeders to a substation means a fault on
                    // one is a sixth of a district in the dark, which is
                    // the difference between an outage and a catastrophe.
                    let mine: Vec<usize> = sites
                        .iter()
                        .filter(|(_, mk, _)| *mk == m)
                        .map(|(site, _, _)| *site)
                        .enumerate()
                        .filter(|(idx, _)| idx % (n * FEEDERS_PER_SUBSTATION)
                            == k * FEEDERS_PER_SUBSTATION + j)
                        .map(|(_, site)| site)
                        .collect();
                    self.lines.push(Line {
                        name: format!("{name} feeder {}-{}", k + 1, j + 1),
                        level: Level::Feeder,
                        serves: Some(m),
                        feeds: mine,
                        parent: Some(sub),
                        ring_fed: *population > RING_FED_ABOVE,
                        capacity: 0.0,
                        condition: 1.0,
                        up: true,
                        cause: None,
                    });
                }
            }
        }

        for (site, market, name) in sites {
            self.lines.push(Line {
                name: format!("{name} supply"),
                level: Level::Service,
                serves: None,
                feeds: vec![*site],
                parent: None,
                // The wire into one building has nothing to switch to.
                ring_fed: false,
                capacity: 0.0,
                condition: 1.0,
                up: true,
                cause: None,
            });
            let _ = market;
        }
    }

    /// Capacity actually available right now.
    /// **How much the grid can carry — transmission only.**
    ///
    /// A cut feeder does not reduce what the country can generate or move;
    /// it disconnects what is behind it. Counting distribution into
    /// capacity is what made a fault anywhere a shortage everywhere.
    pub fn capacity(&self) -> f64 {
        self.lines
            .iter()
            .filter(|l| l.up && l.level == Level::Transmission)
            .map(|l| l.capacity)
            .sum::<f64>()
            * (1.0 - self.loss)
    }

    /// **Whether this site is cut off from the grid**, whatever the grid
    /// as a whole is doing. A downed service connection takes out one
    /// building; a downed feeder takes out the market behind it.
    pub fn cut_off(&self, site: usize, market: usize) -> bool {
        let _ = market;
        (0..self.lines.len()).any(|i| self.out(i) && self.lines[i].feeds.contains(&site))
    }

    /// **Whether a line is actually off supply**, which is not the same as
    /// whether it is broken.
    ///
    /// Two reasons it may be broken and still carrying: it is **ring-fed**,
    /// so the operator switched round the fault and supply was back in
    /// minutes — the repair still has to happen, but nobody sat in the
    /// dark for it. And transmission is meshed and built N-1, so a single
    /// circuit out drops no customers at all.
    ///
    /// One reason it may be intact and still dead: **whatever feeds it is
    /// out**. A substation going down takes its feeders with it, which is
    /// what makes this a hierarchy rather than a list.
    pub fn out(&self, line: usize) -> bool {
        let l = &self.lines[line];
        if !l.up && !l.ring_fed {
            return true;
        }
        match l.parent {
            Some(p) => self.out(p),
            None => false,
        }
    }

    /// Everything below transmission that is currently out.
    pub fn faults_below_transmission(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| !l.up && l.level != Level::Transmission)
            .count()
    }

    /// How many sites are off supply right now, and why. For reporting.
    pub fn dark_sites(&self, n_sites: usize) -> usize {
        (0..n_sites).filter(|&s| self.cut_off(s, 0)).count()
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
/// How a road gets past a mountain barrier — the decision an engineer
/// actually faces, and the one that separates a trunk route from a track.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Crossing {
    /// Open country. Nothing in the way worth naming.
    Level,
    /// Over the top. Cheap to build because it follows the ground, but it
    /// means steep grades, slow heavy traffic, and **snow closes it every
    /// winter** — which is why an economy that depends on one has a
    /// seasonal hole in it.
    Pass {
        /// Height of the summit, 0..1 of the world's relief.
        summit: f64,
        /// Coldest temperature on the crossing, 0..1. Colder closes for
        /// longer.
        cold: f64,
    },
    /// Bored through. Enormous capital cost — tens of millions a kilometre
    /// against two for a road on the flat — bought because a pass cannot
    /// carry the freight. Flat, fast, and open in February.
    Tunnel {
        /// Capital cost, in millions.
        capital: f64,
    },
}

impl Crossing {
    pub fn name(self) -> &'static str {
        match self {
            Crossing::Level => "level",
            Crossing::Pass { .. } => "pass",
            Crossing::Tunnel { .. } => "tunnel",
        }
    }

    /// Whether snow shuts this crossing on a given day.
    ///
    /// A pass high enough and cold enough is simply gone for the winter.
    /// Real alpine passes close for four to six months; lower ones only in
    /// the worst weeks.
    pub fn shut_by_snow(self, season: Season, cold: f64) -> bool {
        let _ = cold;
        match self {
            Crossing::Pass { cold, summit } => {
                // How much of the year it is lost, from how cold and high
                // it is. A high, bitter col is shut all winter and half of
                // spring; a low one only at the depth of it.
                let severity = (1.0 - cold) * summit;
                match season {
                    Season::Winter => severity > 0.18,
                    Season::Spring | Season::Autumn => severity > 0.34,
                    Season::Summer => false,
                }
            }
            _ => false,
        }
    }
}

pub struct Route {
    pub name: String,
    pub a: usize,
    pub b: usize,
    /// Currency per unit moved. Road freight is roughly 12x sea per
    /// tonne-km (spec A.9), which is why the mode matters so much.
    ///
    /// This is what the haul costs *today*, on the road as it currently
    /// is. A neglected network raises it.
    pub freight_cost: f64,
    /// What the haul costs on a road in good repair. `freight_cost` is
    /// this, worsened by however far the surface has been let go.
    pub sound_cost: f64,
    /// Kilometres along the road, which is not the gap between the towns.
    ///
    /// Bulk freight only cares what a tonne costs, but a person has to
    /// actually go, and how long that takes is distance divided by whatever
    /// they are travelling on. Forty-nine kilometres is nothing in a lorry
    /// and two days on foot with a load.
    pub km: f64,
    /// The worst stretch of road anywhere along it.
    ///
    /// Not the average. A lorry is stopped by the one unmade mile, not by
    /// the mean quality of the three hundred either side of it, and this is
    /// the number that decides both what can travel and how fast.
    pub surface: Surface,
    /// How this route gets over whatever is in its way.
    pub crossing: Crossing,
    /// True while a pass is shut by snow. Set each day from the season.
    pub snowed_in: bool,
    /// Units per day the route can carry.
    pub capacity: f64,
    pub open: bool,
    /// What the economy itself shipped along here today, and which way.
    ///
    /// **This is where haulage work comes from.** A driver does not invent
    /// a cargo; he drives the freight that firms are already moving, and
    /// they move it for their own reasons. Inventing loads from inventory
    /// gaps instead had a man shuttling the same grain between the same
    /// two towns for a decade, and tightening the rule to stop that left
    /// him with no work at all.
    pub moved: Option<(Commodity, usize, f64)>,
}

impl Route {
    /// Whether anything can move along this today. A road that is open in
    /// principle but under four metres of snow carries nothing.
    pub fn usable(&self) -> bool {
        self.open && !self.snowed_in
    }
}

/// What somebody is travelling over. Speed depends on it far more than
/// freight cost does.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Surface {
    Water,
    Highway,
    Road,
    Track,
    Open,
}

impl Surface {
    pub fn name(self) -> &'static str {
        match self {
            Surface::Water => "water",
            Surface::Highway => "highway",
            Surface::Road => "road",
            Surface::Track => "track",
            Surface::Open => "open country",
        }
    }
}

// ---------------------------------------------------------------------------
// The calendar
// ---------------------------------------------------------------------------

pub const DAYS_PER_YEAR: u64 = 365;

/// **The week, which the model did not have.**
///
/// A year of 365 days was being lived as 365 identical ones. Real working
/// life is shaped by the week far more sharply than by the season: an
/// office keeps Monday to Friday, a shop is open seven days and is
/// *busiest* at the weekend, and a factory or a hospital runs a rota that
/// does not care what day it is.
///
/// Which is why part-timers and students work weekends — not by accident,
/// but because that is when the trade is and when the full-timers will
/// not.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    /// Day 0 of the calendar is a Monday, which is as arbitrary and as
    /// useful as any other choice.
    pub fn on(day: u64) -> Weekday {
        match day % 7 {
            0 => Weekday::Monday,
            1 => Weekday::Tuesday,
            2 => Weekday::Wednesday,
            3 => Weekday::Thursday,
            4 => Weekday::Friday,
            5 => Weekday::Saturday,
            _ => Weekday::Sunday,
        }
    }

    pub fn is_weekend(self) -> bool {
        matches!(self, Weekday::Saturday | Weekday::Sunday)
    }

    pub fn name(self) -> &'static str {
        match self {
            Weekday::Monday => "Monday",
            Weekday::Tuesday => "Tuesday",
            Weekday::Wednesday => "Wednesday",
            Weekday::Thursday => "Thursday",
            Weekday::Friday => "Friday",
            Weekday::Saturday => "Saturday",
            Weekday::Sunday => "Sunday",
        }
    }

    /// **How much trade a shop does today**, against an average day.
    ///
    /// Real retail footfall peaks on Saturday at something like 1.4-1.6
    /// times a weekday, and Sunday is shorter hours in most of the West.
    /// This is the reason weekend shifts exist at all, and therefore the
    /// reason students and part-timers work them.
    pub fn retail_trade(self) -> f64 {
        match self {
            Weekday::Saturday => 1.50,
            Weekday::Sunday => 1.10,
            Weekday::Friday => 1.10,
            Weekday::Monday | Weekday::Tuesday => 0.85,
            _ => 0.90,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    pub fn name(self) -> &'static str {
        match self {
            Season::Spring => "spring",
            Season::Summer => "summer",
            Season::Autumn => "autumn",
            Season::Winter => "winter",
        }
    }

    /// Season on day `day` at a given latitude sign. The southern
    /// hemisphere is six months out of step with the northern, which is
    /// why a planet can feed itself year-round while any one country
    /// cannot.
    pub fn on(day: u64, southern: bool) -> Season {
        let mut d = day % DAYS_PER_YEAR;
        if southern {
            d = (d + DAYS_PER_YEAR / 2) % DAYS_PER_YEAR;
        }
        match d * 4 / DAYS_PER_YEAR {
            0 => Season::Spring,
            1 => Season::Summer,
            2 => Season::Autumn,
            _ => Season::Winter,
        }
    }
}

/// The farming year.
///
/// Crops are not produced evenly. Ground is prepared and sown, the crop
/// grows, and then almost the whole year's grain arrives inside a few
/// weeks — after which the country lives on what it stored. That shape is
/// what makes food prices move, gives a trader something to anticipate,
/// and makes a bad harvest matter for twelve months rather than one.
///
/// Returns a multiplier on a farm's rated output for the day.
pub fn harvest_curve(day: u64, southern: bool) -> f64 {
    let mut d = (day % DAYS_PER_YEAR) as f64;
    if southern {
        d = (d + DAYS_PER_YEAR as f64 / 2.0) % DAYS_PER_YEAR as f64;
    }
    // Peak in early autumn, the way a real cereal harvest falls.
    let peak = DAYS_PER_YEAR as f64 * 0.62;
    let mut gap = (d - peak).abs();
    if gap > DAYS_PER_YEAR as f64 / 2.0 {
        gap = DAYS_PER_YEAR as f64 - gap;
    }
    // Narrow bell: most of the year's crop lands within about six weeks,
    // and a trickle the rest of the time from other produce.
    let width = 26.0;
    let z = gap / width;
    let bell = (-z * z).exp();
    // Scaled so a full year integrates to roughly one year of rated
    // output, which keeps annual supply matched to annual demand.
    0.06 + 6.9 * bell
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
/// **The company that actually services an area.**
///
/// A grid is not owned by "the state" in one lump: it is licensed out in
/// territories, and each holder keeps its own stores. Britain has fourteen
/// distribution licence areas; the United States has hundreds of
/// investor-owned, municipal and cooperative utilities. Which one serves
/// the fault decides whose spare gets fitted.
///
/// **And they lend to each other.** Mutual assistance is a real,
/// formalised arrangement — after a storm, thousands of linemen and their
/// plant cross state lines under standing agreements — and for the one
/// part that matters most there is a named scheme: the **Spare
/// Transformer Equipment Program**, under which utilities pool large
/// transformers and commit to releasing them to each other. Grid Assurance
/// does the same commercially. It exists because a large power transformer
/// is built to order and cannot be bought in an emergency at any price.
pub struct Utility {
    pub name: String,
    /// The markets in its licence area.
    pub serves: Vec<usize>,
    /// Large transformers in store. Real utilities hold roughly one spare
    /// per five to ten units in service, and it looks like waste every day
    /// but one.
    pub spares: usize,
}

/// Where a replacement transformer came from, which is the whole
/// difference between a bad week and a bad year.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Sourced {
    /// Off the servicing company's own shelf.
    Own,
    /// **Borrowed from a neighbour**, and then hauled. A large power
    /// transformer is 100-400 tonnes and 3.5-4.5 m wide — an abnormal load
    /// needing an order from the highway authority, a surveyed route and a
    /// move at walking pace. Weeks, against the year it takes to build one.
    Borrowed { from: String },
    /// Nobody had one. Built to order.
    Built,
}

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
    /// The companies servicing this economy's areas, and their stores.
    /// Empty falls back to the pooled `spare_transformers`, which is what
    /// the hand-built scenarios use.
    pub utilities: Vec<Utility>,
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

    /// Share of the road network's needed upkeep that actually gets
    /// funded.
    ///
    /// Nobody opens a maintained bridge, so maintenance is the line real
    /// governments cut first and the decay takes a decade to show — which
    /// is exactly why a negligent state can look fine for a long time and
    /// then not.
    pub fn maintenance_funding(self) -> f64 {
        match self {
            Doctrine::Prudent => 1.0,
            Doctrine::Negligent => 0.55,
        }
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
            utilities: Vec::new(),
            incidents: Vec::new(),
        }
    }

    /// **Where the next transformer is coming from, and how long it
    /// takes.**
    ///
    /// Own shelf, a neighbour's shelf, or a factory queue — and the three
    /// answers are days, weeks and the better part of a year. This is the
    /// single most consequential thing a utility's stores decide.
    pub fn source_transformer(&mut self, market: usize) -> (u64, Sourced) {
        /// Fitting one that is already on site.
        const FIT_DAYS: u64 = 5;
        /// **Hauling a borrowed one.** 100-400 tonnes and 3.5-4.5 m wide
        /// is an abnormal load: an order from the highway authority, a
        /// route surveyed for bridges and headroom, and a move at walking
        /// pace. Real mutual-aid delivery runs two to six weeks.
        const HAUL_DAYS: u64 = 21;

        if self.utilities.is_empty() {
            return if self.spare_transformers > 0 {
                self.spare_transformers -= 1;
                (self.repair_days + FIT_DAYS, Sourced::Own)
            } else {
                (self.transformer_lead_days, Sourced::Built)
            };
        }

        // Whose area is this?
        let mine = self
            .utilities
            .iter()
            .position(|u| u.serves.contains(&market));
        if let Some(i) = mine {
            if self.utilities[i].spares > 0 {
                self.utilities[i].spares -= 1;
                return (self.repair_days + FIT_DAYS, Sourced::Own);
            }
        }
        // Nothing on our own shelf: ask the neighbours. Deterministic
        // order, so a seed rebuilds the same history.
        if let Some(j) = (0..self.utilities.len())
            .find(|&j| Some(j) != mine && self.utilities[j].spares > 0)
        {
            self.utilities[j].spares -= 1;
            let from = self.utilities[j].name.clone();
            return (self.repair_days + FIT_DAYS + HAUL_DAYS, Sourced::Borrowed { from });
        }
        (self.transformer_lead_days, Sourced::Built)
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
    /// Seeds the weather, so a world replays identically.
    pub weather_seed: u64,
    /// Per nation: the state of its roads, 0 (impassable ruin) to 1 (as
    /// built).
    pub road_condition: Vec<f64>,
    /// Per nation: the share of needed maintenance actually funded.
    ///
    /// This is the maintenance deficit of spec C.4, and the reason it is
    /// worth modelling: new bridges get opening ceremonies and maintained
    /// ones do not, so real governments underfund upkeep and the network
    /// decays for a decade before anything visibly breaks — long after the
    /// leadership responsible has moved on.
    pub maintenance_funding: Vec<f64>,
    /// Electricity that could not be supplied today — the load shed.
    pub unserved_power: f64,
    /// Household demand that could not be met, per commodity. This is
    /// people going without, and it feeds C1's grievance conditions.
    pub unmet_demand: Basket,
    /// The labour market of each town, in the trades this economy has.
    ///
    /// Kept here rather than on `Market` because it is derived state: it
    /// is recomputed every day from what the works actually managed to
    /// run, which is what makes a blackout put people out of work.
    pub workforce: Vec<crate::labour::Workforce>,
    /// **The state, if this economy has one.** It raises revenue off the
    /// economy and spends it on services, and those services are the
    /// largest single block of jobs in a developed country — 14-21% of
    /// the workforce, and none of it existed here.
    pub government: Option<crate::state::Government>,
    /// **The private services** — construction, hospitality, recreation,
    /// offices. About 43% of all employment, and none of it existed.
    pub services: Option<crate::services::Services>,
}

impl Economy {
    /// Advance one day.
    pub fn step(&mut self) {
        self.unserved_power = 0.0;
        self.unmet_demand = basket();

        for site in self.ledger.sites.iter_mut() {
            site.ran = 0.0;
        }
        for route in self.routes.iter_mut() {
            route.moved = None;
        }

        self.turn_of_the_year();
        self.close_the_passes();
        self.run_response();
        self.generate_power();
        self.allocate_power();
        self.produce();
        // Sell first, then reorder — a shop restocks against what it has
        // left at close of business, which is what makes the day's cover
        // figure mean "days of stock in hand".
        self.consume_households();
        // **After the shops have sold**, not before.
        //
        // Who was working today falls out of what the works ran and what
        // the tills took, and running this before the day's trade read
        // every shop as shut: sales were still zero, so a supermarket
        // rostered a third of its people and could not keep a cashier
        // housed.
        crate::labour::update(self);
        self.distribute();
        self.trade();
        self.update_prices();
        // **At the end of the day, after everything has moved.** Meat
        // that was sold this morning is not in the cold store tonight,
        // and a cargo that arrived is.
        self.spoil_stock();
        self.discard_unused_power();

        self.ledger.day += 1;

        #[cfg(debug_assertions)]
        self.ledger.assert_conserved();
    }

    /// Draw the coming year's weather, once, on the day the growing year
    /// turns.
    ///
    /// Real yields vary by roughly a fifth from year to year in a
    /// temperate country, with a long tail of genuinely bad years —
    /// drought, a wet harvest, a late frost. Modelled as a multiplier
    /// skewed low, because good years cluster near average while bad ones
    /// can be very bad.
    fn turn_of_the_year(&mut self) {
        let day = self.ledger.day;
        if day % DAYS_PER_YEAR != 0 {
            return;
        }
        let year = day / DAYS_PER_YEAR;
        let seed = self.weather_seed;

        for m in self.markets.iter_mut() {
            // Keyed by nation, so a country's provinces share a season
            // while its neighbours may be having a quite different year.
            // That difference is half of why trade exists.
            let mut z = seed
                .wrapping_add(year.wrapping_mul(0x9E37_79B9_7F4A_7C15))
                .wrapping_add((m.nation as u64).wrapping_mul(0x517C_C1B7_2722_0A95));
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            let u = ((z >> 11) as f64) / ((1u64 << 53) as f64); // 0..1

            // Roughly 0.78..1.15, mean near 0.97, with the low tail longer
            // than the high one. Real cereal yields vary by ten to twenty
            // per cent year on year; a fifth down is a bad year a country
            // rides out on its reserves, not a catastrophe. A wider spread
            // than reality produces a famine every few years, which is
            // neither true nor interesting.
            m.harvest_quality = 0.78 + 0.37 * u.powf(0.7);
        }

        self.age_the_roads();
    }

    /// A year of wear against a year of maintenance.
    ///
    /// Left entirely alone a road surface is finished in fifteen to twenty
    /// years, so it loses of the order of six per cent of its condition
    /// annually. Funding replaces that share of the loss. The floor is
    /// well above zero because even an abandoned road remains a formation
    /// people drive slowly along; it does not become open country again.
    fn age_the_roads(&mut self) {
        const DECAY_PER_YEAR: f64 = 0.06;
        for (n, cond) in self.road_condition.iter_mut().enumerate() {
            let funded = self.maintenance_funding.get(n).copied().unwrap_or(1.0);
            let lost = DECAY_PER_YEAR * (1.0 - funded).max(0.0);
            *cond = (*cond - lost).clamp(0.35, 1.0);
        }

        // Freight on a worn road costs more: slower running, heavier wear
        // on lorries, loads broken for weight limits, detours round a
        // closed bridge. A route between two nations is only as good as
        // the worse of the two.
        for r in self.routes.iter_mut() {
            let na = self.markets[r.a].nation as usize;
            let nb = self.markets[r.b].nation as usize;
            let worst = self
                .road_condition
                .get(na)
                .copied()
                .unwrap_or(1.0)
                .min(self.road_condition.get(nb).copied().unwrap_or(1.0));
            r.freight_cost = r.sound_cost * (1.0 + 0.8 * (1.0 - worst));
        }
    }

    /// Shut every pass the snow has taken, and reopen those it has left.
    ///
    /// A country whose trunk route crosses a col has a hole in its economy
    /// every winter, and one that paid for the tunnel does not. That is
    /// the whole argument for the tunnel, and it is why the decision is
    /// worth modelling rather than averaging away into a cost per
    /// kilometre.
    fn close_the_passes(&mut self) {
        let day = self.ledger.day;
        for r in self.routes.iter_mut() {
            // The season at the end that has the winter.
            let season = self.markets[r.a].season(day);
            let other = self.markets[r.b].season(day);
            let shut = r.crossing.shut_by_snow(season, 0.0)
                || r.crossing.shut_by_snow(other, 0.0);
            r.snowed_in = shut;
        }
    }

    /// Season in market `m`.
    pub fn season_at(&self, m: usize) -> Season {
        self.markets[m].season(self.ledger.day)
    }

    /// Today's harvest multiplier in market `m`.
    pub fn harvest_at(&self, m: usize) -> f64 {
        self.markets[m].harvest(self.ledger.day)
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
                    Fault::Transformer(name) => {
                        // **Whose area is it in?** The grid knows which
                        // market each piece of plant serves, so the
                        // company that services it is the one whose shelf
                        // is emptied — and whose neighbours get asked.
                        let market = self
                            .grid
                            .lines
                            .iter()
                            .find(|l| l.name == *name)
                            .and_then(|l| l.serves)
                            .unwrap_or(0);
                        // Own shelf, a neighbour's shelf, or a factory
                        // queue — days, weeks, or the better part of a
                        // year.
                        self.response.source_transformer(market).0
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
    /// **What the country will actually draw today.**
    ///
    /// The transmission capacity is a ceiling, not a target: a grid
    /// dispatches against load. Without this a station burned every tonne
    /// it could reach and generated to fill the wires, which made it a
    /// coal incinerator — raise the colliery's output and it simply burnt
    /// more, so the steelworks beside it on the same coalfield never got a
    /// tonne however much was mined.
    pub fn power_demand(&self) -> f64 {
        let mut want = 0.0;
        for site in 0..self.ledger.sites.len() {
            let s = &self.ledger.sites[site];
            if s.kind == SiteKind::PowerPlant {
                continue;
            }
            if self.grid.cut_off(site, s.market) {
                continue;
            }
            if let Some(r) = s.recipe {
                want += RECIPES[r].power * self.runnable(site);
            }
        }
        for m in self.markets.iter() {
            want += m.daily_household_demand(Commodity::Electricity);
        }
        want
    }

    fn generate_power(&mut self) {
        // Dispatch against load, capped by what the wires can carry.
        let carry = self.grid.capacity().min(self.power_demand());
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
            // A plant's staffing follows what it actually generated, not
            // its rated ceiling. `throughput` on a power station is a
            // sentinel standing for "as much as the grid can carry", and
            // reading headcount off it staffed one coal station with four
            // million people.
            self.ledger.sites[site].ran = batches;

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
                Some(r) => RECIPES[r].power * self.runnable(site),
                None => 0.0,
            };
            // **A cut feeder or service connection takes this site out
            // whatever the grid is doing.** Without this, the only kind of
            // fault the model had was a national one.
            if self.grid.cut_off(site, s.market) {
                continue;
            }
            if need > 0.0 {
                wants.push((site, need));
            }
        }
        // **Fuel, then food, then shops, then heavy industry.** Lumping
        // all industry together at one rank was wrong the moment there was
        // more than one kind of it: a goods factory draws 1.0 MWh a tonne
        // against a cannery's 0.35 and is several times the size, so
        // "larger first" handed it the whole supply and shut the food
        // chain down. **Making cutlery is not more important than making
        // food.**
        //
        // Real grids shed in this order too, and the heaviest users are
        // *paid* to be first out: an **interruptible tariff** buys a
        // steelworks or a smelter cheaper power in exchange for being cut
        // on demand, which is exactly why they sit below a bakery here.
        wants.sort_by(|a, b| {
            let rank = |i: usize| match self.ledger.sites[i].kind {
                // The mine first: without coal nothing generates at all
                // tomorrow, so starving it to keep a factory running today
                // is how a grid talks itself into a blackout.
                SiteKind::Mine | SiteKind::IronMine => 0,
                // The food chain.
                SiteKind::Factory | SiteKind::Mill | SiteKind::Butcher => 1,
                SiteKind::Farm => 2,
                SiteKind::Shop => 3,
                // **Steel before the factories that eat it.** They are
                // both heavy industry, but shedding the steelworks to keep
                // the works running stops the works a fortnight later for
                // want of metal — the same reasoning that puts the
                // colliery first.
                SiteKind::Steelworks => 4,
                // Heavy manufacturing, on an interruptible tariff.
                SiteKind::Works => 5,
                _ => 6,
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

    /// **What a site can actually run today**, given the inputs it holds.
    ///
    /// A plant with nothing to work draws no power, and pricing its demand
    /// off its rated capacity instead is how a grid talks itself into a
    /// famine. The factories here wanted 1.0 MWh a tonne against a rating
    /// they had no steel to meet; the station burned the coal to supply
    /// them, and that coal was exactly what the steelworks next door
    /// needed to make the steel. Six attempts at fixing it elsewhere —
    /// bigger collieries, shed priorities, two-pass distribution — changed
    /// nothing at all, because every one of them was feeding a demand that
    /// should never have existed.
    fn runnable(&self, site: usize) -> f64 {
        let s = &self.ledger.sites[site];
        let Some(r) = s.recipe else { return 0.0 };
        let mut batches = s.throughput;
        for &(c, per) in RECIPES[r].inputs {
            if per <= 0.0 {
                continue;
            }
            batches = batches.min(self.ledger.stock(site, c) / per);
        }
        batches.max(0.0)
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
            let recipe = &RECIPES[r];
            // **A blackout stops the factory; it does not stop the farm.**
            //
            // Spec A.2 is explicit that the farm's critical dependency is
            // water rather than power, and it is right: a grid failure
            // shuts a mill within the hour, while the tractors run on
            // diesel and the harvest comes in regardless. Gating every
            // site on the grid made a dead transformer idle a nation's
            // agriculture, which is both wrong and much too convenient a
            // way to cause a famine.
            //
            // What ought to stop a farm is an irrigation main, and there
            // is no water network yet — so for now a farm is simply not
            // the grid's to switch off. `needs_water` has been sitting
            // unused since it was written; this is what it was for.
            if !s.powered && !recipe.needs_water {
                continue; // no power, no production
            }

            // A farm does not produce evenly through the year: it produces
            // when the crop is ready. Everything else runs flat.
            let mut batches = self.site_capacity(site);
            if s.kind == SiteKind::Farm {
                batches *= self.harvest_at(s.market);
            }
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
            self.ledger.sites[site].ran = batches;

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
        for r in self.routes.iter().filter(|r| r.usable()) {
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
        // **Everybody's running needs before anybody's stockpile.**
        //
        // One pass in site-index order let the first consumer on the list
        // fill its yard to a three-day cover before the second had run at
        // all. It never showed while each commodity had a single consumer;
        // the moment a steelworks and a power station both wanted coal,
        // the station — earlier in the list, and asking for three days of
        // a national grid's burn — took every tonne the pit raised, and
        // the works beside it on the same coalfield made no steel. No
        // tinplate, so no cans, so the canneries stopped, so a country
        // with full granaries went hungry.
        //
        // Two passes fix it, and it is what a real allocator does under
        // rationing: cover everyone's daily draw first, then let whoever
        // is short build inventory with what is left.
        self.distribute_to_cover(1.0);
        self.distribute_to_cover(3.0);
    }

    fn distribute_to_cover(&mut self, days: f64) {
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
                        daily * c.target_cover_days() * (days / 3.0)
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
                        // **A power station's `throughput` is a sentinel**
                        // meaning "whatever the grid can carry" (1e9), and
                        // reading it as a rate here asked for 0.38 x 1e9 x
                        // 3 — a billion tonnes of coal. The station then
                        // took every tonne the pit raised and the
                        // steelworks in the same town, on the same
                        // coalfield, stood with nothing to smelt.
                        //
                        // This is the second time the sentinel has bitten:
                        // read as a rate it once staffed one station with
                        // 4.1M people. It only surfaced now because until
                        // there was a steel industry nothing else in the
                        // country wanted coal.
                        let rate = if self.ledger.sites[dst].kind == SiteKind::PowerPlant {
                            self.grid.capacity().min(self.power_demand())
                        } else {
                            self.ledger.sites[dst].throughput
                        };
                        per * rate * days
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
                // Local suppliers first, then anywhere the network reaches.
                // Without the local pass, sites are drawn on in whatever
                // order they happen to sit in the list, so every mill in
                // the country empties the capital's granary before touching
                // the one next door — and the capital reads as famine-struck
                // while the provinces sit on full silos.
                let order: Vec<usize> = (0..self.ledger.sites.len())
                    .filter(|&s| self.ledger.sites[s].market == market)
                    .chain(
                        (0..self.ledger.sites.len())
                            .filter(|&s| self.ledger.sites[s].market != market),
                    )
                    .collect();

                for src in order {
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
                    // What the shop actually sold today. A shop is busy or
                    // it is not, and how many tills it opens follows.
                    self.ledger.sites[site].ran += take;
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
            if !self.routes[r].usable() {
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

                // Ship from whoever in the surplus market holds the goods
                // to whoever in the deficit market has room. Restricting
                // this to shops made it dead for everything nobody buys
                // over a counter: grain sits in granaries, so a grain
                // arbitrage could be worth taking and nothing would move.
                let holds = |s: usize, m: usize| {
                    self.ledger.sites[s].market == m
                        && self.ledger.sites[s].capacity[c as usize] > 0.0
                };
                let source: Vec<usize> = (0..self.ledger.sites.len())
                    .filter(|&s| holds(s, from_m))
                    .collect();
                let sink: Vec<usize> = (0..self.ledger.sites.len())
                    .filter(|&s| holds(s, to_m))
                    .collect();
                // Deliver where there is most room, which is where the
                // shortage is deepest.
                let dst = sink.iter().copied().max_by(|&a, &b| {
                    let room = |s: usize| {
                        self.ledger.sites[s].capacity[c as usize] - self.ledger.stock(s, c)
                    };
                    room(a).total_cmp(&room(b)).then(b.cmp(&a))
                });
                let (Some(dst), false) = (dst, source.is_empty()) else {
                    continue;
                };

                // Only genuine surplus moves. A trader who empties his home
                // market to chase a price has no home market — and without
                // this the two towns simply slosh stock back and forth.
                // Measured against the market's own total draw, household
                // and industrial, since for grain the mills are the buyers.
                let keep = self.daily_draw(from_m, c) * c.target_cover_days();
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
                    let carried = self.routes[r].moved.map_or(0.0, |(_, _, t)| t);
                    if qty > carried {
                        self.routes[r].moved = Some((c, to_m, qty));
                    }
                    budget -= qty;
                }
            }
        }
    }

    /// Price from stock cover against demand. Spec A.6.
    ///
    /// Demand is household *plus* industrial: a mill wanting grain is as
    /// real a buyer as a family wanting bread, and for the goods nobody
    /// eats directly it is the only buyer there is. Pricing on household
    /// demand alone leaves grain and flour with no price at all, and hides
    /// the whole seasonal signal — which lives in what the mill pays at
    /// harvest versus what it pays in spring, not in the price of a loaf.
    fn update_prices(&mut self) {
        for m in 0..self.markets.len() {
            for &c in Commodity::ALL.iter() {
                let industrial: f64 = (0..self.ledger.sites.len())
                    .filter(|&s| self.ledger.sites[s].market == m)
                    .filter_map(|s| {
                        let r = self.ledger.sites[s].recipe?;
                        let per = RECIPES[r]
                            .inputs
                            .iter()
                            .find(|&&(ic, _)| ic == c)
                            .map(|&(_, q)| q)?;
                        Some(per * self.ledger.sites[s].throughput)
                    })
                    .sum();

                let demand = self.markets[m].daily_household_demand(c) + industrial;
                if demand <= 0.0 {
                    self.markets[m].cover[c as usize] = f64::INFINITY;
                    continue;
                }

                // Stock held anywhere in the market, not only in shops:
                // grain sitting in a granary is grain the mill can buy.
                let shop_only = c.per_capita_annual() > 0.0 && industrial <= 0.0;
                let stock: f64 = (0..self.ledger.sites.len())
                    .filter(|&s| {
                        self.ledger.sites[s].market == m
                            && (!shop_only || self.ledger.sites[s].kind == SiteKind::Shop)
                    })
                    .map(|s| self.ledger.stock(s, c))
                    .sum();

                let cover = stock / demand;
                self.markets[m].cover[c as usize] = cover;

                // Price the crop year, not today's silo reading.
                //
                // A grain stock legitimately swings by half between harvest
                // and midsummer, and if price tracked that reading it would
                // swing several-fold every year — which real grain prices
                // do not, because merchants buy at harvest precisely
                // because they expect a better price later, and the buying
                // damps the very swing they are betting on. Averaging cover
                // over a period as long as the commodity keeps is the cheap
                // way to get that behaviour without modelling speculators:
                // a perishable reacts within days, grain over months.
                // A quarter of the stock cycle, not the whole of it. Long
                // enough to damp the seasonal swing, short enough that a
                // market notices a cargo arriving: at the full window a
                // grain price took five months to respond to imports, so
                // trade relieved the shortage and the price never knew.
                let window = (c.target_cover_days() * 0.25).max(3.0);
                let alpha = 1.0 / window;
                let seen = self.markets[m].expected_cover[c as usize];
                let cover = seen * (1.0 - alpha) + cover * alpha;
                self.markets[m].expected_cover[c as usize] = cover;

                // Elasticity relates a *proportional* shortfall to a
                // proportional price move: a 20% shortfall in a good with
                // elasticity -0.25 raises price about 80%, which is the
                // real relationship. Raising scarcity to the power of
                // 1/elasticity instead compounds to absurdity — an early
                // version of this priced food at four thousand times cost.
                let target = c.target_cover_days().max(0.5);
                let gap = (target - cover) / target;
                // The floor is well above zero: a glut is a bad price, not
                // a free good. Producers stop selling long before that, and
                // in a real surplus the crop is stored or exported rather
                // than given away.
                let multiplier =
                    (1.0 + gap / c.elasticity().abs()).clamp(0.7, 8.0);
                self.markets[m].price[c as usize] = c.base_cost() * multiplier;
            }
        }
    }

    /// **Food goes off, and a cold chain is what stops it.**
    ///
    /// Real shelf lives: fresh meat is finished in one to two days at
    /// ambient and keeps four to six weeks chilled; grain keeps for years,
    /// a can indefinitely. So a site holding meat loses about half of it a
    /// day with the power off and about a thirtieth with it on — which is
    /// the difference between an inconvenience and a total loss, and the
    /// reason a blackout is worse for a butcher than for a mill.
    ///
    /// A mill loses production while the power is off and catches up
    /// after. A butcher loses the stock.
    fn spoil_stock(&mut self) {
        for site in 0..self.ledger.sites.len() {
            let cold = self.ledger.sites[site].powered;
            for c in Commodity::ALL {
                if c == Commodity::Electricity {
                    continue;
                }
                let rate = c.spoilage_per_day(cold || !c.needs_cold());
                if rate <= 0.0 {
                    continue;
                }
                let held = self.ledger.stock(site, c);
                if held <= 1e-9 {
                    continue;
                }
                let lost = (held * rate).min(held);
                if lost > 1e-9 {
                    self.ledger.apply(
                        &mut self.journal,
                        Event::Spoiled {
                            site,
                            commodity: c,
                            qty: lost,
                        },
                    );
                }
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

    /// Everything market `m` draws down of `c` in a day — households at the
    /// counter and works at the gate both. For the goods nobody buys over a
    /// counter the industrial draw is the only demand there is, so leaving
    /// it out makes grain look unwanted.
    pub fn daily_draw(&self, m: usize, c: Commodity) -> f64 {
        let industrial: f64 = (0..self.ledger.sites.len())
            .filter(|&s| self.ledger.sites[s].market == m)
            .filter_map(|s| {
                let r = self.ledger.sites[s].recipe?;
                let per = RECIPES[r]
                    .inputs
                    .iter()
                    .find(|&&(ic, _)| ic == c)
                    .map(|&(_, q)| q)?;
                Some(per * self.ledger.sites[s].throughput)
            })
            .sum();
        self.markets[m].daily_household_demand(c) + industrial
    }

    /// How much of `c` market `m` would part with: what it holds above its
    /// own working reserve.
    ///
    /// A market does not sell its last store of something at the going rate
    /// however much somebody offers, and this is the rule that makes that
    /// true for everyone. Applying it to firms but not to people let a
    /// trader buy a town's reserve at the posted price and carry it over
    /// the hill, which is a licence to print money rather than a trade.
    pub fn surplus(&self, m: usize, c: Commodity) -> f64 {
        let keep = self.daily_draw(m, c) * c.target_cover_days();
        let held: f64 = (0..self.ledger.sites.len())
            .filter(|&s| self.ledger.sites[s].market == m)
            .map(|s| self.ledger.stock(s, c))
            .sum();
        (held - keep).max(0.0)
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
