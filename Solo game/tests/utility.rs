//! **A bill is monthly, in arrears, and nobody is cut off the day they
//! cannot pay it.**
//!
//! Every gate here is a way the first version of this was wrong: charging
//! weekly, disconnecting instantly, and having no way at all to be in debt
//! to the electricity company.

use scale_sim::basket::Climate;
use scale_sim::utility::*;

/// Run an account for a year, paying a fixed amount a day, and report what
/// happened on the way.
fn a_year(
    account: &mut Account,
    kwh_a_day: f64,
    pay_a_day: f64,
    where_: Climate,
    from_day: u32,
) -> (Option<u32>, Option<u32>, u32) {
    let mut noticed = None;
    let mut cut = None;
    let mut protected_days = 0;
    for d in 0..365u32 {
        let e = a_day(account, kwh_a_day, pay_a_day, where_, from_day + d);
        if e.notice && noticed.is_none() {
            noticed = Some(d);
        }
        if e.cut_off && cut.is_none() {
            cut = Some(d);
        }
        if e.protected {
            protected_days += 1;
        }
    }
    (noticed, cut, protected_days)
}

/// **Gate: a tariff is not a price per unit.**
///
/// There is a fixed charge before a single kilowatt-hour, which is
/// regressive and is exactly why a household using almost nothing still has
/// a bill worth worrying about.
#[test]
fn using_nothing_still_costs_something() {
    let t = Tariff::ordinary();
    assert!(
        t.bill_for(0.0) > 0.0,
        "a month of using nothing came to nothing"
    );
    assert!((t.bill_for(0.0) - t.standing_charge).abs() < 1e-9);

    // **And the last unit costs more than the first.** An inclining block
    // tariff is how a regulator makes heavy use pay for itself.
    let light = t.bill_for(200.0);
    let heavy = t.bill_for(1_200.0);
    assert!(
        heavy > light * 6.0,
        "six times the units cost less than six times the money"
    );
    assert!(t.marginal_rate(1_000.0) > t.marginal_rate(100.0));

    // The fixed charge is a far larger share of a small bill, which is the
    // regressive part stated as a number.
    let small_share = t.standing_charge / t.bill_for(150.0);
    let large_share = t.standing_charge / t.bill_for(1_200.0);
    assert!(
        small_share > large_share * 3.0,
        "the standing charge weighed the same on a small bill as a large one"
    );

    // An ordinary American month: about 855 kWh.
    let ordinary = t.bill_for(855.0);
    assert!(ordinary > t.bill_for(400.0) && ordinary < t.bill_for(1_500.0));
}

/// **Gate: a month of usage, then a bill.** Not a charge every week.
#[test]
fn the_meter_is_read_once_a_month() {
    let mut a = Account::new(Tariff::ordinary());
    let mild = Climate::temperate();
    let mut bills = 0;
    // Paying as it goes, so it is never cut off — a disconnected meter
    // stops being read, which is correct and is not what this gate is
    // about.
    for d in 0..365u32 {
        let e = a_day(&mut a, 28.0, 30.0, mild, d);
        if e.billed > 0.0 {
            bills += 1;
        }
    }
    assert_eq!(bills, 12, "a year produced {bills} bills");
    // And a bill covers a month of usage rather than a day of it.
    let mut b = Account::new(Tariff::ordinary());
    let mut first = 0.0;
    for d in 0..31u32 {
        let e = a_day(&mut b, 28.0, 30.0, mild, d);
        if e.billed > 0.0 {
            first = e.billed;
        }
    }
    // A month of units, charged once — not thirty standing charges, which
    // is what multiplying a day's bill by thirty would give.
    let t = Tariff::ordinary();
    assert!(
        (first - t.bill_for(28.0 * 30.0)).abs() < 1e-6,
        "the bill was {first:.2} against a month of use at {:.2}",
        t.bill_for(28.0 * 30.0)
    );
    assert!(
        first > t.bill_for(28.0) * 10.0,
        "a monthly bill covered a day"
    );
    assert!(
        first < t.bill_for(28.0) * 30.0,
        "a month cost thirty standing charges"
    );
}

/// **Gate: nobody is cut off the day they cannot pay.**
///
/// Due date, late fee, a written notice, and only then a crew. Every step
/// is a real statutory or tariff period, and the gap between them is the
/// difference between a bad month and destitution.
#[test]
fn the_road_to_being_cut_off_has_four_steps_and_takes_months() {
    let mut a = Account::new(Tariff::ordinary());
    let mild = Climate::temperate();
    // Deliberately a mild place in a mild season, so no winter rule
    // protects them: start in May.
    let (noticed, cut, protected) = a_year(&mut a, 28.0, 0.0, mild, 130);

    assert_eq!(protected, 0, "a temperate May was treated as a cold winter");
    let noticed = noticed.expect("nobody was ever served notice");
    let cut = cut.expect("nobody paying anything at all was never cut off");

    // The first bill lands at day 30; the notice cannot come before the
    // due date plus the statutory wait.
    assert!(
        noticed >= 30 + DAYS_TO_PAY + DAYS_BEFORE_NOTICE,
        "a notice was served on day {noticed}, before it legally could be"
    );
    assert!(
        cut >= noticed + NOTICE_PERIOD_DAYS,
        "the crew came {} days after the notice",
        cut - noticed
    );
    // Which is months, not a week.
    assert!(cut > 70, "cut off after {cut} days of not paying");
    assert!(
        cut < 130,
        "still on supply after {cut} days of paying nothing at all"
    );

    // **A household that pays keeps its supply**, obviously, and the gate
    // is here because the one above is only meaningful against it.
    let mut paying = Account::new(Tariff::ordinary());
    let (n, c, _) = a_year(&mut paying, 28.0, 20.0, mild, 130);
    assert_eq!(c, None, "somebody paying their bill was cut off");
    assert_eq!(n, None, "somebody paying their bill was served notice");
    assert_eq!(paying.standing, Standing::Current);
}

/// **Gate: in a cold state in winter they cannot cut you off at all.**
///
/// Not a kindness: a statute. Minnesota's Cold Weather Rule runs October to
/// April and about thirty states have one. The consequence is the thing
/// worth having — a household that cannot pay in January accumulates debt
/// instead of losing supply, and the reckoning arrives in spring.
#[test]
fn a_minnesota_winter_forbids_it_and_the_debt_grows_instead() {
    let cold = Climate::cold();
    let mild = Climate::temperate();

    // The same household, paying nothing, from the first of November.
    let mut in_cold = Account::new(Tariff::ordinary());
    let (_, cut_cold, protected) = a_year(&mut in_cold, 40.0, 0.0, cold, 305);
    let mut in_mild = Account::new(Tariff::ordinary());
    let (_, cut_mild, _) = a_year(&mut in_mild, 40.0, 0.0, mild, 305);

    assert!(
        protected > 30,
        "a Minnesota winter protected nobody for {protected} days"
    );
    let cut_cold = cut_cold.expect("never cut off at all, even in May");
    let cut_mild = cut_mild.expect("a temperate household was never cut off");
    assert!(
        cut_cold > cut_mild + 60,
        "cut off on day {cut_cold} in Minnesota against {cut_mild} in a mild state"
    );

    // **And the debt is far larger by then**, because the meter ran the
    // whole winter. This is why arrears peak in spring.
    assert!(
        in_cold.arrears > in_mild.arrears * 1.5,
        "Minnesota owed {:.0} against {:.0}",
        in_cold.arrears,
        in_mild.arrears
    );

    // The protection is seasonal, not permanent.
    assert!(
        protected_today(cold, 15),
        "mid-January in Minnesota was not protected"
    );
    assert!(
        !protected_today(cold, 190),
        "July in Minnesota was protected from the cold"
    );
    // And where the danger is heat, the rule sits over the summer instead.
    assert!(
        protected_today(Climate::hot(), 200),
        "a July heatwave in Florida was not protected"
    );
    assert!(!protected_today(Climate::hot(), 15));
}

/// **Gate: getting back on costs more than staying on.**
///
/// The arrears in full, a reconnection charge, and a deposit — money
/// somebody who could not find a month's bill certainly does not have.
/// Which is what makes being cut off self-sustaining in the same way
/// homelessness is.
#[test]
fn being_cut_off_is_harder_to_get_out_of_than_into() {
    let mut a = Account::new(Tariff::ordinary());
    let mild = Climate::temperate();
    let (_, cut, _) = a_year(&mut a, 28.0, 0.0, mild, 130);
    assert!(cut.is_some());
    assert_eq!(a.standing, Standing::Disconnected);

    let owed = a.arrears;
    let to_get_back = cost_to_reconnect(&a);
    assert!(
        to_get_back > owed + RECONNECTION_FEE,
        "getting reconnected cost the arrears and nothing else"
    );
    assert!(
        to_get_back > Tariff::ordinary().bill_for(855.0) * 2.0,
        "reconnection came to less than two months of supply"
    );

    // Part-paying does not do it.
    assert!(
        !reconnect(&mut a, to_get_back * 0.9),
        "it reconnected on nine tenths of the money"
    );
    assert_eq!(a.standing, Standing::Disconnected);
    assert!(reconnect(&mut a, to_get_back));
    assert_eq!(a.standing, Standing::Current);
    assert!(
        a.deposit > 0.0,
        "a customer with a record of not paying left no deposit"
    );

    // **And a disconnected meter records nothing.** Being cut off is not a
    // discount, it is the absence of supply.
    let mut off = Account::new(Tariff::ordinary());
    off.standing = Standing::Disconnected;
    for d in 0..40u32 {
        a_day(&mut off, 40.0, 0.0, mild, d);
    }
    assert!(
        off.unbilled_kwh < 1e-9,
        "a cut-off house was still running the meter"
    );
}

/// **Gate: a levelised plan changes nothing about the bill and everything
/// about whether January is payable.**
#[test]
fn budget_billing_does_not_make_it_cheaper() {
    let mut a = Account::new(Tariff::ordinary());
    // A household using 12,000 kWh a year, heavily weighted to the winter.
    budget_plan(&mut a, 12_000.0);
    let monthly = a.monthly_due();
    let t = Tariff::ordinary();

    let flat_year = t.bill_for(1_000.0) * 12.0;
    assert!(
        (monthly * 12.0 - flat_year).abs() < 1e-6,
        "a budget plan changed the year's total: {:.0} against {flat_year:.0}",
        monthly * 12.0
    );

    // **What it buys is that the worst month is not the worst month.** A
    // January of 2,000 kWh against a levelised twelfth.
    let january = t.bill_for(2_000.0);
    assert!(
        january > monthly * 1.5,
        "a hard January was no worse than an average month"
    );
}

/// **Gate: help with the bill is targeted at burden, not at poverty as
/// such** — and it runs out, because it is an appropriation rather than an
/// entitlement.
///
/// Real: LIHEAP reaches about six million US households at an average of
/// roughly $500 a year, which is about a sixth of those eligible.
#[test]
fn assistance_goes_to_the_households_it_costs_most() {
    let bill = 1_600.0;
    // A comfortable household: the bill is 3% of income, the ordinary
    // figure, and nothing is due.
    assert_eq!(assistance(bill, 53_000.0, 1.0), 0.0);
    // A low-income one: an 8.6% burden, which is the real average for the
    // bottom of the distribution.
    let poor = assistance(bill, 18_600.0, 1.0);
    assert!(
        poor > 0.0,
        "a household spending 8.6% of its income on energy got nothing"
    );
    assert!(poor < bill, "assistance covered the whole bill");

    // **And it is cut when the appropriation is.** Same household, a
    // programme funded at a third.
    let squeezed = assistance(bill, 18_600.0, 0.33);
    assert!(
        squeezed < poor * 0.4,
        "cutting the funding by two thirds changed nothing"
    );
}
