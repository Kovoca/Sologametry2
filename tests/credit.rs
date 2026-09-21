//! **Every delivery has a commercial outcome: paid, owed, or refused.**
//!
//! What this replaces moved the goods and capped the payment at whatever
//! the buyer held, putting the shortfall on a counter. Nothing recorded a
//! receivable, so the supplier was simply poorer and the conservation
//! assertion was satisfied throughout — money conserving while the
//! transactions were wrong.
//!
//! **A lower unpaid counter would not establish success.** What these
//! gates ask is whether deliveries now have valid terms, whether
//! obligations settle correctly, and whether failing to pay has a
//! consequence rather than buying unlimited supply.

use scale_sim::credit::{Book, Refusal, Terms};
use scale_sim::money::{Account, Why};

const BUYER: Account = Account::Firm(1);
const SELLER: Account = Account::Firm(2);
const OTHER: Account = Account::Firm(3);

/// **A purchase without cash or authorised credit cannot move goods it
/// cannot pay for.**
///
/// This is the defect stated as a rule. The old path moved everything and
/// recorded a shortfall; here what cannot be paid for or borrowed is
/// **refused**, and the caller sizes the delivery on what came back.
#[test]
fn nothing_moves_that_is_neither_paid_for_nor_lent() {
    let book = Book::new();

    // Cash covers it: no credit needed and nothing refused.
    let flush = book.what_can_be_bought(0, BUYER, SELLER, 100.0, 250.0, Terms::net_30(500.0));
    assert_eq!(flush.paid_now, 100.0);
    assert_eq!(flush.on_credit, 0.0);
    assert_eq!(flush.refused, 0.0);
    assert_eq!(flush.share_of(100.0), 1.0);

    // Some cash and room on the account: the rest goes on the book.
    let part = book.what_can_be_bought(0, BUYER, SELLER, 100.0, 40.0, Terms::net_30(500.0));
    assert_eq!(part.paid_now, 40.0);
    assert_eq!(part.on_credit, 60.0);
    assert_eq!(part.refused, 0.0);

    // No cash and a supplier who gives no credit: nothing moves.
    let broke = book.what_can_be_bought(0, BUYER, SELLER, 100.0, 0.0, Terms::cash_only());
    assert_eq!(broke.paid_now, 0.0);
    assert_eq!(broke.on_credit, 0.0);
    assert_eq!(broke.refused, 100.0);
    assert_eq!(broke.why, Refusal::NoCash);
    assert_eq!(
        broke.share_of(100.0),
        0.0,
        "a buyer with no cash and no credit took delivery of something"
    );
}

/// **Two invoices between the same pair keep their own due dates.**
///
/// The reason the record is per invoice rather than one balance per pair.
/// A hundred on day 0 net 30 and two hundred on day 20 net 50 are *one
/// hundred* due on day 30 — and "three hundred owed" cannot say so.
#[test]
fn two_invoices_between_one_pair_keep_different_due_dates() {
    let mut book = Book::new();
    let first = book
        .raise(0, BUYER, SELLER, 100.0, Terms::net_30(1_000.0), Why::Supply)
        .expect("no invoice");
    let second = book
        .raise(
            20,
            BUYER,
            SELLER,
            200.0,
            Terms {
                days: 30,
                limit: 1_000.0,
            },
            Why::Supply,
        )
        .expect("no invoice");

    assert_eq!(book.get(first).unwrap().due, 30);
    assert_eq!(book.get(second).unwrap().due, 50);
    assert_eq!(book.between(BUYER, SELLER), 300.0);

    // **Outstanding is not overdue**, and on day 31 only the first is.
    assert_eq!(book.overdue_by(BUYER, 25), 0.0, "nothing is due yet");
    assert_eq!(
        book.overdue_by(BUYER, 31),
        100.0,
        "on day 31 the first invoice is overdue and the second is not"
    );
    assert_eq!(book.overdue_by(BUYER, 51), 300.0, "by day 51 both are");
    assert_eq!(book.owed_by(BUYER), 300.0);
    assert_eq!(book.owed_to(SELLER), 300.0);
    assert_eq!(book.owed_by(SELLER), 0.0, "the seller owes nothing");
}

/// **A partial payment reduces the right balance, exactly once.**
///
/// And it is not a second sale: what was billed and what was collected
/// are reported apart, because adding a collection to revenue is how a
/// credit model double-counts its own turnover.
#[test]
fn a_partial_payment_reduces_the_right_balance_exactly_once() {
    let mut book = Book::new();
    let key = book
        .raise(0, BUYER, SELLER, 100.0, Terms::net_30(1_000.0), Why::Supply)
        .expect("no invoice");
    let also = book
        .raise(0, BUYER, OTHER, 500.0, Terms::net_30(1_000.0), Why::Supply)
        .expect("no invoice");

    assert_eq!(book.settle(key, 30.0), 30.0);
    assert_eq!(book.get(key).unwrap().outstanding(), 70.0);
    assert_eq!(
        book.get(also).unwrap().outstanding(),
        500.0,
        "paying one supplier reduced what was owed to another"
    );
    assert_eq!(book.owed_by(BUYER), 570.0);

    // **Overpaying applies only what is outstanding.** A payment cannot
    // take a balance negative, and it cannot be counted twice.
    assert_eq!(
        book.settle(key, 999.0),
        70.0,
        "more was applied than was owed"
    );
    assert_eq!(book.get(key).unwrap().outstanding(), 0.0);
    assert_eq!(
        book.settle(key, 50.0),
        0.0,
        "a settled invoice took a payment"
    );

    assert_eq!(book.billed, 600.0);
    assert_eq!(
        book.collected, 100.0,
        "billed and collected are not one figure"
    );
    assert_eq!(
        book.get(key).unwrap().amount,
        100.0,
        "the original sale moved"
    );
    book.assert_conserved();
}

/// **Writing a debt down is the creditor's loss, not the debtor's
/// release.**
///
/// A supplier deciding it will never see the money reduces what the
/// supplier expects to collect. It does not reduce what the customer
/// owes, and a model in which it does has invented a way to settle a
/// bill by being unreliable enough.
#[test]
fn giving_up_on_a_debt_does_not_forgive_it() {
    let mut book = Book::new();
    let key = book
        .raise(0, BUYER, SELLER, 400.0, Terms::net_30(1_000.0), Why::Supply)
        .expect("no invoice");
    book.mark_doubtful(key);

    assert!(book.get(key).unwrap().doubtful);
    assert_eq!(
        book.owed_by(BUYER),
        400.0,
        "the debtor was released by the creditor's own despair"
    );
    assert_eq!(book.lost, 400.0, "the creditor did not record the loss");
    // And the debtor can still pay it, which is what makes it a debt.
    assert_eq!(book.settle(key, 400.0), 400.0);
    assert_eq!(book.owed_by(BUYER), 0.0);
    // Marking it twice must not double the recorded loss.
    book.mark_doubtful(key);
    assert_eq!(book.lost, 400.0);
}

/// **A buyer in arrears loses access to further unsecured supply.**
///
/// The consequence that makes failing to pay cost something. Without it
/// an insolvent firm buys for ever on a book that only grows, which is
/// the same unbounded state as the counter this replaces — moved rather
/// than fixed.
#[test]
fn an_insolvent_buyer_stops_being_supplied() {
    let mut book = Book::new();
    let terms = Terms::net_30(1_000.0);
    book.raise(0, BUYER, SELLER, 200.0, terms, Why::Supply);

    // Day 20: still within terms, so credit is still offered.
    let within = book.what_can_be_bought(20, BUYER, SELLER, 300.0, 0.0, terms);
    assert_eq!(
        within.on_credit, 300.0,
        "an invoice not yet due stopped supply"
    );
    assert_eq!(within.why, Refusal::None);

    // Day 40: the invoice is past its due date and nothing further is
    // supplied on credit — whatever headroom the limit would allow.
    let after = book.what_can_be_bought(40, BUYER, SELLER, 300.0, 0.0, terms);
    assert_eq!(after.on_credit, 0.0);
    assert_eq!(after.refused, 300.0);
    assert_eq!(after.why, Refusal::InArrears);

    // **And cash still buys.** Being in arrears is not being cut off from
    // the market; it is being cut off from *credit*.
    let cash = book.what_can_be_bought(40, BUYER, SELLER, 300.0, 300.0, terms);
    assert_eq!(cash.paid_now, 300.0);
    assert_eq!(cash.refused, 0.0);

    // Pay the overdue bill and the credit comes back.
    let owed: Vec<_> = book.due_from(BUYER);
    book.settle(owed[0], 200.0);
    let mended = book.what_can_be_bought(40, BUYER, SELLER, 300.0, 0.0, terms);
    assert_eq!(mended.on_credit, 300.0, "paying up did not restore supply");
}

/// **A credit limit is what one supplier will carry for one customer.**
///
/// Not a global allowance: a firm at its limit with one supplier can
/// still buy from another, which is exactly what a buyer in trouble does.
#[test]
fn a_limit_is_between_two_firms_and_not_a_global_allowance() {
    let mut book = Book::new();
    let terms = Terms::net_30(500.0);
    book.raise(0, BUYER, SELLER, 500.0, terms, Why::Supply);

    let maxed = book.what_can_be_bought(10, BUYER, SELLER, 100.0, 0.0, terms);
    assert_eq!(maxed.on_credit, 0.0);
    assert_eq!(maxed.why, Refusal::AtLimit);

    let elsewhere = book.what_can_be_bought(10, BUYER, OTHER, 100.0, 0.0, terms);
    assert_eq!(
        elsewhere.on_credit, 100.0,
        "one supplier's limit stopped a different supplier trading"
    );
}

/// **A payment goes against the oldest bill first**, and the answer does
/// not depend on how a registry happens to walk.
#[test]
fn what_is_paid_first_is_what_fell_due_first() {
    let mut book = Book::new();
    // Raised out of order on purpose: the later invoice is due sooner.
    let late = book
        .raise(
            0,
            BUYER,
            SELLER,
            100.0,
            Terms {
                days: 90,
                limit: 1e9,
            },
            Why::Supply,
        )
        .expect("no invoice");
    let early = book
        .raise(
            10,
            BUYER,
            SELLER,
            100.0,
            Terms {
                days: 5,
                limit: 1e9,
            },
            Why::Supply,
        )
        .expect("no invoice");

    let order = book.due_from(BUYER);
    assert_eq!(
        order.first().copied(),
        Some(early),
        "the bill that falls due first is not the one paid first"
    );
    assert_eq!(order.last().copied(), Some(late));
}

/// **The book grows with what is owed, not with history.**
///
/// The unbounded state this project has had to remove four times,
/// refused in advance. Safe for the reason `registry.rs` asserts: the
/// counter is written down rather than derived from the highest key
/// present, so a collected invoice's name is never reissued.
#[test]
fn a_settled_invoice_does_not_stay_on_the_book_for_ever() {
    let mut book = Book::new();
    let paid = book
        .raise(0, BUYER, SELLER, 100.0, Terms::net_30(1e9), Why::Supply)
        .expect("no invoice");
    let open = book
        .raise(0, BUYER, SELLER, 100.0, Terms::net_30(1e9), Why::Supply)
        .expect("no invoice");
    book.settle(paid, 100.0);

    book.roll_the_book(31, 90);
    assert_eq!(book.len(), 2, "a settled invoice was collected too early");

    book.roll_the_book(200, 90);
    assert_eq!(book.len(), 1, "a settled invoice stayed on the book");
    assert!(book.get(open).is_some(), "an unpaid invoice was collected");
    assert_eq!(book.owed_by(BUYER), 100.0);
}
