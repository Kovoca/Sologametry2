//! **What somebody does about it, and what happens when that stops
//! working.**
//!
//! Mind spec section 10: *coping and breakdown, **staged** rather than a
//! tantrum table*. A tantrum table is a roll at a threshold — cross a
//! stress number, draw a card, get a rage or a melancholy. It fails three
//! ways: no coping in it, no order to it, no way back out.
//!
//! # What this state machine is, and is not
//!
//! `FunctionalState` is a **designed general model of functional
//! strain**. It is deliberately *not* labelled with Maslach's burnout
//! model, which the first version of this module leaned on wrongly in
//! two separate ways:
//!
//! - Maslach describes **three dimensions** — exhaustion, cynicism or
//!   psychological distance from work, and reduced professional efficacy
//!   — not a ladder of whole-person stages. Lashing out, withdrawing and
//!   drinking are not burnout dimensions at all.
//! - It is specifically a response to **chronic occupational
//!   conditions**, so it cannot carry bereavement, illness or war.
//!
//! The ordering is not settled firmly enough to enforce as a universal
//! ladder either: some process theories put exhaustion before cynicism
//! and other phase models and longitudinal profiles come out
//! differently. Where occupational burnout is actually wanted, `Burnout`
//! keeps its three axes separately — which is also what lets the dutiful
//! worker exist: extreme exhaustion, little visible cynicism, and
//! performance held up for a while by compensatory effort.
//!
//! Two further namings the earlier version got wrong: **coping is not a
//! stage**, because people go on coping in every state — including by
//! denial, withdrawal and drink — and **"broken" is too global**, since
//! somebody impaired at work may still be a competent parent.
//!
//! # Chronic strain and acute crisis are different mechanisms
//!
//! An ordinary bad afternoon must not produce chronic impairment. One
//! catastrophic afternoon can still produce panic, dissociation, flight,
//! aggression or collapse on the spot. `Acute` is that second route and
//! does not run through the debt at all.

use crate::mind::{Facet, Mind};

// ---------------------------------------------------------------------
// control: what they think, and what is so
// ---------------------------------------------------------------------

/// **What somebody believes they can affect.** Coping selection reads
/// this and nothing else, because appraised control is what people
/// actually act on.
///
/// Control is **not one scalar**. Somebody cannot reverse a terminal
/// diagnosis and can absolutely control their symptoms, their money,
/// their care and what they do with the time left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlAppraisal {
    /// Can I stop the thing itself?
    pub source: f64,
    /// Can I change what it does to me?
    pub consequences: f64,
    /// Can I govern how I react?
    pub own_response: f64,
}

impl Default for ControlAppraisal {
    fn default() -> Self {
        ControlAppraisal { source: 0.5, consequences: 0.5, own_response: 0.5 }
    }
}

impl ControlAppraisal {
    /// The best handle they think they have on the situation itself.
    pub fn instrumental(&self) -> f64 {
        self.source.max(self.consequences)
    }
}

/// **What is actually so.** Resolution reads this, never selection.
///
/// The gap between the two is where both important errors live: "I can
/// fix this" when they cannot, which is futile effort and frustration;
/// and "nothing can be done" when something could have been, which is a
/// missed opportunity and learned helplessness.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActualControl {
    pub source: f64,
    pub consequences: f64,
    /// Whether they can simply leave.
    pub exit: f64,
    /// Whether they have the skill, money, time, standing and access to
    /// act at all.
    pub means: f64,
}

impl Default for ActualControl {
    fn default() -> Self {
        ActualControl { source: 0.5, consequences: 0.5, exit: 0.2, means: 0.5 }
    }
}

// ---------------------------------------------------------------------
// the strategies
// ---------------------------------------------------------------------

/// **What somebody is doing about it.**
///
/// The fourteen the Brief COPE measures. They are **first class**: the
/// individual strategy and how it turned out matter more than the family
/// it is filed under.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Coping {
    Active,
    Planning,
    InstrumentalSupport,
    EmotionalSupport,
    Reframing,
    Acceptance,
    Faith,
    Humour,
    Distraction,
    Denial,
    SubstanceUse,
    Venting,
    Disengagement,
    SelfBlame,
}

/// **A design tag, and they overlap.**
///
/// Carver is explicit that the Brief COPE has no overall score and
/// recommends no one way of deriving a dominant style, and later factor
/// analyses have found anywhere from two to fifteen higher-order
/// factors. So this mapping is **our synthesis of Lazarus and Folkman
/// with the Brief COPE's items**, not something the instrument
/// validates — and no family carries a payoff of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Family {
    Problem,
    Emotion,
    Avoidant,
}

/// **What kind of avoidance**, because they are not one thing. Putting a
/// hopeless goal down is not the same act as drinking to not think about
/// an eviction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AvoidanceKind {
    /// A break, a night's sleep, deliberately putting it aside until
    /// morning. **Frequently adaptive.**
    TemporaryRespite,
    Denial,
    /// Giving up on the goal — which is right when the goal is genuinely
    /// unreachable.
    BehavioralDisengagement,
    /// Refusing to feel the thing, which is what interrupts the process
    /// that would otherwise finish.
    ExperientialAvoidance,
    SubstanceEscape,
}

impl Coping {
    pub const ALL: [Coping; 14] = [
        Coping::Active,
        Coping::Planning,
        Coping::InstrumentalSupport,
        Coping::EmotionalSupport,
        Coping::Reframing,
        Coping::Acceptance,
        Coping::Faith,
        Coping::Humour,
        Coping::Distraction,
        Coping::Denial,
        Coping::SubstanceUse,
        Coping::Venting,
        Coping::Disengagement,
        Coping::SelfBlame,
    ];

    /// Tags, plural. **Faith is the clearest case**: it can supply
    /// meaning, a community, acceptance, or a way of not looking at it,
    /// depending entirely on how it is being used.
    pub fn families(self) -> &'static [Family] {
        match self {
            Coping::Active | Coping::Planning => &[Family::Problem],
            Coping::InstrumentalSupport => &[Family::Problem, Family::Emotion],
            Coping::EmotionalSupport => &[Family::Emotion],
            Coping::Reframing | Coping::Acceptance | Coping::Humour => &[Family::Emotion],
            Coping::Faith => &[Family::Emotion, Family::Avoidant],
            Coping::Distraction => &[Family::Emotion, Family::Avoidant],
            Coping::Denial | Coping::SubstanceUse | Coping::Disengagement => &[Family::Avoidant],
            Coping::Venting => &[Family::Emotion, Family::Avoidant],
            Coping::SelfBlame => &[Family::Avoidant],
        }
    }

    pub fn is(self, f: Family) -> bool {
        self.families().contains(&f)
    }

    pub fn avoidance(self) -> Option<AvoidanceKind> {
        Some(match self {
            Coping::Distraction => AvoidanceKind::TemporaryRespite,
            Coping::Denial => AvoidanceKind::Denial,
            Coping::Disengagement => AvoidanceKind::BehavioralDisengagement,
            Coping::SelfBlame | Coping::Venting => AvoidanceKind::ExperientialAvoidance,
            Coping::SubstanceUse => AvoidanceKind::SubstanceEscape,
            _ => return None,
        })
    }

    /// Whether it requires another person to actually be there.
    pub fn needs_company(self) -> bool {
        matches!(
            self,
            Coping::InstrumentalSupport | Coping::EmotionalSupport | Coping::Venting
        )
    }
}

// ---------------------------------------------------------------------
// choosing, as propensities
// ---------------------------------------------------------------------

/// **What is actually available to somebody right now.**
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circumstances {
    pub severity: f64,
    /// Somebody is there who might be asked.
    pub company: bool,
    pub substance_available: bool,
    /// Somebody is present whom they must not frighten — which is most
    /// of why people hold themselves together.
    pub someone_to_protect: bool,
    /// The other party can do them harm if crossed.
    pub other_has_authority: bool,
}

impl Default for Circumstances {
    fn default() -> Self {
        Circumstances {
            severity: 0.5,
            company: true,
            substance_available: true,
            someone_to_protect: false,
            other_has_authority: false,
        }
    }
}

/// **Propensities, not a verdict.**
///
/// Personality constrains a repertoire; it does not dictate one
/// permanent answer. The same violent man lashes out when confronted,
/// holds himself in front of his daughter, says nothing to a magistrate
/// and drinks afterwards — and none of that is out of character.
///
/// The invariant worth having is **the same person in a genuinely
/// identical state produces the same propensities**, which is what makes
/// a seeded draw from them reproducible. It is not that one person
/// always performs the same act regardless of circumstance.
pub fn propensities(
    mind: &Mind,
    control: &ControlAppraisal,
    c: &Circumstances,
    debt: f64,
) -> Vec<(Coping, f64)> {
    let z = |f: Facet| mind.person.z(f) as f64;
    let instrumental = control.instrumental();
    let mut out: Vec<(Coping, f64)> = Vec::new();

    for k in Coping::ALL {
        if k.needs_company() && !c.company {
            continue;
        }
        if k == Coping::SubstanceUse && !c.substance_available {
            continue;
        }

        // **Appraised control steers the choice**, which is Lazarus's
        // point and is about selection only. Nothing here says a family
        // is harmful; that is settled when the attempt resolves.
        let mut w = if k.is(Family::Problem) {
            0.2 + 0.8 * instrumental
        } else if k.is(Family::Emotion) {
            0.2 + 0.8 * (1.0 - instrumental).max(control.own_response)
        } else {
            0.5
        };

        w += match k {
            Coping::Active => 0.35 * z(Facet::Perseverance) + 0.2 * z(Facet::Assertiveness),
            Coping::Planning => 0.35 * z(Facet::Orderliness) + 0.2 * z(Facet::Curiosity),
            Coping::InstrumentalSupport => {
                0.3 * z(Facet::Gregariousness) - 0.3 * z(Facet::Privacy)
            }
            Coping::EmotionalSupport => 0.3 * z(Facet::Gregariousness) + 0.2 * z(Facet::Trust),
            Coping::Reframing => 0.3 * z(Facet::Cheerfulness) + 0.2 * z(Facet::Tolerance),
            Coping::Acceptance => 0.25 * z(Facet::Tolerance) - 0.2 * z(Facet::Anxiety),
            Coping::Faith => 0.3 * z(Facet::Dutifulness),
            Coping::Humour => 0.35 * z(Facet::Cheerfulness),
            Coping::Distraction => 0.25 * z(Facet::ExcitementSeeking),
            Coping::Denial => 0.3 * z(Facet::Anxiety) - 0.3 * z(Facet::Curiosity),
            Coping::SubstanceUse => 0.3 * z(Facet::ExcitementSeeking) + 0.2 * z(Facet::Gloom),
            Coping::Venting => 0.4 * z(Facet::Anger),
            Coping::Disengagement => 0.3 * z(Facet::Gloom) - 0.3 * z(Facet::Perseverance),
            Coping::SelfBlame => 0.4 * z(Facet::Gloom) + 0.2 * z(Facet::Anxiety),
        };

        // **Circumstance, not character** — and the suppression is
        // proportional as well as absolute, because a flat penalty is
        // nothing to somebody two and a half standard deviations into a
        // disposition. The more there is to hold back, the more of it
        // holding back removes.
        if k == Coping::Venting {
            if c.someone_to_protect {
                w = w * 0.3 - 0.6;
            }
            if c.other_has_authority {
                w = w * 0.35 - 0.5;
            }
        }
        if k == Coping::SubstanceUse && c.someone_to_protect {
            w = w * 0.6 - 0.3;
        }

        // **Willpower holds somebody to the harder option**, and mounting
        // debt pushes them off it.
        if k.is(Family::Avoidant) {
            w += 0.5 * debt - 0.4 * mind.willpower as f64;
        }
        out.push((k, w));
    }
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    out
}

/// The likeliest, for a caller that does not want to sample.
pub fn choose(
    mind: &Mind,
    control: &ControlAppraisal,
    c: &Circumstances,
    debt: f64,
) -> Coping {
    propensities(mind, control, c, debt)[0].0
}

// ---------------------------------------------------------------------
// attempts, and what the world does about them
// ---------------------------------------------------------------------

/// **What somebody actually did**, which is not the same as what it
/// achieved. A strategy raises an attempt; the world settles it.
///
/// Without this seam, coping is a private spell that subtracts stress
/// whether or not anything happened.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attempt {
    pub strategy: Coping,
    /// Whether this one needs another person to respond.
    pub asks_somebody: bool,
    /// Whether it aims at the situation rather than the feeling.
    pub aims_at_the_world: bool,
}

/// **What another person actually did about it**, supplied by the
/// caller from `relations.rs` and `social.rs`.
///
/// Received and perceived support are empirically distinct, and their
/// main and buffering effects differ by type — so this cannot be a
/// generic multiplier. An unwanted lecture is *offered* support that
/// makes things worse.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct SupportGiven {
    /// Practical help that bears on the problem.
    pub practical: f64,
    /// Somebody listening.
    pub emotional: f64,
    /// **How the recipient read it.** Support intended is not support
    /// received.
    pub read_as_helpful: f64,
    /// Help that creates an obligation, which is a real cost.
    pub obligation: f64,
}

/// What an attempt came to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outcome {
    /// Load taken off now.
    pub relief: f64,
    /// **Added to the debt**, and only ever because something concrete
    /// happened: a solvable problem left to worsen, a fear reinforced, a
    /// process interrupted, a cost incurred.
    pub deferred: f64,
    /// Whether the situation itself improved.
    pub the_problem_moved: f64,
    /// Whether they came away believing they could not affect it.
    pub helplessness: f64,
}

/// **Settle an attempt against the world.**
///
/// `worsens_if_ignored` is what makes avoidance costly *sometimes*: an
/// approaching eviction gets worse while somebody drinks, and an
/// unchangeable bereavement does not.
pub fn resolve(
    a: Attempt,
    actual: &ActualControl,
    support: &SupportGiven,
    severity: f64,
    worsens_if_ignored: f64,
) -> Outcome {
    let mut o = Outcome {
        relief: 0.0,
        deferred: 0.0,
        the_problem_moved: 0.0,
        helplessness: 0.0,
    };

    if let Some(kind) = a.strategy.avoidance() {
        // **Immediate relief, or nobody would ever choose it.**
        o.relief = 0.30 * severity;
        // **And the debt is a consequence, not a tax on the family.**
        // Where nothing was going to get better anyway, respite is free.
        let concrete = match kind {
            AvoidanceKind::TemporaryRespite => 0.15,
            AvoidanceKind::BehavioralDisengagement => {
                // Right, when the goal was genuinely out of reach.
                0.2 + 0.8 * actual.source
            }
            AvoidanceKind::Denial => 0.9,
            AvoidanceKind::ExperientialAvoidance => 0.8,
            AvoidanceKind::SubstanceEscape => 1.0,
        };
        o.deferred = 0.30 * severity * concrete * (0.35 + worsens_if_ignored);
        if kind == AvoidanceKind::SubstanceEscape {
            // A cost of its own, before anything else gets worse.
            o.deferred += 0.05 * severity;
        }
        return o;
    }

    if a.asks_somebody {
        let helpful = support.read_as_helpful.clamp(0.0, 1.0);
        o.relief = (0.10 * support.emotional + 0.15 * support.practical) * helpful * severity;
        o.the_problem_moved = support.practical * actual.means * 0.4;
        // **An unwanted lecture is offered support that costs.**
        if helpful < 0.3 && (support.emotional + support.practical) > 0.2 {
            o.deferred += 0.08 * severity;
        }
        o.deferred += 0.05 * support.obligation;
        return o;
    }

    if a.aims_at_the_world {
        // **It costs the effort because it failed, not because its
        // family carries a penalty.** Real control and the means to use
        // it decide, and the actor's own belief has nothing to do with
        // it.
        let got = actual.source.max(actual.consequences) * actual.means;
        o.the_problem_moved = got;
        o.relief = 0.28 * got * severity;
        let wasted = 1.0 - got;
        o.deferred = 0.10 * wasted * severity;
        // Trying and failing is where helplessness is actually learned.
        o.helplessness = wasted * 0.5;
    } else {
        // Aimed at the feeling. It does not move the situation and does
        // not need to.
        o.relief = 0.24 * severity;
    }
    o
}

/// Raise the attempt a strategy implies.
pub fn attempt(strategy: Coping) -> Attempt {
    Attempt {
        strategy,
        asks_somebody: strategy.needs_company() && strategy.avoidance().is_none(),
        aims_at_the_world: matches!(strategy, Coping::Active),
    }
}

// ---------------------------------------------------------------------
// functional state
// ---------------------------------------------------------------------

/// **A designed general model of functional strain.** Not Maslach, and
/// deliberately not named for him.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FunctionalState {
    Regulated,
    Strained,
    Depleted,
    /// Functioning badly. **Not "broken"** — impairment is not global,
    /// and somebody failing at work may still be a competent parent.
    Impaired,
}

impl FunctionalState {
    /// What is left of somebody's capacity in the domain under strain.
    pub fn capacity(self) -> f64 {
        match self {
            FunctionalState::Regulated => 1.0,
            FunctionalState::Strained => 0.85,
            FunctionalState::Depleted => 0.55,
            FunctionalState::Impaired => 0.15,
        }
    }
}

/// **Occupational burnout, kept as its three real dimensions** for where
/// it is actually wanted, rather than flattened into the ladder.
///
/// This is what lets the dutiful worker be represented honestly:
/// exhaustion very high, cynicism low, efficacy still holding — because
/// he is spending more and more effort to keep it there.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Burnout {
    pub exhaustion: f64,
    pub cynicism: f64,
    pub reduced_efficacy: f64,
}

/// **An acute crisis**, which is not chronic strain arriving early.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Acute {
    Panic,
    Dissociation,
    Flight,
    Aggression,
    Freeze,
}

pub const STRAIN_PER_DAY: f64 = 0.010;

/// **Recovery is slower than acquisition.**
pub const RECOVERY_PER_DAY: f64 = 0.004;

/// **How deep the hole goes.** Unbounded, a decade under it takes a
/// century to clear, so somebody who had a very bad ten years could
/// never recover in a lifetime.
pub const STRAIN_CEILING: f64 = 1.2;

pub const ENTER: [f64; 3] = [0.35, 0.65, 0.90];
/// **Lower than where it was entered.** Hysteresis, and it is real.
pub const LEAVE: [f64; 3] = [0.25, 0.50, 0.75];

/// **Where somebody stands, and what it has already cost them.**
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strain {
    /// Saturating and recoverable.
    pub debt: f64,
    pub state: FunctionalState,
    pub days_in_state: u32,
    /// **Capping the debt must not erase the duration.** Ten years at
    /// the ceiling is not ten days at the ceiling, and the difference
    /// shows up elsewhere — illness, lost work, a wrecked marriage,
    /// dependence, a durable well-being injury, coping habits that have
    /// set.
    pub days_severely_impaired: u32,
}

impl Default for Strain {
    fn default() -> Self {
        Strain {
            debt: 0.0,
            state: FunctionalState::Regulated,
            days_in_state: 0,
            days_severely_impaired: 0,
        }
    }
}

impl Strain {
    /// **How readily it comes back.** Prior episodes are among the
    /// strongest predictors of recurrence there is.
    ///
    /// Recorded and exposed rather than fed back into the thresholds
    /// here, deliberately: a feedback that changed within an interval
    /// would break the timestep invariance this module guarantees.
    /// Whoever wants it applies it.
    pub fn relapse_sensitivity(&self) -> f64 {
        (self.days_severely_impaired as f64 / 730.0).min(1.0)
    }

    pub fn a_day_passes(&mut self, pressure: f64, tolerance: f64) {
        self.advance(1, pressure, tolerance);
    }

    /// **Advance an arbitrary number of days at once, with the same
    /// result as taking them one at a time.**
    ///
    /// This is the property slice 9 depends on. "One stage at a time"
    /// has to mean *chronologically*, not one transition per call —
    /// otherwise a person simulated in detail traverses several stages
    /// while a distant one updated once after two years moves only one,
    /// and somebody's mental state comes to depend on whether they
    /// happened to be loaded.
    ///
    /// Legitimate because the load path is monotone across an interval
    /// of constant pressure, so thresholds are met in order and the
    /// crossing times are solvable.
    pub fn advance(&mut self, days: u32, pressure: f64, tolerance: f64) {
        if days == 0 {
            return;
        }
        let n = days as f64;
        let excess = pressure - tolerance;
        let rate = if excess > 0.0 {
            excess * STRAIN_PER_DAY
        } else {
            excess.max(-1.0) * RECOVERY_PER_DAY
        };

        let start = self.debt;
        let end = (start + rate * n).clamp(0.0, STRAIN_CEILING);
        let severe_before = self.state >= FunctionalState::Depleted;

        // Step the ladder until it settles, recording the threshold that
        // produced the last move so the time in the new state is exact.
        let mut last_threshold: Option<f64> = None;
        loop {
            let next = match self.state {
                FunctionalState::Regulated if end >= ENTER[0] => {
                    Some((FunctionalState::Strained, ENTER[0]))
                }
                FunctionalState::Strained if end >= ENTER[1] => {
                    Some((FunctionalState::Depleted, ENTER[1]))
                }
                FunctionalState::Strained if end < LEAVE[0] => {
                    Some((FunctionalState::Regulated, LEAVE[0]))
                }
                FunctionalState::Depleted if end >= ENTER[2] => {
                    Some((FunctionalState::Impaired, ENTER[2]))
                }
                FunctionalState::Depleted if end < LEAVE[1] => {
                    Some((FunctionalState::Strained, LEAVE[1]))
                }
                FunctionalState::Impaired if end < LEAVE[2] => {
                    Some((FunctionalState::Depleted, LEAVE[2]))
                }
                _ => None,
            };
            match next {
                Some((to, thr)) => {
                    self.state = to;
                    last_threshold = Some(thr);
                }
                None => break,
            }
        }

        // Days at or below Depleted, for the duration record. The path is
        // monotone, so one crossing time settles it.
        let severe_after = self.state >= FunctionalState::Depleted;
        let crossing = |thr: f64| -> f64 {
            if rate.abs() < 1e-12 {
                return 0.0;
            }
            ((thr - start) / rate).clamp(0.0, n)
        };
        let severe_days = match (severe_before, severe_after) {
            (true, true) => n,
            (false, true) => n - crossing(ENTER[1]),
            (true, false) => crossing(LEAVE[1]),
            (false, false) => 0.0,
        };
        self.days_severely_impaired = self
            .days_severely_impaired
            .saturating_add(severe_days.round().max(0.0) as u32);

        self.days_in_state = match last_threshold {
            None => self.days_in_state.saturating_add(days),
            Some(thr) => (n - crossing(thr)).round().max(0.0) as u32,
        };
        self.debt = end;
    }

    /// **What somebody might do at the worst of it**, as ranked
    /// propensities out of who they are *and where they are*.
    ///
    /// The replacement for the tantrum table is not a different table. A
    /// violent man does not lash out at a magistrate, and holds himself
    /// together in front of a child — and the ordinary shape of a bad
    /// stretch is withdrawal *and* drinking *and* a row, in sequence,
    /// rather than one of them forever.
    pub fn crisis_propensities(mind: &Mind, c: &Circumstances) -> Vec<(Acute, f64)> {
        let z = |f: Facet| mind.person.z(f) as f64;
        // **Inhibition scales with the drive.** A violent man has more
        // to hold back than a mild one, and holding it back is exactly
        // what he does in front of his own child or a man who can ruin
        // him. Subtracting a constant left him swinging either way.
        let mut aggression = 0.6 * z(Facet::Violence) + 0.4 * z(Facet::Anger);
        if c.someone_to_protect {
            aggression = aggression * 0.25 - 0.8;
        }
        if c.other_has_authority {
            aggression = aggression * 0.30 - 0.7;
        }
        let mut out = vec![
            (Acute::Aggression, aggression),
            (Acute::Panic, 0.6 * z(Facet::Anxiety) + 0.2 * z(Facet::StressVulnerability)),
            (
                Acute::Dissociation,
                0.5 * z(Facet::StressVulnerability) + 0.3 * z(Facet::Privacy),
            ),
            (
                Acute::Flight,
                0.4 * z(Facet::Anxiety) - 0.4 * z(Facet::Assertiveness)
                    + if c.other_has_authority { 0.4 } else { 0.0 },
            ),
            (
                Acute::Freeze,
                0.4 * z(Facet::StressVulnerability) - 0.3 * z(Facet::Assertiveness)
                    + if c.someone_to_protect { 0.3 } else { 0.0 },
            ),
        ];
        out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        out
    }
}
