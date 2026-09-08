//! **A cargo is somewhere, and it takes time to get anywhere.**
//!
//! Freight was instantaneous: one `Event::Shipped` took tonnes off a shed
//! in one town and put them on a shelf in another in the same statement.
//! That is a fair abstraction for a lorry going across town and a poor one
//! for six hundred miles, and it makes four ordinary things inexpressible.
//!
//! - **Goods on the road are still somebody's.** They have left the seller
//!   and not reached the buyer, and if the model has nowhere to put them
//!   they either stop existing for a day or they exist twice. Which is why
//!   in-transit tonnage is inside the conservation check rather than
//!   beside it.
//! - **A contract is struck before it is performed.** What was agreed on
//!   Monday is what is paid on Thursday, whatever the price did in
//!   between — which is most of what a forward price *is*, and cannot be
//!   said at all while buying and delivering are one instruction.
//! - **A cargo can be lost.** Perishables rot on a lorry exactly as they
//!   rot in a shed, and a load that does not arrive is an ordinary thing
//!   that happens to real hauliers.
//! - **And it can arrive at a full shed.** Between the lorry leaving and
//!   the lorry arriving, somebody may have filled the space it was going
//!   into. The same rule `schedule.rs` had to learn about finished work:
//!   room is checked on arrival, not only at planning.
//!
//! The first real consumer of `registry.rs`, which is why that exists: a
//! consignment has to keep one name across a save, must never be confused
//! with the site it left or the market it is bound for, and has to leave a
//! tombstone — because the journal goes on naming it long after it has
//! been tipped.

use crate::econ::Commodity;
use crate::registry::Key;
use crate::save::{Reader, SaveError, Store, Writer};

/// **What a particular consignment is called.** Not an index into
/// anything, and not the same type as a site, a market or a carrier.
pub type ShipmentId = Key<Shipment>;

/// **A legal day, not a theoretical one.** Nine hours is the driving
/// maximum; seven is a working day once loading, queueing, town speeds and
/// mandatory breaks come out. A real artic covers 550-700 km.
pub const KM_PER_DAY: f64 = 620.0;

/// How many nights a haul spends on the road.
///
/// **Nought is the ordinary answer, and that is not a degenerate case.**
/// The average British road haul is about 94 km, which a lorry does and
/// comes home from inside a day — so most freight really is same-day, and
/// a model that made every delivery an overnight saga would be wrong about
/// the common case in order to be right about the rare one. What the
/// distance decides is whether the load sleeps somewhere.
pub fn days_on_the_road(km: f64) -> u64 {
    if !km.is_finite() || km <= 0.0 {
        return 0;
    }
    (km / KM_PER_DAY).floor() as u64
}

/// Where a consignment has got to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leg {
    /// On its way, and due on the day it is due.
    OnTheRoad,
    /// Arrived and could not be tipped, because the shed it was going into
    /// is full. Real, common and expensive: a lorry standing at a bay is a
    /// lorry not earning, which is exactly why demurrage is charged for it.
    Waiting,
    /// Tipped — in full, or in whatever was left of it.
    Delivered,
    /// Nothing arrived and nothing is coming.
    WrittenOff,
}

/// Why some of a consignment did not arrive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loss {
    /// It went off on the way. **The only cause wired**, because it is the
    /// only one this model has a rate for that it did not make up: a
    /// perishable on a lorry spoils at the rate it spoils at, and whether
    /// the vehicle keeps it cold is the whole difference.
    Spoiled,
    /// Damaged in handling, or in a wreck.
    Damaged,
    /// Stolen. Real, and in tonnage terms very small.
    Stolen,
}

/// **One consignment**: these goods, from this seller, to this buyer, by
/// this carrier, at this price, due on this day.
#[derive(Clone, Debug, PartialEq)]
pub struct Shipment {
    pub commodity: Commodity,

    // --- who ---
    /// The site it was collected from.
    pub consignor: usize,
    /// The site it is for. **Named at despatch**, because a haulier
    /// delivers to a consignee and not to whatever shelf has room — the
    /// rule `logistics.rs` learned when a town's food ended up in a
    /// cannery's output store and people went hungry with the books
    /// balanced.
    pub consignee: usize,
    /// Which carrier has it. They are paid on delivery.
    pub carrier: usize,
    pub from_market: usize,
    pub to_market: usize,

    // --- when ---
    pub left: u64,
    /// The day it is expected. Not a promise: a full shed keeps it
    /// waiting.
    pub due: u64,

    // --- what ---
    /// What set off.
    pub despatched: f64,
    /// What is still aboard.
    pub aboard: f64,
    /// What has been tipped so far — a part load goes into what room there
    /// is and the rest waits for tomorrow.
    pub delivered: f64,
    /// What did not survive the journey, and how.
    pub lost: f64,
    pub how_lost: Option<Loss>,

    // --- money ---
    /// **The contract price of the whole consignment, struck at
    /// despatch.** What the market does to the price while the lorry is
    /// moving is not the buyer's problem and not the seller's windfall.
    pub goods: f64,
    /// What the haul was contracted at.
    pub freight: f64,
    /// Whether the vehicle keeps it cold, which for a perishable decides
    /// whether there is anything at the far end.
    pub refrigerated: bool,

    pub leg: Leg,
}

impl Shipment {
    /// What a tonne of this cost delivered — the goods plus the carriage,
    /// which between them are what the consignee's works actually pays for
    /// its input.
    pub fn landed_per_tonne(&self) -> f64 {
        if self.despatched <= 1e-12 {
            return 0.0;
        }
        (self.goods + self.freight) / self.despatched
    }

    /// The share of the contract that applies to a part tipped.
    pub fn share(&self, tonnes: f64) -> (f64, f64) {
        if self.despatched <= 1e-12 {
            return (0.0, 0.0);
        }
        let f = (tonnes / self.despatched).clamp(0.0, 1.0);
        (self.goods * f, self.freight * f)
    }

    pub fn in_transit(&self) -> bool {
        matches!(self.leg, Leg::OnTheRoad | Leg::Waiting)
    }

    /// **What rots on the way.**
    ///
    /// A lorry is a store like any other, and the model already knows how
    /// fast each commodity leaks out of one, so nothing is invented here.
    /// What a journey adds is that the clock runs while nobody can do
    /// anything about it.
    pub fn spoilage_today(&self) -> f64 {
        self.aboard * self.commodity.spoilage_per_day(self.refrigerated)
    }
}

// ---------------------------------------------------------------------
// writing one down
// ---------------------------------------------------------------------

impl Store for Leg {
    fn store(&self, w: &mut Writer) {
        // **An explicit code, not a position.** Inserting a variant
        // tomorrow must not silently reinterpret every save made today.
        w.u8(match self {
            Leg::OnTheRoad => 1,
            Leg::Waiting => 2,
            Leg::Delivered => 3,
            Leg::WrittenOff => 4,
        });
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(match r.u8()? {
            1 => Leg::OnTheRoad,
            2 => Leg::Waiting,
            3 => Leg::Delivered,
            4 => Leg::WrittenOff,
            n => return Err(SaveError::UnknownCode("shipment leg", n as u32)),
        })
    }
}

impl Store for Loss {
    fn store(&self, w: &mut Writer) {
        w.u8(match self {
            Loss::Spoiled => 1,
            Loss::Damaged => 2,
            Loss::Stolen => 3,
        });
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(match r.u8()? {
            1 => Loss::Spoiled,
            2 => Loss::Damaged,
            3 => Loss::Stolen,
            n => return Err(SaveError::UnknownCode("shipment loss", n as u32)),
        })
    }
}

impl Store for Shipment {
    fn store(&self, w: &mut Writer) {
        w.u16(self.commodity.wire_code());
        w.len(self.consignor);
        w.len(self.consignee);
        w.len(self.carrier);
        w.len(self.from_market);
        w.len(self.to_market);
        w.u64(self.left);
        w.u64(self.due);
        w.f64(self.despatched);
        w.f64(self.aboard);
        w.f64(self.delivered);
        w.f64(self.lost);
        match self.how_lost {
            None => w.u8(0),
            Some(l) => {
                w.u8(1);
                l.store(w);
            }
        }
        w.f64(self.goods);
        w.f64(self.freight);
        w.bool(self.refrigerated);
        self.leg.store(w);
    }

    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let code = r.u16()?;
        let commodity = Commodity::from_wire_code(code)
            .ok_or(SaveError::UnknownCode("commodity", code as u32))?;
        let s = Shipment {
            commodity,
            consignor: r.read_len()?,
            consignee: r.read_len()?,
            carrier: r.read_len()?,
            from_market: r.read_len()?,
            to_market: r.read_len()?,
            left: r.u64()?,
            due: r.u64()?,
            despatched: r.finite_f64()?,
            aboard: r.finite_f64()?,
            delivered: r.finite_f64()?,
            lost: r.finite_f64()?,
            how_lost: match r.u8()? {
                0 => None,
                1 => Some(Loss::load(r)?),
                n => return Err(SaveError::UnknownCode("shipment loss tag", n as u32)),
            },
            goods: r.finite_f64()?,
            freight: r.finite_f64()?,
            refrigerated: r.bool()?,
            leg: Leg::load(r)?,
        };
        s.check()?;
        Ok(s)
    }
}

impl Shipment {
    /// **What a manifest cannot say and still be a manifest.**
    ///
    /// Every one of these decodes cleanly: a finite float in a known
    /// field, a valid `Leg` code, a length inside its bound. Nothing in
    /// the codec can catch them, and letting one through puts a
    /// contradiction *inside* the conservation assertion that is this
    /// project's only defence against a quiet leak — `Ledger::total`
    /// counts `aboard`, so a load whose parts do not add up to what was
    /// despatched makes tonnage appear or vanish and every subsequent
    /// check passes.
    ///
    /// A save is not a trusted input. It has been on a disk, through a
    /// backup, possibly through somebody's editor.
    fn check(&self) -> Result<(), SaveError> {
        // **A negative tonnage is not a small one.** `finite_f64` refuses
        // a NaN and accepts -1e9 quite happily.
        for q in [self.despatched, self.aboard, self.delivered, self.lost] {
            if q < 0.0 {
                return Err(SaveError::Impossible("shipment holds a negative tonnage"));
            }
        }
        if self.goods < 0.0 || self.freight < 0.0 {
            return Err(SaveError::Impossible("shipment has a negative value"));
        }
        // **Nothing arrives before it leaves.**
        if self.due < self.left {
            return Err(SaveError::Impossible("shipment is due before it set off"));
        }
        // **The manifest has to reconcile.** What is still aboard, what
        // was tipped and what did not survive are the whole of what set
        // off, and this is the invariant the ledger's own arithmetic
        // rests on. The tolerance is relative, because these are tonnages
        // that have been through a weighted average.
        let parts = self.aboard + self.delivered + self.lost;
        if (parts - self.despatched).abs() > 1e-6 * self.despatched.max(1.0) {
            return Err(SaveError::Impossible(
                "shipment manifest does not add up to what was despatched",
            ));
        }
        // **A leg and a state cannot contradict each other.** A cargo
        // still on the road with nothing aboard is not on the road; one
        // written off that arrived was not written off; and a loss needs
        // a reason, which is the pair `how_lost` exists to carry.
        match self.leg {
            Leg::OnTheRoad | Leg::Waiting => {
                if self.aboard <= 0.0 {
                    return Err(SaveError::Impossible(
                        "shipment in transit with an empty hold",
                    ));
                }
            }
            Leg::Delivered | Leg::WrittenOff => {
                if self.aboard > 1e-9 {
                    return Err(SaveError::Impossible(
                        "finished shipment still holding cargo",
                    ));
                }
            }
        }
        if self.leg == Leg::WrittenOff && self.delivered > 1e-9 {
            return Err(SaveError::Impossible(
                "a written-off shipment that delivered",
            ));
        }
        if (self.lost > 1e-9) != self.how_lost.is_some() {
            return Err(SaveError::Impossible(
                "shipment loss and its cause disagree about whether anything was lost",
            ));
        }
        Ok(())
    }
}
