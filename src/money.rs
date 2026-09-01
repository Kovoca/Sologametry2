//! Money, and who has it.
//!
//! The economy priced everything and paid for nothing. Households took
//! goods off a shelf without the shop being any better off; a worker was
//! paid out of nowhere; a firm bought a thousand tonnes of ore and its
//! balance did not move, because it had no balance. `Person` balanced its
//! own books — `money == start + earned − spent − staked`, asserted — but
//! that only proved a person's *pocket* was consistent, not that the
//! money in it had come from anywhere.
//!
//! The project's notes have called this the largest hole in the model for
//! some time, and named the consequence precisely:
//!
//! > Real disinflation with sticky wages causes *unemployment* for exactly
//! > this reason — firms cannot afford the real wage — and we cannot model
//! > that until a wage is somebody's cost.
//!
//! **That is the whole point of this module.** A wage has to be an expense
//! somebody actually bears before a firm can fail to bear it. Everything
//! else here is bookkeeping in service of that.
//!
//! ## The same discipline as the commodity ledger
//!
//! `econ::Ledger` makes the "state quietly evaporates at a handoff" bug
//! class impossible by having exactly one write path and asserting
//! conservation every tick. This does the same for money: every movement
//! is a `Transfer` between two named accounts, recorded, and the total
//! across all accounts is constant.
//!
//! Money is not created by working and destroyed by eating. It circulates.

use std::collections::BTreeMap;

/// Who can hold money.
///
/// Deliberately few. The interesting distinctions are firm/household/state
/// and inside/outside the country; anything finer is a subdivision of one
/// of those and can wait until something needs it.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Account {
    /// A firm, keyed by its site index. Every works, shop, mine and depot
    /// has one.
    Firm(usize),
    /// **Everybody in one town, pooled.** The population is statistical
    /// until somebody is individuated, so its money is too — a sampled
    /// `Person`'s pocket is drawn from this pool and returned to it, never
    /// held alongside it.
    Households(usize),
    /// The state: one treasury, taxing and spending.
    State,
    /// **The rest of the world.** A country is not a closed system: it
    /// pays for what it imports and is paid for what it exports, and the
    /// difference has to go somewhere. Without this account, a nation that
    /// buys more than it sells would appear to conjure money.
    Abroad,
}

impl Account {
    pub fn name(self) -> String {
        match self {
            Account::Firm(i) => format!("firm {i}"),
            Account::Households(m) => format!("households {m}"),
            Account::State => "the state".into(),
            Account::Abroad => "abroad".into(),
        }
    }
}

/// Why money moved. The counterpart of `econ::Use`, and it exists for the
/// same reason: a total that cannot be explained is a bug you cannot find.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Why {
    /// A household bought something at a counter.
    Purchase,
    /// A firm bought an input from another firm.
    Supply,
    /// Wages.
    Payroll,
    /// Freight charged by a carrier.
    Freight,
    /// Tax collected.
    Tax,
    /// The state paying its own staff and buying its own supplies.
    PublicSpending,
    /// Rent, on a dwelling.
    Rent,
    /// **What a firm made and did not need**, paid out to whoever owns it.
    Profit,
    /// Bought from or sold to the rest of the world.
    Trade,
}

/// One movement of money. Conserving by construction: it always has a
/// payer and a payee, so there is no way to write a transaction that
/// creates or destroys.
#[derive(Copy, Clone, Debug)]
pub struct Transfer {
    pub day: u64,
    pub from: Account,
    pub to: Account,
    pub amount: f64,
    pub why: Why,
}

/// Every balance, and the record of how they got that way.
pub struct Treasury {
    balances: BTreeMap<Account, f64>,
    /// What was in existence on day zero. Conservation is measured
    /// against this.
    opening: f64,
    /// The day's movements, cleared each tick. The full history is not
    /// kept — a year of a country's transactions is millions of entries
    /// and nothing reads them — but the day's are, because that is what
    /// makes a shortfall inspectable.
    pub today: Vec<Transfer>,
    /// Running totals by reason, for reporting.
    pub flows: BTreeMap<&'static str, f64>,
    /// **What could not be paid.** A firm that cannot make payroll does
    /// not pay a negative wage; it pays what it has, and the shortfall is
    /// recorded here because it is the thing worth knowing.
    pub unpaid: f64,
}

impl Treasury {
    pub fn new() -> Treasury {
        Treasury {
            balances: BTreeMap::new(),
            opening: 0.0,
            today: Vec::new(),
            flows: BTreeMap::new(),
            unpaid: 0.0,
        }
    }

    /// **Put money into the world**, once, at the start.
    ///
    /// The only way a balance is created without a payer, and it is
    /// deliberately not callable mid-run: a country's money stock changes
    /// through a central bank, and there is not one yet. Anything that
    /// wants to model credit creation has to say so explicitly rather than
    /// arriving through the back door.
    pub fn open(&mut self, account: Account, amount: f64) {
        *self.balances.entry(account).or_insert(0.0) += amount;
        self.opening += amount;
    }

    pub fn balance(&self, account: Account) -> f64 {
        self.balances.get(&account).copied().unwrap_or(0.0)
    }

    pub fn total(&self) -> f64 {
        self.balances.values().sum()
    }

    /// **Move money, and return what actually moved.**
    ///
    /// A payer cannot pay what it does not have, and the shortfall is the
    /// point rather than an inconvenience: it is how a firm discovers it
    /// cannot make payroll. `Abroad` is the exception — the rest of the
    /// world is not liquidity-constrained from this country's point of
    /// view — and the state may run a deficit, which real states do.
    pub fn pay(&mut self, day: u64, from: Account, to: Account, amount: f64, why: Why) -> f64 {
        if amount <= 0.0 || from == to {
            return 0.0;
        }
        let capped = match from {
            Account::Abroad | Account::State => amount,
            _ => amount.min(self.balance(from).max(0.0)),
        };
        if capped <= 1e-9 {
            self.unpaid += amount;
            return 0.0;
        }
        if capped < amount {
            self.unpaid += amount - capped;
        }
        *self.balances.entry(from).or_insert(0.0) -= capped;
        *self.balances.entry(to).or_insert(0.0) += capped;
        self.today.push(Transfer {
            day,
            from,
            to,
            amount: capped,
            why,
        });
        *self.flows.entry(reason_name(why)).or_insert(0.0) += capped;
        capped
    }

    /// Start of a new day.
    pub fn open_the_books(&mut self) {
        self.today.clear();
        self.unpaid = 0.0;
    }

    /// **Nothing appears and nothing vanishes.**
    ///
    /// The same guarantee `Ledger::assert_conserved` gives for tonnage.
    /// Every transfer has a payer and a payee, so this can only fail if
    /// somebody has reached past `pay` — which is exactly what it is here
    /// to catch.
    pub fn assert_conserved(&self) {
        let held = self.total();
        let drift = (held - self.opening).abs();
        // Measured against the stock of money, not against zero: a
        // currency that has changed hands a billion times accumulates
        // rounding.
        let tolerance = (self.opening.abs() * 1e-9).max(1e-6);
        assert!(
            drift <= tolerance,
            "money is not conserved: {held:.6} in existence against {:.6} issued, \
             a drift of {drift:.6}",
            self.opening
        );
    }

    /// Every account with money in it, for reporting.
    pub fn accounts(&self) -> impl Iterator<Item = (&Account, &f64)> {
        self.balances.iter()
    }
}

impl Default for Treasury {
    fn default() -> Self {
        Treasury::new()
    }
}

fn reason_name(why: Why) -> &'static str {
    match why {
        Why::Purchase => "purchases",
        Why::Supply => "supply",
        Why::Payroll => "payroll",
        Why::Freight => "freight",
        Why::Tax => "tax",
        Why::PublicSpending => "public spending",
        Why::Rent => "rent",
        Why::Profit => "profit",
        Why::Trade => "trade",
    }
}
