//! **Every bill has a funding path, and every unpaid bill is owed until it
//! is paid or charged off.**
//!
//! What this replaces: `Treasury::pay` paid what a payer held and put the
//! rest on a tally that was wiped every morning. A town that could not pay
//! its builders had had the work done for nothing by the next day, a state
//! that did not fund its share of a hospital bill simply did not owe it,
//! and money conserved throughout — which is how books can balance while
//! the transactions are wrong.
//!
//! These gates hold the households, the states and the insurers to the
//! rule. **Firms are not yet held to it**: goods between works, carriage,
//! wages and a works' own power still fall on the day's tally when a buyer
//! cannot pay, and that is the next piece rather than something these
//! gates pretend to cover.
//!
//! Each gate was checked by breaking the thing it names.

use scale_sim::credit::Origin;
use scale_sim::econ::{Commodity, Doctrine, Economy};
use scale_sim::money::{Account, Why};
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Region;
use scale_sim::save::{Reader, Store, Writer};
use scale_sim::state::{Capacity, HealthSystem};

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

fn bytes_of(e: &Economy) -> Vec<u8> {
    let mut w = Writer::new();
    e.store(&mut w);
    w.bytes
}

fn reload(bytes: &[u8]) -> Economy {
    Economy::load(&mut Reader::new(bytes)).expect("the economy would not load back")
}

/// **Empty a town's purse into its neighbour's**, so its bills cannot be
/// paid and no money leaves the country or appears from nowhere.
fn empty(e: &mut Economy, town: usize, into: usize) {
    let held = e.treasury.balance(Account::Households(town));
    let day = e.ledger.day;
    e.treasury.pay(
        day,
        Account::Households(town),
        Account::Households(into),
        held,
        Why::Purchase,
    );
}

/// The obligations one debtor took on today: key, creditor, origin, amount.
fn raised_today(
    e: &Economy,
    debtor: Account,
) -> Vec<(
    scale_sim::registry::Key<scale_sim::credit::Invoice>,
    Account,
    Origin,
    f64,
)> {
    let day = e.ledger.day;
    e.obligations
        .iter()
        .filter(|(_, i)| i.debtor == debtor && i.raised == day)
        .map(|(k, i)| (k, i.creditor, i.origin, i.amount))
        .collect()
}

/// **A bill that cannot be paid is owed, by a named debtor to a named
/// creditor for a named bill, until it is paid.**
///
/// A town is emptied before its bills come in. What it could not pay must
/// be on the book with the terms it was left on; a second day's bills must
/// not wipe the first day's debts; a reload must bring every one of them
/// back; and when the town has money again and the debts fall due, the
/// payments must reduce **those same obligations**, not raise new ones.
#[test]
fn a_bill_that_cannot_be_paid_is_owed_until_it_is() {
    let mut e = a_nation().economy;
    for gov in e.governments.values_mut() {
        gov.health = HealthSystem::PrivateInsurance;
    }
    for _ in 0..30 {
        e.step();
    }
    let town = Account::Households(0);

    // Day one: nothing in the purse when the bills arrive.
    empty(&mut e, 0, 1);
    e.step();
    let first = raised_today(&e, town);
    assert!(
        !first.is_empty(),
        "an empty town was billed and owes nothing — its bills were forgotten"
    );
    let day_one = e.ledger.day;
    for &(key, creditor, origin, amount) in first.iter() {
        let inv = e
            .obligations
            .get(key)
            .expect("an obligation is not on the book");
        assert_eq!(
            inv.due,
            day_one + Economy::DAYS_TO_PAY,
            "an obligation for {} was left on terms it was not given",
            origin.name()
        );
        let billed = e.bills_today[&(town, creditor, origin)];
        assert!(
            (billed.paid + billed.owed - billed.amount).abs() <= 1e-9 * billed.amount.max(1.0),
            "a {} bill of {:.3e} was paid {:.3e} and owed {:.3e}",
            origin.name(),
            billed.amount,
            billed.paid,
            billed.owed
        );
        assert!(
            (billed.owed - amount).abs() <= 1e-9 * amount.max(1.0),
            "the book says {} is owed for {} and the bill says {}",
            amount,
            origin.name(),
            billed.owed
        );
    }
    let origins: std::collections::BTreeSet<Origin> = first.iter().map(|x| x.2).collect();
    assert!(
        origins.contains(&Origin::Services) && origins.contains(&Origin::HealthPremium),
        "an empty town owed for {:?} and not for its services and its premium",
        origins
    );

    // Day two: new bills must not wipe the old debts.
    empty(&mut e, 0, 1);
    e.step();
    for &(key, _, origin, amount) in first.iter() {
        let inv = e.obligations.get(key).unwrap_or_else(|| {
            panic!(
                "yesterday's {} debt disappeared when today's bill was calculated",
                origin.name()
            )
        });
        assert!(
            (inv.outstanding() - amount).abs() <= 1e-9 * amount.max(1.0),
            "yesterday's {} debt of {amount:.3e} is now {:.3e} with nothing paid",
            origin.name(),
            inv.outstanding()
        );
    }
    assert!(
        !raised_today(&e, town).is_empty(),
        "the second day's unpaid bills were not owed"
    );

    // A reload brings back every obligation, as it was, and the two
    // branches go on the same way — which is where a field lost on the way
    // through would show.
    let bytes = bytes_of(&e);
    let mut back = reload(&bytes);
    let listed = |x: &Economy| -> Vec<(u64, Account, Account, Origin, u64, u64, u64)> {
        x.obligations
            .iter()
            .map(|(k, i)| {
                (
                    k.number(),
                    i.debtor,
                    i.creditor,
                    i.origin,
                    i.raised,
                    i.due,
                    i.outstanding().to_bits(),
                )
            })
            .collect()
    };
    assert_eq!(
        listed(&e),
        listed(&back),
        "the obligations did not come back from a save as they went in"
    );

    // Now the town keeps its money, and the debts fall due.
    for _ in 0..(Economy::DAYS_TO_PAY + 2) {
        e.step();
        back.step();
    }
    assert_eq!(
        bytes_of(&e),
        bytes_of(&back),
        "a reloaded world went on differently from the one it was saved from"
    );
    let mut reduced = 0;
    for &(key, creditor, origin, amount) in first.iter() {
        // **The same obligation**, found by the bill it arose from — a
        // payment that raised another would leave this one where it was.
        assert_eq!(
            e.obligations.find(town, creditor, origin, day_one),
            e.obligations.get(key).map(|_| key),
            "a {} bill is on the book under a different name",
            origin.name()
        );
        let left = e
            .obligations
            .get(key)
            .map(|i| i.outstanding())
            .unwrap_or(0.0);
        if left < amount * (1.0 - 1e-9) {
            reduced += 1;
        }
    }
    assert!(
        reduced > 0,
        "the town had money again and its debts fell due, and not one was paid down"
    );
}

/// **Every bill presented today was paid, owed, or both — and nothing a
/// household, a state or an insurer failed to pay fell on the day's
/// tally.**
///
/// The stressed case: a weak state with nothing put by, and a town emptied
/// every few days, over long enough for the state's arrears and the town's
/// debts to pile up and fall due.
#[test]
fn every_bill_has_a_funding_path() {
    let mut e = a_nation().economy;
    for gov in e.governments.values_mut() {
        gov.health = HealthSystem::SocialInsurance;
        gov.capacity = Capacity::Weak;
    }
    let n = e.nations()[0];
    let held = e.treasury.balance(Account::State(n));
    let day = e.ledger.day;
    e.treasury.pay(
        day,
        Account::State(n),
        Account::Households(1),
        held,
        Why::PublicSpending,
    );
    let mut owed_bills = 0;
    let mut state_owed = 0;
    for d in 0..120 {
        if d % 7 == 0 {
            empty(&mut e, 0, 1);
        }
        e.step();
        let today = e.ledger.day;
        for (&(debtor, creditor, origin), b) in e.bills_today.iter() {
            assert!(
                (b.paid + b.owed - b.amount).abs() <= 1e-9 * b.amount.max(1.0),
                "day {d}: a {} bill from {debtor:?} to {creditor:?} of {:.6e} was paid \
                 {:.6e} and owed {:.6e}",
                origin.name(),
                b.amount,
                b.paid,
                b.owed
            );
            if b.owed > 1e-9 {
                owed_bills += 1;
                if matches!(debtor, Account::State(_)) {
                    state_owed += 1;
                }
                let on_book = e
                    .obligations
                    .find(debtor, creditor, origin, today)
                    .and_then(|k| e.obligations.get(k))
                    .map(|i| i.amount)
                    .unwrap_or(0.0);
                assert!(
                    (on_book - b.owed).abs() <= 1e-9 * b.owed.max(1.0),
                    "day {d}: {:.6e} of a {} bill was owed and {:.6e} is on the book",
                    b.owed,
                    origin.name(),
                    on_book
                );
            }
        }
        for (&(from, to, why), &v) in e.treasury.unpaid_by.iter() {
            let held_to_it = match from {
                Account::Households(_) | Account::State(_) => true,
                Account::ServiceSector(_) => why != "payroll",
                _ => false,
            };
            assert!(
                !held_to_it || v <= 1e-9,
                "day {d}: {from:?} failed to pay {to:?} {v:.6e} for {why} and it was \
                 counted and forgotten rather than owed"
            );
        }
    }
    assert!(
        owed_bills > 0 && state_owed > 0,
        "nothing went unpaid ({owed_bills} bills, {state_owed} of the state's) — the rule \
         was never exercised"
    );
}

/// **Known bills come before the discretionary shopping, and the necessities
/// are not cut for them.**
///
/// The same country twice from the same bytes, one of them with a bill of
/// the town's whole purse falling due tomorrow. With the bill, the town must
/// buy less of the goods whose spending rises faster than income, the same
/// food, meat and remedies, and must pay the bill.
#[test]
fn known_bills_come_before_the_discretionary_shopping() {
    let mut e = a_nation().economy;
    for _ in 0..30 {
        e.step();
    }
    let bytes = bytes_of(&e);
    let mut plain = reload(&bytes);
    let mut billed = reload(&bytes);
    let town = 0;
    let purse = billed.treasury.balance(Account::Households(town));
    let day = billed.ledger.day;
    let key = billed
        .obligations
        .bill(
            day,
            Account::Households(town),
            Account::ServiceSector(town),
            purse,
            1,
            Why::Purchase,
            Origin::Services,
        )
        .expect("no bill");
    plain.step();
    billed.step();

    let goods = Commodity::RetailGoods as usize;
    assert!(
        billed.went_without[goods] > plain.went_without[goods] * 1.05 + 1e-9,
        "with a bill of its whole purse due, the town went without {:.4e} of goods against \
         {:.4e} with none — the goods were bought before the bill",
        billed.went_without[goods],
        plain.went_without[goods]
    );
    assert!(
        billed.held_back_today > 0.0,
        "nothing was held back for a bill the town knew was due"
    );
    for c in [
        Commodity::ProcessedFood,
        Commodity::Meat,
        Commodity::Remedies,
    ] {
        let (a, b) = (
            plain.went_without[c as usize],
            billed.went_without[c as usize],
        );
        assert!(
            (a - b).abs() <= 1e-9 * a.abs().max(1.0),
            "{c:?}: the town went without {b:.6e} with the bill due and {a:.6e} without — a \
             necessity was cut to pay a bill"
        );
    }
    let left = billed
        .obligations
        .get(key)
        .map(|i| i.outstanding())
        .unwrap_or(0.0);
    assert!(
        left < purse * 0.5,
        "the town held money back for a bill and then did not pay it: {left:.3e} of \
         {purse:.3e} still owed"
    );
}

/// **A state with no borrowing facility can still be in debt.** Nobody lends
/// to a state here, so its cash never goes below nought; but a state that
/// cannot raise what its hospitals bill owes them the rest, and the debt is
/// on the book and growing — not a balance that looks flat.
#[test]
fn a_state_in_arrears_is_in_debt_and_not_overdrawn() {
    let mut e = a_nation().economy;
    for gov in e.governments.values_mut() {
        gov.health = HealthSystem::TaxFunded;
        gov.capacity = Capacity::Weak;
    }
    let n = e.nations()[0];
    let state = Account::State(n);
    let held = e.treasury.balance(state);
    let day = e.ledger.day;
    e.treasury.pay(
        day,
        state,
        Account::Households(0),
        held,
        Why::PublicSpending,
    );
    let mut owed_at = Vec::new();
    for d in 0..120 {
        e.step();
        let cash = e.treasury.balance(state);
        assert!(
            cash >= 0.0,
            "day {d}: the state holds {cash:.3e} — it spent money nobody lent it"
        );
        owed_at.push(e.obligations.owed_by(state));
    }
    let (early, late) = (owed_at[29], owed_at[119]);
    assert!(
        late > 0.0 && late > early,
        "a state that cannot raise its hospital bills owed {early:.3e} on day 30 and \
         {late:.3e} on day 120 — its distress is not on the book"
    );
    // **And it is owed to its hospitals**, not to nobody.
    for (_, inv) in e.obligations.iter().filter(|(_, i)| i.debtor == state) {
        assert_eq!(inv.origin, Origin::Care);
        assert!(matches!(inv.creditor, Account::Firm(_)));
    }
}

/// **A household's debt left unpaid for half a year past due is charged off
/// — as an event, dated and recorded by kind — and nothing else ends an
/// obligation but paying it.**
#[test]
fn an_unpaid_debt_is_charged_off_on_the_record() {
    let mut e = a_nation().economy;
    for _ in 0..10 {
        e.step();
    }
    let town = Account::Households(0);
    empty(&mut e, 0, 1);
    e.step();
    let first = raised_today(&e, town);
    assert!(!first.is_empty(), "an empty town owed nothing");
    let raised = e.ledger.day;
    // Keep it empty until well past the policy's horizon.
    let horizon = Economy::DAYS_TO_PAY + scale_sim::credit::Book::CHARGE_OFF_AFTER;
    for _ in 0..horizon {
        empty(&mut e, 0, 1);
        e.step();
        assert!(
            e.ledger.day > raised + horizon
                || first.iter().all(|&(k, ..)| e.obligations.get(k).is_some()),
            "a debt left the book on day {} before the charge-off horizon",
            e.ledger.day
        );
    }
    empty(&mut e, 0, 1);
    e.step();
    for &(k, _, origin, _) in first.iter() {
        match e.obligations.look(k) {
            scale_sim::registry::Lookup::Gone(stone) => assert_eq!(
                stone.how,
                "charged off",
                "a {} debt ended as '{}' when nothing paid it",
                origin.name(),
                stone.how
            ),
            other => panic!(
                "a {} debt unpaid {} days past due is still {:?}",
                origin.name(),
                horizon,
                other
            ),
        }
    }
    assert!(e.obligations.charged_off > 0.0);
    assert!(
        e.obligations
            .charged_off_by
            .get(&Origin::Services)
            .copied()
            .unwrap_or(0.0)
            > 0.0,
        "the charge-offs were not recorded by the kind of bill"
    );
    e.obligations.assert_conserved();
}

/// **A bill presented twice in one day is billed once.** A retried payment,
/// or a phase run twice, must not pay twice or owe twice.
#[test]
fn a_retried_bill_is_not_billed_twice() {
    let mut e = a_nation().economy;
    for _ in 0..5 {
        e.step();
    }
    let town = Account::Households(0);
    let purse = e.treasury.balance(town);
    // Not a bill the day itself raises, so the retry is ours alone: a farm
    // is never billed for building work.
    let farm = e
        .ledger
        .sites
        .iter()
        .position(|s| s.kind == scale_sim::econ::SiteKind::Farm)
        .expect("no farm");
    let creditor = Account::Firm(farm);
    let first = e.charge(town, creditor, purse * 3.0, Why::Purchase, Origin::Upkeep);
    let after_one = (
        e.treasury.balance(town),
        e.obligations.billed,
        e.obligations.len(),
    );
    let second = e.charge(town, creditor, purse * 3.0, Why::Purchase, Origin::Upkeep);
    let after_two = (
        e.treasury.balance(town),
        e.obligations.billed,
        e.obligations.len(),
    );
    assert_eq!(first, second, "a retry did not report the bill it retried");
    assert!(
        first.owed > 0.0,
        "nothing was owed, so the retry proved nothing"
    );
    assert_eq!(
        after_one, after_two,
        "presenting the same bill again paid or owed it again"
    );
}

/// **The book accounts for every penny billed**: collected, still owed, or
/// charged off, and nothing else. A save whose book does not add up is
/// refused rather than loaded.
#[test]
fn a_book_that_does_not_add_up_is_refused() {
    let mut book = scale_sim::credit::Book::new();
    let a = Account::Households(0);
    let b = Account::ServiceSector(0);
    let k = book
        .bill(3, a, b, 120.0, 30, Why::Purchase, Origin::Services)
        .expect("no bill");
    book.settle(k, 20.0);
    book.bill(
        4,
        a,
        Account::Firm(2),
        55.5,
        30,
        Why::Purchase,
        Origin::Upkeep,
    );
    let gone = book
        .bill(1, a, Account::Firm(3), 7.25, 0, Why::Purchase, Origin::Care)
        .expect("no bill");
    book.charge_off(gone, 300);
    book.assert_conserved();

    let mut w = Writer::new();
    book.store(&mut w);
    let back = scale_sim::credit::Book::load(&mut Reader::new(&w.bytes)).expect("would not load");
    let mut w2 = Writer::new();
    back.store(&mut w2);
    assert_eq!(
        w.bytes, w2.bytes,
        "a book changed on the way through the bytes"
    );
    assert_eq!(back.charged_off, 7.25);
    assert_eq!(back.owed_by(a), 100.0 + 55.5);
    // The retry index is rebuilt: the same bill, presented again after a
    // reload, is the same obligation.
    let mut back = back;
    let again = back.bill(3, a, b, 120.0, 30, Why::Purchase, Origin::Services);
    assert_eq!(
        again,
        Some(k),
        "after a reload, a retried bill was owed again"
    );

    // A book billed for more than it accounts for: an obligation that left
    // it without being paid or charged off.
    let mut w3 = Writer::new();
    let mut doctored = scale_sim::credit::Book::load(&mut Reader::new(&w.bytes)).unwrap();
    doctored.billed += 1_000.0;
    doctored.store(&mut w3);
    assert!(
        scale_sim::credit::Book::load(&mut Reader::new(&w3.bytes)).is_err(),
        "a book billed for a thousand it cannot account for was loaded"
    );
}

/// **The codes are a bijection and an unknown code is refused.**
#[test]
fn every_kind_of_bill_has_its_own_name_on_disk() {
    let mut seen = std::collections::BTreeSet::new();
    for o in Origin::ALL {
        assert!(
            seen.insert(o.code()),
            "two kinds of bill share code {}",
            o.code()
        );
        assert_eq!(Origin::from_code(o.code()), Some(o));
    }
    assert_eq!(Origin::from_code(0), None);
    assert_eq!(Origin::from_code(200), None);
}
