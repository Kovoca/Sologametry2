//! **A bank does not lend out deposits. Making a loan creates one.**
//!
//! Every gate here is a way the textbook story is wrong, or a way a
//! monetary model quietly stops adding up. The headline is not a matter of
//! opinion: the Bank of England published it *(McLeay, Radia & Thomas,
//! 2014)*.

use scale_sim::bank::*;
use scale_sim::money::{Account, Treasury};

fn a_borrower(income: f64, standing: f64) -> Applicant {
    Applicant {
        account: Account::Households(0),
        income,
        existing_payments: 0.0,
        savings: 8_000.0,
        standing,
        settled: true,
    }
}

/// **A bank with a realistic shape.** Capital is about a tenth of what it
/// will lend, not three times it, and its deposits are what it lends
/// against — a bank sitting on idle deposits pays interest on them and
/// earns nothing.
fn a_system() -> (System, Treasury, usize) {
    let mut sys = System::new(Rates::ordinary());
    let bank = sys.add_bank(400_000.0, 150_000.0);
    let mut t = Treasury::new();
    t.open(Account::Households(0), 20_000.0);
    t.open(Account::Bank(bank), 400_000.0);
    (sys, t, bank)
}

// =====================================================================
// the headline
// =====================================================================

/// **Gate: making a loan creates the deposit. No reserves move and no
/// saver is deprived of anything.**
///
/// The whole module exists for this. The textbook story — savers deposit,
/// banks lend the money on, a reserve ratio multiplies it up — is
/// backwards, and a model built on it cannot represent a modern economy.
#[test]
fn lending_creates_the_money_it_lends() {
    let (mut sys, mut t, bank) = a_system();
    let before_reserves = sys.bank(bank).reserves;
    let before_deposits = sys.bank(bank).deposits;
    let before_money = t.total();
    let other_savers = t.balance(Account::Households(0));

    let who = a_borrower(4_000.0, 0.8);
    let offer = underwrite(sys.bank(bank), &sys.rates, &who, Credit::CarLoan, 24_000.0, 24_000.0)
        .expect("a solid borrower was refused a car loan");
    sys.advance(bank, Account::Firm(9), &offer, &mut t, 1, None);

    // **Both sides of the balance sheet grew.**
    assert!((sys.bank(bank).loans - offer.principal).abs() < 1e-6);
    assert!(
        (sys.bank(bank).deposits - before_deposits - offer.principal).abs() < 1e-6,
        "the deposit did not appear alongside the loan"
    );
    sys.assert_balanced();

    // **And no reserves moved.** This is the part the multiplier story
    // cannot express.
    assert!(
        (sys.bank(bank).reserves - before_reserves).abs() < 1e-9,
        "lending moved {:.2} of reserves",
        sys.bank(bank).reserves - before_reserves
    );

    // **Nobody else is a penny worse off.** Nothing was lent *out* of
    // anything.
    assert_eq!(t.balance(Account::Households(0)), other_savers);

    // The money supply is larger by exactly the loan, and the books still
    // add up because the creation went through a named door.
    assert!((t.total() - before_money - offer.principal).abs() < 1e-6);
    assert!((sys.created - offer.principal).abs() < 1e-9);
    t.assert_conserved();
}

/// **Gate: repaying the principal destroys the money. Paying the interest
/// does not.**
///
/// The asymmetry is the thing. Interest is a transfer — the borrower is
/// poorer, the bank richer, the money still exists. Principal is gone.
#[test]
fn repaying_it_unmakes_the_money_and_interest_only_moves_it() {
    let (mut sys, mut t, bank) = a_system();
    let who = a_borrower(4_000.0, 0.8);
    let offer =
        underwrite(sys.bank(bank), &sys.rates, &who, Credit::CarLoan, 24_000.0, 24_000.0).unwrap();
    let id = sys.advance(bank, Account::Households(0), &offer, &mut t, 1, None);
    let after_lending = t.total();

    let p = sys.take_payment(id, 10_000.0, &mut t, 31);
    assert!(!p.missed && p.paid > 0.0);
    assert!(p.interest > 0.0 && p.principal > 0.0);

    // **The principal is gone from the world; the interest merely moved.**
    let expected = after_lending - p.principal;
    assert!(
        (t.total() - expected).abs() < 1e-6,
        "the money supply fell by {:.2} against a principal repayment of {:.2}",
        after_lending - t.total(),
        p.principal
    );
    assert!(t.balance(Account::Bank(bank)) > 400_000.0, "the bank was not paid its interest");
    assert!((sys.destroyed - p.principal).abs() < 1e-6);
    sys.assert_balanced();
    t.assert_conserved();

    // **Early on it is nearly all interest**, which is why paying a
    // mortgage for five years barely dents it.
    let who = a_borrower(9_000.0, 0.85);
    let m = underwrite(sys.bank(bank), &sys.rates, &who, Credit::Mortgage, 300_000.0, 320_000.0)
        .unwrap();
    let mid = sys.advance(bank, Account::Households(0), &m, &mut t, 40, None);
    let first = sys.take_payment(mid, 99_000.0, &mut t, 70);
    assert!(
        first.interest > first.principal * 2.0,
        "the first mortgage payment was {:.0} interest against {:.0} principal",
        first.interest,
        first.principal
    );
}

/// **Gate: the interest was never created alongside the principal.**
///
/// The deepest consequence, and it falls out of the arithmetic rather than
/// being asserted anywhere. A closed economy where the only money is
/// borrowed cannot repay principal *and* interest out of what exists — so
/// somebody has to keep borrowing, or somebody has to default. Both happen.
#[test]
fn an_economy_of_pure_credit_cannot_pay_its_own_interest() {
    let mut sys = System::new(Rates::ordinary());
    let bank = sys.add_bank(500_000.0, 200_000.0);
    let mut t = Treasury::new();
    // **Nobody starts with anything.** Every penny in this world will have
    // been lent into existence.
    t.open(Account::Bank(bank), 500_000.0);
    let outside_the_banks = 0.0;
    assert!((t.balance(Account::Households(0)) - outside_the_banks).abs() < 1e-9);

    let who = Applicant {
        account: Account::Households(0),
        income: 5_000.0,
        existing_payments: 0.0,
        savings: 0.0,
        standing: 0.85,
        settled: true,
    };
    let offer = underwrite(sys.bank(bank), &sys.rates, &who, Credit::PersonalLoan, 40_000.0, 0.0)
        .expect("refused");
    let id = sys.advance(bank, Account::Households(0), &offer, &mut t, 1, None);

    let lent = offer.principal;
    let owed = offer.monthly * offer.months as f64;
    assert!(
        owed > lent,
        "borrowing 40,000 at 12% for four years repaid {owed:.0} against {lent:.0}"
    );

    // **The money to pay the interest does not exist.** It was never
    // created: the loan made the principal and nothing made the rest.
    let in_the_world = t.balance(Account::Households(0));
    assert!(
        (in_the_world - lent).abs() < 1e-6,
        "the borrower holds {in_the_world:.0} against a loan of {lent:.0}"
    );
    assert!(
        owed - lent > 1.0,
        "there was no interest to be short of"
    );

    // Pay it down until the money runs out, and it runs out before the
    // debt does.
    let mut months = 0;
    loop {
        let have = t.balance(Account::Households(0));
        let p = sys.take_payment(id, have, &mut t, 30 * (months + 1) as u64);
        if p.missed || p.cleared || months > 60 {
            break;
        }
        months += 1;
    }
    let still_owed = sys.loans.get(&id).map(|l| l.outstanding).unwrap_or(0.0);
    assert!(
        still_owed > 0.0,
        "a closed economy repaid a loan in full out of money that was never created"
    );
    t.assert_conserved();
}

// =====================================================================
// what actually limits it
// =====================================================================

/// **Gate: capital limits lending, not reserves.**
///
/// A bank stuffed with reserves and no capital cannot write a loan; one
/// with capital and modest reserves can. That is the opposite of the
/// money-multiplier story and it is what Basel III is actually written in.
#[test]
fn a_bank_runs_out_of_capital_before_it_runs_out_of_reserves() {
    let rates = Rates::ordinary();
    let who = a_borrower(20_000.0, 0.9);

    // Thin capital, enormous reserves.
    let mut thin = System::new(rates);
    let a = thin.add_bank(20_000.0, 5_000_000.0);
    // Lend it up to its capital limit.
    let big = Offer {
        kind: Credit::Mortgage,
        principal: 190_000.0,
        rate: 0.07,
        monthly: 1_264.0,
        months: 360,
        deposit_required: 0.0,
    };
    let mut t = Treasury::new();
    t.open(Account::Bank(a), 20_000.0);
    thin.advance(a, Account::Households(0), &big, &mut t, 1, None);
    assert!(thin.bank(a).reserves > 1_000_000.0, "the bank is short of reserves, not capital");
    assert_eq!(
        underwrite(thin.bank(a), &rates, &who, Credit::Mortgage, 200_000.0, 260_000.0),
        Err(Refused::NotEnoughCapital),
        "a bank at its capital limit with five million in reserves wrote another mortgage"
    );

    // Thick capital, modest reserves: it lends.
    let mut thick = System::new(rates);
    let b = thick.add_bank(400_000.0, 90_000.0);
    assert!(
        underwrite(thick.bank(b), &rates, &who, Credit::Mortgage, 200_000.0, 260_000.0).is_ok(),
        "a well capitalised bank could not lend for want of reserves"
    );

    // **And the multiple is an outcome, not a cause.** Nobody sets it.
    let mut sys = System::new(rates);
    let c = sys.add_bank(300_000.0, 400_000.0);
    let mut t = Treasury::new();
    t.open(Account::Bank(c), 300_000.0);
    let before = sys.observed_multiple();
    for k in 0..80 {
        let o = Offer {
            kind: Credit::CarLoan,
            principal: 25_000.0,
            rate: 0.08,
            monthly: 438.0,
            months: 72,
            deposit_required: 0.0,
        };
        sys.advance(c, Account::Households(k), &o, &mut t, 1, None);
    }
    assert!(sys.observed_multiple() > before, "lending did not change the ratio");
    assert!(
        sys.broad_money() > sys.base_money(),
        "two million of lending and there was still no more money than cash:          broad {:.0} against base {:.0}",
        sys.broad_money(),
        sys.base_money()
    );
    // Real US M2 against base money is about 3.75; nobody sets it, and it
    // is whatever profitable prudent lending produces.
    assert!(sys.observed_multiple() > 1.5);
    sys.assert_balanced();
}

/// **Gate: four different reasons to be turned down, and they are not
/// interchangeable.**
///
/// Being refused for want of a deposit is a different problem from being
/// refused for the record, and only one of them is fixable this year.
#[test]
fn no_is_not_one_answer() {
    let rates = Rates::ordinary();
    let mut sys = System::new(rates);
    let bank = sys.add_bank(2_000_000.0, 900_000.0);
    let b = sys.bank(bank);

    // Cannot service it: real underwriting caps debt-to-income at 43%.
    let poor = a_borrower(1_100.0, 0.8);
    assert_eq!(
        underwrite(b, &rates, &poor, Credit::Mortgage, 300_000.0, 320_000.0),
        Err(Refused::CannotAfford)
    );
    assert!(poor.debt_to_income(1_800.0) > MAX_DEBT_TO_INCOME);

    // Nothing down, on a mortgage that wants 3.5%.
    let no_deposit = Applicant { savings: 100.0, ..a_borrower(12_000.0, 0.8) };
    assert_eq!(
        underwrite(b, &rates, &no_deposit, Credit::Mortgage, 300_000.0, 300_000.0),
        Err(Refused::NotEnoughSecurity)
    );

    // A record nobody will touch.
    let bad_record = a_borrower(12_000.0, 0.12);
    assert_eq!(
        underwrite(b, &rates, &bad_record, Credit::CarLoan, 20_000.0, 22_000.0),
        Err(Refused::NoCreditworthiness)
    );

    // **And no address is a refusal on its own**, which is one more way
    // being homeless is self-sustaining.
    let nowhere = Applicant { settled: false, ..a_borrower(12_000.0, 0.9) };
    assert_eq!(
        underwrite(b, &rates, &nowhere, Credit::PersonalLoan, 5_000.0, 0.0),
        Err(Refused::NoCreditworthiness)
    );
    // The pawnbroker does not care where you live, which is exactly the
    // gap he fills.
    assert!(underwrite(b, &rates, &nowhere, Credit::Pawn, 200.0, 800.0).is_ok());
}

// =====================================================================
// what it costs, and who pays it
// =====================================================================

/// **Gate: the poor pay more, and it is not a small difference.**
///
/// Real subprime auto finance runs 15-21% against about 7% for prime, so
/// the same car costs half as much again over the term. That is the trap in
/// numbers rather than in sentiment.
#[test]
fn the_same_car_costs_the_poor_man_far_more() {
    let rates = Rates::ordinary();
    let mut sys = System::new(rates);
    let bank = sys.add_bank(3_000_000.0, 1_000_000.0);
    let b = sys.bank(bank);

    let prime = underwrite(b, &rates, &a_borrower(6_000.0, 0.95), Credit::UsedCarLoan, 18_000.0, 18_000.0)
        .expect("a prime borrower was refused");
    let subprime = underwrite(b, &rates, &a_borrower(6_000.0, 0.35), Credit::UsedCarLoan, 18_000.0, 18_000.0)
        .expect("a subprime borrower was refused");

    assert!(subprime.rate > prime.rate * 1.4, "prime {:.1}% subprime {:.1}%", prime.rate * 100.0, subprime.rate * 100.0);
    assert!(
        (0.13..0.24).contains(&subprime.rate),
        "subprime used car finance came out at {:.1}%",
        subprime.rate * 100.0
    );
    assert!(
        subprime.monthly > prime.monthly * 1.12,
        "the same car cost {:.0} a month against {:.0}",
        subprime.monthly,
        prime.monthly
    );

    // **And a mortgage repays about two and a half times what was
    // borrowed**, which is the number nobody looks at.
    let m = underwrite(b, &rates, &a_borrower(11_000.0, 0.9), Credit::Mortgage, 300_000.0, 340_000.0)
        .unwrap();
    assert!(
        (2.0..3.0).contains(&m.times_over()),
        "a thirty-year mortgage repaid {:.2} times the loan",
        m.times_over()
    );

    // **A payday lender is not on the same ladder.** Real APRs are around
    // 400%.
    assert!(rates.quoted(Credit::Payday, 0.5) > 3.0);
    assert!(
        rates.quoted(Credit::Payday, 0.5) > rates.quoted(Credit::Mortgage, 0.9) * 40.0,
        "the man with nothing paid less than forty times what the homeowner did"
    );
}

/// **Gate: losing the car does not clear the debt.**
///
/// The part people do not expect. Real repossessed cars fetch about half
/// the outstanding balance at auction, so the borrower loses the car *and*
/// owes the shortfall — and the shortfall comes out of the bank's capital,
/// which is what capital is for.
#[test]
fn they_take_the_car_and_you_still_owe_the_difference() {
    let (mut sys, mut t, bank) = a_system();
    let who = a_borrower(4_500.0, 0.6);
    let offer =
        underwrite(sys.bank(bank), &sys.rates, &who, Credit::CarLoan, 26_000.0, 26_000.0).unwrap();
    let id = sys.advance(bank, Account::Households(0), &offer, &mut t, 1, None);

    // Three missed payments and a car lender has had enough.
    for m in 1..=3 {
        let p = sys.take_payment(id, 0.0, &mut t, 30 * m);
        assert!(p.missed);
        if m < 3 {
            assert!(!p.defaulted, "a car was repossessed after {m} missed payments");
        }
    }
    assert!(sys.loans[&id].defaulted());

    let capital_before = sys.bank(bank).capital;
    // Auction: about half the balance, which is the real figure.
    let f = sys.foreclose(id, 12_500.0, 120);
    assert!(f.took_the_security);
    assert!(f.shortfall > 5_000.0, "the auction covered all but {:.0}", f.shortfall);
    assert!(f.still_owed > 0.0, "losing the car cleared the debt");
    assert!(
        sys.bank(bank).capital < capital_before,
        "a write-off cost the shareholders nothing"
    );
    assert!((sys.bank(bank).written_off - f.shortfall).abs() < 1e-6);
    sys.assert_balanced();
    t.assert_conserved();
}

/// **Gate: enough bad debt and the bank is gone — and it is illiquid
/// first.**
///
/// Which is how banks actually fail. A bank with sound loans and no
/// reserves cannot meet its withdrawals, and that is what a run is;
/// insolvency is a slower and separate thing.
#[test]
fn a_bank_is_illiquid_before_it_is_insolvent() {
    let mut sys = System::new(Rates::ordinary());
    let bank = sys.add_bank(120_000.0, 60_000.0);
    let mut t = Treasury::new();
    t.open(Account::Bank(bank), 120_000.0);

    assert!(!sys.bank(bank).insolvent() && !sys.bank(bank).illiquid());

    // Write a book of loans, then have them all go bad.
    let mut ids = Vec::new();
    for k in 0..4 {
        let o = Offer {
            kind: Credit::PersonalLoan,
            principal: 90_000.0,
            rate: 0.12,
            monthly: 2_370.0,
            months: 48,
            deposit_required: 0.0,
        };
        ids.push(sys.advance(bank, Account::Households(k), &o, &mut t, 1, None));
    }
    assert!(sys.bank(bank).loans > 300_000.0);

    for id in &ids {
        sys.foreclose(*id, 0.0, 200);
    }
    assert!(
        sys.bank(bank).insolvent(),
        "360,000 of unsecured loans went bad and the bank had capital left: {:.0}",
        sys.bank(bank).capital
    );
    sys.assert_balanced();

    // **Illiquidity is the other failure and it comes first**, and the only
    // way to produce it honestly is to let reserves actually leave — which
    // is the one sense in which reserves constrain a bank at all. A
    // one-bank world can never be illiquid, because the money it creates
    // has nowhere else to go.
    let mut solid = System::new(Rates::ordinary());
    let big = solid.add_bank(300_000.0, 400_000.0);
    let rival = solid.add_bank(300_000.0, 400_000.0);
    let mut t2 = Treasury::new();
    t2.open(Account::Bank(big), 300_000.0);
    for k in 0..12 {
        let o = Offer {
            kind: Credit::Mortgage,
            principal: 200_000.0,
            rate: 0.07,
            monthly: 1_330.0,
            months: 360,
            deposit_required: 0.0,
        };
        solid.advance(big, Account::Households(k), &o, &mut t2, 1, None);
    }
    assert!(!solid.bank(big).illiquid(), "it was short before anybody spent anything");
    assert!(!solid.bank(big).insolvent(), "a bank with a good mortgage book was insolvent");

    // **Every borrower spends the money at somebody who banks elsewhere**,
    // which is what a mortgage is *for*, and the reserves go with it.
    for _ in 0..12 {
        solid.settle(big, rival, 200_000.0);
    }
    assert!(
        !solid.bank(big).insolvent(),
        "the loans were sound and the bank was declared insolvent anyway"
    );
    assert!(
        solid.bank(big).illiquid(),
        "it lent out 2.4M, paid it all away, and still had reserves: {:.0} against {:.0} of deposits",
        solid.bank(big).reserves,
        solid.bank(big).deposits
    );
    solid.assert_balanced();

    // **And a run is the same thing at the counter.** Sound loans, and the
    // till is empty by the afternoon — which is what happened to Silicon
    // Valley Bank in 2023, $42bn in a single day, with a loan book that was
    // not the problem.
    let mut run = System::new(Rates::ordinary());
    let r = run.add_bank(150_000.0, 200_000.0);
    let mut t3 = Treasury::new();
    t3.open(Account::Bank(r), 150_000.0);
    for k in 0..5 {
        let o = Offer {
            kind: Credit::Mortgage,
            principal: 200_000.0,
            rate: 0.07,
            monthly: 1_330.0,
            months: 360,
            deposit_required: 0.0,
        };
        run.advance(r, Account::Households(k), &o, &mut t3, 1, None);
    }
    let till = run.bank(r).reserves;
    let owed = run.bank(r).deposits;
    assert!(owed > till * 3.0, "the bank was holding most of its deposits in cash");
    assert!(!run.bank(r).insolvent(), "the mortgage book was sound");

    let asked_for = owed;
    let got = run.withdraw(r, asked_for);
    assert!(
        got < asked_for,
        "every depositor was paid in full out of a till holding a third of it"
    );
    assert!(
        (got - till).abs() < 1e-6,
        "the till paid out {got:.0} with {till:.0} in it"
    );
    assert!(run.bank(r).reserves < 1e-6, "there was money left after a run cleaned it out");
    assert!(run.bank(r).illiquid());
    run.assert_balanced();
}

// =====================================================================
// and the books
// =====================================================================

/// **Gate: a bank's balance sheet always balances, and the money supply is
/// the opening stock plus everything lent less everything repaid.**
///
/// The invariant `money.rs` used to state — the total never moves — was
/// only ever true because credit did not exist. This is the stronger
/// version, and it still fails the instant somebody reaches past a named
/// door.
#[test]
fn the_books_close_through_a_whole_lending_cycle() {
    let (mut sys, mut t, bank) = a_system();
    let opening = t.total();

    let mut ids = Vec::new();
    for k in 0..6u64 {
        let who = a_borrower(5_500.0, 0.75);
        let Ok(o) =
            underwrite(sys.bank(bank), &sys.rates, &who, Credit::CarLoan, 22_000.0, 22_000.0)
        else {
            break;
        };
        ids.push(sys.advance(bank, Account::Households(k as usize), &o, &mut t, 1, None));
        sys.assert_balanced();
        t.assert_conserved();
    }
    assert!(ids.len() >= 3, "only {} loans were written", ids.len());

    // A year of payments, with one borrower falling behind.
    for month in 1..=12u64 {
        for (n, id) in ids.iter().enumerate() {
            if !sys.loans.contains_key(id) {
                continue;
            }
            let available = if n == 0 && month > 6 { 0.0 } else { 9_999.0 };
            sys.take_payment(*id, available, &mut t, month * 30);
            sys.assert_balanced();
            t.assert_conserved();
        }
        sys.credit_interest(bank, &mut t, month * 30);
        sys.assert_balanced();
    }

    // The one who stopped paying loses the car.
    if sys.loans.contains_key(&ids[0]) {
        sys.foreclose(ids[0], 9_000.0, 400);
    }
    sys.assert_balanced();
    t.assert_conserved();

    // **And the arithmetic is checkable by hand.**
    let should_be = opening + t.created - t.destroyed;
    assert!((t.total() - should_be).abs() < 1e-6);
    assert!(t.created > 0.0 && t.destroyed > 0.0);
    assert!((sys.created - t.created).abs() < 1e-6, "the bank and the treasury disagree");

    // **A bank makes money by lending its deposits out, and loses on the
    // ones it does not.** Real US banks run a loan-to-deposit ratio around
    // 70%; a regulator worries above 90% because there is nothing left to
    // meet withdrawals with.
    let ltd = sys.loan_to_deposit(bank);
    assert!(ltd > 0.2, "the bank had lent only {:.0}% of its deposits", ltd * 100.0);
    let nim = sys.net_interest_margin(bank);
    assert!(
        nim > 0.0,
        "the bank lent {:.0}% of its deposits for a year and still lost money: {nim:.4}",
        ltd * 100.0
    );
}

/// **Gate: an amortisation schedule is the real formula.**
#[test]
fn the_payment_is_the_one_a_bank_would_quote() {
    // A textbook case that can be checked by hand: 200,000 at 6% over 30
    // years is 1,199.10 a month.
    let m = Loan::level_payment(200_000.0, 0.06, 360);
    assert!((m - 1_199.10).abs() < 0.5, "came out at {m:.2} against 1,199.10");

    // 25,000 over 5 years at 7.2% is 497.62.
    let car = Loan::level_payment(25_000.0, 0.072, 60);
    assert!((car - 497.62).abs() < 0.5, "came out at {car:.2} against 497.62");

    // Zero interest divides evenly, and a zero term asks for nothing.
    assert!((Loan::level_payment(1_200.0, 0.0, 12) - 100.0).abs() < 1e-9);
    assert_eq!(Loan::level_payment(1_200.0, 0.05, 0), 0.0);

    // A longer term is a smaller payment and **more money altogether**,
    // which is the trade every borrower is offered and most take.
    let five = Loan::level_payment(30_000.0, 0.08, 60) * 60.0;
    let seven = Loan::level_payment(30_000.0, 0.08, 84) * 84.0;
    assert!(seven > five, "stretching the term cost nothing extra");
}
