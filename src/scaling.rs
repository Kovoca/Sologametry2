//! **What a person is when nobody is looking at them.**
//!
//! Mind spec sections 25 and 26 — update frequencies, event-driven
//! wherever possible; and scaling across loaded, settlement and distant.
//! The design doc's rule for the whole project is that **populations stay
//! statistical until attention or consequence promotes them**, and this
//! is where that is made to mean something specific.
//!
//! # The invariant
//!
//! **Changing fidelity must not change the person.** A man simulated in
//! detail for two years and the same man advanced once over the same two
//! years have to come out in the same place, or his mind depends on
//! whether the engine happened to be looking at him — which is the same
//! fault the strain ladder had before `Strain::advance` became analytic,
//! one level up.
//!
//! Where that cannot hold exactly it is **named and bounded** rather than
//! quietly hoped for. See `pressure_is_averaged_and_that_understates_it`
//! in the tests: strain accumulates faster than it recovers, so a coarse
//! record fed the *mean* of a pressure that swung either side of
//! tolerance comes out better off than the man actually was. The
//! asymmetry is real, the direction is always the same, and the bound is
//! asserted.
//!
//! # What is kept, and what is thrown away
//!
//! The rule that makes a distant person cheap is the project's oldest
//! one: **generated, never stored**. A personality is drawn from a seed,
//! so it is not saved — it is redrawn. What *is* saved is only what life
//! did to it, which `growth::Growth` already holds as a list of dated
//! changes that evaluate analytically at any date.
//!
//! Individual memories are the thing genuinely lost on demotion, and that
//! is honest rather than regrettable: a distant person keeps the memories
//! that **shaped** them, because those are core memories and core
//! memories are already recorded as growth.

use crate::coping::{
    propensities, Acute, ActualControl, Circumstances, ControlAppraisal, Coping,
    FunctionalState, Strain, ENTER, LEAVE,
};
use crate::growth::Growth;
use crate::id::Id;
use crate::mind::{Facet, Mind, Value};
use crate::person::Person;
use crate::rng::Rng;

/// **The version of the generator that made somebody.**
///
/// Bumped whenever the draw could produce a different person: a new
/// facet, a changed loading, a different RNG or draw order. A record
/// carrying an older schema is one whose seed can no longer be trusted
/// to reproduce anybody — which is exactly why the baseline is saved
/// rather than re-derived.
pub const GENERATION_SCHEMA: u32 = 1;

/// What a person was made from, kept so a save can say whether the
/// generator that made them still exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PersonOrigin {
    pub seed: u64,
    pub schema: u32,
    /// The day they were born, which is an input to who they are and
    /// must never be recomputed from anything current.
    pub born: u64,
}

/// **How closely somebody is being simulated.**
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fidelity {
    /// Everything: tiles, perception, memory traces, conversation.
    Loaded,
    /// In a town, with no position finer than which market they are in.
    /// `witness.rs` already has this tier for perception.
    Settlement,
    /// A number in a population, advanced in one step when asked.
    Distant,
}

// ---------------------------------------------------------------------
// what a distant person carries
// ---------------------------------------------------------------------

/// **A standing stressor**, which is the thing a distant person is
/// actually living with. Cheap, dated, and enough to resume from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Standing {
    pub severity: f64,
    pub since: u64,
    /// Whether leaving it alone makes it worse — the term that decides
    /// what avoidance costs.
    pub worsens_if_ignored: f64,
    pub actual: ActualControl,
}

/// **Learned coping habits.**
///
/// What somebody has come to reach for, as distinct from what their
/// personality inclines them to. This is where "entrenched coping
/// habits" after a long bad stretch actually live, and it is why two
/// people of identical temperament can face the same trouble differently
/// twenty years apart.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Habits {
    weight: [f32; 14],
}

impl Default for Habits {
    fn default() -> Self {
        Habits { weight: [0.0; 14] }
    }
}

fn index_of(c: Coping) -> usize {
    Coping::ALL.iter().position(|x| *x == c).unwrap()
}

/// How fast a habit sets. Coping style is moderately stable, so this is
/// slow: a strategy used daily takes months to become the first thing
/// somebody reaches for.
pub const HABIT_PER_USE: f32 = 0.004;
pub const HABIT_CEILING: f32 = 0.8;

impl Habits {
    pub fn of(&self, c: Coping) -> f32 {
        self.weight[index_of(c)]
    }

    /// Used it, `times` days running. It becomes very slightly more his
    /// way of doing things.
    ///
    /// **Written so that one call for `n` days equals `n` calls for one**,
    /// which is what lets a distant person keep the same habits as a
    /// loaded one. The gain clamps at the end because the series is
    /// monotone, and the crowding-out compounds because that is what
    /// repeating it daily actually does.
    pub fn used(&mut self, c: Coping, times: u32) {
        if times == 0 {
            return;
        }
        let i = index_of(c);
        self.weight[i] = (self.weight[i] + HABIT_PER_USE * times as f32).min(HABIT_CEILING);
        // What is practised crowds out what is not, or everything drifts
        // upward together and the profile says nothing.
        let per_day = 1.0 - 0.15 * HABIT_PER_USE;
        let decay = per_day.powi(times as i32);
        for (j, w) in self.weight.iter_mut().enumerate() {
            if j != i {
                *w *= decay;
            }
        }
    }

    /// Put a weight back, for a reload.
    pub fn set(&mut self, c: Coping, w: f32) {
        self.weight[index_of(c)] = w;
    }

    pub fn strongest(&self) -> Coping {
        let mut best = (Coping::ALL[0], f32::MIN);
        for c in Coping::ALL {
            let w = self.of(c);
            if w > best.1 {
                best = (c, w);
            }
        }
        best.0
    }
}

/// **Everything a person out of sight must carry**, and nothing else.
///
/// The list is short on purpose: what is here is what a promotion needs
/// in order not to invent anybody.
#[derive(Clone, Debug, PartialEq)]
pub struct Coarse {
    pub who: Id<Person>,
    /// Where the person came from. **A seed is not sufficient save
    /// state** — see `PersonOrigin`.
    pub origin: PersonOrigin,
    /// **The immutable baseline, saved outright.**
    ///
    /// "Generated, never stored" is right for terrain, which is a pure
    /// function of coordinates that nothing has a stake in. It is wrong
    /// for a person: the same seed produces a *different* human being if
    /// the RNG, the draw order, the facet count, the loadings, the
    /// culture or the inheritance arithmetic ever change — and
    /// inheritance is the worst of it, since a baseline recomputed from
    /// parents who have since aged and adapted is not the baseline
    /// anybody was born with.
    ///
    /// Twenty-five numbers are nothing. Saving them removes an entire
    /// class of version fragility, and the seed stays for everything
    /// that is genuinely regenerable.
    pub baseline: Vec<f32>,
    /// What life did to that drawn personality, dated and analytic.
    pub growth: Growth,
    pub strain: Strain,
    pub perceived_control: ControlAppraisal,
    pub habits: Habits,
    pub standing: Vec<Standing>,
    /// Raised and not yet settled.
    pub attempts_outstanding: u16,
    /// Whether they believe anybody would help if asked. Distinct from
    /// support actually received, which cannot be known in advance.
    pub support_expected: f64,
    /// **When this record was last brought up to date.** Without it a
    /// coarse advance has no interval to advance over.
    pub last_update: u64,
}

impl Coarse {
    /// Build a record for somebody, taking the baseline the world drew
    /// for them so it is never derived twice.
    pub fn of(who: Id<Person>, origin: PersonOrigin, mind: &Mind, day: u64) -> Self {
        let mut c = Coarse::new(who, origin.seed, day);
        c.origin = origin;
        c.baseline = Facet::ALL.iter().map(|&f| mind.person.baseline_of(f)).collect();
        c
    }

    pub fn new(who: Id<Person>, seed: u64, day: u64) -> Self {
        Coarse {
            who,
            origin: PersonOrigin { seed, schema: GENERATION_SCHEMA, born: 0 },
            baseline: Vec::new(),
            growth: Growth::new(),
            strain: Strain::default(),
            perceived_control: ControlAppraisal::default(),
            habits: Habits::default(),
            standing: Vec::new(),
            attempts_outstanding: 0,
            support_expected: 0.5,
            last_update: day,
        }
    }

    /// What is pressing on them, summed and capped.
    pub fn pressure(&self) -> f64 {
        self.standing.iter().map(|s| s.severity).sum::<f64>().min(1.0)
    }

    /// **Bring the record up to a date in one step.**
    ///
    /// Event-driven, per spec section 25: it advances to the next stage
    /// boundary rather than in fixed ticks, because between boundaries
    /// nothing about the person changes qualitatively. Solvable because
    /// `Strain`'s load path is linear in time at constant pressure.
    ///
    /// The coping choice is recomputed at each boundary and held between
    /// them. That is exact wherever the choice would not have changed
    /// anyway, which is the ordinary case — somebody's way of dealing
    /// with a standing trouble does not change on a Tuesday.
    pub fn advance_to(&mut self, day: u64, mind: &Mind, tolerance: f64) {
        if day <= self.last_update {
            return;
        }
        let mut remaining = (day - self.last_update) as u32;
        let pressure = self.pressure();
        let mut guard = 0;
        while remaining > 0 {
            let chose = self.reach_for(mind);
            // A guard against a boundary that never arrives; past it the
            // rest of the interval is one chunk, which is what a settled
            // person's life is anyway.
            let step = if guard >= 64 {
                remaining
            } else {
                self.days_to_next_boundary(pressure, tolerance).clamp(1, remaining)
            };
            guard += 1;
            self.habits.used(chose, step);
            self.strain.advance(step, pressure, tolerance);
            remaining -= step;
        }
        self.last_update = day;
    }

    /// How long until the ladder would move, at this pressure.
    fn days_to_next_boundary(&self, pressure: f64, tolerance: f64) -> u32 {
        let excess = pressure - tolerance;
        let rate = if excess > 0.0 {
            excess * crate::coping::STRAIN_PER_DAY
        } else {
            excess.max(-1.0) * crate::coping::RECOVERY_PER_DAY
        };
        if rate.abs() < 1e-12 {
            return u32::MAX;
        }
        let target = if rate > 0.0 {
            match self.strain.state {
                FunctionalState::Regulated => ENTER[0],
                FunctionalState::Strained => ENTER[1],
                FunctionalState::Depleted => ENTER[2],
                FunctionalState::Impaired => return u32::MAX,
            }
        } else {
            match self.strain.state {
                FunctionalState::Regulated => return u32::MAX,
                FunctionalState::Strained => LEAVE[0],
                FunctionalState::Depleted => LEAVE[1],
                FunctionalState::Impaired => LEAVE[2],
            }
        };
        let days = (target - self.strain.debt) / rate;
        if days <= 0.0 {
            1
        } else {
            days.ceil().min(u32::MAX as f64) as u32
        }
    }

    /// What they reach for, given who they are and what they have come to
    /// do. Habits tilt the ranking; they do not replace it.
    pub fn reach_for(&self, mind: &Mind) -> Coping {
        let c = Circumstances {
            severity: self.pressure(),
            company: self.support_expected > 0.2,
            ..Default::default()
        };
        let mut ranked = propensities(mind, &self.perceived_control, &c, self.strain.debt);
        for (k, w) in ranked.iter_mut() {
            *w += self.habits.of(*k) as f64;
        }
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        ranked[0].0
    }

    /// **Keep the record from growing without bound.**
    ///
    /// A life of eighty years accumulates changes that no longer do
    /// anything. Dropping one whose present worth is negligible *and*
    /// which is past the horizon anybody actually measured costs nothing
    /// that could be defended, and it is the difference between a save
    /// that grows with a life and one that grows with what mattered in it.
    /// **Advance through a schedule of things that happen.**
    ///
    /// Constant-pressure invariance is necessary and not sufficient: an
    /// analytic step is only valid while nothing changes, so the
    /// interval has to be **split at every discontinuity** — a stressor
    /// beginning or ending, support arriving, work lost, a deadline, a
    /// crisis. Given the same schedule this satisfies the semigroup
    /// property, `advance(x, a + b) == advance(advance(x, a), b)`, which
    /// is the real requirement.
    pub fn advance_through(&mut self, mind: &Mind, tolerance: f64, schedule: &[Change]) {
        // **On the day counts.** Filtering strictly after the last
        // update silently dropped anything happening on the day the
        // record already stood at, which is exactly the case a caller
        // stepping day by day produces every time.
        let mut pending: Vec<&Change> = schedule
            .iter()
            .filter(|c| c.day >= self.last_update)
            .collect();
        pending.sort_by_key(|c| c.day);
        for c in pending {
            self.advance_to(c.day, mind, tolerance);
            c.what.apply(self);
        }
    }

    /// **Coalesced exactly, in two parts.**
    ///
    /// A persistence residual is a floor, so nothing ever decays to
    /// nothing and a "drop what is negligible" rule drops precisely zero
    /// entries however long the life. But **matching today's total is
    /// not enough** — a sum of decays with different half-lives is not
    /// one decay, so a merged entry that agrees now can disagree
    /// tomorrow.
    ///
    /// What is exact is splitting each contribution into the part that
    /// will never move again and the part still fading, aggregating them
    /// **separately**, and grouping by cause — which is also grouping by
    /// dynamics, since the residual and the half-life come from it. A
    /// sum of identical decays really is one decay of the summed amount.
    /// Both pieces then reproduce every future value and not merely this
    /// one.
    ///
    /// Contributions hidden behind the adaptation clamp survive, because
    /// this works on the raw list and the clamp is applied after summing.
    pub fn compact(&mut self, today: u64) {
        use crate::growth::{Cause, DurableTarget, Episodic, Persistence};
        // Everything past its horizon, grouped by where it landed and
        // what caused it — which is also grouped by its dynamics, since
        // the residual and half-life come from the cause.
        let mut settled: Vec<(DurableTarget, Cause, f32, f32)> = Vec::new();
        let mut keep: Vec<Episodic> = Vec::new();
        for e in self.growth.episodics.drain(..) {
            if e.within_evidence(today) {
                keep.push(e);
                continue;
            }
            let (perm, fading) = e.settled_and_fading(today);
            match settled.iter_mut().find(|(t, c, _, _)| *t == e.target && *c == e.cause) {
                Some(slot) => {
                    slot.2 += perm;
                    slot.3 += fading;
                }
                None => settled.push((e.target, e.cause, perm, fading)),
            }
        }
        for (target, cause, perm, fading) in settled {
            let p = cause.persistence();
            // **The permanent part**, which never moves again, so one
            // entry with a residual of 1 reproduces it for ever.
            if perm.abs() > 1e-7 {
                keep.push(Episodic {
                    target,
                    cause,
                    persistence: Persistence {
                        residual_fraction: 1.0,
                        calibration_horizon_days: 0.0,
                        recovery_half_life_days: p.recovery_half_life_days,
                    },
                    initial: perm,
                    day: today,
                });
            }
            // **The part still fading**, which is exact because
            // everything in the group shares a half-life: a sum of
            // identical decays is one decay of the summed amount, dated
            // now.
            if fading.abs() > 1e-7 {
                keep.push(Episodic {
                    target,
                    cause,
                    persistence: Persistence {
                        residual_fraction: 0.0,
                        calibration_horizon_days: 0.0,
                        recovery_half_life_days: p.recovery_half_life_days,
                    },
                    initial: fading,
                    day: today,
                });
            }
        }
        keep.sort_by(|a, b| {
            a.day
                .cmp(&b.day)
                .then(a.target.cmp(&b.target))
                .then(a.cause.cmp(&b.cause))
                .then(a.persistence.residual_fraction.total_cmp(&b.persistence.residual_fraction))
        });
        self.growth.episodics = keep;
        self.growth
            .roles
            .retain(|r| r.ended.is_none() || r.current(today).abs() > 0.005);
        self.standing.retain(|s| s.severity > 0.01);
    }
}

// ---------------------------------------------------------------------
// promotion and demotion
// ---------------------------------------------------------------------

/// **Something that happens on a particular day** and changes what the
/// next analytic step may assume.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Change {
    pub day: u64,
    pub what: What,
}

/// The discontinuities a coarse advance has to stop at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum What {
    /// A new trouble begins. Its severity is passed through
    /// `Strain::felt_severity`, which is the **single** place relapse
    /// sensitivity is ever applied.
    StressorBegins(Standing),
    /// One ends — the least severe first, so a schedule is unambiguous.
    StressorEnds,
    /// Somebody's belief about who would help changes.
    SupportChanges(f64),
    /// What they believe they can affect changes.
    ControlChanges(ControlAppraisal),
    /// Something catastrophic, which is not chronic strain arriving
    /// early and does not touch the ladder.
    Crisis { kind: Acute, activation: f64, because_of: u64 },
}

impl What {
    fn apply(self, c: &mut Coarse) {
        match self {
            What::StressorBegins(mut s) => {
                s.severity = c.strain.felt_severity(s.severity);
                c.standing.push(s);
            }
            What::StressorEnds => {
                if let Some(i) = c
                    .standing
                    .iter()
                    .enumerate()
                    .min_by(|a, b| a.1.severity.total_cmp(&b.1.severity).then(a.0.cmp(&b.0)))
                    .map(|(i, _)| i)
                {
                    c.standing.remove(i);
                }
            }
            What::SupportChanges(v) => c.support_expected = v,
            What::ControlChanges(v) => c.perceived_control = v,
            What::Crisis { kind, activation, because_of } => {
                let day = c.last_update;
                c.strain.crisis_strikes(kind, activation, day, because_of);
            }
        }
    }
}

/// **Somebody being simulated properly.**
#[derive(Clone, Debug)]
pub struct Detailed {
    pub who: Id<Person>,
    pub seed: u64,
    pub origin: PersonOrigin,
    pub mind: Mind,
    pub growth: Growth,
    pub strain: Strain,
    pub perceived_control: ControlAppraisal,
    pub habits: Habits,
    pub standing: Vec<Standing>,
    pub attempts_outstanding: u16,
    pub support_expected: f64,
    pub day: u64,
}

/// The culture a mind is drawn against. Passed in rather than stored, so
/// a promotion in the same world reproduces the same person.
pub type Culture<'a> = &'a [(Value, i8)];

/// **Draw the person their seed says they are**, then apply what life did
/// to them. Nothing here is invented: the same seed and the same growth
/// record give the same man every time.
pub fn promote(c: &Coarse, culture: Culture<'_>, day: u64) -> Detailed {
    let mut mind = Mind::draw(&mut Rng::new(c.origin.seed), culture);
    // **The saved baseline wins.** The draw supplies everything a record
    // does not carry — values, willpower, empathy — and then the one
    // thing that must never drift is put back exactly as it was.
    if c.baseline.len() == Facet::ALL.len() {
        for (i, &f) in Facet::ALL.iter().enumerate() {
            mind.person.set_baseline(f, c.baseline[i]);
        }
    }
    c.growth.settle_into(&mut mind, day);
    Detailed {
        who: c.who,
        seed: c.origin.seed,
        origin: c.origin,
        mind,
        growth: c.growth.clone(),
        strain: c.strain,
        perceived_control: c.perceived_control,
        habits: c.habits,
        standing: c.standing.clone(),
        attempts_outstanding: c.attempts_outstanding,
        support_expected: c.support_expected,
        day,
    }
}

/// **Put somebody back down to a record.**
///
/// Lossy, and specifically so: episodes, concerns and individual memory
/// traces do not survive. What shaped them does, because a core memory is
/// already a growth entry — so the man who comes back up is the man who
/// went down, minus the recollection of particular afternoons.
pub fn demote(d: &Detailed) -> Coarse {
    Coarse {
        who: d.who,
        origin: d.origin,
        baseline: Facet::ALL.iter().map(|&f| d.mind.person.baseline_of(f)).collect(),
        growth: d.growth.clone(),
        strain: d.strain,
        perceived_control: d.perceived_control,
        habits: d.habits,
        standing: d.standing.clone(),
        attempts_outstanding: d.attempts_outstanding,
        support_expected: d.support_expected,
        last_update: d.day,
    }
}

impl Detailed {
    /// A day, simulated properly.
    pub fn a_day_passes(&mut self, tolerance: f64) {
        let chose = self.reach_for();
        self.habits.used(chose, 1);
        let pressure = self.pressure();
        self.strain.a_day_passes(pressure, tolerance);
        self.day += 1;
        self.growth.settle_into(&mut self.mind, self.day);
    }

    pub fn pressure(&self) -> f64 {
        self.standing.iter().map(|s| s.severity).sum::<f64>().min(1.0)
    }

    pub fn reach_for(&self) -> Coping {
        let c = Circumstances {
            severity: self.pressure(),
            company: self.support_expected > 0.2,
            ..Default::default()
        };
        let mut ranked =
            propensities(&self.mind, &self.perceived_control, &c, self.strain.debt);
        for (k, w) in ranked.iter_mut() {
            *w += self.habits.of(*k) as f64;
        }
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        ranked[0].0
    }

    /// What a facet reads at, for comparing a promoted person against one
    /// who never left.
    pub fn z(&self, f: Facet) -> f32 {
        self.mind.person.z(f)
    }
}
