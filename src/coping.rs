//! **What somebody does about it, and what happens when that stops
//! working.**
//!
//! Mind spec section 10, whose title is the whole argument: *coping and
//! breakdown, **staged** rather than a tantrum table*.
//!
//! A tantrum table is a roll at a threshold. Cross a stress number, draw
//! a card, get a tantrum or a berserk rage or a melancholy. It is the
//! single most-imitated thing about Dwarf Fortress's minds and it is the
//! part worth replacing, for three reasons:
//!
//! - **It has no coping in it.** People overwhelmingly do something about
//!   their circumstances long before they collapse, and what they do is
//!   the interesting part.
//! - **It has no stages.** Nobody goes from fine to berserk. Real
//!   deterioration is ordered — alarm, resistance, exhaustion *(Selye)*;
//!   emotional exhaustion, then cynicism, then a collapse in what you
//!   believe you can do *(Maslach)*. Each stage is visible before the
//!   next arrives.
//! - **It has no way back.** A table can break somebody and cannot mend
//!   them, when in fact most people recover from most things — slowly,
//!   and more slowly the further they went.
//!
//! What replaces the die is not another die. **The form a breakdown
//! takes is the person**: a violent man lashes out, a private one stops
//! speaking, a dutiful one works until he falls over. Two people at
//! identical strain break differently and each of them breaks the same
//! way twice.

use crate::mind::{Facet, Mind};

// ---------------------------------------------------------------------
// coping
// ---------------------------------------------------------------------

/// **What somebody is doing about it.**
///
/// The families are Lazarus and Folkman's, and the individual strategies
/// are the ones the Brief COPE inventory actually measures rather than a
/// list invented for a game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Coping {
    /// Doing something about the situation itself.
    Active,
    /// Working out what to do, which is not the same as doing it.
    Planning,
    /// Asking for help — practical help.
    SeekingHelp,
    /// Being comforted by somebody. Different from being *helped*.
    SeekingComfort,
    /// Finding another way to see it.
    Reframing,
    /// Taking it as it is. **Not** giving up: accepting what cannot be
    /// changed is approach coping and it works.
    Acceptance,
    /// Ritual, observance, prayer.
    Faith,
    Humour,
    /// Doing something else and not thinking about it.
    Distraction,
    /// Refusing that it is happening.
    Denial,
    Drink,
    /// Letting it out at somebody.
    Venting,
    /// Giving up on the goal.
    Disengagement,
    /// Deciding it is your own fault.
    SelfBlame,
}

/// Which family a strategy belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Focus {
    /// Aimed at the situation. Effective **only where there is control**.
    Problem,
    /// Aimed at the feeling. Effective where there is not.
    Emotion,
    /// Aimed at not being there. Works now, costs later.
    Avoidant,
}

impl Coping {
    pub const ALL: [Coping; 14] = [
        Coping::Active,
        Coping::Planning,
        Coping::SeekingHelp,
        Coping::SeekingComfort,
        Coping::Reframing,
        Coping::Acceptance,
        Coping::Faith,
        Coping::Humour,
        Coping::Distraction,
        Coping::Denial,
        Coping::Drink,
        Coping::Venting,
        Coping::Disengagement,
        Coping::SelfBlame,
    ];

    pub fn focus(self) -> Focus {
        match self {
            Coping::Active | Coping::Planning | Coping::SeekingHelp => Focus::Problem,
            Coping::SeekingComfort | Coping::Reframing | Coping::Acceptance | Coping::Faith
            | Coping::Humour => Focus::Emotion,
            Coping::Distraction | Coping::Denial | Coping::Drink | Coping::Venting
            | Coping::Disengagement | Coping::SelfBlame => Focus::Avoidant,
        }
    }

    /// **Approach or avoidance**, which is the division that predicts how
    /// somebody ends up. Avoidant coping predicts worse outcomes about as
    /// consistently as anything in the literature does.
    pub fn is_approach(self) -> bool {
        self.focus() != Focus::Avoidant
    }

    /// Whether it needs somebody else to be there.
    pub fn needs_company(self) -> bool {
        matches!(self, Coping::SeekingHelp | Coping::SeekingComfort | Coping::Venting)
    }
}

/// **What somebody is coping with.**
///
/// `control` is the whole of why this struct exists: the best-replicated
/// result in the coping literature is that **controllability decides
/// which family helps**, and that using the wrong one is itself harmful.
/// Planning your way out of a bereavement does not work and costs the
/// effort of trying.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Situation {
    /// How bad, 0..1.
    pub severity: f64,
    /// **How much of it is actually theirs to change**, 0..1.
    pub control: f64,
    /// Whether anybody is around to be asked.
    pub company: bool,
    /// Whether drink is to be had.
    pub drink_available: bool,
}

impl Default for Situation {
    fn default() -> Self {
        Situation { severity: 0.5, control: 0.5, company: true, drink_available: true }
    }
}

/// What a day of coping did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coped {
    pub chose: Coping,
    /// Load taken off today.
    pub relief: f64,
    /// **What it added to the debt.** Positive for avoidance, and this is
    /// the entire reason avoidance is a trap rather than a mistake.
    pub deferred: f64,
}

/// **How well a strategy fits the situation.**
///
/// Approach coping is *matched* when its family suits the
/// controllability, and a matched strategy is what actually reduces
/// strain. A mismatch relieves almost nothing and still costs the trying.
pub fn fit(c: Coping, s: &Situation) -> f64 {
    match c.focus() {
        Focus::Problem => s.control,
        Focus::Emotion => 1.0 - s.control,
        // Avoidance does not care what the situation is, which is part of
        // its appeal.
        Focus::Avoidant => 0.6,
    }
}

/// **What somebody reaches for.**
///
/// Not a roll on a table: a preference order out of the person and the
/// situation. The same man in the same circumstances reaches for the same
/// thing, and two different men do not.
pub fn choose(mind: &Mind, s: &Situation, strain: f64) -> Coping {
    let z = |f: Facet| mind.person.z(f) as f64;
    let mut best = (Coping::Acceptance, f64::MIN);
    for c in Coping::ALL {
        if c.needs_company() && !s.company {
            continue;
        }
        if c == Coping::Drink && !s.drink_available {
            continue;
        }
        let mut w = fit(c, s);
        w += match c {
            Coping::Active => 0.35 * z(Facet::Perseverance) + 0.2 * z(Facet::Assertiveness),
            Coping::Planning => 0.35 * z(Facet::Orderliness) + 0.2 * z(Facet::Curiosity),
            Coping::SeekingHelp => 0.3 * z(Facet::Gregariousness) - 0.3 * z(Facet::Privacy),
            Coping::SeekingComfort => 0.3 * z(Facet::Gregariousness) + 0.2 * z(Facet::Trust),
            Coping::Reframing => 0.3 * z(Facet::Cheerfulness) + 0.2 * z(Facet::Tolerance),
            Coping::Acceptance => 0.25 * z(Facet::Tolerance) - 0.2 * z(Facet::Anxiety),
            Coping::Faith => 0.3 * z(Facet::Dutifulness),
            Coping::Humour => 0.35 * z(Facet::Cheerfulness),
            Coping::Distraction => 0.25 * z(Facet::ExcitementSeeking),
            Coping::Denial => 0.3 * z(Facet::Anxiety) - 0.3 * z(Facet::Curiosity),
            Coping::Drink => 0.3 * z(Facet::ExcitementSeeking) + 0.2 * z(Facet::Gloom),
            Coping::Venting => 0.4 * z(Facet::Anger),
            Coping::Disengagement => 0.3 * z(Facet::Gloom) - 0.3 * z(Facet::Perseverance),
            Coping::SelfBlame => 0.4 * z(Facet::Gloom) + 0.2 * z(Facet::Anxiety),
        };
        // **Willpower is what holds somebody to the harder option.**
        if !c.is_approach() {
            w += 0.5 * strain - 0.4 * mind.willpower as f64;
        }
        if w > best.1 {
            best = (c, w);
        }
    }
    best.0
}

/// **Do it, and find out what it was worth.**
///
/// The shape that matters: **avoidance gives the most relief today.**
/// A model in which it simply does not work cannot explain why anybody
/// avoids anything, and people avoid constantly. It works — and it puts
/// the whole of what it relieved onto the debt, with interest.
pub fn cope(mind: &Mind, s: &Situation, strain: f64) -> Coped {
    let chose = choose(mind, s, strain);
    let matched = fit(chose, s);
    match chose.focus() {
        Focus::Avoidant => {
            let relief = 0.30 * s.severity;
            Coped { chose, relief, deferred: relief * 1.35 }
        }
        _ => {
            // **Support buffers most where it is needed most.** The
            // stress-buffering hypothesis is specifically that support
            // matters at high stress and does little at low, which is why
            // this scales with severity rather than being a flat bonus.
            let buffer = if chose.needs_company() { 0.10 * s.severity } else { 0.0 };
            let relief = (0.25 * matched + buffer) * s.severity;
            // The effort of trying is spent whether or not it fitted, so
            // a mismatch is a cost and not merely a nil return.
            let wasted = 0.06 * (1.0 - matched) * s.severity;
            Coped { chose, relief, deferred: wasted }
        }
    }
}

// ---------------------------------------------------------------------
// staged breakdown
// ---------------------------------------------------------------------

/// **How far somebody has gone**, in order and without skipping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// Managing. Most people, most of the time.
    Coping,
    /// **Resistance**: holding it together and paying for it. Short
    /// temper, poor sleep, less patience.
    Strained,
    /// **Exhaustion.** Maslach's sequence begins here — the tiredness
    /// comes first and the cynicism follows it.
    Exhausted,
    /// Not functioning. Months, not days.
    Broken,
}

impl Stage {
    /// What is left of somebody's capacity to work and attend.
    pub fn capacity(self) -> f64 {
        match self {
            Stage::Coping => 1.0,
            Stage::Strained => 0.85,
            Stage::Exhausted => 0.55,
            Stage::Broken => 0.15,
        }
    }
}

/// **What a breakdown looks like in this particular person.**
///
/// The replacement for the tantrum table, and the point is that it is
/// *not drawn*. It is read off who they are, so it is the same every
/// time for the same person and different between people.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Breaks {
    /// Lashes out at whoever is nearest.
    Violently,
    /// Shouting, weeping, a scene.
    Loudly,
    /// Stops speaking to anybody and will not be reached.
    Withdrawn,
    /// Keeps working, perfectly, until they fall over. The one that gets
    /// missed, because from the outside nothing is wrong.
    StillWorking,
    /// Drinks.
    Drinking,
}

/// **Accumulated cost of being under it**, and the stage that follows.
///
/// Not an instantaneous threshold on today's stress: a bad afternoon is
/// not a breakdown and a year of grinding pressure is, even if no single
/// day of it looked dramatic. That is allostatic load, and it is why the
/// accumulator exists at all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strain {
    /// 0..1-ish, and it may exceed 1.
    pub load: f64,
    pub stage: Stage,
    /// Days at the current stage, because how long somebody has been
    /// there matters to what comes next.
    pub days_here: u32,
}

impl Default for Strain {
    fn default() -> Self {
        Strain { load: 0.0, stage: Stage::Coping, days_here: 0 }
    }
}

/// How fast the debt builds under sustained overload. Set so that
/// moderate, unrelieved pressure reaches strain in a few months and
/// exhaustion inside a year, which is the pace burnout actually runs at.
pub const STRAIN_PER_DAY: f64 = 0.010;

/// **Recovery is slower than acquisition**, by a lot. Severe burnout
/// takes one to three years to come back from, and sick leave for
/// exhaustion is measured in months. This is the single most important
/// asymmetry in the module.
pub const RECOVERY_PER_DAY: f64 = 0.004;

/// **How deep the hole goes, and it has a bottom.**
///
/// Left unbounded, ten years under it builds a debt that takes a century
/// to clear — so somebody who had a very bad decade could never recover
/// in a lifetime, which is false. Being broken is a *state*, and more
/// pressure once somebody is in it does not keep deepening indefinitely.
///
/// With the recovery rate, the worst case clears in something like two
/// years of real relief, which is what severe burnout actually takes.
pub const STRAIN_CEILING: f64 = 1.2;

/// Where each stage begins.
pub const ENTER: [f64; 3] = [0.35, 0.65, 0.90];
/// **And where it ends, which is lower.** Hysteresis, and it is real:
/// burnout does not lift the week the workload does.
pub const LEAVE: [f64; 3] = [0.25, 0.50, 0.75];

impl Strain {
    /// A day passes under some net pressure.
    ///
    /// `pressure` is today's load after coping, `tolerance` what this
    /// person carries without cost.
    pub fn a_day_passes(&mut self, pressure: f64, tolerance: f64) {
        let excess = pressure - tolerance;
        if excess > 0.0 {
            self.load = (self.load + excess * STRAIN_PER_DAY).min(STRAIN_CEILING);
        } else {
            self.load = (self.load + excess.max(-1.0) * RECOVERY_PER_DAY).max(0.0);
        }

        let was = self.stage;
        // **One step at a time, in both directions.** Nobody goes from
        // managing to broken, and nobody goes from broken to fine.
        self.stage = match self.stage {
            Stage::Coping if self.load >= ENTER[0] => Stage::Strained,
            Stage::Strained if self.load >= ENTER[1] => Stage::Exhausted,
            Stage::Strained if self.load < LEAVE[0] => Stage::Coping,
            Stage::Exhausted if self.load >= ENTER[2] => Stage::Broken,
            Stage::Exhausted if self.load < LEAVE[1] => Stage::Strained,
            Stage::Broken if self.load < LEAVE[2] => Stage::Exhausted,
            s => s,
        };
        if self.stage == was {
            self.days_here = self.days_here.saturating_add(1);
        } else {
            self.days_here = 0;
        }
    }

    /// **How this one breaks.** Read off the person, never drawn.
    pub fn breaks_as(mind: &Mind) -> Breaks {
        let z = |f: Facet| mind.person.z(f) as f64;
        let mut best = (Breaks::Withdrawn, f64::MIN);
        for (b, w) in [
            (Breaks::Violently, 0.6 * z(Facet::Violence) + 0.4 * z(Facet::Anger)),
            (Breaks::Loudly, 0.5 * z(Facet::Anger) + 0.3 * z(Facet::Gregariousness)),
            (Breaks::Withdrawn, 0.5 * z(Facet::Privacy) + 0.3 * z(Facet::Gloom)),
            (
                Breaks::StillWorking,
                0.6 * z(Facet::Dutifulness) + 0.4 * z(Facet::Perseverance),
            ),
            (
                Breaks::Drinking,
                0.5 * z(Facet::ExcitementSeeking) + 0.3 * z(Facet::Gloom)
                    - 0.3 * z(Facet::Dutifulness),
            ),
        ] {
            if w > best.1 {
                best = (b, w);
            }
        }
        best.0
    }
}
