//! **Not everything is reusable, and what decides it is money.**
//!
//! `material.rs` has said since it was written what each material can ever
//! come back as — feedstock, downcycled, fuel, or nothing — and
//! `teardown.rs` has been producing piles of it. What neither could say is
//! whether anybody would actually take the pile, because that is not a
//! property of the material. It is the spread between what a mill pays and
//! what it costs to collect, sort and haul it, and **that spread is why
//! steel is recycled everywhere and mixed plastics almost nowhere.**
//!
//! Real US scrap prices, per tonne, and the range is four orders of
//! magnitude:
//!
//! | | |
//! |---|---|
//! | copper, bare bright | **~$8,500** |
//! | brass | ~$4,500 |
//! | aluminium, used cans | ~$1,400 |
//! | stainless 304 | ~$1,300 |
//! | lead, battery scrap | ~$1,000 |
//! | ferrous, HMS #1 | ~$350 |
//! | cardboard | ~$120 |
//! | glass | ~$20, **often negative once sorted** |
//! | mixed rigid plastics | **$0 to negative** |
//! | tyres | **a gate fee of $100-200** |
//!
//! And the recycling rates follow the prices almost exactly *(US EPA)*:
//! lead-acid batteries **99%**, steel cans 71%, paper 68%, aluminium cans
//! 50%, glass 31%, **plastics 8.7%**. Nobody is being virtuous about lead.

use crate::material::Material;
use crate::material::Recovers;

// =====================================================================
// what a material is worth at the gate
// =====================================================================

/// **What a yard pays for a tonne of it, and a negative number means you
/// pay them.**
///
/// A gate fee is not a failure of the model. It is the ordinary case for
/// tyres, gypsum, treated timber and mixed waste, and it is exactly why
/// fly-tipping exists: somebody worked out that the fee was avoidable.
pub fn price_a_tonne(m: Material) -> f64 {
    use Material::*;
    match m {
        Copper => 8_500.0,
        Brass => 4_500.0,
        Aluminium => 1_400.0,
        Stainless => 1_300.0,
        ToolSteel => 400.0,
        // **Lead is worth having and that is the whole reason 99% of it
        // comes back.** Nobody recycles car batteries out of principle.
        Lead => 1_000.0,
        Zinc => 1_800.0,
        Nichrome => 2_200.0,
        Solder => 3_000.0,
        Silicon => 600.0,
        MildSteel => 350.0,
        Paperboard => 120.0,
        // Fuel-grade: worth something to burn and nothing to a mill.
        Oak | Pine | Plywood => 40.0,
        Cotton | Wool | Leather => 20.0,
        // **Downcycled, and barely worth the haul.**
        Glass => 20.0,
        Polyethylene => 250.0,
        Abs => 100.0,
        Concrete | Brick => 8.0,
        // **You pay to get rid of these**, which is the ordinary case for
        // far more of a household than anybody expects.
        Rubber => -150.0,
        Particleboard => -60.0,
        Gypsum => -70.0,
        Ceramic | Mortar => -55.0,
        Polyester => -40.0,
        Adhesive | Paint | Lubricant => -220.0,
        // Hazardous, and the fee reflects the handling rather than the
        // material.
        Lithium | Electrolyte => -900.0,
        Propellant => -3_000.0,
        // Food and the like: composted or tipped.
        Flour | Thread | Mica | Ferrite => -55.0,
        // Nobody weighs in water.
        Water => 0.0,
    }
}

/// **You get the clean price only if you have clean metal.**
///
/// A motor is not copper. It is a motor, and it sells as electric motor
/// scrap at a fraction of the copper price, because somebody has to strip
/// it before a smelter will look at it. The whole scrap trade lives in that
/// gap, and pricing an unsorted object at the sum of its clean fractions
/// overvalued a dead washing machine by three times.
///
/// Real US grades, per tonne, for what is chemically the same copper:
///
/// | | |
/// |---|---|
/// | bare bright copper wire | **$8,500** |
/// | insulated wire, low grade | $1,500-2,500 |
/// | electric motors | **$350-500** |
/// | mixed appliance scrap | $150-250 |
/// | a whole end-of-life car | $200-250 |
pub const UNSORTED_RATE: f64 = 200.0;

/// What a tonne fetches given how much work has been done to it. Anything
/// worth more than mixed scrap is dragged down toward it by not having been
/// taken apart; anything worth less is not improved by it.
pub fn as_found(m: Material, stripped: bool) -> f64 {
    let clean = price_a_tonne(m);
    if stripped || clean <= UNSORTED_RATE {
        return clean;
    }
    // A yard buying a whole object pays a mixed rate and takes the upside
    // for itself, because it is the one that will do the stripping.
    UNSORTED_RATE + (clean - UNSORTED_RATE) * 0.12
}

/// **What stripping it is worth**, against what it costs in somebody's
/// time. The reason a yard shreds and a man with a Saturday strips.
pub fn worth_stripping(materials: &[(Material, f64)], hours: f64, wage: f64) -> bool {
    let sorted: f64 = materials.iter().map(|&(m, t)| as_found(m, true) * t).sum();
    let found: f64 = materials.iter().map(|&(m, t)| as_found(m, false) * t).sum();
    sorted - found > hours * wage
}

/// **The tipping fee for mixed waste**, which is the floor under every
/// other decision here. Real US average is about $55 a tonne and the range
/// is $30-150 depending on how far the nearest hole is.
pub const TIPPING_FEE: f64 = 55.0;

/// **What it costs to haul a tonne a kilometre**, which is what turns a
/// positive price into a negative one over enough distance. The reason
/// there is a scrapyard in every town and a copper refinery in hardly any.
pub const HAULAGE_PER_TONNE_KM: f64 = 0.14;

// =====================================================================
// what turns up at the gate
// =====================================================================

/// **A load, as it actually arrives**: a heap of mixed material with
/// something wrong with it.
#[derive(Clone, Debug, Default)]
pub struct Load {
    /// What is in it, by mass in tonnes.
    pub materials: Vec<(Material, f64)>,
    /// **What is in it that should not be.** Single-stream recycling runs
    /// at 15-25% contamination in reality, and a MRF will reject a load
    /// over about 10%.
    pub contamination: f64,
    /// Whether anything in it is a fire, a poison or an explosion. A
    /// lithium cell in a shredder is a real and growing problem — US
    /// facilities report hundreds of fires a year.
    pub declared_hazard: bool,
}

impl Load {
    pub fn of(materials: &[(Material, f64)]) -> Self {
        Load {
            materials: materials.to_vec(),
            ..Default::default()
        }
    }

    pub fn tonnes(&self) -> f64 {
        self.materials.iter().map(|m| m.1).sum()
    }

    /// **Whether it is carrying something dangerous, declared or not.**
    /// Asked of the materials rather than of anybody's paperwork, which is
    /// exactly the difference that catches a yard out.
    pub fn actually_hazardous(&self) -> bool {
        self.materials
            .iter()
            .any(|&(m, kg)| kg > 0.0 && m.hazardous())
    }
}

/// What a yard will and will not take.
#[derive(Clone, Debug)]
pub struct Yard {
    pub owner: u64,
    /// How far the load has to travel to reach it.
    pub km: f64,
    /// **Whether it is licensed for hazardous material.** Most are not, and
    /// a load that turns out to contain some is turned away — which is how
    /// it ends up in a hedge.
    pub takes_hazardous: bool,
    /// Whether it sorts, or takes only what is already clean. A yard that
    /// sorts can accept a dirtier load and pays less for it.
    pub sorts: bool,
    /// Above this, the load is refused outright. Real MRF limits are around
    /// 10%; China's National Sword set 0.5% and collapsed the world market
    /// for mixed recyclate overnight.
    pub contamination_limit: f64,
}

impl Yard {
    /// An ordinary town scrapyard: metals, no licence for anything nasty,
    /// and it will pick through a load.
    pub fn ordinary(owner: u64, km: f64) -> Self {
        Yard {
            owner,
            km,
            takes_hazardous: false,
            sorts: true,
            contamination_limit: 0.15,
        }
    }

    /// A licensed processor, further away and able to take what nobody else
    /// will.
    pub fn licensed(owner: u64, km: f64) -> Self {
        Yard {
            owner,
            km,
            takes_hazardous: true,
            sorts: true,
            contamination_limit: 0.30,
        }
    }
}

// =====================================================================
// what happens to it
// =====================================================================

/// What the yard did with a load.
#[derive(Clone, Debug, PartialEq)]
pub enum Settlement {
    /// Taken, and this much changed hands. **Negative means the customer
    /// paid**, which is the ordinary case for a great deal of it.
    Taken {
        paid: f64,
        /// What went back out as feedstock, by mass.
        recovered: Vec<(Material, f64)>,
        /// What could not be sold and went to the hole, in tonnes.
        residue: f64,
    },
    /// **Turned away.** The load goes home, or into a hedge.
    Refused(Refusal),
}

/// Why a load was refused, because the reasons are not interchangeable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Too much of the wrong thing in it.
    TooContaminated,
    /// Something in it needs a licence this yard does not hold.
    Hazardous,
    /// It would cost more to handle than it is worth, and the customer will
    /// not pay the difference.
    NotWorthTaking,
}

impl Refusal {
    pub fn name(self) -> &'static str {
        match self {
            Refusal::TooContaminated => "too much rubbish in it",
            Refusal::Hazardous => "not licensed for it",
            Refusal::NotWorthTaking => "not worth the handling",
        }
    }
}

/// **Weigh in.**
///
/// The arithmetic a real yard does: what the material is worth, less the
/// haul, less what has to be tipped, less the cost of sorting — and if the
/// answer is negative the customer pays the difference or takes it away
/// again.
pub fn weigh_in(load: &Load, yard: &Yard, customer_will_pay: f64) -> Settlement {
    // **Hazardous content is checked, not taken on trust.** A yard that
    // finds a lithium cell in a bale has a fire, not a discrepancy.
    if load.actually_hazardous() && !yard.takes_hazardous {
        return Settlement::Refused(Refusal::Hazardous);
    }
    if load.contamination > yard.contamination_limit {
        return Settlement::Refused(Refusal::TooContaminated);
    }

    let tonnes = load.tonnes();
    if tonnes <= 0.0 {
        return Settlement::Refused(Refusal::NotWorthTaking);
    }

    let mut gross = 0.0;
    let mut recovered: Vec<(Material, f64)> = Vec::new();
    let mut residue = 0.0;
    for &(m, t) in &load.materials {
        // **Contamination is not only a discount, it is a loss.** What is
        // mixed in with the wrong thing goes to the hole with it.
        let clean = t * (1.0 - load.contamination);
        residue += t - clean;
        let price = price_a_tonne(m);
        gross += price * clean;
        match m.recovers() {
            Recovers::Feedstock | Recovers::Downcycled if price > 0.0 => {
                match recovered.iter_mut().find(|r| r.0 == m) {
                    Some(r) => r.1 += clean,
                    None => recovered.push((m, clean)),
                }
            }
            // **Nothing is not a small something.** What cannot come back
            // is tipped, and the tipping is what the gate fee pays for.
            _ => residue += clean,
        }
    }

    // The haul, which is what makes distance decide whether a material is
    // recycled at all.
    let haul = tonnes * yard.km * HAULAGE_PER_TONNE_KM;
    // And the hole.
    let tipping = residue * TIPPING_FEE;
    // Sorting costs labour and a yard that does not sort simply cannot
    // take a mixed load.
    let sorting = if yard.sorts {
        tonnes * 18.0 * (1.0 + load.contamination * 4.0)
    } else {
        0.0
    };

    let net = gross - haul - tipping - sorting;
    if net < 0.0 && -net > customer_will_pay {
        return Settlement::Refused(Refusal::NotWorthTaking);
    }
    Settlement::Taken {
        paid: net,
        recovered,
        residue,
    }
}

/// **Whether it is worth anybody's while to collect this at all.**
///
/// The rule `logistics.rs` already applies to freight, arriving at waste:
/// a material is collected where the price beats the haul and buried where
/// it does not. Which is why there is a scrap merchant in every town and
/// mixed plastics go in the ground almost everywhere.
pub fn worth_collecting(m: Material, km: f64) -> bool {
    price_a_tonne(m) > km * HAULAGE_PER_TONNE_KM
}

/// **How far a tonne of it can travel before it stops being worth having.**
///
/// Reported against the earth rather than as a bare number, because
/// "60,714 km" is arithmetically correct and says nothing: the planet is
/// 40,000 km around, so anything past a few thousand simply means the
/// material goes wherever there is a buyer. Which is the real fact about
/// copper and the real fact about glass, and they are different facts.
pub fn economic_range_km(m: Material) -> f64 {
    (price_a_tonne(m) / HAULAGE_PER_TONNE_KM).max(0.0)
}

/// Whether it is worth shipping anywhere at all, which is what a range
/// beyond a few thousand kilometres actually means.
pub fn goes_anywhere(m: Material) -> bool {
    economic_range_km(m) > 5_000.0
}

// =====================================================================
// what a thing costs to be rid of
// =====================================================================

/// **Some things cost money to get rid of properly, and that is why they
/// end up in a hedge.**
///
/// A refrigerator is the standard case: its refrigerant has to be recovered
/// by a certified technician *(EPA Section 608)* before the shell can be
/// shredded, which costs $10-30. A mattress is $20-40 because it jams a
/// shredder and has to be pulled apart by hand. Tyres are a gate fee
/// everywhere.
pub fn special_handling(name: &str) -> f64 {
    match name {
        "refrigerator" => 22.0,
        "space heater" => 0.0,
        "telephone" => 4.0,
        "motor car" => 0.0,
        "washing machine" => 0.0,
        "battery pack" => 3.0,
        _ => 0.0,
    }
}

/// **What a whole object is worth as scrap**, net of getting it there and
/// of anything that has to be taken out of it first.
///
/// The number that decides whether a dead appliance is worth a trip to the
/// yard or gets left at the kerb — and for most household goods it is
/// close to nothing, which is exactly why so much of it is fly-tipped.
pub fn worth_as_scrap(name: &str, materials: &[(Material, f64)], km: f64, stripped: bool) -> f64 {
    let load = Load::of(materials);
    let tonnes = load.tonnes();
    let gross: f64 = materials
        .iter()
        .map(|&(m, t)| as_found(m, stripped) * t)
        .sum();
    gross - tonnes * km * HAULAGE_PER_TONNE_KM - special_handling(name)
}

// =====================================================================
// or you put it in the truck yourself
// =====================================================================

/// **What a household can shift on its own.**
///
/// The thing that decides whether getting rid of something costs ninety
/// dollars or eight, and it is a fact about what they own rather than about
/// the object. A car will take bagged rubbish and will not take a washing
/// machine: 70 kg through a car door is not a question of weight, it is a
/// question of shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carrying {
    /// On foot, with a bag.
    ByHand,
    /// A car boot. Bags and boxes, nothing bulky.
    ACar,
    /// A pickup, a van, or a car and a trailer. **This is the one that
    /// changes the arithmetic.**
    ATruck,
}

impl Carrying {
    /// What it will actually take in one trip, in kilograms. Real: a US
    /// half-ton pickup carries 700-900 kg and about two cubic yards of
    /// household junk.
    pub fn payload_kg(self) -> f64 {
        match self {
            Carrying::ByHand => 25.0,
            Carrying::ACar => 120.0,
            Carrying::ATruck => 800.0,
        }
    }

    /// Whether an awkward object will go in it at all, which is a question
    /// about the doors and not about the springs.
    pub fn takes_bulky(self) -> bool {
        matches!(self, Carrying::ATruck)
    }
}

/// **What the town does about the things nobody can carry.**
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Council {
    /// Free bulky collections a year. Real US practice varies enormously:
    /// many cities give one to three, others charge for every item, and
    /// some do nothing at all.
    pub free_collections_a_year: u32,
    /// What it charges beyond that. Real: $25-50 an item.
    pub per_item: f64,
    /// The minimum charge at the transfer station, whatever the weight.
    /// Real: $20-30, which is why one small item costs the same as half a
    /// load.
    pub minimum_gate_charge: f64,
    /// How far the tip is.
    pub km: f64,
    /// **How likely somebody is to be seen leaving it in a lay-by.** The
    /// only thing that reliably deters it.
    pub chance_seen: f64,
}

impl Council {
    pub fn a_town() -> Self {
        Council {
            free_collections_a_year: 2,
            per_item: 35.0,
            minimum_gate_charge: 25.0,
            km: 11.0,
            chance_seen: 0.55,
        }
    }

    /// Somewhere with no service and nobody watching, which is where the
    /// hedges are full.
    pub fn out_in_the_country() -> Self {
        Council {
            free_collections_a_year: 0,
            per_item: 60.0,
            minimum_gate_charge: 30.0,
            km: 42.0,
            chance_seen: 0.06,
        }
    }
}
/// **The ways of being rid of something.**
///
/// Three or four of these is not enough, and the missing ones are not
/// exotic: a great deal of what leaves a household is given away, taken
/// back by the shop that delivered the new one, put on the pavement for
/// whoever wants it, or simply put in the loft and never dealt with at all.
/// Which one somebody takes depends on what they own, what the town offers,
/// what their time is worth and who is watching.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HowToGetRidOfIt {
    /// It is worth money. Somebody will pay for it, or the yard will.
    SellIt { paid: f64 },
    /// **A deposit comes back.** The single most effective recycling
    /// mechanism there is: a core charge on a car battery is why 99% of
    /// lead-acid batteries are returned, and bottle-bill states get 60-90%
    /// against about 25% everywhere else.
    RedeemTheDeposit { refund: f64 },
    /// **The shop takes the old one away** when it delivers the new one.
    /// Real US retailers do this for $20-35 or free with delivery, and in
    /// the EU it is a legal duty. For a big appliance it is the commonest
    /// route by far, and it means a household replacing something mostly
    /// has no disposal problem at all.
    TradeIn { allowance: f64 },
    /// Given to somebody who wants it — a friend, a neighbour, a listing
    /// for free to collector.
    GiveItAway,
    /// **A charity, if they will take it**, and they refuse a great deal:
    /// no mattresses, no upholstery without fire labels, nothing that does
    /// not work. Donating junk is a cost transfer, and thrift stores spend
    /// real money disposing of what they are given.
    Donate,
    /// **Put it out on the pavement.** Anything metal is gone within hours
    /// in most cities, because people drive round collecting it — free
    /// disposal that works only for things with scrap value in them.
    LeaveItOut,
    /// The town takes it away, within the allowance.
    Kerbside,
    /// **Put it in the truck and drive it to the tip.** The gate fee, the
    /// fuel, and an afternoon.
    TakeItYourself { cost: f64, hours: f64 },
    /// Pay somebody with a truck. Real US junk removal: $75-150 for one
    /// large item.
    PayAHauler { fee: f64 },
    /// **Put it in the loft and decide later**, which is what most people
    /// do with most things. The US self-storage industry is about $44bn and
    /// roughly one household in nine rents a unit, a great deal of it
    /// holding things nobody will use again.
    KeepIt,
    /// Keep it for the parts and get rid of the rest.
    Cannibalize,
    /// **Burn it.** Free, rural, and illegal almost everywhere there is
    /// anybody to notice.
    BurnIt,
    /// **Leave it in the woods.** Illegal, free, and extremely common where
    /// the tip is a long way off and nobody is watching — the distance is
    /// itself a cause, which is why rural fly-tipping is worse.
    FlyTip,
    /// Leave it where it stands: in the flat when the tenancy ends, or by
    /// the roadside where it died.
    AbandonIt,
}

impl HowToGetRidOfIt {
    pub fn name(self) -> &'static str {
        use HowToGetRidOfIt::*;
        match self {
            SellIt { .. } => "weigh it in",
            RedeemTheDeposit { .. } => "take it back for the deposit",
            TradeIn { .. } => "let the shop take the old one",
            GiveItAway => "give it to somebody",
            Donate => "take it to a charity shop",
            LeaveItOut => "put it out on the pavement",
            Kerbside => "put it out for the council",
            TakeItYourself { .. } => "take it to the tip",
            PayAHauler { .. } => "pay somebody to take it",
            KeepIt => "put it in the loft",
            Cannibalize => "strip it for parts",
            BurnIt => "burn it",
            FlyTip => "leave it in the woods",
            AbandonIt => "walk away from it",
        }
    }

    /// What it costs in money. Negative is money in.
    pub fn cost(self) -> f64 {
        use HowToGetRidOfIt::*;
        match self {
            SellIt { paid } => -paid,
            RedeemTheDeposit { refund } => -refund,
            TradeIn { allowance } => -allowance,
            TakeItYourself { cost, .. } => cost,
            PayAHauler { fee } => fee,
            _ => 0.0,
        }
    }

    /// Whether it is lawful. **Three of these are not**, and that is the
    /// point of listing them.
    pub fn legal(self) -> bool {
        !matches!(self, HowToGetRidOfIt::FlyTip | HowToGetRidOfIt::AbandonIt)
    }

    /// **Whether the object still exists afterwards.** Which is a different
    /// question from whether the household still owns it, and is what makes
    /// giving it away worth distinguishing from burning it.
    pub fn survives(self) -> bool {
        use HowToGetRidOfIt::*;
        matches!(
            self,
            SellIt { .. }
                | RedeemTheDeposit { .. }
                | TradeIn { .. }
                | GiveItAway
                | Donate
                | LeaveItOut
                | KeepIt
                | FlyTip
                | AbandonIt
        )
    }

    /// What became of it, in the vocabulary `wip.rs` already uses.
    pub fn disposition(self) -> crate::wip::Disposition {
        use crate::wip::Disposition as D;
        use HowToGetRidOfIt::*;
        match self {
            SellIt { .. } | RedeemTheDeposit { .. } | TradeIn { .. } | LeaveItOut => {
                D::OfferedForSale
            }
            GiveItAway | Donate => D::OfferedForSale,
            KeepIt => D::Stored,
            Cannibalize => D::Cannibalized,
            Kerbside | TakeItYourself { .. } | PayAHauler { .. } | BurnIt => D::AwaitingTransport,
            FlyTip | AbandonIt => D::Abandoned,
        }
    }
}

/// What a hauler charges. Real US junk removal is $75-150 for a single
/// large item and $150-600 for a truckload, and the minimum is what makes
/// one small thing expensive.
pub fn hauler_fee(kg: f64, km: f64) -> f64 {
    (75.0 + kg * 0.18 + km * 1.1).max(75.0)
}

/// **Nobody drives one thing to the tip.**
///
/// The minimum gate charge, the fuel and the afternoon are all costs of the
/// *trip* rather than of the object, so one item carries the whole of them
/// and a truckload divides them. Which is exactly why a man with a truck
/// lets things pile up in the yard and then goes once — and why a single
/// broken fridge is cheaper left out for the council than driven anywhere.
///
/// Special handling does **not** divide: the technician charges per fridge.
pub fn cost_of_a_trip(kg: f64, council: &Council, special: f64, items: u32) -> (f64, f64) {
    let share = items.max(1) as f64;
    let by_weight = kg / 1_000.0 * TIPPING_FEE;
    let gate = by_weight.max(council.minimum_gate_charge / share);
    // Real: a pickup does about 12 l/100 km, and the return trip is twice
    // the distance to the tip.
    let fuel = council.km * 2.0 / 100.0 * 12.0 * 0.90 / share;
    let hours = (0.75 + council.km * 2.0 / 45.0) / share;
    (gate + fuel + special, hours)
}

/// **What a charity will actually take.**
///
/// They refuse a great deal, and for good reasons: a mattress cannot be
/// resold, upholstery without a fire label cannot be sold at all, and a
/// broken electrical is a liability. Real thrift operations spend serious
/// money disposing of what they are given, so donating junk is a cost
/// transfer rather than a good deed.
pub fn a_charity_would_take_it(name: &str, still_works: bool) -> bool {
    if !still_works {
        return false;
    }
    !matches!(
        name,
        "bed" | "mattress" | "sofa" | "refrigerator" | "cooking stove"
    )
}

/// **What comes back on a deposit.**
///
/// Real: a US car battery carries a $10-22 core charge, and it is exactly
/// why 99% of lead-acid batteries are returned — the highest recovery rate
/// of anything, and nothing to do with anybody's conscience.
pub fn deposit_on(name: &str) -> f64 {
    match name {
        "battery pack" => 15.0,
        "car battery" => 18.0,
        _ => 0.0,
    }
}

/// Everything a household needs to know to decide, gathered so the
/// decision does not need fourteen arguments.
#[derive(Clone, Copy, Debug)]
pub struct Circumstances<'a> {
    pub name: &'static str,
    pub materials: &'a [(Material, f64)],
    pub kg: f64,
    /// Whether it is awkward as well as heavy. A washing machine does not
    /// go in a car, and that is about the doors rather than the springs.
    pub bulky: bool,
    pub still_works: bool,
    pub carrying: Carrying,
    pub council: &'a Council,
    pub collections_used: u32,
    /// How many other things would go on the same trip.
    pub others_going: u32,
    /// What an hour of their time is worth to them.
    pub hourly_worth: f64,
    /// Their regard for the rules, -1 to 1.
    pub scruple: f64,
    /// Whether there is anywhere to put it and forget about it.
    pub room_to_store: bool,
    /// Whether anybody has said they want it.
    pub somebody_wants_it: bool,
    /// Whether there is a charity within reach that takes this sort of
    /// thing.
    pub charity_nearby: bool,
    /// Whether a new one is being delivered, and the shop will take the old
    /// one away. **The commonest route of all for a large appliance.**
    pub being_replaced: bool,
    /// Whether the pavement here is somewhere things vanish from.
    pub scrappers_about: bool,
    /// Whether burning it in the yard would be noticed.
    pub somewhere_to_burn: bool,
    pub event: u64,
}

/// **How this household, in this place, gets rid of this thing.**
///
/// Every branch is a real behaviour and the ordering is most of the
/// content. What decides it is not virtue: it is what they own, what the
/// town offers, what their afternoon is worth, and whether anybody would
/// see them.
pub fn how_to_get_rid_of_it(c: &Circumstances) -> HowToGetRidOfIt {
    use HowToGetRidOfIt::*;

    // 1. **A deposit comes back**, and that beats everything because it is
    //    money for doing the thing you were going to do anyway.
    let deposit = deposit_on(c.name);
    if deposit > 0.0 {
        return RedeemTheDeposit { refund: deposit };
    }

    // 2. **The shop takes the old one when it brings the new one.** No
    //    trip, no fee, no decision — which is why most large appliances
    //    never become a disposal problem at all.
    if c.being_replaced {
        return TradeIn {
            allowance: if c.still_works { 25.0 } else { 0.0 },
        };
    }

    // 3. **Reuse before recycling**, which is the waste hierarchy and is
    //    also simply true: a working radio is worth more to a person who
    //    wants one than to a smelter.
    if c.still_works && c.somebody_wants_it {
        return GiveItAway;
    }

    // 4. It is worth money at the yard — **and worth the bother of
    //    getting it there.** Twenty-four cents of scrap is not a reason to
    //    drive anywhere, so the value has to beat an hour of the time it
    //    takes.
    let at_the_yard = worth_as_scrap(c.name, c.materials, c.council.km, false);
    let worth_the_trip = at_the_yard > (c.hourly_worth * 0.5).max(5.0);
    if worth_the_trip && (!c.bulky || c.carrying.takes_bulky()) {
        return SellIt { paid: at_the_yard };
    }
    if c.charity_nearby && a_charity_would_take_it(c.name, c.still_works) {
        return Donate;
    }

    // 5. **Put it on the pavement and it goes.** Free, and it only works
    //    for things with metal in them, because it is the scrappers who
    //    take it.
    if c.scrappers_about && at_the_yard > -5.0 && !c.bulky {
        return LeaveItOut;
    }
    if c.scrappers_about && at_the_yard > 0.0 {
        return LeaveItOut;
    }

    // 6. The town takes it, if it still owes them a collection.
    if c.collections_used < c.council.free_collections_a_year {
        return Kerbside;
    }

    // Now it costs something, and the options are weighed.
    let (trip_cost, trip_hours) = cost_of_a_trip(
        c.kg,
        c.council,
        special_handling(c.name),
        c.others_going + 1,
    );
    let can_shift = c.kg <= c.carrying.payload_kg() && (!c.bulky || c.carrying.takes_bulky());

    let mut options: Vec<(HowToGetRidOfIt, f64)> = Vec::new();
    let hauler = PayAHauler {
        fee: hauler_fee(c.kg, c.council.km) + special_handling(c.name),
    };
    options.push((hauler, hauler.cost()));
    let paid_collection = PayAHauler {
        fee: c.council.per_item,
    };
    options.push((paid_collection, paid_collection.cost()));
    if can_shift {
        let mine = TakeItYourself {
            cost: trip_cost,
            hours: trip_hours,
        };
        // **Whose afternoon it is matters.** The same trip is cheap for
        // somebody with time and dear for somebody without.
        options.push((mine, mine.cost() + trip_hours * c.hourly_worth));
    }
    // **Keeping it is a real answer and mostly a free one**, which is
    // exactly why lofts and garages are full. It defers rather than
    // decides, and it costs whatever the space is worth.
    if c.room_to_store {
        options.push((KeepIt, 6.0));
    }
    // **Cannibalising is not an alternative to disposal**, and putting it
    // in this list made it a free escape from every decision here: strip
    // the motor out and you still have the carcass in the yard. It is a
    // thing you do *before* getting rid of what is left, so it belongs to
    // whoever is deciding what to keep rather than to this choice.

    let best = options
        .iter()
        .copied()
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(o, _)| o)
        .unwrap_or(hauler);

    // 7. **And the ways round it.** Doing it properly costs money and
    //    burning it or leaving it in the woods does not. What deters is
    //    being seen — the rule `custom.rs` already carries, that certainty
    //    deters and severity mostly does not — and **the distance to the
    //    tip is itself a cause**, which is why rural fly-tipping is worse.
    //
    //    The gain is relative to the person: sixty dollars is nothing to
    //    somebody comfortable and two days' work to somebody who is not,
    //    and it is the second one who leaves it in a ditch.
    let a_days_earnings = (c.hourly_worth * 8.0).max(1.0);
    let money = (best.cost() / a_days_earnings).clamp(0.0, 1.0);
    let bother = match best {
        TakeItYourself { hours, .. } => (hours / 4.0).clamp(0.0, 0.5),
        _ => 0.0,
    };
    let gain = (money + bother).clamp(0.0, 1.0);
    let tempted = crate::custom::will_bend(
        (c.scruple.clamp(-1.0, 1.0) * 50.0) as i8,
        c.scruple.clamp(-2.0, 2.0),
        gain,
        c.council.chance_seen,
    );
    // **A propensity is not a decision.** `will_bend` says how likely such
    // a person is to do it; a keyed draw says whether this one did, so the
    // same man on the same day always answers the same way and reloading a
    // save cannot make him a fly-tipper.
    let u = crate::rng::Rng::new(crate::save::channel(
        c.event,
        c.kg as u64,
        "getting round it",
    ))
    .next_f32() as f64;
    if u < tempted {
        // **Which corner is cut depends on what is to hand.** A bonfire in
        // a yard nobody overlooks is easier than a drive to the woods, and
        // it is what happens to anything that burns.
        let burns = c
            .materials
            .iter()
            .any(|&(m, t)| t > 0.0 && matches!(m.recovers(), Recovers::Fuel));
        if c.somewhere_to_burn && burns && !c.bulky {
            return BurnIt;
        }
        return FlyTip;
    }
    best
}

// =====================================================================
// what a shredder does to a car
// =====================================================================

/// **A car is about 95% recovered by weight, and the rest is real.**
///
/// Real end-of-life vehicle figures: metals are 70-75% of a car and come
/// back almost entirely, and **automotive shredder residue** — the fluff of
/// foam, fabric, glass and mixed plastic left after the magnets have taken
/// everything worth having — is 20-25% of the mass and goes to landfill
/// almost everywhere.
pub fn shred(load: &Load) -> (Vec<(Material, f64)>, f64) {
    let mut out: Vec<(Material, f64)> = Vec::new();
    let mut fluff = 0.0;
    for &(m, t) in &load.materials {
        // A shredder and a magnet are extremely good at metal and no good
        // at all at anything else.
        let recovered = match m.recovers() {
            Recovers::Feedstock if price_a_tonne(m) > 300.0 => 0.97,
            Recovers::Feedstock => 0.85,
            Recovers::Downcycled => 0.30,
            _ => 0.0,
        };
        if recovered > 0.0 {
            match out.iter_mut().find(|o| o.0 == m) {
                Some(o) => o.1 += t * recovered,
                None => out.push((m, t * recovered)),
            }
        }
        fluff += t * (1.0 - recovered);
    }
    (out, fluff)
}
