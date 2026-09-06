//! **Who is counted, and against what.**
//!
//! A denominator error is the easiest mistake in this whole project to
//! make and the hardest to see, because the result is a plausible
//! percentage. It has already happened twice:
//!
//! - `Workforce::hands` counts the trades the *works* employ and nothing
//!   else, and dividing the state and the private services by it gave
//!   employment shares over **100%**;
//! - the retail benchmark was taken as **14.1%**, which is a *combined*
//!   wholesale-and-retail figure, so calibrating shops against it would
//!   have replaced a tonnage error with a sector-boundary one.
//!
//! So the reporting is typed. **Every share names what it is a share
//! of**, and there is no bare `percentage_of` to reach for.
//!
//! # Four counts, and they are not each other
//!
//! ```text
//! jobs  ≠  employed people  ≠  full-time equivalents  ≠  paid hours
//! ```
//!
//! One person may hold two jobs; one job may be half a week. Retail is
//! where this bites hardest — around 60% of it is part-time — so its
//! share of *headcount* is materially larger than its share of *hours*,
//! and a model whose staffing comes out of labour-hours is producing the
//! second while the famous figures quote the first.

/// **What a share is a share of.** Carried with the number so a figure
/// cannot be quietly compared against the wrong base.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Base {
    Population,
    WorkingAge,
    LabourForce,
    EmployedPeople,
    Jobs,
    FullTimeEquivalents,
}

impl Base {
    pub fn name(self) -> &'static str {
        match self {
            Base::Population => "everybody",
            Base::WorkingAge => "working age",
            Base::LabourForce => "labour force",
            Base::EmployedPeople => "people in work",
            Base::Jobs => "jobs",
            Base::FullTimeEquivalents => "full-time equivalents",
        }
    }
}

/// A number that knows what it is a share of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Share {
    pub value: f64,
    pub of: Base,
}

impl Share {
    pub fn percent(&self) -> f64 {
        self.value * 100.0
    }
    /// **Comparable only against the same base.** Returns `None` rather
    /// than a misleading answer.
    pub fn compare(&self, other: &Share) -> Option<std::cmp::Ordering> {
        if self.of != other.of {
            return None;
        }
        self.value.partial_cmp(&other.value)
    }
}

/// **The counts, kept apart.**
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Census {
    pub population: f64,
    /// 16 to 64, which is about 64% of a developed population.
    pub working_age: f64,
    /// Working or looking for work. Real participation runs 45-55% of
    /// the whole population.
    pub labour_force: f64,
    pub employed_people: f64,
    /// **More than the people**, because some hold two. About 3.7% of
    /// Britons have a second job.
    pub jobs: f64,
    /// **Fewer than the people**, because many work part of a week.
    pub full_time_equivalents: f64,
    pub annual_paid_hours: f64,
}

impl Census {
    pub fn base(&self, of: Base) -> f64 {
        match of {
            Base::Population => self.population,
            Base::WorkingAge => self.working_age,
            Base::LabourForce => self.labour_force,
            Base::EmployedPeople => self.employed_people,
            Base::Jobs => self.jobs,
            Base::FullTimeEquivalents => self.full_time_equivalents,
        }
    }

    pub fn share(&self, n: f64, of: Base) -> Share {
        let b = self.base(of);
        Share { value: if b > 0.0 { n / b } else { 0.0 }, of }
    }

    /// **`Workforce::hands` is not a denominator.** It is what it says it
    /// is — labour available to the works this economy models — and using
    /// it as a base is the mistake this type exists to make difficult.
    /// There is deliberately no `Base` for it.
    pub fn of_a_nation(population: f64) -> Census {
        let working_age = population * 0.64;
        let labour_force = population * 0.50;
        let employed_people = labour_force * 0.95;
        Census {
            population,
            working_age,
            labour_force,
            employed_people,
            // 3.7% hold a second job.
            jobs: employed_people * 1.037,
            // Part-time work is 24% of British employment against an EU
            // average of 17%, so hours come to well under heads.
            full_time_equivalents: employed_people * 0.82,
            annual_paid_hours: employed_people * 0.82 * 1_750.0,
        }
    }
}

// ---------------------------------------------------------------------
// what the real figures actually cover
// ---------------------------------------------------------------------

/// **A published share, with the boundary it was drawn at.**
///
/// The reason this exists: "wholesale and retail trade; repair of motor
/// vehicles" is **one statistical section**, and at about 14-15% of
/// employment it is the figure everybody quotes. Shops are a little
/// under two thirds of it. Calibrating a shop model against the section
/// total would put roughly half as many people again behind a counter as
/// belong there, and would do it while looking correct.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Benchmark {
    pub what: &'static str,
    /// As a share of people in work, which is how these are published.
    pub of_employed: f64,
    /// What it does *not* include.
    pub excludes: &'static str,
}

/// The distribution sector, split at the boundaries the statistics
/// actually use. Approximate, and they vary by country and year.
pub const DISTRIBUTION: [Benchmark; 3] = [
    Benchmark {
        what: "retail — shops, what a household buys over a counter",
        of_employed: 0.091,
        excludes: "wholesale, warehousing, motor trade",
    },
    Benchmark {
        what: "wholesale — warehouses, distributors, brokers, business to business",
        of_employed: 0.036,
        excludes: "anything sold to a household",
    },
    Benchmark {
        what: "motor trade — sale and repair of vehicles",
        of_employed: 0.018,
        excludes: "everything else",
    },
];

/// What the three come to together, which is the number usually quoted
/// and the one this project had been aiming shops at.
pub fn whole_distribution_sector() -> f64 {
    DISTRIBUTION.iter().map(|b| b.of_employed).sum()
}

/// **Retail's share of hours is smaller than its share of heads.**
///
/// About 60% of retail work is part-time, so a model whose staffing is
/// computed from labour-hours should be aiming at this rather than at the
/// headcount figure — which is a second boundary error waiting behind the
/// first.
pub fn retail_share_of_fte() -> f64 {
    let headcount = DISTRIBUTION[0].of_employed;
    // 40% full, 60% at roughly half a week.
    headcount * (0.40 + 0.60 * 0.5)
}
