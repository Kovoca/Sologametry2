//! **What the world has done to somebody.**
//!
//! The mind reacts to events and, until this existed, **nothing in the
//! world produced any**. The seam was there — `scaling::What` — and the
//! only thing that ever pushed a stressor through it was the demo
//! binary, by hand. Which meant the honest description of the mind
//! stack was: not random, and not caused either.
//!
//! This module **invents nothing**. Every input is a fact some other
//! part of the simulation already computes: whether somebody is out of
//! work, how many hands in their town are working, how long they have
//! gone hungry, whether they have a roof, whether somebody died. What it
//! does is translate those into the events a mind can appraise.
//!
//! # The important part is not the trigger, it is the controllability
//!
//! A job loss is not one event. In a town where nine hands in ten are
//! working it is a setback with an obvious remedy; in a town where three
//! are, it is a wall. `coping.rs` already distinguishes what somebody
//! *believes* they can do from what they actually can — and this is
//! where the second one comes from. **The world decides how controllable
//! a misfortune is, and nothing is rolled to make that so.**

use crate::coping::ActualControl;
use crate::growth::ShapesWellbeing;
use crate::scaling::{Change, Standing, What};

/// **Facts, computed elsewhere.**
///
/// Every field here is something another module already knows:
/// `labour::Workforce` knows what share of hands are working,
/// `person::Person` knows about hunger and a roof, `populace` knows who
/// died.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circumstance {
    /// Lost work today.
    pub lost_work: bool,
    /// Found it again today.
    pub found_work: bool,
    /// **The share of hands in this town that are working**, straight
    /// from `labour::Workforce::chance_of_work`. This is what turns one
    /// misfortune into two different misfortunes.
    pub chance_of_work: f64,
    /// Days of food they can pay for — what makes the consequences of
    /// losing work survivable or not.
    pub days_of_savings: f64,
    /// Whether they could go somewhere else: fare, a vehicle, anywhere
    /// to go.
    pub could_leave: f64,
    /// Consecutive days short of food.
    pub hungry_days: u32,
    pub homeless: bool,
    /// Somebody close to them died today.
    pub bereaved: bool,
    /// Hurt badly enough that it will not simply mend.
    pub maimed: bool,
}

impl Default for Circumstance {
    fn default() -> Self {
        Circumstance {
            lost_work: false,
            found_work: false,
            chance_of_work: 0.8,
            days_of_savings: 30.0,
            could_leave: 0.3,
            hungry_days: 0,
            homeless: false,
            bereaved: false,
            maimed: false,
        }
    }
}

impl Circumstance {
    /// **What losing this job actually leaves somebody able to do.**
    ///
    /// Not a constant, and not a draw. `source` is whether another job
    /// exists to be got; `consequences` is whether savings can absorb it;
    /// `means` is whether they can act at all; `exit` is whether they can
    /// leave. The same event in two towns is two events.
    pub fn control_over_losing_work(&self) -> ActualControl {
        ActualControl {
            source: self.chance_of_work.clamp(0.0, 1.0),
            // A month in hand makes the consequences manageable; nothing
            // in hand makes them immediate.
            consequences: (self.days_of_savings / 60.0).clamp(0.0, 1.0),
            exit: self.could_leave.clamp(0.0, 1.0),
            means: (0.2 + 0.6 * (self.days_of_savings / 60.0).clamp(0.0, 1.0)).min(1.0),
        }
    }

    /// How bad it is to be out of work here, 0..1. Thin labour markets
    /// and empty pockets, which is the combination that ruins people.
    pub fn severity_of_losing_work(&self) -> f64 {
        let no_prospects = 1.0 - self.chance_of_work.clamp(0.0, 1.0);
        let no_cushion = 1.0 - (self.days_of_savings / 60.0).clamp(0.0, 1.0);
        (0.35 + 0.35 * no_prospects + 0.30 * no_cushion).clamp(0.0, 1.0)
    }
}

/// **Turn what happened into what a mind can appraise.**
///
/// `event` is bumped for each thing emitted, so every happening has a
/// stable id — which is what `Strain::appraise` needs in order to feel
/// one trouble once rather than once a day, and what the journal needs
/// in order to resolve an outcome once rather than every time it is
/// asked.
pub fn befell(c: &Circumstance, day: u64, event: &mut u64) -> Vec<Change> {
    let mut out = Vec::new();
    let mut next = || {
        *event += 1;
        *event
    };

    if c.lost_work {
        let _ = next();
        out.push(Change {
            day,
            what: What::StressorBegins(Standing {
                severity: c.severity_of_losing_work(),
                since: day,
                // Left alone it gets worse: savings run down, a
                // reference goes stale, the trade moves on.
                worsens_if_ignored: 0.75,
                actual: c.control_over_losing_work(),
            }),
        });
        // **And the well-being injury, in its own layer.** Losing work is
        // a life-satisfaction result and lands there, not on the traits.
        out.push(Change {
            day,
            what: What::Wellbeing { by: ShapesWellbeing::LostWork, strength: 1.0, toward: -1.0 },
        });
    }

    if c.found_work {
        out.push(Change { day, what: What::StressorEnds });
    }

    if c.bereaved {
        let _ = next();
        out.push(Change {
            day,
            what: What::Wellbeing {
                by: ShapesWellbeing::Bereavement,
                strength: 1.0,
                toward: -1.0,
            },
        });
    }

    if c.maimed {
        let _ = next();
        out.push(Change {
            day,
            what: What::Wellbeing {
                by: ShapesWellbeing::Impairment,
                strength: 1.0,
                toward: -1.0,
            },
        });
    }

    // **Hunger and no roof are troubles in their own right**, and they
    // are nearly uncontrollable from inside — which is exactly why
    // `person.rs` finds homelessness self-sustaining.
    if c.hungry_days >= 7 {
        let _ = next();
        out.push(Change {
            day,
            what: What::StressorBegins(Standing {
                severity: (0.3 + 0.05 * c.hungry_days as f64).min(1.0),
                since: day,
                worsens_if_ignored: 0.9,
                actual: ActualControl {
                    source: 0.15 * c.chance_of_work,
                    consequences: 0.2,
                    exit: c.could_leave * 0.5,
                    means: 0.15,
                },
            }),
        });
    }

    if c.homeless {
        let _ = next();
        out.push(Change {
            day,
            what: What::StressorBegins(Standing {
                severity: 0.8,
                since: day,
                worsens_if_ignored: 0.85,
                actual: ActualControl {
                    source: 0.1 * c.chance_of_work,
                    consequences: 0.15,
                    exit: c.could_leave,
                    // Getting back in costs a deposit and a month up
                    // front, which is the real barrier `person.rs`
                    // already models.
                    means: (c.days_of_savings / 90.0).clamp(0.0, 1.0),
                },
            }),
        });
    }

    // Support is not a fact about the person; it is a fact about who is
    // still around. A town where nobody is working has less to spare.
    if c.lost_work || c.homeless {
        out.push(Change {
            day,
            what: What::SupportChanges((0.25 + 0.5 * c.chance_of_work).clamp(0.0, 1.0)),
        });
    }

    out
}
