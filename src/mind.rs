//! **A person is not one happiness number.**
//!
//! Slice 1 of `docs/mind-spec.md`: the static mind, an appraisal that
//! produces *several* emotions from one event, and the separation of
//! stress, mood and focus.
//!
//! ## What this is, precisely
//!
//! **A measured Big Five substrate expressed through behavioural
//! facets.** Not "the Big Five", and not a clone of a game's tables. The
//! distinction earns its keep immediately: thirty independently drawn
//! sliders **are not** a five-factor model, because the whole content of
//! the model is that facets *covary through their parent domain*. Anger,
//! anxiety, gloom and vulnerability to stress are not four coin flips;
//! they are four expressions of one thing.
//!
//! ```text
//! domain z ~ N(0, 1)
//! facet z  = loading × domain + √(1 − loading²) × residual
//! ```
//!
//! Facet loadings on their parent domain run around **0.5–0.75**, which
//! leaves each facet substantial variance of its own — so a person can be
//! an anxious *and* even-tempered neurotic, which is a real kind of
//! person.
//!
//! **Where each loading comes from is recorded, because they do not all
//! come from the same place.** NEO-PI-R has **thirty** facets, six to a
//! domain; the list here is a deliberate subset plus extensions this
//! simulation needs. Cruelty, violence, vengefulness and greed are *not*
//! NEO facets — their negative agreeableness loadings are sensible
//! modelled mappings and nothing more. And published loadings are
//! estimates from particular samples and models, not constants: real NEO
//! analyses find useful secondary cross-loadings, and exploratory
//! structural models fit better than a perfectly clean one-facet-one-
//! domain structure *(Furnham et al.)*, which this deliberately is.
//!
//! `Facet::provenance` carries that, so a later document cannot present a
//! designed mapping as a measured coefficient.
//!
//! ## Latent inside, 0–100 at the edges
//!
//! Personality is stored as a **z-score** and 0–100 is a presentation
//! scale. Storing the bounded score was what let a test assert a game's
//! neutral band instead of a measured statistic.
//!
//! | | |
//! |---|---|
//! | within \|z\| ≤ 1 | **68.3%** |
//! | within \|z\| ≤ 2 | **95.4%** |
//! | **extreme**, defined as \|z\| > 2 | **4.6%** |
//!
//! "Extreme" is *defined*, not eyeballed; "under 6%" is not reproducible.
//!
//! ## Four things that change, and only one of them is the person
//!
//! ```text
//! expressed(t) = developmental baseline
//!              + age trajectory(t)
//!              + durable adaptation(t)
//!              + temporary state
//! ```
//!
//! A core memory moves **durable adaptation**. It does not rewrite the
//! baseline — that is who somebody grew up to be, and it is not editable
//! by one bad afternoon.

use crate::rng::Rng;
use crate::social::{Content, Delivery};
use crate::witness::Cues;

// ---------------------------------------------------------------------
// the five factors, and the facets that hang off them
// ---------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Domain {
    Neuroticism,
    Extraversion,
    Agreeableness,
    Conscientiousness,
    Openness,
}

impl Domain {
    pub const ALL: [Domain; 5] = [
        Domain::Neuroticism,
        Domain::Extraversion,
        Domain::Agreeableness,
        Domain::Conscientiousness,
        Domain::Openness,
    ];
    fn index(self) -> usize {
        match self {
            Domain::Neuroticism => 0,
            Domain::Extraversion => 1,
            Domain::Agreeableness => 2,
            Domain::Conscientiousness => 3,
            Domain::Openness => 4,
        }
    }

    /// **The maturity principle, as a population tendency in z per
    /// decade.** Conscientiousness and agreeableness rise across
    /// adulthood, neuroticism falls.
    ///
    /// Deliberately weak, and deliberately not compulsory: a coordinated
    /// analysis of sixteen longitudinal samples *(Graham et al.)* found
    /// substantial heterogeneity, flattening, and in several traits
    /// late-life reversals including **rising** neuroticism. Every person
    /// also carries a slope of their own, so a good share move against
    /// the average. A rule every actor obeys is not a tendency.
    fn drift_per_decade(self) -> f32 {
        match self {
            Domain::Conscientiousness => 0.10,
            Domain::Agreeableness => 0.08,
            Domain::Neuroticism => -0.08,
            Domain::Extraversion => -0.01,
            Domain::Openness => -0.03,
        }
    }
}

/// **Where a loading came from.** No behaviour depends on it; it exists
/// so that a designed mapping is never mistaken for a measured one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LoadingSource {
    /// A NEO-PI-R facet, loading in the range those analyses report.
    MeasuredNeo,
    /// Not a NEO facet, but a close stand-in for one that is.
    EmpiricalProxy,
    /// This simulation's own, mapped onto a domain because the mapping is
    /// sensible — not because anybody measured it.
    DesignedExtension,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Facet {
    // neuroticism
    Anxiety,
    Anger,
    Gloom,
    StressVulnerability,
    Envy,
    // extraversion
    Gregariousness,
    Assertiveness,
    ExcitementSeeking,
    Cheerfulness,
    Privacy,
    Pride,
    // agreeableness
    Trust,
    Altruism,
    Tolerance,
    Gratitude,
    Cruelty,
    Violence,
    Vengefulness,
    Greed,
    // conscientiousness
    Dutifulness,
    Perseverance,
    Orderliness,
    Ambition,
    // openness
    Curiosity,
    LoveOfMaking,
}

pub const FACETS: usize = 25;

impl Facet {
    pub const ALL: [Facet; FACETS] = [
        Facet::Anxiety,
        Facet::Anger,
        Facet::Gloom,
        Facet::StressVulnerability,
        Facet::Envy,
        Facet::Gregariousness,
        Facet::Assertiveness,
        Facet::ExcitementSeeking,
        Facet::Cheerfulness,
        Facet::Privacy,
        Facet::Pride,
        Facet::Trust,
        Facet::Altruism,
        Facet::Tolerance,
        Facet::Gratitude,
        Facet::Cruelty,
        Facet::Violence,
        Facet::Vengefulness,
        Facet::Greed,
        Facet::Dutifulness,
        Facet::Perseverance,
        Facet::Orderliness,
        Facet::Ambition,
        Facet::Curiosity,
        Facet::LoveOfMaking,
    ];

    fn index(self) -> usize {
        Facet::ALL.iter().position(|f| *f == self).unwrap()
    }

    /// **Which factor it is an expression of, and how strongly.**
    ///
    /// A negative loading is not a curiosity: cruelty, violence,
    /// vengefulness and greed are all *low* agreeableness, and privacy is
    /// low extraversion. That is what makes them covary the right way
    /// round without anybody wiring it by hand.
    pub fn parent(self) -> (Domain, f32) {
        use Domain::*;
        use Facet::*;
        match self {
            Anxiety => (Neuroticism, 0.75),
            Anger => (Neuroticism, 0.60),
            Gloom => (Neuroticism, 0.75),
            StressVulnerability => (Neuroticism, 0.70),
            Envy => (Neuroticism, 0.55),
            Gregariousness => (Extraversion, 0.75),
            Assertiveness => (Extraversion, 0.65),
            ExcitementSeeking => (Extraversion, 0.60),
            Cheerfulness => (Extraversion, 0.70),
            Privacy => (Extraversion, -0.55),
            Pride => (Extraversion, 0.45),
            Trust => (Agreeableness, 0.65),
            Altruism => (Agreeableness, 0.70),
            Tolerance => (Agreeableness, 0.60),
            Gratitude => (Agreeableness, 0.55),
            Cruelty => (Agreeableness, -0.70),
            Violence => (Agreeableness, -0.55),
            Vengefulness => (Agreeableness, -0.60),
            Greed => (Agreeableness, -0.45),
            Dutifulness => (Conscientiousness, 0.70),
            Perseverance => (Conscientiousness, 0.75),
            Orderliness => (Conscientiousness, 0.65),
            Ambition => (Conscientiousness, 0.60),
            Curiosity => (Openness, 0.70),
            LoveOfMaking => (Openness, 0.70),
        }
    }

    /// See [`LoadingSource`]. Anxiety, anger, gloom, vulnerability,
    /// gregariousness, assertiveness, excitement-seeking, cheerfulness,
    /// trust, altruism, dutifulness, orderliness and the two openness
    /// facets are NEO facets or close to them. Envy, pride, tolerance,
    /// gratitude, privacy, perseverance and ambition stand in for ones
    /// that are. The remaining four are this simulation's own.
    pub fn provenance(self) -> LoadingSource {
        use Facet::*;
        use LoadingSource::*;
        match self {
            Anxiety | Anger | Gloom | StressVulnerability => MeasuredNeo,
            Gregariousness | Assertiveness | ExcitementSeeking | Cheerfulness => MeasuredNeo,
            Trust | Altruism | Dutifulness | Orderliness => MeasuredNeo,
            Curiosity | LoveOfMaking => MeasuredNeo,
            Envy | Pride | Tolerance | Gratitude | Privacy => EmpiricalProxy,
            Perseverance | Ambition => EmpiricalProxy,
            Cruelty | Violence | Vengefulness | Greed => DesignedExtension,
        }
    }

    pub fn name(self) -> &'static str {
        use Facet::*;
        match self {
            Anxiety => "anxiety",
            Anger => "anger",
            Gloom => "gloom",
            StressVulnerability => "vulnerability to stress",
            Envy => "envy",
            Gregariousness => "gregariousness",
            Assertiveness => "assertiveness",
            ExcitementSeeking => "excitement-seeking",
            Cheerfulness => "cheerfulness",
            Privacy => "privacy",
            Pride => "pride",
            Trust => "trust",
            Altruism => "altruism",
            Tolerance => "tolerance",
            Gratitude => "gratitude",
            Cruelty => "cruelty",
            Violence => "violence",
            Vengefulness => "vengefulness",
            Greed => "greed",
            Dutifulness => "dutifulness",
            Perseverance => "perseverance",
            Orderliness => "orderliness",
            Ambition => "ambition",
            Curiosity => "curiosity",
            LoveOfMaking => "love of making things",
        }
    }
}

/// **What somebody grew up to be, plus what life has done since.**
///
/// The four components are kept apart because they change on entirely
/// different timescales and for entirely different reasons — and because
/// a core memory is allowed to move exactly one of them.
#[derive(Clone, Debug, PartialEq)]
pub struct Personality {
    /// Who they grew up to be, in z. Never edited after the person is
    /// made.
    baseline: [f32; FACETS],
    /// **The genetic part of the baseline**, kept so offspring can
    /// inherit a breeding value rather than a phenotype.
    genetic: [f32; FACETS],
    /// Each person's own slope, in z per decade, so some move against the
    /// population tendency.
    slope: [f32; 5],
    /// What life has durably done. This is what a core memory moves.
    pub adaptation: [f32; FACETS],
    /// Years lived, for the age trajectory.
    pub age_years: f32,
}

/// **Heritability is a population variance ratio, not a share of one
/// person.**
///
/// Twin estimates for the five domains run about **41–61%** *(Jang et
/// al.)* and variant-based estimates are lower. What that emphatically
/// does not license is `personality = 0.5 × parents + 0.5 ×
/// environment`, which is a claim about an individual and is
/// meaningless. What it licenses is a **breeding value**: mid-parent plus
/// segregation noise, a developmental residual on top, and the check is a
/// *population* correlation between relatives.
///
/// **Which heritability is being modelled has to be pinned down, because
/// the two claims are the same claim.** This is an *additive* model, so
/// `r(parent, child) ≈ h²/2` — and asserting h² of 0.40–0.60 while also
/// requiring relatives to correlate at 0.15–0.20 asks for two different
/// numbers at once. Higher twin estimates can carry non-additive genetic
/// effects and design differences that a breeding value does not
/// represent.
///
/// So: **additive heritability 0.40, expected parent–offspring ≈ 0.20.**
/// A multimethod family study found parent–offspring and sibling
/// correlations near **0.20** with narrow-sense heritability around
/// **40%**, while ordinary single-method estimates come in at 0.15 or
/// below *(Mõttus et al.)*. Trait-specific values can come later.
const HERITABILITY: f32 = 0.40;

/// **How well a personality can be measured at all.** Good inventories
/// report internal consistency around 0.80, and that ceiling is why
/// observed stability never reaches true stability.
pub const RELIABILITY: f32 = 0.80;

/// A standard normal, from the project's own generator — no `rand`.
fn gauss(rng: &mut Rng) -> f32 {
    // Box-Muller. The guard keeps the log finite.
    let u1 = (rng.next_f32()).max(1e-7);
    let u2 = rng.next_f32();
    (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos()
}

impl Personality {
    /// Draw somebody from the population.
    pub fn draw(rng: &mut Rng) -> Self {
        let domains: [f32; 5] = std::array::from_fn(|_| gauss(rng));
        Self::from_domains(rng, domains)
    }

    fn from_domains(rng: &mut Rng, domains: [f32; 5]) -> Self {
        let h = HERITABILITY.sqrt();
        let mut baseline = [0.0f32; FACETS];
        let mut genetic = [0.0f32; FACETS];
        for f in Facet::ALL {
            let (d, loading) = f.parent();
            // **The covariance is the model.** A facet is its domain
            // times its loading, plus what is specific to it — which is
            // what makes anger and anxiety correlate without either being
            // a copy of the other.
            let specific = (1.0 - loading * loading).max(0.0).sqrt() * gauss(rng);
            let z = loading * domains[d.index()] + specific;
            baseline[f.index()] = z;
            // Split that phenotype into the part that can be passed on
            // and the part that cannot.
            genetic[f.index()] = h * z;
        }
        Personality {
            baseline,
            genetic,
            slope: std::array::from_fn(|_| gauss(rng) * 0.12),
            adaptation: [0.0; FACETS],
            age_years: 30.0,
        }
    }

    /// **A child of two parents**, by breeding value.
    ///
    /// Mid-parent genetic value plus segregation noise, then a
    /// developmental residual on top. Nothing here claims a percentage of
    /// any individual came from anywhere; what it produces is a
    /// population in which relatives correlate at about the right rate.
    pub fn inherit(rng: &mut Rng, a: &Personality, b: &Personality) -> Self {
        let h = HERITABILITY.sqrt();
        let mut baseline = [0.0f32; FACETS];
        let mut genetic = [0.0f32; FACETS];
        for f in Facet::ALL {
            let i = f.index();
            // Half of each parent's breeding value. The remaining genetic
            // variance is segregation — which is why siblings differ.
            let mid = 0.5 * (a.genetic[i] + b.genetic[i]);
            let seg = (0.5f32).sqrt() * h * gauss(rng);
            genetic[i] = mid + seg;
            // And everything that is not inherited.
            let env = (1.0 - HERITABILITY).max(0.0).sqrt() * gauss(rng);
            baseline[i] = genetic[i] + env;
        }
        Personality {
            baseline,
            genetic,
            slope: std::array::from_fn(|_| gauss(rng) * 0.12),
            adaptation: [0.0; FACETS],
            age_years: 0.0,
        }
    }

    /// **The facet as it is expressed now**, in z.
    pub fn z(&self, f: Facet) -> f32 {
        let i = f.index();
        let (d, loading) = f.parent();
        let decades = (self.age_years - 30.0) / 10.0;
        // The population tendency reaches a facet through its domain and
        // is scaled by how much of that facet the domain accounts for —
        // so a facet only weakly tied to its factor drifts only weakly.
        let maturity = d.drift_per_decade() * decades * loading;
        let personal = self.slope[d.index()] * decades * loading;
        self.baseline[i] + maturity + personal + self.adaptation[i]
    }

    /// **What a questionnaire would say**, which is not what is true.
    ///
    /// Standardised, so the observation has the same variance as the
    /// trait and the arithmetic is exact:
    ///
    /// ```text
    /// observed = √R × latent + √(1 − R) × noise
    /// ```
    ///
    /// Two independent observations of an unchanged person then correlate
    /// at exactly **R**, and an observation correlates with the truth at
    /// **√R**. Adding raw noise to the latent score instead — which is
    /// what this did first — inflates the variance and gives 0.83 where
    /// 0.80 was wanted. For an interval, `r_observed = r_latent × √(R₁R₂)`,
    /// so at R = 0.80 a true stability of 0.85 shows up as 0.68.
    ///
    /// **Only for calibration and for anything that reports a score.** A
    /// person deciding what to do uses their expressed personality, not a
    /// noisy questionnaire about themselves.
    pub fn observed(&self, f: Facet, rng: &mut Rng) -> f32 {
        RELIABILITY.sqrt() * self.z(f) + (1.0 - RELIABILITY).sqrt() * gauss(rng)
    }

    /// **0–100 for showing somebody**, never for storing. A z of 0 is 50.
    pub fn score(&self, f: Facet) -> u8 {
        (50.0 + self.z(f) * 16.67).round().clamp(0.0, 100.0) as u8
    }

    /// Push a facet durably, the way a core memory is allowed to.
    ///
    /// **Bounded**: life bends people, it does not replace them, and this
    /// clamp is the whole of what enforces it — nothing else limits how
    /// often a memory may push. It is set so that latent rank-order
    /// stability over twenty years lands near **0.85**, which is *not*
    /// the 0.6–0.7 usually quoted: those are **observed** test-retest
    /// correlations carrying measurement error. Observed = true ×
    /// reliability, so 0.85 × 0.80 ≈ 0.68, which is where the literature
    /// sits. Comparing a latent trait against an observed coefficient
    /// makes people far less stable than they are.
    pub fn adapt(&mut self, f: Facet, by: f32) {
        let i = f.index();
        self.adaptation[i] = (self.adaptation[i] + by).clamp(-1.5, 1.5);
    }

    /// Years pass. Only the trajectory moves; the baseline is who they
    /// grew up to be.
    pub fn a_year_passes(&mut self) {
        self.age_years += 1.0;
    }

    /// Let a test build somebody specific without fighting the draw.
    pub fn set_baseline(&mut self, f: Facet, z: f32) {
        self.baseline[f.index()] = z;
    }
}

// ---------------------------------------------------------------------
// values
// ---------------------------------------------------------------------

/// **What somebody holds to be admirable or proper.** A facet is how they
/// react; a value is what they believe. Collapse them and a hot-tempered
/// pacifist cannot be expressed.
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Conviction {
    pub topic: Value,
    /// -50 (contempt) to +50 (reverence).
    pub held: i8,
    /// What the culture around them holds, so the *distance* can be
    /// measured.
    pub cultural: i8,
}

impl Conviction {
    pub fn heterodoxy(self) -> i16 {
        (self.held as i16 - self.cultural as i16).abs()
    }
}

// ---------------------------------------------------------------------
// emotion: episodes and the concerns that keep producing them
// ---------------------------------------------------------------------

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
    Yearning,
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
    pub fn valence(self) -> f64 {
        use Emotion::*;
        match self {
            Joy | Pride | Satisfaction => 1.0,
            Gratitude | Affection | Relief => 0.8,
            Admiration | Hope => 0.6,
            Boredom => -0.2,
            Frustration | Discouragement | Yearning => -0.5,
            Anxiety | Loneliness | Envy => -0.6,
            Fear | Anger | Guilt => -0.8,
            Resentment | Outrage | Shame => -0.8,
            Grief | Humiliation => -1.0,
        }
    }

    /// **How much it stirs the body up.** This is now only about
    /// activation — it no longer decides how long anything lasts.
    pub fn activation(self) -> f64 {
        use Emotion::*;
        match self {
            Anger | Outrage | Fear => 1.0,
            Humiliation | Joy => 0.8,
            Anxiety | Envy | Frustration => 0.7,
            Pride | Resentment | Shame => 0.5,
            Gratitude | Affection | Admiration => 0.4,
            Hope | Guilt | Relief | Yearning => 0.3,
            Satisfaction | Loneliness => 0.2,
            Grief | Discouragement => 0.2,
            Boredom => 0.0,
        }
    }

    pub fn name(self) -> &'static str {
        use Emotion::*;
        match self {
            Joy => "joy",
            Pride => "pride",
            Satisfaction => "satisfaction",
            Gratitude => "gratitude",
            Affection => "affection",
            Relief => "relief",
            Admiration => "admiration",
            Hope => "hope",
            Fear => "fear",
            Anxiety => "anxiety",
            Anger => "anger",
            Resentment => "resentment",
            Grief => "grief",
            Yearning => "yearning",
            Guilt => "guilt",
            Shame => "shame",
            Envy => "envy",
            Outrage => "outrage",
            Frustration => "frustration",
            Discouragement => "discouragement",
            Humiliation => "humiliation",
            Loneliness => "loneliness",
            Boredom => "boredom",
        }
    }
}

/// **What an emotion is about, and why it comes back.**
///
/// The correction that matters most in this module. Treating duration as
/// a property of arousal made grief one uninterrupted year-long sadness,
/// which is not what grief is. Grief is a *persistent concern* — an
/// attachment, a future and a role all lost — that throws off repeated
/// waves of sadness, yearning, anger, relief and guilt. Rage is the
/// mirror image: the activation is gone within the hour and the
/// **grievance** can sit unresolved for years and produce fresh rage
/// every time it is touched.
///
/// Emotion-duration research bears this out: what lengthened an emotion
/// was its **importance**, its initial intensity, and the eliciting
/// situation *reappearing* — physically or in thought *(Verduyn et al.)*.
/// Arousal alone is not enough to carry it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConcernKind {
    Bereavement,
    Grievance,
    Threat,
    BlockedGoal,
}

impl ConcernKind {
    fn first_feeling(self) -> Emotion {
        match self {
            ConcernKind::Bereavement => Emotion::Grief,
            ConcernKind::Grievance => Emotion::Anger,
            ConcernKind::Threat => Emotion::Anxiety,
            ConcernKind::BlockedGoal => Emotion::Frustration,
        }
    }
}

/// **A concern is not permanently loud.**
///
/// Without this an elderly person accumulates decades of nonzero
/// bereavements, grievances and failed goals, and carries all of them
/// every day for ever. What actually happens is that the attachment and
/// the memory stay while the *daily burden* goes: a dormant loss costs
/// almost nothing on an ordinary Tuesday and still produces an episode at
/// an anniversary, a familiar place, or a remembered conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConcernState {
    /// Live: throws off episodes unprompted and weighs every day.
    Active,
    /// Accommodated into a life. Almost no ordinary load; still answers a
    /// cue.
    Dormant,
    /// Settled — the grievance was answered, the goal abandoned, the loss
    /// made sense of. A cue may still touch it, faintly.
    Resolved,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Concern {
    pub kind: ConcernKind,
    pub state: ConcernState,
    /// How much it matters. The strongest predictor of how long an
    /// emotion runs.
    pub importance: f64,
    /// How much of it is still open. A grievance that has been answered,
    /// or a loss that has been made sense of, stops throwing off waves.
    pub unresolvedness: f64,
    /// Habituation: the same concern hurts less on the hundredth
    /// occasion than the first, without ceasing to be there.
    pub adaptation: f64,
    pub age_days: f64,
}

impl Concern {
    /// **What it costs on an ordinary day.** Nearly nothing once it has
    /// been accommodated — which is what accommodation *is*.
    pub fn pressure(&self) -> f64 {
        self.depth()
            * match self.state {
                ConcernState::Active => 1.0,
                ConcernState::Dormant => 0.06,
                ConcernState::Resolved => 0.0,
            }
    }

    /// **What is still there to be touched.** Unchanged by dormancy: the
    /// attachment and the memory do not go, only the daily burden does.
    /// This is what a cue reaches.
    pub fn depth(&self) -> f64 {
        (self.importance * self.unresolvedness * (1.0 - self.adaptation)).max(0.0)
    }
}

/// One burst of feeling: a valence, an activation, and what it is about.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Episode {
    pub what: Emotion,
    pub strength: f64,
    /// Bodily stirring-up. **This** is what fades quickly, and what takes
    /// attention while it lasts.
    pub activation: f64,
    pub age_days: f64,
    /// Which standing concern threw it off, if any.
    pub about: Option<usize>,
}

// ---------------------------------------------------------------------
// appraisal
// ---------------------------------------------------------------------

/// **What happened.** The facts, as far as anybody has them — not what
/// they mean.
///
/// The boundary matters before slice 2 rather than after it. If the event
/// itself carries `unfair`, then everybody who hears about it inherits
/// the same moral conclusion, and there is no room for two witnesses to
/// disagree about a promotion, for a rumour to be wrong, or for the
/// person who made the decision to think it was perfectly proper.
///
/// ```text
/// Happening:   the manager chose Alice; Bob was also a candidate
/// Bob reads:   unfair 0.82, confirms a fear 0.61
/// Carol reads: unfair 0.05, confirms a fear 0.00
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Happening {
    pub severity: f64,
    pub to_me: f64,
    pub to_mine: f64,
    pub deliberate: bool,
    pub my_doing: bool,
    pub control: f64,
    pub unexpected: f64,
    /// Which value it bears on, and whether it upheld or broke it. Still
    /// a fact about the act; what it is *worth* is the reader's.
    pub bears_on: Option<(Value, bool)>,
    pub someone_gained: bool,
    pub blocks_a_goal: bool,
    pub nothing_happening: bool,
    /// **Somebody decided this**, so there is a process to have an
    /// opinion about. An accident has no fairness.
    pub by_a_decision: bool,
}

impl Default for Happening {
    fn default() -> Self {
        Happening {
            severity: 0.0,
            to_me: 0.0,
            to_mine: 0.0,
            deliberate: false,
            my_doing: false,
            control: 0.5,
            unexpected: 0.3,
            bears_on: None,
            someone_gained: false,
            blocks_a_goal: false,
            nothing_happening: false,
            by_a_decision: false,
        }
    }
}

/// **What one person made of it.** Produced by [`Mind::read`], never
/// shared: two people reading the same happening produce two of these.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Appraisal {
    pub severity: f64,
    pub to_me: f64,
    pub to_mine: f64,
    pub deliberate: bool,
    pub my_doing: bool,
    pub control: f64,
    pub unexpected: f64,
    pub touches: Option<(Value, bool)>,
    pub someone_gained: bool,
    /// **How crooked they think the process was**, which is a different
    /// complaint from losing, and is theirs and not the event's. Somebody
    /// with no stake and no strong feeling about fairness reads almost
    /// none.
    pub unfair: f64,
    /// **How much it confirms something they already feared about
    /// themselves.** A defeat is only humiliating to somebody who half
    /// expected it.
    pub confirms_a_fear: f64,
    pub blocks_a_goal: bool,
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
            unfair: 0.0,
            confirms_a_fear: 0.0,
            blocks_a_goal: false,
            nothing_happening: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stress {
    pub load: f64,
    pub baseline: f64,
    pub tolerance: f64,
    pub recovery: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Mood {
    pub valence: f64,
    pub arousal: f64,
    pub irritability: f64,
    pub anxiety_bias: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Focus {
    pub current: f64,
    pub capacity: f64,
}

#[derive(Clone, Debug)]
pub struct Mind {
    pub person: Personality,
    pub values: Vec<Conviction>,
    pub episodes: Vec<Episode>,
    pub concerns: Vec<Concern>,
    pub stress: Stress,
    pub mood: Mood,
    pub focus: Focus,
    /// Cognitive capability, which is not disposition. Somebody can read
    /// another's pain perfectly and not care.
    pub willpower: f32,
    pub empathy: f32,
    /// **What they want that is not dinner.** Filled in by `needs.rs`,
    /// which is what finally makes the *content and unfocused* case
    /// reachable: until this existed, nothing but agitation could take
    /// anybody's attention.
    pub needs: Option<crate::needs::Needs>,
}

impl Mind {
    pub fn draw(rng: &mut Rng, culture: &[(Value, i8)]) -> Self {
        let person = Personality::draw(rng);
        let values = Value::ALL
            .iter()
            .map(|&topic| {
                let cultural = culture
                    .iter()
                    .find(|(t, _)| *t == topic)
                    .map(|(_, v)| *v)
                    .unwrap_or(0);
                // **An individual departs from their culture**, or there
                // is no room for a heretic, a reformer, or a criminal who
                // believes they are in the right.
                let drift = gauss(rng) * 18.0;
                Conviction {
                    topic,
                    held: (cultural as f32 + drift).clamp(-50.0, 50.0) as i8,
                    cultural,
                }
            })
            .collect();
        let vuln = unit(person.z(Facet::StressVulnerability));
        let mut m = Mind {
            values,
            episodes: Vec::new(),
            concerns: Vec::new(),
            stress: Stress {
                load: 0.0,
                baseline: unit(person.z(Facet::Gloom)) / 3.0,
                tolerance: 1.0 - 0.5 * vuln,
                recovery: 0.02 * (1.5 - vuln),
            },
            mood: Mood::default(),
            focus: Focus { current: 0.85, capacity: 0.85 },
            willpower: gauss(rng),
            empathy: gauss(rng),
            needs: None,
            person,
        };
        m.needs = Some(crate::needs::Needs::of(&m));
        m
    }

    /// **Read a happening.** This is the perception/appraisal boundary,
    /// and it is where two people looking at one event stop agreeing.
    ///
    /// Nothing here decides what is *true* — the facts arrive already
    /// settled. What it decides is what they are worth to this person:
    /// whether the process was crooked, and whether the outcome confirms
    /// something they already suspected about themselves.
    pub fn read(&self, h: &Happening) -> Appraisal {
        // **A crooked process needs a process, a stake and somebody who
        // cares about fairness.** All three, which is why a bystander
        // reads almost nothing and the loser reads a great deal.
        let cares = (self.conviction(Value::Fairness) as f64 / 50.0).max(0.0);
        let stake = h.to_me.min(1.0);
        let against_me = if h.severity < 0.0 { 1.0 } else { 0.0 };
        let suspicious = 1.0 - self.f(Facet::Trust);
        let unfair = if h.by_a_decision {
            (stake * against_me * (0.35 * cares + 0.45 * cares * suspicious + 0.2 * suspicious))
                .clamp(0.0, 1.0)
        } else {
            0.0
        };
        // **A defeat confirms a fear** in somebody already inclined to
        // think poorly of themselves, and barely registers in somebody
        // who is not.
        let confirms = if h.severity < 0.0 {
            (stake * (0.8 * self.f(Facet::Gloom) + 0.4 * (1.0 - self.f(Facet::Pride)) - 0.2))
                .clamp(0.0, 1.0)
        } else {
            0.0
        };
        Appraisal {
            severity: h.severity,
            to_me: h.to_me,
            to_mine: h.to_mine,
            deliberate: h.deliberate,
            my_doing: h.my_doing,
            control: h.control,
            unexpected: h.unexpected,
            touches: h.bears_on,
            someone_gained: h.someone_gained,
            unfair,
            confirms_a_fear: confirms,
            blocks_a_goal: h.blocks_a_goal,
            nothing_happening: h.nothing_happening,
        }
    }

    pub fn conviction(&self, topic: Value) -> i8 {
        self.values
            .iter()
            .find(|c| c.topic == topic)
            .map(|c| c.held)
            .unwrap_or(0)
    }

    fn f(&self, f: Facet) -> f64 {
        unit(self.person.z(f))
    }

    /// **One event, several emotions, and they may disagree.**
    ///
    /// The trait does not say what somebody is angry *about*; the value
    /// does not guarantee anger. What produces the disposition is their
    /// interaction:
    ///
    /// ```text
    /// emotion = appraisal(event, values, relationships, beliefs)
    ///         × trait susceptibility
    ///         × current vulnerability
    ///         × regulation
    /// ```
    ///
    /// Appraisal is the bridge, which is why neither layer alone decides
    /// anything.
    pub fn appraise(&self, ev: &Appraisal) -> Vec<Episode> {
        let mut out: Vec<Episode> = Vec::new();
        let pct = |f: Facet| self.f(f);

        let relevance = (ev.to_me + 0.6 * ev.to_mine).min(1.4);
        // **Current vulnerability**: the same remark lands differently on
        // somebody already carrying a load, and an irritable mood makes
        // an ambiguous event an unkind one.
        let vulnerable = 1.0 + 0.4 * self.mood.irritability * (ev.severity < 0.0) as u8 as f64
            + 0.3 * (self.stress.load / self.stress.tolerance.max(0.1)).min(1.0);
        // **Regulation**: willpower does not stop somebody feeling it, it
        // damps what comes out.
        let regulation = 1.0 - 0.25 * unit(self.willpower);
        let bite =
            ev.severity.abs() * relevance * (0.6 + 0.8 * ev.unexpected) * vulnerable * regulation;
        if bite < 0.02 && !ev.nothing_happening && !ev.someone_gained {
            return out;
        }

        let mut add = |what: Emotion, strength: f64| {
            if strength > 0.03 {
                out.push(Episode {
                    what,
                    strength: strength.min(1.0),
                    activation: what.activation(),
                    age_days: 0.0,
                    about: None,
                });
            }
        };
        let helpless = 1.0 - ev.control;

        if ev.severity > 0.0 {
            add(Emotion::Joy, bite * (0.4 + 0.6 * pct(Facet::Cheerfulness)));
            if ev.to_me > 0.5 && !ev.deliberate {
                add(Emotion::Pride, bite * (0.3 + 0.7 * pct(Facet::Pride)));
                add(Emotion::Satisfaction, bite * 0.6);
            }
            if ev.deliberate && !ev.my_doing {
                add(Emotion::Gratitude, bite * (0.3 + 0.7 * pct(Facet::Gratitude)));
                add(Emotion::Affection, bite * 0.5 * pct(Facet::Trust));
            }
        } else if ev.severity < 0.0 {
            if ev.deliberate && !ev.my_doing {
                add(Emotion::Anger, bite * (0.3 + 0.9 * pct(Facet::Anger)));
                add(
                    Emotion::Resentment,
                    bite * helpless * (0.2 + 0.8 * pct(Facet::Vengefulness)),
                );
            }
            if ev.my_doing {
                add(Emotion::Guilt, bite * (0.3 + 0.7 * pct(Facet::Dutifulness)));
                add(Emotion::Shame, bite * pct(Facet::Pride) * 0.7);
            }
            add(Emotion::Grief, bite * ev.to_mine * 0.9);
            add(
                Emotion::Fear,
                bite * helpless * (0.2 + 0.8 * pct(Facet::Anxiety)) * ev.to_me,
            );
            add(
                Emotion::Anxiety,
                bite * helpless * (0.2 + 0.8 * pct(Facet::Anxiety)) * 0.7,
            );
        }

        if let Some((topic, upheld)) = ev.touches {
            let held = self.conviction(topic) as f64 / 50.0;
            if held > 0.0 {
                let force = held * ev.severity.abs().max(0.3);
                if upheld {
                    add(Emotion::Admiration, force * 0.6);
                } else {
                    // **Intolerance sharpens outrage**, which is why the
                    // loading on tolerance is negative here.
                    add(Emotion::Outrage, force * (0.6 + 0.6 * (1.0 - pct(Facet::Tolerance))));
                }
            }
        }

        if ev.someone_gained {
            let r = relevance.max(0.4).min(1.0);
            add(Emotion::Envy, (0.15 + 0.9 * pct(Facet::Envy)) * r);
            add(Emotion::Frustration, (0.1 + 0.8 * pct(Facet::Ambition)) * r);
            add(Emotion::Discouragement, (0.5 - pct(Facet::Pride)).max(0.0) * r);
            // **Gladness for a friend, in the same head as the envy.**
            add(Emotion::Joy, 0.5 * pct(Facet::Altruism) * ev.to_mine);
        }
        if ev.unfair > 0.01 {
            let cares = (self.conviction(Value::Fairness) as f64 / 50.0).max(0.0);
            add(Emotion::Outrage, ev.unfair * (0.3 + 0.7 * cares));
            add(Emotion::Resentment, ev.unfair * (0.2 + 0.8 * pct(Facet::Vengefulness)));
        }
        if ev.confirms_a_fear > 0.01 {
            add(Emotion::Shame, ev.confirms_a_fear * (0.3 + 0.7 * pct(Facet::Gloom)));
            add(
                Emotion::Discouragement,
                ev.confirms_a_fear * (0.3 + 0.7 * pct(Facet::Gloom)),
            );
        }
        if ev.blocks_a_goal {
            add(Emotion::Frustration, bite.max(0.3) * (0.3 + 0.7 * pct(Facet::Ambition)));
            add(Emotion::Discouragement, bite.max(0.2) * pct(Facet::Gloom));
        }
        if ev.nothing_happening {
            add(
                Emotion::Boredom,
                (0.2 + 0.8 * pct(Facet::ExcitementSeeking)) * (0.3 + 0.7 * pct(Facet::Curiosity)),
            );
        }

        out.sort_by(|a, b| b.strength.total_cmp(&a.strength).then(a.what.cmp(&b.what)));
        out
    }

    /// Take on a standing concern — a loss, a grievance, a threat, a
    /// blocked goal. It is this, not the emotion, that lasts.
    pub fn take_on(&mut self, kind: ConcernKind, importance: f64) -> usize {
        self.concerns.push(Concern {
            kind,
            state: ConcernState::Active,
            importance,
            unresolvedness: 1.0,
            adaptation: 0.0,
            age_days: 0.0,
        });
        self.concerns.len() - 1
    }

    /// **Something reminded them.**
    ///
    /// A place, a name, an anniversary, a conversation, a similar danger
    /// — slice 2 supplies what did the reminding; this is the door it
    /// comes through. A cue reaches a concern's *depth* rather than its
    /// daily pressure, so a dormant loss can still take somebody apart on
    /// the right afternoon.
    ///
    /// A strong cue can wake a dormant concern back up, which is what
    /// makes an anniversary worse than the week around it.
    pub fn cued(&mut self, which: usize, strength: f64) -> Option<Episode> {
        let c = self.concerns.get_mut(which)?;
        let force = c.depth() * strength.clamp(0.0, 1.0);
        if force <= 0.02 {
            return None;
        }
        if c.state == ConcernState::Dormant && strength > 0.7 {
            c.state = ConcernState::Active;
            c.adaptation = (c.adaptation - 0.1).max(0.0);
        }
        let what = c.kind.first_feeling();
        Some(Episode {
            what,
            strength: force,
            activation: what.activation(),
            age_days: 0.0,
            about: Some(which),
        })
    }

    /// The grievance was answered, the loss made sense of, the goal let
    /// go. Nothing is deleted — it settles.
    pub fn settle(&mut self, which: usize) {
        if let Some(c) = self.concerns.get_mut(which) {
            c.state = ConcernState::Resolved;
            c.unresolvedness = (c.unresolvedness * 0.2).min(0.15);
        }
    }

    pub fn feel(&mut self, episodes: Vec<Episode>) {
        for e in &episodes {
            let v = e.what.valence();
            let vuln = self.f(Facet::StressVulnerability);
            if v < 0.0 {
                self.stress.load += -v * e.strength * (0.4 + 1.2 * vuln) * 0.15;
            } else {
                self.stress.load -= v * e.strength * 0.08;
            }
            self.mood.valence = 0.9 * self.mood.valence + 0.1 * v * e.strength;
            self.mood.arousal = 0.9 * self.mood.arousal + 0.1 * e.activation * e.strength;
            if matches!(e.what, Emotion::Anger | Emotion::Resentment | Emotion::Outrage) {
                self.mood.irritability = (self.mood.irritability + 0.15 * e.strength).min(1.0);
            }
            if matches!(e.what, Emotion::Fear | Emotion::Anxiety) {
                self.mood.anxiety_bias = (self.mood.anxiety_bias + 0.15 * e.strength).min(1.0);
            }
        }
        self.stress.load = self.stress.load.max(0.0);
        self.episodes.extend(episodes);
    }

    /// A day passes.
    ///
    /// **This is the daily scale, and the decay here is an abstraction
    /// of it.** Acute activation can be gone in minutes or hours; what
    /// carries an episode across days is repeated attention, rumination,
    /// exposure and reappraisal, not the arousal itself *(Verduyn et
    /// al.)*. A tick of a day therefore stores what integrates over that
    /// day, and a locally simulated person would run this in minutes.
    /// **It is not a claim that everything takes days to fade.**
    ///
    /// What differs between grief and rage is not a decay rate — it is
    /// whether a standing concern is still throwing off fresh episodes.
    pub fn a_day_passes(&mut self, rng: &mut Rng) {
        for e in self.episodes.iter_mut() {
            e.age_days += 1.0;
            // Half-life of about two days, for anything. An emotion that
            // is still there a week later is being re-made, not preserved.
            e.strength *= 0.5f64.powf(0.5);
            e.activation *= 0.5f64.powf(0.8);
        }
        self.episodes.retain(|e| e.strength > 0.02);

        // **The concern re-emits.** This is what makes a bereavement last
        // a year and a grievance last a decade, without either being one
        // continuous feeling.
        let mut fresh: Vec<Episode> = Vec::new();
        for (i, c) in self.concerns.iter_mut().enumerate() {
            c.age_days += 1.0;
            // Habituation, slowly. It hurts less; it does not go away.
            c.adaptation = (c.adaptation + 0.0015).min(0.75);
            // **And then it goes quiet.** Accommodation is not the same
            // as resolution: the loss is still there and still answers a
            // cue, but it stops being what every day is about.
            if c.state == ConcernState::Active && c.adaptation > 0.55 {
                c.state = ConcernState::Dormant;
            }
            let pressure = c.pressure();
            if pressure <= 0.02 {
                continue;
            }
            // Waves, not a level. **This spontaneous roll is a
            // placeholder for a cue.** Slice 2 supplies the real ones — a
            // place, a name, an anniversary, a similar danger — and
            // `cued` is the door they come through; intrusive
            // recollection out of nowhere stays one of them.
            if (rng.next_f32() as f64) < 0.10 + 0.25 * pressure {
                let what = match c.kind {
                    ConcernKind::Bereavement => {
                        if rng.next_f32() < 0.5 {
                            Emotion::Grief
                        } else {
                            Emotion::Yearning
                        }
                    }
                    ConcernKind::Grievance => {
                        if rng.next_f32() < 0.5 {
                            Emotion::Anger
                        } else {
                            Emotion::Resentment
                        }
                    }
                    ConcernKind::Threat => Emotion::Anxiety,
                    ConcernKind::BlockedGoal => Emotion::Frustration,
                };
                fresh.push(Episode {
                    what,
                    strength: pressure * (0.4 + 0.6 * rng.next_f32() as f64),
                    activation: what.activation(),
                    age_days: 0.0,
                    about: Some(i),
                });
            }
        }
        if !fresh.is_empty() {
            self.feel(fresh);
        }

        if let Some(n) = self.needs.as_mut() {
            n.a_day_passes();
        }
        self.mood.valence *= 0.93;
        self.mood.arousal *= 0.93;
        self.mood.irritability *= 0.90;
        self.mood.anxiety_bias *= 0.90;

        let toward = self.stress.baseline;
        self.stress.load += (toward - self.stress.load) * self.stress.recovery;
        self.stress.load = self.stress.load.max(0.0);

        self.recompute_focus();
    }

    /// **Focus is not the inverse of stress, and it is not untouched by
    /// it either.**
    ///
    /// What takes attention first is *acute activation* and intrusive
    /// recollection. But chronic load still exerts a smaller, indirect
    /// penalty — through vigilance, rumination, exhaustion and bad sleep
    /// — and leaving it out entirely would say a person can carry
    /// anything indefinitely at no cost, which is not true either.
    ///
    /// Both cases survive: grieving and functional, delighted and
    /// temporarily useless.
    fn recompute_focus(&mut self) {
        let acute: f64 = self
            .episodes
            .iter()
            .map(|e| e.activation * e.strength)
            .sum::<f64>()
            .min(1.0);
        let intrusive: f64 = self
            .concerns
            .iter()
            .map(|c| c.pressure())
            .sum::<f64>()
            .min(1.0)
            * 0.15;
        let chronic = (self.stress.load / self.stress.tolerance.max(0.1)).min(1.5) * 0.12;
        // **What somebody is going without.** The other half of the pair
        // this separation exists for: a contented scholar kept from a
        // book for a month cannot settle to anything, and no amount of
        // calm fixes it.
        let wanting = self.needs.as_ref().map(|n| n.debt()).unwrap_or(0.0) * 0.45;
        let willed = unit(self.willpower);
        let cost = acute * (1.0 - 0.5 * willed) + intrusive + chronic + wanting;
        self.focus.current = (self.focus.capacity - cost).clamp(0.0, 1.0);
    }

    pub fn feeling_of(&self, what: Emotion) -> f64 {
        self.episodes
            .iter()
            .filter(|e| e.what == what)
            .map(|e| e.strength)
            .fold(0.0, f64::max)
    }

    pub fn overloaded(&self) -> bool {
        self.stress.load > self.stress.tolerance
    }
}

/// z to 0..1, for weighting. ±2.5 z covers essentially everybody.
fn unit(z: f32) -> f64 {
    ((z as f64 / 5.0) + 0.5).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------
// what a listener made of it
// ---------------------------------------------------------------------

/// **What a listener concluded somebody meant.**
///
/// Lives here rather than in `social.rs` on purpose. A module that could
/// manufacture this would need the speaker's intent to do it, and
/// telepathy would come back in through module ownership rather than
/// through a field.
///
/// The verdicts are a **different vocabulary** from the speaker's
/// strategies: `Ingratiate` is a plan and `Flattery` is a conclusion, and
/// one word for both would let the listener read the plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reading {
    SincerePraise,
    Flattery,
    FriendlyTeasing,
    Mockery,
    Threat,
    Manipulation,
    Consolation,
    /// Meant kindly and landed badly, which is what a clumsy
    /// encouragement sounds like.
    Condescension,
    PlainStatement,
    AnApology,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeightedReading {
    pub reading: Reading,
    pub weight: f64,
}

/// **What one person took from one exchange.**
///
/// Weighted, because a listener can be pleased and suspicious at the same
/// time — sincere praise and flattery are different informational
/// signals, and a remark can plausibly be either.
#[derive(Clone, Debug, PartialEq)]
pub struct ListenerReading {
    /// The literal content, when the words got through. **Comprehension
    /// and interpretation are separate**: somebody can understand every
    /// word and misjudge entirely what was meant by them.
    pub understood: Option<Content>,
    pub inferred: Vec<WeightedReading>,
    /// How much of it they took to be meant.
    pub sincerity: f64,
    /// Warm to hostile, as read.
    pub stance: f64,
    pub confidence: f64,
}

impl ListenerReading {
    pub fn weight_of(&self, r: Reading) -> f64 {
        self.inferred.iter().find(|w| w.reading == r).map(|w| w.weight).unwrap_or(0.0)
    }
    /// The likeliest verdict — for a line of dialogue or a label, never
    /// for the arithmetic, which uses the whole distribution.
    pub fn likeliest(&self) -> Option<Reading> {
        self.inferred
            .iter()
            .max_by(|a, b| a.weight.total_cmp(&b.weight).then(b.reading.cmp(&a.reading)))
            .map(|w| w.reading)
    }
}

impl Mind {
    /// **Read an act.** The listener's own conclusion, from what actually
    /// reached them.
    ///
    /// Nothing in `SocialAct` says why it was said. What this works from
    /// is the content, the delivery cues that got through, and what this
    /// listener already believes about the speaker — so the same words
    /// with the same delivery produce different verdicts in different
    /// heads, and the same words with *fewer cues* produce a different
    /// verdict in the same head.
    pub fn read_act(
        &self,
        act: &Content,
        delivery: &Delivery,
        cues: &Cues,
        about_the_speaker: Option<(f64, f64)>,
    ) -> ListenerReading {
        let pct = |f: Facet| (self.person.z(f) as f64 / 5.0 + 0.5).clamp(0.0, 1.0);
        // What is known of the speaker: how much they are trusted, and
        // how well they are known.
        let (trusted, known) = about_the_speaker.unwrap_or((0.0, 0.0));
        // **A suspicious listener reads a motive into anything**, and a
        // trusting one takes things at face value.
        let wary = (1.0 - pct(Facet::Trust)) * 0.6 + (1.0 - (trusted + 1.0) / 2.0) * 0.4;
        // Reading somebody accurately is a skill, and it needs the cues.
        let acuity = (0.3 * pct(Facet::Gregariousness)
            + 0.4 * ((self.empathy as f64 / 5.0) + 0.5).clamp(0.0, 1.0)
            + 0.3 * known)
            .clamp(0.0, 1.0);
        let seen = cues.completeness();
        // **Missing the face is what makes a remark ambiguous.**
        let clarity = (0.25 + 0.75 * seen) * (0.5 + 0.5 * acuity);

        let understood = cues.words.then_some(*act);
        let mut inferred: Vec<WeightedReading> = Vec::new();
        let mut push = |r: Reading, w: f64| {
            if w > 0.02 {
                inferred.push(WeightedReading { reading: r, weight: w });
            }
        };

        match act {
            Content::Praise { strength, .. } => {
                // Warmth that shows, from somebody trusted, reads as
                // meant. Eagerness showing through reads as an angle.
                let genuine = (delivery.warmth * clarity + 0.3 * trusted).clamp(0.0, 1.0);
                // **A kindness from somebody you distrust gets explained
                // away**, and strongly — the ultimate attribution error
                // is that a disliked person's good behaviour is put down
                // to an angle rather than to them. Half-weighting it left
                // a thoroughly suspicious man taking warm praise mostly
                // at face value.
                let angled = (delivery.eagerness + wary * 0.8).clamp(0.0, 1.0);
                push(Reading::SincerePraise, genuine * strength);
                push(Reading::Flattery, angled * strength);
                push(Reading::Manipulation, angled * wary * 0.6);
                // **Clumsy encouragement sounds like being patronised**,
                // and what makes it so is in the *delivery*: emphatic
                // approval said without warmth. Reading it off the
                // listener's clarity was wrong twice over — that measures
                // how well the listener could see, which does not change
                // when the speaker fumbles it, so a deft compliment and a
                // graceless one landed identically.
                let flat = (1.0 - delivery.warmth.max(0.0)) * delivery.emphasis;
                push(Reading::Condescension, (flat + 0.5 * delivery.hesitation) * 0.9);
            }
            Content::Barb { sharpness, .. } => {
                // **The grin is the whole difference.** With it, teasing;
                // without it, the identical words are mockery.
                let friendly = if cues.expression && delivery.smiling {
                    0.7 + 0.3 * clarity
                } else {
                    // No face to read: fall back on what is known of the
                    // man, and on how ready you are to think ill.
                    (0.45 + 0.35 * trusted - 0.4 * wary).clamp(0.0, 1.0)
                };
                push(Reading::FriendlyTeasing, friendly * sharpness);
                push(Reading::Mockery, (1.0 - friendly) * sharpness + delivery.edge * 0.5);
            }
            Content::Insult { strength } => {
                push(Reading::Threat, strength * (0.5 + 0.5 * delivery.edge));
                push(Reading::Mockery, strength * 0.6);
            }
            Content::Console => {
                let believed = (delivery.warmth * clarity + 0.3 * trusted).clamp(0.0, 1.0);
                push(Reading::Consolation, believed);
                push(Reading::Condescension, (1.0 - believed) * 0.6);
                push(Reading::Manipulation, wary * (1.0 - believed) * 0.5);
            }
            Content::Request { costs_them } => {
                push(Reading::PlainStatement, 0.6);
                push(Reading::Manipulation, wary * costs_them);
            }
            Content::Apology(a) => {
                // **An apology is read, not applied.** Whether it repairs
                // anything is the listener's decision, later.
                let credible = ((a.acknowledgement + a.responsibility + a.remorse) / 3.0
                    * clarity
                    + 0.25 * trusted
                    - 0.3 * wary)
                    .clamp(0.0, 1.0);
                push(Reading::AnApology, credible);
                push(Reading::Manipulation, (1.0 - credible) * wary);
            }
            Content::Claim(c) => {
                // Confidence in the voice is not truth, and a wary
                // listener knows it.
                push(Reading::PlainStatement, (c.asserted * (0.5 + 0.5 * trusted)).clamp(0.0, 1.0));
                push(Reading::Manipulation, wary * 0.4 * c.asserted);
            }
            Content::Remark { .. } => push(Reading::PlainStatement, 0.8),
        }

        inferred.sort_by(|a, b| b.weight.total_cmp(&a.weight).then(a.reading.cmp(&b.reading)));
        let sincerity = inferred
            .iter()
            .filter(|w| {
                matches!(
                    w.reading,
                    Reading::SincerePraise | Reading::Consolation | Reading::AnApology | Reading::PlainStatement
                )
            })
            .map(|w| w.weight)
            .fold(0.0, f64::max);
        let hostile = inferred
            .iter()
            .filter(|w| matches!(w.reading, Reading::Mockery | Reading::Threat))
            .map(|w| w.weight)
            .fold(0.0, f64::max);
        ListenerReading {
            understood,
            inferred,
            sincerity,
            stance: (delivery.warmth * clarity - hostile).clamp(-1.0, 1.0),
            // **Fewer cues, less certainty.** Which is what makes a
            // misreading a misreading rather than a coin toss.
            confidence: clarity,
        }
    }

    /// **What that did to them**, as an ordinary appraisal.
    ///
    /// The social module produces no emotion of its own: a reading
    /// becomes a `Happening`, this mind reads it, and everything
    /// downstream is the machinery that already exists.
    pub fn appraise_reading(&self, r: &ListenerReading, from_a_friend: f64) -> Vec<Episode> {
        let good = r.weight_of(Reading::SincerePraise)
            + r.weight_of(Reading::Consolation)
            + r.weight_of(Reading::FriendlyTeasing) * 0.4;
        let bad = r.weight_of(Reading::Mockery) + r.weight_of(Reading::Threat)
            + r.weight_of(Reading::Condescension) * 0.7;
        let h = Happening {
            severity: (good - bad).clamp(-1.0, 1.0),
            to_me: 1.0,
            to_mine: from_a_friend,
            deliberate: true,
            control: 0.4,
            unexpected: 1.0 - r.confidence,
            ..Default::default()
        };
        let mut felt = self.appraise(&self.read(&h));
        // **Pleased and suspicious at once.** A reading that is largely
        // sincere and partly self-serving is exactly that, and one enum
        // on either side would have to choose.
        let suspicion = r.weight_of(Reading::Flattery) + r.weight_of(Reading::Manipulation);
        if suspicion > 0.15 {
            felt.push(Episode {
                what: Emotion::Anxiety,
                strength: (suspicion * 0.5).min(1.0),
                activation: Emotion::Anxiety.activation(),
                age_days: 0.0,
                about: None,
            });
        }
        felt
    }
}
