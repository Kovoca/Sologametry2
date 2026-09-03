//! **What is done here.**
//!
//! A *value* is a fact about a person — `mind::Conviction`, drawn
//! against their culture and free to depart from it. A **norm** is a
//! fact about a *place*: what is done, what is not done, and what
//! everybody present will think of you for it.
//!
//! Keeping them apart is the whole point of the module, because the
//! interesting cases all live in the gap:
//!
//! - somebody who **breaches a norm they personally hold** — weakness,
//!   and they know it;
//! - somebody who **keeps one they do not hold** — prudence, or
//!   cowardice, depending who is asked;
//! - somebody who breaches one **they have never heard of**, which is
//!   what being a foreigner is.
//!
//! And the sharp end: **a witness cannot see that you did not know.**
//! The same perception boundary as everywhere else — what crosses is the
//! act, not the intent behind it. A stranger who fails to return a
//! greeting is rude here and merely elsewhere at home, and the person
//! who took offence has no way to tell which.

/// **A thing that is done, or not done, in a particular place.**
///
/// Deliberately mundane. Grand moral questions are `mind::Value`; these
/// are the rules nobody writes down and everybody notices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Norm {
    /// Whether you speak to somebody you do not know.
    GreetStrangers,
    /// Whether a courtesy is answered.
    ThankForCourtesy,
    /// How much room you leave.
    KeepDistance,
    /// Whether you say the thing or go round it.
    Directness,
    /// Whether refusing food or drink is an insult.
    AcceptHospitality,
    /// Whether being late is an offence or a fact of life.
    Punctuality,
    /// Whether age or standing is marked in how you address somebody.
    DeferToElders,
    /// Whether haggling is expected or insulting.
    Haggle,
}

impl Norm {
    pub const ALL: [Norm; 8] = [
        Norm::GreetStrangers,
        Norm::ThankForCourtesy,
        Norm::KeepDistance,
        Norm::Directness,
        Norm::AcceptHospitality,
        Norm::Punctuality,
        Norm::DeferToElders,
        Norm::Haggle,
    ];

    fn index(self) -> usize {
        Norm::ALL.iter().position(|n| *n == self).unwrap()
    }
}

/// **How things are done in one place.**
///
/// Each figure is *how strongly the norm is held*, −1 to +1: positive
/// means it is expected, negative means the opposite is expected, and
/// near zero means nobody minds either way. **Zero is a real answer** —
/// most places have no opinion about most things, and a model where
/// every norm is live everywhere would make travel unbearable rather
/// than interesting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Custom {
    held: [f64; 8],
}

impl Default for Custom {
    fn default() -> Self {
        Custom { held: [0.0; 8] }
    }
}

impl Custom {
    pub fn new(from: &[(Norm, f64)]) -> Self {
        let mut c = Custom::default();
        for &(n, v) in from {
            c.held[n.index()] = v.clamp(-1.0, 1.0);
        }
        c
    }

    pub fn holds(&self, n: Norm) -> f64 {
        self.held[n.index()]
    }

    /// **How badly this lands here.**
    ///
    /// `did` is what somebody did, −1 to +1 on the same scale as the
    /// norm: a positive act where a positive norm is held is
    /// conformity, and the same act where the opposite is expected is a
    /// breach. Which is the whole of it — **the act is the same and the
    /// verdict is not**.
    pub fn reads_as_breach(&self, n: Norm, did: f64) -> f64 {
        let expected = self.holds(n);
        if expected.abs() < 0.15 {
            // Nobody minds here, so there is nothing to breach.
            return 0.0;
        }
        // **Only a shortfall counts.** Distance from what was expected
        // is the wrong measure: doing *more* of the thing the place
        // expects is not a breach of it. Being especially courteous
        // where courtesy is the rule is not rudeness, and the first
        // version of this said it was.
        let did = did.clamp(-1.0, 1.0);
        let shortfall = if expected > 0.0 { expected - did } else { did - expected };
        (shortfall.max(0.0) / 2.0 * expected.abs()).clamp(0.0, 1.0)
    }

    /// **Where two places disagree**, which is what makes a foreigner a
    /// foreigner rather than merely a stranger.
    pub fn differs_from(&self, other: &Custom) -> Vec<(Norm, f64)> {
        let mut out: Vec<(Norm, f64)> = Norm::ALL
            .iter()
            .map(|&n| (n, self.holds(n) - other.holds(n)))
            .filter(|(_, d)| d.abs() > 0.4)
            .collect();
        out.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()).then(a.0.cmp(&b.0)));
        out
    }
}

/// **What somebody who saw it makes of you.**
///
/// A breach is *evidence*, read by whoever was there — and this is the
/// part that matters: **it carries no note saying the man did not know.**
/// `relations.rs` already refuses to let time count as evidence and
/// discounts what was done under duress; none of that helps here,
/// because from outside a foreigner's innocent breach and a local's
/// deliberate rudeness look identical.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Judged {
    /// How much it counts against them, 0..1.
    pub against_them: f64,
    /// Whether it was bad enough to be worth remarking on.
    pub worth_mentioning: bool,
}

/// Judge an act by the norms of the place it happened in — never by the
/// norms of whoever did it.
pub fn judged_here(here: &Custom, n: Norm, did: f64) -> Judged {
    let breach = here.reads_as_breach(n, did);
    Judged {
        against_them: breach,
        worth_mentioning: breach > 0.35,
    }
}

/// **Whether they knew better**, which is a question about *them* and
/// changes nothing about how it was read.
///
/// Kept as a separate function on purpose. It is what the person
/// themselves feels — shame at having got it wrong, or nothing at all —
/// and it is not available to anybody who watched.
pub fn did_they_know(theirs: &Custom, n: Norm) -> bool {
    theirs.holds(n).abs() >= 0.15
}

// ---------------------------------------------------------------------
// what somebody actually does
// ---------------------------------------------------------------------

/// **What this person actually does about a norm**, on the same −1..+1
/// scale the norm is held on.
///
/// A norm says what is expected; it does not say what anybody does. What
/// closes that gap is disposition **modulated by state**, and the state
/// half is the interesting one: *a normally polite man in a foul mood is
/// not polite*, and he has not become a different person.
///
/// **Keeping a norm is an act of self-control**, so it runs on the same
/// depletable capacity as holding back a blow — `coping::regulatory_
/// capacity`, which strain and exhaustion spend. That is not a
/// convenience: it is the claim that manners fail for the same reason
/// tempers do, which is why somebody a year into a bad stretch is
/// short with people who have done nothing to them.
///
/// - `holds_it` is what *they* believe, from their own upbringing.
/// - `mood` is today, from `Mind::mood`.
/// - `capacity` is what they have left to spend.
pub fn would_keep(holds_it: f64, dutifulness: f64, mood: f64, capacity: f64) -> f64 {
    let disposition = (holds_it.clamp(-1.0, 1.0) + 0.25 * dutifulness.clamp(-2.0, 2.0)).clamp(-1.0, 1.0);
    if disposition <= 0.0 {
        // They do not hold it. A good mood does not invent a custom.
        return disposition;
    }
    // **A bad day costs, and a spent man costs more.** Neither reverses
    // anybody: what erodes is the *margin* they were keeping it by.
    let held_together = (0.45 + 0.55 * capacity.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let today = (1.0 + 0.35 * mood.clamp(-1.0, 1.0)).clamp(0.4, 1.35);
    (disposition * held_together * today).clamp(-1.0, 1.0)
}

/// **Whether somebody will bend a rule.**
///
/// Ethics as a disposition rather than a switch: what varies between
/// people is how much a rule weighs against what breaking it is worth.
///
/// - `regard_for_law` is their `Value::Law` conviction, −50..+50.
/// - `gain` is what is in it for them, 0..1.
/// - `chance_seen` is what they think the odds of being caught are.
///
/// **Certainty matters more than severity**, which is one of the more
/// robust findings in criminology: raising the odds of being caught
/// deters, and raising the punishment mostly does not. So the penalty is
/// not a term here at all, and that is deliberate.
///
/// A designed model, and labelled as one: the shape is defensible and
/// the coefficients are not measured.
pub fn will_bend(regard_for_law: i8, dutifulness: f64, gain: f64, chance_seen: f64) -> f64 {
    let scruple = (regard_for_law as f64 / 50.0).clamp(-1.0, 1.0)
        + 0.3 * dutifulness.clamp(-2.0, 2.0) / 2.0;
    // **The gain gates it, and does not merely add to it.** Subtracting
    // scruple gave an unscrupulous man a standing appetite for breaking
    // rules with nothing whatever in it — which is not wickedness, it is
    // arithmetic. Nobody bends a rule for nothing.
    let appetite = ((1.0 - scruple) / 2.0).clamp(0.0, 1.0);
    let worth_it = gain.clamp(0.0, 1.0);
    // Being seen is what deters, and it deters the scrupulous least
    // because they were not going to anyway.
    let deterred = 1.0 - 0.7 * chance_seen.clamp(0.0, 1.0);
    (worth_it * appetite * deterred).clamp(0.0, 1.0)
}

/// **A bad day and a bad character look the same from outside.**
///
/// The perception boundary once more: a witness reads the act, and the
/// act is all there is. Somebody who was curt because he had just been
/// told something terrible is judged exactly as somebody who is simply
/// curt — which is true to life and follows from what was already built
/// rather than being added here.
pub fn judged_without_excuse(here: &Custom, n: Norm, did: f64) -> Judged {
    judged_here(here, n, did)
}

/// A few places, so a test and a demo have somewhere to stand. Real
/// variation is larger than this and in the same directions.
pub mod places {
    use super::{Custom, Norm};

    /// Formal, indirect, and you greet people.
    pub fn old_country() -> Custom {
        Custom::new(&[
            (Norm::GreetStrangers, 0.8),
            (Norm::ThankForCourtesy, 0.9),
            (Norm::Directness, -0.6),
            (Norm::DeferToElders, 0.8),
            (Norm::AcceptHospitality, 0.7),
            (Norm::Haggle, -0.5),
            (Norm::Punctuality, 0.3),
            (Norm::KeepDistance, -0.3),
        ])
    }

    /// Blunt, informal, and nobody speaks to strangers.
    pub fn the_city() -> Custom {
        Custom::new(&[
            (Norm::GreetStrangers, -0.7),
            (Norm::ThankForCourtesy, 0.6),
            (Norm::Directness, 0.8),
            (Norm::DeferToElders, -0.2),
            (Norm::AcceptHospitality, 0.0),
            (Norm::Haggle, -0.7),
            (Norm::Punctuality, 0.8),
            (Norm::KeepDistance, 0.7),
        ])
    }

    /// A market town where the price is a conversation.
    pub fn the_market() -> Custom {
        Custom::new(&[
            (Norm::GreetStrangers, 0.6),
            (Norm::ThankForCourtesy, 0.5),
            (Norm::Directness, 0.2),
            (Norm::Haggle, 0.9),
            (Norm::Punctuality, -0.4),
            (Norm::AcceptHospitality, 0.5),
            (Norm::KeepDistance, -0.5),
            (Norm::DeferToElders, 0.3),
        ])
    }
}
