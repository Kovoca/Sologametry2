//! Buildings built out of fixtures, and the jobs that fall out of them.
//!
//! The same idea as `vehicle.rs`, applied where the design doc says it
//! should be: *"a tile-layered data model, generalizing CDDA's vehicle-part
//! system to buildings."* A vehicle is frames and engines and cargo bays; a
//! shop is shelves and tills and a loading dock. Neither has its
//! capabilities typed in.
//!
//! **The point here is the staff.** A shop does not employ people because
//! a table says retail is a tenth of the workforce. It employs them
//! because somebody has to work each till, fill each shelf and unload each
//! lorry, and how many of each it has depends on how much it sells. Take
//! the tills out and the cashiers go with them.
//!
//! That is why this exists now: the economy modelled farms, mills,
//! canneries and mines — the small half of employment — and gave a nation
//! of sixteen million people shops that employed **nobody at all**. Retail
//! is around a tenth of all jobs in a developed economy against about 1.5%
//! in food manufacturing, so the half that was missing was the big one.
//!
//! Not yet built: the walkable tile-by-tile interior, the wall and floor
//! layers, and the electrical and water nodes hung off them. Fixtures
//! first, the same way parts came before vehicle interiors.

/// A piece of fitted-out furniture that does a job.
///
/// Deliberately coarse — a bank of shelving, not each shelf. It gets finer
/// when there is a reason for it to.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Fixture {
    /// A checkout. Somebody has to ring the goods through.
    Till,
    /// A bay of shelving on the shop floor. Somebody has to keep it full.
    Shelving,
    /// Racking in the stockroom out the back.
    StockRack,
    /// A dock for the lorries. Somebody has to unload them.
    LoadingBay,
    /// A served counter — bakery, deli, fishmonger.
    Counter,
}

impl Fixture {
    /// **People it takes to work it**, as full-time equivalents covering
    /// the hours the shop is open *(real, near enough)*.
    ///
    /// A checkout needs somebody on it whenever the doors are open, which
    /// is more than one person's week. Shelf-filling is the biggest single
    /// job in a supermarket and is done by fewer people spread over more
    /// stock. A loading dock is a couple of hands.
    ///
    /// Sanity check against a real large supermarket: about 75 t of goods
    /// a day and roughly 300 staff. The fixtures below give something near
    /// 190, the shortfall being management, cleaning, security and the
    /// online picking that none of these fixtures represent.
    pub fn staff(self) -> f64 {
        match self {
            Fixture::Till => 1.4,
            Fixture::Shelving => 0.25,
            Fixture::StockRack => 0.04,
            Fixture::LoadingBay => 1.2,
            Fixture::Counter => 1.6,
        }
    }

    /// Tonnes a day it can put through, where that is what limits it.
    pub fn throughput_t(self) -> f64 {
        match self {
            // A till serves ~25 customers an hour and a basket is about
            // 7 kg, so 175 kg an hour and something over 2 t across a
            // trading day.
            Fixture::Till => 2.5,
            Fixture::LoadingBay => 40.0,
            Fixture::Counter => 0.4,
            _ => 0.0,
        }
    }

    /// Tonnes it will hold, where holding is what it is for.
    pub fn holds_t(self) -> f64 {
        match self {
            Fixture::Shelving => 0.4,
            Fixture::StockRack => 3.0,
            _ => 0.0,
        }
    }

    /// Floor it takes up, in square metres.
    pub fn floor_m2(self) -> f64 {
        match self {
            Fixture::Till => 6.0,
            Fixture::Shelving => 4.0,
            Fixture::StockRack => 3.0,
            Fixture::LoadingBay => 40.0,
            Fixture::Counter => 12.0,
        }
    }

    /// What fitting it out costs, in days of an unskilled wage.
    pub fn price_in_wage_days(self) -> f64 {
        match self {
            Fixture::Till => 20.0,
            Fixture::Shelving => 4.0,
            Fixture::StockRack => 3.0,
            Fixture::LoadingBay => 60.0,
            Fixture::Counter => 45.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Fixture::Till => "checkout",
            Fixture::Shelving => "shelving",
            Fixture::StockRack => "stockroom racking",
            Fixture::LoadingBay => "loading bay",
            Fixture::Counter => "served counter",
        }
    }
}

/// **How many people one person can actually supervise** *(real)*.
///
/// Span of control runs about eight to fifteen in retail and light
/// manufacturing — fewer where the work is skilled or dangerous, more
/// where it is repetitive and in one room. Managers and supervisors
/// together come to something like a tenth to a seventh of employment,
/// which is what a span of ten produces once it is applied twice.
pub const SPAN_OF_CONTROL: f64 = 10.0;

/// **Who owns it, which is a different question from who runs it.**
///
/// Position and ownership are separate axes — the design doc's rule — and
/// the form follows the size, because the reason to incorporate is that
/// the thing has grown past what one purse can carry or one person can be
/// liable for.
///
/// Real distribution *(US)*: about **73% of firms are sole
/// proprietorships and 19% corporations**, and yet corporations take some
/// **81% of business receipts** and nearly all the employment. Almost
/// every *business* is one person; almost every *job* is at a company.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Ownership {
    /// One owner, who works in it. **Unlimited liability**: the business's
    /// debts are his debts, and there is no capital but his own and what
    /// he can borrow against his house.
    SoleTrader,
    /// A few owners who work in it and are liable together. The
    /// professions run this way — law, medicine, accountancy — and so do
    /// most farms.
    Partnership,
    /// **A separate legal person.** Liability stops at the company, it can
    /// raise money by selling shares in itself, and the owners generally
    /// do not work there at all. That last part is what makes a manager a
    /// position rather than a proprietor: real authority, answerable
    /// upward to somebody who owns it and is somewhere else.
    Corporation,
}

impl Ownership {
    /// **The form follows the size**, because the reasons to incorporate
    /// are reasons that only arrive with scale: more capital than one
    /// person has, more risk than one person will carry, and more people
    /// than one person can be personally answerable for.
    ///
    /// The thresholds are where real businesses actually change form:
    /// below about six hands the owner is working alongside them, and past
    /// fifty or so almost everything is incorporated.
    pub fn for_size(staff: f64) -> Ownership {
        if staff < SMALL_ENOUGH_TO_RUN_YOURSELF {
            Ownership::SoleTrader
        } else if staff < 50.0 {
            Ownership::Partnership
        } else {
            Ownership::Corporation
        }
    }

    /// Whether whoever owns it is also on the premises working.
    pub fn owner_works_there(self) -> bool {
        self != Ownership::Corporation
    }

    /// Whether a bad year can take the owner's house.
    pub fn unlimited_liability(self) -> bool {
        self != Ownership::Corporation
    }

    /// Whether it can raise money from people who will never set foot in
    /// it. This is the whole reason the form exists.
    pub fn can_sell_shares(self) -> bool {
        self == Ownership::Corporation
    }

    pub fn name(self) -> &'static str {
        match self {
            Ownership::SoleTrader => "sole trader",
            Ownership::Partnership => "partnership",
            Ownership::Corporation => "company",
        }
    }
}

/// **Nobody runs more than about this many layers deep.** Real large
/// organisations are five to eight; Walmart's two million people are about
/// seven. Past that a hierarchy stops being able to hear its own bottom.
pub const MOST_LAYERS: usize = 8;

/// Below this many hands, nobody is a full-time anything.
///
/// **The owner works the till.** A corner shop has a proprietor who
/// serves, orders, sweeps up and does the books, and inventing a separate
/// manager for him is how you end up with three staff and two of them
/// supervising. Real small businesses are one person wearing every hat,
/// and the hats only separate once there are enough hands to need it.
const SMALL_ENOUGH_TO_RUN_YOURSELF: f64 = 6.0;

/// A building: a count of each fixture in it.
#[derive(Clone, Debug, Default)]
pub struct Building {
    pub fixtures: Vec<(Fixture, f64)>,
}

impl Building {
    /// Fit out a shop to sell `tonnes_per_day`, holding `cover_days` of it.
    ///
    /// **Everything is sized off the trade it does.** A village shop and a
    /// city supermarket are the same arrangement of the same furniture at
    /// different counts, which is why one employs four people and the
    /// other three hundred without either number being written down
    /// anywhere.
    pub fn shop(tonnes_per_day: f64, cover_days: f64) -> Self {
        let t = tonnes_per_day.max(0.0);
        let held = t * cover_days.max(0.0);
        // A third of the stock sits out on the shop floor and the rest is
        // out the back, which is roughly how a supermarket splits.
        Building {
            fixtures: vec![
                (Fixture::Till, (t / Fixture::Till.throughput_t()).ceil()),
                (
                    Fixture::Shelving,
                    (held * 0.33 / Fixture::Shelving.holds_t()).ceil(),
                ),
                (
                    Fixture::StockRack,
                    (held * 0.67 / Fixture::StockRack.holds_t()).ceil(),
                ),
                (
                    Fixture::LoadingBay,
                    (t / Fixture::LoadingBay.throughput_t()).ceil().max(1.0),
                ),
                // One served counter per few hundred tonnes a day: the
                // bakery, the deli, the fish slab.
                (Fixture::Counter, (t / 250.0).ceil().max(1.0)),
            ],
        }
    }

    fn sum(&self, f: impl Fn(Fixture) -> f64) -> f64 {
        self.fixtures.iter().map(|&(fx, n)| f(fx) * n).sum()
    }

    /// Hands on the floor: tills, shelves, the dock. Not the people
    /// watching them.
    pub fn floor_staff(&self) -> f64 {
        self.sum(|f| f.staff())
    }

    /// **How many people work here, at full stretch.**
    ///
    /// The floor, plus the people who see that the floor is doing what it
    /// was told, plus whoever answers for the lot of them.
    pub fn staff(&self) -> f64 {
        self.floor_staff() + self.supervisors() + self.managers()
    }

    /// Supervisors: one per span of hands, and none at all in a place
    /// small enough that the owner can see the whole of it from the door.
    pub fn supervisors(&self) -> f64 {
        let floor = self.floor_staff();
        if floor < SMALL_ENOUGH_TO_RUN_YOURSELF {
            return 0.0;
        }
        (floor / SPAN_OF_CONTROL).ceil()
    }

    /// Managers, including the one who owns it.
    ///
    /// **One person can be both**, and in a small place is. A shop with
    /// three staff has a proprietor who serves on the till; a shop with
    /// three hundred has a store manager, a deputy and a department head
    /// for each corner of it, and an owner who is somewhere else
    /// entirely.
    pub fn managers(&self) -> f64 {
        let floor = self.floor_staff();
        if floor < SMALL_ENOUGH_TO_RUN_YOURSELF {
            // The owner. He is also the manager, and the cashier.
            return 0.0;
        }
        // **Depth follows size, and it stacks.** One layer of supervision
        // is not enough once there are supervisors enough to need
        // supervising, so the pyramid is built up until the top layer is
        // small enough for one person to hold. Two fixed layers put the
        // same shape on a corner shop and a distribution centre.
        let mut layer = self.supervisors();
        let mut above = 0.0;
        for _ in 0..MOST_LAYERS {
            if layer <= 1.0 {
                break;
            }
            layer = (layer / SPAN_OF_CONTROL).ceil();
            above += layer;
        }
        // And whoever owns it, who is not one of the managers.
        above + 1.0
    }

    /// **How many layers there are between the floor and the top**, which
    /// is what "the depth depends on the size" means.
    ///
    /// Real organisations run five to eight layers at the very largest —
    /// Walmart's two million people are about seven deep — and a corner
    /// shop is one. The arithmetic is just repeated division by the span:
    /// ten hands need one chargehand, a hundred need a manager over the
    /// chargehands, a thousand need somebody over the managers.
    /// Who owns this one.
    pub fn ownership(&self) -> Ownership {
        Ownership::for_size(self.staff())
    }

    pub fn layers(&self) -> usize {
        let mut n = self.floor_staff();
        if n < SMALL_ENOUGH_TO_RUN_YOURSELF {
            return 1; // the owner, who also works
        }
        let mut layers = 1;
        while n > SPAN_OF_CONTROL && layers < MOST_LAYERS {
            n = (n / SPAN_OF_CONTROL).ceil();
            layers += 1;
        }
        layers + 1 // whoever owns it
    }

    /// **The rota for today**, in worker-shifts.
    ///
    /// A shop does not flex its staffing by the hour — it writes a rota.
    /// Somebody decides on Wednesday how many people are wanted on
    /// Saturday, and that is how many turn up. Thirty checkouts, eight of
    /// them open on a wet Tuesday, because eight people were put on the
    /// morning shift and not thirty.
    ///
    /// **This is where retail's precarity actually lives.** You are
    /// employed and the rota gives you three days this week — around 60%
    /// of UK retail work is part-time and variable hours are the norm, so
    /// the question a shop worker asks is not whether they have a job but
    /// whether they are on next week.
    ///
    /// About a third of the floor's hours are fixed whatever the trade —
    /// the stockroom, opening and closing, the deliveries that come
    /// whether anybody buys anything or not — and the rest is rostered
    /// against expected takings. The people in charge are on regardless;
    /// that is most of what being in charge is.
    pub fn shifts_today(&self, utilisation: f64) -> f64 {
        let u = utilisation.clamp(0.0, 1.0);
        const FIXED_SHARE: f64 = 0.33;
        let floor = self.floor_staff() * (FIXED_SHARE + (1.0 - FIXED_SHARE) * u);
        (floor + self.supervisors() + self.managers()).ceil()
    }

    /// What the rota costs in people. Same number, named for the other
    /// question it answers.
    pub fn staff_today(&self, utilisation: f64) -> f64 {
        self.shifts_today(utilisation)
    }

    /// Staff doing one particular job, so a person can look for that work.
    pub fn staff_at(&self, fixture: Fixture) -> f64 {
        self.fixtures
            .iter()
            .filter(|&&(fx, _)| fx == fixture)
            .map(|&(fx, n)| fx.staff() * n)
            .sum()
    }

    pub fn floor_m2(&self) -> f64 {
        self.sum(|f| f.floor_m2())
    }

    pub fn holds_t(&self) -> f64 {
        self.sum(|f| f.holds_t())
    }

    pub fn price_in_wage_days(&self) -> f64 {
        self.sum(|f| f.price_in_wage_days())
    }
}

// ---------------------------------------------------------------------------
// What a thing costs to build
// ---------------------------------------------------------------------------

/// **What a structure is made of**, in tonnes of traded material.
///
/// Aggregate and sand are deliberately absent although they are the bulk
/// of any structure by weight: real aggregate travels under 50 km and is
/// dug from whatever pit is nearest, so it is not a traded commodity at
/// this scale. What a country actually has to *get* is the binder, the
/// metal and the wood — which is why cement, steel and timber are the
/// three numbers here and gravel is not.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Bill {
    pub cement: f64,
    pub steel: f64,
    pub timber: f64,
}

impl Bill {
    pub fn total(&self) -> f64 {
        self.cement + self.steel + self.timber
    }

    /// Scale a bill by an area, a length or a count.
    pub fn times(&self, n: f64) -> Bill {
        Bill {
            cement: self.cement * n,
            steel: self.steel * n,
            timber: self.timber * n,
        }
    }
}

/// What a structure's bill is measured against.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Per {
    /// A square metre of floor.
    FloorArea,
    /// A metre of run — a fence or a wall is a line, not a room.
    Length,
}

/// **Everything from a fence to a buried bunker**, and the point of
/// putting them on one scale is that the range is enormous and not
/// intuitive.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Structure {
    /// Post and rail. The cheapest thing anybody builds, and still made
    /// of something.
    Fence,
    /// A timber shed on a thin slab.
    Shed,
    /// A detached house, two storeys.
    House,
    /// The same house sharing party walls, which is most of why a terrace
    /// is cheaper per dwelling and not merely denser.
    Terrace,
    /// Four to six storeys, which needs a frame rather than load-bearing
    /// walls — and that is where the steel starts.
    Tenement,
    /// Big span, steel portal frame, concrete slab.
    Warehouse,
    /// Industrial: heavier floors for machinery, cranes overhead.
    Works,
    /// **Hardened, above ground.** 0.4 m of reinforced concrete, which is
    /// the thickness that matters for blast and for gamma: concrete's
    /// halving thickness is about 6 cm, so 40 cm is roughly a hundredfold
    /// attenuation.
    Shelter,
    /// **Hardened and buried.** A metre of reinforced concrete, earth
    /// pressure on every wall, and a hole to put it in.
    Bunker,
}

impl Structure {
    pub const ALL: [Structure; 9] = [
        Structure::Fence,
        Structure::Shed,
        Structure::House,
        Structure::Terrace,
        Structure::Tenement,
        Structure::Warehouse,
        Structure::Works,
        Structure::Shelter,
        Structure::Bunker,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Structure::Fence => "fence",
            Structure::Shed => "shed",
            Structure::House => "house",
            Structure::Terrace => "terrace",
            Structure::Tenement => "tenement",
            Structure::Warehouse => "warehouse",
            Structure::Works => "works",
            Structure::Shelter => "shelter",
            Structure::Bunker => "bunker",
        }
    }

    pub fn per(self) -> Per {
        match self {
            Structure::Fence => Per::Length,
            _ => Per::FloorArea,
        }
    }

    /// **Materials per square metre of floor** (or per metre, for a
    /// fence).
    ///
    /// The house is the anchor and it is a real one: a 93 m² house takes
    /// about **20 t of cement and 3.5 t of steel**, which is 0.22 and
    /// 0.038 a square metre — and the rule of thumb builders use, 4 kg of
    /// steel per square foot, is the same number.
    ///
    /// Everything else is that house, made lighter or made hard.
    pub fn materials(self) -> Bill {
        let b = |cement, steel, timber| Bill {
            cement,
            steel,
            timber,
        };
        match self {
            // ~5 kg of timber a metre, and a staple.
            Structure::Fence => b(0.0, 0.0002, 0.005),
            Structure::Shed => b(0.05, 0.002, 0.030),
            Structure::House => b(0.22, 0.043, 0.050),
            // A party wall is one wall doing two jobs.
            Structure::Terrace => b(0.19, 0.036, 0.045),
            // Load-bearing masonry stops working at about four storeys,
            // so this is a frame — and a frame is steel.
            Structure::Tenement => b(0.30, 0.070, 0.020),
            Structure::Warehouse => b(0.18, 0.055, 0.005),
            Structure::Works => b(0.30, 0.080, 0.005),
            // **0.4 m of reinforced concrete over walls and roof** comes
            // to ~2.5 m³ of concrete per m² of floor. A cubic metre of
            // concrete holds ~320 kg of cement, and blast-grade rebar
            // runs 150 kg/m³ against an ordinary building's 80-120.
            Structure::Shelter => b(0.80, 0.375, 0.0),
            // A metre of concrete, and 200 kg/m³ of steel in it.
            Structure::Bunker => b(1.60, 1.000, 0.0),
        }
    }

    /// **Spoil to be dug out and carted away**, cubic metres per square
    /// metre of floor. Zero for anything that sits on the ground.
    pub fn excavated_m3(self) -> f64 {
        match self {
            // A footing, and nothing more.
            Structure::House | Structure::Terrace | Structure::Works => 0.3,
            Structure::Tenement => 0.6,
            // The structure itself, plus working room round it, plus the
            // cover over the top.
            Structure::Bunker => 4.0,
            _ => 0.0,
        }
    }

    pub fn buried(self) -> bool {
        self == Structure::Bunker
    }

    /// **How much dearer this gets below the water table.**
    ///
    /// `ground.rs` has known the depth to water since it was written and
    /// nothing had ever asked it. It decides this: a basement above the
    /// water table needs damp-proofing, one below it needs *tanking* —
    /// a continuous waterproof box holding back real pressure, and pumps
    /// for ever afterwards. Real practice puts that at two to three times
    /// the structural cost, and it is why **New Orleans has no basements**
    /// and nor does anywhere built on a marsh.
    ///
    /// `depth_to_water_m` comes straight from `World::depth_to_water_m`.
    pub fn wetness_multiplier(self, depth_to_water_m: f64, dug_to_m: f64) -> f64 {
        if !self.buried() || dug_to_m <= 0.0 {
            return 1.0;
        }
        if depth_to_water_m >= dug_to_m {
            // Dry ground. Damp-proofing only.
            return 1.0;
        }
        // How far below the water table the floor sits.
        let below = (dug_to_m - depth_to_water_m).max(0.0);
        // Tanking, and then a bit more for every metre of head.
        (2.0 + 0.35 * below).min(6.0)
    }
}
