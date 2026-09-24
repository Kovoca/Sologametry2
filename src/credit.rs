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
//!
//! ## Bills are on the book; trade between firms is not yet
//!
//! **The book now carries every bill a household, a state or an insurer
//! could not pay** — `Economy::charge` pays what cash covers and owes the
//! rest, under a bill's own identity (`Origin` and the day), so a retry is
//! not a second debt, a later payment settles *that* obligation, a save
//! brings it back, and nothing ends it but paying it or charging it off.
//! Firm-to-firm goods, carriage, wages and a works' own power still fall on
//! the treasury's day tally when a buyer cannot pay: connecting them is
//! bounded trade credit, authorised before the goods move, which is the next
//! piece and not this one.

use crate::money::{Account, Why};
use crate::registry::{Key, Registry};
use std::collections::{BTreeMap, BTreeSet};

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

/// **What an obligation is for**: the kind of bill it arose from.
///
/// The same debtor can owe the same creditor for different things on the
/// same day — a town owes its service sector a health premium, a motor
/// premium and the day's services — and they are different bills, due
/// separately and collected separately, so the kind is part of a bill's
/// identity rather than a label on it.
///
/// **The codes are frozen.** A code is a name on disk; moving one
/// reinterprets every saved obligation. Appended, never inserted.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Origin {
    /// Goods from one firm to another.
    Supply,
    /// A hospital's bill, to whichever of the state, the insurers and the
    /// patient carries each share of it.
    Care,
    /// A builder's bill for the work done on a town's buildings.
    Upkeep,
    /// The private services a town's people bought: the kitchens, the
    /// offices, the recreation.
    Services,
    /// A health insurance premium.
    HealthPremium,
    /// A motor insurance premium.
    VehiclePremium,
    /// Tax owed on the day's pay.
    Tax,
    /// A household's power bill.
    Power,
    /// A claim an insurer owes on cover it sold.
    Claim,
}

impl Origin {
    pub const ALL: [Origin; 9] = [
        Origin::Supply,
        Origin::Care,
        Origin::Upkeep,
        Origin::Services,
        Origin::HealthPremium,
        Origin::VehiclePremium,
        Origin::Tax,
        Origin::Power,
        Origin::Claim,
    ];

    /// Its name on disk. Exhaustive, so a new kind of bill cannot compile
    /// until somebody has decided what it is called in a save.
    pub fn code(self) -> u8 {
        match self {
            Origin::Supply => 1,
            Origin::Care => 2,
            Origin::Upkeep => 3,
            Origin::Services => 4,
            Origin::HealthPremium => 5,
            Origin::VehiclePremium => 6,
            Origin::Tax => 7,
            Origin::Power => 8,
            Origin::Claim => 9,
        }
    }

    pub fn from_code(code: u8) -> Option<Origin> {
        Origin::ALL.iter().copied().find(|o| o.code() == code)
    }

    pub fn name(self) -> &'static str {
        match self {
            Origin::Supply => "supply",
            Origin::Care => "care",
            Origin::Upkeep => "building work",
            Origin::Services => "services",
            Origin::HealthPremium => "health premium",
            Origin::VehiclePremium => "motor premium",
            Origin::Tax => "tax",
            Origin::Power => "power",
            Origin::Claim => "claims",
        }
    }
}

/// **What became of one bill when it was presented**: what it came to,
/// what was paid there and then, and what was left owing. `paid + owed`
/// is `amount`, always — which is the whole of what "every bill has a
/// funding path" means, and the thing the gates read.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Billed {
    pub amount: f64,
    pub paid: f64,
    pub owed: f64,
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
    /// **What bill it arose from.**
    pub origin: Origin,
    /// Raised through `bill`, which means it is one particular bill: the
    /// same debtor, creditor, kind and day presented again is a retry and
    /// adds nothing. An invoice raised through `raise` is anonymous and
    /// two of them can stand side by side.
    pub once: bool,
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
///
/// **An obligation ends in one of two ways, and both are recorded**: it is
/// paid, or it is charged off. Nothing else takes one off the book — not
/// a new day, not a new bill, not a debtor running out of money — which is
/// what distinguishes a debt from a shortfall on a counter that is cleared
/// every morning.
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
    /// **What has ever been charged off**, and by kind of bill. A charge-off
    /// is an accounting event with a date and an amount, not a debt that
    /// quietly stopped being mentioned.
    pub charged_off: f64,
    pub charged_off_by: BTreeMap<Origin, f64>,
    /// Derived, and rebuilt on load: which live invoice each bill is, so a
    /// retry finds it.
    bills: BTreeMap<(Account, Account, Origin, u64), Key<Invoice>>,
    /// Derived, and rebuilt on load: each debtor's live invoices, so asking
    /// what one town owes does not walk every invoice in the world.
    by_debtor: BTreeMap<Account, BTreeSet<Key<Invoice>>>,
}

impl Book {
    pub fn new() -> Self {
        Book::default()
    }

    fn keep(&mut self, inv: Invoice) -> Key<Invoice> {
        let (debtor, creditor, origin, raised, once) =
            (inv.debtor, inv.creditor, inv.origin, inv.raised, inv.once);
        let key = self.invoices.add(inv);
        self.by_debtor.entry(debtor).or_default().insert(key);
        if once {
            self.bills.insert((debtor, creditor, origin, raised), key);
        }
        key
    }

    fn forget(&mut self, key: Key<Invoice>, day: u64, how: &str) -> Option<Invoice> {
        let inv = self.invoices.end(key, day, how)?;
        if let Some(set) = self.by_debtor.get_mut(&inv.debtor) {
            set.remove(&key);
            if set.is_empty() {
                self.by_debtor.remove(&inv.debtor);
            }
        }
        if inv.once {
            self.bills
                .remove(&(inv.debtor, inv.creditor, inv.origin, inv.raised));
        }
        Some(inv)
    }

    /// **Raise an invoice**, which is one way one comes into being.
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
        Some(self.keep(Invoice {
            debtor,
            creditor,
            raised: day,
            due: day + terms.days,
            amount,
            settled: 0.0,
            doubtful: false,
            what,
            origin: Origin::Supply,
            once: false,
        }))
    }

    /// **Owe what was not paid on one particular bill.**
    ///
    /// The bill is who owes, who is owed, what it was for and the day it
    /// arose. **Presented again, it adds nothing** and hands back the
    /// obligation already on the book — so a retried payment, a phase run
    /// twice, or a day replayed after a reload cannot turn one debt into
    /// two. A later payment reduces this obligation through `settle`; it
    /// never raises another.
    #[allow(clippy::too_many_arguments)]
    pub fn bill(
        &mut self,
        day: u64,
        debtor: Account,
        creditor: Account,
        amount: f64,
        days_to_pay: u64,
        what: Why,
        origin: Origin,
    ) -> Option<Key<Invoice>> {
        if let Some(&k) = self.bills.get(&(debtor, creditor, origin, day)) {
            return Some(k);
        }
        if amount <= 1e-9 || debtor == creditor {
            return None;
        }
        self.billed += amount;
        Some(self.keep(Invoice {
            debtor,
            creditor,
            raised: day,
            due: day + days_to_pay,
            amount,
            settled: 0.0,
            doubtful: false,
            what,
            origin,
            once: true,
        }))
    }

    /// The obligation already on the book for one bill, if there is one.
    pub fn find(
        &self,
        debtor: Account,
        creditor: Account,
        origin: Origin,
        day: u64,
    ) -> Option<Key<Invoice>> {
        self.bills.get(&(debtor, creditor, origin, day)).copied()
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

    /// **Days past due before an unpaid household bill is charged off.**
    ///
    /// **A designed policy, anchored on the nearest regulatory rule and not
    /// derived from it.** US bank regulators classify open-end retail credit
    /// that is 180 days past due as a loss to be charged off, and
    /// closed-end at 120 *(FFIEC, Uniform Retail Credit Classification and
    /// Account Management Policy, Federal Register, 12 June 2000)*. A
    /// hospital's or a builder's bad-debt policy is its own and nobody
    /// regulates it; the open-end figure is used because a household's
    /// running bills are more like a revolving account than a loan.
    pub const CHARGE_OFF_AFTER: u64 = 180;

    /// **Charge one obligation off**: the creditor recognises that it will
    /// not be paid, and it leaves the book with a dated record of what
    /// was lost. Returns what was charged off.
    ///
    /// Not a forgiveness the debtor asked for and not a debt that quietly
    /// lapsed — an event, recorded by kind, whose total the gates hold
    /// against what was billed and collected. **What a charged-off debt
    /// becomes afterwards** — sold to a collector, pursued in court, still
    /// owed at law — is not modelled, and is named rather than implied.
    pub fn charge_off(&mut self, key: Key<Invoice>, day: u64) -> f64 {
        let Some(inv) = self.forget(key, day, "charged off") else {
            return 0.0;
        };
        let lost = inv.outstanding();
        self.charged_off += lost;
        *self.charged_off_by.entry(inv.origin).or_default() += lost;
        lost
    }

    /// **Charge off whatever a policy says has gone too long**: every
    /// obligation from a debtor `eligible` accepts that is more than
    /// `CHARGE_OFF_AFTER` days past its due date. Returns what was charged
    /// off today.
    pub fn charge_off_overdue(&mut self, day: u64, eligible: impl Fn(Account) -> bool) -> f64 {
        let stale: Vec<Key<Invoice>> = self
            .invoices
            .iter()
            .filter(|(_, i)| {
                eligible(i.debtor) && !i.settled_in_full() && day > i.due + Self::CHARGE_OFF_AFTER
            })
            .map(|(k, _)| k)
            .collect();
        stale.into_iter().map(|k| self.charge_off(k, day)).sum()
    }

    pub fn get(&self, key: Key<Invoice>) -> Option<&Invoice> {
        self.invoices.get(key)
    }

    /// What became of an obligation, including one that has left the book
    /// — paid off and rolled away, or charged off.
    pub fn look(&self, key: Key<Invoice>) -> crate::registry::Lookup<'_, Invoice> {
        self.invoices.look(key)
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

    fn of(&self, who: Account) -> impl Iterator<Item = (Key<Invoice>, &Invoice)> + '_ {
        self.by_debtor
            .get(&who)
            .into_iter()
            .flat_map(|set| set.iter())
            .filter_map(move |&k| self.invoices.get(k).map(|i| (k, i)))
    }

    /// What this account owes, over every invoice against it.
    pub fn owed_by(&self, who: Account) -> f64 {
        self.of(who).map(|(_, i)| i.outstanding()).sum()
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
        self.of(who)
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
        self.of(who).map(|(_, i)| i.overdue_on(day)).sum()
    }

    /// What this account has to pay **by today**: everything falling due on
    /// or before it. What a budget has to find room for.
    pub fn due_by(&self, who: Account, day: u64) -> f64 {
        self.of(who)
            .filter(|(_, i)| i.due <= day)
            .map(|(_, i)| i.outstanding())
            .sum()
    }

    /// What one customer owes one supplier, which is what a credit limit
    /// is measured against.
    pub fn between(&self, debtor: Account, creditor: Account) -> f64 {
        self.of(debtor)
            .filter(|(_, i)| i.creditor == creditor)
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
            .of(who)
            .filter(|(_, i)| !i.settled_in_full())
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
            self.forget(k, day, "settled");
        }
    }

    /// **Forget the graves of obligations that ended long ago**, so the
    /// record of what was paid off does not grow with history either. The
    /// totals — billed, collected, charged off — keep the account.
    pub fn forget_ended_before(&mut self, day: u64) {
        self.invoices.forget_graves_before(day);
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
            (owed - by).abs() < 1e-6 * owed.abs().max(1.0),
            "what is owed ({owed}) is not what anybody owes ({by})"
        );
        // **Every penny billed is collected, still owed, or charged off**,
        // and nothing else: an obligation that left the book any other way
        // is one that quietly lapsed.
        let accounted = self.collected + owed + self.charged_off;
        assert!(
            (self.billed - accounted).abs() <= 1e-6 * self.billed.abs().max(1.0),
            "{} billed against {} collected, {} owed and {} charged off — an obligation \
             left the book without being paid or charged off",
            self.billed,
            self.collected,
            owed,
            self.charged_off
        );
    }
}

// =====================================================================
// the book, written down
// =====================================================================

use crate::save::{Reader, SaveError, Store, Writer};

impl Store for Invoice {
    fn store(&self, w: &mut Writer) {
        self.debtor.store(w);
        self.creditor.store(w);
        w.u64(self.raised);
        w.u64(self.due);
        w.f64(self.amount);
        w.f64(self.settled);
        w.u8(self.doubtful as u8);
        self.what.store(w);
        w.u8(self.origin.code());
        w.u8(self.once as u8);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let debtor = Account::load(r)?;
        let creditor = Account::load(r)?;
        let raised = r.u64()?;
        let due = r.u64()?;
        let amount = r.finite_f64()?;
        let settled = r.finite_f64()?;
        let doubtful = match r.u8()? {
            0 => false,
            1 => true,
            n => return Err(SaveError::UnknownCode("doubtful flag", n as u32)),
        };
        let what = Why::load(r)?;
        let code = r.u8()?;
        let origin =
            Origin::from_code(code).ok_or(SaveError::UnknownCode("bill origin", code as u32))?;
        let once = match r.u8()? {
            0 => false,
            1 => true,
            n => return Err(SaveError::UnknownCode("bill identity flag", n as u32)),
        };
        // **Each of these decodes perfectly well and none can be true**: a
        // debt to oneself, a bill for nothing, a payment beyond the bill,
        // and a bill due before it arose.
        if debtor == creditor {
            return Err(SaveError::Impossible("an obligation owed to oneself"));
        }
        if amount <= 0.0 || settled < 0.0 {
            return Err(SaveError::Impossible("an obligation for nothing"));
        }
        if settled > amount * (1.0 + 1e-9) + 1e-9 {
            return Err(SaveError::Impossible(
                "more paid against an obligation than it was for",
            ));
        }
        if due < raised {
            return Err(SaveError::Impossible("an obligation due before it arose"));
        }
        Ok(Invoice {
            debtor,
            creditor,
            raised,
            due,
            amount,
            settled,
            doubtful,
            what,
            origin,
            once,
        })
    }
}

/// **The totals are written down and the indexes are not.** Which live
/// invoice a bill is, and which invoices a debtor has, are read off the
/// invoices themselves on the way back in — a second copy that had to agree
/// with the first could silently stop agreeing.
impl Store for Book {
    fn store(&self, w: &mut Writer) {
        self.invoices.store(w);
        w.f64(self.lost);
        w.f64(self.billed);
        w.f64(self.collected);
        w.f64(self.charged_off);
        w.len(self.charged_off_by.len());
        for (o, v) in self.charged_off_by.iter() {
            w.u8(o.code());
            w.f64(*v);
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let invoices = Registry::<Invoice>::load(r)?;
        let lost = r.finite_f64()?;
        let billed = r.finite_f64()?;
        let collected = r.finite_f64()?;
        let charged_off = r.finite_f64()?;
        let n = r.count()?;
        let mut charged_off_by = BTreeMap::new();
        for _ in 0..n {
            let code = r.u8()?;
            let o = Origin::from_code(code)
                .ok_or(SaveError::UnknownCode("bill origin", code as u32))?;
            let v = r.finite_f64()?;
            if v < 0.0 {
                return Err(SaveError::Impossible("a negative charge-off"));
            }
            if charged_off_by.insert(o, v).is_some() {
                return Err(SaveError::Impossible("one kind of bill charged off twice"));
            }
        }
        if lost < 0.0 || billed < 0.0 || collected < 0.0 || charged_off < 0.0 {
            return Err(SaveError::Impossible("a book with a negative total"));
        }
        let mut book = Book {
            invoices: Registry::new(),
            lost,
            billed,
            collected,
            charged_off,
            charged_off_by,
            bills: BTreeMap::new(),
            by_debtor: BTreeMap::new(),
        };
        book.invoices = invoices;
        let live: Vec<(Key<Invoice>, Account, Account, Origin, u64, bool)> = book
            .invoices
            .iter()
            .map(|(k, i)| (k, i.debtor, i.creditor, i.origin, i.raised, i.once))
            .collect();
        for (k, debtor, creditor, origin, raised, once) in live {
            book.by_debtor.entry(debtor).or_default().insert(k);
            if once
                && book
                    .bills
                    .insert((debtor, creditor, origin, raised), k)
                    .is_some()
            {
                return Err(SaveError::Impossible("one bill owed twice"));
            }
        }
        // **A book whose totals do not account for its invoices is refused
        // at the door**, the way a treasury whose money does not conserve
        // is: every penny billed is collected, still owed, or charged off.
        let owed = book.outstanding();
        let accounted = collected + owed + charged_off;
        if (billed - accounted).abs() > 1e-6 * billed.abs().max(1.0) {
            return Err(SaveError::Impossible(
                "obligations that do not account for what was billed",
            ));
        }
        let by_kind: f64 = book.charged_off_by.values().sum();
        if (by_kind - charged_off).abs() > 1e-6 * charged_off.abs().max(1.0) {
            return Err(SaveError::Impossible(
                "charge-offs by kind that do not add up to the total",
            ));
        }
        Ok(book)
    }
}
