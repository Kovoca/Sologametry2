//! A town of people, not a number of them.
//!
//! `labour.rs` says how many hands a town has and what share of them are
//! idle. `person.rs` follows one man through a year. This is the join: a
//! **sample** of individuated people living in the markets the economy
//! already models, each running the same day the single man does.
//!
//! ## Why a sample rather than everybody
//!
//! A city of sixteen million cannot be sixteen million `Person`s, and
//! should not be: the design doc's rule is that populations stay
//! statistical until attention or consequence promotes them. What is
//! sampled here is a representative cohort — every individuated person
//! stands for some thousands of real ones — so that what happens to the
//! sample can be checked against what the statistics claim.
//!
//! ## The point of it
//!
//! **The individuals and the aggregates must agree.** If a town's
//! workforce statistics say a fifth are idle and the sampled people are
//! all in work, one of the two models is wrong and the disagreement says
//! which. That check is the whole reason to run people rather than
//! numbers, and it is the thing a single man in a single town could never
//! reveal.

use crate::econ::Economy;
use crate::person::{live_a_day_with, Housing, Person, Trade};
use crate::rng::Rng;

/// A person's fate over a run, kept so a cohort can be summarised without
/// re-walking every log.
#[derive(Default, Clone, Copy, Debug)]
pub struct Outcome {
    pub days_worked: u64,
    pub days_hungry: u64,
    pub days_homeless: u64,
    pub died: bool,
}

pub struct Populace {
    pub people: Vec<Person>,
    /// **How many real people each individuated one stands for.**
    ///
    /// Not decoration: it is what lets the cohort be compared with the
    /// town's own figures, and what stops anybody mistaking a hundred
    /// simulated lives for a hundred thousand real ones.
    pub represents: Vec<f64>,
    /// Everyone who has died, so a run can be judged on the people it
    /// killed as well as the ones still standing.
    pub gone: Vec<(String, u64)>,
    rng: Rng,
}

/// Names enough to tell people apart. Not a naming system — that belongs
/// with the history sim, which will want families and places.
const FIRST: [&str; 16] = [
    "Alma", "Bert", "Cora", "Dai", "Elsie", "Fred", "Grace", "Hal", "Ida", "Joe", "Kit", "Lil",
    "Mabel", "Ned", "Ollie", "Pearl",
];
const LAST: [&str; 12] = [
    "Ash", "Brook", "Carden", "Dell", "Ewart", "Finn", "Gale", "Hollis", "Ivey", "Judd", "Kemp",
    "Lowe",
];

impl Populace {
    /// **Seed a cohort in every market**, in the trades the town actually
    /// has.
    ///
    /// Trades are drawn in proportion to the posts the labour model says
    /// exist, so a farming town is sampled with labourers and a city with
    /// shop workers. Seeding them evenly would have produced hauliers in
    /// a town with nothing to haul, and the cohort would have told you
    /// about the sampling rather than about the town.
    pub fn seed(econ: &Economy, per_market: usize, seed: u64) -> Self {
        let mut rng = Rng::new(seed ^ 0x5DEE_CE66_D9B0_1B0D);
        let mut people = Vec::new();
        let mut represents = Vec::new();

        for m in 0..econ.markets.len() {
            let pop = econ.markets[m].population;
            if pop < 1.0 {
                continue;
            }
            let n = per_market.max(1);
            for _ in 0..n {
                let first = FIRST[(rng.next_f32() * FIRST.len() as f32) as usize % FIRST.len()];
                let last = LAST[(rng.next_f32() * LAST.len() as f32) as usize % LAST.len()];
                let trade = draw_trade(&mut rng);
                // **Nobody starts with anything.** A fortnight's food and
                // a room, which is the position the single-person runs
                // start from and the one that makes a bad month bite.
                let money = 40.0 + rng.next_f32() as f64 * 60.0;
                let mut p = Person::new(format!("{first} {last}"), trade, m, money);
                // **People differ, and that is the point.** Three draws
                // averaged, so most are middling and the very good and the
                // very poor are rare — which is what a real distribution
                // of anything looks like and what a flat draw does not.
                p.diligence = ((rng.next_f32() + rng.next_f32() + rng.next_f32()) / 3.0) as f64;
                people.push(p);
                represents.push(pop / n as f64);
            }
        }

        Populace {
            people,
            represents,
            gone: Vec::new(),
            rng,
        }
    }

    /// One day for everybody.
    ///
    /// **The economy steps once, not once per person.** Running the
    /// economy inside the loop would let each person trade against a
    /// slightly different world and quietly break conservation.
    pub fn live_a_day(&mut self, econ: &mut Economy, day: u64) {
        // **Is there a post going?**
        //
        // Not a ratio picked to look right: the number of supervisory
        // posts is a real figure the labour model already computes from
        // the works and shops that exist, at a span of control of about
        // ten. **There is no ladder with room for everybody on it**, and
        // without this every labourer in a three-year run was made up to
        // chargehand — the cohort became all supervisors, which is a
        // promotion timer with nobody left to supervise.
        //
        // The cohort is a sample, so the posts are scaled to it: if the
        // town has one supervisory post per twenty hands, so does the
        // sample.
        let n_markets = econ.markets.len();
        let mut vacancy = vec![false; n_markets];
        for m in 0..n_markets {
            let mine: Vec<&Person> = self.people.iter().filter(|p| p.market == m).collect();
            if mine.is_empty() {
                continue;
            }
            let w = &econ.workforce[m];
            let share = if w.posts > 1e-9 {
                (w.supervisory_posts / w.posts).clamp(0.0, 0.35)
            } else {
                0.0
            };
            let bosses = mine.iter().filter(|p| p.trade == Trade::Supervisor).count() as f64;
            vacancy[m] = bosses < mine.len() as f64 * share;
        }

        for p in self.people.iter_mut() {
            if p.condition <= 0.0 {
                continue;
            }
            let free = vacancy.get(p.market).copied().unwrap_or(false);
            live_a_day_with(p, econ, day, free);
        }
        self.bury_the_dead(day);
    }

    /// **Somebody who starves is replaced, because the town has not
    /// shrunk.**
    ///
    /// The cohort is a sample of a population that the economy is still
    /// counting in full. Letting the sample dwindle would make a town look
    /// emptier the longer it was watched, which is an artefact of the
    /// sampling and not a fact about the town.
    fn bury_the_dead(&mut self, day: u64) {
        for i in 0..self.people.len() {
            if self.people[i].condition > 0.0 {
                continue;
            }
            self.gone
                .push((self.people[i].name.clone(), day));
            let market = self.people[i].market;
            let first =
                FIRST[(self.rng.next_f32() * FIRST.len() as f32) as usize % FIRST.len()];
            let last = LAST[(self.rng.next_f32() * LAST.len() as f32) as usize % LAST.len()];
            let trade = draw_trade(&mut self.rng);
            let mut p = Person::new(format!("{first} {last}"), trade, market, 50.0);
            p.diligence = ((self.rng.next_f32() + self.rng.next_f32() + self.rng.next_f32())
                / 3.0) as f64;
            self.people[i] = p;
        }
    }

    /// **Days worked by one trade only**, which is the only fair way to
    /// hold the cohort against the workforce statistics.
    ///
    /// `labour::Workforce` counts the trades the *works* employ — farm,
    /// mill, cannery, mine. A shop worker is rostered by `building.rs` and
    /// does not appear in it at all. So a town can carry idle industrial
    /// hands and busy shops at the same time, and comparing the whole
    /// cohort against the industrial figure compares two different
    /// populations. That is not a disagreement between the models; it is
    /// a disagreement between the questions.
    pub fn worked_by(&self, market: usize, trade: Trade, days_elapsed: u64) -> f64 {
        let mine: Vec<&Person> = self
            .people
            .iter()
            .filter(|p| p.market == market && p.trade == trade)
            .collect();
        if mine.is_empty() {
            return f64::NAN;
        }
        let days: u64 = mine.iter().map(|p| p.days_worked).sum();
        days as f64 / (days_elapsed.max(1) * mine.len() as u64) as f64
    }

    /// How the cohort in one market is faring.
    pub fn summary(&self, market: usize, days_elapsed: u64) -> Cohort {
        let mine: Vec<&Person> = self
            .people
            .iter()
            .filter(|p| p.market == market)
            .collect();
        let n = mine.len().max(1) as f64;
        Cohort {
            people: mine.len(),
            // **Share of days worked, against days lived.** Not against
            // `days_idle`, which counts days *since* the last work and
            // resets — dividing by it gave every town 100%. And not
            // today's state either: a rostered shop worker is off two days
            // in three and is not unemployed.
            worked: {
                let days: u64 = mine.iter().map(|p| p.days_worked).sum();
                days as f64 / (days_elapsed.max(1) * mine.len().max(1) as u64) as f64
            },
            // **Who actually went hungry**, not who holds less than a day's
            // food. Nearly everybody holds less than a day's food, because
            // nearly everybody buys daily — reading that as hunger put a
            // prosperous town at 100% starving.
            hungry: mine.iter().filter(|p| p.days_hungry > 0).count() as f64 / n,
            homeless: mine
                .iter()
                .filter(|p| p.housing == Housing::Homeless)
                .count() as f64
                / n,
            mean_money: mine.iter().map(|p| p.money).sum::<f64>() / n,
            mean_condition: mine.iter().map(|p| p.condition).sum::<f64>() / n,
        }
    }
}

/// How a market's sampled people are doing, as shares of the cohort.
#[derive(Default, Clone, Copy, Debug)]
pub struct Cohort {
    pub people: usize,
    /// Share of the cohort's days that were worked.
    pub worked: f64,
    pub hungry: f64,
    pub homeless: f64,
    pub mean_money: f64,
    pub mean_condition: f64,
}

/// **The trades the economy actually contains**, in roughly the
/// proportions the works and shops need them.
///
/// Retail is about a tenth of real employment and the works this economy
/// models are about a fiftieth, which is why shop work dominates a
/// sampled town — and why a haulier is uncommon rather than typical.
fn draw_trade(rng: &mut Rng) -> Trade {
    let r = rng.next_f32();
    if r < 0.55 {
        Trade::Shopworker
    } else if r < 0.85 {
        Trade::Labourer
    } else if r < 0.94 {
        Trade::Haulier
    } else {
        Trade::Supervisor
    }
}
