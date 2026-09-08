//! **A bill arrives monthly, in arrears, and nobody is cut off the day
//! they cannot pay it.**
//!
//! The household model charged for power weekly and disconnected anybody
//! who came up short, which is neither how a utility bills nor how it
//! collects. Real practice is a month of usage, a bill, three weeks to pay
//! it, a reminder, a formal notice, and only then a crew — and in a cold
//! state, in winter, **not even then.**
//!
//! That gap is not a detail. It is the difference between a bad month and
//! destitution, and it is where energy debt comes from: a household that
//! cannot pay in January is not cut off in January, it is *in arrears* in
//! January, and the reckoning comes in April.
//!
//! Real shape, all US:
//!
//! | | |
//! |---|---|
//! | average residential bill | **~$137 a month**, ~855 kWh at ~16 c |
//! | fixed customer charge | $8-15 a month before a single unit |
//! | payment due | 21 days after the bill |
//! | disconnection notice | 10-15 days after that, served separately |
//! | reconnection | $20-75, plus the arrears, plus often a deposit |
//! | households disconnected a year | **~3.5 million** |
//! | households behind on energy | ~20 million, about $20bn owed |
//! | low-income energy burden | **8.6% of income** against 3% for others |

use crate::basket::Climate;

// =====================================================================
// what a supply costs
// =====================================================================

/// **What the meter is read against.**
///
/// A tariff is not a price per unit. There is a fixed charge before a
/// single kilowatt-hour — which is regressive, and is exactly why a poor
/// household using almost nothing still has a bill — and above a threshold
/// the rate usually rises, because an inclining block tariff is how a
/// regulator makes heavy use pay for itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tariff {
    /// Payable whether or not anything is used. Real: $8-15 a month.
    pub standing_charge: f64,
    /// The first block, per kWh.
    pub unit_rate: f64,
    /// Where the second block starts, in kWh a month. Real inclining block
    /// tariffs step at somewhere between 400 and 1,000 kWh.
    pub block_kwh: f64,
    /// What the units above it cost. Typically 15-40% more.
    pub block_rate: f64,
}

impl Tariff {
    /// An ordinary American residential tariff, in the model's own currency
    /// — pinned like every other price to the food chain, so only the
    /// ratios are meant to be read.
    pub fn ordinary() -> Self {
        Tariff {
            standing_charge: 12.0,
            unit_rate: 0.41,
            block_kwh: 600.0,
            block_rate: 0.52,
        }
    }

    /// **What a month of use comes to.**
    pub fn bill_for(&self, kwh: f64) -> f64 {
        let first = kwh.min(self.block_kwh).max(0.0);
        let rest = (kwh - self.block_kwh).max(0.0);
        self.standing_charge + first * self.unit_rate + rest * self.block_rate
    }

    /// **What the last unit cost**, which is what anybody deciding whether
    /// to run the heater another hour is actually facing.
    pub fn marginal_rate(&self, kwh: f64) -> f64 {
        if kwh >= self.block_kwh {
            self.block_rate
        } else {
            self.unit_rate
        }
    }
}

// =====================================================================
// the account
// =====================================================================

/// Where a customer stands with the company.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// Paid up, or within the period allowed.
    Current,
    /// A bill is past its due date. Nothing happens yet except a reminder
    /// and, in most places, a late fee.
    Overdue,
    /// A formal disconnection notice has been served, with a date on it.
    /// **This is the point at which people find the money**, which is why
    /// utilities serve so many more notices than they act on.
    NoticeServed,
    /// Off supply.
    Disconnected,
}

impl Standing {
    pub fn name(self) -> &'static str {
        match self {
            Standing::Current => "up to date",
            Standing::Overdue => "behind",
            Standing::NoticeServed => "under notice",
            Standing::Disconnected => "cut off",
        }
    }

    pub fn supplied(self) -> bool {
        !matches!(self, Standing::Disconnected)
    }
}

/// **A customer's account with the utility.**
///
/// Holds what a real one holds: the meter since the last read, what is
/// owed, how long it has been owed, and whether a notice is out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Account {
    pub tariff: Tariff,
    /// Since the last meter read.
    pub unbilled_kwh: f64,
    /// Days since the last read. A cycle is about 30.
    pub since_read: u32,
    /// **What is owed and unpaid.** The number that makes energy debt a
    /// thing rather than an event.
    pub arrears: f64,
    /// Days past the due date on the oldest unpaid bill.
    pub days_overdue: u32,
    pub standing: Standing,
    /// Held against non-payment, and taken from a customer with a poor
    /// record. Real: one to two months of an average bill, and a serious
    /// barrier to getting reconnected.
    pub deposit: f64,
    /// **Levelised payment.** Real utilities offer budget billing: pay a
    /// twelfth of the year every month so January does not break you. It
    /// does not reduce the bill by a cent and it is the single most useful
    /// thing a struggling household can be signed up to.
    pub budget_plan: Option<f64>,
}

impl Account {
    pub fn new(tariff: Tariff) -> Self {
        Account {
            tariff,
            unbilled_kwh: 0.0,
            since_read: 0,
            arrears: 0.0,
            days_overdue: 0,
            standing: Standing::Current,
            deposit: 0.0,
            budget_plan: None,
        }
    }

    /// What the household should be putting aside a month, if it is on a
    /// levelised plan.
    pub fn monthly_due(&self) -> f64 {
        self.budget_plan.unwrap_or(0.0)
    }
}

/// The days that matter, and every one of them is a real statutory or
/// tariff period rather than a number picked to feel right.
pub const BILLING_CYCLE_DAYS: u32 = 30;
/// Real: 21 days is the commonest term on a US residential bill.
pub const DAYS_TO_PAY: u32 = 21;
/// A notice cannot be served the moment a bill is late. Real rules put it
/// at 10-15 days past due, and it must be in writing and separate.
pub const DAYS_BEFORE_NOTICE: u32 = 15;
/// And the notice itself carries a date. Real: 10-15 days again.
pub const NOTICE_PERIOD_DAYS: u32 = 12;
/// Real reconnection charges run $20-75.
pub const RECONNECTION_FEE: f64 = 40.0;
/// Real late fees are about 1.5% a month on the unpaid balance.
pub const LATE_FEE: f64 = 0.015;

// =====================================================================
// the winter rule
// =====================================================================

/// **In a cold state, in winter, they cannot cut you off at all.**
///
/// Not a kindness the model grants: a statute. Minnesota's Cold Weather
/// Rule runs 1 October to 30 April, and about thirty states have one.
/// Several also have hot-weather rules now, because heat kills as reliably
/// as cold.
///
/// The consequence is the thing worth modelling: a household that cannot
/// pay in January **accumulates debt instead of losing supply**, and the
/// reckoning arrives in spring with a bill nobody can meet. Which is
/// exactly what happens, and why arrears peak in April.
/// **The threshold is a proxy for a policy, not a physical fact.** Whether
/// there is a cold-weather rule is a matter of state law: about thirty
/// states have one, some cold states do not, and a few warm ones have a
/// heat rule instead. Deriving it from the climate is a reasonable default
/// for a generated world and is a simplification, recorded as one — 4,000
/// heating degree-days is the *national average*, so a threshold anywhere
/// near it protects the entire country and the rule stops meaning anything.
pub fn protected_today(where_: Climate, day_of_year: u32) -> bool {
    let d = day_of_year % 365;
    // A hard-winter place, and the months a cold-weather rule covers.
    if where_.heating_degree_days > 5_500.0 && !(120..=273).contains(&d) {
        return true;
    }
    // And where the danger is heat rather than cold, the same protection
    // sits over the summer.
    if where_.cooling_degree_days > 3_000.0 && (152..=258).contains(&d) {
        return true;
    }
    false
}

// =====================================================================
// a day of it
// =====================================================================

/// What the utility did today, for a caller that wants to know.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Event {
    /// A meter was read and a bill raised, for this much.
    pub billed: f64,
    /// A late fee was added.
    pub late_fee: f64,
    /// A disconnection notice was served today.
    pub notice: bool,
    /// The supply was cut today.
    pub cut_off: bool,
    /// It was not cut, because the season forbids it.
    pub protected: bool,
}

/// **Run the account forward a day**, given how much was used and what the
/// household paid.
///
/// Deliberately a day at a time rather than a period, because every one of
/// the thresholds is counted in days and a model that jumps a month cannot
/// serve a notice on the right one.
pub fn a_day(
    account: &mut Account,
    kwh_today: f64,
    paid: f64,
    where_: Climate,
    day_of_year: u32,
) -> Event {
    let mut event = Event::default();

    // Anything paid goes against the oldest debt first, which is how a
    // ledger works and is why part-paying does not clear a notice.
    account.arrears = (account.arrears - paid).max(0.0);
    if account.arrears <= 1e-9 {
        account.days_overdue = 0;
        if account.standing != Standing::Disconnected {
            account.standing = Standing::Current;
        }
    }

    // **Only a live supply runs a meter.** Somebody who has been cut off
    // uses nothing, which is the whole of what being cut off means.
    if account.standing.supplied() {
        account.unbilled_kwh += kwh_today;
        account.since_read += 1;
    }

    // A month goes by and the meter is read.
    if account.since_read >= BILLING_CYCLE_DAYS {
        let bill = account.tariff.bill_for(account.unbilled_kwh);
        account.arrears += bill;
        account.unbilled_kwh = 0.0;
        account.since_read = 0;
        event.billed = bill;
        // The clock on this bill starts now, not when it falls due.
        if account.days_overdue == 0 {
            account.days_overdue = 1;
        }
    } else if account.arrears > 1e-9 {
        account.days_overdue += 1;
    }

    if account.arrears <= 1e-9 {
        return event;
    }

    // Past the due date: a reminder and a late fee, and nothing else.
    if account.days_overdue > DAYS_TO_PAY && account.standing == Standing::Current {
        account.standing = Standing::Overdue;
        let fee = account.arrears * LATE_FEE;
        account.arrears += fee;
        event.late_fee = fee;
    }

    // A formal notice, which is a separate step and is where most people
    // find the money.
    if account.days_overdue > DAYS_TO_PAY + DAYS_BEFORE_NOTICE
        && account.standing == Standing::Overdue
    {
        account.standing = Standing::NoticeServed;
        event.notice = true;
    }

    // And only then a crew — unless the season forbids it.
    if account.days_overdue > DAYS_TO_PAY + DAYS_BEFORE_NOTICE + NOTICE_PERIOD_DAYS
        && account.standing == Standing::NoticeServed
    {
        if protected_today(where_, day_of_year) {
            event.protected = true;
        } else {
            account.standing = Standing::Disconnected;
            event.cut_off = true;
        }
    }

    event
}

/// **What it costs to get back on.**
///
/// The arrears in full, a reconnection charge, and — for a customer with a
/// record of not paying — a deposit of one to two months. Which is money
/// somebody who could not find a month's bill certainly does not have, and
/// is why being cut off is self-sustaining in the same way homelessness is.
pub fn cost_to_reconnect(account: &Account) -> f64 {
    let deposit = if account.deposit > 0.0 {
        0.0
    } else {
        account.tariff.standing_charge * 8.0
    };
    account.arrears + RECONNECTION_FEE + deposit
}

/// Pay it and the supply comes back on.
pub fn reconnect(account: &mut Account, paid: f64) -> bool {
    if paid + 1e-9 < cost_to_reconnect(account) {
        return false;
    }
    account.deposit = account.tariff.standing_charge * 8.0;
    account.arrears = 0.0;
    account.days_overdue = 0;
    account.standing = Standing::Current;
    true
}

/// **Levelise it.** A twelfth of the expected year, every month, so January
/// does not break the household. Costs nothing and changes everything about
/// whether a bill is payable.
pub fn budget_plan(account: &mut Account, expected_year_kwh: f64) {
    let year = account.tariff.bill_for(expected_year_kwh / 12.0) * 12.0;
    account.budget_plan = Some(year / 12.0);
}

/// **Help with the bill.**
///
/// Real: the Low Income Home Energy Assistance Program reaches about six
/// million US households with an average benefit near $500 a year — which
/// covers roughly a third of a year's electricity and is the difference
/// between arrears and disconnection for a great many people. It reaches
/// only about a sixth of those eligible, because it is funded by
/// appropriation rather than entitlement.
pub fn assistance(annual_bill: f64, income: f64, funded: f64) -> f64 {
    // Targeted at households whose energy burden is above the ordinary 3%.
    let burden = if income > 0.0 {
        annual_bill / income
    } else {
        1.0
    };
    if burden < 0.06 {
        return 0.0;
    }
    (annual_bill * 0.33 * funded.clamp(0.0, 1.0)).min(annual_bill)
}
