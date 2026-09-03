//! **How somebody comes to be different from who they were.**
//!
//! Mind spec sections 8 and 20: a person changes, slowly, and *for a
//! reason you can name*.
//!
//! Before this slice, `Personality::adapt` existed and only tests called
//! it, and `Conviction::held` was drawn when the person was made and no
//! argument, defeat, conversion or disillusion could ever move it.
//!
//! # What changed, and what changed *about* them
//!
//! The first version of this module made an error worth recording,
//! because it is the easiest one to make when reading a literature:
//! **it calibrated personality change against life-satisfaction
//! results.** Lucas's findings on unemployment and widowhood — that
//! people largely recover from being widowed and largely do not recover
//! from losing work, even after being re-employed — are about
//! **subjective well-being**. They are not measurements of the Big Five,
//! and a mechanism tag preserves provenance without repairing an outcome
//! mismatch.
//!
//! The recent life-event meta-analysis is explicit about the split: Big
//! Five traits are **core characteristics** and change little and
//! specifically, while life satisfaction and self-esteem are **surface
//! characteristics** and are far more responsive to circumstance
//! *(Bühler et al.)*.
//!
//! | evidence | what was measured | where it goes here |
//! |---|---|---|
//! | Lucas, unemployment | life satisfaction | `WellbeingBaseline` |
//! | Lucas, widowhood | life satisfaction | `WellbeingBaseline` |
//! | Bühler / Bleidorn | personality inventories | `Facet` |
//! | Roberts et al., therapy | chiefly emotional stability | `Facet` |
//!
//! So the two are **different enums with different entry points**, and
//! a life-satisfaction result cannot reach `Personality::adapt` at all —
//! not by convention but because there is no function that takes it
//! there. The same discipline `social.rs` uses to keep a speaker's
//! motives out of a listener's ear.
//!
//! # Doubt comes before change
//!
//! An argument does not move a conviction. It creates doubt; doubt
//! accumulates or fades; only accumulated doubt lets a conviction move.

use crate::mind::{Conviction, Facet, Mind, Value, ADAPTATION_LIMIT};

// ---------------------------------------------------------------------
// what a durable change is a change *to*
// ---------------------------------------------------------------------

/// **The layer a durable change lands in.**
///
/// Two members and not four. `Value` is deliberately absent: convictions
/// move through `Doubts`, and a second route into them would be exactly
/// the "argument changes a mind on the spot" the section exists to
/// forbid. `Concern` depth is owned by `mind.rs`, which already ages and
/// habituates it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DurableTarget {
    /// A trait, in z. A **core characteristic**: slow, small, specific.
    Facet(Facet),
    /// The set-point of life satisfaction, in SD. A **surface
    /// characteristic**: much more responsive to circumstance, and the
    /// layer nearly every published adaptation curve actually measured.
    WellbeingBaseline,
}

/// **How much of a change is still there, and how far the evidence
/// actually goes.**
///
/// A bare retained percentage reads as an asymptote and is nothing of
/// the kind: longitudinal studies are finite, so what they support is
/// "this share of the measured change remained after N years". Storing
/// the horizon alongside keeps 25% and 85% from looking directly
/// comparable when they may come from different outcomes and different
/// follow-up lengths.
///
/// The curve here is therefore a **designed asymptote fitted to a finite
/// observation**, not a claim that the residue is permanent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Persistence {
    /// Share of the original still present at the horizon.
    pub residual_fraction: f32,
    /// **How far out anybody actually looked.** Past this the curve is
    /// extrapolation.
    pub calibration_horizon_days: f32,
    pub recovery_half_life_days: f32,
}

impl Persistence {
    /// What a change of `initial` is worth after `elapsed` days.
    pub fn worth(&self, initial: f32, elapsed: f32) -> f32 {
        let faded = 0.5f32.powf(elapsed / self.recovery_half_life_days);
        initial * (self.residual_fraction + (1.0 - self.residual_fraction) * faded)
    }

    /// Whether a reading at this age is inside what was measured.
    pub fn measured_at(&self, elapsed: f32) -> bool {
        elapsed <= self.calibration_horizon_days
    }
}

// ---------------------------------------------------------------------
// section 8a: what changes a personality
// ---------------------------------------------------------------------

/// **Mechanisms with evidence on personality measures.**
///
/// Small, specific and heterogeneous — the meta-analytic finding is
/// emphatically *not* one generic event effect *(Bühler et al.)*.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShapesPersonality {
    /// **A role that demands something of you daily.** Not an
    /// accumulation of instances but an approach toward a target — see
    /// `Role`.
    RoleDemand,
    /// A core memory's bounded request, from `memory::Trace::plasticity`.
    CoreMemory,
    /// **Sustained deliberate effort**, therapy included. Roberts et al.
    /// found about **0.37 SD, chiefly on emotional stability, over ~24
    /// weeks**, and it persisted — which is evidence that traits move
    /// under sustained intervention, not a transferable amount for every
    /// facet and every event.
    SustainedTreatment,
    /// Trauma. Personality effects are real and **much smaller than the
    /// well-being effects of the same events**.
    Trauma,
}

impl ShapesPersonality {
    pub const ALL: [ShapesPersonality; 4] = [
        ShapesPersonality::RoleDemand,
        ShapesPersonality::CoreMemory,
        ShapesPersonality::SustainedTreatment,
        ShapesPersonality::Trauma,
    ];

    /// What one instance is worth, in z. `RoleDemand` has none: it is not
    /// episodic, so asking is a category error and it answers zero.
    pub fn per_instance(self) -> f32 {
        match self {
            ShapesPersonality::RoleDemand => 0.0,
            // The cap `memory::Trace::plasticity` already applies, kept
            // in one place so the two bounds cannot drift apart.
            ShapesPersonality::CoreMemory => 0.15,
            // One week of it. Twenty-four weeks is the 0.37.
            ShapesPersonality::SustainedTreatment => 0.016,
            ShapesPersonality::Trauma => 0.15,
        }
    }

    pub fn persistence(self) -> Persistence {
        match self {
            ShapesPersonality::RoleDemand => Persistence {
                residual_fraction: 0.60,
                calibration_horizon_days: 365.0 * 4.0,
                recovery_half_life_days: 700.0,
            },
            ShapesPersonality::CoreMemory => Persistence {
                residual_fraction: 0.50,
                calibration_horizon_days: 365.0 * 3.0,
                recovery_half_life_days: 700.0,
            },
            // Roberts: the gains held at follow-up.
            ShapesPersonality::SustainedTreatment => Persistence {
                residual_fraction: 0.80,
                calibration_horizon_days: 365.0,
                recovery_half_life_days: 600.0,
            },
            ShapesPersonality::Trauma => Persistence {
                residual_fraction: 0.55,
                calibration_horizon_days: 365.0 * 5.0,
                recovery_half_life_days: 900.0,
            },
        }
    }
}

/// How long a role takes to do its work: the time constant of the
/// approach, so about 63% of the way there in two years and 95% in six.
pub const ROLE_TIME_CONSTANT_DAYS: f32 = 730.0;

/// **A standing demand, approaching its own target.**
///
/// This is the shape the first version got wrong, and the error was not
/// a small one: applying a fixed daily increment gave about **0.2 z a
/// year**, which drives an ordinary career into the global ±1.5 safety
/// clamp inside a decade. Saturation then becomes the *expected*
/// occupational outcome and the clamp stops being an invariant and
/// starts being the mechanism. A role has a target of its own — around
/// **0.1–0.3 z**, which is what role effects actually measure — and
/// approaches it.
///
/// Held analytically from the dates rather than accumulated, so there is
/// no running total to save, reload or get out of step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Role {
    pub facet: Facet,
    /// Signed, in z.
    pub target: f32,
    pub started: u64,
    pub ended: Option<u64>,
}

impl Role {
    pub fn current(&self, today: u64) -> f32 {
        let until = self.ended.unwrap_or(today).min(today);
        let active = until.saturating_sub(self.started) as f32;
        let reached = self.target * (1.0 - (-active / ROLE_TIME_CONSTANT_DAYS).exp());
        match self.ended {
            None => reached,
            Some(end) if today > end => {
                let since = (today - end) as f32;
                ShapesPersonality::RoleDemand.persistence().worth(reached, since)
            }
            Some(_) => reached,
        }
    }
}

// ---------------------------------------------------------------------
// section 8b: what changes how somebody feels about their life
// ---------------------------------------------------------------------

/// **Mechanisms with evidence on subjective well-being.**
///
/// Every figure here is in SD of life satisfaction and every one of them
/// comes from a study that measured life satisfaction. None of them may
/// touch a facet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShapesWellbeing {
    Bereavement,
    /// **The striking one.** Recovery is incomplete *on average* — with
    /// substantial individual variation, which is why this is a mean and
    /// not a destiny.
    LostWork,
    Impairment,
    /// A tie formed or broken.
    Attachment,
}

impl ShapesWellbeing {
    pub const ALL: [ShapesWellbeing; 4] = [
        ShapesWellbeing::Bereavement,
        ShapesWellbeing::LostWork,
        ShapesWellbeing::Impairment,
        ShapesWellbeing::Attachment,
    ];

    /// The size of the hit on the day, in SD of life satisfaction.
    pub fn per_instance(self) -> f32 {
        match self {
            ShapesWellbeing::Bereavement => 0.80,
            ShapesWellbeing::LostWork => 0.55,
            ShapesWellbeing::Impairment => 0.75,
            ShapesWellbeing::Attachment => 0.25,
        }
    }

    /// **Fitted to what was actually followed.** The German panel work
    /// these come from tracked people for something like fifteen years
    /// either side of the event, so that is the horizon; the residual is
    /// the share of the original still visible at it, and beyond that the
    /// curve is extrapolation rather than measurement.
    pub fn persistence(self) -> Persistence {
        match self {
            // Substantial recovery, not complete.
            ShapesWellbeing::Bereavement => Persistence {
                residual_fraction: 0.15,
                calibration_horizon_days: 365.0 * 15.0,
                recovery_half_life_days: 700.0,
            },
            // Recovery is incomplete on average and is not restored by
            // being re-employed.
            ShapesWellbeing::LostWork => Persistence {
                residual_fraction: 0.45,
                calibration_horizon_days: 365.0 * 15.0,
                recovery_half_life_days: 800.0,
            },
            ShapesWellbeing::Impairment => Persistence {
                residual_fraction: 0.70,
                calibration_horizon_days: 365.0 * 10.0,
                recovery_half_life_days: 900.0,
            },
            ShapesWellbeing::Attachment => Persistence {
                residual_fraction: 0.10,
                calibration_horizon_days: 365.0 * 10.0,
                recovery_half_life_days: 500.0,
            },
        }
    }
}

/// What produced one durable change, kept for provenance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cause {
    Personality(ShapesPersonality),
    Wellbeing(ShapesWellbeing),
}

impl Cause {
    pub fn persistence(self) -> Persistence {
        match self {
            Cause::Personality(m) => m.persistence(),
            Cause::Wellbeing(m) => m.persistence(),
        }
    }
}

/// **One thing that happened, and where it landed.**
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Episodic {
    pub target: DurableTarget,
    /// Where the figure came from. **Provenance only** — the dynamics
    /// are the field below, so a synthesised entry can carry an exact
    /// decay that no single mechanism has.
    pub cause: Cause,
    pub persistence: Persistence,
    /// Signed, in the units of its target.
    pub initial: f32,
    pub day: u64,
}

impl Episodic {
    pub fn new(target: DurableTarget, cause: Cause, initial: f32, day: u64) -> Self {
        Episodic { target, cause, persistence: cause.persistence(), initial, day }
    }

    pub fn worth_now(&self, today: u64) -> f32 {
        let elapsed = today.saturating_sub(self.day) as f32;
        self.persistence.worth(self.initial, elapsed)
    }

    /// **The part that will never move again**, and the part that is
    /// still going. Compaction needs them apart, because a sum of decays
    /// with different half-lives is not one decay.
    pub fn settled_and_fading(&self, today: u64) -> (f32, f32) {
        let elapsed = today.saturating_sub(self.day) as f32;
        let r = self.persistence.residual_fraction;
        let faded = 0.5f32.powf(elapsed / self.persistence.recovery_half_life_days);
        (self.initial * r, self.initial * (1.0 - r) * faded)
    }

    /// Whether this reading is still inside the followed period, or has
    /// run off the end of what anybody measured.
    pub fn within_evidence(&self, today: u64) -> bool {
        self.persistence.measured_at(today.saturating_sub(self.day) as f32)
    }
}

// ---------------------------------------------------------------------
// the record
// ---------------------------------------------------------------------

/// **The record of how somebody came to be who they now are.**
///
/// Kept as a list rather than a number, because the provenance is the
/// point: a total says a man is anxious, and the list says he is anxious
/// because of a roof that came in nine years ago.
///
/// **Nothing here stores a running total.** Every contribution is
/// recomputed from its dates, summed *raw*, and only then projected into
/// the ±1.5 band. That is what makes the result order-independent — A
/// then B is B then A, one 0.8 is two 0.4s, opposing pushes cancel
/// whatever order they arrive in — and what makes a save that lands
/// mid-saturation reload onto the same trajectory. The earlier design
/// recorded how much of the shared ceiling each source happened to
/// receive, which is exactly the bookkeeping that goes wrong when one
/// effect is hidden behind another and the one in front then fades.
#[derive(Clone, Debug, Default)]
pub struct Growth {
    pub episodics: Vec<Episodic>,
    pub roles: Vec<Role>,
}

impl Growth {
    pub fn new() -> Self {
        Growth::default()
    }

    /// **Something happened that moved a trait.** Only mechanisms with
    /// evidence on personality measures can be passed.
    pub fn shaped_personality(
        &mut self,
        facet: Facet,
        by: ShapesPersonality,
        strength: f32,
        toward: f32,
        day: u64,
    ) {
        if by == ShapesPersonality::RoleDemand {
            // A role is a standing demand, not an event. Silently making
            // one an episodic push is how the daily-increment error got
            // in, so it is refused rather than approximated.
            return;
        }
        let sign = if toward < 0.0 { -1.0 } else { 1.0 };
        let size = by.per_instance() * strength.clamp(0.0, 1.0) * sign;
        if size.abs() < 1e-6 {
            return;
        }
        self.episodics.push(Episodic::new(
            DurableTarget::Facet(facet),
            Cause::Personality(by),
            size,
            day,
        ));
    }

    /// **Something happened that changed how somebody feels about their
    /// life.** This is where the life-satisfaction literature lands, and
    /// there is no path from here to a facet.
    pub fn shaped_wellbeing(&mut self, by: ShapesWellbeing, strength: f32, toward: f32, day: u64) {
        let sign = if toward < 0.0 { -1.0 } else { 1.0 };
        let size = by.per_instance() * strength.clamp(0.0, 1.0) * sign;
        if size.abs() < 1e-6 {
            return;
        }
        self.episodics.push(Episodic::new(
            DurableTarget::WellbeingBaseline,
            Cause::Wellbeing(by),
            size,
            day,
        ));
    }

    /// Somebody took up work that demands something of them.
    pub fn took_a_role(&mut self, facet: Facet, target: f32, day: u64) {
        self.roles.push(Role { facet, target, started: day, ended: None });
    }

    /// And left it. The demand stops being made; what it built decays
    /// from there.
    pub fn left_the_role(&mut self, facet: Facet, day: u64) {
        if let Some(r) = self
            .roles
            .iter_mut()
            .find(|r| r.facet == facet && r.ended.is_none())
        {
            r.ended = Some(day);
        }
    }

    /// **The unconstrained sum for a facet.** Every source contributes
    /// its own current value and nothing knows about the ceiling.
    pub fn raw_for(&self, facet: Facet, today: u64) -> f32 {
        let episodic: f32 = self
            .episodics
            .iter()
            .filter(|e| e.target == DurableTarget::Facet(facet))
            .map(|e| e.worth_now(today))
            .sum();
        let roles: f32 = self
            .roles
            .iter()
            .filter(|r| r.facet == facet)
            .map(|r| r.current(today))
            .sum();
        episodic + roles
    }

    /// **What is actually expressed**: the raw sum put through a
    /// saturating map rather than sheared off at the bound.
    ///
    /// **Diminishing plasticity, and it has to happen here rather than
    /// when a change is recorded.** Every durable change leaves a
    /// permanent residue, so without it an ordinary century of ordinary
    /// events presses almost everybody flat against ±1.5 and the safety
    /// clamp becomes the mechanism again.
    ///
    /// Scaling each push by the room left *at the time it happened* was
    /// the obvious fix and is wrong: it makes the result depend on the
    /// order things happened in, which is exactly the property the
    /// aggregation was rebuilt to have. Saturating the **sum** keeps A
    /// then B equal to B then A, keeps one 0.8 equal to two 0.4s, and
    /// still means a trait already far from where it started is hard to
    /// move further and easy to move back.
    ///
    /// It also approaches the bound rather than reaching it, so the
    /// clamp stays a bound that is never actually the answer.
    pub fn expressed_for(&self, facet: Facet, today: u64) -> f32 {
        let raw = self.raw_for(facet, today);
        ADAPTATION_LIMIT * (raw / ADAPTATION_LIMIT).tanh()
    }

    /// **The durable part of how somebody feels about their life**, in SD.
    /// Separate from `Mood`, which is today, and from `Stress`, which is
    /// load.
    pub fn wellbeing(&self, today: u64) -> f32 {
        self.episodics
            .iter()
            .filter(|e| e.target == DurableTarget::WellbeingBaseline)
            .map(|e| e.worth_now(today))
            .sum()
    }

    /// Write both layers into the mind. Each is *set* from the recomputed
    /// total rather than nudged, so this is idempotent: calling it twice
    /// on the same day changes nothing the second time.
    pub fn settle_into(&self, mind: &mut Mind, today: u64) {
        for f in Facet::ALL {
            mind.person.set_durable(f, self.expressed_for(f, today));
        }
        mind.wellbeing_baseline = self.wellbeing(today) as f64;
    }

    /// **What shaped this**, largest first — the reason provenance is
    /// kept at all. Ties break on the cause so the order is stable.
    pub fn what_shaped(&self, target: DurableTarget, today: u64) -> Vec<(Cause, f32)> {
        let mut by: Vec<(Cause, f32)> = Vec::new();
        for e in self.episodics.iter().filter(|e| e.target == target) {
            let v = e.worth_now(today);
            match by.iter_mut().find(|(c, _)| *c == e.cause) {
                Some(slot) => slot.1 += v,
                None => by.push((e.cause, v)),
            }
        }
        if let DurableTarget::Facet(f) = target {
            let roles: f32 = self
                .roles
                .iter()
                .filter(|r| r.facet == f)
                .map(|r| r.current(today))
                .sum();
            if roles.abs() > 1e-4 {
                by.push((Cause::Personality(ShapesPersonality::RoleDemand), roles));
            }
        }
        by.retain(|(_, v)| v.abs() > 1e-4);
        by.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()).then(a.0.cmp(&b.0)));
        by
    }
}

// ---------------------------------------------------------------------
// section 20: arguments and value change
// ---------------------------------------------------------------------

/// **How far a conviction moves when doubt finally cashes out**, in
/// points of the −50..+50 scale.
///
/// **A designed transition size, and not derived from anything.** It was
/// justified here by test–retest correlations of 0.7–0.8 for values, and
/// that is the same denominator error this project already corrected for
/// personality stability: rank-order stability is a *population*
/// statistic saying comparatively high-valued people stay comparatively
/// high. It says nothing about how far one person moves on one occasion,
/// and mean-level change, rank-order stability and individual profile
/// change are three different quantities.
///
/// Four points produces the dynamics the section wants — argument works,
/// slowly, and nobody is converted in an evening. It stands until there
/// is an individual-change calibration to replace it with.
///
/// It is also a *presentation* step: four points near the middle of the
/// scale is not the same latent distance as four points near the end.
pub const CONVICTION_STEP: f32 = 4.0;

/// **What it takes before any of it moves.**
pub const DOUBT_TO_MOVE: f64 = 0.75;

/// **Doubt fades**, which is the whole difference between the person you
/// argue with weekly and the one you argue with once a year.
///
/// **A decay rate and a threshold together imply a ceiling on how far
/// repeated argument can ever push, and it has to be checked rather than
/// assumed** — see `stationary_post_exposure_ceiling`. At three weeks,
/// which is what this was first set to and which reads perfectly
/// plausibly, weekly argument from somebody entirely credible converges
/// on 0.55 against a bar of 0.75, so the model quietly asserted that
/// nobody is ever talked round by anybody they see every week. Nothing
/// about that is visible in either constant; it only shows in the fixed
/// point.
pub const DOUBT_HALF_LIFE_DAYS: f64 = 60.0;

/// **The most doubt a fixed cadence can ever reach, read immediately
/// after an exposure.**
///
/// There are two stationary values and they differ by a factor of the
/// decay, so the phase has to be named or the number is ambiguous:
///
/// ```text
/// just after an argument:  a / (1 − r)
/// just before the next:    a·r / (1 − r)
/// ```
///
/// A calibration diagnostic under fixed assumptions, and **not** a rule
/// that every kind of influence must independently be able to cross the
/// threshold — most influences should not be able to, on their own.
pub fn stationary_post_exposure_ceiling(interval_days: f64, added_each_time: f64) -> f64 {
    let r = 0.5f64.powf(interval_days / DOUBT_HALF_LIFE_DAYS);
    (added_each_time / (1.0 - r)).min(1.0)
}

/// The same fixed point read just *before* the next exposure.
pub fn stationary_pre_exposure_ceiling(interval_days: f64, added_each_time: f64) -> f64 {
    let r = 0.5f64.powf(interval_days / DOUBT_HALF_LIFE_DAYS);
    (added_each_time * r / (1.0 - r)).min(1.0)
}

/// **How an argument arrived**, all of it from the listener's side.
///
/// None of these is the speaker's intent: `credible` and `hostile_intent`
/// are what slice 6's reading produced, which is section 17's rule
/// arriving where it decides whether anybody's mind changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Framing {
    /// How much the listener credits the speaker, 0..1.
    pub credible: f64,
    /// **Whether there was anything new in it.** Repeating the same
    /// sentence weekly is not weekly independent evidence; it mostly
    /// buys familiarity. A friend bringing fresh reasons is a different
    /// thing and this is what separates them.
    pub novelty: f64,
    /// How much this conviction is part of who they think they are.
    pub identity_centrality: f64,
    /// Whether the listener read the speaker as out to get them.
    pub hostile_intent: f64,
    /// Whether the speaker is somebody from the other side.
    pub out_group: f64,
}

impl Default for Framing {
    fn default() -> Self {
        Framing {
            credible: 0.5,
            novelty: 1.0,
            identity_centrality: 0.0,
            hostile_intent: 0.0,
            out_group: 0.0,
        }
    }
}

/// What one argument did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Argued {
    /// They already agreed.
    Preaching,
    /// **Waved away.** The ordinary response to somebody you do not
    /// credit: not entrenchment, just nothing.
    Dismissed,
    /// Doubt went up. **The ordinary outcome when it does land.**
    Doubted,
    /// Doubt was enough, and the conviction moved.
    Moved,
    /// **They dug in.** Genuinely rare, and it needs far more than a
    /// disagreeable source: Wood and Porter found no backfire across 52
    /// issues and more than 10,000 participants, and later work finds
    /// only limited conditional cases. It takes a firmly held conviction
    /// that is *part of somebody's identity*, pushed by an out-group
    /// speaker read as hostile.
    Hardened,
}

/// Doubt is **directional**. One undirected bucket per conviction lets
/// somebody arguing for peace and somebody arguing against it pour into
/// the same reservoir, and whoever happens to speak at the threshold
/// decides which way the person moves — which is nonsense.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DoubtKey {
    pub topic: Value,
    /// −1 or +1: which way the pressure is pushing.
    pub direction: i8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Doubt {
    pub key: DoubtKey,
    /// 0..1.
    pub amount: f64,
    /// The position being urged.
    pub toward: f32,
    pub last_day: u64,
    /// How often this has been heard, for diminishing returns.
    pub times_heard: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Doubts {
    pub open: Vec<Doubt>,
}

impl Doubts {
    pub fn new() -> Self {
        Doubts::default()
    }

    pub fn about(&self, topic: Value, direction: i8) -> f64 {
        self.open
            .iter()
            .find(|d| d.key.topic == topic && d.key.direction == direction)
            .map(|d| d.amount)
            .unwrap_or(0.0)
    }

    /// The most doubt standing on this conviction, whichever way.
    pub fn strongest_about(&self, topic: Value) -> f64 {
        self.open
            .iter()
            .filter(|d| d.key.topic == topic)
            .map(|d| d.amount)
            .fold(0.0, f64::max)
    }

    /// **Somebody argued a position at them.**
    pub fn argued(
        &mut self,
        mind: &mut Mind,
        topic: Value,
        position: i8,
        force: f64,
        how: Framing,
        day: u64,
    ) -> Argued {
        self.fade_to(day);
        let held = mind.conviction(topic) as f32;
        let gap = position as f32 - held;
        if gap.abs() < 6.0 {
            return Argued::Preaching;
        }
        let direction: i8 = if gap < 0.0 { -1 } else { 1 };
        let conviction_strength = (held.abs() / 50.0).clamp(0.0, 1.0) as f64;

        // **Hardening needs identity threat, not merely a source you
        // dislike.** A liar asserting the opposite is waved away.
        let entrenching = conviction_strength > 0.6
            && how.identity_centrality > 0.6
            && how.hostile_intent > 0.5
            && how.out_group > 0.5;
        if entrenching {
            let away = held + -(direction as f32);
            set_conviction(mind, topic, away);
            return Argued::Hardened;
        }

        // Not credited, and not a threat either: nothing happens.
        if how.credible < 0.25 {
            return Argued::Dismissed;
        }

        let purchase = (1.0 - 0.75 * conviction_strength) * force.clamp(0.0, 1.0);
        let d = self.entry(topic, direction, day);
        // **Diminishing returns on hearing it again.** The fifth telling
        // of one argument is not a fifth argument.
        let staleness = 1.0 / (1.0 + d.times_heard as f64 * 0.30);
        let fresh = how.novelty.clamp(0.0, 1.0).max(0.0);
        let independence = fresh + (1.0 - fresh) * staleness;
        let added = purchase * how.credible.clamp(0.0, 1.0) * independence * 0.18;

        d.amount = (d.amount + added).clamp(0.0, 1.0);
        d.toward = position as f32;
        d.last_day = day;
        d.times_heard = d.times_heard.saturating_add(1);

        if d.amount >= DOUBT_TO_MOVE {
            let toward = d.toward;
            d.amount = 0.0;
            d.times_heard = 0;
            let now = mind.conviction(topic) as f32;
            let step = CONVICTION_STEP.min((toward - now).abs());
            set_conviction(mind, topic, now + (toward - now).signum() * step);
            return Argued::Moved;
        }
        Argued::Doubted
    }

    pub fn fade_to(&mut self, day: u64) {
        for d in &mut self.open {
            let elapsed = day.saturating_sub(d.last_day) as f64;
            if elapsed > 0.0 {
                d.amount *= 0.5f64.powf(elapsed / DOUBT_HALF_LIFE_DAYS);
                d.last_day = day;
                // Long enough without hearing it and it is new again.
                if elapsed > DOUBT_HALF_LIFE_DAYS * 4.0 {
                    d.times_heard = 0;
                }
            }
        }
        self.open.retain(|d| d.amount > 0.001);
    }

    fn entry(&mut self, topic: Value, direction: i8, day: u64) -> &mut Doubt {
        let key = DoubtKey { topic, direction };
        if let Some(i) = self.open.iter().position(|d| d.key == key) {
            return &mut self.open[i];
        }
        self.open.push(Doubt { key, amount: 0.0, toward: 0.0, last_day: day, times_heard: 0 });
        self.open.last_mut().unwrap()
    }
}

/// Move a conviction, leaving the cultural reading beside it so
/// `heterodoxy` stays meaningful — **a convert becomes a heretic**, and
/// that is a consequence rather than a flag.
fn set_conviction(mind: &mut Mind, topic: Value, to: f32) {
    let to = to.clamp(-50.0, 50.0) as i8;
    if let Some(c) = mind.values.iter_mut().find(|c| c.topic == topic) {
        c.held = to;
    } else {
        mind.values.push(Conviction { topic, held: to, cultural: 0 });
    }
}
