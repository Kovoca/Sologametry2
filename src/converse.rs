//! **Walking up to somebody and talking to them.**
//!
//! Every part of this already existed and nothing had put it together.
//! That is the whole of this module: it invents no new rule, it
//! *assembles*.
//!
//! When somebody is asked a question, what comes back is built out of:
//!
//! - **what they know**, from `memory` — and how they came to know it,
//!   so a rumour answers differently from having been there;
//! - **whether they could have known**, from `witness` — somebody who
//!   was not there has nothing to tell you, and cannot be made to;
//! - **what they make of you**, from `relations` — trust by domain,
//!   affection, a grievance still open;
//! - **what they are carrying**, from `mind` and `coping` — a man three
//!   years into being out of work is not good company and it is not a
//!   personality trait;
//! - **what they believe**, from `growth` — and how firmly, which is
//!   whether an argument is worth having;
//! - **what they need**, from `needs` — somebody lonely gets more out of
//!   the same conversation.
//!
//! And what crosses back is a `SocialAct`, so the player reads it the
//! same way any other listener does. **There is no privileged channel
//! for the player**: no answer is more truthful because it was a person
//! asking.

use crate::coping::FunctionalState;
use crate::id::Id;
use crate::memory::{Memory, PerceivedWho, Source, Trace, WorldEvent};
use crate::mind::{Facet, Mind};
use crate::relations::{Relationship, TrustIn};
use crate::social::{Content, Delivery, SocialAct, Topic};

/// **Why anybody said anything.**
///
/// The module began by modelling somebody walking up to somebody else
/// and starting a conversation, and that is **not what most talking
/// is**. Overwhelmingly, an exchange is *occasioned*: a man holds a door
/// and hears "thank you" and says "you are welcome", and the whole thing
/// is over in two seconds and was complete. Nobody chose to have a
/// conversation. Somebody did something small and the words came with
/// it.
///
/// The consequence that matters is that **an occasioned exchange is not
/// awkward.** The act supplies the reason to be speaking, which is
/// exactly what a deliberate approach to a stranger lacks — and it is
/// why holding a door for somebody is easy and stopping the same person
/// to ask them something is not.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Occasion {
    /// A small kindness that happened to fall to you.
    Courtesy(Courtesy),
    /// Buying something. The words are part of the transaction.
    Transaction,
    /// Working at the same thing.
    SharedTask,
    /// You were in each other's way.
    Collision,
    /// **You went to talk to them**, which is the rarest of these and
    /// the only one that has to carry its own reason.
    Deliberate,
}

/// The small acts that carry words with them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Courtesy {
    HeldDoor,
    GaveWay,
    ReturnedDropped,
    Helped,
}

impl Occasion {
    /// **How much of a reason to be speaking this already supplies.**
    pub fn reason_to_speak(self) -> f64 {
        match self {
            Occasion::Courtesy(_) => 0.9,
            Occasion::Transaction => 0.95,
            Occasion::SharedTask => 0.8,
            Occasion::Collision => 0.7,
            Occasion::Deliberate => 0.0,
        }
    }

    /// **How long it lasts.** Nearly all of these are two turns and
    /// over, and treating them as the opening of a conversation is how a
    /// model ends up with everybody chatting all day.
    pub fn turns(self) -> u8 {
        match self {
            Occasion::Courtesy(_) | Occasion::Collision => 2,
            Occasion::Transaction => 4,
            Occasion::SharedTask => 6,
            Occasion::Deliberate => 8,
        }
    }

    /// Whether it is finished when it is finished. A courtesy is
    /// **complete**: it does not lead anywhere unless something else
    /// gives it somewhere to go.
    pub fn is_complete_in_itself(self) -> bool {
        !matches!(self, Occasion::Deliberate | Occasion::SharedTask)
    }
}

/// **What a courtesy sounds like.** Two lines, and the second is
/// somebody being decent back.
pub fn courtesy_exchange(c: Courtesy) -> (&'static str, &'static str) {
    match c {
        Courtesy::HeldDoor => ("\"Thank you.\"", "\"You are welcome.\""),
        Courtesy::GaveWay => ("A nod.", "A nod back."),
        Courtesy::ReturnedDropped => ("\"Oh — thank you.\"", "\"Not at all.\""),
        Courtesy::Helped => ("\"That is good of you.\"", "\"It is nothing.\""),
    }
}

/// **How you came to be talking to them at all.**
///
/// `social.rs` already holds that people do not converse because
/// compatible records exist but because they are in the same room for a
/// reason and there is time. This is that, at the moment of approach —
/// and it matters more than most of what either person is like.
///
/// **Walking up to a stranger going about their business is awkward**,
/// and it is awkward for reasons that have nothing to do with either of
/// them: there is no standing reason to be talking, they were doing
/// something else, and the setting is not one where being approached is
/// expected.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Approach {
    /// How well they know you, from the relationship they hold. Zero is
    /// a stranger.
    pub familiarity: f64,
    /// **What they were doing instead.** Interrupting somebody mid-task
    /// is a different act from catching them idle.
    pub they_are_busy: f64,
    /// **Whether the setting is one where this is normal.** A shop
    /// counter, an inn, a market stall: being approached by strangers is
    /// the whole function of the place. A doorstep at night is not.
    pub setting_invites: f64,
    /// Whether other people can see, which cuts both ways — it makes an
    /// approach safer and a refusal more costly.
    pub in_public: bool,
    /// **Why anybody is speaking.** An occasion supplies the reason, and
    /// having a reason is most of what makes an exchange comfortable.
    pub occasion: Occasion,
}

impl Default for Approach {
    /// A stranger, on the street, while they are getting on with
    /// something. The ordinary case, and the awkward one.
    fn default() -> Self {
        Approach {
            familiarity: 0.0,
            they_are_busy: 0.6,
            setting_invites: 0.15,
            in_public: true,
            occasion: Occasion::Deliberate,
        }
    }
}

impl Approach {
    /// Somebody you know, with nothing else to do.
    pub fn a_friend() -> Self {
        Approach {
            familiarity: 0.8,
            they_are_busy: 0.1,
            setting_invites: 0.6,
            in_public: false,
            occasion: Occasion::Deliberate,
        }
    }

    /// **Somebody held a door for you.** A stranger, in the street, and
    /// not awkward at all — which is the point.
    pub fn a_courtesy(c: Courtesy) -> Self {
        Approach {
            familiarity: 0.0,
            they_are_busy: 0.5,
            setting_invites: 0.2,
            in_public: true,
            occasion: Occasion::Courtesy(c),
        }
    }

    /// Across a counter, which is what counters are for.
    pub fn at_a_counter() -> Self {
        Approach {
            familiarity: 0.1,
            they_are_busy: 0.4,
            setting_invites: 1.0,
            in_public: true,
            occasion: Occasion::Transaction,
        }
    }

    /// **How much this costs them, 0..1.**
    ///
    /// Being known is most of it; the setting can do the rest, which is
    /// why a shopkeeper is perfectly civil to a stranger and the same
    /// man on his own doorstep is not. Being busy is a real cost and a
    /// smaller one than either.
    pub fn awkwardness(&self) -> f64 {
        let unknown = 1.0 - self.familiarity.clamp(0.0, 1.0);
        let unexpected = 1.0 - self.setting_invites.clamp(0.0, 1.0);
        let raw = 0.55 * unknown * unexpected + 0.25 * self.they_are_busy.clamp(0.0, 1.0)
            - if self.in_public { 0.05 } else { 0.0 };
        // **Having a reason to be speaking is most of it.** A stranger
        // who has just held a door for you is not an intrusion; the same
        // stranger stopping you to ask something is.
        (raw * (1.0 - 0.9 * self.occasion.reason_to_speak())).clamp(0.0, 1.0)
    }
}

/// What somebody was asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked {
    /// "What happened?" — about a particular event.
    About(Id<WorldEvent>),
    /// "How are you?"
    HowTheyAre,
    /// "What do you think of him?"
    Opinion,
    Greeting,
}

/// **Why they answered the way they did.**
///
/// Exposed because a simulation nobody can interrogate is a simulation
/// nobody can debug, and because the reason is more interesting than the
/// line.
#[derive(Clone, Debug, PartialEq)]
pub struct Because {
    /// Whether they know anything about it at all.
    pub knows: bool,
    /// How they came to know it, if they do.
    pub how: Option<Source>,
    /// How sure they are *now*, which is not how sure they were.
    pub confidence: f64,
    /// What they will say happened — which may be wrong, and stays wrong
    /// until somebody corrects them.
    pub blames: Option<PerceivedWho>,
    /// What they make of whoever is asking.
    pub trusts_asker: f64,
    /// How much they are carrying.
    pub state: FunctionalState,
    /// **What the approach itself cost.** Nothing to do with either
    /// person: it is being a stranger, somewhere this is not done,
    /// while they were doing something else.
    pub awkwardness: f64,
}

/// **What came back.**
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    /// The act itself, which is all a listener gets.
    pub act: SocialAct,
    /// In words, for a terminal. **A rendering, not the thing** — the
    /// same rule the glyphs follow.
    pub said: String,
    pub because: Because,
}

/// **Ask somebody something.**
///
/// `about_the_asker` is the relationship *they* hold — not the one the
/// asker holds, because the two need not agree and it is theirs that
/// governs what they are willing to say.
pub fn ask(
    who: &Mind,
    what_they_know: &Memory,
    about_the_asker: Option<&Relationship>,
    state: FunctionalState,
    lonely: f64,
    how_you_came_to_be_there: &Approach,
    asked: Asked,
    speaker: Id<crate::person::Person>,
    listener: Id<crate::person::Person>,
) -> Answer {
    let trusts = about_the_asker
        .map(|r| r.trust_in(TrustIn::General))
        .unwrap_or(0.0);
    let fond = about_the_asker.map(|r| r.affection()).unwrap_or(0.0);
    let aggrieved = about_the_asker.map(|r| r.resentment()).unwrap_or(0.0);

    // **What being worn out does to talking to somebody.** Not a trait:
    // the same man is worse company in a bad year than a good one.
    let spare = match state {
        FunctionalState::Regulated => 1.0,
        FunctionalState::Strained => 0.8,
        FunctionalState::Depleted => 0.5,
        FunctionalState::Impaired => 0.25,
    };
    // **And the person themselves.** A private man is shorter with a
    // stranger than an open one, and somebody having a bad week is
    // shorter than the same man having a good one — which is mood, not
    // character, and the two are already separate.
    let reserve = (who.person.z(Facet::Privacy) as f64 / 5.0).clamp(-0.4, 0.4);
    let today = who.mood.valence.clamp(-1.0, 1.0);
    // **And how you came to be talking to them at all**, which is
    // frequently the largest term. A private man approached cold in the
    // street by somebody he does not know, while he is doing something
    // else, is the worst case there is — and none of that is about
    // either of them disliking the other.
    let awkward = how_you_came_to_be_there.awkwardness() * (0.6 + 0.8 * reserve.max(0.0));
    let warmth = ((0.3 + 0.5 * fond + 0.3 * trusts - 0.6 * aggrieved - reserve + 0.2 * today
        - awkward)
        * spare)
        .clamp(-1.0, 1.0);

    let mut because = Because {
        knows: false,
        how: None,
        confidence: 0.0,
        blames: None,
        trusts_asker: trusts,
        state,
        awkwardness: how_you_came_to_be_there.awkwardness(),
    };

    let (content, said) = match asked {
        Asked::Greeting => {
            // Somebody lonely is glad of it; somebody with a grievance is
            // not. Neither is a dialogue tree.
            let s = if aggrieved > 0.3 {
                "He looks at you and says nothing much.".to_string()
            } else if because.awkwardness > 0.45 {
                // Not unfriendliness. He was doing something, you are
                // nobody he knows, and this is not a place for it.
                "He gives you a wary look and waits to hear what you want.".to_string()
            } else if lonely > 0.5 {
                "He seems glad of the company.".to_string()
            } else {
                "A nod.".to_string()
            };
            (
                Content::Remark {
                    about: Topic::Weather,
                },
                s,
            )
        }
        Asked::HowTheyAre => {
            // **He answers about his life, not his traits.** Which is the
            // two layers again: a man can be functioning and miserable.
            let s = match state {
                FunctionalState::Regulated => "Well enough.".to_string(),
                FunctionalState::Strained => "Managing. It has been a long while.".to_string(),
                FunctionalState::Depleted => {
                    "He takes a moment. \"I am tired,\" he says, and leaves it.".to_string()
                }
                FunctionalState::Impaired => {
                    "He does not really answer. Whatever is wrong is not a thing he has words for."
                        .to_string()
                }
            };
            (
                Content::Remark {
                    about: Topic::Themselves,
                },
                s,
            )
        }
        Asked::Opinion => {
            let s = if aggrieved > 0.3 {
                "\"I have nothing to say about him.\"".to_string()
            } else if fond > 0.3 {
                "He speaks warmly of him.".to_string()
            } else {
                "A shrug. He hardly knows the man.".to_string()
            };
            (
                Content::Remark {
                    about: Topic::AThirdParty(0),
                },
                s,
            )
        }
        Asked::About(event) => {
            // **The whole point.** What he says comes out of what he
            // took in, how he came by it, and how sure he is now — and
            // he cannot be made to know what he did not see.
            match what_was_seen(what_they_know, event) {
                None => (
                    Content::Remark {
                        about: Topic::Weather,
                    },
                    "\"I would not know. I was not there.\"".to_string(),
                ),
                Some(t) => {
                    because.knows = true;
                    because.how = Some(t.provenance());
                    because.confidence = t.confidence;
                    because.blames = t.blamed.clone();
                    // **A man who does not trust you tells you less**,
                    // and that is discretion rather than deception.
                    let s = if trusts < -0.3 {
                        "\"There was trouble. That is all I will say to you.\"".to_string()
                    } else if because.awkwardness > 0.5 && trusts < 0.2 {
                        // A stranger stopping him in the street gets the
                        // short version, however much he knows.
                        "\"There was some trouble. I have somewhere to be.\"".to_string()
                    } else if reserve > 0.25 && trusts < 0.2 {
                        // Not distrust — reticence. He knows perfectly
                        // well and does not volunteer it to a stranger.
                        "\"I saw it. I would rather not go into it.\"".to_string()
                    } else {
                        crate::memory::testimony(t)
                    };
                    (
                        Content::Remark {
                            about: Topic::AnEvent(t.kind()),
                        },
                        s,
                    )
                }
            }
        }
    };

    Answer {
        act: SocialAct {
            speaker: listener,
            addressee: speaker,
            topic: Topic::Weather,
            content,
            delivery: Delivery {
                warmth,
                // Somebody at the end of themselves is not smooth about
                // it; the hesitation is the state showing through.
                hesitation: (1.0 - spare).max(0.0),
                smiling: warmth > 0.4,
                ..Default::default()
            },
            audience: Vec::new(),
            channel: crate::social::Channel::FaceToFace,
        },
        said,
        because,
    }
}

/// **What this person has of an event, if anything.**
///
/// Not a lookup into the world's record — a search of *theirs*. Somebody
/// who was not there has no trace, and there is no route by which one
/// can be produced for them.
pub fn what_was_seen(what_they_know: &Memory, event: Id<WorldEvent>) -> Option<&Trace> {
    what_they_know
        .traces
        .values()
        .find(|t| t.of_event() == Some(event))
}

/// Whether somebody has anything to say at all — which is not the same
/// as being willing to say it.
pub fn has_anything_to_say(what_they_know: &Memory, event: Id<WorldEvent>) -> bool {
    what_was_seen(what_they_know, event).is_some()
}

/// How much good the conversation did them, which is a `needs` question
/// and not a dialogue one.
pub fn worth_of_it(warmth: f64, lonely: f64, minutes: f64) -> f64 {
    (0.2 + 0.8 * lonely) * warmth.max(0.0) * (minutes / 30.0).min(1.5)
}

/// **What it cost the person who was approached.**
///
/// Small, and not nothing: an interruption from a stranger is a real
/// imposition, which is most of why people do not do it and why a place
/// that exists to be walked into is such a useful thing.
pub fn cost_to_them(a: &Approach, minutes: f64) -> f64 {
    a.awkwardness() * (0.3 + 0.7 * a.they_are_busy.clamp(0.0, 1.0)) * (minutes / 30.0).min(2.0)
}
