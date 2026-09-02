//! **How somebody comes to be different from who they were.**
//!
//! Mind spec sections 8 and 20, and they are two halves of one claim: a
//! person changes, slowly, and *for a reason you can name*.
//!
//! Everything before this slice could give somebody a bad year. Nothing
//! could give them a changed life. `Personality::adapt` existed and only
//! tests ever called it; `Conviction::held` was drawn once when the
//! person was made and no argument, defeat, conversion or disillusion
//! could ever move it. A model in which nobody's mind is ever changed by
//! anything is a poor one to hang a social simulation on.
//!
//! Two rules carry the whole module, and both are the *titles* of their
//! spec sections rather than decoration:
//!
//! - **Personality change is rare, cumulative and mechanism-tagged.**
//!   Not "an event moved a trait" — *which* mechanism moved it, because
//!   that is what decides whether it lasts.
//! - **Doubt comes before change.** An argument does not move a
//!   conviction. It creates doubt, doubt accumulates or fades, and only
//!   accumulated doubt lets a conviction move.

use crate::mind::{Conviction, Facet, Mind, Value, ADAPTATION_LIMIT, FACETS};

// ---------------------------------------------------------------------
// section 8: personality change
// ---------------------------------------------------------------------

/// **Why a facet moved.**
///
/// Recording only that somebody became more anxious throws away the
/// thing that decides what happens next, because **adaptation to life
/// events is real, partial, and wildly mechanism-specific**. That is the
/// finding worth having here and it is not intuitive: people mostly
/// recover from being widowed and mostly do *not* recover from losing
/// their job, and the sizes of the two blows are not what separates them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mechanism {
    /// **A role that demands something of you daily** — a first job,
    /// command, a child. Individually tiny and the steadiest producer of
    /// durable change there is, because it is repeated for years.
    RoleDemand,
    /// Losing somebody.
    Bereavement,
    /// Something that happened once and was terrible.
    Trauma,
    /// **Losing work**, which is the striking one: unusually permanent.
    LostWork,
    /// Somebody deliberately trying to change, treatment included.
    Volitional,
    /// Lasting illness or injury.
    Impairment,
    /// A tie formed or broken.
    Attachment,
}

impl Mechanism {
    pub const ALL: [Mechanism; 7] = [
        Mechanism::RoleDemand,
        Mechanism::Bereavement,
        Mechanism::Trauma,
        Mechanism::LostWork,
        Mechanism::Volitional,
        Mechanism::Impairment,
        Mechanism::Attachment,
    ];

    /// **What one instance is worth, in z.**
    ///
    /// Life events move personality by **0.1–0.3 SD** *(Bleidorn et al.,
    /// meta-analysis of life-event studies)*, and clinical intervention
    /// reaches about **0.37 SD on neuroticism over ~24 weeks** *(Roberts
    /// et al., 207 studies)* — but that is a course of treatment, not a
    /// session, which is why `Volitional` is small per instance and
    /// expects to be repeated.
    pub fn per_instance(self) -> f32 {
        match self {
            // **One day of it**, and it is meant to be repeated for
            // years. The role hypothesis is worth 0.1-0.3 z over a
            // couple of years, so a working day is a five-hundredth of
            // that — at a hundredth, a fortnight in a job would remake
            // somebody.
            Mechanism::RoleDemand => 0.0006,
            Mechanism::Bereavement => 0.30,
            Mechanism::Trauma => 0.35,
            Mechanism::LostWork => 0.20,
            // One week of trying. Twenty-four of them is the 0.37.
            Mechanism::Volitional => 0.016,
            Mechanism::Impairment => 0.30,
            Mechanism::Attachment => 0.10,
        }
    }

    /// **What is left once the person has adapted**, as a share of the
    /// original push.
    ///
    /// Hedonic adaptation is partial and depends entirely on what
    /// happened:
    ///
    /// - **Widowhood**: substantial return toward baseline over about two
    ///   years, though not complete *(Lucas et al.)*.
    /// - **Unemployment**: little or no return, and **not restored by
    ///   getting another job** *(Lucas et al.)* — the strongest evidence
    ///   there is against a single set-point everybody drifts back to.
    /// - **Disability**: very little adaptation *(Lucas)*.
    /// - **Marriage**: a brief bump and back to baseline.
    ///
    /// A role's demands do not fade because the role is still there
    /// making them every day; that is not adaptation, it is repetition.
    pub fn persistence(self) -> f32 {
        match self {
            Mechanism::RoleDemand => 0.90,
            Mechanism::Bereavement => 0.25,
            Mechanism::Trauma => 0.55,
            Mechanism::LostWork => 0.85,
            Mechanism::Volitional => 0.80,
            Mechanism::Impairment => 0.90,
            Mechanism::Attachment => 0.35,
        }
    }

    /// How long the fading takes, in days. Adaptation to bereavement runs
    /// over roughly two years, so a half-life near a year.
    pub fn half_life_days(self) -> f32 {
        match self {
            Mechanism::Bereavement => 300.0,
            Mechanism::Attachment => 240.0,
            Mechanism::Trauma => 900.0,
            _ => 600.0,
        }
    }
}

/// **One durable push, and where it came from.**
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Change {
    pub facet: Facet,
    pub mechanism: Mechanism,
    /// Signed, in z, as it was on the day.
    pub initial: f32,
    pub day: u64,
}

impl Change {
    /// What it is still worth today. Decays from the original toward the
    /// share this mechanism leaves behind — never to nothing, and never
    /// below it.
    pub fn worth_now(&self, today: u64) -> f32 {
        let elapsed = today.saturating_sub(self.day) as f32;
        let keeps = self.mechanism.persistence();
        let faded = 0.5f32.powf(elapsed / self.mechanism.half_life_days());
        self.initial * (keeps + (1.0 - keeps) * faded)
    }
}

/// **The record of how somebody came to be who they now are.**
///
/// Kept as a list rather than a number, because the mechanism tag is the
/// point: a total says a man is anxious, and the list says he is anxious
/// because of a roof that came in nine years ago and has not stopped
/// mattering.
#[derive(Clone, Debug, Default)]
pub struct Growth {
    pub changes: Vec<Change>,
    /// What has already been pushed into the personality, so each day can
    /// send the **difference**. Without it, decay would be invisible: the
    /// personality only takes deltas and would keep whatever it was
    /// handed first.
    applied: [f32; FACETS],
}

fn index_of(f: Facet) -> usize {
    Facet::ALL.iter().position(|x| *x == f).unwrap()
}

impl Growth {
    pub fn new() -> Self {
        Growth::default()
    }

    /// **Something happened that is worth a person changing over.**
    ///
    /// `strength` is 0..1 — how bad, how demanding, how hard they are
    /// trying. `toward` is the direction the facet is pushed.
    ///
    /// This is deliberately not called by an ordinary bad day. Rare means
    /// rare: a fortnight of poor sleep is `Stress`, not a life.
    pub fn happened(
        &mut self,
        facet: Facet,
        mechanism: Mechanism,
        strength: f32,
        toward: f32,
        day: u64,
    ) {
        let size = mechanism.per_instance() * strength.clamp(0.0, 1.0) * toward.signum();
        if size.abs() < 1e-6 {
            return;
        }
        self.changes.push(Change { facet, mechanism, initial: size, day });
    }

    /// What all of it is worth today, for one facet.
    pub fn standing(&self, facet: Facet, today: u64) -> f32 {
        self.changes
            .iter()
            .filter(|c| c.facet == facet)
            .map(|c| c.worth_now(today))
            .sum()
    }

    /// **Push the difference into the personality.**
    ///
    /// Through `Personality::adapt` and nothing else, so the ±1.5 ceiling
    /// still holds and this cannot become a second write path. Sending
    /// the difference rather than the total is what lets a fading change
    /// actually fade — and a negative delta is not a contradiction of the
    /// clamp, it is the whole reason the clamp is a state bound and not a
    /// lifetime budget.
    pub fn settle_into(&mut self, mind: &mut Mind, today: u64) {
        for f in Facet::ALL {
            // **Clamped to the same ceiling the personality enforces.**
            // Without this, a role practised for a working lifetime sums
            // to far more than ±1.5, `applied` records the sum, and the
            // first day it fades the difference is subtracted from a
            // value that never got there — walking somebody's whole
            // adaptation off the far side.
            let want = self.standing(f, today).clamp(-ADAPTATION_LIMIT, ADAPTATION_LIMIT);
            let i = index_of(f);
            let delta = want - self.applied[i];
            if delta.abs() > 1e-6 {
                mind.person.adapt(f, delta);
                self.applied[i] = want;
            }
        }
    }

    /// **What made this person**, largest first — the reason the tag is
    /// kept at all. Ties break on the mechanism so the order is stable.
    pub fn what_shaped(&self, facet: Facet, today: u64) -> Vec<(Mechanism, f32)> {
        let mut by: Vec<(Mechanism, f32)> = Mechanism::ALL
            .iter()
            .map(|&m| {
                let sum: f32 = self
                    .changes
                    .iter()
                    .filter(|c| c.facet == facet && c.mechanism == m)
                    .map(|c| c.worth_now(today))
                    .sum();
                (m, sum)
            })
            .filter(|(_, v)| v.abs() > 1e-4)
            .collect();
        by.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()).then(a.0.cmp(&b.0)));
        by
    }
}

// ---------------------------------------------------------------------
// section 20: arguments and value change
// ---------------------------------------------------------------------

/// **How much a conviction moves when doubt finally cashes out**, in
/// points of the −50..+50 scale.
///
/// Attitudes and values are among the most stable things measured about
/// a person — political and moral values report test–retest correlations
/// around 0.7–0.8 over years — so this is small on purpose. A conviction
/// that could be argued ten points in an evening would be an opinion.
pub const CONVICTION_STEP: f32 = 4.0;

/// **What it takes before any of it moves.** Doubt is on 0..1 and this is
/// most of the way there, so a conviction survives a great deal of
/// disagreement before it gives at all.
pub const DOUBT_TO_MOVE: f64 = 0.75;

/// **Doubt fades**, which is the whole difference between the person you
/// argue with weekly and the one you argue with once a year.
///
/// **A decay rate and a threshold together imply a ceiling on how far
/// anybody can be persuaded, and it has to be checked rather than
/// assumed.** Repeated argument at interval `i` converges on
///
/// ```text
/// most doubt ever reached = added / (1 − ½^(i / half-life))
/// ```
///
/// and if that sits below `DOUBT_TO_MOVE` no amount of repetition moves
/// anybody at all. At three weeks — which is what this was first set to,
/// and it reads perfectly plausibly — weekly argument from somebody
/// entirely credible converges on **0.55 against a bar of 0.75**, so the
/// model quietly asserted that nobody is ever talked round by anybody
/// they see every week. Nothing about that is visible in the constant;
/// it only shows up in the fixed point.
///
/// At two months the same weekly argument clears it and a yearly one is
/// still gone by a factor of seventy before the next conversation, which
/// is the distinction the module is actually for.
pub const DOUBT_HALF_LIFE_DAYS: f64 = 60.0;

/// **Somebody's uncertainty about something they believe.**
///
/// The whole of section 20 is that this object exists. Without it an
/// argument either changes a mind or does not, and both are wrong:
/// arguments overwhelmingly fail to change anybody on the spot and
/// nevertheless people's views do move over years.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Doubt {
    pub topic: Value,
    /// 0..1.
    pub amount: f64,
    /// Where the pressure has been pushing, on the conviction scale.
    pub toward: f32,
    pub last_day: u64,
}

/// What one argument did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Argued {
    /// Nothing: they already agreed.
    Preaching,
    /// Doubt went up. **The ordinary outcome.**
    Doubted,
    /// Doubt was enough, and the conviction moved.
    Moved,
    /// They dug in. **Rare, and it has to be**: the backfire effect is
    /// far weaker and rarer than its fame suggests — large replications
    /// find people update *toward* evidence in almost every condition
    /// tested *(Wood & Porter)*. It survives here only where a strongly
    /// held conviction is pushed by somebody actively distrusted.
    Hardened,
}

/// **Doubt somebody carries**, and the arguments that put it there.
#[derive(Clone, Debug, Default)]
pub struct Doubts {
    pub open: Vec<Doubt>,
}

impl Doubts {
    pub fn new() -> Self {
        Doubts::default()
    }

    pub fn about(&self, topic: Value) -> f64 {
        self.open.iter().find(|d| d.topic == topic).map(|d| d.amount).unwrap_or(0.0)
    }

    /// **Somebody argued a position at them.**
    ///
    /// `force` is what the *listener* made of it — slice 6's reading,
    /// scaled by how credible they found the speaker — and never what the
    /// speaker meant. Section 17's rule, arriving where it decides
    /// whether anybody's mind changes: a brilliant argument from somebody
    /// taken for a liar moves nothing.
    ///
    /// Returns what happened, which is usually nothing much.
    pub fn argued(
        &mut self,
        mind: &mut Mind,
        topic: Value,
        position: i8,
        force: f64,
        credible: f64,
        day: u64,
    ) -> Argued {
        self.fade_to(day);
        let held = mind.conviction(topic) as f32;
        let gap = position as f32 - held;
        // **Preaching to the converted does nothing**, and the model has
        // to say so or every conversation is persuasion.
        if gap.abs() < 6.0 {
            return Argued::Preaching;
        }

        // **A conviction held hard resists**, which is identity-protective
        // cognition and the reason argument works on the undecided.
        let conviction_strength = (held.abs() / 50.0).clamp(0.0, 1.0) as f64;
        let purchase = (1.0 - 0.75 * conviction_strength) * force.clamp(0.0, 1.0);

        // The rare hardening: a firm believer pushed by somebody they
        // take for untrustworthy.
        if credible < 0.2 && conviction_strength > 0.6 {
            let d = self.entry(topic, day);
            d.amount = (d.amount - 0.1 * purchase).max(0.0);
            let held_now = mind.conviction(topic);
            let away = held_now as f32 + gap.signum() * -1.0;
            set_conviction(mind, topic, away);
            return Argued::Hardened;
        }

        let added = purchase * credible.clamp(0.0, 1.0) * 0.18;
        let d = self.entry(topic, day);
        d.amount = (d.amount + added).clamp(0.0, 1.0);
        d.toward = position as f32;
        d.last_day = day;

        if d.amount >= DOUBT_TO_MOVE {
            let toward = d.toward;
            d.amount = 0.0;
            let now = mind.conviction(topic) as f32;
            let step = CONVICTION_STEP.min((toward - now).abs());
            set_conviction(mind, topic, now + (toward - now).signum() * step);
            return Argued::Moved;
        }
        Argued::Doubted
    }

    /// **The most doubt this cadence can ever raise.**
    ///
    /// Exposed because it is the thing a threshold has to be checked
    /// against: see `DOUBT_HALF_LIFE_DAYS`. A caller sizing a new kind of
    /// influence can ask whether it is capable of persuading anybody
    /// before wondering why nobody is being persuaded.
    pub fn ceiling_at(interval_days: f64, added_each_time: f64) -> f64 {
        let kept = 0.5f64.powf(interval_days / DOUBT_HALF_LIFE_DAYS);
        (added_each_time / (1.0 - kept)).min(1.0)
    }

    /// **Doubt decays.** Nothing else in this module makes the difference
    /// between somebody you argue with weekly and somebody you argue with
    /// once.
    pub fn fade_to(&mut self, day: u64) {
        for d in &mut self.open {
            let elapsed = day.saturating_sub(d.last_day) as f64;
            if elapsed > 0.0 {
                d.amount *= 0.5f64.powf(elapsed / DOUBT_HALF_LIFE_DAYS);
                d.last_day = day;
            }
        }
        self.open.retain(|d| d.amount > 0.001);
    }

    fn entry(&mut self, topic: Value, day: u64) -> &mut Doubt {
        if let Some(i) = self.open.iter().position(|d| d.topic == topic) {
            return &mut self.open[i];
        }
        self.open.push(Doubt { topic, amount: 0.0, toward: 0.0, last_day: day });
        self.open.last_mut().unwrap()
    }
}

/// Move a conviction, keeping the cultural reading beside it so
/// `heterodoxy` stays meaningful — **a convert becomes a heretic**, and
/// that is a consequence rather than a separate flag.
fn set_conviction(mind: &mut Mind, topic: Value, to: f32) {
    let to = to.clamp(-50.0, 50.0) as i8;
    if let Some(c) = mind.values.iter_mut().find(|c| c.topic == topic) {
        c.held = to;
    } else {
        mind.values.push(Conviction { topic, held: to, cultural: 0 });
    }
}
