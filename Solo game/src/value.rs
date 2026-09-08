//! **Nine numbers that had been two.**
//!
//! Every figure here is money per tonne, so every one of them is an `f64`
//! and the machine cannot tell them apart. They are not the same quantity
//! and they do not answer the same question, and collapsing them is what
//! produced the worst defect this project has measured:
//!
//! ```text
//! price = landed x scarcity,  landed = goods + freight
//!   =>  price_b - price_a = freight x m
//!   =>  arbitrage         = freight x (m - 1)
//! ```
//!
//! — so the moment anything was scarce anywhere, **every remote market
//! showed a false arbitrage of exactly `freight x (m-1)`** and pairwise
//! trade chased it. A town made to look dear by the carriage that had
//! already got its goods there.
//!
//! Writing that as `landed * multiplier` is one obvious line. So the
//! purpose of this module is not to hold the numbers — `f64` did that
//! perfectly well — but to make that line **fail to compile**.
//!
//! ### What is what
//!
//! | | question it answers |
//! |---|---|
//! | [`ProductionCost`] | what does it cost to *make* here, ex works |
//! | [`InventoryBasis`] | what did the stock on hand cost — historical |
//! | [`SupplierAsk`] | what is a seller asking for it |
//! | [`PurchasePrice`] | what was actually agreed |
//! | [`InboundCharges`] | freight, duty and handling to get it here |
//! | [`LandedBasis`] | purchase plus inbound, per tonne held |
//! | [`ReplacementQuote`] | what would the *next* tonne cost, now |
//! | [`ScarcityPremium`] | what shortage adds on top |
//! | [`ClearingPrice`] | what it actually changes hands at |
//!
//! **A works consumes the [`InventoryBasis`] of what is in its yard.**
//! That is an accounting fact about the past. **Anybody deciding whether
//! to move goods needs the [`ReplacementQuote`]** — what obtaining another
//! tonne would cost today. Using the first where the second belongs is the
//! error underneath the whole business.
//!
//! ### The legal arithmetic, and there is not much of it
//!
//! ```text
//! ProductionCost  + InboundCharges  -> ReplacementQuote
//! PurchasePrice   + InboundCharges  -> LandedBasis
//! LandedBasis     blended           -> InventoryBasis
//! ProductionCost  x Scarcity        -> ScarcityPremium
//! ReplacementQuote + ScarcityPremium -> ClearingPrice
//! ```
//!
//! Note what is missing: **there is no route from a landed cost and a
//! scarcity factor to a clearing price.** Scarcity multiplies the goods
//! and passes the carriage through, because a haulier's bill does not rise
//! when grain is short.

use std::fmt;

/// Builds a newtype over money-per-tonne with the operations that make
/// sense for it and none that do not.
macro_rules! per_tonne {
    ($name:ident, $what:literal) => {
        #[doc = $what]
        #[derive(Clone, Copy, PartialEq, PartialOrd, Default)]
        pub struct $name(f64);

        impl $name {
            pub const ZERO: $name = $name(0.0);

            pub fn new(v: f64) -> Self {
                $name(v)
            }

            /// The bare number, for arithmetic this module deliberately
            /// does not bless and for writing to a save. **Reaching for
            /// this to multiply two of these together is the bug.**
            pub fn get(self) -> f64 {
                self.0
            }

            pub fn is_finite(self) -> bool {
                self.0.is_finite()
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({:.2}/t)", stringify!($name), self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{:.2}/t", self.0)
            }
        }
    };
}

per_tonne!(
    ProductionCost,
    "**What it costs to make here, ex works.** No carriage in it and no \
     scarcity on it: a shortage of grain raises the price of grain and does \
     not make grain dearer to grow."
);
per_tonne!(
    InventoryBasis,
    "**What the stock on hand cost**, per tonne, as a weighted average of \
     what was actually paid for it. An accounting fact about the past, and \
     what a works consumes when it turns its yard into product. It is *not* \
     a trade signal — using a warehouse's history to decide where to send \
     goods is what let a town be made to look dear by the carriage that had \
     already got its goods there."
);
per_tonne!(
    SupplierAsk,
    "**What a seller is asking.** Not what it cost them and not what it \
     will fetch."
);
per_tonne!(
    PurchasePrice,
    "**What was actually agreed**, and fixed at that moment. What the \
     market does to the price while the lorry is moving is not the buyer's \
     problem and not the seller's windfall."
);
per_tonne!(
    LandedBasis,
    "**Purchase plus everything paid to get it here**, per tonne held. The \
     basis a consignment adds to the receiving market's inventory."
);
per_tonne!(
    ReplacementQuote,
    "**What the next tonne would cost, now.** The forward-looking figure, \
     and the one every decision to move goods has to be made on."
);
per_tonne!(
    ScarcityPremium,
    "**What shortage adds on top.** Charged on the goods and never on the \
     carriage."
);
per_tonne!(
    ClearingPrice,
    "**What it actually changes hands at.** A replacement quote plus a \
     scarcity premium, and nothing else."
);

/// **What it costs to get a tonne here**, broken out because the parts
/// behave differently and one of them is a pass-through.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct InboundCharges {
    /// Paid to whoever carried it.
    pub freight: f64,
    /// Paid at the border.
    pub tariff: f64,
    /// Loading, unloading, and the paperwork.
    pub handling: f64,
}

impl InboundCharges {
    pub const NONE: InboundCharges = InboundCharges {
        freight: 0.0,
        tariff: 0.0,
        handling: 0.0,
    };

    pub fn freight(freight: f64) -> Self {
        InboundCharges {
            freight,
            ..InboundCharges::NONE
        }
    }

    pub fn total(self) -> f64 {
        self.freight + self.tariff + self.handling
    }
}

/// **How short a market is**, as the factor its own goods are marked up
/// by.
///
/// One is neither short nor glutted. Deliberately a distinct type from
/// every price above: multiplying a *price* by this is the defect, and the
/// only thing it is allowed to be applied to is a `ProductionCost`.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Scarcity(f64);

impl Scarcity {
    pub const NONE: Scarcity = Scarcity(1.0);

    pub fn new(factor: f64) -> Self {
        Scarcity(factor)
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

// =====================================================================
// the legal arithmetic
// =====================================================================

impl ProductionCost {
    /// **What the next tonne would cost, delivered from here.**
    pub fn delivered(self, charges: InboundCharges) -> ReplacementQuote {
        ReplacementQuote(self.0 + charges.total())
    }

    /// **What shortage adds**, charged on the goods alone.
    ///
    /// This is the one place a scarcity factor may be applied, and the
    /// reason it takes a `ProductionCost` rather than anything with
    /// carriage in it: a haulier's bill does not rise when grain is short.
    pub fn scarcity_premium(self, s: Scarcity) -> ScarcityPremium {
        ScarcityPremium(self.0 * (s.0 - 1.0))
    }
}

impl PurchasePrice {
    /// **What a tonne of this cost once it was here.**
    pub fn landed(self, charges: InboundCharges) -> LandedBasis {
        LandedBasis(self.0 + charges.total())
    }
}

impl ReplacementQuote {
    /// **What it clears at.** The only way to build one.
    pub fn plus_premium(self, premium: ScarcityPremium) -> ClearingPrice {
        ClearingPrice(self.0 + premium.0)
    }
}

impl LandedBasis {
    /// **A delivery arrives and the average moves.**
    ///
    /// Weighted-average cost, which is one of the three inventory methods
    /// real accounting permits and the only one cheap enough to run per
    /// market per commodity per day.
    pub fn blend_into(self, held: InventoryBasis, had: f64, arriving: f64) -> InventoryBasis {
        let total = had + arriving;
        if total <= 1e-9 {
            return InventoryBasis(self.0);
        }
        InventoryBasis((held.0 * had + self.0 * arriving) / total)
    }
}

impl InventoryBasis {
    /// What a works pays for its inputs out of its own yard.
    pub fn as_input_cost(self) -> f64 {
        self.0
    }
}

// =====================================================================
// what the compiler refuses
// =====================================================================

/// **The defect, written out, must not compile.**
///
/// ```compile_fail
/// use scale_sim::value::{LandedBasis, Scarcity};
/// let landed = LandedBasis::new(945.0);
/// let short = Scarcity::new(2.0);
/// // `price = landed x scarcity` -- one obvious line, and the whole bug.
/// let _ = landed * short;
/// ```
///
/// The allowed sibling, reaching the same conclusion the right way round:
///
/// ```
/// use scale_sim::value::{InboundCharges, ProductionCost, Scarcity};
/// let goods = ProductionCost::new(900.0);
/// let carriage = InboundCharges::freight(45.0);
/// let price = goods
///     .delivered(carriage)
///     .plus_premium(goods.scarcity_premium(Scarcity::new(2.0)));
/// assert!((price.get() - 1845.0).abs() < 1e-9);
/// ```
///
/// **And a scarcity premium may not be taken on a delivered figure**,
/// which is the same error one step along:
///
/// ```compile_fail
/// use scale_sim::value::{InboundCharges, ProductionCost, Scarcity};
/// let quote = ProductionCost::new(900.0).delivered(InboundCharges::freight(45.0));
/// let _ = quote.scarcity_premium(Scarcity::new(2.0));
/// ```
///
/// **Nor may a warehouse's history stand in for a replacement quote**,
/// which is the deeper confusion the review named:
///
/// ```compile_fail
/// use scale_sim::value::{InventoryBasis, ScarcityPremium};
/// let basis = InventoryBasis::new(945.0);
/// let _ = basis.plus_premium(ScarcityPremium::new(10.0));
/// ```
///
/// The sibling that is allowed, so the refusals above are failing for the
/// reason claimed and not because of a typo:
///
/// ```
/// use scale_sim::value::{InventoryBasis, LandedBasis};
/// let held = InventoryBasis::new(900.0);
/// let arriving = LandedBasis::new(1000.0);
/// let blended = arriving.blend_into(held, 90.0, 10.0);
/// assert!((blended.get() - 910.0).abs() < 1e-9);
/// ```
pub const WHAT_THE_COMPILER_REFUSES: () = ();
