//! **What passed between two people.**
//!
//! Slice 6 of `docs/mind-spec.md`. Three separate realities, and the
//! whole module exists to keep them apart:
//!
//! ```text
//! what the speaker meant       SpeakerPlan   — private, never leaves
//! what the speaker did         SocialAct     — observable
//! what the listener made of it ListenerReading — in mind.rs, theirs
//! ```
//!
//! **The danger here is telepathy disguised as convenience.** If only
//! observable content and delivery cues cross from the plan into the act,
//! then praise, sarcasm, deception, failed jokes, rejected apologies and
//! honest misunderstanding all come out of the same pipeline without any
//! of them being written as a special case. If anything else crosses,
//! none of them can happen at all.
//!
//! ## This module cannot form an opinion
//!
//! It has no access to [`crate::mind::ListenerReading`] and no way to
//! produce an appraisal. That is deliberate and structural: a module that
//! could manufacture the listener's reaction would need the speaker's
//! intent to do it, and telepathy would come back in through module
//! ownership rather than through a field.
//!
//! What it emits is an act. `witness.rs` decides who got what of it, and
//! `mind.rs` decides what each of them made of it.
//!
//! ## Mixed motives, because praise is rarely one thing
//!
//! A commander may sincerely admire a man *and* praise him publicly to
//! move the unit. One intent enum loses that, so motives are weighted:
//!
//! ```text
//! affection      0.55
//! encouragement  0.70
//! seeking favour 0.25
//! status         0.15
//! belittling     0.00
//! ```

use crate::id::Id;
use crate::memory::EventKind;
use crate::mind::{Facet, Mind};
use crate::person::Person;
use crate::rng::Rng;
use crate::witness::Context;

/// How the words are getting there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Channel {
    FaceToFace,
    /// Across a yard, over machinery: the words arrive and little else.
    Shouted,
    Written,
    /// Somebody carried it.
    Relayed,
}

/// **A chance to speak.** People do not converse because compatible
/// records exist; they converse because they are in the same room for a
/// reason and there is time.
#[derive(Clone, Debug, PartialEq)]
pub struct Opportunity {
    pub with: Vec<Id<Person>>,
    /// Everybody else who can hear. **Public and private are different
    /// acts**, and each of these forms their own view.
    pub audience: Vec<Id<Person>>,
    pub channel: Channel,
    pub context: Context,
    pub minutes: f64,
}

/// **Why somebody is speaking.** Private. Never crosses into the act.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Motive {
    Affection,
    Encouragement,
    SeekingFavour,
    /// Being seen to say it, which is a different thing from saying it.
    StatusPerformance,
    Belittling,
    Curiosity,
    Duty,
    Deception,
    Consolation,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeightedMotive {
    pub motive: Motive,
    pub weight: f64,
}

/// **How the speaker means to go about it.**
///
/// Deliberately a different vocabulary from what a listener may conclude:
/// `Ingratiate` is a plan and `Flattery` is a verdict, and using one word
/// for both would let the listener read the plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Strategy {
    Ingratiate,
    Encourage,
    Tease,
    Intimidate,
    Disclose,
    Console,
    Persuade,
    Apologise,
    StateAFact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Topic {
    TheirWork,
    /// Somebody who is not here. Gossip.
    AThirdParty(u32),
    Themselves,
    Weather,
    /// Something that happened.
    AnEvent(EventKind),
}

/// **Something asserted, and what the speaker actually believed.**
///
/// Three truth states fall out of separating those, and they are all
/// different things:
///
/// | belief | asserted | it is |
/// |---|---|---|
/// | false, sincerely held | confidently | a **mistake** |
/// | doubted | confidently | **deception** |
/// | believed | hedged | tact, or uncertainty |
/// | true | deceptively | **still manipulation** |
///
/// The listener receives the assertion and never `believed`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AssertedClaim {
    /// What is being claimed, as an opaque proposition id.
    pub proposition: u32,
    /// **Private.** How much the speaker actually credits it.
    believed: f64,
    /// Observable: how firmly they said it.
    pub asserted: f64,
}

impl AssertedClaim {
    pub fn said(proposition: u32, believed: f64, asserted: f64) -> Self {
        AssertedClaim { proposition, believed, asserted }
    }
    /// **Only the speaker's own mind may ask this.** A listener sees the
    /// assertion; the gap between the two is what a lie *is*.
    pub fn privately_believed(&self, by: Id<Person>, speaker: Id<Person>) -> Option<f64> {
        (by == speaker).then_some(self.believed)
    }
    /// Whether this was a lie, for the historian rather than for anybody
    /// in the world.
    pub fn was_deceptive(&self) -> bool {
        self.asserted - self.believed > 0.4
    }
}

/// **The parts of an apology.** An apology is an act, not a repair
/// command: what the listener does with it is theirs.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Apology {
    /// That it happened.
    pub acknowledgement: f64,
    /// That it was theirs.
    pub responsibility: f64,
    pub remorse: f64,
    /// Why — which can help or can sound like an excuse.
    pub explanation: f64,
    /// Something offered to put it right.
    pub repair: f64,
    /// That it will not happen again.
    pub promise: f64,
}

/// **What was observably said.**
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Content {
    Praise { about: Topic, strength: f64 },
    Remark { about: Topic },
    Claim(AssertedClaim),
    Request { costs_them: f64 },
    Apology(Apology),
    Insult { strength: f64 },
    /// Something meant to be funny at somebody's expense.
    Barb { at: Topic, sharpness: f64 },
    Console,
}

/// **How it was said.** Everything here is on the surface, and every
/// field of it is something another person could in principle see or
/// hear.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Delivery {
    pub warmth: f64,
    pub emphasis: f64,
    pub hesitation: f64,
    /// **What leaked.** Self-interest a poor speaker fails to keep out of
    /// their voice — observable, and therefore fair game.
    pub eagerness: f64,
    /// Hostility that got through where it was not meant to.
    pub edge: f64,
    pub smiling: bool,
    pub gesturing: bool,
    /// Said in front of people.
    pub publicly: bool,
}

/// **What crossed.** The happening `witness.rs` distributes.
#[derive(Clone, Debug, PartialEq)]
pub struct SocialAct {
    pub speaker: Id<Person>,
    pub addressee: Id<Person>,
    pub topic: Topic,
    pub content: Content,
    pub delivery: Delivery,
    pub audience: Vec<Id<Person>>,
    pub channel: Channel,
}

/// **What the speaker meant.** Private mind state. There is no method
/// anywhere that hands this to anybody else.
#[derive(Clone, Debug, PartialEq)]
pub struct SpeakerPlan {
    pub speaker: Id<Person>,
    pub addressee: Id<Person>,
    pub motives: Vec<WeightedMotive>,
    pub topic: Topic,
    pub strategy: Strategy,
}

impl SpeakerPlan {
    pub fn weight_of(&self, m: Motive) -> f64 {
        self.motives
            .iter()
            .find(|w| w.motive == m)
            .map(|w| w.weight)
            .unwrap_or(0.0)
    }
}

/// **How good somebody is at doing what they meant to do.**
///
/// It buys clarity, appropriateness, credibility, timing and the ability
/// to keep a competing motive out of the delivery. It emphatically does
/// **not** buy success: the listener's traits, values, prior relationship
/// and evidence still decide the reading, and a skilled flatterer can be
/// seen through by a suspicious man.
pub fn social_skill(mind: &Mind) -> f64 {
    let z = |f: Facet| (mind.person.z(f) as f64 / 5.0 + 0.5).clamp(0.0, 1.0);
    (0.4 * z(Facet::Gregariousness) + 0.3 * z(Facet::Assertiveness) + 0.3 * (mind.empathy as f64 / 5.0 + 0.5).clamp(0.0, 1.0))
        .clamp(0.0, 1.0)
}

/// **Do it.**
///
/// The one place a private plan becomes a public act, and the only thing
/// that crosses is content and delivery. Skill decides how faithfully the
/// delivery expresses the strategy and how much of the *other* motives
/// leak into it.
pub fn perform(plan: &SpeakerPlan, skill: f64, publicly: bool, rng: &mut Rng) -> SocialAct {
    let wobble = (rng.next_f32() as f64 - 0.5) * 0.2 * (1.0 - skill);
    // How well the intended manner comes across at all.
    let clarity = (0.35 + 0.65 * skill + wobble).clamp(0.0, 1.0);
    // **Concealment is a skill.** A clumsy speaker's angle shows.
    let leaks = (1.0 - skill).clamp(0.0, 1.0);

    let (content, mut delivery) = match plan.strategy {
        Strategy::Ingratiate => (
            Content::Praise { about: plan.topic, strength: 0.7 },
            Delivery { warmth: 0.8 * clarity, emphasis: 0.6, smiling: true, ..Default::default() },
        ),
        Strategy::Encourage => (
            Content::Praise { about: plan.topic, strength: 0.6 },
            Delivery { warmth: 0.7 * clarity, emphasis: 0.5, smiling: true, gesturing: true, ..Default::default() },
        ),
        Strategy::Tease => (
            Content::Barb { at: plan.topic, sharpness: 0.5 },
            // **The grin is what makes it teasing.** Without it the same
            // words are an insult, which is the whole of the joke that
            // does not land.
            Delivery { warmth: 0.5 * clarity, emphasis: 0.6, smiling: true, gesturing: true, ..Default::default() },
        ),
        Strategy::Intimidate => (
            Content::Insult { strength: 0.6 },
            Delivery { warmth: -0.7, emphasis: 0.9, edge: 0.8, ..Default::default() },
        ),
        Strategy::Disclose => (
            Content::Remark { about: Topic::Themselves },
            Delivery { warmth: 0.5 * clarity, hesitation: 0.4 * leaks, ..Default::default() },
        ),
        Strategy::Console => (
            Content::Console,
            Delivery { warmth: 0.8 * clarity, emphasis: 0.2, smiling: false, gesturing: true, ..Default::default() },
        ),
        Strategy::Persuade => (
            Content::Request { costs_them: 0.4 },
            Delivery { warmth: 0.4 * clarity, emphasis: 0.7, ..Default::default() },
        ),
        Strategy::Apologise => (
            Content::Apology(Apology {
                acknowledgement: 0.9 * clarity,
                responsibility: 0.8 * clarity,
                remorse: 0.8 * clarity,
                explanation: 0.4,
                repair: 0.3,
                promise: 0.5 * clarity,
            }),
            Delivery { warmth: 0.5 * clarity, hesitation: 0.3, ..Default::default() },
        ),
        Strategy::StateAFact => (
            Content::Claim(AssertedClaim::said(0, 1.0, 1.0)),
            Delivery { warmth: 0.1, emphasis: 0.4, ..Default::default() },
        ),
    };

    // **What the speaker could not keep out of it.** Observable, and the
    // less skilled they are the more of it shows.
    delivery.eagerness = (plan.weight_of(Motive::SeekingFavour) * leaks).clamp(0.0, 1.0);
    delivery.edge = (delivery.edge + plan.weight_of(Motive::Belittling) * leaks).clamp(-1.0, 1.0);
    delivery.publicly = publicly || plan.weight_of(Motive::StatusPerformance) > 0.4;

    SocialAct {
        speaker: plan.speaker,
        addressee: plan.addressee,
        topic: plan.topic,
        content,
        delivery,
        audience: Vec::new(),
        channel: if publicly { Channel::FaceToFace } else { Channel::FaceToFace },
    }
}

/// **Nothing to say.**
///
/// An opportunity is not an obligation. Somebody with no pressing need,
/// nothing to ask and nobody they want to talk to says nothing, and
/// silence has to be an ordinary outcome or every room becomes a
/// conversation.
pub fn anything_to_say(mind: &Mind, opportunity: &Opportunity) -> bool {
    if opportunity.minutes < 1.0 || opportunity.with.is_empty() {
        return false;
    }
    let wants = mind
        .needs
        .as_ref()
        .map(|n| {
            n.get(crate::needs::Need::Company).urgency() + n.get(crate::needs::Need::Friendship).urgency()
        })
        .unwrap_or(0.0);
    let outgoing = (mind.person.z(Facet::Gregariousness) as f64 / 5.0 + 0.5).clamp(0.0, 1.0);
    wants + 0.4 * outgoing > 0.35
}

/// **An exchange takes time and attention.**
///
/// A turn budget, so a conversation ends rather than recursing: people
/// run out of things to say, out of time, and out of patience.
#[derive(Clone, Debug, PartialEq)]
pub struct Exchange {
    pub minutes_left: f64,
    pub turns_left: u8,
    pub acts: Vec<SocialAct>,
}

impl Exchange {
    pub fn opening(o: &Opportunity) -> Self {
        Exchange {
            minutes_left: o.minutes,
            // Real conversations are a handful of exchanges, not an
            // unbounded loop.
            turns_left: 8,
            acts: Vec::new(),
        }
    }

    /// Returns false when there is no more conversation to be had.
    pub fn take_a_turn(&mut self, act: SocialAct, minutes: f64) -> bool {
        if self.turns_left == 0 || self.minutes_left < minutes {
            return false;
        }
        self.turns_left -= 1;
        self.minutes_left -= minutes;
        self.acts.push(act);
        true
    }

    pub fn over(&self) -> bool {
        self.turns_left == 0 || self.minutes_left <= 0.0
    }
}
