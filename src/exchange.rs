//! **What this world's money is worth outside it, and who pays for the
//! difference.**
//!
//! The border could buy and sell, and did both at a fixed price:
//! `Commodity::world_price` is the reference cost, and every import and
//! every export settled against it whatever the balance between them came
//! to. So a world that bought three times what it sold went on doing it for
//! ever, and the money left. Measured over seven hundred days on one
//! planet: **5.06e10 paid out for imports against 1.80e10 taken in for
//! exports**, `Account::Abroad` up 3.26e10, and households drained from
//! 4.53e10 to 2.44e10. Nearly the whole of the domestic money loss was the
//! trade deficit, and nothing in the model could answer it.
//!
//! ## A deficit is not a disequilibrium
//!
//! The first version of this made the rate chase the balance to zero, and
//! that is wrong about the world. **Real economies run trade deficits for
//! decades.** The United States has run a current-account deficit every
//! year since 1976 — $918bn on goods and services in 2024 against exports
//! of $3,192bn and imports of $4,110bn — and the United Kingdom every year
//! since 1984. China has run the mirror image for a generation: $3,577bn
//! out against $2,585bn in, a surplus of $992bn.
//!
//! What makes that possible is the identity a model with only a current
//! account cannot satisfy:
//!
//! ```text
//! current account + capital account = 0
//! ```
//!
//! A country buying more than it sells is, *by construction*, selling
//! claims on itself for the difference — bonds, shares, property, direct
//! investment, or simply the supplier's agreement to be paid later.
//! Foreign holdings of US Treasury securities alone are about **$8.5
//! trillion**, and the accumulated counterpart of fifty years of deficits
//! is a net international investment position near **−$26 trillion**.
//!
//! So a current account with no capital account beside it is not a
//! simplification — it is an impossibility. It says money leaves and
//! nothing brings it back, and the only end state is a country with no
//! money, which is exactly what this model was producing.
//!
//! ## One rate, and the reason is the code rather than the world
//!
//! `region::Nations` folds every nation into **one ledger and one
//! treasury**, which is what makes conservation mean anything across a
//! border — and one money is one currency. So what floats here is one rate
//! against everything outside, not one per nation.
//!
//! **This was written up as a currency union and it is not one.** A union
//! is monetary union *without* fiscal union — one currency, twenty
//! governments, twenty tax systems, and no way to transfer from the
//! surplus members to the deficit ones, which is the whole reason Greece
//! could not be devalued out of its trouble. This model has the opposite:
//! one treasury, one `Account::State`, one `Government`, one tax rate and
//! one public payroll for the whole planet, because `Economy::government`
//! and `Economy::services` are each a single `Option`.
//!
//! **What it behaves like is one country with several regions**, which is
//! a coherent thing to be: one currency, one exchequer, internal free
//! trade, and provinces differing in what the ground under them will grow
//! and dig. What is genuinely per-nation is short — the roads and their
//! upkeep, the season, whether the territory reaches the sea, and a duty
//! field that is empty in every world generated so far. So the rate is
//! that one country's rate against the rest of the world, and it is only
//! the word "nations" that was carrying more than the code does.
//!
//! A per-nation currency is a **named gap** and a large one: not an FX
//! rate per nation but a second money, a conversion on every cross-border
//! payment, a conservation rule spanning both, and a state and a tax
//! system per nation before any of it means anything.
//!
//! ## What is modelled is the real rate, not the market
//!
//! Worth stating plainly, because the speed depends on it. Daily foreign
//! exchange turnover is about **$7.5 trillion** *(BIS Triennial Survey,
//! 2022)* against world merchandise trade of roughly $24 trillion a
//! *year* — about $66 billion a day — so **trade is on the order of one
//! per cent of what moves a nominal rate** and the rest is capital. This
//! model has no portfolios, so it cannot pretend to a nominal FX market.
//!
//! What it can do honestly is the **real** rate adjusting to a balance
//! nobody will fund. Three real figures bracket it:
//!
//! - the **J-curve**: a depreciation makes the deficit *worse* first,
//!   because each imported tonne now costs more before any volume
//!   responds, and turns after something like six to twelve months. This
//!   model shows it: over the first 700 days the money paid abroad *rose*
//!   from 5.06e10 to 6.29e10 while exports rose faster in proportion, and
//!   the drain only slowed afterwards;
//! - **PPP reversion**: deviations from purchasing power parity have a
//!   half-life of **three to five years** *(Rogoff, remarkably consistent
//!   across studies)*;
//! - the **Marshall-Lerner condition**: a depreciation improves the
//!   balance only if import and export demand together respond more than
//!   one for one. Here they do, because both decisions are a price against
//!   a parity that moves with the rate, so a town crosses out of importing
//!   and into exporting as it goes.

/// **The price of the outside world's money, in this world's — and the
/// claim the world has built up on this one.**
///
/// One at par, above one after a depreciation. Everything the border
/// settles against the world is the world price times this, so the whole
/// band scales together and **the band still cannot close**: one side adds
/// the costs and the other takes them away, whatever the rate.
#[derive(Debug, Clone, PartialEq)]
pub struct Exchange {
    /// What one unit of foreign money costs here.
    foreign_money: f64,
    /// Money paid out to the world on trade, smoothed.
    out: f64,
    /// Money taken in from the world on trade, smoothed.
    into: f64,
    /// **What the world is owed, net.** Positive means the outside world
    /// holds claims on this one — which is what fifty years of deficit
    /// accumulates into, and the honest cost of a deficit that today felt
    /// free.
    owed_abroad: f64,
}

impl Default for Exchange {
    fn default() -> Self {
        Self::at_par()
    }
}

impl Exchange {
    /// **How long the flow average remembers.** Real trade figures arrive
    /// monthly and quarterly, and a rate responds to the balance rather
    /// than to one morning's cargo. A designed figure, labelled as one.
    const REMEMBERS_DAYS: f64 = 90.0;

    /// **How large an imbalance the world will fund indefinitely**, on the
    /// measure below — the net flow over the gross, so +1 is all imports
    /// and −1 all exports.
    ///
    /// The two most persistent imbalances on earth sit at about this
    /// figure and have for decades: the United States at **+0.126** and
    /// China at **−0.161** on 2024 trade. Neither has been forced to close
    /// and neither has collapsed, because in both cases somebody is
    /// willing to hold the other side.
    ///
    /// **This is a trade-relative figure, and the familiar warning lines
    /// are not.** "A current-account deficit over 5% of GDP is a danger
    /// sign" is a different denominator — trade is about a quarter of US
    /// GDP, so +0.126 of trade is about 3.5% of GDP. Two shares on
    /// different bases do not compare, which is the mistake `census.rs`
    /// exists to stop.
    const FUNDED_IMBALANCE: f64 = 0.15;

    /// **How far the rate moves in a day per unit of unfunded imbalance.**
    ///
    /// At a whole unfunded unit this is about **95% a year**, which is
    /// crisis speed and is what a crisis does: the rouble halved in 2014,
    /// the Turkish lira lost some 85% against the dollar between 2018 and
    /// 2023, and Argentina has done far worse more than once. A designed
    /// coefficient; what anchors it is the pair of real times above — fast
    /// enough that a balance turns inside a year or two, slow enough that
    /// it is not a nominal market tick.
    const A_DAY_PER_UNIT: f64 = 0.0018;

    /// **Bounds, because a runaway is not an adjustment.** A factor of five
    /// on every import price is a currency collapse and those happen; a
    /// factor of a hundred is arithmetic running away, and the difference
    /// matters because every price at the border is built on this.
    const DEAREST: f64 = 5.0;
    const CHEAPEST: f64 = 0.2;

    /// At par: the outside world's money and this world's are worth the
    /// same, nobody owes anybody, and no trade has happened yet.
    pub fn at_par() -> Self {
        Self {
            foreign_money: 1.0,
            out: 0.0,
            into: 0.0,
            owed_abroad: 0.0,
        }
    }

    /// **At a stated rate**, with no history behind it. For anything that
    /// needs to ask what the border would do at a particular rate rather
    /// than wait for one to arrive.
    pub fn at(foreign_money: f64) -> Self {
        Self {
            foreign_money,
            ..Self::at_par()
        }
    }

    /// Loader-only. `Economy`'s codec is the one caller.
    pub fn restore(foreign_money: f64, out: f64, into: f64, owed_abroad: f64) -> Self {
        Self {
            foreign_money,
            out,
            into,
            owed_abroad,
        }
    }

    /// What one unit of foreign money costs here — the figure every price
    /// at the border is multiplied by.
    pub fn foreign_money(&self) -> f64 {
        self.foreign_money
    }

    /// The smoothed trade flows: out, in.
    pub fn flows(&self) -> (f64, f64) {
        (self.out, self.into)
    }

    /// **What the outside world holds against this one.** The stock a
    /// persistent deficit builds, and the thing that makes it cost
    /// something later rather than today.
    pub fn owed_abroad(&self) -> f64 {
        self.owed_abroad
    }

    /// **Where the balance stands**, between −1 (everything sold, nothing
    /// bought) and +1 (everything bought, nothing sold). Zero when no
    /// trade has happened, which is the honest answer and not a signal.
    pub fn imbalance(&self) -> f64 {
        let turnover = self.out + self.into;
        if turnover <= 1e-9 {
            0.0
        } else {
            (self.out - self.into) / turnover
        }
    }

    /// **The part nobody will fund**, which is the only part the rate
    /// answers. An imbalance inside `FUNDED_IMBALANCE` moves nothing,
    /// because in life it does not.
    pub fn unfunded(&self) -> f64 {
        let b = self.imbalance();
        (b.abs() - Self::FUNDED_IMBALANCE).max(0.0) * b.signum()
    }

    /// **What share of today's imbalance the world is willing to carry as
    /// a claim rather than settle in money.**
    ///
    /// One at an ordinary deficit and falling as it grows, because what is
    /// funded is a position somebody wants to hold and there is a limit to
    /// how much of it anybody wants. A **sudden stop** is this share going
    /// to nothing — Thailand in 1997, Argentina in 2001, Greece in 2010 —
    /// and what follows is the rate, or, where the country cannot devalue,
    /// an internal devaluation instead.
    pub fn funded_share(&self) -> f64 {
        let b = self.imbalance().abs();
        if b <= Self::FUNDED_IMBALANCE {
            1.0
        } else {
            Self::FUNDED_IMBALANCE / b
        }
    }

    /// **A day's trade with the world, and what it does to the rate.**
    ///
    /// Called once at the close, so tomorrow's decisions read a settled
    /// figure — the same discipline as the opening photograph. `paid_out`
    /// is what crossed the border to the world on trade today and
    /// `taken_in` is what came back; the capital account is **not** in
    /// either, or the thing being measured would include its own answer.
    ///
    /// Returns the net capital flow to record: positive means the world
    /// acquires that much claim on this economy and the money comes back
    /// in; negative means this economy acquires claims abroad and the
    /// money goes out. The caller moves it, because moving money is the
    /// treasury's business and not this type's.
    pub fn settle(&mut self, paid_out: f64, taken_in: f64) -> f64 {
        let a = 1.0 - 0.5f64.powf(1.0 / Self::REMEMBERS_DAYS);
        self.out += (paid_out - self.out) * a;
        self.into += (taken_in - self.into) * a;

        let move_by = self.unfunded() * Self::A_DAY_PER_UNIT;
        self.foreign_money =
            (self.foreign_money * (1.0 + move_by)).clamp(Self::CHEAPEST, Self::DEAREST);

        let net = (paid_out - taken_in) * self.funded_share();
        self.owed_abroad += net;
        net
    }
}
