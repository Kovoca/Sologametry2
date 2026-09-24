//! **A firm keeps the money it needs to start again.**
//!
//! The profit sweep sized a firm's reserve on what it paid out *today*.
//! A works whose inputs stop pays out nothing — no materials, and with no
//! work no staff — so its reserve fell to thirty days of one wage and the
//! rest of its balance went to households as profit. Measured on the
//! two-town fixture: the cannery held 995,729 on day 60 and **156** five
//! days after its flour was cut. It came back only because this model
//! still lets a works take inputs it cannot pay for.

use scale_sim::econ::{Commodity, Doctrine, Economy, Event, Use};
use scale_sim::money::Account;
use scale_sim::slice::{self, Role};

fn spent_today(e: &Economy, site: usize) -> f64 {
    e.treasury
        .today
        .iter()
        .filter(|t| t.from == Account::Firm(site))
        .map(|t| t.amount)
        .sum()
}

fn cut_flour(e: &mut Economy, sites: &[usize]) {
    for &s in sites {
        let q = e.ledger.stock(s, Commodity::Flour);
        if q > 0.0 {
            e.ledger.apply(
                &mut e.journal,
                Event::Consumed {
                    site: s,
                    commodity: Commodity::Flour,
                    qty: q,
                    reason: Use::Input,
                },
            );
        }
    }
}

/// **An interruption to supply does not pay out the money to restart.**
///
/// Cut a viable works' input for a fortnight, give it back, and require
/// two things. While it waits, it keeps the cash it had, up to a month of
/// what it was actually spending before the cut — both read off its own
/// balance and transfers, not off the rule being tested, with a tenth
/// allowed for what an idle works still pays. And when supply returns it
/// meets its payroll in full from the first day, rather than running on
/// whatever the day's sales happen to bring in.
///
/// *Not a month of spending outright.* This cannery held 995,729 against
/// 1.6M for a month, because a works here does not carry that much, and a
/// rule that paid out nothing at all would have failed that bar.
#[test]
fn an_interruption_does_not_pay_out_the_money_to_restart() {
    let mut e = slice::build(Doctrine::Prudent);
    let can = slice::site(&e, Role::AshfordCannery);
    let mill = slice::site(&e, Role::AshfordMill);
    let milling = e.ledger.sites[mill].throughput;

    let mut spending = 0.0;
    for day in 0..60 {
        e.step();
        if day >= 50 {
            spending += spent_today(&e, can);
        }
    }
    let a_day = spending / 10.0;
    let held = e.treasury.balance(Account::Firm(can));
    let running = e.ledger.sites[can].ran;
    assert!(a_day > 0.0 && running > 0.0, "the cannery was not running");

    e.ledger.sites[mill].throughput = 0.0;
    cut_flour(&mut e, &[can, mill]);
    let mut lowest = f64::INFINITY;
    for _ in 0..15 {
        e.step();
        lowest = lowest.min(e.treasury.balance(Account::Firm(can)));
    }
    assert_eq!(
        e.ledger.sites[can].ran, 0.0,
        "the cut did not stop the cannery"
    );
    let keep = held.min(a_day * 30.0) * 0.9;
    assert!(
        lowest >= keep,
        "the cannery fell to {lowest:.0} while it waited for flour; it held \
         {held:.0} before the cut and spent {a_day:.0} a day"
    );

    e.ledger.sites[mill].throughput = milling;
    let mut restarted = false;
    for day in 0..30 {
        e.step();
        let met = e.payroll_met.get(can).copied().unwrap_or(1.0);
        assert!(
            met >= 0.999,
            "day {day} after the flour came back the cannery met {:.1}% of its payroll",
            met * 100.0
        );
        restarted |= e.ledger.sites[can].ran >= running * 0.99;
    }
    assert!(
        restarted,
        "the cannery never got back to {running:.1} a day"
    );
    e.ledger.assert_conserved();
}

/// **What a firm was given is not the owners' to take.**
///
/// A company may distribute only what it has earned: its paid-in capital
/// stays in the business *(UK Companies Act 2006 s.830; Delaware's DGCL
/// §170)*. So on any day a firm pays out profit it must still hold at least
/// what it opened with.
///
/// Measured before it was a rule: with it gone, 2,648 of 3,305 firm-days on
/// which somebody paid a dividend ended below capital across these two
/// fixtures, and the worst firm kept 0.02% of it — which is to say the old
/// sweep routinely paid out the money firms had been founded with and
/// called it profit. The interruption gate above does not see this,
/// because the cannery's planned needs exceed its capital; this is the
/// half that is about the capital.
#[test]
fn a_firm_pays_out_what_it_earned_and_not_what_it_was_given() {
    use scale_sim::money::Why;
    for (name, mut e) in [
        ("the two-town fixture", slice::build(Doctrine::Prudent)),
        ("the symmetric fixture", slice::symmetric(Doctrine::Prudent)),
    ] {
        let mut paying = 0;
        for _ in 0..200 {
            e.step();
            for s in 0..e.ledger.sites.len() {
                let paid: f64 = e
                    .treasury
                    .today
                    .iter()
                    .filter(|t| t.from == Account::Firm(s) && t.why == Why::Profit)
                    .map(|t| t.amount)
                    .sum();
                if paid <= 0.0 {
                    continue;
                }
                paying += 1;
                let capital = e.treasury.capital(Account::Firm(s));
                let held = e.treasury.balance(Account::Firm(s));
                assert!(
                    held >= capital * (1.0 - 1e-6),
                    "{name}, day {}: {} paid {paid:.0} of profit and holds {held:.0} \
                     against a capital of {capital:.0}",
                    e.ledger.day,
                    e.ledger.sites[s].name
                );
            }
        }
        // Passing on an absence proves nothing: somebody has to have paid.
        assert!(paying > 100, "{name}: only {paying} dividends in 200 days");
    }
}

/// **A save whose capital does not add up is refused.**
///
/// The capitals and the opening stock are two records of one fact — what
/// `open` put into the world — so they have to agree, or a firm's protected
/// capital is money that never came in. A validator that has only seen
/// clean data is untested, so each refusal is provoked, beside a control
/// that must load.
#[test]
fn a_treasury_whose_capital_does_not_add_up_is_refused() {
    use scale_sim::money::Treasury;
    use scale_sim::save::{Reader, SaveError, Store, Writer};
    // Two firms opened with a hundred between them; `capitals` is what the
    // file claims each was given.
    let bytes = |capitals: [f64; 2]| {
        let mut w = Writer::new();
        w.len(2);
        Account::Firm(0).store(&mut w);
        w.f64(60.0);
        Account::Firm(1).store(&mut w);
        w.f64(40.0);
        w.f64(100.0); // what was opened
        w.len(0); // nothing moved today
        w.f64(0.0); // unpaid
        w.f64(0.0); // created by lending
        w.f64(0.0); // destroyed by repayment
        w.len(2);
        Account::Firm(0).store(&mut w);
        w.f64(capitals[0]);
        Account::Firm(1).store(&mut w);
        w.f64(capitals[1]);
        w.bytes
    };
    let t = Treasury::load(&mut Reader::new(&bytes([60.0, 40.0]))).expect("a consistent treasury");
    assert_eq!(t.capital(Account::Firm(0)), 60.0);
    // Each case breaks exactly one rule: the first adds up and has a
    // negative, the second has no negative and does not add up.
    for (capitals, what) in [
        ([150.0, -50.0], "a negative capital that still adds up"),
        ([60.0, 10.0], "less capital than was opened"),
    ] {
        match Treasury::load(&mut Reader::new(&bytes(capitals))) {
            Err(SaveError::Impossible(_)) => {}
            Err(other) => panic!("{what} was refused for the wrong reason: {other:?}"),
            Ok(_) => panic!("{what} was accepted"),
        }
    }
}
