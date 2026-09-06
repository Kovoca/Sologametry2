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
    /// **Timber**, in tonnes of industrial roundwood. From the forest
    /// `biota.rs` has been growing since it was written — standing stock
    /// that until now nothing on the planet could cut down.
    Timber,
    /// **Crude oil**, in tonnes. From the petroleum `geology.rs` puts in
    /// the ground, which had likewise never had a consumer.
    Petroleum,
    /// **Plastics**, in tonnes of resin. Cracked from oil, and the reason
    /// a modern economy needs a petroleum industry for something other
    /// than burning.
    Plastics,
    /// **Machinery**, in tonnes — tools, car parts, engines, pumps, the
    /// capital goods of the design doc's "whole nine yards". The step
    /// between a tonne of steel and a thing somebody buys.
    Machinery,
    /// **Cement**, in tonnes. After water, the most-consumed substance on
    /// earth — about **4.1 billion tonnes a year, half a tonne a head**.
    ///
    /// Made by burning limestone, which is near enough everywhere that a
    /// cement works is sited on its fuel and its market rather than on its
    /// ore. That is why there is no limestone commodity here: the input
    /// that decides where a kiln goes is the coal.
    Cement,
    /// **Medicines and medical supplies**, in tonnes — drugs, dressings,
    /// fluids, disposables.
    ///
    /// Peter's rule applied to a hospital: the machinery to do medicine
    /// and the medicines themselves come from somewhere. Most drugs are
    /// organic synthesis on **petrochemical feedstock**, so a health
    /// service is downstream of an oil supply, and that is not a
    /// contrivance — it is why pharmaceutical plants sit in chemical
    /// clusters.
    Medicine,
    /// **Bulk and fine chemicals**, in tonnes — solvents, reagents,
    /// acids, alkalis, the intermediates everything else is synthesised
    /// from.
    ///
    /// A pharmaceutical works does not start from crude oil any more than
    /// a baker starts from a field: it buys chemicals, and somebody makes
    /// those. It is one of the largest industries on earth and almost
    /// nobody outside it can name a single product.
    Chemicals,
    /// **Over-the-counter remedies**, in tonnes — what a shop sells.
    ///
    /// Paracetamol, ibuprofen, aspirin, antihistamines, antacids, cough
    /// mixtures, 1% hydrocortisone, vitamins. Low doses with a wide safety
    /// margin, which is exactly why they can be sold to anybody without a
    /// prescription.
    ///
    /// **A hospital cannot run on these**, and that is the distinction
    /// worth modelling: you cannot anaesthetise anybody with aspirin.
    Remedies,
}

pub const N_COMMODITIES: usize = 18;

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
        Commodity::Timber,
        Commodity::Petroleum,
        Commodity::Plastics,
        Commodity::Machinery,
        Commodity::Cement,
        Commodity::Medicine,
        Commodity::Chemicals,
        Commodity::Remedies,
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
            Commodity::Timber => "timber",
            Commodity::Petroleum => "oil",
            Commodity::Plastics => "plastic",
            Commodity::Machinery => "machines",
            Commodity::Cement => "cement",
            Commodity::Medicine => "medicine",
            Commodity::Chemicals => "chemicals",
            Commodity::Remedies => "remedies",
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
            Commodity::Timber => -0.4,
            // Oil is the classic inelastic commodity: there is no
            // substitute at short notice, which is why a few percent off
            // supply moves the price by half.
            Commodity::Petroleum => -0.1,
            Commodity::Plastics => -0.4,
            Commodity::Machinery => -0.6,
            Commodity::Cement => -0.3,
            // **The most inelastic thing there is.** Nobody does without
            // insulin because it got dearer; they go without something
            // else, or they die.
            Commodity::Medicine => -0.08,
            Commodity::Chemicals => -0.3,
            // A headache tablet is a comfort, not a necessity, and gives
            // far more readily than the drug that keeps somebody alive.
            Commodity::Remedies => -0.7,
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
            // **What a shop sells is not what a hospital uses.** Roughly
            // 8 kg a head a year of over-the-counter remedies goes across
            // a counter; the medical-grade supply is bought by the state
            // on contract and never appears here.
            Commodity::Remedies => 0.008,
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
            Commodity::Timber => 30.0,
            // **A strategic stock, and a real one.** IEA members are
            // obliged to hold 90 days of net oil imports, which is the
            // single largest deliberate stockpile of anything anywhere.
            Commodity::Petroleum => 60.0,
            Commodity::Plastics => 20.0,
            Commodity::Machinery => 20.0,
            // **Cement does not keep.** Bagged, it is finished in about
            // three months and goes off faster in a damp climate, which
            // is why a works ships continuously and nobody stockpiles it
            // the way they stockpile ore.
            Commodity::Cement => 12.0,
            // A hospital holds weeks, and running out is a different kind
            // of event from running out of anything else here.
            Commodity::Medicine => 45.0,
            Commodity::Chemicals => 20.0,
            Commodity::Remedies => 20.0,
        }
    }

    /// **What it costs to get a tonne out of ground of this quality.**
    ///
    /// A multiplier on the reference cost, and the physical reason is
    /// simple: a poorer deposit means moving, crushing and processing
    /// proportionally more rock for the same tonne of product, so cost goes
    /// roughly as one over the grade. Real spreads, and they are not small:
    ///
    /// | | rich | poor |
    /// |---|---|---|
    /// | crude oil | **$10/bbl** Saudi | $50-60 oil sands |
    /// | coal | $12/ton Powder River surface | $60-70 Appalachian underground |
    /// | iron ore | $15-20/t Pilbara at 62% Fe | $70-100/t Chinese at half that |
    ///
    /// And it is why declining ore grades matter so much: average copper
    /// grade has fallen from about 1.6% in 1990 to 0.6% now, which is the
    /// same copper costing nearly three times as much rock to win.
    pub fn cost_of_working(grade: f64) -> f64 {
        // **The reference has to sit where the deposits actually are.**
        // A nation of any size contains the peak of some deposit, so its
        // best cell measures 0.8-1.0 far more often than not — and
        // centring this at 0.42 handed every country in the world a
        // twofold discount, which then compounded through ore into steel
        // into machinery into goods and left crude steel at a third of its
        // calibrated price.
        //
        // So an ordinary good deposit costs exactly the reference, a
        // world-class one is a tenth cheaper, and a thin one is dear.
        // Clamped at the bottom so a marginal seam is expensive rather
        // than infinite; below that nobody opens the mine at all.
        const ORDINARY_GRADE: f64 = 0.9;
        (ORDINARY_GRADE / grade.clamp(0.05, 1.0)).clamp(0.45, 6.0)
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
            // **Relative to steel, which is what actually matters here.**
            // Real ratios against crude steel: sawn timber ~0.25x, crude
            // oil ~1.0x, polymer resin ~2.3x. The upper end is compressed
            // because the finished-goods price anchors the top of this
            // scale — the model's currency is its own, pinned to the food
            // chain, and only the ratios are meant to be read.
            Commodity::Timber => 110.0,
            Commodity::Petroleum => 220.0,
            Commodity::Plastics => 430.0,
            Commodity::Machinery => 620.0,
            // Real cement is $100-130 a tonne — about a quarter of steel,
            // which is the ratio that matters here.
            Commodity::Cement => 100.0,
            // **The most valuable thing in the economy by weight**, and by
            // a long way. Even generic drugs run tens of thousands a tonne
            // and branded ones far more.
            Commodity::Medicine => 6_000.0,
            // Bulk chemicals are cheap and fine chemicals are not; this
            // is a blend, and it sits between resin and medicine.
            Commodity::Chemicals => 700.0,
            // **Four times cheaper than medical grade**, and in reality
            // the gap is wider still: paracetamol is a bulk chemical at a
            // few pounds a kilo, while a sterile injectable is made under
            // GMP in a validated cleanroom with batch traceability and
            // yields to match. Global pharma is ~$1.6tn of which OTC is
            // ~$180bn — a ninth of the value on a far larger share of the
            // tonnage.
            Commodity::Remedies => 1_400.0,
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
    /// Felling. Wherever the forest is worth cutting.
    Forestry,
    /// An oil field, or the terminal where somebody else's oil is landed.
    OilField,
    /// A cracker, turning oil into polymer.
    Cracker,
    /// A machine works — the step between a tonne of steel and a thing.
    MachineWorks,
    /// A cement kiln.
    CementWorks,
    /// **The building trade.** Consumes fabric and produces nothing that
    /// moves, which is what makes it construction rather than
    /// manufacturing.
    Builders,
    /// Where medicines are made.
    Pharma,
    /// A chemical works — solvents, reagents, the intermediates.
    ChemicalWorks,
    /// **A hospital.** Holds stock, consumes it, and produces nothing that
    /// can be shipped.
    Hospital,
    PowerPlant,
    Shop,
    /// Where goods from outside the modelled region arrive.
    Depot,
}


/// What an hour of work costs a firm, in the model's own currency, pinned
/// like everything else to the food chain.
const WAGE_AN_HOUR: f64 = 22.0;

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
    /// **What this particular site costs to work**, against an ordinary
    /// one — a multiplier on its production cost.
    ///
    /// The number that makes a rich deposit different from a poor one, and
    /// the spread in reality is enormous: Saudi crude lifts for about $10 a
    /// barrel and Canadian oil sands for $50-60, Powder River coal comes out
    /// at $12 a ton and Appalachian underground at $60-70, and Australian
    /// iron ore at 62% Fe costs a fifth of what Chinese ore at half the
    /// grade does. **You have to move, crush and process proportionally
    /// more rock**, which is why cost goes roughly as one over the grade.
    ///
    /// One for a works, which is built to a design rather than found in the
    /// ground.
    pub cost_factor: f64,
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
    /// **A works has a yard for what goes in and a store for what comes
    /// out**, and both have to exist before it can trade.
    ///
    /// Storage is per commodity, so a site with no `capacity` entry for
    /// one of its own recipe inputs can never receive a single tonne of
    /// it — `distribute` clamps every delivery by the room available. It
    /// fails silently: the site simply never runs, and what you see at the
    /// far end is a famine.
    ///
    /// That is exactly what happened when the cannery gained a tinplate
    /// input and kept its old two-commodity store. Rather than trust
    /// whoever writes the next recipe to remember, every input gets a yard
    /// whether or not the caller thought of it.
    ///
    /// **Real works hold more of their inputs than of their output**:
    /// weeks of raw material against days of finished goods, because the
    /// input is what stops the line and the output is what somebody is
    /// waiting for. A steelworks' ore stockyard dwarfs its billet store.
    fn give_every_input_a_yard(sites: &mut [Site]) {
        /// Days of input a works keeps if nobody said otherwise. Long
        /// enough to ride out a delivery being late, which is what a raw
        /// materials yard is for.
        const DEFAULT_YARD_DAYS: f64 = 20.0;
        for s in sites.iter_mut() {
            let Some(r) = s.recipe else { continue };
            // A power station's throughput is a sentinel, not a rate.
            let rate = if s.kind == SiteKind::PowerPlant {
                continue;
            } else {
                s.throughput
            };
            for &(c, per) in RECIPES[r].inputs {
                if !c.storable() {
                    continue;
                }
                let want = per * rate * DEFAULT_YARD_DAYS;
                let i = c as usize;
                if s.capacity[i] < want {
                    s.capacity[i] = want;
                }
            }
        }
    }

    pub fn new(mut sites: Vec<Site>) -> Self {
        Self::give_every_input_a_yard(&mut sites);
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

pub const RECIPES: [Recipe; 32] = [
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
        // **Assembled, not conjured.** A tonne of what a household buys is
        // machines, plastic and wood — and through the machines, steel,
        // and through the steel, ore and coal. Every gram of it leads back
        // to something somebody had to dig up or cut down, which is the
        // whole point of the tree.
        //
        // The steel content is still anchored on the real figure: world
        // crude steel is **~230 kg a head a year** and households take a
        // tonne of goods each, so 0.28 t of machinery at 72% steel puts
        // 0.20 t of steel in every tonne of goods.
        //
        // Timber at 0.20 gives ~200 kg a head a year against a real
        // industrial roundwood figure of ~180, and the plastics come to
        // ~62 kg against a real ~50.
        inputs: &[
            (Commodity::Machinery, 0.28),
            (Commodity::Plastics, 0.04),
            (Commodity::Timber, 0.20),
        ],
        outputs: &[(Commodity::RetailGoods, 1.0)],
        power: 0.6,
        // Final assembly and finishing. The bulk of manufacturing labour
        // now sits in the machine works upstream.
        labour: 30.0,
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
    // -----------------------------------------------------------------
    // The rest of the tree: every good traceable to a primary resource.
    //
    // A single undifferentiated `RetailGoods` made at a depot said nothing
    // about what a country could make or what it had to buy. These give
    // the two resources the world had been generating and nobody had ever
    // asked for — standing timber and petroleum — somewhere to go.
    // -----------------------------------------------------------------
    Recipe {
        name: "forestry",
        inputs: &[],
        outputs: &[(Commodity::Timber, 1.0)],
        power: 0.05,
        // Mechanised harvesting is fast; the labour is in the haulage and
        // the mill rather than the felling.
        labour: 2.0,
        needs_water: false,
    },
    Recipe {
        name: "oil field",
        inputs: &[],
        outputs: &[(Commodity::Petroleum, 1.0)],
        power: 0.10,
        // **The most capital-intensive extraction there is.** A field
        // worth billions is run by a few hundred people, which is exactly
        // why oil wealth does not become employment and why a petro-state
        // has a labour market problem its revenue cannot solve.
        labour: 0.15,
        needs_water: false,
    },
    Recipe {
        name: "oil imports",
        inputs: &[],
        outputs: &[(Commodity::Petroleum, 1.0)],
        power: 0.02,
        labour: 0.1,
        needs_water: false,
    },
    Recipe {
        name: "timber imports",
        inputs: &[],
        outputs: &[(Commodity::Timber, 1.0)],
        power: 0.02,
        labour: 0.1,
        needs_water: false,
    },
    Recipe {
        name: "cracker",
        // Real naphtha cracking runs about 1.3-1.5 t of feedstock per
        // tonne of resin once the energy is counted.
        inputs: &[(Commodity::Petroleum, 1.4)],
        outputs: &[(Commodity::Plastics, 1.0)],
        power: 1.2,
        labour: 1.5,
        needs_water: false,
    },
    Recipe {
        name: "machine works",
        // Tools, engines, car parts, pumps. Mostly steel, with the
        // plastics a modern machine is full of.
        inputs: &[(Commodity::Steel, 0.72), (Commodity::Plastics, 0.08)],
        outputs: &[(Commodity::Machinery, 1.0)],
        power: 1.4,
        // Assembly is the labour-intensive end of manufacturing, which is
        // why it is the part that moves to wherever labour is cheap.
        labour: 60.0,
        needs_water: false,
    },
    Recipe {
        name: "cement works",
        // **Real: 3.2 GJ a tonne of thermal energy and 110 kWh of
        // electricity.** The heat is the process — you are calcining
        // limestone at 1,450 C — so the coal is most of the cost and all
        // of the reason a kiln sits where the fuel is.
        inputs: &[(Commodity::Coal, 0.12)],
        outputs: &[(Commodity::Cement, 1.0)],
        power: 0.11,
        // As capital-intensive as extraction. A modern kiln line making
        // 1.5 Mt a year is run by a couple of hundred people.
        labour: 0.5,
        needs_water: false,
    },
    Recipe {
        name: "cement imports",
        inputs: &[],
        outputs: &[(Commodity::Cement, 1.0)],
        power: 0.02,
        labour: 0.1,
        needs_water: false,
    },
    // **The building trade, which is where all of that ends up.**
    //
    // Half of construction output is repair and maintenance rather than
    // new build, and this is the recipe behind that line in `services.rs`:
    // a country consumes its own fabric and has to keep replacing it.
    // Output is not a commodity because a building is not shipped — the
    // materials are consumed into it, the way food is consumed into
    // people.
    Recipe {
        name: "building trade",
        // Per tonne of fabric put up. Aggregate and sand are the bulk of
        // any structure and are deliberately absent: real aggregate
        // travels under 50 km and is not a traded commodity at this
        // scale. What a country has to *get* is the binder, the metal and
        // the wood.
        inputs: &[
            (Commodity::Cement, 0.62),
            (Commodity::Steel, 0.06),
            (Commodity::Timber, 0.06),
        ],
        outputs: &[],
        power: 0.05,
        // Construction is 6.4% of employment and famously hard to
        // mechanise: the site comes to the work, never the other way.
        labour: 14.0,
        needs_water: false,
    },
    // **A hospital's supplies have to be made by somebody.**
    Recipe {
        name: "pharmaceutical works",
        // **Synthesis from chemicals, not from crude.** A pharmaceutical
        // works no more starts from a barrel of oil than a baker starts
        // from a field: it buys reagents and solvents from the chemical
        // industry, and the yields are poor — several tonnes of input per
        // tonne of active product is normal, which is much of why
        // medicines cost what they do.
        inputs: &[(Commodity::Chemicals, 2.2), (Commodity::Plastics, 0.25)],
        outputs: &[(Commodity::Medicine, 1.0)],
        // Cleanrooms run around the clock whether or not they are making
        // anything, which is why pharmaceutical plants are power-hungry
        // out of all proportion to their tonnage.
        power: 1.8,
        // Low volume, high skill: this is a graduate industry, and the
        // hours per tonne say so.
        labour: 40.0,
        needs_water: true,
    },
    Recipe {
        name: "medicine imports",
        inputs: &[],
        outputs: &[(Commodity::Medicine, 1.0)],
        power: 0.05,
        labour: 0.4,
        needs_water: false,
    },
    // **The chemical industry**, which sits between the refinery and
    // everybody who synthesises anything.
    Recipe {
        name: "chemical works",
        inputs: &[(Commodity::Petroleum, 1.1)],
        outputs: &[(Commodity::Chemicals, 1.0)],
        // Steam crackers and separation trains: the industry is one of
        // the largest industrial users of electricity there is.
        power: 1.6,
        labour: 3.0,
        needs_water: true,
    },
    Recipe {
        name: "chemical imports",
        inputs: &[],
        outputs: &[(Commodity::Chemicals, 1.0)],
        power: 0.02,
        labour: 0.15,
        needs_water: false,
    },
    // **Retail remedies, which are a different industry from medicine.**
    //
    // Same chemistry, wholly different manufacturing: a paracetamol line
    // is high-volume tabletting on a commodity active, while a sterile
    // injectable is made under GMP in a validated cleanroom with batch
    // traceability, environmental monitoring and a QA release. That is
    // why the yields, the labour and the price are not close — and why a
    // country can perfectly well make its own aspirin and still import
    // every vial of anaesthetic it uses.
    Recipe {
        name: "remedy works",
        inputs: &[(Commodity::Chemicals, 1.3), (Commodity::Plastics, 0.15)],
        outputs: &[(Commodity::Remedies, 1.0)],
        power: 0.8,
        labour: 12.0,
        needs_water: true,
    },
    Recipe {
        name: "remedy imports",
        inputs: &[],
        outputs: &[(Commodity::Remedies, 1.0)],
        power: 0.03,
        labour: 0.3,
        needs_water: false,
    },
    // **A hospital is a place that holds supplies**, not a line in a
    // budget.
    //
    // Modelled as a budget line it could not be supplied at all: nothing
    // in the country *wanted* medical grade, because no recipe consumed
    // it and no shop sold it, so `distribute` never moved a gram and the
    // entire national stock sat in the one town with the works while
    // every other hospital held nothing. Making it a site with a recipe
    // fixes that the same way it works for everybody else.
    //
    // A batch is one person served for one day, so `throughput` is the
    // population the hospital covers. It produces nothing that moves —
    // which is exactly what a service is, and why it keeps working when
    // the mill has shut.
    Recipe {
        name: "hospital",
        inputs: &[
            // 12 kg a head a year of medical grade, and a couple of kilos
            // of equipment on a replacement cycle.
            (Commodity::Medicine, 0.012 / 365.0),
            (Commodity::Machinery, 0.0025 / 365.0),
        ],
        outputs: &[],
        // Lights, heating, imaging, theatres: a hospital is one of the
        // most power-hungry buildings a town has, and one of the few that
        // must never lose supply.
        power: 0.9 / 365.0,
        // Health is 1 post per 45 people at ~1,800 hours a year, which is
        // this per person-day.
        labour: 0.11,
        needs_water: true,
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
    pub const FORESTRY: usize = 16;
    pub const OIL_FIELD: usize = 17;
    pub const OIL_IMPORTS: usize = 18;
    pub const TIMBER_IMPORTS: usize = 19;
    pub const CRACKER: usize = 20;
    pub const MACHINE_WORKS: usize = 21;
    pub const CEMENT_WORKS: usize = 22;
    pub const CEMENT_IMPORTS: usize = 23;
    pub const BUILDING_TRADE: usize = 24;
    pub const PHARMA: usize = 25;
    pub const MEDICINE_IMPORTS: usize = 26;
    pub const CHEMICAL_WORKS: usize = 27;
    pub const CHEMICAL_IMPORTS: usize = 28;
    pub const REMEDY_WORKS: usize = 29;
    pub const REMEDY_IMPORTS: usize = 30;
    pub const HOSPITAL: usize = 31;
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
    /// **What it costs to produce here, before scarcity says anything.**
    ///
    /// Kept apart from the price, and the distinction is the whole of what
    /// makes propagation work. A shortage of grain raises the *price* of
    /// grain; it does not make grain any cheaper or dearer to grow. If cost
    /// were built from input prices, that one shortage would be counted
    /// again in flour's cost, again in bread's, and again in bread's own
    /// scarcity multiplier — which is the compounding failure this project
    /// has now recorded three times.
    ///
    /// So **cost carries what a thing genuinely costs to make** — a rich
    /// seam, cheap hydro power, a better process — and price is that times
    /// the local balance of supply and demand. A glut of oil makes cheap
    /// plastics because it makes oil cheaper to *produce* nothing to do
    /// with anybody's stock level; and demand outstripping supply raises
    /// the price at each stage on its own merits, once.
    pub cost: Basket,
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
            cost: price,
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
    /// **Who actually moves the goods.**
    ///
    /// `distribute` is a pull and `trade` is a per-hop price test; both
    /// are pairwise, so a cargo three towns down the road has to clear a
    /// separate test at every hop and usually never sets off. A freight
    /// operator plans the whole journey before the lorry leaves, which is
    /// a different algorithm rather than a better-tuned version of the
    /// same one.
    pub logistics: Option<crate::logistics::Logistics>,
    /// **Who has money.**
    ///
    /// The economy priced everything and paid for nothing: households took
    /// goods off a shelf without the shop being better off, and a wage
    /// came from nowhere. A wage has to be an expense somebody bears
    /// before a firm can fail to bear it.
    pub treasury: crate::money::Treasury,
    /// Hands on today at each site, filled by `labour::update`. Payroll is
    /// paid by a particular employer, so it needs the breakdown.
    pub staff_today: Vec<f64>,
    /// **What share of its wage bill each firm has been able to meet**,
    /// smoothed over weeks.
    ///
    /// This is the number the whole money layer was built to produce. A
    /// firm that cannot pay does not pay a negative wage — it employs
    /// fewer people, which is what real disinflation with sticky wages
    /// does and what this model could not express while a wage came from
    /// nowhere.
    pub payroll_met: Vec<f64>,
    /// **What share of its wage bill the state could raise**, carried from
    /// yesterday.
    ///
    /// Services are paid before the day's tax is collected — a hospital
    /// cannot meet today's payroll out of money it will be given this
    /// evening — so what it can be paid has to be judged on what the
    /// treasury managed last time. A state that cannot collect enough
    /// under-staffs its hospitals, which is exactly how `Capacity` already
    /// says under-funding shows up: as fewer people, not a worse
    /// multiplier.
    pub state_afford: f64,
    /// **The fabric of each town**, in tonnes of building.
    ///
    /// Real building stock comes to something like 50-60 tonnes a head
    /// once dwellings, shops, works and civic buildings are counted — a
    /// 76 m² house is about 150 tonnes and holds 2.4 people.
    pub building_stock: Vec<f64>,
    /// **And what condition it is in**, 0 to 1.
    ///
    /// This is what a service sector is *for*. Construction is 6.4% of
    /// employment and **half of its output is repair and maintenance**
    /// rather than new build — a fact this project has recorded since
    /// `services.rs` was written without anything having a condition to
    /// maintain. A building that is not kept up does not vanish; it
    /// degrades, and degraded stock is cheap stock.
    pub building_condition: Vec<f64>,
}

impl Economy {
    /// Advance one day.
    pub fn step(&mut self) {
        self.unserved_power = 0.0;
        self.unmet_demand = basket();
        // **Open the books at the start of the day, not wipe them at the
        // end.** Clearing on the way out left `today` empty for anything
        // that looked after `step` returned — which is everything.
        self.treasury.open_the_books();

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
        // Reads what the hospitals managed to run today, so it must come
        // after `produce`.
        self.supply_the_state();
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
        // **After the local pull, before the arbitrage.** A firm restocks
        // down the road first, then rings a haulier for what it cannot get
        // locally; speculating on a price gap is a different business
        // again, and it comes last.
        if let Some(mut freight) = self.logistics.take() {
            freight.haul(self, self.ledger.day);
            self.logistics = Some(freight);
        }
        self.trade();
        // **Wages, after the day's work is known.** `labour::update` has
        // already said who was on today; this is what it cost the people
        // who employed them.
        // Services are paid for before their wages fall due.
        self.pay_for_services();
        self.run_the_service_sector();
        self.pay_wages();
        // The state takes its share and pays its own staff out of it.
        self.tax_and_spend();
        // And what is left over after the wages is somebody's income too.
        self.distribute_profits();
        self.update_prices();
        // **At the end of the day, after everything has moved.** Meat
        // that was sold this morning is not in the cold store tonight,
        // and a cargo that arrived is.
        self.spoil_stock();
        // The fabric wears whether or not anybody keeps it up.
        self.maintain_buildings();
        self.discard_unused_power();
        // The same guarantee the commodity ledger gives for tonnage.
        #[cfg(debug_assertions)]
        self.treasury.assert_conserved();

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
                // **A hospital is never shed.** Real grids hold them
                // above everything, on a protected feeder with their own
                // generators, and it is the one load an operator will
                // black out a district to keep.
                SiteKind::Hospital => 0,
                SiteKind::Mine | SiteKind::IronMine | SiteKind::OilField => 1,
                // The food chain.
                SiteKind::Factory | SiteKind::Mill | SiteKind::Butcher => 2,
                SiteKind::Farm => 3,
                SiteKind::Shop => 4,
                // **Steel before the factories that eat it.** They are
                // both heavy industry, but shedding the steelworks to keep
                // the works running stops the works a fortnight later for
                // want of metal — the same reasoning that puts the
                // colliery first.
                SiteKind::Steelworks | SiteKind::Cracker | SiteKind::CementWorks
                | SiteKind::Pharma
                | SiteKind::ChemicalWorks => 5,
                // Heavy manufacturing, on an interruptible tariff.
                SiteKind::Works
                | SiteKind::MachineWorks
                | SiteKind::Forestry
                | SiteKind::Builders => 6,
                _ => 7,
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
        let day = self.ledger.day;
        /// **What a firm pays for an input, against what the next one
        /// down the chain sells it for.**
        ///
        /// Buying and selling at the same price gives every business in
        /// the country a gross margin of exactly nothing, so no shop could
        /// pay a cashier and no mill a miller: they took money in and paid
        /// all of it straight out again. Real gross margins are 25-30% in
        /// retail, 10-15% in wholesale and 20-35% in manufacturing, and a
        /// quarter is the round number in the middle of that.
        const WHOLESALE: f64 = 0.75;
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
                    // **And the buyer pays the seller.**
                    //
                    // Only shops took money from households, so every works
                    // upstream of a counter — farm, mill, mine, steelworks
                    // — had no income whatever. They drained their opening
                    // capital, could not make payroll and shed their staff,
                    // which is a supply chain with no revenue in it rather
                    // than a recession.
                    let due = qty * self.markets[market].price[c as usize] * WHOLESALE;
                    self.treasury.pay(
                        day,
                        crate::money::Account::Firm(dst),
                        crate::money::Account::Firm(src),
                        due,
                        crate::money::Why::Supply,
                    );
                    short -= qty;
                }
            }
        }
    }

    /// People eat. Demand is a floor: what cannot be met is recorded as
    /// people going without, not quietly reduced.
    fn consume_households(&mut self) {
        let day = self.ledger.day;
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
                    // **And somebody pays for it.** Goods came off the
                    // shelf and the shop was no better off, which is the
                    // whole reason a wage could not be anybody's cost.
                    let due = take * self.markets[m].price[c as usize];
                    self.treasury.pay(
                        day,
                        crate::money::Account::Households(m),
                        crate::money::Account::Firm(site),
                        due,
                        crate::money::Why::Purchase,
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

    /// **What share of its supplies the country's hospitals actually
    /// got**, read off the hospitals themselves.
    ///
    /// They are ordinary sites with an ordinary recipe, so `produce` has
    /// already consumed what they could get and `ran` says how much
    /// service that bought. Nothing here shops on the state's behalf —
    /// which was the earlier design, and it could not work, because a
    /// commodity nobody wants is a commodity nothing ever delivers.
    fn supply_the_state(&mut self) {
        if self.government.is_none() {
            return;
        }
        let mut rated = 0.0;
        let mut served = 0.0;
        for s in self.ledger.sites.iter() {
            if s.kind != SiteKind::Hospital {
                continue;
            }
            rated += s.throughput;
            served += s.ran;
        }
        if let Some(gov) = self.government.as_mut() {
            gov.supplied = if rated > 1e-9 {
                (served / rated).clamp(0.0, 1.0)
            } else {
                1.0
            };
        }
    }

    /// **Put money into the world**, once, sized on what it has to move.
    ///
    /// A money stock is not arbitrary. Real narrow money runs somewhere
    /// around a third of a year's consumption, so ninety days of what the
    /// country's households actually spend is the right order — and it is
    /// split the way real balances are, with households holding most of
    /// it and firms working capital against their trade.
    pub fn issue_currency(&mut self) {
        use crate::money::Account;
        const DAYS_OF_SPENDING: f64 = 90.0;

        let mut per_market = vec![0.0f64; self.markets.len()];
        for m in 0..self.markets.len() {
            per_market[m] = Commodity::ALL
                .iter()
                .map(|&c| {
                    self.markets[m].daily_household_demand(c) * self.markets[m].price[c as usize]
                })
                .sum::<f64>()
                * DAYS_OF_SPENDING;
        }
        let national: f64 = per_market.iter().sum();

        // Households hold the bulk of narrow money; a firm holds working
        // capital rather than a fortune.
        for m in 0..self.markets.len() {
            self.treasury.open(Account::Households(m), per_market[m] * 0.55);
        }
        let firms: Vec<usize> = (0..self.ledger.sites.len()).collect();
        if !firms.is_empty() {
            let each = national * 0.35 / firms.len() as f64;
            for i in firms {
                self.treasury.open(Account::Firm(i), each);
            }
        }
        self.treasury.open(Account::State, national * 0.10);
        // The rest of the world starts with a great deal, because from
        // here it is effectively unlimited — what matters is that trade
        // moves money across the boundary rather than conjuring it.
        self.treasury.open(Account::Abroad, national * 10.0);
    }

    /// **Payroll: the point of the whole exercise.**
    ///
    /// A firm pays the people who worked there today, out of its own
    /// balance. What it cannot pay is recorded rather than waived, because
    /// a firm that cannot make payroll is the mechanism the project's
    /// notes have been missing:
    ///
    /// > Real disinflation with sticky wages causes unemployment for
    /// > exactly this reason — firms cannot afford the real wage.
    ///
    /// The wage itself is the one `person.rs` already uses: a multiple of
    /// the **settled** cost of a day's food, not today's price, because
    /// nominal wages are renegotiated about once a year and that lag is
    /// how a supply shock makes people poorer.
    fn pay_wages(&mut self) {
        use crate::money::{Account, Why};
        let day = self.ledger.day;
        if self.staff_today.len() != self.ledger.sites.len() {
            return;
        }
        if self.payroll_met.len() != self.ledger.sites.len() {
            self.payroll_met.resize(self.ledger.sites.len(), 1.0);
        }
        for site in 0..self.ledger.sites.len() {
            let hands = self.staff_today[site];
            if hands <= 0.0 {
                continue;
            }
            let m = self.ledger.sites[site].market;
            let bill = hands * self.day_rate_here(m);
            let paid = self.treasury.pay(
                day,
                Account::Firm(site),
                Account::Households(m),
                bill,
                Why::Payroll,
            );
            // **Nobody is hired and fired by the day**, so what a firm
            // could afford this week is a slow average of what it has been
            // affording. The same three-week stickiness the labour market
            // already uses everywhere else: firms hoard labour through a
            // short stoppage and only shed it when the shortfall persists.
            const STICKY_DAYS: f64 = 21.0;
            let met = if bill > 0.0 { paid / bill } else { 1.0 };
            let a = 1.0 / STICKY_DAYS;
            self.payroll_met[site] = self.payroll_met[site] * (1.0 - a) + met * a;
        }
    }

    /// **The state taxes, and then it pays people.**
    ///
    /// `state.rs` already sizes a public sector properly — health,
    /// education and administration at one post per 45 people, which is
    /// the 14-21% of the workforce real governments employ — but those
    /// posts were an accounting fact and nobody drew a wage from them.
    ///
    /// **A state sizes its take to its spending.** The first attempt
    /// levied `Capacity::tax_take` on every firm's takings, which
    /// over-collects badly: a tax rate is quoted against *value added* and
    /// turnover counts the same value at every step of a supply chain, so
    /// a 38% rate on turnover took nearly ten billion against six billion
    /// of spending and the treasury simply hoarded the difference. That is
    /// precisely why real turnover taxes are levied on the value added.
    ///
    /// So the wage bill is computed first and collected second, in
    /// proportion to who took money today — capped by what the state can
    /// actually reach. **A weak state cannot tax what it cannot reach**,
    /// and the shortfall shows up the way it does everywhere else: as
    /// fewer people paid, not a worse multiplier.
    fn tax_and_spend(&mut self) {
        use crate::money::{Account, Why};
        let Some(gov) = self.government.as_ref() else {
            return;
        };
        let day = self.ledger.day;
        let ceiling = gov.capacity.tax_take() * gov.capacity.collection();
        let posts: Vec<f64> = (0..self.markets.len()).map(|m| gov.posts_in(m)).collect();

        let mut bill: f64 = (0..self.markets.len())
            .map(|m| posts[m] * self.day_rate_here(m))
            .sum();
        // Hospitals are the state's payroll too, and it has to raise the
        // money for them like everything else it does.
        for site in 0..self.ledger.sites.len() {
            if self.ledger.sites[site].kind == SiteKind::Hospital {
                let m = self.ledger.sites[site].market;
                bill += self.staff_today.get(site).copied().unwrap_or(0.0)
                    * self.day_rate_here(m);
            }
        }
        if bill <= 0.0 {
            return;
        }

        // Who took money over the counter today, and how much.
        let takings: Vec<(usize, f64)> = self
            .treasury
            .today
            .iter()
            .filter(|t| t.why == Why::Purchase)
            .filter_map(|t| match t.to {
                Account::Firm(i) => Some((i, t.amount)),
                _ => None,
            })
            .collect();
        let turnover: f64 = takings.iter().map(|&(_, a)| a).sum();
        if turnover <= 0.0 {
            return;
        }

        // What it needs, or what it can reach, whichever is less.
        let wanted = bill.min(turnover * ceiling);
        let rate = wanted / turnover;
        let mut collected = 0.0;
        for (firm, amount) in takings {
            collected += self.treasury.pay(
                day,
                Account::Firm(firm),
                Account::State,
                amount * rate,
                Why::Tax,
            );
        }

        // And pay its own people out of it. A teacher is a job somebody
        // holds, and a state that could not collect enough employs fewer
        // of them rather than paying them less.
        let afford = if bill > 0.0 {
            (collected / bill).min(1.0)
        } else {
            0.0
        };
        // Remembered for tomorrow, when the hospitals are paid before any
        // of this has happened.
        self.state_afford = afford;
        for m in 0..self.markets.len() {
            let pay = posts[m] * self.day_rate_here(m) * afford;
            self.treasury.pay(
                day,
                Account::State,
                Account::Households(m),
                pay,
                Why::PublicSpending,
            );
        }
    }

    /// **The service sector earns and pays like anybody else.**
    ///
    /// Construction, hospitality, recreation and offices come to 37% of
    /// employment, and until now not one of those people was paid by
    /// anybody: `services.rs` counted the posts and the money to fill them
    /// came from nowhere. That is why profit was doing four fifths of the
    /// work of getting money to households when real wages are about
    /// three fifths of household income.
    ///
    /// **Nobody imports a haircut**, so the customer is the town itself.
    /// Households buy the service, the sector pays its people, and what is
    /// left over is its owners' — the same circuit a firm runs, with the
    /// sector pooled per town because it has no premises here.
    fn run_the_service_sector(&mut self) {
        use crate::money::{Account, Why};
        /// A service business keeps a margin over its wage bill: rent,
        /// equipment, and the owner's living. Real service-sector gross
        /// margins run 15-40%; hospitality is at the bottom of that and
        /// professional work at the top.
        const MARGIN: f64 = 1.25;

        let Some(svc) = self.services.as_ref() else {
            return;
        };
        let day = self.ledger.day;
        let posts: Vec<f64> = (0..self.markets.len())
            .map(|m| svc.total_in(m))
            .collect();

        for m in 0..self.markets.len() {
            if posts[m] <= 0.0 {
                continue;
            }
            let wages = posts[m] * self.day_rate_here(m);
            // Bought by the households of the town it stands in.
            self.treasury.pay(
                day,
                Account::Households(m),
                Account::ServiceSector(m),
                wages * MARGIN,
                Why::Purchase,
            );
            // Paid to the people who did the work.
            self.treasury.pay(
                day,
                Account::ServiceSector(m),
                Account::Households(m),
                wages,
                Why::Payroll,
            );
            // And the margin is somebody's income too.
            let over = self.treasury.balance(Account::ServiceSector(m)) - wages * 30.0;
            if over > 0.0 {
                self.treasury.pay(
                    day,
                    Account::ServiceSector(m),
                    Account::Households(m),
                    over,
                    Why::Profit,
                );
            }
        }
    }

    /// **Somebody pays the builders.**
    ///
    /// The building trade consumes cement, steel and timber and produces
    /// nothing that can be shipped — the materials go into a building, the
    /// way food goes into people. That makes it a service, and it had the
    /// same problem the hospital had: staff, costs, and no customer.
    ///
    /// Households pay, because in the end a building is somebody's home or
    /// somebody's premises and construction is bought out of income.
    fn pay_for_services(&mut self) {
        use crate::money::{Account, Why};
        let day = self.ledger.day;
        // **A hospital is a firm the state pays.** It produces nothing
        // that can be shipped and sells to nobody, which is exactly what
        // makes it a service — and it left the one site in the country
        // that must never stop with no way of paying its staff.
        //
        // Paid here rather than in `tax_and_spend`, because that runs
        // after wages fall due and a hospital cannot meet today's payroll
        // out of money it will be given this evening.
        for site in 0..self.ledger.sites.len() {
            if self.ledger.sites[site].kind != SiteKind::Hospital {
                continue;
            }
            let m = self.ledger.sites[site].market;
            let hands = self.staff_today.get(site).copied().unwrap_or(0.0);
            let due = hands * self.day_rate_here(m) * self.state_afford;
            self.treasury.pay(
                day,
                Account::State,
                Account::Firm(site),
                due,
                Why::PublicSpending,
            );
        }

        for site in 0..self.ledger.sites.len() {
            if self.ledger.sites[site].kind != SiteKind::Builders {
                continue;
            }
            let m = self.ledger.sites[site].market;
            let hands = self.staff_today.get(site).copied().unwrap_or(0.0);
            // Wages plus what the materials cost them, which is what a
            // builder actually charges for.
            let materials: f64 = self
                .treasury
                .today
                .iter()
                .filter(|t| t.from == Account::Firm(site) && t.why == Why::Supply)
                .map(|t| t.amount)
                .sum();
            let due = hands * self.day_rate_here(m) + materials;
            self.treasury.pay(
                day,
                Account::Households(m),
                Account::Firm(site),
                due,
                Why::Purchase,
            );
        }
    }

    /// **The residual belongs to somebody.**
    ///
    /// Wages were the first flow and on their own they do not close the
    /// circuit: firms took in three times what they paid out, households
    /// were drained inside a fortnight, and the difference sat in company
    /// balances doing nothing. That difference is **profit**, and in a
    /// real economy it is not lost — it is somebody's income.
    ///
    /// A firm keeps working capital against its own wage bill and remits
    /// what is over. Real corporate cash holdings run to a month or two of
    /// operating costs, which is what the reserve is set to.
    ///
    /// **Known simplification, and a real one:** profit goes to households
    /// in the firm's own town, evenly. Real ownership is concentrated —
    /// `building.rs` already knows that almost every *business* is one
    /// person while almost every *job* is at a company — so this
    /// understates inequality considerably. It conserves, which is what
    /// this pass is for; who owns what is the next question, not this one.
    fn distribute_profits(&mut self) {
        use crate::money::{Account, Why};
        /// Days of its own payroll a firm holds as working capital.
        const RESERVE_DAYS: f64 = 45.0;

        let day = self.ledger.day;
        if self.staff_today.len() != self.ledger.sites.len() {
            return;
        }
        // What each firm actually paid out today, whatever the reason.
        // A reserve sized on payroll alone starves a works that buys far
        // more in materials than it pays in wages — a steelworks' ore bill
        // dwarfs its payroll — so the remittance stripped it of the money
        // it needed to buy next week's ore.
        let mut outgoings = vec![0.0f64; self.ledger.sites.len()];
        for t in self.treasury.today.iter() {
            if let crate::money::Account::Firm(i) = t.from {
                if i < outgoings.len() {
                    outgoings[i] += t.amount;
                }
            }
        }
        for site in 0..self.ledger.sites.len() {
            let m = self.ledger.sites[site].market;
            let wage = self.day_rate_here(m);
            let reserve = (outgoings[site] * RESERVE_DAYS)
                .max(self.staff_today[site] * wage * RESERVE_DAYS)
                .max(wage * 30.0);
            let held = self.treasury.balance(Account::Firm(site));
            let surplus = held - reserve;
            if surplus <= 0.0 {
                continue;
            }
            self.treasury.pay(
                day,
                Account::Firm(site),
                Account::Households(m),
                surplus,
                Why::Profit,
            );
        }
    }

    /// What a day's work fetches in this market, against the settled cost
    /// of living rather than today's price.
    fn day_rate_here(&self, m: usize) -> f64 {
        /// Real low-wage work buys 6-10 days of food for a day's labour.
        const DAYS_OF_FOOD: f64 = 6.0;
        let (anchor, index) = match self.workforce.get(m) {
            Some(w) if w.food_anchor > 0.0 => (w.food_anchor, w.wage_index),
            _ => (
                self.price(m, Commodity::ProcessedFood)
                    * Commodity::ProcessedFood.per_capita_annual()
                    / 365.0,
                1.0,
            ),
        };
        anchor * DAYS_OF_FOOD * index
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
    /// **Prices have to be worked out in the right order**, and the graph
    /// has a loop in it: coal makes electricity and a colliery runs on
    /// electricity. So rather than a topological sort that cannot exist,
    /// the pass is run until it settles — which it does quickly, because
    /// the feedback is tiny (a colliery uses 0.02 MWh a tonne).
    /// **Once, in dependency order — not iterated to a fixed point.**
    ///
    /// The obvious thing is to run the pass until the costs settle, since
    /// the graph has one loop in it (coal makes electricity and a colliery
    /// runs on electricity). That is wrong, and wrong in the way this
    /// project has now been caught by three times: **iterating does not
    /// converge, it compounds.** Each pass recomputes a cost from the last
    /// pass's costs, so a stage sitting below its reference drags the next
    /// stage lower again, and again, geometrically down the chain. Six
    /// passes moved the world price spread for medicine from 2.0x to 4.2x
    /// without a single input actually changing.
    ///
    /// One pass over the commodities in order is enough: the chain is four
    /// or five deep and roughly upstream-first already, and the one
    /// feedback loop is worth 0.02 MWh a tonne and does not need solving.
    fn update_prices(&mut self) {
        self.one_price_pass();
    }

    fn one_price_pass(&mut self) {
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

                // **What it costs to make does not depend on whether
                // anybody wants it today**, so this is worked out before
                // the demand is even looked at.
                //
                // It used to bail out here, which left a commodity nobody
                // currently wants frozen at whatever its cost was when the
                // market opened. Switching a country's building trade off
                // for twenty years then left cement at its full reference
                // cost while the same country with builders had it at two
                // thirds — so a town left to rot came out *dearer* to buy
                // into than one kept up, which is the opposite of the truth
                // and is what caught this.
                let cost = self.cost_of_production(m, c);
                self.markets[m].cost[c as usize] = cost;

                let demand = self.markets[m].daily_household_demand(c) + industrial;
                if demand <= 0.0 {
                    self.markets[m].cover[c as usize] = f64::INFINITY;
                    // **A glut needs surplus stock, not merely an absence
                    // of buyers.** Pricing everything nobody wants at the
                    // floor invented a cheap market in every town that had
                    // no use for a thing, and hauliers went chasing it —
                    // which widened the world price spread for medicine
                    // from 1.6x to 8.8x. With nothing in the warehouse
                    // there is no market at all, and what it would fetch is
                    // what it costs.
                    let held: f64 = (0..self.ledger.sites.len())
                        .filter(|&s| self.ledger.sites[s].market == m)
                        .map(|s| self.ledger.stock(s, c))
                        .sum();
                    self.markets[m].price[c as usize] =
                        if held > 0.0 { cost * 0.7 } else { cost };
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
                // **Price is what it cost to make, times what scarcity is
                // doing to it.** Not a typed-in constant times scarcity,
                // which is what this was: a glut of oil could never make
                // cheap plastics, because plastics had a fixed reference
                // cost of its own and nothing about the oil reached it.
                //
                // Cost sets the floor and scarcity sets the deviation,
                // which is how a real market works: producers will not sell
                // below cost for long, and a shortage bids the price above
                // it however cheap the inputs were.
                self.markets[m].price[c as usize] = cost * multiplier;
            }
        }
    }

    /// **What a tonne of it actually cost to produce here.**
    ///
    /// The point of this, and the thing the model could not do before: a
    /// glut of oil has to become cheap plastics and then cheap goods.
    /// Price used to be a typed-in reference cost times a scarcity
    /// multiplier, so nothing about the oil ever reached the plastics.
    ///
    /// **Measured as a ratio against the reference, not built up from
    /// nothing.** The first attempt added up the listed inputs, the power
    /// and the labour and called that the cost, which understates a primary
    /// commodity enormously: a recipe for grain lists no land, no
    /// machinery, no fuel, no fertiliser and no seed, so the sum came to a
    /// third of what grain really costs and every acceptance test in the
    /// economy moved.
    ///
    /// So the reference cost stays exactly what it was — it is calibrated
    /// against real prices and there is no reason to throw that away — and
    /// what propagates is **how far the inputs have moved from their own
    /// reference**. Everything at reference gives exactly the old number;
    /// oil at half price gives plastics at proportionally less, through
    /// however many stages lie between.
    pub fn cost_of_production(&self, m: usize, c: Commodity) -> f64 {
        let mut best: Option<f64> = None;
        for s in 0..self.ledger.sites.len() {
            let site = &self.ledger.sites[s];
            if site.market != m || site.throughput <= 0.0 {
                continue;
            }
            let Some(r) = site.recipe else { continue };
            let recipe = &RECIPES[r];
            if !recipe.outputs.iter().any(|&(oc, q)| oc == c && q > 0.0) {
                continue;
            }

            // What this recipe's inputs cost at the reference, and what
            // they cost today. **A power figure of 1e9 is the sentinel
            // meaning "whatever the grid can carry"** — this project has
            // been bitten by reading it as a rate twice already — so it is
            // excluded rather than multiplied by anything.
            let power = if recipe.power < 1e8 { recipe.power } else { 0.0 };
            let labour = recipe.labour * WAGE_AN_HOUR;
            let reference: f64 = recipe
                .inputs
                .iter()
                .map(|&(ic, q)| q * ic.base_cost())
                .sum::<f64>()
                + power * Commodity::Electricity.base_cost()
                + labour;
            // **Input *costs*, not input prices.** A shortage of grain
            // raises the price of grain and does not make it dearer to
            // grow, so reading prices here would count one shortage again
            // at every stage downstream and once more in the final good's
            // own scarcity multiplier.
            let actual: f64 = recipe
                .inputs
                .iter()
                .map(|&(ic, q)| q * self.markets[m].cost[ic as usize])
                .sum::<f64>()
                + power * self.markets[m].cost[Commodity::Electricity as usize]
                + labour;

            // **And what this particular ground costs to work**, which is
            // the whole difference between a rich seam and a thin one and
            // is 1.0 for anything built to a design rather than found.
            let moved = if reference > 1e-9 { actual / reference } else { 1.0 };
            let here = c.base_cost() * moved * site.cost_factor;
            best = Some(best.map_or(here, |b: f64| b.min(here)));
        }
        // Nobody here makes it, so what it costs is what it costs to bring
        // in — which is the reference, that being what it is calibrated on.
        best.unwrap_or_else(|| c.base_cost())
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

    /// **Buildings wear out, and somebody has to keep them up.**
    ///
    /// Real maintenance runs **1-2% of a building's capital value a
    /// year** — a roof lasts 25-50 years, wiring 30-40, a boiler 15,
    /// paint 5-10 — and a stock that is not maintained loses condition
    /// rather than disappearing.
    ///
    /// The arithmetic closes on figures that were already here and were
    /// never put side by side: `region.rs` sizes the building trade at
    /// **0.81 tonnes of fabric a head a year**, and a stock of 50-60
    /// tonnes a head needing 1.5% a year wants **0.75-0.9 tonnes a head a
    /// year** to stand still. In other words the entire construction
    /// industry of this model is, to within a rounding error, doing
    /// nothing but maintenance — which is exactly what the real figure
    /// says and what nothing here could previously express.
    ///
    /// The same shape as the road network's maintenance deficit, and for
    /// the same reason: the shortfall compounds slowly enough that
    /// whoever let it happen is long gone before it shows.
    fn maintain_buildings(&mut self) {
        /// Tonnes of fabric per head of population.
        const STOCK_PER_HEAD: f64 = 55.0;
        /// Share of the stock that must be renewed each year to stand
        /// still.
        const UPKEEP_A_YEAR: f64 = 0.015;
        /// How fast condition falls when the work is not done. Slow, like
        /// the roads: a decade of neglect is visible and a year is not.
        const DECAY: f64 = 0.06;

        let n = self.markets.len();
        if self.building_stock.len() != n {
            self.building_stock = (0..n)
                .map(|m| self.markets[m].population * STOCK_PER_HEAD)
                .collect();
            self.building_condition = vec![1.0; n];
        }

        // What the builders actually got done today, town by town.
        let mut done = vec![0.0f64; n];
        for site in self.ledger.sites.iter() {
            if site.kind == SiteKind::Builders {
                done[site.market] += site.ran;
            }
        }

        for m in 0..n {
            let wanted = self.building_stock[m] * UPKEEP_A_YEAR / DAYS_PER_YEAR as f64;
            if wanted <= 0.0 {
                continue;
            }
            let met = (done[m] / wanted).min(1.0);
            let gap = 1.0 - met;
            let a = DECAY / DAYS_PER_YEAR as f64;
            // Falls toward what the upkeep supports, rather than to zero:
            // a half-maintained town settles at a half-maintained state.
            let floor = 1.0 - gap;
            self.building_condition[m] += (floor - self.building_condition[m]) * a;
            self.building_condition[m] = self.building_condition[m].clamp(0.05, 1.0);
        }
    }

    /// What a house costs to buy, and what a room costs to rent, both fall
    /// with the state of the fabric. **Degraded stock is cheap stock**,
    /// which is how under-maintained housing becomes the only housing some
    /// people can afford.
    pub fn fabric_condition(&self, m: usize) -> f64 {
        self.building_condition.get(m).copied().unwrap_or(1.0)
    }

    /// **What a house costs to buy**, in this market.
    ///
    /// Not a number typed in: it is the bill of materials `building.rs`
    /// gives for a dwelling, priced at what those materials actually cost
    /// here, plus the labour to put them together and what the ground is
    /// worth.
    ///
    /// Real proportions: **materials are 40-50% of a build cost** and the
    /// land is anything from a tenth of the price in the country to more
    /// than the building in a city — which is why the land term follows
    /// the size of the town rather than being flat. The whole thing comes
    /// out near the real **8x median annual income**, and nothing was
    /// tuned to make it: a dwelling is 76 m², a wage is six days of food,
    /// and the multiple falls out of those two.
    pub fn house_price(&self, m: usize) -> f64 {
        use crate::building::Use;
        let bill = Use::Dwelling.materials(0.0);
        let materials = bill.cement * self.price(m, Commodity::Cement)
            + bill.steel * self.price(m, Commodity::Steel)
            + bill.timber * self.price(m, Commodity::Timber);
        /// Materials are a little under half of what a build costs; the
        /// rest is the trades who assemble them.
        const MATERIAL_SHARE: f64 = 0.45;
        let built = materials / MATERIAL_SHARE;
        // Land, against the size of the place. A plot in a village is a
        // tenth of the house; in a large city it is worth more than the
        // building standing on it.
        let people = self.markets.get(m).map(|x| x.population).unwrap_or(0.0);
        // Referenced against a large city rather than a million, or every
        // town in a country whose smallest settlement holds two million
        // people saturates the curve and they all cost the same.
        let land = built * (0.1 + 0.85 * (people / 8.0e6).min(1.0).sqrt());
        // A worn-out house is a cheap house. The land under it is not.
        built * (0.4 + 0.6 * self.fabric_condition(m)) + land
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
