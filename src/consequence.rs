//! **How a world fact becomes something that happened to somebody.**
//!
//! The seam between the simulation and the mind, and the reason it is a
//! module rather than a call: **`labour.rs` must not reach into a mind.**
//! It reports what is objectively so; this commits that to history; and
//! only then does anything get appraised.
//!
//! ```text
//! transformer fails
//!   → power unavailable            (infrastructure decides this, and only this)
//!   → production reduced
//!   → the employer decides         (inventory, liquidity, expected repair)
//!   → household income changes
//!   → the worker learns of it      (and not everybody does)
//!   → befall translates what they learned
//!   → appraisal, coping, memory, well-being
//!   → and a conversation exposes the person that made
//! ```
//!
//! # Nobody appraises the causal chain
//!
//! The simulator knows that a transformer failed and that this will cost
//! a man his job. **He does not.** What reaches him is a sequence of
//! separate facts, each with its own day, its own source and its own
//! consequence — the mill has stopped; tomorrow's shift is off; this
//! week's pay is short; you are finished here; the rent cannot be paid.
//!
//! Handing him the chain instead would be telepathy of the same kind
//! `social.rs` was built to make impossible, and it would let him despair
//! on Monday about something that has not happened by Friday.
//!
//! # The transformer does not fire anybody
//!
//! Infrastructure decides how long the power is off. **Whether anybody
//! loses work is the employer's decision**, and it turns on stock in the
//! yard, money in the bank, how long the repair is expected to take and
//! whether there is any demand for the output. A short outage with a
//! full warehouse costs nobody their job; the same outage at a firm
//! living hand to mouth empties the place.

use crate::befall::Circumstance;
use crate::coping::ActualControl;
use crate::labour::Workforce;
use crate::id::Id;
use crate::memory::Source;
use crate::person::Person;
use crate::save::{EventId, Journal, JournalKey};

// ---------------------------------------------------------------------
// what is objectively so
// ---------------------------------------------------------------------

/// **One fact, as the world knows it.**
///
/// Separate rather than a single "he lost his job", because they arrive
/// on different days, from different people, with different weight — and
/// somebody can learn one and not the next.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fact {
    /// The works are not running.
    WorkStopped,
    /// There is no shift tomorrow.
    ShiftCancelled,
    /// This week's pay is short.
    PayReduced { share_lost: f64 },
    /// **You are finished here.**
    EmploymentEnded,
    /// The household cannot meet something it has to meet.
    CannotMeetExpense { short_by_days: f64 },
}

impl Fact {
    /// How hard it lands, before anybody's disposition touches it.
    pub fn weight(self) -> f64 {
        match self {
            Fact::WorkStopped => 0.15,
            Fact::ShiftCancelled => 0.25,
            Fact::PayReduced { share_lost } => 0.2 + 0.5 * share_lost.clamp(0.0, 1.0),
            Fact::EmploymentEnded => 0.8,
            Fact::CannotMeetExpense { short_by_days } => {
                (0.45 + 0.05 * short_by_days.clamp(0.0, 10.0)).min(1.0)
            }
        }
    }

    /// Whether this is the sort of thing somebody is told outright.
    pub fn is_notified(self) -> bool {
        matches!(self, Fact::ShiftCancelled | Fact::EmploymentEnded | Fact::PayReduced { .. })
    }
}

/// A fact, dated and identified, as committed to history.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Happened {
    pub what: Fact,
    pub day: u64,
    pub event: EventId,
}

// ---------------------------------------------------------------------
// the employer, who is the one who decides
// ---------------------------------------------------------------------

/// **What the firm has to absorb an outage with.**
///
/// This is the intermediate ownership the chain must not skip. A grid
/// fault is a fact about power; a redundancy is a fact about the firm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Employer {
    /// Finished goods in the yard, in days of sales.
    pub inventory_days: f64,
    /// How long payroll can be met with nothing coming in.
    pub liquidity_days: f64,
    /// **Whether they keep people on through a stoppage.** Real firms
    /// hoard labour: rehiring and retraining costs more than a few idle
    /// weeks, which is why a short outage costs nobody their job.
    pub retains_labour: f64,
    /// What the repair is *expected* to take, which is what decisions
    /// are made on — not what it turns out to take.
    pub expected_repair_days: f64,
    /// Anything else that can turn the wheels.
    pub alternative_power: f64,
    /// Whether the output is wanted. A stoppage in a slump is an excuse.
    pub demand: f64,
}

impl Default for Employer {
    fn default() -> Self {
        Employer {
            inventory_days: 20.0,
            liquidity_days: 45.0,
            retains_labour: 0.7,
            expected_repair_days: 3.0,
            alternative_power: 0.0,
            demand: 0.8,
        }
    }
}

impl Employer {
    /// **What the firm does about an outage of this length.**
    ///
    /// Returns the facts in the order they would arise, which is also the
    /// order they would be learned.
    pub fn decides(&self, outage_days: f64, from_day: u64, seq: &mut u64) -> Vec<Happened> {
        let mut out = Vec::new();
        if outage_days <= 0.0 {
            return out;
        }
        let mut give = |what: Fact, day: u64, out: &mut Vec<Happened>| {
            *seq += 1;
            out.push(Happened { what, day, event: *seq });
        };

        // Power off is a fact whatever else follows.
        if self.alternative_power < 0.5 {
            give(Fact::WorkStopped, from_day, &mut out);
        } else {
            return out;
        }

        // **Stock absorbs the first part of it.** A full warehouse means
        // the customers never notice and the hands keep working.
        let bites = outage_days - self.inventory_days;
        if bites <= 0.0 {
            return out;
        }

        give(Fact::ShiftCancelled, from_day + self.inventory_days.max(0.0) as u64, &mut out);

        // Pay follows the shifts once the firm stops carrying them.
        let carried = self.liquidity_days * self.retains_labour;
        if bites > carried {
            let lost = ((bites - carried) / bites).clamp(0.1, 1.0);
            give(
                Fact::PayReduced { share_lost: lost },
                from_day + (self.inventory_days + carried).max(0.0) as u64,
                &mut out,
            );
        }

        // **And only then is anybody let go**, and only if the firm
        // expects the stoppage to outlast what it can carry and there is
        // no reason to hold on to them.
        let hopeless = self.expected_repair_days > self.inventory_days + carried;
        let unwanted = self.demand < 0.5;
        if hopeless || (unwanted && bites > carried) {
            give(
                Fact::EmploymentEnded,
                from_day + (self.inventory_days + carried).max(0.0) as u64 + 1,
                &mut out,
            );
        }
        out
    }
}

// ---------------------------------------------------------------------
// what a person can actually do about it
// ---------------------------------------------------------------------

/// **Opportunity is not control.**
///
/// `Workforce::chance_of_work` says how much work there is here. Whether
/// *this* person can get any of it is a further question: they may have
/// no way to travel to it, no ticket for it, no time to look, nobody to
/// mind the children, and nowhere else to go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reemployment {
    /// Suitable vacancies, from the labour market.
    pub opportunity: f64,
    /// Transport, distance, hours.
    pub accessibility: f64,
    pub qualification_fit: f64,
    /// Time and condition to look at all.
    pub ability_to_search: f64,
    pub ability_to_retrain: f64,
    pub ability_to_relocate: f64,
}

impl Default for Reemployment {
    fn default() -> Self {
        Reemployment {
            opportunity: 0.5,
            accessibility: 0.7,
            qualification_fit: 0.7,
            ability_to_search: 0.8,
            ability_to_retrain: 0.3,
            ability_to_relocate: 0.2,
        }
    }
}

impl Reemployment {
    /// Read the market half from the labour model rather than inventing
    /// it. What the *person* can do stays where it was.
    pub fn in_this_town(w: &Workforce, personal: Reemployment) -> Self {
        Reemployment { opportunity: w.chance_of_work(), ..personal }
    }

    /// **What is actually so**, for `coping::resolve` to settle against.
    ///
    /// Never for choosing a strategy — that reads what the person
    /// *believes*, which is a separate thing and frequently wrong in
    /// both directions.
    pub fn actual(&self, runway_days: f64) -> ActualControl {
        let reach = self.opportunity.clamp(0.0, 1.0)
            * (0.35 + 0.65 * self.accessibility.clamp(0.0, 1.0))
            * (0.35 + 0.65 * self.qualification_fit.clamp(0.0, 1.0));
        ActualControl {
            source: reach,
            // **Runway is what makes consequences bearable**, not a
            // sum of money: what matters is how many days it buys.
            consequences: (runway_days / 90.0).clamp(0.0, 1.0),
            exit: self.ability_to_relocate.clamp(0.0, 1.0),
            means: (0.25 * self.ability_to_search
                + 0.15 * self.ability_to_retrain
                + 0.6 * (runway_days / 60.0).clamp(0.0, 1.0))
            .clamp(0.0, 1.0),
        }
    }
}

/// **How long the money lasts**, which is the figure that matters and
/// not the balance.
///
/// Dependants, debt, the price of bread and whatever the household is
/// owed all move it without anything here needing to know about them:
/// they are already in the two numbers.
pub fn runway_days(liquid: f64, essential_per_day: f64) -> f64 {
    if essential_per_day <= 1e-9 {
        return f64::INFINITY;
    }
    (liquid / essential_per_day).max(0.0)
}

// ---------------------------------------------------------------------
// who finds out
// ---------------------------------------------------------------------

/// **Whether this person learns this fact at all, and how.**
///
/// A stranger does not acquire something merely because it is true. The
/// employee is told; the household finds out when the money does not
/// arrive; anybody else hears it from somebody or not at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// It is happening to them.
    TheWorker,
    /// They live on the same money.
    Household,
    /// They work there too.
    Workmate,
    /// Anybody else.
    Otherwise,
}

/// How somebody in this position comes to know, if they do.
///
/// **Being told requires somebody to have told you**, which is why the
/// teller is an argument and not a placeholder: a fact that arrives by
/// word of mouth carries who carried it, and memory keeps that for
/// ever.
pub fn learns(who: Standing, what: Fact, teller: Id<Person>) -> Option<Source> {
    match who {
        Standing::TheWorker => Some(if what.is_notified() {
            Source::Told { by: teller }
        } else {
            Source::Witnessed
        }),
        Standing::Household => match what {
            // The household finds out about the money because the money
            // is theirs; the shift roster is not.
            Fact::PayReduced { .. } | Fact::CannotMeetExpense { .. } => Some(Source::Witnessed),
            Fact::EmploymentEnded => Some(Source::Told { by: teller }),
            _ => None,
        },
        Standing::Workmate => match what {
            Fact::WorkStopped | Fact::ShiftCancelled => Some(Source::Witnessed),
            Fact::EmploymentEnded => Some(Source::Overheard),
            _ => None,
        },
        // **Nothing.** Not a small amount — nothing.
        Standing::Otherwise => None,
    }
}

// ---------------------------------------------------------------------
// the orchestrator
// ---------------------------------------------------------------------

/// **Commit what happened, then tell whoever was in a position to know.**
///
/// The order is the point: the objective fact is history *before* any
/// mind consumes it, and it is applied exactly once however the world is
/// being stepped.
pub struct Consequences;

impl Consequences {
    /// Record the facts. Returns those that were newly committed —
    /// replaying the same day commits nothing twice.
    pub fn commit(journal: &mut Journal, world_seed: u64, facts: &[Happened]) -> Vec<Happened> {
        let mut fresh = Vec::new();
        for f in facts {
            if journal.already(f.event).is_some() {
                continue;
            }
            let seq = journal.next_sequence();
            journal.resolve(
                world_seed,
                JournalKey { time: f.day, phase: 1, sequence: seq, event: f.event },
                "what came of it",
            );
            fresh.push(*f);
        }
        fresh
    }

    /// **What one person makes of what they learned.**
    ///
    /// Only facts that reached them, and only once each: `apply_once` is
    /// what makes a replay after a crash deliver the news rather than
    /// re-deliver it.
    pub fn delivered_to(
        journal: &mut Journal,
        who: Standing,
        teller: Id<Person>,
        facts: &[Happened],
    ) -> Vec<(Happened, Source)> {
        let mut got = Vec::new();
        for f in facts {
            let Some(how) = learns(who, f.what, teller) else {
                continue;
            };
            if journal.apply_once(f.event).is_some() || journal.already(f.event).is_some() {
                got.push((*f, how));
            }
        }
        got
    }

    /// Turn what they learned into the circumstance `befall` reads.
    ///
    /// **Nothing here knows about transformers.** It knows this man has
    /// lost his work, what the market is like, and how long his money
    /// lasts.
    pub fn circumstance(
        got: &[(Happened, Source)],
        market: &Workforce,
        runway: f64,
        essential_per_day: f64,
    ) -> Circumstance {
        let lost_work = got.iter().any(|(h, _)| h.what == Fact::EmploymentEnded);
        let _ = essential_per_day;
        Circumstance {
            lost_work,
            found_work: false,
            chance_of_work: market.chance_of_work(),
            days_of_savings: runway,
            could_leave: 0.2,
            hungry_days: 0,
            homeless: false,
            bereaved: false,
            maimed: false,
        }
    }
}
