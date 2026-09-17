//! **What somebody is trying to do, and why they are still doing it.**
//!
//! Spec A4, the planner layer: marked settled, and until this existed not a
//! line of it was built. People did what the day's code told them — take
//! work at their own trade if any was going, and otherwise wait — and the
//! only decision anybody ever took about their own life was `leave_town`,
//! a pair of typed thresholds written for early testing.
//!
//! The owner's rule is that **the people in this world function as a player
//! does**. Somebody with no work at their trade does what anybody would: hears
//! that the kitchen is taking people on, weighs it against waiting for the
//! works to reopen, and goes and asks. That is a decision with a reason, a
//! plan that lasts beyond the day it was made, and an outcome that can go
//! wrong.
//!
//! ## What the spec settled, and where each piece lives
//!
//! | | |
//! |---|---|
//! | A4.1 no privileged channel | the player will use this same type |
//! | A4.2 opportunities are side-effects, **pushed, never scanned** | [`Lead`], posted by whoever knows of an opening |
//! | A4.3 goals with **hysteresis** | [`CLEARLY_BETTER`] |
//! | A4.4 plans **persist** | [`Plan`], held across days |
//! | A4.5 **four triggers** and nothing else | [`Trigger`] |
//! | A4.6 failure is **normal** | a lead that came to nothing is forgotten |
//! | A4.7 **expiring reservations** | kept by the caller, which owns the openings |
//! | A4.8 a **think budget** | [`thinks_allowed`] and [`take_turns`] |
//!
//! ## Deliberately small
//!
//! One goal and two steps. **Earn a living**: keep at your trade, or go
//! after an opening at another trade you are already qualified for. The
//! next templates — retrain, move for work, start something — are more
//! steps in the same library, scored on the same scale, which is the
//! reason the scale is expected earnings rather than a mood.

use crate::econ::Economy;
use crate::person::{day_rate, qualification_for, skill_premium, Person, Trade};

/// **How long experience takes to fade from what somebody expects.**
///
/// What a person believes about their chances is built from what has
/// actually happened to them, and recent weeks count for more than last
/// year. A half-life of a month is a designed figure: long enough that one
/// bad week does not persuade anybody their trade is finished, short enough
/// that a season of it does.
pub const BELIEF_HALF_LIFE_DAYS: f64 = 30.0;

/// **A week without work is when a plan to work at a trade is judged to be
/// failing.**
///
/// The week is the unit people actually count a spell out of work in —
/// unemployment claims are weekly in the United States and fortnightly in
/// Britain — and it is the "plan failed" trigger of A4.5 for somebody
/// looking.
pub const A_WEEK_OF_LOOKING: u32 = 7;

/// **The drift timer**, A4.5's fourth trigger, and the only one that fires
/// on a life that is going fine.
///
/// A quarter: often enough that somebody can quietly come to want
/// something else, rarely enough that nobody is re-deciding their life
/// every morning. Designed.
pub const DRIFT_DAYS: u64 = 91;

/// **How long a lead stays true.** The figure `person.rs` already uses for
/// a posted haul: a job nobody takes within a week has been taken by
/// somebody else.
pub const LEAD_LIFE_DAYS: u64 = 7;

/// **How much better something new has to look before anybody gives up
/// what they have for it** — A4.3's hysteresis.
///
/// Switching carries costs the arithmetic below does not see — the hold of
/// a contract, the people at work who know you, the plain risk of being
/// wrong about somebody else's workplace — so a new option must clearly
/// beat the current one rather than edge it, or people thrash between two
/// that are nearly equal. **A quarter is designed, not measured.** What
/// would measure it is the soak's occupational mobility against the real
/// figure of roughly a tenth of workers a year.
pub const CLEARLY_BETTER: f64 = 1.25;

/// **News good enough not to wait for the next review** — A4.5's
/// high-salience belief. Twice what somebody is making now is not a thing
/// anybody sits on for a quarter. Designed.
pub const TOO_GOOD_TO_WAIT: f64 = 2.0;

/// **How many leads somebody keeps in mind.** A handful; the oldest goes
/// when a new one arrives. Designed.
pub const LEADS_KEPT: usize = 6;

/// How somebody came to hear of an opening, which is part of what they
/// think it is worth.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Heard {
    /// **Somebody who works there said so**, and said how often they get
    /// work at it. About half of real jobs are found through people the
    /// seeker knows, which is why a well-connected person hears first.
    WordOfMouth,
    /// **The employer put it up.** Anybody looking can read it; it says a
    /// post is going and nothing about what the work is like.
    Notice,
}

/// **An opening somebody knows of.**
///
/// Not the opening itself — that belongs to the town — but one person's
/// belief about it, which can be out of date: the post may already have
/// gone, and the teller may have been luckier than the listener will be.
#[derive(Clone, Debug, PartialEq)]
pub struct Lead {
    pub trade: Trade,
    pub market: usize,
    /// **What the lead leads them to expect**: the share of days with work,
    /// as the teller has found it, or as the reader guesses from their own
    /// experience of this town when all they have is a notice.
    pub chance: f64,
    pub heard: u64,
    pub expires: u64,
    pub how: Heard,
}

/// What somebody is trying to achieve. One, for now.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Goal {
    /// Keep a roof and food by working.
    EarnALiving,
}

/// **One step of a plan**, from the library of task templates A4.4 asks
/// for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// **Work at a trade, or look for work at it**, and see how it is going
    /// by a day.
    Work { trade: Trade, review_on: u64 },
    /// **Go after an opening at another trade**, in one town, until a day.
    /// Holding a reservation on the opening while it lasts, so two people
    /// are not both told they have the same post.
    TryFor {
        trade: Trade,
        market: usize,
        by: u64,
    },
}

/// **A plan persists.** Taken on a trigger and executed on every day after
/// it, without being decided again.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub goal: Goal,
    pub steps: Vec<Step>,
    pub next: usize,
    pub begun: u64,
}

impl Plan {
    /// Keep at the trade they are in.
    pub fn keep_at(trade: Trade, day: u64, review_on: u64) -> Plan {
        Plan {
            goal: Goal::EarnALiving,
            steps: vec![Step::Work { trade, review_on }],
            next: 0,
            begun: day,
        }
    }

    /// Take up another trade: ask for the work, then do it.
    pub fn take_up(trade: Trade, market: usize, day: u64, by: u64) -> Plan {
        Plan {
            goal: Goal::EarnALiving,
            steps: vec![
                Step::TryFor { trade, market, by },
                Step::Work {
                    trade,
                    review_on: day + DRIFT_DAYS,
                },
            ],
            next: 0,
            begun: day,
        }
    }

    pub fn step(&self) -> Option<Step> {
        self.steps.get(self.next).copied()
    }
}

/// **The only four things that make somebody think again**, A4.5.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Trigger {
    StepCompleted,
    PlanFailed,
    Salient,
    Drift,
}

/// What a day came to, as far as earning a living goes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Had work today.
    Worked,
    /// Looked for work and found none.
    Looked,
    /// Neither — on the road, or not in a position to look.
    Neither,
}

/// **What somebody is trying to do, what they know that bears on it, and
/// whether something has told them to think again.**
#[derive(Clone, Debug, Default)]
pub struct Planner {
    pub plan: Option<Plan>,
    leads: Vec<Lead>,
    /// **What they expect of their own trade**: the share of days they get
    /// work at it, as their own experience has taught them. `None` until
    /// they have had any.
    expect: Option<f64>,
    looked_without_work: u32,
    /// Something has happened that A4.5 says is a reason to think again,
    /// and the think budget has not yet got round to them.
    pub pending: Option<Trigger>,
    /// Times they actually reconsidered — which the budget bounds, and
    /// which a gate can count.
    pub thoughts: u64,
    /// Plans that came to nothing.
    pub failures: u64,
    /// Times they went to work at a trade they had not been in.
    pub taken_up: u64,
}

/// **What somebody who has never had any experience of a trade expects of
/// it**: even odds. It stops mattering within a month of real experience.
const NO_EXPERIENCE_YET: f64 = 0.5;

impl Planner {
    pub fn new() -> Self {
        Self::default()
    }

    /// What they expect of their own trade.
    pub fn expectation(&self) -> f64 {
        self.expect.unwrap_or(NO_EXPERIENCE_YET)
    }

    /// The leads they are holding.
    pub fn leads(&self) -> &[Lead] {
        &self.leads
    }

    /// **The trade they are going after, if any, in this town.**
    ///
    /// The one thing the rest of a person's day asks of the plan: it lets
    /// them take work they would otherwise not have looked at.
    pub fn trying_for(&self, market: usize) -> Option<Trade> {
        match self.plan.as_ref()?.step()? {
            Step::TryFor {
                trade, market: m, ..
            } if m == market => Some(trade),
            _ => None,
        }
    }

    /// **A day happened.** Learn from it, and notice whether it is a reason
    /// to think again.
    ///
    /// `drift_offset` spreads everybody's quarterly review across the
    /// quarter, so a whole town does not reconsider its life on one day.
    pub fn observe(
        &mut self,
        day: u64,
        trade: Trade,
        market: usize,
        outcome: Outcome,
        drift_offset: u64,
    ) {
        if self.plan.is_none() {
            self.plan = Some(Plan::keep_at(
                trade,
                day,
                day + 1 + drift_offset % DRIFT_DAYS,
            ));
        }

        // **Experience moves expectation**, on the half-life.
        let keep = 0.5f64.powf(1.0 / BELIEF_HALF_LIFE_DAYS);
        match outcome {
            Outcome::Worked => {
                self.expect = Some(self.expectation() * keep + (1.0 - keep));
                self.looked_without_work = 0;
            }
            Outcome::Looked => {
                self.expect = Some(self.expectation() * keep);
                self.looked_without_work += 1;
            }
            Outcome::Neither => {}
        }
        self.leads.retain(|l| l.expires > day);

        let Some(step) = self.plan.as_ref().and_then(|p| p.step()) else {
            self.plan = Some(Plan::keep_at(trade, day, day + DRIFT_DAYS));
            return;
        };
        match step {
            Step::TryFor {
                trade: wanted,
                market: there,
                by,
            } => {
                if trade == wanted {
                    // **Taken on.** The step is done and the next one —
                    // doing the work — begins.
                    if let Some(p) = self.plan.as_mut() {
                        p.next += 1;
                    }
                    self.taken_up += 1;
                    self.raise(Trigger::StepCompleted);
                } else if market != there || day >= by {
                    // **It came to nothing.** The post had gone, or the
                    // odds were never what the teller's were. Normal, not
                    // an error: the lead is forgotten, because what it
                    // said is no longer believed.
                    self.leads
                        .retain(|l| !(l.trade == wanted && l.market == there));
                    self.failures += 1;
                    self.plan = Some(Plan::keep_at(trade, day, day + DRIFT_DAYS));
                    self.raise(Trigger::PlanFailed);
                }
            }
            Step::Work {
                trade: at,
                review_on,
            } => {
                if at != trade {
                    // Their trade changed under the plan — made up to
                    // supervisor, most often. The plan follows the life.
                    self.plan = Some(Plan::keep_at(trade, day, review_on.max(day + 1)));
                } else if outcome == Outcome::Looked
                    && self.looked_without_work >= A_WEEK_OF_LOOKING
                    && self.looked_without_work.is_multiple_of(A_WEEK_OF_LOOKING)
                {
                    self.raise(Trigger::PlanFailed);
                } else if day >= review_on {
                    if let Some(p) = self.plan.as_mut() {
                        p.steps[p.next] = Step::Work {
                            trade: at,
                            review_on: day + DRIFT_DAYS,
                        };
                    }
                    self.raise(Trigger::Drift);
                }
            }
        }
    }

    fn raise(&mut self, why: Trigger) {
        if self.pending.is_none() {
            self.pending = Some(why);
        }
    }

    /// **Somebody told them, or they read it.**
    ///
    /// Pushed, never scanned: this is the only way a lead arrives. `worth`
    /// is what the lead is worth to *them* and `now` what their present
    /// course is — the caller works both out, because both depend on who
    /// they are. News worth twice what they have does not wait for a
    /// review.
    pub fn hear(&mut self, lead: Lead, worth: f64, now: f64) {
        if let Some(have) = self
            .leads
            .iter_mut()
            .find(|l| l.trade == lead.trade && l.market == lead.market)
        {
            // Heard again: fresher, and the latest word on it.
            have.expires = have.expires.max(lead.expires);
            have.heard = lead.heard;
            have.chance = lead.chance;
            have.how = lead.how;
        } else {
            self.leads.push(lead);
            if self.leads.len() > LEADS_KEPT {
                // The one about to go stale anyway.
                let (i, _) = self
                    .leads
                    .iter()
                    .enumerate()
                    .min_by_key(|(i, l)| (l.expires, *i))
                    .expect("not empty");
                self.leads.remove(i);
            }
        }
        if worth > TOO_GOOD_TO_WAIT * now.max(1e-9) {
            self.raise(Trigger::Salient);
        }
    }

    /// **Think again.** Run only when a trigger is pending and the think
    /// budget allows.
    ///
    /// Weighs going on as they are against every lead they hold for work
    /// they are allowed to do in the town they are in, on one scale —
    /// what a day of it is worth to them, at their own skill, times how
    /// often they expect to get it — and changes course only for something
    /// that clearly beats what they are doing.
    ///
    /// `reserve` is asked before a plan is committed to, and says whether
    /// there is still an opening to go after; it takes the opening if so.
    /// Returns the trade they have set out to take up, if any.
    pub fn reconsider(
        &mut self,
        who: &Person,
        econ: &Economy,
        day: u64,
        mut reserve: impl FnMut(usize, Trade) -> bool,
    ) -> Option<Trade> {
        self.pending = None;
        self.thoughts += 1;
        self.leads.retain(|l| l.expires > day);

        // What they are doing now is worth: their own trade on their own
        // experience, or — if they are already going after something — that.
        let chasing = self.trying_for(who.market);
        let now = match chasing {
            Some(t) => self
                .leads
                .iter()
                .find(|l| l.trade == t && l.market == who.market)
                .map(|l| worth_to(who, econ, l.trade, l.chance))
                .unwrap_or_else(|| worth_to(who, econ, who.trade, self.expectation())),
            None => worth_to(who, econ, who.trade, self.expectation()),
        };

        let mut options: Vec<(f64, Lead)> = self
            .leads
            .iter()
            .filter(|l| {
                l.market == who.market
                    && l.trade != who.trade
                    && Some(l.trade) != chasing
                    && l.trade != Trade::Supervisor
                    && who.qualification >= qualification_for(l.trade)
            })
            .map(|l| (worth_to(who, econ, l.trade, l.chance), l.clone()))
            .collect();
        options.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then(a.1.trade.index().cmp(&b.1.trade.index()))
        });

        for (worth, lead) in options {
            if worth <= now * CLEARLY_BETTER {
                break;
            }
            if reserve(lead.market, lead.trade) {
                self.plan = Some(Plan::take_up(
                    lead.trade,
                    lead.market,
                    day,
                    lead.expires.max(day + 1),
                ));
                return Some(lead.trade);
            }
        }
        None
    }
}

/// **What a day of a trade is worth to this person**, in the currency of
/// the town they are in: the going rate, what their own skill at it makes
/// of that, and how often they expect to get the work.
///
/// One scale for every option, which is what lets the next templates —
/// retraining, moving, starting something — be weighed against these
/// rather than bolted on beside them.
pub fn worth_to(who: &Person, econ: &Economy, trade: Trade, chance: f64) -> f64 {
    day_rate(econ, who.market, trade)
        * skill_premium(who.level(trade.skill()), trade.wants_level())
        * chance.clamp(0.0, 1.0)
}

/// **How many people in one town may think again today** — A4.8's hard
/// cap. An eighth of the sample, and never fewer than two. Designed.
pub fn thinks_allowed(people_in_town: usize) -> usize {
    (people_in_town / 8).max(2)
}

/// **Whose turn it is to think**, from those waiting.
///
/// Takes up to `allowed`, starting at a place that moves with the day, so
/// the same few never go first every morning and nobody waits for ever.
/// Everybody not chosen keeps their trigger for tomorrow.
pub fn take_turns<T: Copy>(waiting: &[T], allowed: usize, day: u64) -> Vec<T> {
    if waiting.is_empty() || allowed == 0 {
        return Vec::new();
    }
    let n = waiting.len();
    let start = (day as usize).wrapping_mul(7) % n;
    (0..n.min(allowed))
        .map(|i| waiting[(start + i) % n])
        .collect()
}
