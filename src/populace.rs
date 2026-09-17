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
use crate::id::{Arena, Id};
use crate::person::{
    day_rate, household_share_for, live_a_day_with, qualification_for, Housing, Person,
    Qualification, State, Trade,
};
use crate::planner::{self, Heard, Lead, Outcome as Day};
use crate::rng::Rng;
use crate::travel::Conveyance;

/// **How people actually live**, which is not one to a house.
///
/// Real British composition: **one person 30%, a couple 27%, a couple with
/// children 22%, a lone parent 10%, and about 11% other** — which is
/// chiefly shared houses and adult children at home. The average household
/// is **2.36 people**, and 28% of 20-34 year olds live with their parents.
///
/// The distinction that matters here is only how many adults are carrying
/// the costs, because that is what the equivalence scale reads.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Household {
    /// Living alone: 30% of households and the most expensive way to live.
    Alone,
    /// A couple, with or without children. Two adults on one rent.
    Couple,
    /// A shared house or flat: unrelated adults splitting the cost, which
    /// is about one private renter in five.
    Shared(usize),
    /// An adult child still at home, or a parent taken in. 28% of 20-34
    /// year olds, and it is overwhelmingly about money.
    Family(usize),
}

impl Household {
    /// How many adults are carrying it.
    pub fn adults(self) -> usize {
        match self {
            Household::Alone => 1,
            Household::Couple => 2,
            Household::Shared(n) | Household::Family(n) => n.max(1),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Household::Alone => "alone",
            Household::Couple => "a couple",
            Household::Shared(_) => "sharing",
            Household::Family(_) => "family",
        }
    }
}

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
    /// **An arena, not a vector, because people die.**
    ///
    /// A dead person used to be overwritten in place by their
    /// replacement: slot 7 was Alice the haulier on Monday and Bob the
    /// shop worker on Tuesday, and nothing could tell. Nothing else holds
    /// a person's index across a day *yet*, which is the only reason it
    /// has never bitten — and the moment anything does (a tenancy, a
    /// debt, a firm's payroll) it would hand Alice's savings to Bob and
    /// every conservation check in the model would still pass.
    pub people: Arena<Person>,
    /// **How many real people each individuated one stands for.**
    ///
    /// Not decoration: it is what lets the cohort be compared with the
    /// town's own figures, and what stops anybody mistaking a hundred
    /// simulated lives for a hundred thousand real ones.
    pub represents: Vec<f64>,
    /// How each of them lives, which decides what share of a household's
    /// costs they carry.
    /// Parallel to the arena's *slots*, because a household is a
    /// property of the place in the sample rather than of the person: a
    /// replacement moves into the same household the deceased left.
    pub households: Vec<Household>,
    /// **Estates that went to the state** because nobody was left to take
    /// them. Kept rather than discarded, so the money can be accounted
    /// for instead of quietly ceasing to exist.
    pub escheated: f64,
    /// Estates that passed to kin.
    pub inherited: f64,
    /// Everyone who has died, so a run can be judged on the people it
    /// killed as well as the ones still standing.
    pub gone: Vec<(String, u64)>,
    /// **Openings somebody has set out after**, held until they get the
    /// work, give up, or the hold lapses — spec A4.7. Without it a town
    /// with one post going would send everybody who heard of it.
    pub reservations: Vec<Reservation>,
    rng: Rng,
}

/// **A hold on an opening while somebody goes after it.** Expiring, so a
/// person who dies or changes their mind does not hold a post for ever.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reservation {
    pub who: Id<Person>,
    pub market: usize,
    pub trade: Trade,
    pub until: u64,
}

/// **How far a sampled trade's skill is drawn toward the ordinary**, in
/// people.
///
/// The standard shrinkage of a sample mean: a reading from `n` people is
/// weighted `n / (n + k)` against the expected level, with `k` the ratio of
/// how much individuals differ to how much towns do. Individuals in a trade
/// here spread about a level and a half either side of their town's mean;
/// towns' means plausibly differ by about half a level. Nine, designed:
/// three people are trusted a quarter, nine people a half, thirty people
/// three quarters.
const A_SAMPLE_IS_THIS_MANY_PEOPLE_SHORT: f64 = 9.0;

/// **How many people who know of an opening tell somebody else about it
/// each day.** A few; designed. Word of mouth is how about half of real
/// jobs are found, and it reaches whoever the teller happens to know.
const TOLD_A_DAY: usize = 3;

/// **Hold an opening for somebody setting out after it**, if one is still
/// free — spec A4.7, and the only place the rule is written.
///
/// `room` is how many posts in that trade the town has beyond the people in
/// it; every hold already on it counts against that. A whole post must be
/// left, or the answer is no and they keep doing what they were doing.
pub fn hold_an_opening(
    reservations: &mut Vec<Reservation>,
    who: Id<Person>,
    market: usize,
    trade: Trade,
    room: f64,
    day: u64,
) -> bool {
    let held = reservations
        .iter()
        .filter(|r| r.market == market && r.trade == trade)
        .count() as f64;
    if room - held < 1.0 {
        return false;
    }
    reservations.push(Reservation {
        who,
        market,
        trade,
        until: day + planner::LEAD_LIFE_DAYS,
    });
    true
}

/// A deterministic number from a few others — who hears what must rebuild
/// identically from a seed like everything else.
fn mix(parts: &[u64]) -> u64 {
    let mut h = 0x9E37_79B9_7F4A_7C15u64;
    for &p in parts {
        h ^= p.wrapping_add(0x6A09_E667_F3BC_C909);
        h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h ^= h >> 31;
    }
    h
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
        let mut people = Arena::new();
        let mut represents = Vec::new();
        let mut households = Vec::new();
        let posts = crate::occupation::jobs_by_occupation(econ);

        for m in 0..econ.markets.len() {
            let pop = econ.markets[m].population;
            if pop < 1.0 {
                continue;
            }
            let n = per_market.max(1);
            for _ in 0..n {
                let first = FIRST[(rng.next_f32() * FIRST.len() as f32) as usize % FIRST.len()];
                let last = LAST[(rng.next_f32() * LAST.len() as f32) as usize % LAST.len()];
                // Settled below, once it is known what they are qualified
                // to do.
                let trade = Trade::Sales;
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
                // **What they can learn is not how hard they work at it.**
                // Same shape of draw, an independent roll: plenty of able
                // people are idle and plenty of dogged ones are not clever.
                let free = ((rng.next_f32() + rng.next_f32() + rng.next_f32()) / 3.0) as f64;
                // **Nobody lives one to a house.** Real composition: 30%
                // alone, 27% a couple, 22% a couple with children, 10% a
                // lone parent, ~11% sharing or family — average household
                // 2.36 people.
                let h = draw_household(&mut rng);
                p.household_share = household_share_for(h.adults());
                // **A working-age spread**, not everybody thirty. Real:
                // 64% of a population is 16-64, and a cohort of workers is
                // drawn from that span.
                p.age_years = 18.0 + rng.next_f32() as f64 * 45.0;
                // **About 35% of working-age adults hold a degree** and
                // initial participation in higher education is around 38%
                // of young people; apprenticeships add another slice. So
                // most people have school and no more, which is what most
                // work requires.
                //
                // **Nobody holds a qualification they could not finish**, so
                // the draw is gated the same way the children's is. It skews
                // the graduate body upward in ability without any rule
                // saying so — which is what a real graduate body looks like.
                let q = rng.next_f32() as f64;
                p.qualification = if q < 0.32 {
                    Qualification::Degree
                } else if q < 0.52 {
                    Qualification::Vocational
                } else {
                    Qualification::School
                };
                // **These adults already got through it**, so ability is
                // drawn *given* the qualification rather than gated by it.
                // Gating it instead multiplies two thirds by a third and
                // gives a country of 9% graduates — a test caught exactly
                // that. Sampling a population that has already run the
                // pipeline is not the same as running the pipeline.
                //
                // A holder is somewhere above the floor, skewed upward;
                // somebody who left at sixteen may be able or not, because
                // plenty of able people never went.
                let floor = p.qualification.takes_to_finish();
                p.aptitude = if floor > 0.0 {
                    (floor + (1.0 - floor) * free).min(1.0)
                } else {
                    free
                };
                // **The adults are given their years as well as their
                // skills.** A world's starting population has already
                // worked: somebody of forty has twenty-odd years at their
                // trade behind them, and a model where every adult begins
                // untrained has no experienced anybody on its first
                // morning.
                //
                // Practice stops at what they are capable of, so it is
                // experience that gets a person to their ceiling and
                // aptitude that decides where the ceiling is. Most people
                // reach it; **legendary is rare because the ability to get
                // there is rare**, not because the hours are unavailable.
                // **What they do is drawn from the work this town has**,
                // among what their qualification lets them do — which is
                // what this function's own description always said and the
                // code did not. It drew from national shares and put anybody
                // not qualified for the draw behind a shop counter, so half
                // of every sample were shop workers, a few percent were
                // doctors and electricians the economy has no work for, and
                // a farming town was sampled with a handful of labourers.
                p.trade = draw_work(&mut rng, p.qualification, &posts[m]);
                // **After the trade is settled, not before.** Assigning
                // the years first put them against the skill of a trade
                // the person then did not end up in, and 44% of a country
                // came out untrained at everything.
                p.settle_into(p.trade);
                households.push(h);
                people.add(p);
                represents.push(pop / n as f64);
            }
        }

        let mut folk = Populace {
            escheated: 0.0,
            inherited: 0.0,
            people,
            represents,
            households,
            gone: Vec::new(),
            reservations: Vec::new(),
            rng,
        };
        folk.marry_the_couples();
        folk
    }

    /// **A couple is two people, and the sample knows only one of them.**
    ///
    /// Households are drawn per sampled person — this one lives alone,
    /// that one is half of a couple — so nothing said *which* couple. For
    /// probate it has to: a spouse is the first rung of the ladder and by
    /// far the commonest answer, since **the great majority of estates go
    /// to one**, and a model where nobody is married would escheat almost
    /// everything to the state.
    ///
    /// So couples in the same market are paired off, two at a time, in
    /// slot order — deterministic, like everything else here. An odd one
    /// out stays single, which is honest: their spouse is one of the
    /// people the sample did not draw.
    fn marry_the_couples(&mut self) {
        let mut waiting: Vec<(usize, Id<Person>)> = Vec::new();
        for id in self.people.ids().collect::<Vec<_>>() {
            if !matches!(
                self.households[id.slot()],
                Household::Couple | Household::Family(_)
            ) {
                continue;
            }
            let market = self.people[id].market;
            if let Some(pos) = waiting.iter().position(|(m, _)| *m == market) {
                let (_, other) = waiting.remove(pos);
                self.people[id].spouse = Some(other);
                self.people[other].spouse = Some(id);
            } else {
                waiting.push((market, id));
            }
        }
    }

    /// **A year older, and some of them new.**
    ///
    /// The model had no ages, so there were no children, nobody retired
    /// and nobody was replaced — a population that could only shrink, by
    /// starving. This is the replenishment: people age, some have
    /// children, and children grow up and start costing less.
    ///
    /// Real figures throughout. **Total fertility is 1.44 in Britain
    /// against a replacement rate of 2.1** — most developed countries are
    /// below replacement and only hold their numbers by immigration. Mean
    /// age at a first birth is 29, working age is 16-64, and life
    /// expectancy is about 81.
    ///
    /// **And a health service is what keeps that number up.** Infant
    /// mortality is 3.9 per thousand live births in Britain and over 25 in
    /// a state that cannot fund a hospital — which is the single largest
    /// difference public spending makes to how long anybody lives, and it
    /// falls straight out of the budget line that already exists.
    fn a_year_passes(&mut self, econ: &Economy, day: u64) {
        let posts = crate::occupation::jobs_by_occupation(econ);
        /// Births per woman over a lifetime. Replacement is 2.1.
        const FERTILITY: f64 = 1.7;
        /// Roughly the span over which they arrive: 20 to 40.
        const CHILDBEARING_YEARS: f64 = 20.0;
        /// Real: 3.9 per 1,000 live births where there is a health
        /// service, and 25 or worse where there is not.
        const INFANT_DEATHS_WITH_A_HOSPITAL: f64 = 0.0039;
        const INFANT_DEATHS_WITHOUT: f64 = 0.055;

        // **What the hospitals can actually deliver**, which is the staff
        // the budget pays for *and* the medicines they can get hold of.
        //
        // Reading the budget line alone said a fully funded health service
        // was a working one even with every pharmacy empty. The drugs are
        // made in a factory out of oil, so a country cut off from the
        // feedstock has hospitals full of staff who cannot treat anybody —
        // and this is the one place in the model where that decides
        // whether somebody lives.
        // **And it is the health service of the country somebody lives
        // in.** This read one figure for the whole economy, which is right
        // for a world holding one country and says the wrong thing in a
        // world holding four: a child born in a nation whose state cannot
        // fund a hospital had the survival odds of the richest country on
        // the map.
        let infant_deaths_in = |m: usize| {
            let health = econ
                .government(m)
                .map(|g| g.health_delivered())
                .unwrap_or(0.0);
            INFANT_DEATHS_WITHOUT + (INFANT_DEATHS_WITH_A_HOSPITAL - INFANT_DEATHS_WITHOUT) * health
        };

        let mut grown: Vec<(Id<Person>, f64)> = Vec::new();
        // **Handles up front.** Births during the loop add to the arena,
        // and iterating it live would age the newborn on the day it was
        // born. Taking the handles first is the same discipline the
        // ledger follows: decide what to act on, then act.
        let everyone: Vec<Id<Person>> = self.people.ids().collect();
        for i in everyone {
            self.people[i].age_years += 1.0;
            for c in self.people[i].children.iter_mut() {
                *c += 1.0;
            }
            // **A child who reaches sixteen leaves and becomes somebody.**
            //
            // This is the difference between the adults the world starts
            // with and the ones it makes. The starting population is
            // *given* qualifications — it has to be, or nothing functions
            // on the first morning — but a child born into the simulation
            // has to actually go and get one, and whether it can is
            // decided by what its household could carry.
            let leaving: Vec<f64> = self.people[i]
                .children
                .iter()
                .copied()
                .filter(|&c| c >= crate::person::SCHOOL_ENDS)
                .collect();
            self.people[i]
                .children
                .retain(|&c| c < crate::person::SCHOOL_ENDS);
            for _ in leaving {
                grown.push((i, self.rng.next_f32() as f64));
            }

            // **Whether a child arrives this year.** Spread over the
            // childbearing span, so a lifetime comes to the fertility
            // rate; halved because it takes two and only one carries it
            // here.
            let age = self.people[i].age_years;
            if (20.0..40.0).contains(&age) {
                // **Family support lifts the birth rate — weakly.**
                //
                // The honest version, because the evidence is honest about
                // it: OECD fertility fell from 1.8 to 1.7 between 2009 and
                // 2017 across countries spending heavily, and Korea has
                // cheap childcare and the lowest fertility on earth.
                // Housing, hours and what is expected of a parent all bear
                // on it. What family spending reliably buys is that a
                // parent can *work*; the birth rate responds, but weakly —
                // France's 4% of GDP buys 1.79 against Britain's 1.44, so
                // call it a fifth either way.
                let home = self.people[i].market;
                let support = econ
                    .government(home)
                    .map(|g| 1.0 - g.childcare_borne_by_parents())
                    .unwrap_or(0.0);
                let chance = FERTILITY * (1.0 + 0.22 * support) / CHILDBEARING_YEARS / 2.0;
                if (self.rng.next_f32() as f64) < chance {
                    // **Born, and it may not live.** This is where a
                    // hospital shows up in a population rather than in a
                    // budget.
                    if (self.rng.next_f32() as f64) >= infant_deaths_in(home) {
                        self.people[i].children.push(0.0);
                        // A child in the house changes what the household
                        // costs, on the same equivalence scale.
                        let adults = self.households[i.slot()].adults();
                        self.people[i].household_share = household_share_for(adults)
                            * (1.0 + 0.3 * self.people[i].children.len() as f64 / adults as f64);
                    } else {
                        self.people[i].note(day, "the child did not live".to_string());
                    }
                }
            }
        }

        // **Who goes on, and who goes to work.**
        //
        // Real participation: about **38% of young people enter higher
        // education** and apprenticeship starts run ~340,000 a year, with
        // the rest leaving at sixteen or eighteen. But it is not a lottery
        // — a degree is three years earning nothing, and **whether a
        // household can carry somebody for three years is what decides
        // it**. That is the mechanism by which advantage reproduces
        // itself, and it needs no special rule: it is the arithmetic.
        for (parent, roll) in grown {
            let household = &self.people[parent];
            // What the family can carry, roughly: money in hand against
            // three years of somebody not earning.
            let three_years = day_rate(econ, household.market, Trade::Sales)
                * crate::econ::DAYS_PER_YEAR as f64
                * 3.0;
            let can_carry = (household.money / three_years.max(1.0)).clamp(0.0, 1.0);

            // A state that funds education carries some of it instead, and
            // that is most of what a maintenance grant is for.
            let helped = econ
                .government(household.market)
                .map(|g| {
                    let i = crate::state::Service::ALL
                        .iter()
                        .position(|&s| s == crate::state::Service::Education)
                        .unwrap_or(0);
                    g.funded[i]
                })
                .unwrap_or(0.0);
            let afford = (can_carry + 0.35 * helped).clamp(0.0, 1.0);

            // **The other gate: not everybody can do everything.**
            //
            // Ability is partly inherited and partly not, and how much of
            // that is the genes and how much is growing up in a house with
            // books in it is the oldest argument in the subject. The model
            // takes no side and keeps the pull weak: most of the draw is
            // the child's own.
            let free =
                ((self.rng.next_f32() + self.rng.next_f32() + self.rng.next_f32()) / 3.0) as f64;
            let aptitude = (0.72 * free + 0.28 * household.aptitude).clamp(0.0, 1.0);

            // **Grades, and this is where the real mechanism lives.**
            //
            // The first version gated on money at eighteen and gave a 20x
            // gap between a rich child and a poor one, where the real gap
            // in entry to higher education is about **2x** (England, by
            // area: ~28% of the least advantaged fifth against ~57% of the
            // most). A fee is not what does the damage.
            //
            // What does is the sixteen years before it. Feinstein's work on
            // the 1970 British Cohort found children of deprived families
            // who tested well at 22 months were on average overtaken by
            // higher-status children before primary school — the strong
            // form of that crossover has since been challenged, so the
            // model takes the defensible part: **what a child can show at
            // sixteen is their own ability plus what was done for them**.
            //
            // A state that funds schools is buying back part of that
            // difference, which is the whole argument for funding them —
            // and it does it by **making grades track ability instead of
            // money**, not by handing everybody better grades. Adding a
            // funding bonus to attainment put two thirds of a country
            // through university; a school system does not raise the mean,
            // it decides what the mean is made of.
            let on_money = 0.30 * (1.0 - helped) + 0.08 * helped;
            let attained = ((1.0 - on_money) * aptitude + on_money * afford).clamp(0.0, 1.0);

            // **Two gates, and both must pass.** Below the floor a course
            // is not merely unlikely, it is out of reach — no amount of
            // money finishes a degree for somebody who cannot do the work.
            // Above it, the odds climb with the grades, and money still
            // buys a little: fees, maintenance, and three years of not
            // earning.
            let odds = |q: Qualification| -> f64 {
                let floor = q.takes_to_finish();
                if attained < floor {
                    return 0.0;
                }
                let headroom = ((attained - floor) / (1.0 - floor)).clamp(0.0, 1.0);
                ((0.30 + 0.70 * headroom) * (0.62 + 0.38 * afford)).clamp(0.0, 1.0)
            };

            // Scaled so the cohort lands near the real 38% entering higher
            // education once both gates have taken their cut.
            let qualification = if roll < 1.33 * odds(Qualification::Degree) {
                Qualification::Degree
            } else if attained >= Qualification::Vocational.takes_to_finish()
                && roll < 1.33 * odds(Qualification::Degree) + 0.26
            {
                // An apprenticeship is paid, so it is far less gated by
                // what a family has — which is exactly why it is the route
                // for people a degree is out of reach for. It has a floor
                // of its own, and somebody below both leaves at sixteen.
                Qualification::Vocational
            } else {
                Qualification::School
            };

            let market = household.market;
            let first = FIRST[(self.rng.next_f32() * FIRST.len() as f32) as usize % FIRST.len()];
            let last = LAST[(self.rng.next_f32() * LAST.len() as f32) as usize % LAST.len()];
            // Work this town has, that they are qualified for.
            let trade = draw_work(&mut self.rng, qualification, &posts[market]);
            let mut p = Person::new(format!("{first} {last}"), trade, market, 30.0);
            p.qualification = qualification;
            p.age_years = 16.0 + qualification.years_to_earn();
            p.diligence =
                ((self.rng.next_f32() + self.rng.next_f32() + self.rng.next_f32()) / 3.0) as f64;
            p.aptitude = aptitude;
            let h = draw_household(&mut self.rng);
            p.household_share = household_share_for(h.adults());
            // **A child knows who it came from.** The first kinship link
            // in the model, and it is what probate walks on a death.
            p.parents.push(parent);
            if let Some(other) = self.people[parent].spouse {
                p.parents.push(other);
            }
            let stands_for = self.represents.get(parent.slot()).copied().unwrap_or(1.0);
            self.settle(p, h, stands_for);
        }
    }

    /// One day for everybody.
    ///
    /// **The economy steps once, not once per person.** Running the
    /// economy inside the loop would let each person trade against a
    /// slightly different world and quietly break conservation.
    pub fn live_a_day(&mut self, econ: &mut Economy, day: u64) {
        self.live_a_day_bounded(econ, day, true)
    }

    /// The same day, with the supply of promotions optionally ignored.
    ///
    /// **Public for the reason `biota::settle` and `person::live_a_day`
    /// are**: a population correlation cannot show a mechanism, so the
    /// only way to test that advancement is bounded by vacancies is to
    /// run the same cohort twice and vary that one thing. Passing `false`
    /// is the bug this rule exists to prevent — promotion on time served
    /// alone, with nobody to supervise — and is not a mode the game runs
    /// in.
    pub fn live_a_day_bounded(&mut self, econ: &mut Economy, day: u64, bounded: bool) {
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

        // **Tell the economy how good its workforce actually is.**
        //
        // `person.rs` has had a full skill model since it was written —
        // levels on a quadratic anchored at ten thousand hours, a ceiling
        // from aptitude, and rust for what goes unused — and its only
        // consumer was the person's own wage. Production had never heard
        // of it, so a town of masters made exactly what a town of novices
        // made. This is the join, and it runs before the day's work so a
        // works produces at the skill of the people who turned up.
        //
        // **A sample, weighted by what each stands for**, which is the
        // same discipline the rest of this file keeps: an individuated
        // person represents some thousands of real ones, and averaging
        // them unweighted would let a town's rare trades outvote its
        // common ones. Trades nobody in the sample works are left absent,
        // and `Economy::hands_at` reads that as the level the trade wants
        // — the honest answer when the sample cannot say.
        //
        // **And the sample speaks only for its share of a trade.** Two
        // sampled labourers in a town whose works give labouring a third of
        // its posts are two people standing for a small part of that third,
        // not for all of it: the rest of the labourers are real, experienced
        // and unsampled. Averaging over whoever happened to be drawn let
        // three newcomers to a trade halve the skill a works ran at — which
        // a planner moving people into trades made visible at once, and
        // which replacing the dead had been doing slowly all along. So what
        // the sample does not cover is worked at the ordinary level, by the
        // same honesty as a trade nobody sampled at all.
        let posts = crate::occupation::jobs_by_occupation(econ);
        {
            let trades = Trade::ALL.len();
            let mut sum = vec![0.0f64; n_markets * trades];
            let mut weight = vec![0.0f64; n_markets * trades];
            let mut heads = vec![0usize; n_markets * trades];
            let mut on_the_floor = vec![0.0f64; n_markets];
            for (id, p) in self.people.iter() {
                if p.market >= n_markets {
                    continue;
                }
                let stands_for = self
                    .represents
                    .get(id.slot())
                    .copied()
                    .unwrap_or(1.0)
                    .max(0.0);
                let i = p.market * trades + p.trade.index();
                sum[i] += p.competence() as f64 * stands_for;
                weight[i] += stands_for;
                heads[i] += 1;
                if p.trade != Trade::Supervisor {
                    on_the_floor[p.market] += stands_for;
                }
            }
            for m in 0..n_markets {
                let floor_posts: f64 = Trade::ALL
                    .iter()
                    .filter(|&&t| t != Trade::Supervisor)
                    .map(|t| posts[m][t.index()])
                    .sum();
                for (t, &trade) in Trade::ALL.iter().enumerate() {
                    let i = m * trades + t;
                    if weight[i] <= 1e-9 {
                        continue;
                    }
                    let sampled = sum[i] / weight[i];
                    let post_share = if floor_posts > 1e-9 {
                        posts[m][t] / floor_posts
                    } else {
                        0.0
                    };
                    let cover = if trade == Trade::Supervisor || post_share <= 1e-9 {
                        // Nothing to say it is a small part of anything.
                        1.0
                    } else {
                        (weight[i] / on_the_floor[m].max(1e-9) / post_share).min(1.0)
                    };
                    // **And for only as much as that many people can say.**
                    // Three sampled farm hands are three people, and three
                    // people cannot tell anybody that a town's thousands of
                    // farm hands are unusually good: their average swings
                    // by half a level from one draw to the next, and a
                    // works' output swung with it. Split into thirty-odd
                    // occupations the sample holds two or three people a
                    // trade, and seed 7's depots landed 5% more imports
                    // because two dockers happened to be good at it.
                    //
                    // So the reading is trusted in proportion to the people
                    // behind it — the ordinary shrinkage of a small
                    // sample's mean toward what is expected.
                    let n = heads[i] as f64;
                    let trust = n / (n + A_SAMPLE_IS_THIS_MANY_PEOPLE_SHORT);
                    let ordinary = crate::econ::ORDINARY_HAND;
                    econ.set_hands(m, trade, ordinary + cover * trust * (sampled - ordinary));
                }
            }
        }

        let mut vacancy = vec![0.0f64; n_markets];
        for m in 0..n_markets {
            let mine: Vec<&Person> = self.people.values().filter(|p| p.market == m).collect();
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
            // **How many posts are going, not whether any are.**
            //
            // This was a boolean, so on any day a vacancy existed *every*
            // eligible person in the town was made up at once. It went
            // unnoticed while the sample was mostly shop workers, who
            // reach the threshold rarely and at scattered times — and then
            // the population was given its real trade mix, offices and
            // public service came in at a third of it, and those work 85%
            // of weekdays rather than 28%. They cleared the threshold
            // together and a town came out 27% supervisors against 9% of
            // its posts.
            //
            // A vacancy is a number of posts. Counting them down as they
            // are filled is the whole of the fix.
            vacancy[m] = (mine.len() as f64 * share - bosses).max(0.0);
        }

        let everyone: Vec<Id<Person>> = self.people.ids().collect();
        let mut outcomes: std::collections::BTreeMap<Id<Person>, Day> = Default::default();
        for i in everyone {
            if self.people[i].condition <= 0.0 {
                continue;
            }
            let m = self.people[i].market;
            let free = !bounded || vacancy.get(m).copied().unwrap_or(0.0) >= 1.0;
            let was = self.people[i].trade;
            let worked_before = self.people[i].days_worked;
            // A week's contract that ends today ends in a day of rest, not
            // in a day of looking for work and finding none.
            let a_week_ending = matches!(self.people[i].state, State::Working { until } if day >= until)
                && self.people[i].job.as_ref().is_some_and(|j| j.days > 1.0);
            live_a_day_with(&mut self.people[i], econ, day, free);
            if was != Trade::Supervisor && self.people[i].trade == Trade::Supervisor {
                if let Some(v) = vacancy.get_mut(m) {
                    *v -= 1.0;
                }
            }

            // **What the day came to**, which is all a plan learns from.
            let p = &self.people[i];
            let outcome = if !p.alive() {
                Day::Neither
            } else if p.days_worked > worked_before
                || (matches!(p.state, State::Working { .. }) && p.job.is_some())
            {
                Day::Worked
            } else if matches!(p.state, State::Idle) && !a_week_ending {
                Day::Looked
            } else {
                // On the road between towns.
                Day::Neither
            };
            let (trade, market) = (p.trade, p.market);
            let offset = mix(&[i.slot() as u64, 0xD41F7]);
            self.people[i]
                .planner
                .observe(day, trade, market, outcome, offset);
            outcomes.insert(i, outcome);
        }
        self.bury_the_dead(day, &posts);
        self.plan_ahead(econ, day, &outcomes, &posts);
        // A year turns.
        if day > 0 && day.is_multiple_of(crate::econ::DAYS_PER_YEAR) {
            self.a_year_passes(econ, day);
        }
    }

    /// **What a town's openings do to the people in it** — spec A4.2, A4.7
    /// and A4.8, in that order.
    ///
    /// 1. **Where the openings are.** The town's posts by trade, against
    ///    how many of the sample work each; a trade with a whole post more
    ///    than people in it, after the holds already on it, has an opening.
    ///    The sample stands for the town, so its shares are held against
    ///    the town's.
    /// 2. **Word of it goes out**, pushed and never scanned: somebody who
    ///    worked that trade today tells a few people they know, and says
    ///    how often they get work at it. Where nobody in the sample does
    ///    the work, the employer puts a notice up, and whoever was looking
    ///    today reads it.
    /// 3. **Those with a reason think again**, as many as the town's think
    ///    budget allows, the rest tomorrow — and an opening is held for
    ///    whoever sets out after it.
    ///
    /// Supervising is not on offer here: it is promotion, and it has its
    /// own count of vacancies.
    fn plan_ahead(
        &mut self,
        econ: &Economy,
        day: u64,
        outcomes: &std::collections::BTreeMap<Id<Person>, Day>,
        posts: &[[f64; crate::occupation::N_OCCUPATIONS]],
    ) {
        let n_markets = econ.markets.len();
        let trades = Trade::ALL.len();

        let mut in_town: Vec<Vec<Id<Person>>> = vec![Vec::new(); n_markets];
        for (id, p) in self.people.iter() {
            if p.alive() && p.market < n_markets {
                in_town[p.market].push(id);
            }
        }

        // **Holds lapse.** Taken up, given up, dead, or simply out of time.
        let people = &self.people;
        self.reservations.retain(|r| {
            r.until > day
                && people.holds(r.who)
                && people[r.who].trade != r.trade
                && people[r.who].planner.trying_for(r.market) == Some(r.trade)
        });

        // ---- 1. where the openings are, before anybody's hold ----------
        let mut room = vec![vec![0.0f64; trades]; n_markets];
        for m in 0..n_markets {
            let floor: Vec<Id<Person>> = in_town[m]
                .iter()
                .copied()
                .filter(|&id| self.people[id].trade != Trade::Supervisor)
                .collect();
            let total: f64 = Trade::ALL
                .iter()
                .filter(|&&t| t != Trade::Supervisor)
                .map(|t| posts[m][t.index()])
                .sum();
            if total <= 1e-9 || floor.is_empty() {
                continue;
            }
            for t in Trade::ALL {
                if t == Trade::Supervisor {
                    continue;
                }
                let expected = posts[m][t.index()] / total * floor.len() as f64;
                let holders = floor.iter().filter(|&&id| self.people[id].trade == t).count();
                room[m][t.index()] = expected - holders as f64;
            }
        }
        let held = |rs: &[Reservation], m: usize, t: Trade| {
            rs.iter().filter(|r| r.market == m && r.trade == t).count() as f64
        };

        // ---- 2. word goes out ------------------------------------------
        for m in 0..n_markets {
            if in_town[m].len() < 2 {
                continue;
            }
            for t in Trade::ALL {
                if t == Trade::Supervisor
                    || room[m][t.index()] - held(&self.reservations[..], m, t) < 1.0
                {
                    continue;
                }
                let tellers: Vec<Id<Person>> = in_town[m]
                    .iter()
                    .copied()
                    .filter(|&id| {
                        self.people[id].trade == t && outcomes.get(&id) == Some(&Day::Worked)
                    })
                    .collect();
                let (how, audience, chance_told, teller) = if tellers.is_empty() {
                    let readers: Vec<Id<Person>> = in_town[m]
                        .iter()
                        .copied()
                        .filter(|&id| {
                            self.people[id].trade != t
                                && outcomes.get(&id) == Some(&Day::Looked)
                        })
                        .collect();
                    (Heard::Notice, readers, None, None)
                } else {
                    let teller =
                        tellers[(mix(&[day, m as u64, t.index() as u64]) % tellers.len() as u64)
                            as usize];
                    let listeners: Vec<Id<Person>> = in_town[m]
                        .iter()
                        .copied()
                        .filter(|&id| id != teller && self.people[id].trade != t)
                        .collect();
                    let chance = self.people[teller].planner.expectation();
                    (Heard::WordOfMouth, listeners, Some(chance), Some(teller))
                };
                if audience.is_empty() {
                    continue;
                }
                let mut told: Vec<Id<Person>> = Vec::new();
                for j in 0..TOLD_A_DAY {
                    let pick = audience[(mix(&[day, m as u64, t.index() as u64, j as u64 + 1])
                        % audience.len() as u64) as usize];
                    if !told.contains(&pick) {
                        told.push(pick);
                    }
                }
                // **And the people closest to the teller hear first.**
                if let Some(spouse) = teller.and_then(|w| self.people[w].spouse) {
                    if self.people.holds(spouse)
                        && self.people[spouse].market == m
                        && self.people[spouse].trade != t
                        && !told.contains(&spouse)
                    {
                        told.push(spouse);
                    }
                }
                for r in told {
                    let p = &self.people[r];
                    if p.qualification < qualification_for(t) {
                        continue;
                    }
                    // A notice says a post is going and nothing about the
                    // work; the reader can only guess from how this town
                    // has treated them.
                    let chance = chance_told.unwrap_or_else(|| p.planner.expectation());
                    let worth = planner::worth_to(p, econ, t, chance);
                    let now = planner::worth_to(p, econ, p.trade, p.planner.expectation());
                    let lead = Lead {
                        trade: t,
                        market: m,
                        chance,
                        heard: day,
                        expires: day + planner::LEAD_LIFE_DAYS,
                        how,
                    };
                    self.people[r].planner.hear(lead, worth, now);
                }
            }
        }

        // ---- 3. those with a reason think again ------------------------
        for m in 0..n_markets {
            let waiting: Vec<Id<Person>> = in_town[m]
                .iter()
                .copied()
                .filter(|&id| self.people[id].planner.pending.is_some())
                .collect();
            let allowed = planner::thinks_allowed(in_town[m].len());
            for id in planner::take_turns(&waiting, allowed, day) {
                let mut mind = std::mem::take(&mut self.people[id].planner);
                let reservations = &mut self.reservations;
                let room = &room;
                mind.reconsider(&self.people[id], econ, day, |mk, t| {
                    let open = room.get(mk).map(|r| r[t.index()]).unwrap_or(0.0);
                    hold_an_opening(reservations, id, mk, t, open, day)
                });
                self.people[id].planner = mind;
            }
        }
    }

    /// **One way into the sample**, so the arrays that run alongside the
    /// arena follow the slot it chose rather than being pushed blindly.
    ///
    /// A birth used to `push` onto all three at once, which is only right
    /// while nothing is ever removed: once a death frees a slot, the
    /// arena reuses it and a pushed household lands at the end, against
    /// nobody.
    fn settle(&mut self, p: Person, h: Household, represents: f64) -> Id<Person> {
        let id = self.people.add(p);
        let slot = id.slot();
        if slot == self.households.len() {
            self.households.push(h);
            self.represents.push(represents);
        } else {
            self.households[slot] = h;
            self.represents[slot] = represents;
        }
        id
    }

    /// **Where a dead person's estate goes.**
    ///
    /// Until now it went nowhere: the deceased was overwritten and their
    /// replacement handed fifty out of the air. Money was destroyed at one
    /// end of the sample and created at the other, and nothing caught it,
    /// because a person's pocket is not yet inside the money ledger.
    ///
    /// **Intestate succession**, which is what applies to about two thirds
    /// of Americans — **67% die without a will**. Every state runs
    /// essentially the same ladder, and it is the one asked for here:
    ///
    /// 1. the **spouse**;
    /// 2. failing that, the **surviving children**, in equal shares;
    /// 3. failing that, the **parents**;
    /// 4. failing all of it, the estate **escheats to the state**.
    ///
    /// Escheat is genuinely rare, because most people have somebody — but
    /// unclaimed property is not: US states are holding something like
    /// **$70bn** of it. There is no estate tax here and that is realistic:
    /// the federal exemption is about $13.6M, so it touches roughly one
    /// estate in a thousand and none of these.
    ///
    /// **The house and the vehicle go with it**, to whoever takes the
    /// largest share — which is the consequence worth having, because an
    /// heir who inherits a house stops paying rent.
    /// **Public so a test can hold everything still and vary one
    /// thing**, the same reason `biota::settle` and `person::live_a_day`
    /// are. A ladder with four rungs cannot be checked by running six
    /// years and hoping the right deaths happen.
    pub fn probate(&mut self, who: Id<Person>) {
        let estate = self.people[who].money;
        let house = self.people[who].housing;
        let conveyance = self.people[who].conveyance;

        // The ladder, stopping at the first rung with anybody living on
        // it. A handle only counts if it still resolves — which is the
        // whole reason identity had to become durable before this could
        // be written at all.
        let widow = self.people[who].spouse.filter(|s| self.people.holds(*s));
        let spouse: Vec<Id<Person>> = widow.into_iter().collect();
        let heirs = if !spouse.is_empty() {
            spouse
        } else {
            let children: Vec<Id<Person>> = self
                .people
                .iter()
                .filter(|(_, p)| p.parents.contains(&who))
                .map(|(i, _)| i)
                .collect();
            if !children.is_empty() {
                children
            } else {
                self.people[who]
                    .parents
                    .iter()
                    .copied()
                    .filter(|p| self.people.holds(*p))
                    .collect()
            }
        };

        // **Widowhood.** The survivor stops being married before the
        // estate moves, so nobody is left holding a handle to a dead
        // spouse — the arena would catch it, but a model that knows
        // somebody is widowed is better than one that finds out by
        // failing a lookup.
        if let Some(widow) = widow {
            self.people[widow].spouse = None;
        }

        if heirs.is_empty() {
            // **Escheat.** Recorded rather than evaporated: a number that
            // vanishes is a number nobody can check.
            self.escheated += estate;
            return;
        }

        let share = estate / heirs.len() as f64;
        for &h in &heirs {
            self.people[h].money += share;
        }
        // The principal heir takes the roof and the wheels.
        let first = heirs[0];
        if house == Housing::Owned && self.people[first].housing != Housing::Owned {
            self.people[first].housing = Housing::Owned;
        }
        if self.people[first].conveyance == Conveyance::OnFoot {
            self.people[first].conveyance = conveyance;
        }
        self.inherited += estate;
    }

    /// **Somebody who starves is replaced, because the town has not
    /// shrunk.**
    ///
    /// The cohort is a sample of a population that the economy is still
    /// counting in full. Letting the sample dwindle would make a town look
    /// emptier the longer it was watched, which is an artefact of the
    /// sampling and not a fact about the town.
    fn bury_the_dead(&mut self, day: u64, posts: &[[f64; crate::occupation::N_OCCUPATIONS]]) {
        let everyone: Vec<Id<Person>> = self.people.ids().collect();
        for i in everyone {
            if self.people[i].condition > 0.0 {
                continue;
            }
            self.gone.push((self.people[i].name.clone(), day));
            let market = self.people[i].market;
            let first = FIRST[(self.rng.next_f32() * FIRST.len() as f32) as usize % FIRST.len()];
            let last = LAST[(self.rng.next_f32() * LAST.len() as f32) as usize % LAST.len()];
            let mut p = Person::new(format!("{first} {last}"), Trade::Sales, market, 50.0);
            p.aptitude =
                ((self.rng.next_f32() + self.rng.next_f32() + self.rng.next_f32()) / 3.0) as f64;
            p.diligence =
                ((self.rng.next_f32() + self.rng.next_f32() + self.rng.next_f32()) / 3.0) as f64;
            // **A replacement is somebody else from the population, not a
            // school-leaver.** Drawing their qualification off the trade
            // meant every death diluted the country's skills, and the
            // degree share fell from 31% to 18% over twenty-five years for
            // no reason anybody had decided.
            let q = self.rng.next_f32();
            p.qualification = if q < 0.32 {
                Qualification::Degree
            } else if q < 0.52 {
                Qualification::Vocational
            } else {
                Qualification::School
            };
            // **The work this town has, among what they may do**, and the
            // years at it after the trade is known — settling the years
            // first put them against a trade the person then did not end
            // up in, which `seed` already records going wrong.
            let here = posts.get(market).copied().unwrap_or([0.0; crate::occupation::N_OCCUPATIONS]);
            p.trade = draw_work(&mut self.rng, p.qualification, &here);
            // A replacement is a cross-section of the living, not a
            // school leaver, so they bring their years with them.
            p.settle_into(p.trade);
            let h = draw_household(&mut self.rng);
            p.household_share = household_share_for(h.adults());
            // **Removed and replaced, not overwritten.** The slot is
            // reused — that is what keeps the sample the same size — but
            // the generation moves, so a handle to the deceased stops
            // resolving instead of quietly naming their successor.
            // Whoever moves in stands for the same number of real
            // people the deceased did — the town has not shrunk.
            // **Probate first, while the deceased still resolves.** After
            // the slot is reused there is nobody to read an estate off.
            self.probate(i);
            let stands_for = self.represents.get(i.slot()).copied().unwrap_or(1.0);
            self.people.remove(i);
            self.settle(p, h, stands_for);
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
            .values()
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
            .values()
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
/// **How people live**, in the real British proportions: one person 30%,
/// a couple 27%, a couple with children 22%, a lone parent 10%, and about
/// 11% sharing or family. A lone parent carries a household alone, so it
/// counts as one adult.
fn draw_household(rng: &mut Rng) -> Household {
    let r = rng.next_f32();
    if r < 0.30 {
        Household::Alone
    } else if r < 0.79 {
        // Couple, with or without children: two adults on one rent either
        // way, and children do not pay rent.
        Household::Couple
    } else if r < 0.89 {
        // A lone parent is one adult carrying a household, which is
        // exactly why lone parents are the poorest household type there
        // is.
        Household::Alone
    } else if r < 0.95 {
        Household::Shared(3)
    } else {
        Household::Family(2)
    }
}

/// **A trade from the work a town actually has**, among what a
/// qualification allows, in proportion to its posts.
///
/// Supervising is never drawn: it is what a floor hand is promoted to. A
/// town with no posts to read — a hand-built fixture with no works in it —
/// falls back on national shares, still gated by qualification.
fn draw_work(rng: &mut Rng, qualification: Qualification, posts: &[f64; crate::occupation::N_OCCUPATIONS]) -> Trade {
    let open = |t: Trade| t != Trade::Supervisor && qualification >= qualification_for(t);
    let total: f64 = Trade::ALL
        .iter()
        .filter(|&&t| open(t))
        .map(|t| posts[t.index()].max(0.0))
        .sum();
    if total <= 1e-9 {
        // No employers to read: the United States' mix, among what they
        // may do.
        return draw_from(rng, qualification, crate::occupation::national_mix());
    }
    draw_from(rng, qualification, posts)
}

/// A trade in proportion to `weights`, among what a qualification allows.
fn draw_from(
    rng: &mut Rng,
    qualification: Qualification,
    weights: &[f64; crate::occupation::N_OCCUPATIONS],
) -> Trade {
    let open = |t: Trade| t != Trade::Supervisor && qualification >= qualification_for(t);
    let total: f64 = Trade::ALL
        .iter()
        .filter(|&&t| open(t))
        .map(|t| weights[t.index()].max(0.0))
        .sum();
    let r = rng.next_f32() as f64 * total;
    let mut at = 0.0;
    // Nothing at all open to them is not possible — every qualification
    // allows the work that needs none — but a fixture can have no posts.
    let mut last = Trade::Sales;
    for t in Trade::ALL {
        if !open(t) || weights[t.index()] <= 0.0 {
            continue;
        }
        at += weights[t.index()];
        last = t;
        if r < at {
            return t;
        }
    }
    last
}

