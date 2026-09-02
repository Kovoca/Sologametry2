//! **What one person expects and feels toward another.**
//!
//! Slice 5 of `docs/mind-spec.md`. A relationship here is a **directed,
//! compressed belief about somebody** — not an objective social fact, not
//! a current emotion, and not a replacement for memory.
//!
//! ```text
//! happening → perception → appraisal → evidence → directed update
//! ```
//!
//! Alice's relationship toward Bob and Bob's toward Alice are two
//! separate records that need not agree about anything.
//!
//! ## Eight dimensions, and none may collapse into another
//!
//! | | is | must not become |
//! |---|---|---|
//! | familiarity | exposure and knowledge | affection |
//! | affection | liking and attachment | trust or duty |
//! | trust | expected reliability | friendship |
//! | respect | esteem, competence, moral regard | fear |
//! | fear | expected threat *from this person* | a current fear |
//! | resentment | a durable attributed grievance | current anger |
//! | gratitude | a durable attributed benefit | affection |
//! | obligation | perceived duty or debt | legal duty |
//!
//! The collapses are what make a model flat. Knowing somebody well is not
//! liking them; liking somebody is not relying on them; and respecting an
//! enemy is an ordinary thing that a single "opinion" number cannot say.
//!
//! ## Dispositions generate episodes; they are not episodes
//!
//! ```text
//! relationship.fear       + a threatening encounter → a fear episode
//! relationship.resentment + a reminder              → an anger episode
//! ```
//!
//! Storing the emotion in the relationship would mean somebody is
//! permanently, continuously afraid of a man they have not seen for a
//! year — which is the same mistake slice 1 corrected with concerns, in a
//! different place.
//!
//! ## Objective ties are facts, and are kept elsewhere
//!
//! Parenthood, marriage, command, employment and debt are true whether or
//! not the people can stand each other. Keeping [`SocialFact`] apart from
//! the dimensions is what permits a hated parent, a trusted subordinate,
//! a beloved spouse who cannot be relied on for money, a respected enemy,
//! and a debt somebody sincerely acknowledges and resents.
//!
//! ## A label is derived, and there is never only one
//!
//! `friend = true` is not stored anywhere. Somebody can be a friend, a
//! rival, a creditor and feared, all at once, and the set changes when
//! the dimensions do.

use crate::id::Id;
use crate::mind::{Emotion, Episode, Facet, Mind};
use crate::person::Person;

/// **An objective tie.** True regardless of how anybody feels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SocialFact {
    ParentOf,
    ChildOf,
    MarriedTo,
    SiblingOf,
    Commands,
    EmployedBy,
    ContractuallyOwes,
    MemberOfWith,
}

/// **What sort of reliability is at issue.**
///
/// A single trust number is enough to begin with and is *not* enough for
/// long, so the seam is here from the start: somebody can be brave beside
/// you in a fight, chronically late, incapable of keeping a secret and
/// perfectly honest with money. General trust is a weighted summary of
/// these; the domain evidence stays available.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrustIn {
    General,
    Secrets,
    Money,
    Danger,
    Craft,
    Promises,
}

impl TrustIn {
    pub const ALL: [TrustIn; 6] = [
        TrustIn::General,
        TrustIn::Secrets,
        TrustIn::Money,
        TrustIn::Danger,
        TrustIn::Craft,
        TrustIn::Promises,
    ];
    fn index(self) -> usize {
        TrustIn::ALL.iter().position(|t| *t == self).unwrap()
    }
}

/// **What sort of esteem.** Respect splits the same way trust does, and
/// for the same reason: competence, moral regard and deference to
/// standing are three different judgements about one person.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RespectFor {
    General,
    Competence,
    Character,
    Standing,
}

impl RespectFor {
    pub const ALL: [RespectFor; 4] = [
        RespectFor::General,
        RespectFor::Competence,
        RespectFor::Character,
        RespectFor::Standing,
    ];
    fn index(self) -> usize {
        RespectFor::ALL.iter().position(|r| *r == self).unwrap()
    }
}

/// **A durable attributed grievance**, which is emphatically not the
/// need-deprivation kind in `needs.rs`.
///
/// Two different words that had to stop being one: a *need* grievance is
/// going without something for a long time; an interpersonal grievance is
/// somebody having done you wrong. They behave differently, they are
/// repaired differently, and only one of them has a defendant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grievance {
    pub severity: f64,
    /// Whether it is still open. An apology or a correction closes it
    /// without erasing it.
    pub unresolved: f64,
    pub day: u64,
}

/// **What one person believes about another.** Directed.
#[derive(Clone, Debug, PartialEq)]
pub struct Relationship {
    pub subject: Id<Person>,
    pub object: Id<Person>,

    /// How well they know them. Rises with any contact at all.
    pub familiarity: f64,
    affection_of: Estimate,
    trust: [Estimate; 6],
    respect: [Estimate; 4],
    fear_of: Estimate,
    gratitude_to: Estimate,
    /// Perceived duty or debt. Not the same as the objective one.
    pub obligation: f64,

    /// **Enough provenance to explain a large change**, without deriving
    /// the whole relationship from every memory on every tick.
    pub last_meaningful: Option<u64>,
    pub best_thing_they_did: f64,
    pub worst_thing_they_did: f64,
    pub grievances: Vec<Grievance>,
}

impl Relationship {
    /// Two strangers.
    pub fn strangers(subject: Id<Person>, object: Id<Person>) -> Self {
        Relationship {
            subject,
            object,
            familiarity: 0.0,
            affection_of: Estimate::default(),
            trust: [Estimate::default(); 6],
            respect: [Estimate::default(); 4],
            fear_of: Estimate::default(),
            gratitude_to: Estimate::default(),
            obligation: 0.0,
            last_meaningful: None,
            best_thing_they_did: 0.0,
            worst_thing_they_did: 0.0,
            grievances: Vec::new(),
        }
    }

    /// **Resentment is the sum of what is still open.** Derived rather
    /// than stored, so correcting a false accusation reduces it by
    /// closing the grievance that caused it — and does not touch
    /// affection or trust, which is exactly how being cleared works.
    pub fn resentment(&self) -> f64 {
        self.grievances
            .iter()
            .map(|g| g.severity * g.unresolved)
            .sum::<f64>()
            .min(1.0)
    }

    pub fn affection(&self) -> f64 {
        self.affection_of.value()
    }
    pub fn fear(&self) -> f64 {
        self.fear_of.value()
    }
    pub fn gratitude(&self) -> f64 {
        self.gratitude_to.value()
    }
    /// **How sure they are of their own opinion**, which is what makes a
    /// long acquaintance hard to overturn.
    pub fn sureness_of_affection(&self) -> f64 {
        self.affection_of.confidence()
    }
    pub fn sureness_of_trust(&self, what: TrustIn) -> f64 {
        self.trust[what.index()].confidence()
    }

    pub fn trust_in(&self, what: TrustIn) -> f64 {
        if what == TrustIn::General {
            // A weighted summary of what is known, falling back to the
            // general figure where nothing specific has been seen.
            let specific: Vec<f64> = TrustIn::ALL
                .iter()
                .filter(|t| **t != TrustIn::General)
                .map(|t| self.trust[t.index()].value())
                .filter(|v| *v != 0.0)
                .collect();
            if specific.is_empty() {
                return self.trust[0].value();
            }
            let mean = specific.iter().sum::<f64>() / specific.len() as f64;
            // **No general impression is not a bad one.** Diluting the
            // summary toward zero would make somebody proven reliable in
            // every particular look only middlingly trustworthy.
            if self.trust[0].value() == 0.0 {
                return mean;
            }
            return 0.4 * self.trust[0].value() + 0.6 * mean;
        }
        self.trust[what.index()].value()
    }

    pub fn respect_for(&self, what: RespectFor) -> f64 {
        if what == RespectFor::General {
            let specific: Vec<f64> = RespectFor::ALL
                .iter()
                .filter(|r| **r != RespectFor::General)
                .map(|r| self.respect[r.index()].value())
                .filter(|v| *v != 0.0)
                .collect();
            if specific.is_empty() {
                return self.respect[0].value();
            }
            let mean = specific.iter().sum::<f64>() / specific.len() as f64;
            if self.respect[0].value() == 0.0 {
                return mean;
            }
            return 0.4 * self.respect[0].value() + 0.6 * mean;
        }
        self.respect[what.index()].value()
    }
}

/// **What one interaction is taken to have shown.**
///
/// Produced from an appraisal by the perceiver, so the same interaction
/// yields different evidence to each participant — which is why one
/// exchange can leave one person grateful and the other embarrassed.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Evidence {
    /// Any contact at all, however trivial.
    pub contact: f64,
    /// They were pleasant, or they were not.
    pub warmth: f64,
    /// They came through, or they did not — and at what.
    pub reliability: f64,
    pub reliability_in: Option<TrustIn>,
    /// They were good at something, or admirable, or important.
    pub esteem: f64,
    pub esteem_for: Option<RespectFor>,
    /// They were dangerous.
    pub menace: f64,
    /// They did something for you at a cost to themselves.
    pub kindness: f64,
    /// They wronged you. **This is the interpersonal grievance** and it
    /// carries its own severity.
    pub wrong: f64,
    /// A debt was incurred or discharged.
    pub owing: f64,
}

impl Default for TrustIn {
    fn default() -> Self {
        TrustIn::General
    }
}

/// How fast each dimension moves. **They are deliberately not equal**:
/// familiarity comes free with contact, trust is slow to build, and a
/// betrayal is fast.
const FAMILIARITY_RATE: f64 = 0.18;
const AFFECTION_RATE: f64 = 0.06;
const TRUST_UP: f64 = 0.05;
const TRUST_DOWN: f64 = 0.45;
const RESPECT_RATE: f64 = 0.08;
const FEAR_RATE: f64 = 0.30;

/// **How much is believed, and how firmly.**
///
/// Magnitude and confidence are different things, and collapsing them
/// loses something real: one polite act and thirty years of unbroken
/// civility may both imply affection of about 0.1, and the second should
/// be far harder for one rude afternoon to overturn.
///
/// So an estimate carries the *weight of evidence behind it*, and a new
/// observation is folded in as a weighted mean. Repetition then buys
/// **confidence in a modest conclusion** rather than a larger one.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Estimate {
    magnitude: f64,
    weight: f64,
}

/// Beyond this, nobody's mind is ever changed again. A ceiling exists so
/// that a long enough history cannot become unfalsifiable.
const MOST_EVIDENCE: f64 = 40.0;

/// **Bad is stronger than good.** Negative information weighs more
/// heavily in forming an impression than positive information of the same
/// size — one of the better-replicated findings in the area *(Baumeister;
/// Rozin & Royzman)* — so an unkindness is more diagnostic than a
/// kindness.
const NEGATIVITY_BIAS: f64 = 2.5;

/// **A severe betrayal is not another data point.**
///
/// A weighted mean alone makes a long history nearly immovable, which is
/// right for civility and wrong for treachery: one clear defection
/// reveals a *disposition*, and what it actually does to somebody is
/// invalidate the history rather than be averaged against it. "I did not
/// know him at all" is the ordinary way of saying that the prior weight
/// has just been discounted.
///
/// Which is why trust is hard to build and easy to destroy — not because
/// the numbers move at different rates, but because one of them throws
/// the evidence away.
const BETRAYAL: f64 = 0.7;
const BETRAYAL_DISCOUNT: f64 = 5.0;

impl Estimate {
    pub fn value(self) -> f64 {
        self.magnitude
    }

    /// How much has been seen, against as much as anybody ever weighs.
    /// Somebody you have watched for years is harder to be wrong about —
    /// and harder to change your mind about.
    ///
    /// **Measured against the ceiling** rather than an arbitrary
    /// fraction of it, so that a betrayal discounting the history is
    /// visible as a loss of confidence and not merely of regard: being
    /// robbed does not leave you as sure of a man as you ever were.
    pub fn confidence(self) -> f64 {
        (self.weight / MOST_EVIDENCE).min(1.0)
    }

    /// Take in one observation. `telling` is how diagnostic this sort of
    /// evidence is at all.
    fn observe(&mut self, evidence: f64, telling: f64) {
        let e = evidence.clamp(-1.0, 1.0);
        if e == 0.0 {
            return;
        }
        let bias = if e < 0.0 { NEGATIVITY_BIAS } else { 1.0 };
        if e < -BETRAYAL {
            self.weight /= BETRAYAL_DISCOUNT;
        }
        let k = telling * bias * e.abs();
        self.magnitude = (self.magnitude * self.weight + e * k) / (self.weight + k);
        self.weight = (self.weight + k).min(MOST_EVIDENCE);
    }
}

/// **A trivial act cannot take you past what a trivial act is worth.**
///
/// Approaching a target rather than accumulating is *most* of saturation
/// and not all of it: aiming every piece of evidence at ±1 and varying
/// only the rate is still accumulation, just slower. Two thousand
/// courtesies at a tenth of a point each came out at **1.00 of
/// devotion** — a man adored for passing the salt often enough.
///
/// So what an interaction pulls you toward is *its own magnitude*. A
/// nodding acquaintance of thirty years is a nodding acquaintance;
/// getting past that takes evidence of a different order, which is what
/// real closeness runs on — strength and disclosure, not merely
/// frequency.
fn toward(current: f64, evidence: f64, rate: f64) -> f64 {
    let target = evidence.clamp(-1.0, 1.0);
    // Never drag somebody *back* toward a weak reading: a small kindness
    // from a close friend does not cool the friendship to its own size.
    if (target - current) * target.signum() < 0.0 {
        return current;
    }
    (current + (target - current) * rate).clamp(-1.0, 1.0)
}

impl Relationship {
    /// **Take in what one interaction showed.**
    ///
    /// Every dimension moves on its own evidence and none of them moves
    /// on another's. Familiarity rises with any contact; affection only
    /// with warmth; trust only with reliability, and only in the domain
    /// it was shown.
    pub fn saw(&mut self, e: &Evidence, day: u64) {
        if e.contact > 0.0 {
            self.familiarity = toward(self.familiarity, 1.0, FAMILIARITY_RATE * e.contact);
        }
        if e.warmth != 0.0 {
            self.affection_of.observe(e.warmth, 1.0);
        }
        if e.reliability != 0.0 {
            // **Reliability is the most diagnostic thing anybody shows
            // you**, which is why trust is what a betrayal wrecks.
            let which = e.reliability_in.unwrap_or(TrustIn::General);
            self.trust[which.index()].observe(e.reliability, 1.6);
        }
        if e.esteem != 0.0 {
            let which = e.esteem_for.unwrap_or(RespectFor::General);
            self.respect[which.index()].observe(e.esteem, 1.0);
        }
        if e.menace != 0.0 {
            // Menace is read fast and forgotten slowly: one frightening
            // encounter tells you a great deal.
            self.fear_of.observe(e.menace.clamp(0.0, 1.0), 3.0);
        }
        if e.kindness != 0.0 {
            self.gratitude_to.observe(e.kindness, 2.0);
        }
        if e.owing != 0.0 {
            self.obligation = (self.obligation + e.owing).clamp(-1.0, 1.0);
        }
        if e.wrong > 0.0 {
            self.grievances.push(Grievance { severity: e.wrong.min(1.0), unresolved: 1.0, day });
        }

        let weight = e.warmth.abs().max(e.wrong).max(e.menace).max(e.reliability.abs());
        if weight > 0.25 {
            self.last_meaningful = Some(day);
            self.best_thing_they_did = self.best_thing_they_did.max(e.warmth.max(e.kindness));
            self.worst_thing_they_did = self.worst_thing_they_did.max(e.wrong.max(e.menace));
        }
    }

    /// **An apology repairs only if it is believed.**
    ///
    /// Sincerity, taking responsibility, and costing the apologiser
    /// something. A cheap apology closes nothing, which is why saying
    /// sorry does not automatically work.
    pub fn apologised(&mut self, sincerity: f64, responsibility: f64, cost: f64) {
        let credited = (sincerity * responsibility).sqrt() * (0.5 + 0.5 * cost);
        for g in self.grievances.iter_mut() {
            g.unresolved = (g.unresolved * (1.0 - credited)).max(0.0);
        }
    }

    /// **The blame was wrong.** Closes the grievance and touches nothing
    /// else: being cleared of something is not the same as being liked
    /// again, and affection and trust have their own histories.
    pub fn cleared(&mut self) {
        for g in self.grievances.iter_mut() {
            g.unresolved = 0.0;
        }
    }

    /// **The disposition produces a feeling; it is not one.**
    ///
    /// Meeting a man you are afraid of is what makes you afraid *now*.
    /// Between meetings you are not continuously terrified of somebody you
    /// have not seen.
    pub fn on_meeting(&self, mind: &Mind, threatening: bool) -> Vec<Episode> {
        let mut out = Vec::new();
        let jumpy = (mind.person.z(Facet::Anxiety) as f64 / 5.0 + 0.5).clamp(0.0, 1.0);
        if self.fear() > 0.1 && threatening {
            let strength = self.fear() * (0.5 + 0.5 * jumpy);
            out.push(Episode {
                what: Emotion::Fear,
                strength: strength.min(1.0),
                activation: Emotion::Fear.activation(),
                age_days: 0.0,
                about: None,
            });
        }
        let sore = self.resentment();
        if sore > 0.1 {
            out.push(Episode {
                what: Emotion::Anger,
                strength: sore.min(1.0),
                activation: Emotion::Anger.activation(),
                age_days: 0.0,
                about: None,
            });
        }
        if self.gratitude() > 0.2 {
            out.push(Episode {
                what: Emotion::Gratitude,
                strength: self.gratitude().min(1.0),
                activation: Emotion::Gratitude.activation(),
                age_days: 0.0,
                about: None,
            });
        }
        if self.affection() > 0.3 {
            out.push(Episode {
                what: Emotion::Affection,
                strength: self.affection().min(1.0),
                activation: Emotion::Affection.activation(),
                age_days: 0.0,
                about: None,
            });
        }
        out
    }
}

/// **What a relationship can be called.** Derived, plural, and never
/// stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Label {
    Stranger,
    Acquaintance,
    Friend,
    CloseFriend,
    Rival,
    Enemy,
    Feared,
    Respected,
    Grudge,
    Beholden,
    Creditor,
    Kin,
    Spouse,
    Commander,
    Employer,
}

/// **A relationship has as many names as it has aspects.**
///
/// Never one exclusive answer: somebody can be a friend, a rival, a
/// creditor and feared at the same time, and the set changes when the
/// dimensions do. `friend = true` is not stored anywhere — but "friend"
/// is still what the friendship need looks for.
pub fn labels(r: &Relationship, facts: &[SocialFact]) -> Vec<Label> {
    let mut out = Vec::new();
    if r.familiarity < 0.15 {
        out.push(Label::Stranger);
    } else if r.affection() < 0.3 {
        out.push(Label::Acquaintance);
    }
    if r.familiarity > 0.3 && r.affection() > 0.3 && r.trust_in(TrustIn::General) > 0.2 {
        out.push(Label::Friend);
        if r.affection() > 0.7 && r.trust_in(TrustIn::General) > 0.6 {
            out.push(Label::CloseFriend);
        }
    }
    if r.resentment() > 0.25 {
        out.push(Label::Grudge);
    }
    if r.resentment() > 0.5 && r.affection() < 0.0 {
        out.push(Label::Enemy);
    }
    // **A rival is not an enemy**: contested, familiar, and not hated.
    if r.familiarity > 0.3 && r.respect_for(RespectFor::Competence) > 0.3 && r.affection() < 0.2 {
        out.push(Label::Rival);
    }
    if r.fear() > 0.35 {
        out.push(Label::Feared);
    }
    if r.respect_for(RespectFor::General) > 0.4 {
        out.push(Label::Respected);
    }
    if r.obligation > 0.3 {
        out.push(Label::Beholden);
    }
    if r.obligation < -0.3 {
        out.push(Label::Creditor);
    }
    for f in facts {
        match f {
            SocialFact::ParentOf | SocialFact::ChildOf | SocialFact::SiblingOf => {
                out.push(Label::Kin)
            }
            SocialFact::MarriedTo => out.push(Label::Spouse),
            SocialFact::Commands => out.push(Label::Commander),
            SocialFact::EmployedBy => out.push(Label::Employer),
            _ => {}
        }
    }
    out.sort();
    out.dedup();
    out
}

/// **How much company with this person is worth**, for the friendship
/// need.
///
/// Not co-presence: an hour with somebody you are close to is worth many
/// with somebody you merely know, and an hour with somebody you resent is
/// worth nothing at all. This is what `needs.rs` means by
/// `SomebodyKnown` — asked of the *relationship*, not of the room.
pub fn worth_of_company(r: &Relationship, quality: f64) -> f64 {
    let closeness = (0.3 * r.familiarity + 0.7 * r.affection().max(0.0)).clamp(0.0, 1.0);
    (closeness * quality - 0.5 * r.resentment()).max(0.0)
}
