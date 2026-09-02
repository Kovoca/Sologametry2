//! **A person is not one happiness number.**
//!
//! The whole argument of `docs/mind-spec.md` in one line, and the reason
//! this module exists: a mind is an assembly of temperament, beliefs,
//! tastes, needs, ambitions, relationships, immediate emotions,
//! accumulated stress and memories, and **an event goes on affecting
//! somebody after it is over**.
//!
//! This is slice 1 of nine: the static mind, an appraisal that produces
//! *several* emotions from one event, and the separation of stress, mood
//! and focus. Memory, perception, needs, relationships and the social
//! layer come after it.
//!
//! ## Calibrated on the Big Five, not on a game's tables
//!
//! Dwarf Fortress's personality system **is** a five-factor model, which
//! is the hook that lets this project keep its own rule — anchor on
//! measured figures — while building what the specification asks for.
//! The real anchors:
//!
//! | | measured |
//! |---|---|
//! | structure | five factors, facets grouped beneath them |
//! | distribution | approximately normal; most people are middling |
//! | heritability | **40-60%** |
//! | rank-order stability | r ≈ **0.6-0.7** across decades |
//! | mean-level drift | conscientiousness and agreeableness rise with age, neuroticism falls — the *maturity principle* |
//!
//! That last row is why facets are stored as something that can move at
//! all: personality is stable, not fixed, and the specification's slice 7
//! needs somewhere for a core memory to push.
//!
//! **A facet is a weight, not a command.** High anger propensity does not
//! mean attacking people; it means a lower threshold, a stronger
//! reaction, slower de-escalation and a greater chance of choosing a
//! confrontational response. Everything here obeys that.

use crate::rng::Rng;

/// **Capability, not desire.** An attribute says what somebody *can* do,
/// never what they want — a highly empathic person may be cruel, because
/// they read another's pain accurately and do not care.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attributes {
    pub analytical: u8,
    pub memory: u8,
    pub willpower: u8,
    pub creativity: u8,
    pub intuition: u8,
    pub patience: u8,
    pub linguistic: u8,
    pub spatial: u8,
    /// Reading what somebody else feels. Distinct from caring about it,
    /// which is `Facets::altruism`.
    pub empathy: u8,
    pub social_awareness: u8,
}

/// **How somebody tends to react**, 0-100, most people between 40 and 60.
///
/// Grouped under the five factors, because that is what the structure
/// actually is. A world where everybody has three extreme traits is a
/// collection of caricatures, so the distribution matters more than the
/// list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Facets {
    // --- neuroticism ---
    pub anger: u8,
    pub anxiety: u8,
    pub depression_propensity: u8,
    /// How much a given load of stress hurts. Separate from how much
    /// arrives, which is what the appraisal decides.
    pub stress_vulnerability: u8,
    pub envy: u8,
    // --- extraversion ---
    pub gregariousness: u8,
    pub assertiveness: u8,
    pub excitement_seeking: u8,
    pub cheerfulness: u8,
    // --- agreeableness ---
    pub trust: u8,
    pub altruism: u8,
    pub cruelty: u8,
    pub tolerance: u8,
    pub gratitude: u8,
    // --- conscientiousness ---
    pub dutifulness: u8,
    pub perseverance: u8,
    pub orderliness: u8,
    pub ambition: u8,
    // --- openness ---
    pub curiosity: u8,
    pub creativity_love: u8,
    // --- appetite and inhibition ---
    pub greed: u8,
    pub violence: u8,
    pub pride: u8,
    pub vengefulness: u8,
    /// How much of themselves they will let anybody see. High privacy
    /// refuses consolation, which is one of the ways grief goes wrong.
    pub privacy: u8,
}

/// **What somebody holds to be admirable or proper.** Distinct from a
/// facet: a facet is how they react, a value is what they believe.
///
/// The distinction is the whole point. Two people with identical anger
/// behave differently if one values peace and law and the other does not
/// — the first shouts and threatens to report you, the second hits you.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Value {
    Law,
    Loyalty,
    Family,
    Friendship,
    Power,
    Truth,
    Fairness,
    Cooperation,
    Independence,
    Sacrifice,
    Tradition,
    Craftsmanship,
    Knowledge,
    Nature,
    Peace,
    Leisure,
    Commerce,
    MartialProwess,
    Artistry,
}

impl Value {
    pub const ALL: [Value; 19] = [
        Value::Law,
        Value::Loyalty,
        Value::Family,
        Value::Friendship,
        Value::Power,
        Value::Truth,
        Value::Fairness,
        Value::Cooperation,
        Value::Independence,
        Value::Sacrifice,
        Value::Tradition,
        Value::Craftsmanship,
        Value::Knowledge,
        Value::Nature,
        Value::Peace,
        Value::Leisure,
        Value::Commerce,
        Value::MartialProwess,
        Value::Artistry,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Value::Law => "law",
            Value::Loyalty => "loyalty",
            Value::Family => "family",
            Value::Friendship => "friendship",
            Value::Power => "power",
            Value::Truth => "truth",
            Value::Fairness => "fairness",
            Value::Cooperation => "cooperation",
            Value::Independence => "independence",
            Value::Sacrifice => "sacrifice",
            Value::Tradition => "tradition",
            Value::Craftsmanship => "craftsmanship",
            Value::Knowledge => "knowledge",
            Value::Nature => "nature",
            Value::Peace => "peace",
            Value::Leisure => "leisure",
            Value::Commerce => "commerce",
            Value::MartialProwess => "martial prowess",
            Value::Artistry => "artistic expression",
        }
    }
}

/// Conviction runs from intense rejection to intense admiration, and
/// carries **where it came from**: culture sets a baseline and an
/// individual departs from it, which is what makes a heretic possible.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Conviction {
    pub topic: Value,
    /// -50 (contempt) to +50 (reverence).
    pub held: i8,
    /// What the culture around them holds, so the *distance* can be
    /// measured. A person who values peace at +40 in a warlike culture is
    /// a different person from one who does so in a peaceful one.
    pub cultural: i8,
}

impl Conviction {
    /// How far this person stands from the people around them.
    pub fn heterodoxy(self) -> i16 {
        (self.held as i16 - self.cultural as i16).abs()
    }
}

/// **An emotion, not a happiness delta.** One event produces several of
/// these, and they can disagree with each other.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Emotion {
    Joy,
    Pride,
    Satisfaction,
    Gratitude,
    Affection,
    Relief,
    Admiration,
    Hope,
    Fear,
    Anxiety,
    Anger,
    Resentment,
    Grief,
    Guilt,
    Shame,
    Envy,
    Outrage,
    Frustration,
    Discouragement,
    Humiliation,
    Loneliness,
    Boredom,
}

impl Emotion {
    /// **Pleasant or not**, +1 to -1. The one number an emotion is
    /// allowed to be reduced to, and only for arithmetic on mood.
    pub fn valence(self) -> f64 {
        match self {
            Emotion::Joy | Emotion::Pride | Emotion::Satisfaction => 1.0,
            Emotion::Gratitude | Emotion::Affection | Emotion::Relief => 0.8,
            Emotion::Admiration | Emotion::Hope => 0.6,
            Emotion::Boredom => -0.2,
            Emotion::Frustration | Emotion::Discouragement => -0.5,
            Emotion::Anxiety | Emotion::Loneliness | Emotion::Envy => -0.6,
            Emotion::Fear | Emotion::Anger | Emotion::Guilt => -0.8,
            Emotion::Resentment | Emotion::Outrage | Emotion::Shame => -0.8,
            Emotion::Grief | Emotion::Humiliation => -1.0,
        }
    }

    /// **How much it stirs somebody up**, which is not the same as
    /// whether it is pleasant. Grief and boredom are both unpleasant and
    /// only one of them makes anybody do something.
    pub fn arousal(self) -> f64 {
        match self {
            Emotion::Anger | Emotion::Outrage | Emotion::Fear => 1.0,
            Emotion::Humiliation | Emotion::Joy => 0.8,
            Emotion::Anxiety | Emotion::Envy | Emotion::Frustration => 0.7,
            Emotion::Pride | Emotion::Resentment | Emotion::Shame => 0.5,
            Emotion::Gratitude | Emotion::Affection | Emotion::Admiration => 0.4,
            Emotion::Hope | Emotion::Guilt | Emotion::Relief => 0.3,
            Emotion::Satisfaction | Emotion::Loneliness => 0.2,
            Emotion::Grief | Emotion::Discouragement => 0.2,
            Emotion::Boredom => 0.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Emotion::Joy => "joy",
            Emotion::Pride => "pride",
            Emotion::Satisfaction => "satisfaction",
            Emotion::Gratitude => "gratitude",
            Emotion::Affection => "affection",
            Emotion::Relief => "relief",
            Emotion::Admiration => "admiration",
            Emotion::Hope => "hope",
            Emotion::Fear => "fear",
            Emotion::Anxiety => "anxiety",
            Emotion::Anger => "anger",
            Emotion::Resentment => "resentment",
            Emotion::Grief => "grief",
            Emotion::Guilt => "guilt",
            Emotion::Shame => "shame",
            Emotion::Envy => "envy",
            Emotion::Outrage => "outrage",
            Emotion::Frustration => "frustration",
            Emotion::Discouragement => "discouragement",
            Emotion::Humiliation => "humiliation",
            Emotion::Loneliness => "loneliness",
            Emotion::Boredom => "boredom",
        }
    }
}

/// One felt emotion, with what caused it and how fast it fades.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Felt {
    pub what: Emotion,
    /// 0-1.
    pub strength: f64,
    /// Days since it began.
    pub age_days: f64,
}

/// **What happened, as this person read it.**
///
/// Deliberately an appraisal input rather than a world event: slice 3
/// splits `WorldEvent` from `PerceivedEvent` and this is the shape the
/// second one collapses to. A person reacts to what they believe
/// happened.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Appraisal {
    /// How bad or good, before any of this person is taken into account.
    /// -1 to +1.
    pub severity: f64,
    /// Did it happen to *them*? 0-1.
    pub to_me: f64,
    /// Did it happen to somebody they care about? 0-1.
    pub to_mine: f64,
    /// Somebody did this on purpose. Blame needs an author.
    pub deliberate: bool,
    /// Whether the author was this person themselves — which is the
    /// difference between anger and guilt.
    pub my_doing: bool,
    /// Could they have stopped it? Low control is what turns fear into
    /// anxiety and anger into resentment.
    pub control: f64,
    /// How much of a surprise. Novelty drives intensity.
    pub unexpected: f64,
    /// A value this bears on, and whether it was upheld or broken.
    pub touches: Option<(Value, bool)>,
    /// Somebody else got something this person wanted.
    pub someone_gained: bool,
    /// It blocked something they were trying to do.
    pub blocks_a_goal: bool,
    /// Nothing happened, for a long time. Boredom has to come from
    /// somewhere and it is not an event.
    pub nothing_happening: bool,
}

impl Default for Appraisal {
    fn default() -> Self {
        Appraisal {
            severity: 0.0,
            to_me: 0.0,
            to_mine: 0.0,
            deliberate: false,
            my_doing: false,
            control: 0.5,
            unexpected: 0.3,
            touches: None,
            someone_gained: false,
            blocks_a_goal: false,
            nothing_happening: false,
        }
    }
}

/// **Accumulated burden**, over months and years. Not mood and not focus.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stress {
    /// What they are carrying.
    pub load: f64,
    /// Where they sit when nothing is happening — temperament, not
    /// circumstance.
    pub baseline: f64,
    /// How much they can carry before it shows.
    pub tolerance: f64,
    /// How fast it drains on a good day.
    pub recovery: f64,
}

/// **A medium-term bias**, over hours and weeks. What it does is change
/// how an *ambiguous* event is read: an irritable person hears an insult
/// in a neutral remark, an anxious one hears a threat.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Mood {
    pub valence: f64,
    pub arousal: f64,
    pub irritability: f64,
    pub anxiety_bias: f64,
}

/// **Cognitive readiness.** Emphatically not stress: a grieving parent
/// can be flatly focused on a sick child, and a contented scholar who has
/// been kept from a book for a month cannot concentrate on anything.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Focus {
    pub current: f64,
    pub capacity: f64,
}

/// Everything that makes somebody react the way they do.
#[derive(Clone, Debug)]
pub struct Mind {
    pub attributes: Attributes,
    pub facets: Facets,
    pub values: Vec<Conviction>,
    pub feeling: Vec<Felt>,
    pub stress: Stress,
    pub mood: Mood,
    pub focus: Focus,
}

/// Draw 0-100 with most people in the middle.
///
/// Three uniforms averaged — the same shape this project already uses for
/// aptitude and diligence — which puts about two thirds inside 40-60 and
/// makes an extreme trait genuinely uncommon rather than merely less
/// likely. A world where everybody has three of them is a collection of
/// caricatures.
fn facet(rng: &mut Rng) -> u8 {
    let t = (rng.next_f32() + rng.next_f32() + rng.next_f32()) / 3.0;
    (t * 100.0).round().clamp(0.0, 100.0) as u8
}

impl Mind {
    /// **A person, drawn.** Culture supplies the baseline convictions and
    /// the individual departs from it.
    pub fn draw(rng: &mut Rng, culture: &[(Value, i8)]) -> Self {
        let mut f = |r: &mut Rng| facet(r);
        let facets = Facets {
            anger: f(rng),
            anxiety: f(rng),
            depression_propensity: f(rng),
            stress_vulnerability: f(rng),
            envy: f(rng),
            gregariousness: f(rng),
            assertiveness: f(rng),
            excitement_seeking: f(rng),
            cheerfulness: f(rng),
            trust: f(rng),
            altruism: f(rng),
            cruelty: f(rng),
            tolerance: f(rng),
            gratitude: f(rng),
            dutifulness: f(rng),
            perseverance: f(rng),
            orderliness: f(rng),
            ambition: f(rng),
            curiosity: f(rng),
            creativity_love: f(rng),
            greed: f(rng),
            violence: f(rng),
            pride: f(rng),
            vengefulness: f(rng),
            privacy: f(rng),
        };
        let attributes = Attributes {
            analytical: f(rng),
            memory: f(rng),
            willpower: f(rng),
            creativity: f(rng),
            intuition: f(rng),
            patience: f(rng),
            linguistic: f(rng),
            spatial: f(rng),
            empathy: f(rng),
            social_awareness: f(rng),
        };
        // **An individual departs from their culture**, by a normal-ish
        // amount. Everybody agreeing with the culture exactly leaves no
        // room for a heretic, a reformer or a criminal who thinks they
        // are in the right.
        let values = Value::ALL
            .iter()
            .map(|&topic| {
                let cultural = culture
                    .iter()
                    .find(|(t, _)| *t == topic)
                    .map(|(_, v)| *v)
                    .unwrap_or(0);
                let drift =
                    ((rng.next_f32() + rng.next_f32() + rng.next_f32()) / 3.0 - 0.5) * 60.0;
                Conviction {
                    topic,
                    held: (cultural as f32 + drift).clamp(-50.0, 50.0) as i8,
                    cultural,
                }
            })
            .collect();
        let vulnerability = facets.stress_vulnerability as f64 / 100.0;
        Mind {
            attributes,
            facets,
            values,
            feeling: Vec::new(),
            stress: Stress {
                load: 0.0,
                // A gloomy temperament sits higher when nothing at all is
                // happening, which is what a propensity *is*.
                baseline: facets.depression_propensity as f64 / 300.0,
                tolerance: 1.0 - 0.5 * vulnerability,
                recovery: 0.02 * (1.5 - vulnerability),
            },
            mood: Mood::default(),
            focus: Focus { current: 0.8, capacity: 0.8 },
        }
    }

    /// What this person holds about a topic.
    pub fn conviction(&self, topic: Value) -> i8 {
        self.values
            .iter()
            .find(|c| c.topic == topic)
            .map(|c| c.held)
            .unwrap_or(0)
    }

    /// **One event, several emotions, and they may disagree.**
    ///
    /// The specification's central mechanism. The same promotion makes an
    /// envious person envious, an ambitious one frustrated and an
    /// affectionate one glad *for* their friend — often all three at
    /// once, in the same head. That is a person with mixed feelings, not
    /// a number that went down by ten.
    ///
    /// Appraisal theory in the ordinary sense *(Lazarus, Scherer)*: the
    /// emotion follows from what the event **means** to this person, and
    /// the questions asked are the standard ones — was I harmed, by whom,
    /// on purpose, could I have stopped it, does it break something I
    /// believe in.
    pub fn appraise(&self, ev: &Appraisal) -> Vec<Felt> {
        let mut out: Vec<Felt> = Vec::new();
        let f = &self.facets;
        let pct = |v: u8| v as f64 / 100.0;

        // How much this lands at all. An event that touches nobody they
        // care about and breaks nothing they believe is barely an event.
        let relevance = (ev.to_me + 0.6 * ev.to_mine).min(1.4);
        let bite = ev.severity.abs()
            * relevance
            * (0.6 + 0.8 * ev.unexpected)
            // **Mood colours an ambiguous event.** A mild event read by
            // an irritable person is not a mild event.
            * (1.0 + 0.4 * self.mood.irritability * (ev.severity < 0.0) as u8 as f64);
        if bite < 0.02 && !ev.nothing_happening {
            return out;
        }

        let mut add = |what: Emotion, strength: f64| {
            if strength > 0.03 {
                out.push(Felt { what, strength: strength.min(1.0), age_days: 0.0 });
            }
        };

        let helpless = 1.0 - ev.control;

        if ev.severity > 0.0 {
            add(Emotion::Joy, bite * (0.4 + 0.6 * pct(f.cheerfulness)));
            if ev.to_me > 0.5 && !ev.deliberate {
                add(Emotion::Pride, bite * (0.3 + 0.7 * pct(f.pride)));
                add(Emotion::Satisfaction, bite * 0.6);
            }
            // **Somebody did you a kindness**, which is a different thing
            // from a good day.
            if ev.deliberate && !ev.my_doing {
                add(Emotion::Gratitude, bite * (0.3 + 0.7 * pct(f.gratitude)));
                add(Emotion::Affection, bite * 0.5 * pct(f.trust));
            }
        } else if ev.severity < 0.0 {
            // Harm with an author is anger; harm without one is grief or
            // fear depending on whether it is over.
            if ev.deliberate && !ev.my_doing {
                add(Emotion::Anger, bite * (0.3 + 0.9 * pct(f.anger)));
                // **Resentment is anger you could do nothing about**, and
                // it is what a vengeful person keeps.
                add(
                    Emotion::Resentment,
                    bite * helpless * (0.2 + 0.8 * pct(f.vengefulness)),
                );
            }
            if ev.my_doing {
                add(Emotion::Guilt, bite * (0.3 + 0.7 * pct(f.dutifulness)));
                add(Emotion::Shame, bite * pct(f.pride) * 0.7);
            }
            add(Emotion::Grief, bite * ev.to_mine * 0.9);
            add(
                Emotion::Fear,
                bite * helpless * (0.2 + 0.8 * pct(f.anxiety)) * ev.to_me,
            );
            add(
                Emotion::Anxiety,
                bite * helpless * (0.2 + 0.8 * pct(f.anxiety)) * 0.7,
            );
        }

        // **A value broken is outrage, and it does not need to touch
        // you.** This is what makes somebody care about a stranger's
        // treatment, and it is the difference between a person and a
        // utility function over their own outcomes.
        if let Some((topic, upheld)) = ev.touches {
            let held = self.conviction(topic) as f64 / 50.0;
            if held > 0.0 {
                let force = held * ev.severity.abs().max(0.3) * (0.5 + 0.5 * pct(f.tolerance).recip().min(2.0));
                if upheld {
                    add(Emotion::Admiration, force * 0.6);
                } else {
                    add(Emotion::Outrage, force * 0.9);
                }
            }
        }

        // **Somebody else got what you wanted.** Envy and frustration are
        // different feelings about the same fact, and which one somebody
        // has says a great deal about them.
        if ev.someone_gained {
            add(Emotion::Envy, (0.2 + 0.9 * pct(f.envy)) * relevance.min(1.0));
            add(
                Emotion::Frustration,
                (0.1 + 0.8 * pct(f.ambition)) * relevance.min(1.0),
            );
            add(Emotion::Discouragement, (0.5 - pct(f.pride)).max(0.0));
        }
        if ev.blocks_a_goal {
            add(Emotion::Frustration, bite.max(0.3) * (0.3 + 0.7 * pct(f.ambition)));
            add(
                Emotion::Discouragement,
                bite.max(0.2) * pct(f.depression_propensity),
            );
        }
        if ev.nothing_happening {
            add(
                Emotion::Boredom,
                (0.2 + 0.8 * pct(f.excitement_seeking)) * (0.3 + 0.7 * pct(f.curiosity)),
            );
        }

        // Deterministic: strongest first, then by name, so a seed
        // reproduces the same head.
        out.sort_by(|a, b| {
            b.strength
                .total_cmp(&a.strength)
                .then(a.what.cmp(&b.what))
        });
        out
    }

    /// Feel something, and let it move stress and mood.
    ///
    /// **Three different things, updated three different ways** — the
    /// specification is emphatic about it and it is the structural claim
    /// that most separates this from a happiness bar.
    pub fn feel(&mut self, felt: Vec<Felt>) {
        for e in &felt {
            // Stress takes the unpleasant ones, weighted by how much this
            // person is hurt by them at all.
            let v = e.what.valence();
            let vuln = self.facets.stress_vulnerability as f64 / 100.0;
            if v < 0.0 {
                self.stress.load += -v * e.strength * (0.4 + 1.2 * vuln) * 0.15;
            } else {
                self.stress.load -= v * e.strength * 0.08;
            }
            // Mood is a slow average of what has been felt lately.
            self.mood.valence = 0.9 * self.mood.valence + 0.1 * v * e.strength;
            self.mood.arousal = 0.9 * self.mood.arousal + 0.1 * e.what.arousal() * e.strength;
            if matches!(e.what, Emotion::Anger | Emotion::Resentment | Emotion::Outrage) {
                self.mood.irritability = (self.mood.irritability + 0.15 * e.strength).min(1.0);
            }
            if matches!(e.what, Emotion::Fear | Emotion::Anxiety) {
                self.mood.anxiety_bias = (self.mood.anxiety_bias + 0.15 * e.strength).min(1.0);
            }
        }
        self.stress.load = self.stress.load.max(0.0);
        self.feeling.extend(felt);
    }

    /// A day passes: emotions fade, mood drifts back, stress drains
    /// slowly, focus follows from what is left.
    pub fn a_day_passes(&mut self) {
        for e in self.feeling.iter_mut() {
            e.age_days += 1.0;
            // **Arousal is what fades**, and it fades fast; grief is low
            // arousal and lasts, rage is high arousal and does not. That
            // is why somebody is still grieving a year later and nobody
            // is still furious.
            let half_life = 1.0 + 12.0 * (1.0 - e.what.arousal());
            e.strength *= 0.5f64.powf(1.0 / half_life);
        }
        self.feeling.retain(|e| e.strength > 0.02);

        self.mood.valence *= 0.93;
        self.mood.arousal *= 0.93;
        self.mood.irritability *= 0.90;
        self.mood.anxiety_bias *= 0.90;

        // Stress drains toward temperament, never to nothing.
        let toward = self.stress.baseline;
        self.stress.load += (toward - self.stress.load) * self.stress.recovery;
        self.stress.load = self.stress.load.max(0.0);

        self.recompute_focus();
    }

    /// **Focus is not the opposite of stress.**
    ///
    /// What takes focus away is *arousal* — being stirred up — and
    /// unmet need, which arrives in slice 4. Load on its own does not:
    /// a grieving parent nursing a sick child is carrying an enormous
    /// amount and concentrating completely, and that is the case this
    /// separation exists to allow.
    fn recompute_focus(&mut self) {
        let stirred: f64 = self
            .feeling
            .iter()
            .map(|e| e.what.arousal() * e.strength)
            .sum::<f64>()
            .min(1.0);
        let willed = self.attributes.willpower as f64 / 100.0;
        // Willpower buys back some of what agitation costs — which is
        // what being able to work through something *is*.
        let cost = stirred * (1.0 - 0.5 * willed);
        self.focus.current = (self.focus.capacity - cost).clamp(0.0, 1.0);
    }

    /// How strongly they are feeling a given emotion right now.
    pub fn feeling_of(&self, what: Emotion) -> f64 {
        self.feeling
            .iter()
            .filter(|e| e.what == what)
            .map(|e| e.strength)
            .fold(0.0, f64::max)
    }

    /// Whether the load has gone past what they can carry. **Not a
    /// tantrum trigger** — slice 8 decides what somebody does about it,
    /// from their personality, and it will not be one table.
    pub fn overloaded(&self) -> bool {
        self.stress.load > self.stress.tolerance
    }
}
