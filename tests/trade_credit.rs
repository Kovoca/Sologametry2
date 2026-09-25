//! **Between firms, a delivery is paid for, owed on agreed terms, or not
//! made.**
//!
//! What this replaces moved the goods and capped the payment at whatever
//! the buyer held, putting the shortfall on a tally wiped the next morning,
//! so every supplier was a compulsory lender whose loan was forgotten by
//! breakfast. Now a delivery is authorised before it moves — out of the
//! buyer's cash, and, unless it buys for cash on delivery, out of credit
//! the supplier agrees to within a limit and on net-30 terms — and what is
//! owed stays owed until it is paid or charged off.
//!
//! Each gate was checked by breaking the thing it names.

use scale_sim::credit::Origin;
use scale_sim::econ::{Doctrine, Economy};
use scale_sim::money::{Account, Why};
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Region;
use scale_sim::slice::{self, Role};

fn a_nation() -> Region {
    let world = scale_sim::world::World::generate(384, 216, 20260828);
    let polities = Polities::partition(&world, 24);
    let settlements = scale_sim::settlement::Settlements::place(&world, &polities, 3000);
    let network = Network::build(&world, &settlements, 500);
    let &(id, _) = polities.ranked().first().expect("no nations");
    Region::extract(
        &world,
        &polities,
        &settlements,
        &network,
        id,
        5,
        Doctrine::Prudent,
    )
    .expect("no region")
}

/// Move money between two accounts without creating or destroying any.
fn shift(e: &mut Economy, from: Account, to: Account, amount: f64) {
    let day = e.ledger.day;
    e.treasury.pay(day, from, to, amount, Why::Purchase);
}

/// **Nothing a firm fails to pay is forgotten.** Over a run of a real
/// nation, on either rule, the day's tally of failed payments holds nothing
/// from a firm — every shortfall is owed on the book — except capital lent
/// abroad, which records what crossed and is not a bill. And every delivery
/// between firms that went on credit is on the book for what was owed.
#[test]
fn nothing_a_firm_fails_to_pay_is_forgotten() {
    for cash_on_delivery in [false, true] {
        let mut e = a_nation().economy;
        e.experiments.cash_on_delivery = cash_on_delivery;
        let mut owed_days = 0;
        for d in 0..150 {
            e.step();
            for (&(from, to, why), &v) in e.treasury.unpaid_by.iter() {
                if why == "capital" {
                    continue;
                }
                assert!(
                    v <= 1e-6,
                    "cash on delivery {cash_on_delivery}, day {d}: {from:?} failed to pay \
                     {to:?} {v:.6e} for {why}, and it was counted and forgotten"
                );
            }
            for (&(debtor, creditor, origin), b) in e.bills_today.iter() {
                assert!(
                    (b.paid + b.owed - b.amount).abs() <= 1e-9 * b.amount.max(1.0),
                    "day {d}: a {} charge from {debtor:?} to {creditor:?} of {:.6e} was paid \
                     {:.6e} and owed {:.6e}",
                    origin.name(),
                    b.amount,
                    b.paid,
                    b.owed
                );
                if b.owed > 1e-6 && matches!(debtor, Account::Firm(_)) {
                    owed_days += 1;
                    let on_book = e
                        .obligations
                        .find(debtor, creditor, origin, e.ledger.day)
                        .and_then(|k| e.obligations.get(k))
                        .map(|i| i.amount)
                        .unwrap_or(0.0);
                    assert!(
                        on_book >= b.owed * (1.0 - 1e-9),
                        "day {d}: a firm owed {:.6e} for {} and the book has {:.6e}",
                        b.owed,
                        origin.name(),
                        on_book
                    );
                }
            }
        }
        if !cash_on_delivery {
            assert!(
                owed_days > 0,
                "no firm owed anything on credit in 150 days — the rule was never exercised"
            );
        }
    }
}

/// **For cash on delivery, goods between firms move only as far as they are
/// paid for.** Nothing is owed for supply or carriage — a firm that cannot
/// pay has the delivery cut — while wages, for work already done, can still
/// be owed.
#[test]
fn cash_on_delivery_moves_only_what_is_paid_for() {
    let mut e = a_nation().economy;
    e.experiments.cash_on_delivery = true;
    let mut moved = 0.0;
    for d in 0..120 {
        e.step();
        for (&(debtor, _, origin), b) in e.bills_today.iter() {
            if !matches!(debtor, Account::Firm(_)) {
                continue;
            }
            if origin == Origin::Supply {
                moved += b.amount;
            }
            // A consignment struck days ago is settled on arrival, and is
            // unloaded only as far as the consignee can pay then — so this
            // holds for goods that came by lorry too.
            if matches!(origin, Origin::Supply | Origin::Carriage) {
                assert!(
                    b.owed <= 1e-6 * b.amount.max(1.0),
                    "day {d}: {debtor:?} was sent {:.3e} of {} for cash on delivery and owes \
                     {:.3e} of it",
                    b.amount,
                    origin.name(),
                    b.owed
                );
            }
        }
    }
    assert!(moved > 0.0, "no goods moved between firms at all");
}

/// **A works seriously in arrears is on stop: cash, and nothing on credit.**
#[test]
fn a_works_on_stop_buys_only_for_cash() {
    let mut e = slice::viable(Doctrine::Prudent);
    // Far enough in that a bill can be dated a hundred days past due.
    for _ in 0..150 {
        e.step();
    }
    let mill = slice::site(&e, Role::HarwickMill);
    let farm = e
        .ledger
        .sites
        .iter()
        .position(|s| s.kind == scale_sim::econ::SiteKind::Farm)
        .expect("no farm");
    let buyer = Account::Firm(mill);
    let seller = Account::Firm(farm);
    // Empty the mill's till into the farm's.
    let held = e.treasury.balance(buyer);
    shift(&mut e, buyer, seller, held);
    let value = 1_000.0;
    let before = e.authorise(mill, seller, value, 0.0);
    assert!(
        before > 0.99,
        "a works with no cash and no arrears was refused credit: {before:.3}"
    );
    // A bill a hundred days past due, and it is on stop.
    let day = e.ledger.day;
    e.obligations.bill(
        day.saturating_sub(130),
        buyer,
        seller,
        500.0,
        30,
        Why::Supply,
        Origin::Supply,
    );
    let after = e.authorise(mill, seller, value, 0.0);
    assert!(
        after < 1e-9,
        "a works a hundred days in arrears with no cash was still sent {:.0}% on credit",
        after * 100.0
    );
    // Cash still buys.
    shift(&mut e, seller, buyer, 400.0);
    let with_cash = e.authorise(mill, seller, value, 0.0);
    assert!(
        (with_cash - 0.4).abs() < 1e-6,
        "a works on stop with 400 in hand could have {:.0}% of a 1,000 delivery",
        with_cash * 100.0
    );
}

/// **Wages owed come before suppliers, and a firm in arrears keeps a day's
/// working capital before it pays them.**
#[test]
fn wages_first_then_working_capital_then_suppliers() {
    let mut e = slice::viable(Doctrine::Prudent);
    for _ in 0..60 {
        e.step();
    }
    let mill = slice::site(&e, Role::HarwickMill);
    let farm = e
        .ledger
        .sites
        .iter()
        .position(|s| s.kind == scale_sim::econ::SiteKind::Farm)
        .expect("no farm");
    let firm = Account::Firm(mill);
    let town = Account::Households(e.ledger.sites[mill].market);
    let keep = e.planned_outlay(mill);
    assert!(keep > 0.0);
    // Leave the mill a day's working capital and a little over.
    let held = e.treasury.balance(firm);
    shift(&mut e, firm, Account::Firm(farm), held - keep * 1.5);
    let day = e.ledger.day;
    // An old supplier's bill, long due, far larger than the cash; and wages
    // owed from yesterday.
    let old = e
        .obligations
        .bill(
            day.saturating_sub(40),
            firm,
            Account::Firm(farm),
            keep * 10.0,
            30,
            Why::Supply,
            Origin::Supply,
        )
        .expect("no bill");
    let wages = e
        .obligations
        .bill(
            day.saturating_sub(1),
            firm,
            town,
            keep * 0.25,
            0,
            Why::Payroll,
            Origin::Wages,
        )
        .expect("no bill");
    e.settle_between_firms();
    let left = |k| e.obligations.get(k).map(|i| i.outstanding()).unwrap_or(0.0);
    assert!(
        left(wages) <= 1e-6,
        "wages owed were left {:.3} unpaid while a supplier was paid",
        left(wages)
    );
    let cash = e.treasury.balance(firm);
    assert!(
        cash >= keep * (1.0 - 1e-9),
        "a works in arrears paid its suppliers down to {cash:.3}, below the {keep:.3} it needs \
         to run tomorrow"
    );
    assert!(
        left(old) < keep * 10.0,
        "a works with cash over a day's working capital paid its overdue supplier nothing"
    );
}

/// **A works in arrears goes on producing.** On the viable fixture the mill
/// mills at a loss before the harvest and falls behind with its farms. When
/// it handed every penny to its oldest creditor it went on stop with nothing
/// to buy grain with and stood idle for two hundred days, the cannery idle
/// beside it; keeping a day's working capital, it pays its farms down out of
/// the rest and does not stop.
#[test]
fn a_works_in_arrears_goes_on_producing() {
    let mut e = slice::viable(Doctrine::Prudent);
    let mill = slice::site(&e, Role::HarwickMill);
    let mut idle = 0;
    let mut longest = 0;
    let mut behind = false;
    for _ in 0..730 {
        e.step();
        if e.obligations.overdue_by(Account::Firm(mill), e.ledger.day) > 0.0 {
            behind = true;
        }
        if e.ledger.sites[mill].ran <= 1e-9 {
            idle += 1;
            longest = longest.max(idle);
        } else {
            idle = 0;
        }
    }
    assert!(
        behind,
        "the mill never fell behind with its farms, so the rule was never exercised"
    );
    assert!(
        longest <= 30,
        "the mill stood idle {longest} days running while in arrears"
    );
}

/// **No dividend while a firm owes.** Capital maintenance, once there are
/// debts: what a firm owes comes before what its owners take.
#[test]
fn a_firm_pays_no_dividend_while_it_owes() {
    let mut e = slice::viable(Doctrine::Prudent);
    for _ in 0..30 {
        e.step();
    }
    let paid_profit = |e: &Economy, site: usize| -> f64 {
        e.treasury
            .today
            .iter()
            .filter(|t| t.from == Account::Firm(site) && t.why == Why::Profit)
            .map(|t| t.amount)
            .sum()
    };
    // Find a works that is paying its owners on an ordinary day.
    let mut payer = None;
    for _ in 0..30 {
        e.step();
        if let Some(s) = (0..e.ledger.sites.len()).find(|&s| paid_profit(&e, s) > 0.0) {
            payer = Some(s);
            break;
        }
    }
    let site = payer.expect("no works paid a dividend in thirty days");
    let firm = Account::Firm(site);
    // Now it owes, not yet due, as much as it holds.
    let day = e.ledger.day;
    let other = (0..e.ledger.sites.len()).find(|&s| s != site).unwrap();
    let held = e.treasury.balance(firm);
    e.obligations.bill(
        day,
        firm,
        Account::Firm(other),
        held * 2.0,
        30,
        Why::Supply,
        Origin::Supply,
    );
    e.step();
    assert!(
        paid_profit(&e, site) <= 1e-9,
        "a works owing twice what it holds paid its owners {:.3e}",
        paid_profit(&e, site)
    );
}

/// **A firm's debt left 180 days past due is charged off**, the same event a
/// household's is — dated, by kind, and the creditor's loss. Without it a
/// works that will never pay keeps a debt on the book for ever, which is a
/// recovery sustained by invoices nobody will settle.
#[test]
fn a_firms_debt_long_past_due_is_charged_off() {
    let mut e = slice::viable(Doctrine::Prudent);
    for _ in 0..260 {
        e.step();
    }
    let mill = slice::site(&e, Role::HarwickMill);
    let farm = e
        .ledger
        .sites
        .iter()
        .position(|s| s.kind == scale_sim::econ::SiteKind::Farm)
        .expect("no farm");
    let day = e.ledger.day;
    let old = e
        .obligations
        .bill(
            day - 250,
            Account::Firm(mill),
            Account::Firm(farm),
            1_000.0,
            30,
            Why::Supply,
            Origin::Supply,
        )
        .expect("no bill");
    // It is already more than 180 days past due, so tomorrow morning's
    // book-keeping takes it — before anybody could pay it.
    e.step();
    match e.obligations.look(old) {
        scale_sim::registry::Lookup::Gone(stone) => assert_eq!(stone.how, "charged off"),
        other => panic!("a firm's debt 220 days past due is still {other:?}"),
    }
}
