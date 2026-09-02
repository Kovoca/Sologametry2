//! **What somebody needs that is not food.**
//!
//! Slice 4 of `docs/mind-spec.md`. Two kinds, and they must not be one
//! list: a physical drive unmet is dangerous, and a psychological need
//! unmet takes your attention.
//!
//! ## Needs drive focus, and focus is not stress
//!
//! Slice 1 already separated stress, mood and focus and left a hole where
//! this goes. The pair of cases the separation exists for:
//!
//! - **stressed and focused** — a grieving parent nursing a sick child;
//! - **content and unfocused** — a scholar who has been kept from a book
//!   for a month and cannot settle to anything.
//!
//! The second one needs *this*. Until now nothing could produce it except
//! being stirred up, and a person whose needs were all neglected was
//! perfectly serene.
//!
//! ## Satisfaction has to be semantic
//!
//! The rule the whole module turns on, and the one that is easy to get
//! wrong by accident: **do not satisfy "socialise" because two people
//! stood near each other.** An activity has to declare what it actually
//! provides, and under what conditions.
//!
//! | | provides |
//! |---|---|
//! | passing a stranger in a corridor | almost nothing |
//! | talking with a friend | company **and** friendship |
//! | arguing | excitement, and **not** friendship |
//! | working a loom | craft practice, not creation |
//! | designing a new tapestry | **both** |
//! | standing in a temple | nothing at all |
//! | attending the service | worship |
//!
//! That last pair is the sharpest of them. Being in the building is not
//! the activity, and a model that counts presence will have a population
//! whose spiritual needs are met by walking past a church.
//!
//! ## Calibrated on how people actually spend a day
//!
//! Time-use surveys are the anchor, because a need that is satisfied too
//! fast makes everybody content and one satisfied too slowly makes
//! everybody wretched. Per person per day, averaged over everybody
//! *(American Time Use Survey)*:
//!
//! | | hours |
//! |---|---|
//! | sleep | 8.8 |
//! | leisure and sport | 5.4 |
//! | of which television | 2.8 |
//! | **socialising and communicating** | **0.6** |
//! | eating and drinking | 1.1 |
//! | reading | 0.3 |
//! | religious and spiritual | 0.1 |
//!
//! Thirty-eight minutes a day of socialising is the figure everything
//! here is set against — and it is an *average* over a population in
//! which a fifth do almost none.

use crate::mind::{Facet, Mind, Value};

/// **A physical drive.** Unmet, these do not distract you: they kill you.
/// Kept apart from the psychological needs for exactly that reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Drive {
    Hunger,
    Thirst,
    Sleep,
    Warmth,
    Pain,
    Safety,
}

/// **A psychological need.** Unmet, these take your attention, and over
/// long enough they make you unhappy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Need {
    /// Anybody at all: the general social need.
    Company,
    /// Particular people you are close to. **Not the same need**, which
    /// is why somebody can be surrounded all day and lonely.
    Friendship,
    Family,
    Romance,
    Worship,
    /// Having something to do. Idleness is its own complaint.
    Occupation,
    /// Making something that did not exist.
    Creation,
    Learning,
    Excitement,
    /// Working at a thing you are good at, which is not the same as
    /// making something new.
    Craft,
    MartialPractice,
    Tradition,
    Introspection,
    Celebration,
    Rest,
    Nature,
}

impl Need {
    pub const ALL: [Need; 16] = [
        Need::Company,
        Need::Friendship,
        Need::Family,
        Need::Romance,
        Need::Worship,
        Need::Occupation,
        Need::Creation,
        Need::Learning,
        Need::Excitement,
        Need::Craft,
        Need::MartialPractice,
        Need::Tradition,
        Need::Introspection,
        Need::Celebration,
        Need::Rest,
        Need::Nature,
    ];

    fn index(self) -> usize {
        Need::ALL.iter().position(|n| *n == self).unwrap()
    }

    /// **How long it takes to go from satisfied to wanting**, in days.
    ///
    /// Set against real time use: socialising is a daily business —
    /// thirty-eight minutes of it, on average — while a festival is a
    /// thing that comes round, and sitting and thinking is something
    /// people go weeks without noticing they have not done.
    fn days_to_empty(self) -> f64 {
        match self {
            Need::Company => 2.0,
            Need::Rest => 1.5,
            Need::Occupation => 2.0,
            Need::Friendship => 6.0,
            Need::Family => 5.0,
            Need::Romance => 4.0,
            Need::Excitement => 8.0,
            Need::Craft => 5.0,
            Need::Learning => 10.0,
            Need::Creation => 14.0,
            Need::Worship => 7.0,
            Need::MartialPractice => 10.0,
            Need::Introspection => 20.0,
            Need::Nature => 14.0,
            Need::Tradition => 30.0,
            Need::Celebration => 45.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Need::Company => "company",
            Need::Friendship => "friends",
            Need::Family => "family",
            Need::Romance => "romance",
            Need::Worship => "worship",
            Need::Occupation => "something to do",
            Need::Creation => "making something",
            Need::Learning => "learning",
            Need::Excitement => "excitement",
            Need::Craft => "practising a craft",
            Need::MartialPractice => "training",
            Need::Tradition => "keeping the old ways",
            Need::Introspection => "thinking",
            Need::Celebration => "celebrating",
            Need::Rest => "rest",
            Need::Nature => "green things",
        }
    }
}

/// **What has to be true for an activity to satisfy a need**, which is
/// the whole of the semantic rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Requires {
    /// Nothing: doing it is enough.
    Nothing,
    /// The other person has to be somebody you know. A stranger in a
    /// corridor is not company.
    SomebodyKnown,
    /// Kin.
    Kin,
    /// **Taking part**, rather than being in the room where it happens.
    TakingPart,
    /// Making something that did not exist before, rather than making
    /// another of something.
    SomethingNew,
}

/// One thing an activity gives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gives {
    pub need: Need,
    /// Share of the need filled by an hour of it.
    pub per_hour: f64,
    pub requires: Requires,
}

/// **What somebody is doing.** Named, because the point is that each one
/// declares what it actually provides rather than being scored on a
/// generic "sociality".
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Doing {
    PassAStranger,
    TalkWithAFriend,
    DrinkWithWorkmates,
    Argue,
    SitWithFamily,
    WorkTheLoom,
    DesignSomething,
    StandInATemple,
    AttendTheService,
    ReadABook,
    DrillWithTheMilitia,
    WalkInTheFields,
    Feast,
    SitAndThink,
    Sleep,
    StareAtAWall,
}

impl Doing {
    /// **What it provides, and on what condition.**
    pub fn gives(self) -> &'static [Gives] {
        use Doing::*;
        use Need::*;
        use Requires::*;
        match self {
            // **Almost nothing.** Proximity is not company, and a model
            // that counts it will have a population whose social needs
            // are met by walking to work.
            PassAStranger => &[Gives { need: Company, per_hour: 0.02, requires: Nothing }],
            TalkWithAFriend => &[
                Gives { need: Company, per_hour: 0.8, requires: Nothing },
                Gives { need: Friendship, per_hour: 0.7, requires: SomebodyKnown },
            ],
            DrinkWithWorkmates => &[
                Gives { need: Company, per_hour: 0.7, requires: Nothing },
                Gives { need: Friendship, per_hour: 0.25, requires: SomebodyKnown },
                Gives { need: Rest, per_hour: 0.3, requires: Nothing },
            ],
            // **An argument is contact and it is not friendship.** It can
            // even be exciting. It does not make anybody less lonely.
            Argue => &[
                Gives { need: Company, per_hour: 0.3, requires: Nothing },
                Gives { need: Excitement, per_hour: 0.5, requires: Nothing },
            ],
            SitWithFamily => &[
                Gives { need: Family, per_hour: 0.8, requires: Kin },
                Gives { need: Company, per_hour: 0.5, requires: Nothing },
            ],
            // **A loom is practice, not creation.** Doing well what you
            // already know how to do is a real satisfaction and a
            // different one.
            WorkTheLoom => &[
                Gives { need: Craft, per_hour: 0.5, requires: Nothing },
                Gives { need: Occupation, per_hour: 0.6, requires: Nothing },
            ],
            DesignSomething => &[
                Gives { need: Craft, per_hour: 0.4, requires: Nothing },
                Gives { need: Creation, per_hour: 0.6, requires: SomethingNew },
                Gives { need: Occupation, per_hour: 0.5, requires: Nothing },
            ],
            // **Being in the building is not the activity.**
            StandInATemple => &[],
            AttendTheService => &[
                Gives { need: Worship, per_hour: 0.9, requires: TakingPart },
                Gives { need: Tradition, per_hour: 0.4, requires: Nothing },
                Gives { need: Company, per_hour: 0.2, requires: Nothing },
            ],
            ReadABook => &[
                Gives { need: Learning, per_hour: 0.5, requires: Nothing },
                Gives { need: Rest, per_hour: 0.3, requires: Nothing },
            ],
            DrillWithTheMilitia => &[
                Gives { need: MartialPractice, per_hour: 0.7, requires: TakingPart },
                Gives { need: Company, per_hour: 0.3, requires: Nothing },
            ],
            WalkInTheFields => &[
                Gives { need: Nature, per_hour: 0.7, requires: Nothing },
                Gives { need: Rest, per_hour: 0.4, requires: Nothing },
                Gives { need: Introspection, per_hour: 0.2, requires: Nothing },
            ],
            Feast => &[
                Gives { need: Celebration, per_hour: 0.8, requires: TakingPart },
                Gives { need: Company, per_hour: 0.6, requires: Nothing },
                Gives { need: Tradition, per_hour: 0.3, requires: Nothing },
            ],
            SitAndThink => &[
                Gives { need: Introspection, per_hour: 0.6, requires: Nothing },
                Gives { need: Rest, per_hour: 0.2, requires: Nothing },
            ],
            Sleep => &[Gives { need: Rest, per_hour: 0.4, requires: Nothing }],
            StareAtAWall => &[],
        }
    }
}

/// **What was true while they were doing it.** The conditions the
/// requirements are checked against — supplied by the world, not assumed.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Circumstances {
    pub with_somebody_known: bool,
    pub with_kin: bool,
    /// Taking part rather than standing at the back.
    pub taking_part: bool,
    /// Making something that did not exist before.
    pub something_new: bool,
}

impl Circumstances {
    fn meets(self, r: Requires) -> bool {
        match r {
            Requires::Nothing => true,
            Requires::SomebodyKnown => self.with_somebody_known,
            Requires::Kin => self.with_kin,
            Requires::TakingPart => self.taking_part,
            Requires::SomethingNew => self.something_new,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NeedState {
    /// How much this person needs it at all. Somebody who does not care
    /// about worship is not troubled by never worshipping.
    pub weight: f64,
    /// 1 is met, 0 is not.
    pub satisfaction: f64,
    pub days_since: f64,
}

impl NeedState {
    /// **What it is costing them.** A heavy need barely met costs more
    /// than a light need wholly neglected, which is why weight and
    /// satisfaction are two numbers.
    pub fn urgency(&self) -> f64 {
        self.weight * (1.0 - self.satisfaction)
    }
}

/// Everything somebody wants that is not dinner.
#[derive(Clone, Debug, PartialEq)]
pub struct Needs {
    states: [NeedState; 16],
}

impl Needs {
    /// **Weights out of values and personality**, which is what makes two
    /// people in the same town want different lives.
    ///
    /// ```text
    /// weight = baseline
    ///        + what they believe
    ///        + what they are like
    ///        + where they are in a life
    /// ```
    pub fn of(mind: &Mind) -> Self {
        let f = |x: Facet| (mind.person.z(x) as f64 / 4.0).clamp(-0.5, 0.5);
        let v = |x: Value| (mind.conviction(x) as f64 / 50.0).clamp(-1.0, 1.0);
        let w = |base: f64, terms: f64| (base + terms).clamp(0.05, 1.0);
        let mut states = [NeedState { weight: 0.3, satisfaction: 1.0, days_since: 0.0 }; 16];
        let set = |states: &mut [NeedState; 16], n: Need, weight: f64| {
            states[n.index()].weight = weight;
        };
        // Gregariousness drives the general social need; valuing
        // friendship drives the particular one. They are not the same,
        // which is why a sociable person with no friends is still lonely.
        set(&mut states, Need::Company, w(0.45, f(Facet::Gregariousness) - 0.5 * f(Facet::Privacy)));
        set(&mut states, Need::Friendship, w(0.35, 0.4 * v(Value::Friendship) + 0.5 * f(Facet::Altruism)));
        set(&mut states, Need::Family, w(0.3, 0.5 * v(Value::Family)));
        set(&mut states, Need::Romance, w(0.3, 0.3 * f(Facet::Gregariousness)));
        set(&mut states, Need::Worship, w(0.15, 0.6 * v(Value::Tradition)));
        set(&mut states, Need::Occupation, w(0.4, f(Facet::Perseverance) + 0.3 * f(Facet::Dutifulness)));
        set(&mut states, Need::Creation, w(0.2, 0.5 * f(Facet::LoveOfMaking) + 0.4 * v(Value::Artistry)));
        set(&mut states, Need::Learning, w(0.25, f(Facet::Curiosity) + 0.4 * v(Value::Knowledge)));
        set(&mut states, Need::Excitement, w(0.25, f(Facet::ExcitementSeeking)));
        set(&mut states, Need::Craft, w(0.3, 0.5 * v(Value::Craftsmanship) + 0.3 * f(Facet::Perseverance)));
        set(&mut states, Need::MartialPractice, w(0.1, 0.7 * v(Value::MartialProwess)));
        set(&mut states, Need::Tradition, w(0.15, 0.6 * v(Value::Tradition)));
        set(&mut states, Need::Introspection, w(0.15, 0.4 * f(Facet::Privacy)));
        set(&mut states, Need::Celebration, w(0.2, 0.4 * f(Facet::Gregariousness)));
        set(&mut states, Need::Rest, w(0.4, 0.0));
        set(&mut states, Need::Nature, w(0.2, 0.4 * v(Value::Nature)));
        Needs { states }
    }

    pub fn get(&self, n: Need) -> NeedState {
        self.states[n.index()]
    }

    /// **Do something for a while, and get what it actually provides.**
    ///
    /// Returns what was satisfied, so a caller can tell the difference
    /// between an evening that helped and one that did not.
    pub fn did(&mut self, what: Doing, hours: f64, how: Circumstances) -> Vec<(Need, f64)> {
        let mut got = Vec::new();
        for g in what.gives() {
            if !how.meets(g.requires) {
                continue;
            }
            let s = &mut self.states[g.need.index()];
            let before = s.satisfaction;
            s.satisfaction = (s.satisfaction + g.per_hour * hours).min(1.0);
            s.days_since = 0.0;
            if s.satisfaction > before {
                got.push((g.need, s.satisfaction - before));
            }
        }
        got
    }

    /// A day passes and everything drains at its own rate.
    pub fn a_day_passes(&mut self) {
        for n in Need::ALL {
            let s = &mut self.states[n.index()];
            s.satisfaction = (s.satisfaction - 1.0 / n.days_to_empty()).max(0.0);
            s.days_since += 1.0;
        }
    }

    /// **What all of it is costing, together.**
    ///
    /// Deliberately not a sum: twelve mildly unmet needs are a bad week,
    /// not a catastrophe, so it saturates. What a heavily neglected
    /// single need does is worse than what several mild ones do, which is
    /// why the largest is weighted separately.
    pub fn debt(&self) -> f64 {
        let total: f64 = Need::ALL.iter().map(|n| self.get(*n).urgency()).sum();
        let worst = Need::ALL
            .iter()
            .map(|n| self.get(*n).urgency())
            .fold(0.0, f64::max);
        (0.35 * worst + 0.10 * total).min(1.0)
    }

    /// The one most worth doing something about — what an idle person
    /// goes looking for.
    pub fn most_pressing(&self) -> Option<(Need, f64)> {
        Need::ALL
            .iter()
            .map(|n| (*n, self.get(*n).urgency()))
            .filter(|(_, u)| *u > 0.05)
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
    }

    /// **Long neglect is not merely distracting.**
    ///
    /// Focus goes first and unhappiness follows, which is the order it
    /// happens in: somebody kept from what they care about is scattered
    /// for a fortnight before they are miserable about it.
    ///
    /// **A fortnight is the floor**, because a multiple of the drain rate
    /// alone made four days of solitude a formal complaint. Company
    /// empties in two days — that is when it starts to distract — and a
    /// bad patch is not a grievance. What makes it one is that it has
    /// gone on.
    pub fn grievances(&self) -> Vec<Need> {
        Need::ALL
            .iter()
            .copied()
            .filter(|n| {
                let s = self.get(*n);
                s.satisfaction <= 0.0
                    && s.weight > 0.35
                    && s.days_since > (4.0 * n.days_to_empty()).max(14.0)
            })
            .collect()
    }
}
