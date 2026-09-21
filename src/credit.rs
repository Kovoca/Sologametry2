//! **Goods moved and nobody was billed.**
//!
//! `Treasury::pay` caps a transfer at what the payer holds, adds the
//! shortfall to a counter, and the goods have **already moved**. So
//! nothing anywhere records a receivable: the supplier is simply poorer,
//! the buyer holds the stock, and the conservation assertion is satisfied
//! throughout — which is exactly how money can conserve while the
//! transactions are wrong. Over one year of one world it came to
//! **1.53e11** against firm-to-firm supply, the largest category there
//! is, and it propagates: a supplier that was not paid cannot meet
//! payroll and lays people off.
//!
//! **Until every delivery has a named commercial outcome — paid, owed, or
//! refused — matching a calibration band is weak evidence**, because a
//! missing payment can be offsetting another error.
//!
//! ## An invoice, not a balance
//!
//! The first design was one edge, `(debtor, creditor) -> amount`, on the
//! argument that a single record cannot disagree with itself. It cannot,
//! and **it also cannot carry terms**:
//!
//! | delivery | amount | invoiced | due |
//! |---|---|---|---|
//! | first | 100 | day 0 | day 30 |
//! | second | 200 | day 20 | day 50 |
//!
//! On day 30 only 100 is due. Collapsing those into "300 owed" destroys
//! the distinction that decides whether anybody is in arrears. So the
//! authoritative record is **per invoice**, and both firms' positions are
//! *derived* by walking invoices — which keeps the one-record property
//! that makes a receivable and a payable unable to drift apart, and adds
//! the terms a balance cannot hold.
//!
//! ## Credit is agreed before the goods move
//!
//! The ordering is the whole of it. If goods transfer and an obligation
//! appears *whenever payment fails*, *every supplier becomes a compulsory
//! lender* — which is not trade credit, it is the same defect with a
//! ledger entry. So a purchase is decided first, and the inventory move,
//! the cash and the invoice commit together.
//!
//! ## Outstanding is not overdue
//!
//! A net-30 invoice is an ordinary trade asset before day 30 and
//! **arrears** after it. They are different states and only the second
//! restricts further supply.
//!
//! ## What is real, and what is a dial
//!
//! **Net 30 is contractual and is an input.** Days sales outstanding is
//! an *outcome* — it measures collection performance and moves with
//! customer behaviour, business mix and the calculation method — so it is
//! reported and compared against, **never set**. Setting invoices to
//! clear at a chosen number of days would reproduce that number and test
//! nothing.
//!
//! ## And wiring it in starved the country, three times over
//!
//! **This is not yet connected to `distribute`, and the reason is a
//! measurement rather than an omission.** Three attempts, each on seed 7
//! over two years against a baseline of 6.4% unemployment and five days
//! of food cover:
//!
//! | | unemployment | food cover | households |
//! |---|---|---|---|
//! | before | 6.4% | 5.00 | 4.40e10 |
//! | credit off one day overdue | **82.6%** | 0.04 | 5.4e8 |
//! | on stop at 60 days instead | 47.9% | 0.35 | 7.3e8 |
//! | + a working-capital floor | 59.2% | 3.72 | 3.23e11, debt 9.7e12 |
//!
//! Each failure taught something and the third is where it stopped:
//!
//! - **Cutting credit the day after an invoice falls due is not what
//!   trade does.** Net-30 terms against a DSO nearer 37-56 days means the
//!   *ordinary* invoice is paid late and the supplier goes on supplying.
//!   That rule refused 2.25e14 against 7.6e10 extended. `ON_STOP_AFTER`
//!   is the correction and it is kept, because it is right whatever
//!   happens to the rest.
//! - **The refusals were not the cause**, which only an experiment could
//!   say. With the credit limit made infinite — nothing refused at all —
//!   unemployment still reached 51.0%. What drained the economy was
//!   **collections**: firms paying due bills down to an empty till and
//!   then being unable to buy tomorrow's inputs.
//! - **A working-capital floor moved the failure rather than fixing
//!   it.** Keeping a week of a firm's own outgoings made the floor a
//!   function of what it had just spent — including the collections —
//!   so a firm that spent freely kept everything, nothing was collected,
//!   and the debt stock went to 9.7e12 with refusals at 4.24e18.
//!
//! **The finding underneath all three is the one worth having: the
//! unpaid counter is load-bearing.** This economy only functions because
//! firms take goods they cannot pay for — 1.53e11 of supply a year on
//! seed 7 — and the moment a delivery requires cash or agreed credit,
//! the circuit that was being papered over fails in the open. So the
//! prerequisite is not a better credit rule. It is **closing the money
//! circuit**: households buying what they can pay for rather than on
//! credit nobody extended, and the two wage scales that leave them short
//! (`docs/status.md` items 3 and 10).
//!
//! What is kept is the record, the terms, the decision and the gates,
//! because every one of them is right and none of them is what failed.

use crate::money::{Account, Why};
use crate::registry::{Key, Registry};

/// **What a supplier will let a customer owe, and for how long.**
///
/// Net 30 is the standard term in business-to-business trade. The limit
/// is what this supplier will carry unsecured for this customer, and it
/// is the thing arrears eat into.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Terms {
    /// Days from the invoice to the due date.
    pub days: u64,
    /// The most this supplier will have outstanding with this customer.
    pub limit: f64,
}

impl Terms {
    /// **Net 30, the ordinary business-to-business term.**
    pub const NET_30: u64 = 30;

    pub fn net_30(limit: f64) -> Self {
        Terms {
            days: Self::NET_30,
            limit,
        }
    }

    /// Cash only — no credit at all, which is what a customer already in
    /// arrears is offered.
    pub fn cash_only() -> Self {
        Terms {
            days: 0,
            limit: 0.0,
        }
    }
}

/// One invoice: a delivery that was not paid for in cash, and the terms
/// it was supplied on.
///
/// **`amount` never changes.** What was settled against it accumulates,
/// and `outstanding` is the difference — so a partial payment cannot be
/// mistaken for a smaller sale, and the original transaction stays
/// legible after any number of part payments.
#[derive(Clone, Debug, PartialEq)]
pub struct Invoice {
    pub debtor: Account,
    pub creditor: Account,
    /// The day it was raised, which is the day the goods moved.
    pub raised: u64,
    /// The day it falls due. Before it the debt is outstanding; after it,
    /// overdue.
    pub due: u64,
    /// What was billed. Immutable.
    pub amount: f64,
    /// What has been paid against it.
    pub settled: f64,
    /// **The creditor's judgement that it will not be paid.** It reduces
    /// what the creditor expects to collect and **does not reduce what
    /// the debtor owes** — writing a debt down is a loss taken by
    /// whoever is owed, not a forgiveness granted to whoever owes.
    pub doubtful: bool,
    /// What the goods were, so a collection can be told from a sale.
    pub what: Why,
}

impl Invoice {
    pub fn outstanding(&self) -> f64 {
        (self.amount - self.settled).max(0.0)
    }

    pub fn overdue_on(&self, day: u64) -> f64 {
        if day > self.due {
            self.outstanding()
        } else {
            0.0
        }
    }

    pub fn settled_in_full(&self) -> bool {
        self.outstanding() <= 1e-9
    }
}

/// **What a purchase actually came to**, which is the answer the
/// inventory move has to be sized on.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Purchase {
    /// Paid in cash, now.
    pub paid_now: f64,
    /// Supplied on credit, and about to become an invoice.
    pub on_credit: f64,
    /// Wanted and not supplied, because neither cash nor credit reached
    /// it. **The goods do not move for this part** — which is the whole
    /// difference from what this replaces.
    pub refused: f64,
    /// Why the refused part was refused, for whoever is reading a run.
    pub why: Refusal,
}

impl Purchase {
    /// What the buyer actually gets, as a share of what it asked for.
    pub fn share_of(&self, wanted: f64) -> f64 {
        if wanted <= 1e-9 {
            return 1.0;
        }
        ((self.paid_now + self.on_credit) / wanted).clamp(0.0, 1.0)
    }
}

/// Why a buyer could not have all it asked for. **Four answers, because
/// "no" is not one answer** — the same reasoning `bank.rs` already
/// applies to a refused loan.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Nothing was refused.
    None,
    /// No cash, and this supplier extends no credit to anybody.
    NoCash,
    /// Cash short and the customer is already at what this supplier will
    /// carry.
    AtLimit,
    /// Cash short and the customer has bills already past their due date,
    /// which is what stops supply in life.
    InArrears,
}

/// **Every invoice in the world, and the one place they are written.**
///
/// A `Registry` rather than a `Vec`, because an invoice has to keep one
/// name across a save and must never be confused with the delivery it
/// came from or the firm that owes it — both of which are `usize` and
/// would both compile.
#[derive(Clone, Debug, Default)]
pub struct Book {
    invoices: Registry<Invoice>,
    /// What has ever been written down as a loss, for the diagnostic.
    /// A total nobody can see is a total nobody checks.
    pub lost: f64,
    /// What has ever been billed on credit, and what has ever been
    /// collected — reported separately, because a collection is not a
    /// second sale.
    pub billed: f64,
    pub collected: f64,
}

impl Book {
    pub fn new() -> Self {
        Book::default()
    }

    /// **Raise an invoice**, which is the only way one comes into being.
    pub fn raise(
        &mut self,
        day: u64,
        debtor: Account,
        creditor: Account,
        amount: f64,
        terms: Terms,
        what: Why,
    ) -> Option<Key<Invoice>> {
        if amount <= 1e-9 || debtor == creditor {
            return None;
        }
        self.billed += amount;
        Some(self.invoices.add(Invoice {
            debtor,
            creditor,
            raised: day,
            due: day + terms.days,
            amount,
            settled: 0.0,
            doubtful: false,
            what,
        }))
    }

    /// **Pay something against an invoice**, returning what was actually
    /// applied — which is never more than is outstanding, so a payment
    /// cannot reduce a balance twice or take one negative.
    pub fn settle(&mut self, key: Key<Invoice>, amount: f64) -> f64 {
        let Some(inv) = self.invoices.get_mut(key) else {
            return 0.0;
        };
        let applied = amount.min(inv.amount - inv.settled).max(0.0);
        inv.settled += applied;
        self.collected += applied;
        applied
    }

    /// **The creditor gives up on it.** Its own expectation falls and the
    /// debtor still owes every penny.
    pub fn mark_doubtful(&mut self, key: Key<Invoice>) {
        if let Some(inv) = self.invoices.get_mut(key) {
            if !inv.doubtful {
                self.lost += inv.outstanding();
                inv.doubtful = true;
            }
        }
    }

    pub fn get(&self, key: Key<Invoice>) -> Option<&Invoice> {
        self.invoices.get(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (Key<Invoice>, &Invoice)> {
        self.invoices.iter()
    }

    pub fn len(&self) -> usize {
        self.invoices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.invoices.is_empty()
    }

    /// What this account owes, over every invoice against it.
    pub fn owed_by(&self, who: Account) -> f64 {
        self.invoices
            .iter()
            .filter(|(_, i)| i.debtor == who)
            .map(|(_, i)| i.outstanding())
            .sum()
    }

    /// What this account is owed.
    pub fn owed_to(&self, who: Account) -> f64 {
        self.invoices
            .iter()
            .filter(|(_, i)| i.creditor == who)
            .map(|(_, i)| i.outstanding())
            .sum()
    }

    /// **Days past due before a supplier puts an account on stop.**
    ///
    /// Not zero, which is what "overdue" alone would mean: net-30 terms
    /// against a DSO nearer 37-56 days says the ordinary invoice is paid
    /// late and trade carries on. Sixty days is the early end of real
    /// on-stop practice.
    pub const ON_STOP_AFTER: u64 = 60;

    /// What this account owes so far past due that a supplier stops
    /// trading with it. **This is the figure that restricts supply** —
    /// `overdue_by` is the ordinary lateness that does not.
    pub fn seriously_overdue(&self, who: Account, day: u64) -> f64 {
        self.invoices
            .iter()
            .filter(|(_, i)| i.debtor == who)
            .map(|(_, i)| {
                if day > i.due + Self::ON_STOP_AFTER {
                    i.outstanding()
                } else {
                    0.0
                }
            })
            .sum()
    }

    /// What this account owes **past its due date**, which is ordinary
    /// lateness and is reported rather than acted on.
    pub fn overdue_by(&self, who: Account, day: u64) -> f64 {
        self.invoices
            .iter()
            .filter(|(_, i)| i.debtor == who)
            .map(|(_, i)| i.overdue_on(day))
            .sum()
    }

    /// What one customer owes one supplier, which is what a credit limit
    /// is measured against.
    pub fn between(&self, debtor: Account, creditor: Account) -> f64 {
        self.invoices
            .iter()
            .filter(|(_, i)| i.debtor == debtor && i.creditor == creditor)
            .map(|(_, i)| i.outstanding())
            .sum()
    }

    /// Everything still owed, anywhere.
    pub fn outstanding(&self) -> f64 {
        self.invoices.iter().map(|(_, i)| i.outstanding()).sum()
    }

    /// Everything owed past its due date.
    pub fn overdue(&self, day: u64) -> f64 {
        self.invoices.iter().map(|(_, i)| i.overdue_on(day)).sum()
    }

    /// **The invoices one debtor should pay next**, oldest due date
    /// first — which is what a firm actually does, and what makes a
    /// partial payment land somewhere definite rather than being spread
    /// by whatever order a vector happened to be in.
    pub fn due_from(&self, who: Account) -> Vec<Key<Invoice>> {
        let mut mine: Vec<(Key<Invoice>, u64, u64)> = self
            .invoices
            .iter()
            .filter(|(_, i)| i.debtor == who && !i.settled_in_full())
            .map(|(k, i)| (k, i.due, k.number()))
            .collect();
        // Due date first, then the order they were raised in, so the
        // answer never depends on how the registry happens to walk.
        mine.sort_by(|a, b| a.1.cmp(&b.1).then(a.2.cmp(&b.2)));
        mine.into_iter().map(|(k, _, _)| k).collect()
    }

    /// **Clear the invoices that are finished**, so the book grows with
    /// what is owed rather than with history. Safe for exactly the reason
    /// `registry.rs` states: the counter is written down rather than
    /// derived from the highest key present, so a collected invoice's
    /// name is never handed out again.
    pub fn roll_the_book(&mut self, day: u64, keep_for: u64) {
        let done: Vec<Key<Invoice>> = self
            .invoices
            .iter()
            .filter(|(_, i)| i.settled_in_full() && day >= i.due + keep_for)
            .map(|(k, _)| k)
            .collect();
        for k in done {
            self.invoices.end(k, day, "settled");
        }
    }

    /// **What this buyer can actually have.**
    ///
    /// The decision that has to happen *before* the goods move. Cash
    /// first, then whatever credit this supplier will extend — and a
    /// customer with bills already overdue is offered none, which is what
    /// stops supply in life.
    pub fn what_can_be_bought(
        &self,
        day: u64,
        buyer: Account,
        seller: Account,
        wanted: f64,
        cash: f64,
        terms: Terms,
    ) -> Purchase {
        let wanted = wanted.max(0.0);
        let paid_now = cash.max(0.0).min(wanted);
        let short = wanted - paid_now;
        if short <= 1e-9 {
            return Purchase {
                paid_now,
                on_credit: 0.0,
                refused: 0.0,
                why: Refusal::None,
            };
        }

        // **Arrears stop supply before a limit does** — but not on the
        // first day one, and the first version of this deadlocked a whole
        // economy by getting that wrong.
        //
        // **Paying late is the norm in business-to-business trade, not a
        // failure.** Terms are net 30 and domestic days-sales-outstanding
        // runs nearer 37, with a cross-industry median nearer 56, so the
        // *average* invoice is settled after its due date and the
        // supplier goes on supplying. A rule that cut off credit the day
        // after an invoice fell due refused essentially every delivery in
        // the world: 2.25e14 refused against 7.6e10 extended, unemployment
        // 6.4% to 82.6% and food cover to a twenty-fifth of a day.
        //
        // What a supplier actually does is put an account **on stop**
        // when it is *seriously* overdue — trade practice is 60 to 90
        // days past due, which is also where a receivable starts being
        // written down.
        if self.seriously_overdue(buyer, day) > 1e-9 {
            return Purchase {
                paid_now,
                on_credit: 0.0,
                refused: short,
                why: Refusal::InArrears,
            };
        }
        if terms.limit <= 0.0 {
            return Purchase {
                paid_now,
                on_credit: 0.0,
                refused: short,
                why: Refusal::NoCash,
            };
        }

        let headroom = (terms.limit - self.between(buyer, seller)).max(0.0);
        let on_credit = short.min(headroom);
        let refused = short - on_credit;
        Purchase {
            paid_now,
            on_credit,
            refused,
            why: if refused > 1e-9 {
                Refusal::AtLimit
            } else {
                Refusal::None
            },
        }
    }

    /// **The books balance by construction**, and this says so out loud:
    /// what every debtor owes is what every creditor is owed, because
    /// there is one record and both sides read it.
    ///
    /// It is a real check all the same — it catches an invoice whose
    /// settled figure has run past its amount, which is the way a
    /// double-counted collection would show.
    pub fn assert_conserved(&self) {
        let mut owed = 0.0;
        for (_, i) in self.invoices.iter() {
            assert!(
                i.settled <= i.amount + 1e-6,
                "an invoice of {} has {} settled against it — a payment has been counted twice",
                i.amount,
                i.settled
            );
            assert!(i.amount > 0.0, "an invoice for nothing");
            owed += i.outstanding();
        }
        let by: f64 = {
            let mut who: std::collections::BTreeSet<Account> = Default::default();
            for (_, i) in self.invoices.iter() {
                who.insert(i.debtor);
            }
            who.into_iter().map(|a| self.owed_by(a)).sum()
        };
        assert!(
            (owed - by).abs() < 1e-6,
            "what is owed ({owed}) is not what anybody owes ({by})"
        );
    }
}
