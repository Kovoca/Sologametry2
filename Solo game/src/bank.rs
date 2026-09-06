//! **A bank does not lend out deposits. Making a loan creates one.**
//!
//! This is the single most misunderstood mechanism in economics and it is
//! not a matter of opinion: the Bank of England published a paper saying so
//! plainly *(McLeay, Radia & Thomas, "Money creation in the modern
//! economy", 2014)*. The textbook story — savers deposit, banks lend the
//! money on, a reserve ratio multiplies it up — is **backwards**. What
//! actually happens is that a bank writes a loan on one side of its balance
//! sheet and a deposit on the other, both out of nothing, and the money
//! supply has just grown. Repayment destroys it again.
//!
//! ```text
//! lend 20,000:   loans +20,000  (asset)      deposits +20,000  (liability)
//! repay 500:     loans    -500               deposits    -500
//! interest 120:  deposits -120               capital     +120
//! ```
//!
//! The first two lines change the money supply. The third does not — it
//! moves money from the borrower to the bank, which is the bank's living
//! and is also why an indebted economy needs somebody to keep borrowing:
//! the interest was never created alongside the principal.
//!
//! **What limits lending is therefore not reserves.** It is capital
//! adequacy, liquidity, and — most days — whether anybody creditworthy
//! wants to borrow.
//!
//! Real shape, all US unless said:
//!
//! | | |
//! |---|---|
//! | currency in circulation | ~$2.3tn |
//! | broad money (M2) | ~$21tn |
//! | **share of money that is bank deposits** | **~89%** |
//! | Basel III total capital ratio | 8%, ~10.5% with buffers |
//! | leverage ratio | 3-5% |
//! | net interest margin | ~3% |
//! | FDIC insurance | $250,000 a depositor |

use crate::money::{Account, Treasury, Why};
use std::collections::BTreeMap;

// =====================================================================
// what money costs
// =====================================================================

/// **The rate ladder**, from what the central bank charges to what a man
/// with nothing pays.
///
/// The spread between the top and the bottom of this is the whole of
/// consumer finance, and it is enormous: real US figures put a policy rate
/// near 5%, a mortgage at 7%, a used car at 11.5%, a credit card at 21.5%
/// and **a payday loan at about 400% APR.** Somebody with no options pays
/// eighty times what the government does, which is most of how poverty
/// compounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rates {
    /// What the central bank charges banks. Everything else is quoted off
    /// it. Real US 2023-24: 5.25-5.50%, and near zero for most of the
    /// decade before.
    pub policy: f64,
    /// What a bank pays a depositor. Real: 0.5% at a big bank and 4.5% at
    /// one that has to compete, which is a large part of why deposits
    /// move.
    pub on_deposits: f64,
}

impl Rates {
    /// An ordinary modern setting.
    pub fn ordinary() -> Self {
        Rates { policy: 0.0525, on_deposits: 0.021 }
    }

    /// The decade of cheap money.
    pub fn cheap() -> Self {
        Rates { policy: 0.0025, on_deposits: 0.001 }
    }

    /// **What this borrower pays for this kind of credit.**
    ///
    /// The policy rate, plus what the product costs to run and lose money
    /// on, plus what this particular borrower looks like. Real spreads over
    /// the policy rate: a mortgage about 1.5-2 points, a new car 2, a used
    /// car 6, a personal loan 7, a credit card **16**.
    pub fn quoted(&self, kind: Credit, standing: f64) -> f64 {
        let spread = match kind {
            Credit::Mortgage => 0.017,
            // **The spread is what the best borrower pays over the policy
            // rate**, and it is small: real super-prime new-car finance is
            // 5.25% against a 5.25% policy rate, and used is 7.13%. All the
            // rest of the ladder is the risk loading below.
            Credit::CarLoan => 0.005,
            Credit::UsedCarLoan => 0.019,
            Credit::PersonalLoan => 0.070,
            Credit::CreditCard => 0.163,
            Credit::TradeCredit => 0.0,
            // **A payday lender is not on this ladder at all.** Real APRs
            // are around 400%, which is not a risk premium — it is a
            // two-week fee annualised, charged to people with nowhere else
            // to go.
            Credit::Payday => 3.95,
            Credit::Pawn => 1.95,
        };
        // **And the poor pay more, which is the trap in one line.** Real
        // subprime auto finance runs 15-21% against 7% for prime, so the
        // same car costs half as much again over the term.
        let risk = (1.0 - standing.clamp(0.0, 1.0)) * kind.risk_loading();
        self.policy + spread + risk
    }
}

/// The kinds of credit a household or a firm actually uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Credit {
    /// Secured on a house, 15-30 years. The largest debt most people ever
    /// take on.
    Mortgage,
    /// Secured on a new car, 5-7 years. **The missing piece**: real US
    /// transport is 17% of household expenditure and almost all of it is
    /// borrowed.
    CarLoan,
    /// The same, on a used one, and much dearer.
    UsedCarLoan,
    /// Unsecured, 3-7 years.
    PersonalLoan,
    /// Revolving and unsecured. Real US: ~21.5% and about half of accounts
    /// carry a balance.
    CreditCard,
    /// **Net 30.** One business letting another have the goods now and the
    /// money later, and by volume the largest source of short-term
    /// business finance there is.
    TradeCredit,
    /// Two weeks, secured on a wage packet, at an APR nobody would print
    /// on a poster.
    Payday,
    /// Secured on whatever was walked in with.
    Pawn,
}

impl Credit {
    pub fn name(self) -> &'static str {
        match self {
            Credit::Mortgage => "a mortgage",
            Credit::CarLoan => "a car loan",
            Credit::UsedCarLoan => "a used car loan",
            Credit::PersonalLoan => "a personal loan",
            Credit::CreditCard => "a credit card",
            Credit::TradeCredit => "trade credit",
            Credit::Payday => "a payday loan",
            Credit::Pawn => "a pawnbroker",
        }
    }

    /// The usual term, in months.
    pub fn typical_term_months(self) -> u32 {
        match self {
            Credit::Mortgage => 360,
            Credit::CarLoan => 72,
            Credit::UsedCarLoan => 60,
            Credit::PersonalLoan => 48,
            // Revolving: no term, and the minimum payment is what makes it
            // last for ever.
            Credit::CreditCard => 0,
            Credit::TradeCredit => 1,
            Credit::Payday => 1,
            Credit::Pawn => 4,
        }
    }

    /// **Whether the lender can take something back.** Which is what makes
    /// a mortgage cheap and a credit card dear, and it is nearly the whole
    /// of the difference.
    pub fn secured(self) -> bool {
        matches!(self, Credit::Mortgage | Credit::CarLoan | Credit::UsedCarLoan | Credit::Pawn)
    }

    /// **How much a poor credit record adds**, and it is not a small
    /// number. Smaller on a mortgage, because the lender can take the
    /// house; far larger on a car, because it depreciates faster than the
    /// loan does.
    ///
    /// Real, from the auto finance market by credit tier — the spread from
    /// the best borrower to the worst is ten points on a new car and
    /// **fourteen on a used one**:
    ///
    /// | | new | used |
    /// |---|---|---|
    /// | super prime | 5.25% | 7.13% |
    /// | prime | 6.87% | 9.36% |
    /// | nonprime | 9.83% | 13.92% |
    /// | subprime | 13.18% | 18.86% |
    /// | deep subprime | **15.43%** | **21.55%** |
    pub fn risk_loading(self) -> f64 {
        match self {
            Credit::Mortgage => 0.035,
            Credit::CarLoan => 0.115,
            Credit::UsedCarLoan => 0.165,
            Credit::PersonalLoan => 0.130,
            Credit::CreditCard => 0.110,
            Credit::TradeCredit => 0.040,
            Credit::Payday | Credit::Pawn => 0.0,
        }
    }

    /// **The most a lender will advance against the thing.** Real: an FHA
    /// mortgage goes to 96.5% of the value, a car loan to 100-120%
    /// because the dealer rolls the negative equity in, and a pawnbroker
    /// lends a quarter to a half of what the item is worth.
    pub fn max_loan_to_value(self) -> f64 {
        match self {
            Credit::Mortgage => 0.965,
            Credit::CarLoan => 1.10,
            Credit::UsedCarLoan => 1.00,
            Credit::Pawn => 0.35,
            _ => 0.0,
        }
    }
}

// =====================================================================
// what a loan is
// =====================================================================

/// **A debt, and the schedule it is paid on.**
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Loan {
    pub id: u64,
    pub kind: Credit,
    pub borrower: Account,
    pub bank: usize,
    /// What is still owed.
    pub outstanding: f64,
    /// What was advanced, kept because it is what the schedule was built
    /// on.
    pub advanced: f64,
    /// Annual, as a fraction.
    pub rate: f64,
    /// What falls due each month.
    pub monthly: f64,
    pub months_left: u32,
    /// Payments missed in a row. **Not a total**: catching up resets it,
    /// which is how forbearance works.
    pub missed: u32,
    /// What the lender can take, if anything.
    pub security: Option<u64>,
}

impl Loan {
    /// **The amortisation formula**, which is the real one and not an
    /// approximation: a level payment covering interest on the declining
    /// balance and retiring the principal by the end of the term.
    ///
    /// ```text
    /// M = P · r(1+r)^n / ((1+r)^n − 1)
    /// ```
    pub fn level_payment(principal: f64, annual_rate: f64, months: u32) -> f64 {
        if months == 0 {
            return 0.0;
        }
        let r = annual_rate / 12.0;
        if r <= 0.0 {
            return principal / months as f64;
        }
        let g = (1.0 + r).powi(months as i32);
        principal * r * g / (g - 1.0)
    }

    /// **What it costs altogether**, which is the number nobody looks at
    /// and the one that matters. Real: a 30-year mortgage at 7% pays back
    /// about 2.4 times what was borrowed.
    pub fn total_repayable(&self) -> f64 {
        self.monthly * self.months_left as f64
    }

    /// How much of this month's payment is interest rather than principal.
    /// **Early on it is nearly all interest**, which is why paying a
    /// mortgage for five years barely dents it.
    pub fn interest_this_month(&self) -> f64 {
        self.outstanding * self.rate / 12.0
    }

    pub fn in_arrears(&self) -> bool {
        self.missed > 0
    }

    /// **When a lender stops waiting.** Real: a mortgage goes to
    /// foreclosure after 120 days of missed payments by federal rule, a car
    /// can be repossessed after one missed payment in many states though
    /// lenders usually wait 60-90 days, and a credit card is charged off at
    /// 180.
    pub fn defaulted(&self) -> bool {
        let patience = match self.kind {
            Credit::Mortgage => 4,
            Credit::CarLoan | Credit::UsedCarLoan => 3,
            Credit::CreditCard => 6,
            Credit::PersonalLoan => 4,
            Credit::TradeCredit => 2,
            Credit::Payday | Credit::Pawn => 1,
        };
        self.missed >= patience
    }
}

// =====================================================================
// the balance sheet
// =====================================================================

/// **A bank, as a balance sheet.**
///
/// The identity `assets = liabilities + capital` is asserted rather than
/// assumed, for the same reason `econ::Ledger` asserts conservation: a
/// balance sheet that can quietly stop balancing is a bug you cannot find.
#[derive(Clone, Debug)]
pub struct Bank {
    pub id: usize,
    /// **Asset.** Money at the central bank. Used to settle with other
    /// banks, and *not* what lending is made out of.
    pub reserves: f64,
    /// **Asset.** What people owe it.
    pub loans: f64,
    /// **Liability.** What it owes its customers — and what almost all of
    /// the money in the economy actually is.
    pub deposits: f64,
    /// **Liability.** Reserves borrowed to settle with — from another bank
    /// overnight, or from the central bank as lender of last resort.
    ///
    /// A bank that cannot settle does not lose capital, it *borrows*, and
    /// getting that wrong breaks the balance sheet: an obligation to repay
    /// is a liability and not a reduction in the shareholders' stake. It is
    /// also the thing that makes a lending spree dangerous rather than
    /// free, and being unable to roll it over is what actually kills a
    /// bank.
    pub borrowed: f64,
    /// **Capital.** The shareholders' stake, and the thing that absorbs
    /// losses. It is what actually limits lending.
    pub capital: f64,
    /// What has been lent and will not come back.
    pub written_off: f64,
    /// Cumulative interest earned, kept apart so the diagnostic can show a
    /// bank's living rather than inferring it.
    pub interest_earned: f64,
    pub interest_paid: f64,
}

impl Bank {
    pub fn new(id: usize, capital: f64, reserves: f64) -> Self {
        // Capital subscribed in cash: the shareholders' money arrives as
        // reserves, so both sides start equal.
        Bank {
            id,
            reserves: reserves + capital,
            loans: 0.0,
            deposits: reserves,
            borrowed: 0.0,
            capital,
            written_off: 0.0,
            interest_earned: 0.0,
            interest_paid: 0.0,
        }
    }

    pub fn assets(&self) -> f64 {
        self.reserves + self.loans
    }

    /// **The identity.** Everything the bank owns is owed to somebody:
    /// depositors and whoever it borrowed from first, and shareholders with
    /// whatever is left.
    pub fn balances(&self) -> bool {
        (self.assets() - self.deposits - self.borrowed - self.capital).abs() < 1e-6
    }

    pub fn assert_balanced(&self) {
        assert!(
            self.balances(),
            "bank {} does not balance: assets {:.2} against deposits {:.2}              + borrowed {:.2} + capital {:.2}",
            self.id,
            self.assets(),
            self.deposits,
            self.borrowed,
            self.capital
        );
    }

    /// **Capital as a share of what has been lent**, which is the ratio
    /// Basel III is written in. Real requirement: 8% total, and about
    /// 10.5% once the conservation buffer is counted.
    pub fn capital_ratio(&self) -> f64 {
        if self.loans <= 0.0 {
            return 1.0;
        }
        self.capital / self.loans
    }

    /// **The leverage ratio**, against everything rather than against
    /// risk-weighted assets — the backstop that exists because
    /// risk-weighting can be gamed and was. Real: 3% in the EU, 5% for
    /// large US banks.
    pub fn leverage_ratio(&self) -> f64 {
        if self.assets() <= 0.0 {
            return 1.0;
        }
        self.capital / self.assets()
    }

    /// **Insolvent** when the losses have eaten the shareholders.
    pub fn insolvent(&self) -> bool {
        self.capital <= 0.0
    }

    /// **Illiquid before insolvent**, which is how banks actually fail. A
    /// bank with perfectly sound loans and no reserves cannot meet its
    /// withdrawals, and that is what a run is — Silicon Valley Bank lost
    /// $42bn in a single day in 2023 and its loan book was not the problem.
    ///
    /// The threshold is an operational one rather than a statutory ratio:
    /// Basel III's liquidity coverage requirement is written against thirty
    /// days of stressed outflows, which for a retail bank works out at
    /// something like a tenth of deposits.
    pub fn illiquid(&self) -> bool {
        self.reserves < self.deposits * 0.10
    }
}

/// **What limits lending**, and it is not a reserve multiplier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The bank would be below its capital requirement.
    NotEnoughCapital,
    /// It could not settle if the money were spent elsewhere.
    NotEnoughLiquidity,
    /// The borrower cannot service it out of income.
    CannotAfford,
    /// Too much lent against too little security.
    NotEnoughSecurity,
    /// The record is too poor at any price.
    NoCreditworthiness,
    /// Nobody offers this here.
    NotOffered,
}

impl Refused {
    pub fn name(self) -> &'static str {
        match self {
            Refused::NotEnoughCapital => "the bank is at its capital limit",
            Refused::NotEnoughLiquidity => "the bank is short of reserves",
            Refused::CannotAfford => "they could not keep up the payments",
            Refused::NotEnoughSecurity => "not enough of a deposit",
            Refused::NoCreditworthiness => "the record is too poor",
            Refused::NotOffered => "nobody here lends for that",
        }
    }
}

/// Basel III's total capital requirement with the conservation buffer,
/// which is what a bank actually manages to.
pub const REQUIRED_CAPITAL_RATIO: f64 = 0.105;
/// The leverage backstop.
pub const REQUIRED_LEVERAGE_RATIO: f64 = 0.04;
/// What a bank keeps against ordinary withdrawals.
pub const PRUDENT_RESERVE_RATIO: f64 = 0.06;

// =====================================================================
// who can borrow, and how much
// =====================================================================

/// **What a lender knows about somebody before it decides.**
///
/// Real underwriting is four questions and they are all here: can they
/// service it out of income, have they paid before, is there anything to
/// take back, and how much of their own money is in it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Applicant {
    pub account: Account,
    /// Take-home, a month.
    pub income: f64,
    /// What they already have to find each month before this.
    pub existing_payments: f64,
    /// What they can put down.
    pub savings: f64,
    /// **The record**, 0 to 1. Real US credit scores run 300-850 and the
    /// prime line is about 660-680, so this is that scale flattened.
    pub standing: f64,
    /// Whether they have somewhere to be found, which matters more than it
    /// sounds: real lenders will not touch somebody with no address.
    pub settled: bool,
}

impl Applicant {
    /// **Debt to income**, the number an underwriter looks at first. Real
    /// US: the qualified-mortgage rule caps it at 43%, and lenders start
    /// getting uncomfortable above 36%.
    pub fn debt_to_income(&self, new_payment: f64) -> f64 {
        if self.income <= 0.0 {
            return f64::INFINITY;
        }
        (self.existing_payments + new_payment) / self.income
    }
}

/// The maximum debt-to-income an underwriter will write. Real: 43% is the
/// qualified-mortgage line in the US, and it is where lenders stop for most
/// products.
pub const MAX_DEBT_TO_INCOME: f64 = 0.43;

/// **The offer, or the reason there is not one.**
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Offer {
    pub kind: Credit,
    pub principal: f64,
    pub rate: f64,
    pub monthly: f64,
    pub months: u32,
    /// What has to be found up front.
    pub deposit_required: f64,
}

impl Offer {
    /// What is handed back over the life of it, against what was borrowed.
    /// **Real: a 30-year mortgage at 7% repays about 2.4 times the loan**,
    /// and almost nobody who signs one has worked that out.
    pub fn times_over(&self) -> f64 {
        if self.principal <= 0.0 {
            return 0.0;
        }
        self.monthly * self.months as f64 / self.principal
    }
}

/// **Would they lend, and on what terms?**
///
/// Four tests in the order an underwriter applies them, and each one is a
/// different reason to be turned down — which matters, because "no" is not
/// one answer. Being refused for want of a deposit is a different problem
/// from being refused for the record, and only one of them is fixable this
/// year.
pub fn underwrite(
    bank: &Bank,
    rates: &Rates,
    who: &Applicant,
    kind: Credit,
    price: f64,
    security_worth: f64,
) -> Result<Offer, Refused> {
    // **A lender wants an address.** Real ones will not write a loan to
    // somebody with nowhere to be found, which is one more way being
    // homeless is self-sustaining.
    if !who.settled && kind != Credit::Pawn {
        return Err(Refused::NoCreditworthiness);
    }
    // **Below a floor, nobody will write it at any price** — and the floors
    // are real underwriting minimums rather than a judgement about
    // character. `standing` is a US credit score flattened onto 0..1:
    // FICO runs 300-850, so `(score - 300) / 550`.
    //
    // | | score | here |
    // |---|---|---|
    // | conventional mortgage | 620 | 0.58 |
    // | FHA mortgage | 580 | 0.51 |
    // | prime line | 670 | 0.67 |
    // | subprime auto | ~500 | 0.36 |
    // | buy-here-pay-here | none at all | 0.0 |
    //
    // **Subprime auto lending is enormous and real**, which is why the car
    // floor is so much lower than the mortgage one: there is always
    // somebody who will finance a car, because they can come and take it
    // back.
    let floor = match kind {
        Credit::Payday | Credit::Pawn => 0.0,
        Credit::Mortgage => 0.51,
        // **Deep subprime auto lending is real and large**, and below even
        // that a buy-here-pay-here lot will finance somebody with no score
        // at all — because they can come and take the car back, and they
        // do. There is very nearly no floor here.
        Credit::UsedCarLoan => 0.08,
        Credit::CarLoan => 0.34,
        Credit::CreditCard => 0.36,
        Credit::PersonalLoan => 0.45,
        Credit::TradeCredit => 0.40,
    };
    if who.standing < floor {
        return Err(Refused::NoCreditworthiness);
    }

    // **The deposit.** How much of their own money has to be in it, which
    // is what stops the lender losing on the first bad month.
    let ltv = kind.max_loan_to_value();
    let mut principal = price;
    let mut deposit = 0.0;
    if kind.secured() {
        let most = security_worth * ltv;
        if most <= 0.0 {
            return Err(Refused::NotEnoughSecurity);
        }
        if price > most {
            deposit = price - most;
            principal = most;
        }
        if deposit > who.savings {
            return Err(Refused::NotEnoughSecurity);
        }
    }

    let rate = rates.quoted(kind, who.standing);
    let months = kind.typical_term_months();
    let monthly = if months == 0 {
        // Revolving credit has no term. **The minimum payment is what
        // makes it last for ever**: real US cards ask 1-3% of the balance,
        // which on a 21.5% card barely covers the interest.
        principal * 0.025
    } else {
        Loan::level_payment(principal, rate, months)
    };

    // **Can they actually keep it up?**
    if who.debt_to_income(monthly) > MAX_DEBT_TO_INCOME {
        return Err(Refused::CannotAfford);
    }

    // **And can the bank?** Not a reserve multiplier — the capital ratio,
    // which is what actually binds, and the leverage backstop that exists
    // because risk weights can be gamed and were.
    let after = Bank { loans: bank.loans + principal, ..bank.clone() };
    if after.capital_ratio() < REQUIRED_CAPITAL_RATIO {
        return Err(Refused::NotEnoughCapital);
    }
    if after.leverage_ratio() < REQUIRED_LEVERAGE_RATIO {
        return Err(Refused::NotEnoughCapital);
    }
    // Liquidity: it has to be able to settle when the money is spent
    // somewhere else, which is the only sense in which reserves constrain
    // lending at all.
    if bank.reserves < (bank.deposits + principal) * 0.01 {
        return Err(Refused::NotEnoughLiquidity);
    }

    Ok(Offer { kind, principal, rate, monthly, months, deposit_required: deposit })
}

// =====================================================================
// the banking system
// =====================================================================

/// **Every bank, and the money they have between them made.**
///
/// Sits alongside `money::Treasury` rather than inside it, because the two
/// answer different questions: the Treasury says who holds the money and
/// this says where it came from. Together they have to agree, and a gate
/// asserts that they do.
#[derive(Clone, Debug)]
pub struct System {
    pub banks: Vec<Bank>,
    pub rates: Rates,
    pub loans: BTreeMap<u64, Loan>,
    next_loan: u64,
    /// **Every unit of money ever created by lending**, and every unit
    /// destroyed by repayment. Kept because the conservation check on the
    /// Treasury has to become "the total moved by exactly this much"
    /// rather than "the total did not move".
    pub created: f64,
    pub destroyed: f64,
    /// What the deposit insurer has had to make good.
    pub insurance_paid: f64,
    /// **Reserves the central bank has had to create** so that somebody
    /// could settle. Real: this is what an emergency facility is, and it
    /// grows base money — the Fed's balance sheet went from $900bn to
    /// $2.2tn in the last months of 2008 doing exactly this.
    pub central_bank_lending: f64,
}

/// Real: FDIC cover is $250,000 a depositor a bank, and it exists to stop
/// runs rather than to compensate for them.
pub const DEPOSIT_INSURANCE: f64 = 250_000.0;

impl System {
    pub fn new(rates: Rates) -> Self {
        System {
            banks: Vec::new(),
            rates,
            loans: BTreeMap::new(),
            next_loan: 1,
            created: 0.0,
            destroyed: 0.0,
            insurance_paid: 0.0,
            central_bank_lending: 0.0,
        }
    }

    pub fn add_bank(&mut self, capital: f64, reserves: f64) -> usize {
        let id = self.banks.len();
        self.banks.push(Bank::new(id, capital, reserves));
        id
    }

    pub fn bank(&self, id: usize) -> &Bank {
        &self.banks[id]
    }

    /// **Broad money: what people can actually spend.** Overwhelmingly
    /// bank deposits — real US puts currency at about 11% of M2 and the
    /// rest is somebody's promise.
    pub fn broad_money(&self) -> f64 {
        self.banks.iter().map(|b| b.deposits).sum()
    }

    /// **Base money: what the central bank issued.** Reserves plus cash,
    /// and it is a small fraction of the above.
    pub fn base_money(&self) -> f64 {
        self.banks.iter().map(|b| b.reserves).sum()
    }

    /// The ratio between them, which the textbooks call a multiplier and
    /// treat as a cause. **It is an outcome**: banks lend when it is
    /// profitable and prudent, and the ratio is whatever that produces.
    pub fn observed_multiple(&self) -> f64 {
        let base = self.base_money();
        if base <= 0.0 {
            return 0.0;
        }
        self.broad_money() / base
    }

    pub fn total_lending(&self) -> f64 {
        self.banks.iter().map(|b| b.loans).sum()
    }

    pub fn assert_balanced(&self) {
        for b in &self.banks {
            b.assert_balanced();
        }
    }

    /// **Make the loan, and with it the money.**
    ///
    /// The one operation this whole module exists for. Both sides of the
    /// bank's balance sheet grow: the loan is an asset and the borrower's
    /// new deposit is a liability. **No reserves move and no saver is
    /// deprived of anything** — which is the fact the textbook story gets
    /// backwards.
    pub fn advance(
        &mut self,
        bank: usize,
        borrower: Account,
        offer: &Offer,
        treasury: &mut Treasury,
        day: u64,
        security: Option<u64>,
    ) -> u64 {
        let id = self.next_loan;
        self.next_loan += 1;

        {
            let b = &mut self.banks[bank];
            b.loans += offer.principal;
            b.deposits += offer.principal;
            b.assert_balanced();
        }
        self.created += offer.principal;
        // And the borrower can spend it, which is the whole point.
        treasury.create_credit(borrower, offer.principal, day, Why::Lending);

        self.loans.insert(
            id,
            Loan {
                id,
                kind: offer.kind,
                borrower,
                bank,
                outstanding: offer.principal,
                advanced: offer.principal,
                rate: offer.rate,
                monthly: offer.monthly,
                months_left: offer.months,
                missed: 0,
                security,
            },
        );
        id
    }

    /// **A month's payment: interest to the bank, principal into thin
    /// air.**
    ///
    /// The asymmetry is the thing. Interest is a transfer — the borrower is
    /// poorer and the bank is richer and the money still exists. Principal
    /// is *destroyed*: the deposit shrinks and so does the loan, and the
    /// money supply is smaller than it was.
    ///
    /// Which is why an economy carrying debt needs somebody to keep
    /// borrowing. **The interest was never created alongside the
    /// principal**, so in aggregate it can only be found out of somebody
    /// else's new loan.
    pub fn take_payment(
        &mut self,
        loan_id: u64,
        available: f64,
        treasury: &mut Treasury,
        day: u64,
    ) -> Payment {
        let Some(loan) = self.loans.get_mut(&loan_id) else {
            return Payment::default();
        };
        let due = loan.monthly.min(loan.outstanding + loan.interest_this_month());
        if available + 1e-9 < due {
            loan.missed += 1;
            let missed = loan.missed;
            let defaulted = loan.defaulted();
            return Payment { due, paid: 0.0, missed: true, defaulted, arrears: missed, ..Default::default() };
        }

        let interest = loan.interest_this_month();
        let principal = (due - interest).max(0.0).min(loan.outstanding);
        loan.outstanding -= principal;
        loan.months_left = loan.months_left.saturating_sub(1);
        loan.missed = 0;
        let bank = loan.bank;
        let borrower = loan.borrower;
        let cleared = loan.outstanding <= 1e-6;

        {
            let b = &mut self.banks[bank];
            // **Principal: both sides shrink and the money is gone.**
            b.loans -= principal;
            b.deposits -= principal;
            // **Interest: the depositor's money becomes the bank's.**
            b.deposits -= interest;
            b.capital += interest;
            b.interest_earned += interest;
            b.assert_balanced();
        }
        self.destroyed += principal;
        treasury.destroy_credit(borrower, principal, day, Why::Repayment);
        treasury.pay(day, borrower, Account::Bank(bank), interest, Why::Interest);

        if cleared {
            self.loans.remove(&loan_id);
        }
        Payment { due, paid: due, interest, principal, cleared, ..Default::default() }
    }

    /// **What happens when they stop paying.**
    ///
    /// A secured lender takes the thing back and sells it, usually for less
    /// than is owed — real repossessed cars fetch about half the loan
    /// balance at auction, so the borrower loses the car *and* still owes
    /// the shortfall. An unsecured lender writes it off against capital,
    /// which is what capital is for.
    pub fn foreclose(&mut self, loan_id: u64, security_fetches: f64, day: u64) -> Foreclosure {
        let Some(loan) = self.loans.remove(&loan_id) else {
            return Foreclosure::default();
        };
        let recovered = if loan.kind.secured() { security_fetches.max(0.0) } else { 0.0 };
        let shortfall = (loan.outstanding - recovered).max(0.0);
        let b = &mut self.banks[loan.bank];
        // What comes back is money the borrower does not owe any more, so
        // the loan asset goes and reserves arrive in its place.
        b.loans -= loan.outstanding;
        b.reserves += recovered;
        // **And the rest comes out of capital**, which is exactly what
        // capital is for and why a bank with too little of it fails.
        b.capital -= shortfall;
        b.written_off += shortfall;
        b.assert_balanced();
        self.destroyed += loan.outstanding - recovered;
        let _ = day;
        Foreclosure {
            took_the_security: loan.kind.secured(),
            recovered,
            shortfall,
            still_owed: shortfall,
            bank_now_insolvent: self.banks[loan.bank].insolvent(),
        }
    }

    /// **Interest on deposits**, which is what a bank pays for its funding
    /// and the other half of its margin.
    pub fn credit_interest(&mut self, bank: usize, treasury: &mut Treasury, day: u64) -> f64 {
        let monthly = self.rates.on_deposits / 12.0;
        let b = &mut self.banks[bank];
        let paid = b.deposits * monthly;
        b.capital -= paid;
        b.deposits += paid;
        b.interest_paid += paid;
        b.assert_balanced();
        let _ = (treasury, day);
        paid
    }

    /// **Paying somebody at another bank moves reserves.**
    ///
    /// This is the only sense in which reserves constrain a bank at all —
    /// not as a pool that lending is drawn from, but as what has to be
    /// handed over when the money it created is spent somewhere else. A
    /// bank whose customers pay away more than they receive loses reserves
    /// and has to find funding, and that is what makes a lending spree
    /// dangerous rather than free.
    pub fn settle(&mut self, from_bank: usize, to_bank: usize, amount: f64) -> f64 {
        if from_bank == to_bank || amount <= 0.0 {
            return 0.0;
        }
        let moved = amount.min(self.banks[from_bank].reserves.max(0.0));
        let short = amount - moved;
        self.banks[from_bank].reserves -= moved;
        self.banks[from_bank].deposits -= amount;
        // **A bank that cannot settle borrows the difference** — overnight
        // from another bank, or from the central bank as lender of last
        // resort. That is a *liability*, not a hole in its capital, and
        // getting it wrong breaks the balance sheet outright.
        if short > 0.0 {
            self.banks[from_bank].borrowed += short;
            self.central_bank_lending += short;
        }
        // The payee is paid in full whatever the payer had to do to find
        // it, which is the whole point of a settlement system.
        self.banks[to_bank].reserves += amount;
        self.banks[to_bank].deposits += amount;
        self.banks[from_bank].assert_balanced();
        self.banks[to_bank].assert_balanced();
        moved
    }

    /// **Somebody wants their money in cash.**
    ///
    /// Which comes straight out of reserves, and is what a run is: not a
    /// question of whether the loans are good, but of whether the till has
    /// enough in it this afternoon.
    pub fn withdraw(&mut self, bank: usize, amount: f64) -> f64 {
        let b = &mut self.banks[bank];
        let paid = amount.min(b.reserves.max(0.0)).min(b.deposits.max(0.0));
        b.reserves -= paid;
        b.deposits -= paid;
        b.assert_balanced();
        paid
    }

    /// **How much of its deposits it has lent out.**
    ///
    /// The number that decides whether a bank makes money at all: it pays
    /// interest on every deposit and earns it only on what it has lent, so
    /// a bank sitting on idle deposits loses on them. Real US banks run a
    /// loan-to-deposit ratio around 70%, and a regulator starts worrying
    /// above about 90% because there is nothing left to meet withdrawals
    /// with.
    pub fn loan_to_deposit(&self, bank: usize) -> f64 {
        let b = &self.banks[bank];
        if b.deposits <= 0.0 {
            return 0.0;
        }
        b.loans / b.deposits
    }

    /// **The net interest margin**, which is a bank's living: what it
    /// charges less what it pays. Real US banks run about 3%.
    pub fn net_interest_margin(&self, bank: usize) -> f64 {
        let b = &self.banks[bank];
        if b.assets() <= 0.0 {
            return 0.0;
        }
        (b.interest_earned - b.interest_paid) / b.assets()
    }
}

/// What came of one month's payment.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Payment {
    pub due: f64,
    pub paid: f64,
    pub interest: f64,
    /// **Destroyed**, not transferred.
    pub principal: f64,
    pub missed: bool,
    pub arrears: u32,
    pub defaulted: bool,
    pub cleared: bool,
}

/// What came of taking it back.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Foreclosure {
    pub took_the_security: bool,
    pub recovered: f64,
    /// **What the sale did not cover, and the borrower still owes.** Which
    /// is the part people do not expect: losing the car does not clear the
    /// debt.
    pub shortfall: f64,
    pub still_owed: f64,
    pub bank_now_insolvent: bool,
}
