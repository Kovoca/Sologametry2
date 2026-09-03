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

// ---------------------------------------------------------------------
// where people live is what makes the custom
// ---------------------------------------------------------------------

/// **What a place is like to live in**, as facts the world already
/// generates.
///
/// This module used to carry three hand-written cultures — an old
/// country, a city, a market town — which is precisely the fault this
/// project rejects everywhere else: assuming wheat everywhere, or every
/// nation growing 125% of what it eats. **Nobody decides what is done
/// here.** It follows from how many people there are, how close together,
/// in what climate, how far from anywhere, and what they do for a living.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Conditions {
    pub population: f64,
    /// People per square kilometre.
    pub density: f64,
    pub mean_temp_c: f64,
    /// How hard the ground is to live off, 0..1. One of the standing
    /// threats that make a society hold its rules tightly.
    pub scarcity: f64,
    /// How far from anywhere else, 0..1.
    pub remoteness: f64,
    /// Somewhere the price is arrived at rather than posted.
    pub is_market: bool,
    pub coastal: bool,
    /// Share of the people who work the land.
    pub farming_share: f64,
}

impl Default for Conditions {
    fn default() -> Self {
        Conditions {
            population: 5_000.0,
            density: 500.0,
            mean_temp_c: 10.0,
            scarcity: 0.3,
            remoteness: 0.4,
            is_market: false,
            coastal: false,
            farming_share: 0.4,
        }
    }
}

impl Conditions {
    /// **How tightly the rules are held here**, 0..1.
    ///
    /// The best-supported single dimension of cultural variation there
    /// is: across thirty-three nations, societies under more ecological
    /// and historical threat — crowding, scarce resources, disease,
    /// invasion — hold their norms harder and tolerate deviance less
    /// *(Gelfand et al.)*. Nothing about that is a preference; it is what
    /// living close together on thin ground does to a rule.
    pub fn tightness(&self) -> f64 {
        let crowding = (self.density / 4_000.0).clamp(0.0, 1.0);
        let thin_ground = self.scarcity.clamp(0.0, 1.0);
        // And a place everybody can leave holds its rules loosely.
        let nowhere_to_go = self.remoteness.clamp(0.0, 1.0);
        (0.2 + 0.35 * crowding + 0.30 * thin_ground + 0.25 * nowhere_to_go).clamp(0.0, 1.0)
    }

    /// **How fast life goes here.** Pace rises with size, wealth and
    /// cold — measured across thirty-one countries by walking speed,
    /// clock accuracy and how long it takes to buy a stamp *(Levine &
    /// Norenzayan)*.
    pub fn pace(&self) -> f64 {
        let size = (self.population.max(1.0).log10() / 7.0).clamp(0.0, 1.0);
        let cold = ((15.0 - self.mean_temp_c) / 30.0).clamp(0.0, 1.0);
        (0.15 + 0.6 * size + 0.35 * cold).clamp(0.0, 1.0)
    }

    /// **Whether anybody could know everybody.**
    ///
    /// A village of three hundred is a place where a stranger is
    /// remarkable. A city of eight million is one where greeting
    /// everybody is not a choice anybody has — which is the overload
    /// account of urban reserve, and it shows up in real helping-
    /// behaviour studies as a fall with size and density rather than a
    /// difference in character.
    /// Anchored on two real figures rather than a convenient divisor:
    /// **about a hundred and fifty** people is the most anybody keeps
    /// real relationships with *(Dunbar)*, and by **fifty thousand** it
    /// is certainly gone. In between it decays. Dividing the logarithm by
    /// four instead put the line at ten thousand and came out saying a
    /// village of four hundred does not greet strangers, which is the
    /// opposite of what a village is.
    pub fn everybody_knows_everybody(&self) -> f64 {
        let lo = 150.0f64.log10();
        let hi = 50_000.0f64.log10();
        (1.0 - (self.population.max(1.0).log10() - lo) / (hi - lo)).clamp(0.0, 1.0)
    }
}

/// **Derive what is done here from where here is.**
pub fn norms_of(c: &Conditions) -> Custom {
    let known = c.everybody_knows_everybody();
    let tight = c.tightness();
    let pace = c.pace();

    // Speaking to a stranger: ordinary where a stranger is a rarity,
    // not done where they are the whole street.
    let greet = (1.6 * known - 0.7).clamp(-1.0, 1.0);

    // **A courtesy is answered nearly everywhere**, and said out loud
    // less where reciprocity is simply assumed.
    let thank = (0.85 - 0.3 * known).clamp(0.0, 1.0);

    // **Personal space is larger where it is cold** — measured across
    // forty-two countries, and it tracks temperature more than anything
    // about the people *(Sorokowska et al.)* — and larger again where
    // there are too many people to stand near.
    let distance = ((15.0 - c.mean_temp_c) / 25.0 + 0.4 * (c.density / 4_000.0)).clamp(-1.0, 1.0);

    // **Indirectness is what a place where everybody will meet again can
    // afford.** Say the blunt thing in a village and you live with it;
    // a mobile, crowded place has no such memory.
    let direct = (1.0 - 1.6 * known).clamp(-1.0, 1.0);

    // **Guest-right is strongest where travel is dangerous and there is
    // no inn** — deserts, mountains, the far edges of anywhere.
    let hospitality = (0.25 + 0.6 * c.remoteness + 0.35 * c.scarcity - 0.4 * (c.population.max(1.0).log10() / 6.0))
        .clamp(-1.0, 1.0);

    // Punctuality follows the pace of the place and the clock the work
    // is kept by.
    let punctual = (1.2 * pace - 0.5 - 0.4 * c.farming_share).clamp(-1.0, 1.0);

    // **Age is deferred to where what an old person knows is still worth
    // knowing**, which is farming and craft rather than a mobile
    // industrial town.
    let elders = (0.7 * c.farming_share + 0.4 * tight - 0.5 * (c.population.max(1.0).log10() / 6.0))
        .clamp(-1.0, 1.0);

    // **Haggling is what happens where the price is not posted.** Fixed
    // prices are an invention of scale retail — the Bon Marché in 1852,
    // Wanamaker in 1876 — and they end it wherever they arrive.
    let scale_retail = (c.population.max(1.0).log10() / 6.0).clamp(0.0, 1.0);
    let haggle = if c.is_market {
        (0.9 - 0.5 * scale_retail).clamp(-1.0, 1.0)
    } else {
        (0.2 - 1.1 * scale_retail).clamp(-1.0, 1.0)
    };

    Custom::new(&[
        (Norm::GreetStrangers, greet),
        (Norm::ThankForCourtesy, thank),
        (Norm::KeepDistance, distance),
        (Norm::Directness, direct),
        (Norm::AcceptHospitality, hospitality),
        (Norm::Punctuality, punctual),
        (Norm::DeferToElders, elders),
        (Norm::Haggle, haggle),
    ])
}

/// Places, **derived rather than declared**. Each is a set of conditions
/// the world could produce; what is done there follows from them.
pub mod places {
    use super::{norms_of, Conditions, Custom};

    /// A farming village at the end of a long road.
    pub fn a_village() -> Conditions {
        Conditions {
            population: 400.0,
            density: 60.0,
            mean_temp_c: 9.0,
            scarcity: 0.35,
            remoteness: 0.8,
            is_market: false,
            coastal: false,
            farming_share: 0.8,
        }
    }

    /// Eight million people in the cold.
    pub fn a_metropolis() -> Conditions {
        Conditions {
            population: 8_000_000.0,
            density: 6_000.0,
            mean_temp_c: 9.0,
            scarcity: 0.1,
            remoteness: 0.0,
            is_market: false,
            coastal: true,
            farming_share: 0.01,
        }
    }

    /// A hot market town where the price is a conversation.
    pub fn a_market_town() -> Conditions {
        Conditions {
            population: 14_000.0,
            density: 900.0,
            mean_temp_c: 24.0,
            scarcity: 0.45,
            remoteness: 0.45,
            is_market: true,
            coastal: false,
            farming_share: 0.5,
        }
    }

    /// A settlement on thin ground a long way from help.
    pub fn a_desert_outpost() -> Conditions {
        Conditions {
            population: 900.0,
            density: 40.0,
            mean_temp_c: 28.0,
            scarcity: 0.9,
            remoteness: 0.95,
            is_market: true,
            coastal: false,
            farming_share: 0.3,
        }
    }

    pub fn old_country() -> Custom {
        norms_of(&a_village())
    }
    pub fn the_city() -> Custom {
        norms_of(&a_metropolis())
    }
    pub fn the_market() -> Custom {
        norms_of(&a_market_town())
    }
}
