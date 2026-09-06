//! **A household does not want a washing machine. It wants clean clothes.**
//!
//! That is the whole of this module, and everything awkward in it follows
//! from taking it seriously. An economy that models demand as "buys 1.0 t
//! of retail goods a year" cannot say that a working machine stops a
//! purchase, that a broken one creates a *repair* job before it creates a
//! sale, that a second-hand one does the same work as a new one, or that
//! somebody with no machine and no money washes by hand and loses the
//! evening to it.
//!
//! ```text
//! need -> service -> what it already has + its own hands
//!      -> repair / reuse / second-hand / substitute
//!      -> buy new -> and if nobody sells it, unmet demand
//! ```
//!
//! **Unmet demand is a real state**, not a purchase that quietly happens
//! anyway. A country with no cookers has households cooking on fires, and
//! that has to be visible rather than smoothed away.

use crate::id::Id;
use crate::item::{Catalogue, DefId, ItemInstance, Lifecycle, Placement, Store};

// =====================================================================
// what a household is for
// =====================================================================

/// **What a household needs done.** Not what it owns — what it requires to
/// be true of its life.
///
/// Each of these is met by a *service*, and a service can be delivered by a
/// possession, by somebody's hands, by buying it, or not at all. Which is
/// why the list contains no objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Need {
    /// Enough to eat. No substitute and no postponement.
    Nutrition,
    /// Food made edible. A hearth, a stove, a takeaway, or raw.
    CookedFood,
    /// Food that has not gone off — the whole reason a cold box is worth a
    /// month of wages.
    FoodKeeping,
    /// Rooms at a liveable temperature. Driven by climate far more than by
    /// anything about the people.
    Warmth,
    /// Being able to see after dark.
    Light,
    /// Clean water to hand.
    Water,
    /// Clothes that are clean.
    CleanClothes,
    /// Something to wear, which is a different need from washing it.
    Clothed,
    /// Washing yourself.
    Hygiene,
    /// Somewhere to sleep that is not the floor.
    Rest,
    /// Getting to work and back.
    Mobility,
    /// Being reachable, and reaching other people.
    Contact,
    /// Not being ill, and being treated when you are.
    Health,
    /// Something to do. The first thing to go and the last thing anybody
    /// admits to needing.
    Diversion,
}

pub const ALL_NEEDS: [Need; 14] = {
    use Need::*;
    [
        Nutrition, CookedFood, FoodKeeping, Warmth, Light, Water, CleanClothes, Clothed, Hygiene,
        Rest, Mobility, Contact, Health, Diversion,
    ]
};

impl Need {
    pub fn name(self) -> &'static str {
        use Need::*;
        match self {
            Nutrition => "being fed",
            CookedFood => "cooked food",
            FoodKeeping => "food that keeps",
            Warmth => "a warm room",
            Light => "light after dark",
            Water => "clean water",
            CleanClothes => "clean clothes",
            Clothed => "something to wear",
            Hygiene => "washing",
            Rest => "somewhere to sleep",
            Mobility => "getting about",
            Contact => "keeping in touch",
            Health => "not being ill",
            Diversion => "something to do",
        }
    }

    /// **Which spending category it lands in**, which is how the whole
    /// basket gets checked against a real one.
    pub fn category(self) -> Category {
        use Need::*;
        match self {
            Nutrition | CookedFood | FoodKeeping => Category::Food,
            Warmth | Light | Water => Category::Utilities,
            CleanClothes | Clothed => Category::Apparel,
            Hygiene | Rest => Category::Household,
            Mobility => Category::Transport,
            Contact | Diversion => Category::Leisure,
            Health => Category::Healthcare,
        }
    }

    /// **Going without is not equally survivable.** Food and warmth kill; a
    /// dull evening does not, which is exactly why the poorest household
    /// spends its money in the order it does.
    ///
    /// This is what gets ranked when there is not enough for everything, so
    /// **Engel's law falls out of the ordering** rather than being asserted
    /// as a share.
    pub fn urgency(self) -> f64 {
        use Need::*;
        match self {
            Nutrition => 1.00,
            Water => 0.98,
            Warmth => 0.92,
            Rest => 0.88,
            Health => 0.85,
            CookedFood => 0.80,
            Hygiene => 0.70,
            Clothed => 0.68,
            Light => 0.62,
            Mobility => 0.60,
            FoodKeeping => 0.45,
            CleanClothes => 0.42,
            Contact => 0.35,
            Diversion => 0.20,
        }
    }

    /// **Some needs are a fact about the house and some about the people.**
    ///
    /// Heating a room for four costs barely more than heating it for one;
    /// feeding four costs four times feeding one. That asymmetry is most of
    /// why sharing a roof is cheaper per head, and it is the same
    /// equivalence the household model already applies to rent.
    pub fn per_head(self) -> f64 {
        use Need::*;
        match self {
            // Wholly per person.
            Nutrition | Clothed | CleanClothes | Hygiene | Health => 1.0,
            // Mostly per person, partly shared.
            Rest | Mobility | Contact | Diversion => 0.85,
            // Shared, with a little more for each extra person.
            CookedFood | Water | Light => 0.35,
            FoodKeeping => 0.30,
            // A house is a house.
            Warmth => 0.12,
        }
    }
}

/// **The things a real household budget divides into**, so the model can be
/// held against a published one instead of against taste.
///
/// Shares are the US Consumer Expenditure Survey, 2023: housing 32.9% of
/// average annual expenditures, transportation 17.0, food 12.9, healthcare
/// 8.0, entertainment 4.7, apparel 2.6. Shelter alone is 20.9 of that
/// housing figure and is modelled by `person::Housing`, so what this module
/// covers is everything in the house except the rent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    Food,
    /// Fuel, power, water — 6.6% of expenditure and almost none of it
    /// optional.
    Utilities,
    /// Furnishings, equipment and supplies: 2.7 + 1.1 + 1.6.
    Household,
    Apparel,
    Transport,
    Healthcare,
    Leisure,
    /// Rent or mortgage. Named here for completeness and paid elsewhere.
    Shelter,
}

pub const ALL_CATEGORIES: [Category; 8] = {
    use Category::*;
    [Food, Utilities, Household, Apparel, Transport, Healthcare, Leisure, Shelter]
};

impl Category {
    pub fn name(self) -> &'static str {
        match self {
            Category::Food => "food",
            Category::Utilities => "utilities",
            Category::Household => "household goods",
            Category::Apparel => "clothing",
            Category::Transport => "transport",
            Category::Healthcare => "healthcare",
            Category::Leisure => "recreation",
            Category::Shelter => "shelter",
        }
    }

    /// **What a real American household spends on it**, as a share of all
    /// expenditure *(BLS Consumer Expenditure Survey 2023: average annual
    /// expenditures about $77,300 across a household of 2.5)*. A
    /// calibration target, never an input.
    pub fn real_share(self) -> f64 {
        match self {
            Category::Shelter => 0.209,
            Category::Transport => 0.170,
            Category::Food => 0.129,
            Category::Healthcare => 0.080,
            Category::Utilities => 0.066,
            Category::Household => 0.054,
            Category::Leisure => 0.049,
            Category::Apparel => 0.026,
        }
    }
}

// =====================================================================
// who is in it
// =====================================================================

/// **Who the household has to provide for.**
///
/// Ages matter, and not only in the obvious direction: an infant costs far
/// more in care than in food, and a child eats about two thirds of what an
/// adult does. Real: an adult wants 2,000-2,500 kcal a day, a child of ten
/// about 1,800, a toddler about 1,200.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Roster {
    pub adults: u32,
    /// Under five. The expensive ones, and not because of the food.
    pub infants: u32,
    /// Five to fifteen.
    pub children: u32,
    /// Over sixty-five. At home more of the day, so warm for longer.
    pub elderly: u32,
}

impl Roster {
    pub fn of(adults: u32) -> Self {
        Roster { adults, ..Default::default() }
    }

    pub fn with(mut self, infants: u32, children: u32) -> Self {
        self.infants = infants;
        self.children = children;
        self
    }

    pub fn heads(self) -> u32 {
        self.adults + self.infants + self.children + self.elderly
    }

    /// **Consuming heads**, weighted by age. Not the equivalence scale —
    /// that is about sharing a roof; this is about how much of a thing each
    /// person actually gets through.
    pub fn eaters(self) -> f64 {
        self.adults as f64
            + self.elderly as f64 * 0.85
            + self.children as f64 * 0.70
            + self.infants as f64 * 0.35
    }

    /// From the household types the population model already draws.
    pub fn from_household(h: crate::populace::Household) -> Self {
        use crate::populace::Household::*;
        match h {
            Alone => Roster::of(1),
            Couple => Roster::of(2),
            Shared(n) | Family(n) => Roster::of(n.max(1) as u32),
        }
    }
}

// =====================================================================
// where it is
// =====================================================================

/// **What the place demands of them**, which is a fact about the ground and
/// not about the people.
///
/// Heating is the big one: real US heating degree-days run from about 150
/// in Miami to 7,800 in Minneapolis against a national average near 4,000,
/// and cooling runs the other way — Miami 4,400, Minneapolis 700. A
/// household in a hard winter is not the same household.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Climate {
    /// Degree-days below 18.3 °C over a year.
    pub heating_degree_days: f64,
    /// Degree-days above it.
    pub cooling_degree_days: f64,
}

impl Climate {
    /// The national average, for a case that is not about climate.
    pub fn temperate() -> Self {
        Climate { heating_degree_days: 4_000.0, cooling_degree_days: 1_300.0 }
    }

    pub fn cold() -> Self {
        Climate { heating_degree_days: 7_800.0, cooling_degree_days: 700.0 }
    }

    pub fn hot() -> Self {
        Climate { heating_degree_days: 150.0, cooling_degree_days: 4_400.0 }
    }

    /// **Cooling is not free either**, and in a hot country it is the
    /// larger bill. Both land on one need, because what a household wants
    /// is a room it can live in.
    pub fn comfort_factor(self) -> f64 {
        (self.heating_degree_days / 4_000.0 + self.cooling_degree_days / 4_000.0 * 0.55)
            .clamp(0.15, 3.0)
    }

    /// **A cold country needs more clothes and heavier ones.** Real apparel
    /// spending varies less than the climate does, because a coat lasts
    /// years, so the effect is real and modest.
    pub fn clothing_factor(self) -> f64 {
        (0.85 + self.heating_degree_days / 4_000.0 * 0.25).clamp(0.85, 1.6)
    }
}

// =====================================================================
// how much of each service
// =====================================================================

/// **How much of a service this household wants met**, on an index where
/// 1.0 is one ordinary adult in an ordinary climate for a year.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Requirement {
    pub need: Need,
    pub level: f64,
}

/// What a household of this make-up, in this place, requires.
pub fn requirements(who: Roster, where_: Climate) -> Vec<Requirement> {
    let heads = who.heads().max(1) as f64;
    ALL_NEEDS
        .iter()
        .map(|&need| {
            // One household, plus so much for each extra head — which is
            // what makes warmth nearly flat and food nearly linear.
            let scale = match need {
                Need::Nutrition => who.eaters(),
                _ => 1.0 + (heads - 1.0) * need.per_head(),
            };
            let climate = match need {
                Need::Warmth => where_.comfort_factor(),
                Need::Clothed => where_.clothing_factor(),
                // **Light is a winter need.** A far northern winter is dark
                // for most of the working day.
                Need::Light => 0.8 + where_.heating_degree_days / 4_000.0 * 0.3,
                // Keeping food is harder where it is hot, which is exactly
                // why refrigeration mattered most in hot countries.
                Need::FoodKeeping => 0.8 + where_.cooling_degree_days / 4_000.0 * 0.4,
                _ => 1.0,
            };
            Requirement { need, level: scale * climate }
        })
        .collect()
}

/// The level of one particular need, for a caller that wants one.
pub fn requirement_for(need: Need, who: Roster, where_: Climate) -> f64 {
    requirements(who, where_)
        .into_iter()
        .find(|r| r.need == need)
        .map(|r| r.level)
        .unwrap_or(0.0)
}

// =====================================================================
// what delivers a service
// =====================================================================

/// **How a need is being met right now.**
///
/// The point of naming the route rather than a good is that the same need
/// can be met four ways with wholly different consequences: a machine costs
/// money once and electricity thereafter, hands cost an evening, buying the
/// service costs money every time, and nothing costs whatever going without
/// costs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Route {
    /// Something they own is doing it.
    Owned(Id<ItemInstance>),
    /// Somebody in the house is doing it by hand. Costs hours, not money.
    ByHand { hours_a_week: f64 },
    /// Bought as a service, over and over — a laundry, a canteen, a bus
    /// fare. The expensive way, and the way people without capital pay.
    Bought,
    /// A shared or communal facility: a standpipe, a launderette in the
    /// building, a neighbour who does not mind.
    Shared,
    /// Not met.
    Unmet,
}

impl Route {
    pub fn met(self) -> bool {
        !matches!(self, Route::Unmet)
    }

    pub fn name(self) -> &'static str {
        match self {
            Route::Owned(_) => "has one",
            Route::ByHand { .. } => "does it by hand",
            Route::Bought => "pays for it",
            Route::Shared => "uses a shared one",
            Route::Unmet => "goes without",
        }
    }
}

/// **What an object does for a household**, which is a fact about the
/// household and not about the object — a bucket serves three needs in a
/// place with no tap and none at all in a place with one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Serves {
    pub need: Need,
    /// How much of the requirement one of them covers.
    pub covers: f64,
    /// **What fraction of the hand-work is still left.**
    ///
    /// Not hours saved. A machine does not subtract a fixed number of hours
    /// from the job — it leaves a residue of it, and the residue scales
    /// with the household. You still load a washing machine, hang the
    /// washing out and put it away; you still cook on a stove. Real: a
    /// machine takes laundry from most of a day to something under an hour,
    /// which is a fraction and not a subtraction.
    pub leaves: f64,
    /// **What it costs to run, in kWh a week at full output.** A durable is
    /// not free once bought, and this is where the utility bill comes from
    /// — the whole reason a cold country is expensive to live in even when
    /// everybody already owns a heater.
    pub kwh_a_week: f64,
    /// **Whether it draws whether or not you are using it.**
    ///
    /// A refrigerator runs day and night and a second one really does
    /// double the bill. A heater runs as hard as the weather makes it, so
    /// two radiators in a mild house each run at part load — and counting
    /// them the way a fridge is counted put a man with three heaters on a
    /// bill larger than his food.
    pub standing: bool,
    /// **Whether owning it means the job is done, or only that it can be
    /// done.**
    ///
    /// The distinction the first version of this module got wrong. A wash
    /// tub genuinely washes clothes and a household that owns one does not
    /// have clean clothes — it has the means to spend Monday getting them.
    /// A machine does the job; a tub is the job made possible. Collapsing
    /// the two makes a tub as good as a machine at a twentieth of the
    /// price, and then nobody ever buys a machine.
    pub hands_on: bool,
}

/// What each catalogued thing does for a household.
///
/// **Kept here rather than on the item definition** because it is not a
/// property of the object: a stove serves `CookedFood` only in a world
/// where cooking is a need somebody has. The item layer knows what a thing
/// *is*; this layer knows what it is *for*.
pub fn what_it_does(name: &str) -> &'static [Serves] {
    use Need::*;
    // Draws as hard as the demand makes it.
    macro_rules! does {
        ($($n:expr, $c:expr, $l:expr, $k:expr);* $(;)?) => {
            &[$(Serves { need: $n, covers: $c, leaves: $l, kwh_a_week: $k,
                         standing: false, hands_on: false }),*]
        };
    }
    // Draws whether or not anybody is using it.
    macro_rules! always_on {
        ($($n:expr, $c:expr, $l:expr, $k:expr);* $(;)?) => {
            &[$(Serves { need: $n, covers: $c, leaves: $l, kwh_a_week: $k,
                         standing: true, hands_on: false }),*]
        };
    }
    macro_rules! helps {
        ($($n:expr, $c:expr, $l:expr);* $(;)?) => {
            &[$(Serves { need: $n, covers: $c, leaves: $l, kwh_a_week: 0.0,
                         standing: false, hands_on: true }),*]
        };
    }
    // Real annual consumption, turned into a week: a refrigerator runs
    // 400-500 kWh a year, an electric range 500-700, a washing machine
    // about 150, and **home heating dwarfs all of it** at something like
    // 11,700 kWh a year of delivered heat in an average American house.
    match name {
        // Things that do the job.
        "washing machine" => does![CleanClothes, 4.0, 0.15, 3.0],
        "cooking stove" => does![CookedFood, 4.0, 0.35, 26.0],
        "refrigerator" => always_on![FoodKeeping, 4.0, 0.10, 9.0],
        "space heater" => does![Warmth, 1.0, 0.05, 110.0],
        "electric light" => always_on![Light, 3.0, 0.05, 5.0],
        "bed" => does![Rest, 1.0, 0.00, 0.0],
        "bicycle" => does![Mobility, 1.0, 0.60, 0.0],
        // **The running figure for a car is not only its fuel.** Insurance,
        // tyres, servicing and repairs are together larger than the petrol:
        // real US expenditure is 3.5% on gasoline and 5.7% on other vehicle
        // expenses. Expressed in the same units as everything else so one
        // price covers the household, which is a simplification and is
        // recorded as one.
        "motor car" => does![Mobility, 4.0, 0.10, 300.0],
        "telephone" => always_on![Contact, 4.0, 0.10, 0.4],
        "radio set" => always_on![Diversion, 3.0, 0.20, 1.5],
        "coat" => does![Clothed, 1.0, 0.00, 0.0],
        "work clothes" => does![Clothed, 1.0, 0.00, 0.0],
        // Things that make the job possible, and it is still your Monday.
        "cooking pot" => helps![CookedFood, 1.0, 0.85],
        "wash tub" => helps![CleanClothes, 1.0, 0.70],
        "water butt" => helps![Water, 2.0, 0.55],
        _ => &[],
    }
}

/// Whether a thing is worth keeping rather than consuming, which the item
/// layer already answers.
pub fn is_durable(cat: &Catalogue, def: DefId) -> bool {
    cat.get(def)
        .map(|d| matches!(d.family.lifecycle(), Lifecycle::Durable | Lifecycle::SemiDurable))
        .unwrap_or(false)
}

/// Whether it is a flow rather than a stock — food, fuel, soap.
pub fn is_flow(cat: &Catalogue, def: DefId) -> bool {
    cat.get(def)
        .map(|d| matches!(d.family.lifecycle(), Lifecycle::Perishable | Lifecycle::Consumable))
        .unwrap_or(false)
}

/// Everything in the catalogue that would serve a given need.
pub fn things_that_serve(cat: &Catalogue, need: Need) -> Vec<(DefId, Serves)> {
    let mut out: Vec<(DefId, Serves)> = Vec::new();
    for d in cat.iter() {
        for &s in what_it_does(d.name) {
            if s.need == need {
                out.push((d.id, s));
            }
        }
    }
    // Cheapest-covering first, and a stable tiebreak, because a seed has to
    // rebuild the same world.
    out.sort_by(|a, b| {
        b.1.covers.total_cmp(&a.1.covers).then(a.0 .0.cmp(&b.0 .0))
    });
    out
}

/// Whether a household with these possessions has anything that would do
/// this job, and how well it is working.
pub fn provision_for(
    need: Need,
    level: f64,
    owned: &[Id<ItemInstance>],
    store: &Store,
    cat: &Catalogue,
) -> Route {
    let mut covered = 0.0;
    let mut best: Option<(f64, Id<ItemInstance>)> = None;
    let mut leaves = 1.0f64;
    let mut hands_only = true;
    for &id in owned {
        let Some(item) = store.get(id) else { continue };
        let Some(def) = cat.get(item.definition) else { continue };
        for &s in what_it_does(def.name) {
            if s.need != need {
                continue;
            }
            // **A thing that does not work does not serve.** Which is the
            // whole reason condition had to exist before this module could.
            // Half damaged is out of service: an appliance with a real
            // fault does not half-wash the clothes.
            if !in_service(item) {
                continue;
            }
            // **Coverage adds up, and it has to reach the requirement.**
            // One heater in a Minnesota winter is not a warm house, and
            // treating any coverage at all as enough is what made Miami and
            // Minneapolis come out identical.
            // **A machine has a throughput; a tub has only your time.**
            // One tub washes a family's clothes — it takes longer, and the
            // hours below carry that. So an aid always covers the
            // requirement and an appliance has to be big enough for it.
            let score = if s.hands_on {
                level.max(s.covers)
            } else {
                s.covers * (1.0 - item.condition.damage)
            };
            covered += score;
            leaves = leaves.min(s.leaves);
            if !s.hands_on {
                hands_only = false;
            }
            if best.map(|(b, _)| score > b).unwrap_or(true) {
                best = Some((score, id));
            }
        }
    }
    if covered + 1e-9 < level {
        return Route::Unmet;
    }
    match best {
        // **Owning the tub is not having clean clothes.** The hours are
        // still somebody's Monday, and they are charged as such.
        Some(_) if hands_only => {
            Route::ByHand { hours_a_week: hours_to_do_it_by_hand(need, level) * leaves }
        }
        Some((_, id)) => Route::Owned(id),
        None => Route::Unmet,
    }
}

/// Whether anything the household owns for this need needs power to work.
pub fn runs_on_power(
    need: Need,
    owned: &[Id<ItemInstance>],
    store: &Store,
    cat: &Catalogue,
) -> bool {
    owned.iter().any(|&id| {
        store
            .get(id)
            .and_then(|i| cat.get(i.definition))
            .map(|d| {
                what_it_does(d.name)
                    .iter()
                    .any(|s| s.need == need && s.kwh_a_week > 0.0 && !s.hands_on)
            })
            .unwrap_or(false)
    })
}

/// **What running everything the household owns costs**, in kWh a week.
/// The utility bill, and the reason a warm house in a cold place is
/// expensive to keep whether or not the heater was cheap.
pub fn running_kwh(
    owned: &[Id<ItemInstance>],
    store: &Store,
    cat: &Catalogue,
    wanted: &[Requirement],
) -> f64 {
    // Gather what is installed against each need, because **the demand is a
    // property of the household and the capacity is a property of the
    // things**, and the bill is where the two meet. Adding up nameplates
    // one appliance at a time cannot express that.
    let mut per_need: Vec<(Need, f64, f64, f64)> = Vec::new(); // need, capacity, following, standing
    for &id in owned {
        let Some(item) = store.get(id) else { continue };
        if !in_service(item) {
            continue;
        }
        let Some(def) = cat.get(item.definition) else { continue };
        for s in what_it_does(def.name) {
            if s.kwh_a_week <= 0.0 && s.covers <= 0.0 {
                continue;
            }
            let slot = match per_need.iter_mut().find(|x| x.0 == s.need) {
                Some(x) => x,
                None => {
                    per_need.push((s.need, 0.0, 0.0, 0.0));
                    per_need.last_mut().unwrap()
                }
            };
            slot.1 += s.covers;
            if s.standing {
                slot.3 += s.kwh_a_week;
            } else {
                slot.2 += s.kwh_a_week;
            }
        }
    }
    per_need
        .iter()
        .map(|&(need, capacity, following, standing)| {
            let level = wanted.iter().find(|r| r.need == need).map(|r| r.level).unwrap_or(1.0);
            // **A demand-following load runs to the demand**, shared over
            // whatever is installed: two radiators in a mild house each run
            // at part load, and a third one changes nothing but the
            // purchase price.
            let served = if capacity > 0.0 { (level / capacity).clamp(0.0, 1.0) } else { 0.0 };
            following * served + standing
        })
        .sum()
}

/// **Whether a thing is in service.** A real fault takes an appliance out,
/// and half-working is not a thing a washing machine does.
pub fn in_service(item: &ItemInstance) -> bool {
    item.condition.damage < 0.5
}

// =====================================================================
// what it would do about a gap
// =====================================================================

/// **The ways a household can close a gap**, in the order it considers
/// them — which is the order real people consider them, and not the order
/// an economic model usually assumes.
///
/// A demand model that jumps straight to *buy a new one* cannot produce a
/// repair trade, a second-hand market, or the enormous amount of unpaid
/// work that poor households do instead of buying things.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Intent {
    /// It is already met. **Nothing happens**, which is the commonest and
    /// most important outcome.
    Nothing,
    /// Something they own is broken and worth mending. Comes *before*
    /// replacement, always.
    Repair(Id<ItemInstance>),
    /// Buy one somebody else has finished with. Same service, a fraction
    /// of the price, and a real market.
    BuyUsed(DefId),
    /// Buy a new one.
    BuyNew(DefId),
    /// **Make one.** Which goes through the same work orders a factory
    /// uses — the same recipe, the same three kinds of time, the same
    /// failure model. What differs is the bench.
    MakeIt(DefId),
    /// **Take one nobody owns.** A tip, a wreck, a house somebody left.
    /// Free, worn out, and what a household with no money and no market
    /// actually does.
    Scavenge(DefId),
    /// Do it by hand instead. Free of money and expensive in hours.
    ByHand { hours_a_week: f64 },
    /// Buy the service each time rather than the machine once. What people
    /// without capital do, and why it costs them more in the end.
    PayForTheService,
    /// **Go without.** Not a failure of the model: a real answer, and the
    /// one that makes poverty legible.
    GoWithout,
}

impl Intent {
    pub fn name(self) -> &'static str {
        match self {
            Intent::Nothing => "already met",
            Intent::Repair(_) => "mend it",
            Intent::BuyUsed(_) => "buy one second-hand",
            Intent::BuyNew(_) => "buy a new one",
            Intent::MakeIt(_) => "make one",
            Intent::Scavenge(_) => "find one",
            Intent::ByHand { .. } => "do it by hand",
            Intent::PayForTheService => "pay somebody each time",
            Intent::GoWithout => "go without",
        }
    }

    /// Whether it puts an order into a market.
    pub fn is_a_purchase(self) -> bool {
        matches!(self, Intent::BuyUsed(_) | Intent::BuyNew(_))
    }
}

/// **What a household can pay with.**
///
/// Three separate things, because they run out in a particular order and
/// the order is what makes a shock bite: today's money, then what is put
/// by, then what somebody will lend — and a household with no savings and
/// no credit is one bad week from going without.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Means {
    /// Coming in this period.
    pub income: f64,
    /// Put by. Real: about a third of US households could not cover a $400
    /// emergency from savings, which is exactly the case this represents.
    pub savings: f64,
    /// What could be borrowed. Nought for most poor households, which is
    /// most of what being poor consists of.
    pub credit: f64,
}

impl Means {
    pub fn total(self) -> f64 {
        self.income + self.savings + self.credit
    }

    /// **What it can actually put down today.** Credit is a last resort and
    /// savings are not spent lightly, so a large one-off purchase is much
    /// harder than the total suggests.
    pub fn for_a_lump_sum(self) -> f64 {
        self.income * 0.35 + self.savings * 0.60 + self.credit * 0.50
    }
}

/// **Somebody, selling something, at a price, and only so many of them.**
///
/// A price with no stock behind it is how a model comes to fill every order
/// it is given. And the seller is named, because a household order that
/// cannot be filled by *the player's shop* is not a household order the
/// player can build a business on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stall {
    pub def: DefId,
    pub price: f64,
    /// How many are on the shelf. Nought is a real answer.
    pub stock: u32,
    /// Whose it is. Anybody at all, the player included.
    pub seller: u64,
    /// Second-hand stock is a different trade in the same town.
    pub used: bool,
}

impl Stall {
    pub fn new(def: DefId, price: f64, stock: u32, seller: u64) -> Self {
        Stall { def, price, stock, seller, used: false }
    }

    pub fn second_hand(def: DefId, price: f64, stock: u32, seller: u64) -> Self {
        Stall { def, price, stock, seller, used: true }
    }
}

/// What the market will supply, and at what price. **Asked rather than
/// assumed**, because "nobody sells one" has to be a possible answer.
#[derive(Clone, Debug, Default)]
pub struct Market {
    pub stalls: Vec<Stall>,
    /// Whether there is anybody who will mend things, and what an hour of
    /// them costs. Real: a repair trade is the first thing to disappear
    /// from a place and the last thing to come back.
    pub repairer: Option<f64>,
    /// Services somebody sells by the go — a launderette, a canteen, a bus.
    pub services: Vec<(Need, f64)>,
    /// **Things lying about that nobody owns.** A tip, a wreck, an empty
    /// house. Free, and worn out.
    pub abandoned: Vec<DefId>,
    /// What running everything costs, per kWh. `None` means there is no
    /// supply at all — which is most of the world for most of history, and
    /// still a fifth of it.
    pub power: Option<f64>,
}

impl Market {
    fn cheapest(&self, def: DefId, used: bool) -> Option<&Stall> {
        self.stalls
            .iter()
            .filter(|s| s.def == def && s.used == used && s.stock > 0)
            .min_by(|a, b| a.price.total_cmp(&b.price).then(a.seller.cmp(&b.seller)))
    }

    pub fn price_new(&self, def: DefId) -> Option<f64> {
        self.cheapest(def, false).map(|s| s.price)
    }

    pub fn price_used(&self, def: DefId) -> Option<f64> {
        self.cheapest(def, true).map(|s| s.price)
    }

    pub fn price_service(&self, need: Need) -> Option<f64> {
        self.services.iter().find(|(n, _)| *n == need).map(|(_, p)| *p)
    }

    /// **Take one off the shelf.** Returns who was paid and what, or
    /// nothing at all — which is what an empty shelf means and is not the
    /// same as no shop.
    pub fn take(&mut self, def: DefId, used: bool) -> Option<(u64, f64)> {
        let who = self.cheapest(def, used).map(|s| (s.seller, s.price))?;
        let stall = self
            .stalls
            .iter_mut()
            .find(|s| s.def == def && s.used == used && s.seller == who.0 && s.stock > 0)?;
        stall.stock -= 1;
        Some(who)
    }

    /// Add stock, which is how a shop restocks and how a household puts
    /// something into the second-hand trade.
    pub fn stock(&mut self, stall: Stall) {
        match self
            .stalls
            .iter_mut()
            .find(|s| s.def == stall.def && s.used == stall.used && s.seller == stall.seller)
        {
            Some(s) => s.stock += stall.stock,
            None => self.stalls.push(stall),
        }
    }

    /// Whether anybody sells this as a flow at all.
    pub fn sells_flow(&self, need: Need) -> bool {
        self.services.iter().any(|(n, _)| *n == need)
    }

    /// **The price of power.**
    ///
    /// In the model's own currency, pinned like every other price to the
    /// food chain — the ratio is what is meant to be read, not the number.
    /// It is set so that an ordinary household's utility bill comes to
    /// about half its food bill, which is the real relationship: 6.6% of US
    /// household expenditure goes on utilities, fuels and public services
    /// against 12.9% on food.
    pub fn power_price(&self) -> f64 {
        self.power.unwrap_or(0.0)
    }
}

/// **What it costs to put right**, against what a new one costs.
///
/// Real repair economics, and the reason so much gets thrown away: a
/// repair worth doing costs well under half a replacement, labour is most
/// of the bill, and a machine near the end of its life is not worth
/// mending however cheap the part is.
pub fn worth_repairing(
    item: &ItemInstance,
    cat: &Catalogue,
    market: &Market,
    replacement: Option<f64>,
) -> Option<f64> {
    let repairer_rate = market.repairer?;
    let def = cat.get(item.definition)?;
    // Parts and labour. A worse-damaged thing takes longer.
    let hours = 1.0 + item.condition.damage * 4.0;
    let parts = def.nominal_mass_kg * 0.9 * item.condition.damage;
    let cost = hours * repairer_rate + parts;
    let new_price = replacement.unwrap_or(f64::INFINITY);
    // **Wear is not damage**, and a worn-out machine is not worth mending
    // even when the fault is small. Real repair shops turn work away for
    // exactly this reason.
    if item.condition.wear > 0.85 {
        return None;
    }
    if cost < new_price * 0.5 {
        Some(cost)
    } else {
        None
    }
}

/// **The decision, in the order a household actually makes it.**
///
/// Every branch here is a real behaviour, and the ordering is the content:
/// check what you have, mend it if you can, look for a used one, consider
/// doing it yourself, then buy new, and if none of that is open to you, go
/// without.
pub fn what_to_do(
    need: Need,
    level: f64,
    home: &Household,
    store: &Store,
    cat: &Catalogue,
    market: &Market,
    means: Means,
) -> Intent {
    let owned = &home.owns[..];
    // 1. **Something that works stops everything else.** A household with a
    //    sound washing machine does not enter the market for one, and this
    //    is by far the commonest outcome.
    // **What it has must actually cover what it wants.** A household with
    // one heater in a hard winter is still short of a warm house.
    match provision_for(need, level, owned, store, cat) {
        Route::Owned(_) => return Intent::Nothing,
        // Doing it by hand with the kit is a real provision, and it is
        // still worth upgrading out of if anybody sells the machine.
        Route::ByHand { .. } if !anything_better(need, level, cat, market, means) => {
            return Intent::Nothing
        }
        _ => {}
    }

    // 2. **A broken one is a repair job before it is a sale.** Look for
    //    something they already own that used to do this.
    let candidates: Vec<DefId> = things_that_serve(cat, need).into_iter().map(|(d, _)| d).collect();
    for &id in owned {
        let Some(item) = store.get(id) else { continue };
        let Some(def) = cat.get(item.definition) else { continue };
        if !what_it_does(def.name).iter().any(|s| s.need == need) {
            continue;
        }
        // **What it is weighed against is a replacement for *this*, not the
        // cheapest thing in the shop that touches the same need.** Nobody
        // scraps a washing machine because a tub is thirty pounds.
        let replacement = market.price_new(item.definition);
        if let Some(cost) = worth_repairing(item, cat, market, replacement) {
            if cost <= means.for_a_lump_sum() {
                return Intent::Repair(id);
            }
        }
    }

    // 3. **Second-hand does the same job.** Checked before new, because
    //    that is the order somebody short of money looks in.
    let purse = means.for_a_lump_sum();
    let best_used = best_offer(need, level, &candidates, cat, purse, |d| market.price_used(d));

    // 4. New, if anybody is selling and they can find the money.
    let best_new = best_offer(need, level, &candidates, cat, purse, |d| market.price_new(d));

    // **A used one at half the price wins**, which is what a second-hand
    // market is. A new one wins when the gap is small, because people
    // prefer new at the same money.
    match (best_used, best_new) {
        (Some((ud, up)), Some((nd, np))) => {
            if up < np * 0.75 {
                return Intent::BuyUsed(ud);
            }
            return Intent::BuyNew(nd);
        }
        (Some((ud, _)), None) => return Intent::BuyUsed(ud),
        (None, Some((nd, _))) => return Intent::BuyNew(nd),
        (None, None) => {}
    }

    // 5. **Make one.** Only if somebody here knows how and has the tools,
    //    which is a fact about the household and not about the market.
    if let Some(&d) = candidates.iter().find(|d| home.can_make.contains(d)) {
        return Intent::MakeIt(d);
    }

    // 6. **Find one.** A tip, a wreck, an empty house. Free and worn out,
    //    which is why it comes after buying rather than before: a
    //    household with money buys, and one without takes what it can get.
    if let Some(&d) = candidates.iter().find(|d| market.abandoned.contains(d)) {
        return Intent::Scavenge(d);
    }

    // 7. **Their own hands.** Free of money, and it costs the evening.
    //    **With the right tub it is quicker**, which is exactly why the
    //    cheap thing is worth owning even though it does not do the job.
    let by_hand = hours_to_do_it_by_hand(need, level);
    if by_hand > 0.0 {
        if !has_the_kit(need, owned, store, cat) {
            // Buy the tub if anybody sells one and they can afford it,
            // because doing it without is far worse.
            for &d in &candidates {
                let Some(name) = cat.get(d).map(|x| x.name) else { continue };
                if !what_it_does(name).iter().any(|s| s.need == need && s.hands_on) {
                    continue;
                }
                if let Some(p) = market.price_used(d) {
                    if p <= purse {
                        return Intent::BuyUsed(d);
                    }
                }
                if let Some(p) = market.price_new(d) {
                    if p <= purse {
                        return Intent::BuyNew(d);
                    }
                }
            }
            // Without the kit it takes half as long again — a bucket and a
            // stream against a tub and a mangle.
            return Intent::ByHand { hours_a_week: by_hand * 1.5 };
        }
        return Intent::ByHand { hours_a_week: by_hand };
    }

    // 8. Pay for it each time, if anybody sells it that way and this
    //    week's income runs to it.
    if let Some(p) = market.price_service(need) {
        if p * level <= means.income {
            return Intent::PayForTheService;
        }
    }

    // 9. **And otherwise they go without**, which is a real answer and not
    //    a hole in the model.
    Intent::GoWithout
}

/// **The best thing on offer, which is not simply the cheapest.**
///
/// A wash tub is a twentieth of the price of a machine and washes clothes,
/// so ranking on price alone means nobody in the world ever buys an
/// appliance. What a household wants is the job *done*: something that
/// covers the requirement wins, and only among those does price decide.
/// Failing that, it takes the best partial answer it can afford.
fn best_offer(
    need: Need,
    level: f64,
    candidates: &[DefId],
    cat: &Catalogue,
    purse: f64,
    price: impl Fn(DefId) -> Option<f64>,
) -> Option<(DefId, f64)> {
    let mut covering: Option<(DefId, f64)> = None;
    let mut partial: Option<(DefId, f64, f64)> = None;
    for &d in candidates {
        let Some(p) = price(d) else { continue };
        if p > purse {
            continue;
        }
        let Some(name) = cat.get(d).map(|x| x.name) else { continue };
        let Some(s) = what_it_does(name).iter().find(|s| s.need == need) else { continue };
        // A thing that only makes the job possible is not an answer to
        // wanting the job done — it is the hand route with equipment.
        if s.hands_on {
            continue;
        }
        if s.covers >= level {
            if covering.map(|(_, bp)| p < bp).unwrap_or(true) {
                covering = Some((d, p));
            }
        } else if partial.map(|(_, _, bc)| s.covers > bc).unwrap_or(true) {
            partial = Some((d, p, s.covers));
        }
    }
    covering.or(partial.map(|(d, p, _)| (d, p)))
}

/// **How long it takes without a machine.**
///
/// The figures are the reason appliances sold. Real pre-machine domestic
/// work: laundry by hand was most of a day a week, cooking from scratch on
/// a solid-fuel range hours a day, and fetching water in a place with no
/// tap is measured in kilometres — the UN puts it at 200 million hours a
/// day worldwide, overwhelmingly done by women and girls.
///
/// A need with no hand alternative returns nought: you cannot hand-make
/// nutrition out of nothing, and nobody can produce their own healthcare.
pub fn hours_to_do_it_by_hand(need: Need, level: f64) -> f64 {
    use Need::*;
    let base = match need {
        CleanClothes => 6.0,
        CookedFood => 9.0,
        Water => 7.0,
        Warmth => 3.5,
        Hygiene => 1.5,
        FoodKeeping => 2.5,
        Light => 0.8,
        Diversion => 2.0,
        Mobility => 4.0,
        // No hand route: you cannot make food out of nothing, treat
        // yourself, weave a coat overnight or build a bed from air.
        Nutrition | Clothed | Rest | Contact | Health => 0.0,
    };
    // **Domestic work does not scale with heads.** You wash a bigger load,
    // not four separate loads, and you cook one dinner for four. Real time
    // use bears this out: household work per person falls sharply with
    // household size. Taking it as linear gave a family of four
    // twenty-eight hours of laundry a week.
    base * level.max(0.0).powf(0.55)
}

// =====================================================================
// what the household asks the market for
// =====================================================================

/// Whether there is anything on the market that would actually do the job,
/// as against merely making it possible. What lets a household with a tub
/// go on wanting a machine.
fn anything_better(
    need: Need,
    level: f64,
    cat: &Catalogue,
    market: &Market,
    means: Means,
) -> bool {
    let candidates: Vec<DefId> = things_that_serve(cat, need).into_iter().map(|(d, _)| d).collect();
    let purse = means.for_a_lump_sum();
    best_offer(need, level, &candidates, cat, purse, |d| market.price_used(d)).is_some()
        || best_offer(need, level, &candidates, cat, purse, |d| market.price_new(d)).is_some()
}

/// Whether the household owns the equipment the hand route wants.
fn has_the_kit(need: Need, owned: &[Id<ItemInstance>], store: &Store, cat: &Catalogue) -> bool {
    owned.iter().any(|&id| {
        store
            .get(id)
            .and_then(|i| cat.get(i.definition))
            .map(|d| what_it_does(d.name).iter().any(|s| s.need == need && s.hands_on))
            .unwrap_or(false)
    })
}

/// **An order placed on the world, which may or may not be filled.**
///
/// The distinction that makes this worth having: `wanted` is what the
/// household asked for and `filled` is what it got. A model that only
/// records purchases can never say a country ran out of cookers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Order {
    pub need: Need,
    pub intent: Intent,
    /// What it would cost if somebody supplies it.
    pub price: f64,
    /// How much of the requirement it would cover.
    pub covers: f64,
}

/// What came of a period of ordering.
#[derive(Clone, Debug, Default)]
pub struct Outcome {
    /// Orders somebody filled, with what was paid.
    pub bought: Vec<(Order, f64)>,
    /// **Wanted, affordable, and nobody was selling.** The number that
    /// makes a shortage visible.
    pub unmet: Vec<Order>,
    /// Needs met by somebody in the house doing the work, and the hours it
    /// took out of their week.
    pub by_hand: Vec<(Need, f64)>,
    /// Repairs commissioned. **A demand on the repair trade**, which is a
    /// different trade from retail and mostly does not exist in a model
    /// that only sells new things.
    pub repairs: Vec<(Id<ItemInstance>, f64)>,
    /// Simply not met at all.
    pub without: Vec<Need>,
    /// **Who took the money**, so a shop — the player's included — can see
    /// what a household order was worth to it.
    pub paid_to: Vec<(u64, f64)>,
    /// Jobs the household intends to do itself, to be raised as real work
    /// orders.
    pub to_make: Vec<(Need, DefId)>,
    /// Things taken that nobody owned.
    pub scavenged: Vec<(Need, DefId)>,
    /// The bill for running what it owns, which is the utility line and is
    /// not discretionary.
    pub utilities: f64,
    /// What it got rid of, and what became of each. **Not destruction**:
    /// most of these are sold, stored or robbed for parts.
    pub discarded: Vec<(Id<ItemInstance>, crate::wip::Disposition)>,
    /// **They could not pay it.** Everything electric stopped.
    pub cut_off: bool,
    /// A disconnection notice was served. **Which is where most people find
    /// the money**, and is why utilities serve far more notices than they
    /// act on.
    pub notice_served: bool,
    /// It would have been cut off, and the season forbade it. The debt goes
    /// on growing instead.
    pub protected_by_the_season: bool,
    /// What is owed to the utility at the end of it.
    pub arrears: f64,
    pub spent: f64,
    pub hours: f64,
    /// **What was left at the end of it**, which is the whole reason
    /// anybody ever owns anything expensive. Without it a household spends
    /// the same week for ever and can never save up for the machine that
    /// would give it its evenings back.
    pub left: Means,
}

impl Outcome {
    /// What share of its needs the household actually got, weighted by how
    /// badly each was wanted.
    pub fn satisfaction(&self, wanted: &[Requirement]) -> f64 {
        let total: f64 = wanted.iter().map(|r| r.need.urgency()).sum();
        if total <= 0.0 {
            return 1.0;
        }
        let missed: f64 = self
            .without
            .iter()
            .chain(self.unmet.iter().map(|o| &o.need))
            .map(|n| n.urgency())
            .sum();
        ((total - missed) / total).clamp(0.0, 1.0)
    }

    /// What it spent, by category — the thing that gets held against the
    /// published shares.
    pub fn by_category(&self) -> Vec<(Category, f64)> {
        let mut out: Vec<(Category, f64)> = Vec::new();
        for (o, paid) in &self.bought {
            let c = o.need.category();
            match out.iter_mut().find(|x| x.0 == c) {
                Some(x) => x.1 += paid,
                None => out.push((c, *paid)),
            }
        }
        out.sort_by_key(|x| x.0);
        out
    }
}

// =====================================================================
// the household
// =====================================================================

/// **A household, as a consumer.**
///
/// Deliberately not a person and not a `populace` sample: it is the unit
/// that owns things, needs things and pays for them, and several people
/// share one. Which is the whole reason a second adult is cheap.
#[derive(Clone, Debug)]
pub struct Household {
    pub who: Roster,
    pub where_: Climate,
    /// What it owns that matters to a need. Durables only — nobody tracks
    /// individual potatoes.
    pub owns: Vec<Id<ItemInstance>>,
    /// Flow goods in the cupboard, in days of cover. Food is a flow and an
    /// appliance is a stock, and conflating them is how a model comes to
    /// think a household buys a washing machine every week.
    pub larder: Vec<(Need, f64)>,
    /// Hours a week the household can spend on its own work, over and
    /// above earning a living. Real: US time use puts household activities
    /// at about 1.8 h a day, so a two-adult house has 25 h a week.
    pub free_hours: f64,
    /// Where its things sit when it buys them.
    pub home: Placement,
    /// **What somebody here could make**, which is tools plus knowing how
    /// and is a fact about the household rather than about the market. It
    /// is also the difference between a place where things get mended and
    /// one where they get thrown away.
    pub can_make: Vec<DefId>,
    /// Needs it could not meet, and for how long. **Kept**, because a
    /// shortage that is forgotten every period is not a shortage.
    pub going_without: Vec<(Need, u32)>,
    /// **Its account with the utility.** A bill is monthly, in arrears, and
    /// nobody is cut off the week they cannot pay it — see `utility.rs`.
    /// `None` where there is no supply to be had, which is a fifth of the
    /// world.
    pub supply: Option<crate::utility::Account>,
    /// Where in the year it is, because the winter rules turn on it.
    pub day_of_year: u32,
}

impl Household {
    pub fn new(who: Roster, where_: Climate, home: Placement) -> Self {
        let adults = who.adults.max(1) as f64;
        Household {
            who,
            where_,
            owns: Vec::new(),
            larder: Vec::new(),
            free_hours: 12.5 * adults,
            home,
            can_make: Vec::new(),
            going_without: Vec::new(),
            supply: Some(crate::utility::Account::new(crate::utility::Tariff::ordinary())),
            day_of_year: 0,
        }
    }

    pub fn wants(&self) -> Vec<Requirement> {
        requirements(self.who, self.where_)
    }

    /// Days of cover of a flow need.
    pub fn cover(&self, need: Need) -> f64 {
        self.larder.iter().find(|(n, _)| *n == need).map(|(_, d)| *d).unwrap_or(0.0)
    }

    fn add_cover(&mut self, need: Need, days: f64) {
        match self.larder.iter_mut().find(|(n, _)| *n == need) {
            Some(x) => x.1 += days,
            None => self.larder.push((need, days)),
        }
    }

    /// How long it has been going without something.
    pub fn without_for(&self, need: Need) -> u32 {
        self.going_without.iter().find(|(n, _)| *n == need).map(|(_, d)| *d).unwrap_or(0)
    }

    fn note_without(&mut self, need: Need, days: u32) {
        match self.going_without.iter_mut().find(|(n, _)| *n == need) {
            Some(x) => x.1 += days,
            None => self.going_without.push((need, days)),
        }
    }

    fn note_met(&mut self, need: Need) {
        self.going_without.retain(|(n, _)| *n != need);
    }
}

// =====================================================================
// a period
// =====================================================================

/// **Run a household's week.**
///
/// Every need is considered, in order of how badly it is wanted, and the
/// money runs out where it runs out — which is why a poor household's
/// spending comes out concentrated in food and warmth without any share
/// having been written down. Engel's law is an output here.
#[allow(clippy::too_many_arguments)]
pub fn a_period(
    home: &mut Household,
    store: &mut Store,
    cat: &Catalogue,
    market: &mut Market,
    mut means: Means,
    days: u32,
    event: u64,
) -> Outcome {
    let mut out = Outcome::default();
    let mut wanted = home.wants();
    // **Most urgent first.** Nothing else in this function decides what a
    // poor household buys.
    wanted.sort_by(|a, b| b.need.urgency().total_cmp(&a.need.urgency()));

    let mut hours_left = home.free_hours;

    // **The bill is monthly, in arrears, and nobody is cut off the week
    // they cannot pay it.** A month of usage, a bill, three weeks to pay,
    // a reminder, a formal notice, and only then a crew — and in a cold
    // state in winter, not even then. Which is where energy debt comes
    // from: a household that cannot pay in January is not disconnected in
    // January, it is *in arrears*, and the reckoning arrives in April.
    let mut supply = true;
    if let Some(mut account) = home.supply {
        let per_day = running_kwh(&home.owns, store, cat, &wanted) / 7.0;
        for d in 0..days {
            // **A household pays its energy bill high up the list**, with
            // the rent rather than with the shopping — and pays what it can
            // when it cannot pay all of it, which is what people do.
            let owed = account.arrears;
            let pay = if owed > 0.0 {
                let can = means.income * 0.5 + means.savings * 0.5;
                owed.min(can.max(0.0))
            } else {
                0.0
            };
            if pay > 0.0 {
                let from_income = pay.min(means.income);
                means.income -= from_income;
                means.savings = (means.savings - (pay - from_income)).max(0.0);
                out.spent += pay;
                out.utilities += pay;
                out.bought.push((
                    Order {
                        need: Need::Light,
                        intent: Intent::PayForTheService,
                        price: pay,
                        covers: 1.0,
                    },
                    pay,
                ));
            }
            let e = crate::utility::a_day(
                &mut account,
                per_day,
                pay,
                home.where_,
                home.day_of_year + d,
            );
            if e.cut_off {
                out.cut_off = true;
            }
            if e.notice {
                out.notice_served = true;
            }
            if e.protected {
                out.protected_by_the_season = true;
            }
        }
        supply = account.standing.supplied();
        out.arrears = account.arrears;
        home.supply = Some(account);
    }
    home.day_of_year = (home.day_of_year + days) % 365;

    for req in &wanted {
        let need = req.need;

        // **A flow is bought again every period; a stock is not.** Food
        // does not stop being needed because there was food last week.
        if flow_need(need) {
            let want_days = days as f64;
            let have = home.cover(need);
            let short = (want_days - have).max(0.0);
            if short > 0.0 {
                let unit = market.price_service(need).unwrap_or_else(|| default_flow_price(need));
                let bill = unit * short * req.level;
                let order = Order {
                    need,
                    intent: Intent::PayForTheService,
                    price: bill,
                    covers: short,
                };
                if !market.sells_flow(need) {
                    // **Wanted, and nobody selling.** Not a purchase.
                    out.unmet.push(order);
                    home.note_without(need, days);
                    continue;
                }
                if bill <= means.income + means.savings {
                    let paid = bill;
                    means.income = (means.income - paid).max(0.0);
                    if paid > means.income {
                        means.savings = (means.savings - (paid - means.income)).max(0.0);
                    }
                    out.spent += paid;
                    out.bought.push((order, paid));
                    home.add_cover(need, short);
                    home.note_met(need);
                } else {
                    // Buys what it can and goes short on the rest.
                    let afford = means.income + means.savings;
                    let got = if unit > 0.0 { afford / (unit * req.level.max(0.01)) } else { 0.0 };
                    means.income = 0.0;
                    means.savings = 0.0;
                    out.spent += afford;
                    out.bought.push((order, afford));
                    home.add_cover(need, got);
                    if got < short {
                        home.note_without(need, days);
                        out.without.push(need);
                    }
                }
            } else {
                home.note_met(need);
            }
            continue;
        }

        // **Nothing electric works when the supply is off.** The machines
        // are still theirs and still in the house; what they cannot do is
        // run, so the household falls back on its hands the way it would if
        // it had never owned them.
        if !supply && runs_on_power(need, &home.owns, store, cat) {
            let by_hand = hours_to_do_it_by_hand(need, req.level);
            let h = by_hand.min(hours_left);
            if by_hand > 0.0 && h >= by_hand * 0.6 {
                hours_left -= h;
                out.hours += h;
                out.by_hand.push((need, h));
            } else {
                out.without.push(need);
                home.note_without(need, days);
            }
            continue;
        }

        // A stock need: what would this household do about it?
        let intent = what_to_do(need, req.level, home, store, cat, market, means);
        match intent {
            Intent::Nothing => home.note_met(need),
            Intent::Repair(id) => {
                let Some(item) = store.get(id) else { continue };
                let replacement = market.price_new(item.definition);
                if let Some(cost) = worth_repairing(item, cat, market, replacement) {
                    means.savings = (means.savings - cost).max(0.0);
                    out.spent += cost;
                    out.repairs.push((id, cost));
                    home.note_met(need);
                }
            }
            Intent::BuyUsed(def) | Intent::BuyNew(def) => {
                let used = matches!(intent, Intent::BuyUsed(_));
                let price = if used { market.price_used(def) } else { market.price_new(def) };
                let covers = what_it_does(cat.get(def).map(|d| d.name).unwrap_or(""))
                    .iter()
                    .find(|s| s.need == need)
                    .map(|s| s.covers)
                    .unwrap_or(1.0);
                let order = Order { need, intent, price: price.unwrap_or(0.0), covers };
                // **The shelf is checked at the counter, not at the plan.**
                // A price with nothing behind it fills no order, which is
                // exactly how a shortage should feel to a household.
                let bought = match price {
                    Some(p) if p <= means.for_a_lump_sum() => market.take(def, used),
                    _ => None,
                };
                match bought {
                    Some((seller, p)) => {
                        // **Comes out of savings first**, which is what a
                        // lump sum is and why a household with none cannot
                        // buy the cheap-in-the-long-run answer.
                        let from_savings = p.min(means.savings);
                        means.savings -= from_savings;
                        means.income = (means.income - (p - from_savings)).max(0.0);
                        out.spent += p;
                        out.bought.push((order, p));
                        out.paid_to.push((seller, p));
                        let mut thing = ItemInstance::one(cat, def);
                        if used {
                            // **Second-hand is not new.** Same service,
                            // less life left in it.
                            thing.condition.wear = 0.45;
                        }
                        let id = store.add(thing, home.home);
                        home.owns.push(id);
                        home.note_met(need);
                    }
                    None => {
                        out.unmet.push(order);
                        home.note_without(need, days);
                    }
                }
            }
            Intent::MakeIt(def) => {
                // **Recorded as work to be done, not as a purchase.** The
                // making itself goes through `make_it_yourself`, which is
                // the same work order a factory raises.
                out.to_make.push((need, def));
                home.note_met(need);
            }
            Intent::Scavenge(def) => {
                let mut thing = ItemInstance::one(cat, def);
                // **What you find is worn out**, which is why this is the
                // last route and not the first.
                thing.condition.wear = 0.80;
                thing.condition.damage = 0.20;
                let id = store.add(thing, home.home);
                home.owns.push(id);
                out.scavenged.push((need, def));
                home.note_met(need);
            }
            Intent::ByHand { hours_a_week } => {
                let h = hours_a_week.min(hours_left);
                if h >= hours_a_week * 0.6 {
                    hours_left -= h;
                    out.hours += h;
                    out.by_hand.push((need, h));
                    home.note_met(need);
                } else {
                    // **The hours ran out.** A household doing everything by
                    // hand cannot do everything by hand.
                    out.without.push(need);
                    home.note_without(need, days);
                }
            }
            Intent::PayForTheService => {
                let p = market.price_service(need).unwrap_or(0.0) * req.level;
                if p <= means.income {
                    means.income -= p;
                    out.spent += p;
                    out.bought.push((
                        Order { need, intent, price: p, covers: req.level },
                        p,
                    ));
                    home.note_met(need);
                } else {
                    out.without.push(need);
                    home.note_without(need, days);
                }
            }
            Intent::GoWithout => {
                out.without.push(need);
                home.note_without(need, days);
            }
        }
    }

    // **The week passes: the cupboard empties and the machines wear.**
    for (_, d) in home.larder.iter_mut() {
        *d = (*d - days as f64).max(0.0);
    }
    wear_and_failure(home, store, cat, days, event);

    // **And what is past mending goes.** `dispose_of` existed from the
    // first commit and nothing ever called it, so a household accumulated
    // dead appliances for ever — which is how a man came to own three
    // heaters, one of them broken, and to be charged for running all of
    // them. What it lets go of is also where a second-hand market gets its
    // stock, which is the other half of the same omission.
    let doomed: Vec<Id<ItemInstance>> = home
        .owns
        .iter()
        .copied()
        .filter(|&id| {
            store
                .get(id)
                .map(|i| !in_service(i) || i.condition.wear > 0.95)
                .unwrap_or(true)
        })
        .collect();
    for id in doomed {
        let fate = dispose_of(home, store, id, market, false);
        out.discarded.push((id, fate));
        home.owns.retain(|&x| x != id);
        match fate {
            crate::wip::Disposition::OfferedForSale => {
                if let Some(stall) = offered_for_sale_one(store, cat, id, market, 0) {
                    market.stock(stall);
                }
                store.end(id, crate::item::ItemEnd::Consumed, days);
            }
            _ => {
                store.end(id, crate::item::ItemEnd::Destroyed, days);
            }
        }
    }
    // **What is not spent is put by.** Real saving is exactly this: the
    // residue of a week, and it is how a poor household eventually reaches
    // something it could never buy out of a wage.
    out.left = Means { income: 0.0, savings: means.savings + means.income, credit: means.credit };
    out
}

/// Whether a need is met by a flow bought over and over rather than by
/// something owned. **Food is a flow and a cooker is a stock**, and the
/// two must never be summed.
pub fn flow_need(need: Need) -> bool {
    matches!(need, Need::Nutrition | Need::Health)
}

/// A fallback unit price per day of cover, for a market that has not been
/// given one.
fn default_flow_price(need: Need) -> f64 {
    match need {
        Need::Nutrition => 1.0,
        Need::Health => 0.15,
        _ => 0.5,
    }
}

/// **Things wear out, and then they break.**
///
/// Real service lives: a washing machine 11 years, a refrigerator 13, a
/// stove 15, clothing 2-4. The draw is deterministic and keyed, so
/// reloading a save cannot give somebody a working machine back.
pub fn wear_and_failure(
    home: &mut Household,
    store: &mut Store,
    cat: &Catalogue,
    days: u32,
    event: u64,
) {
    for (k, &id) in home.owns.clone().iter().enumerate() {
        let Some(item) = store.get(id) else { continue };
        let Some(def) = cat.get(item.definition) else { continue };
        let life = def.family.lifecycle().typical_life_days();
        let per_day = 1.0 / life.max(1.0);
        let Some(item) = store.get_mut(id) else { continue };
        item.condition.wear = (item.condition.wear + per_day * days as f64).min(1.0);
        // **An older machine fails more often**, which is the whole shape
        // of an appliance's life and the reason a repair trade exists.
        let hazard = 0.00025 * days as f64 * (0.4 + item.condition.wear * 2.5);
        let u = crate::rng::Rng::new(crate::save::channel(event, k as u64, "durable failure"))
            .next_f32() as f64;
        if u < hazard {
            item.condition.damage = (item.condition.damage + 0.5).min(1.0);
        }
    }
}

// =====================================================================
// making it yourself
// =====================================================================

/// **Household production goes through the same work orders as a factory.**
///
/// Not a parallel mechanism with its own arithmetic: the same recipe, the
/// same three kinds of time, the same failure model, the same quality. What
/// differs is the workplace — a kitchen table rather than a machine shop —
/// and that is exactly what makes home-made goods worse and slower rather
/// than a different category of thing.
pub fn make_it_yourself(
    home: &mut Household,
    store: &mut Store,
    cat: &Catalogue,
    book: &crate::craft::RecipeBook,
    kitchen: &crate::craft::Workplace,
    recipe: usize,
    maker: crate::craft::Maker,
    order_id: u64,
    day: u32,
) -> Result<Id<ItemInstance>, crate::craft::Halt> {
    let mut order =
        crate::craft::WorkOrder::begin_for(order_id, recipe, 1, 0, day, home.home);
    let mut last = crate::craft::Halt::Running;
    for _ in 0..4_000 {
        let before = (order.step, order.step_done, order.elapsed_min);
        last = order.advance(30.0, book, cat, kitchen, maker);
        if order.finished() {
            break;
        }
        if (order.step, order.step_done, order.elapsed_min) == before {
            break;
        }
    }
    if !matches!(last, crate::craft::Halt::Done) {
        return Err(last);
    }
    match order.deliver_into(book, cat, day, store) {
        Ok(id) => {
            home.owns.push(id);
            Ok(id)
        }
        Err(_) => Err(crate::craft::Halt::Abandoned),
    }
}

// =====================================================================
// getting rid of things
// =====================================================================

/// **What a household does with something it has finished with.**
///
/// Not destruction. `wip::Disposition` already carries the five real
/// answers, and a household reaches them for household reasons: a sound
/// thing goes up for sale or to somebody who wants it, a broken thing with
/// good parts gets robbed, and only what nobody wants at all is thrown out.
pub fn dispose_of(
    home: &Household,
    store: &Store,
    item: Id<ItemInstance>,
    market: &Market,
    somebody_wants_it: bool,
) -> crate::wip::Disposition {
    let Some(i) = store.get(item) else { return crate::wip::Disposition::Abandoned };
    let sound = 1.0 - i.condition.wear.max(i.condition.damage);
    let still_used = home.owns.contains(&item) && sound > 0.5;
    // Somebody within reach who would buy it — which is what a second-hand
    // market *is*, seen from the other side.
    let a_buyer = market.stalls.iter().any(|s| s.used) || somebody_wants_it;
    let worth_robbing = sound > 0.15;
    crate::wip::what_becomes_of_it(sound, still_used, a_buyer, worth_robbing, true)
}

/// One thing, priced for the second-hand trade.
pub fn offered_for_sale_one(
    store: &Store,
    cat: &Catalogue,
    id: Id<ItemInstance>,
    market: &Market,
    seller: u64,
) -> Option<Stall> {
    let i = store.get(id)?;
    let d = cat.get(i.definition)?;
    let sound = 1.0 - i.condition.wear.max(i.condition.damage);
    if sound < 0.35 {
        return None;
    }
    let new = market.price_new(i.definition)?;
    Some(Stall::second_hand(d.id, new * (0.18 + sound * 0.25), 1, seller))
}

/// **What a household puts back into the second-hand market**, which is
/// where the used stock in `Market::used` comes from. A market with no
/// households selling into it has nothing on its shelves.
pub fn offered_for_sale(
    home: &Household,
    store: &Store,
    cat: &Catalogue,
    market: &Market,
    seller: u64,
) -> Vec<Stall> {
    home.owns
        .iter()
        .filter_map(|&id| {
            let i = store.get(id)?;
            let d = cat.get(i.definition)?;
            let sound = 1.0 - i.condition.wear.max(i.condition.damage);
            if sound < 0.35 {
                return None;
            }
            // A used thing is worth what is left of it, discounted hard —
            // real second-hand appliances go for a fifth to a third of new.
            let new = market.price_new(i.definition)?;
            Some(Stall::second_hand(d.id, new * (0.18 + sound * 0.25), 1, seller))
        })
        .collect()
}

// =====================================================================
// several households at once
// =====================================================================

/// **What a town's households want, without simulating every one.**
///
/// The design doc's rule arriving at demand: populations stay statistical
/// until something promotes them. What matters is that the two agree — a
/// hundred individuated households and one aggregate standing for a hundred
/// must ask the market for the same thing, or promoting somebody to detail
/// changes the economy.
#[derive(Clone, Debug, Default)]
pub struct TownDemand {
    pub households: f64,
    /// Requirement level summed over all of them, per need.
    pub wanted: Vec<(Need, f64)>,
}

/// Sum what a set of households requires.
pub fn aggregate(homes: &[Household]) -> TownDemand {
    let mut wanted: Vec<(Need, f64)> = Vec::new();
    for h in homes {
        for r in h.wants() {
            match wanted.iter_mut().find(|(n, _)| *n == r.need) {
                Some(x) => x.1 += r.level,
                None => wanted.push((r.need, r.level)),
            }
        }
    }
    wanted.sort_by_key(|x| x.0);
    TownDemand { households: homes.len() as f64, wanted }
}

/// The same thing computed from a representative household and a count,
/// which is what a town that nobody is looking at closely uses.
pub fn aggregate_from(typical: &Household, count: f64) -> TownDemand {
    let wanted = typical
        .wants()
        .into_iter()
        .map(|r| (r.need, r.level * count))
        .collect::<Vec<_>>();
    let mut wanted = wanted;
    wanted.sort_by_key(|x| x.0);
    TownDemand { households: count, wanted }
}

impl TownDemand {
    pub fn level(&self, need: Need) -> f64 {
        self.wanted.iter().find(|(n, _)| *n == need).map(|(_, l)| *l).unwrap_or(0.0)
    }
}

// =====================================================================
// what a shop actually employs people for
// =====================================================================

/// **Retail headcount follows customers served and floor kept, not tonnes
/// moved.**
///
/// This is the error the census work found and could not fix: shop staffing
/// came off the tonnage of the commodities the model happened to have, and
/// a supermarket runs about 300 staff on 75 tonnes a day — four people per
/// daily tonne against a flour mill's 0.2 person-hours per tonne. What a
/// shop employs follows the *transactions*.
///
/// Real anchors: a checkout serves about 25 customers an hour; the median
/// US supermarket is about 3,700 m2 and takes 15-25,000 customers a week;
/// and grocery runs roughly one employee per 500-800 sq ft of selling area,
/// which is 46-74 m2.
///
/// **The answer is in full-time equivalents**, because that is what a model
/// built out of recipe labour-hours produces. Retail is about 9.1% of
/// people in work and only about 6.4% of hours, and confusing the two is
/// exactly the denominator error `census.rs` exists to prevent.
pub fn shop_staff_for(customers_a_day: f64, floor_m2: f64) -> f64 {
    // Tills, at 25 customers an hour over a 12-hour trading day, plus the
    // 1.4 FTE it takes to keep one till manned all day.
    let tills = (customers_a_day / (25.0 * 12.0)).ceil().max(1.0);
    let checkout = tills * 1.4;
    // Filling shelves, the counters, and keeping the place.
    let floor = floor_m2 / 70.0;
    checkout + floor
}

/// **Heads are not full-time equivalents**, and retail is where the gap is
/// widest: about 60% of shop work is part-time and a part-timer averages
/// well under half a week, so a store's payroll is far longer than its
/// hours suggest. Real US retail averages ~30.5 hours a week against a
/// 40-hour full week.
pub fn heads_from_fte(fte: f64) -> f64 {
    fte / 0.76
}

/// How many customers a household is, over a period. Real: US households
/// shop for groceries about 1.6 times a week, plus everything else.
pub fn visits_a_week(who: Roster) -> f64 {
    1.6 + 0.35 * (who.heads().max(1) as f64 - 1.0)
}
