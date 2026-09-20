//! The rest of what people do for a living.
//!
//! The economy modelled farms, mills, canneries, mines, shops and hauliers,
//! and then the state. That is a great deal of the *goods* and about a
//! quarter of the *jobs*. Real employment by sector *(UK, ~33M jobs)*:
//!
//! | sector | share | here |
//! |---|---|---|
//! | health & social work | 13.3% | state |
//! | wholesale, retail, vehicle repair | 14.1% | shops |
//! | education | 8.9% | state |
//! | professional, scientific, technical | 8.9% | **missing** |
//! | administrative & support | 8.7% | **missing** |
//! | manufacturing | 7.6% | mill, cannery, butcher |
//! | **accommodation & food** | **6.8%** | **missing** |
//! | **construction** | **6.4%** | **missing** |
//! | transport & storage | 5.0% | hauliers |
//! | information & communication | 4.5% | **missing** |
//! | public administration & defence | 4.3% | state |
//! | finance & insurance | 3.4% | **missing** |
//! | **arts, entertainment, recreation** | **2.5%** | **missing** |
//! | other services | 2.6% | **missing** |
//! | agriculture, forestry, fishing | 1.1% | farms, pasture |
//! | utilities | 1.2% | power |
//! | mining & quarrying | 0.2% | collieries |
//!
//! What is missing comes to about **43% of all employment** — and it
//! includes the two sectors that decide what a place is like to live in
//! rather than merely to eat in: **somebody has to fix things**, and
//! somewhere has to be open in the evening.
//!
//! These are modelled the way the state's services are: **posts against
//! population, at real ratios**, because a service is consumed where the
//! people are and cannot be shipped. Nobody imports a haircut.

use crate::econ::Economy;

/// A block of private service employment.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Sector {
    /// **Construction — and repair.** About 6.4% of employment, and
    /// roughly *half of construction output is repair and maintenance*
    /// rather than new build. This is who fixes things when they break:
    /// the state's own crews mend the grid, and everything else — roofs,
    /// walls, drains, roads — is somebody's contract.
    Construction,
    /// **Accommodation and food** — pubs, cafés, hotels, and 6.8% of
    /// employment. Also **where the insecurity lives**: 28.8% of this
    /// workforce is on zero-hours contracts, against 2.1% in public
    /// administration.
    Hospitality,
    /// **Arts, entertainment and recreation.** Only 2.5% of employment and
    /// worth having anyway, because it is a large part of the difference
    /// between a place people live in and a place people work in.
    Recreation,
    /// Professional, technical, financial, administrative and support
    /// work — offices, in a word. The largest missing block at about 21%
    /// once its parts are added together, and the one the design doc
    /// warns is hardest to make feel like anything.
    Office,
    /// **Insurance** — the people who write the cover and, mostly, the
    /// people who settle what is claimed on it. Split out of the office
    /// block because it is the one service whose work can be *counted*
    /// rather than assumed: what an insurer employs follows the claims it
    /// has to process, which follows how much there is to go wrong.
    Insurance,
}

impl Sector {
    pub const ALL: [Sector; 5] = [
        Sector::Construction,
        Sector::Hospitality,
        Sector::Recreation,
        Sector::Office,
        Sector::Insurance,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Sector::Construction => "construction",
            Sector::Hospitality => "hospitality",
            Sector::Recreation => "recreation",
            Sector::Office => "offices",
            Sector::Insurance => "insurance",
        }
    }

    /// **Share of all employment** *(real, UK)*.
    pub fn share_of_employment(self) -> f64 {
        match self {
            Sector::Construction => 0.064,
            Sector::Hospitality => 0.068,
            Sector::Recreation => 0.025,
            // Professional and technical 8.9%, administrative and support
            // 8.7%, information 4.5%, finance 3.4%, real estate 1.6% —
            // offices, all of it, **less the insurance people**, who are
            // counted once and counted below.
            Sector::Office => 0.211 - INSURANCE_SHARE_OF_EMPLOYMENT,
            Sector::Insurance => INSURANCE_SHARE_OF_EMPLOYMENT,
        }
    }

    /// **How urban it is.**
    ///
    /// Not every sector spreads evenly. Offices concentrate in cities and
    /// barely exist in a village; construction and hospitality follow
    /// people wherever they are. Which is a real thing about where you
    /// have to move to in order to work at something.
    pub fn concentrates_in_cities(self) -> bool {
        self == Sector::Office
    }
}

/// **How many vehicles there are to a head** *(FHWA, Highway Statistics
/// 2024, Table MV-1: 297,525,836 registered against 341.8M people)*. The
/// exposure an insurer is actually covering.
pub const VEHICLES_A_HEAD: f64 = 0.87;

/// **How often one of them is claimed on in a year**: collision 4.16,
/// comprehensive 3.95, property-damage liability 2.50 and bodily-injury
/// 0.80 per hundred vehicles *(ISO, 2024)*.
pub const CLAIMS_A_VEHICLE_A_YEAR: f64 = 0.1141;

/// **How many claims one adjuster gets through in a year.** Derived, not
/// chosen: 324,230 claims adjusters, examiners and investigators *(OEWS
/// May 2025, 13-1031)* against something like 34 million vehicle claims
/// and 4.5 million on homes.
pub const CLAIMS_AN_ADJUSTER_A_YEAR: f64 = 110.0;

/// **And the clerks behind them**: 214,260 claims and policy processing
/// clerks against those adjusters *(OEWS May 2025, 43-9041)*.
pub const CLERKS_TO_AN_ADJUSTER: f64 = 0.66;

/// **Selling and underwriting it** is the other half of the industry:
/// 479,100 insurance sales agents and 105,420 underwriters *(OEWS May
/// 2025)* against roughly 430 million policies in force.
pub const POLICIES_TO_A_SELLER: f64 = 735.0;

/// What all of that comes to as a share of employment — **derived from the
/// chain above and not typed in**, which is the test: it lands at 0.64%
/// against a real 0.72% (1.12 million people in the four insurance
/// occupations against 155.5 million in work).
pub const INSURANCE_SHARE_OF_EMPLOYMENT: f64 = {
    let claims = VEHICLES_A_HEAD * CLAIMS_A_VEHICLE_A_YEAR;
    let adjusters = claims / CLAIMS_AN_ADJUSTER_A_YEAR;
    let claims_side = adjusters * (1.0 + CLERKS_TO_AN_ADJUSTER);
    // A policy on every vehicle and one on every household.
    let policies = VEHICLES_A_HEAD + 1.0 / 2.53;
    let selling_side = policies / POLICIES_TO_A_SELLER;
    (claims_side + selling_side) / 0.5
};

/// Private service posts in each market.
pub struct Services {
    pub posts: Vec<[f64; 5]>,
}

impl Services {
    /// **Posts against population**, because a service is consumed where
    /// the people are. Nobody imports a haircut.
    ///
    /// Roughly half a population is in work at any time — real
    /// participation runs 45-55% once the young, the old and the unwaged
    /// are taken out — so a sector's share of *employment* becomes a share
    /// of population by halving it.
    pub fn provide(econ: &Economy) -> Services {
        const IN_WORK: f64 = 0.5;
        /// Offices concentrate: a city has them and a market town has few.
        /// Real professional-services employment is roughly twice as
        /// concentrated in large cities as in small towns.
        const CITY: f64 = 250_000.0;

        let posts = econ
            .markets
            .iter()
            .map(|m| {
                let mut out = [0.0f64; 5];
                for (i, s) in Sector::ALL.iter().enumerate() {
                    let mut share = s.share_of_employment();
                    if s.concentrates_in_cities() {
                        // Half in a small town, full in a city, on a
                        // gentle curve between.
                        let urban = (m.population / CITY).min(1.0).sqrt();
                        share *= 0.5 + 0.5 * urban;
                    }
                    out[i] = m.population * IN_WORK * share;
                }
                out
            })
            .collect();
        Services { posts }
    }

    pub fn posts_in(&self, market: usize, sector: Sector) -> f64 {
        let i = Sector::ALL.iter().position(|&s| s == sector).unwrap_or(0);
        self.posts.get(market).map(|p| p[i]).unwrap_or(0.0)
    }

    pub fn total_in(&self, market: usize) -> f64 {
        self.posts
            .get(market)
            .map(|p| p.iter().sum())
            .unwrap_or(0.0)
    }

    /// What share of the working-age population all this employs.
    pub fn share_of_workforce(&self, econ: &Economy) -> f64 {
        const IN_WORK: f64 = 0.5;
        let people: f64 = econ.markets.iter().map(|m| m.population).sum();
        let posts: f64 = self.posts.iter().map(|p| p.iter().sum::<f64>()).sum();
        if people <= 0.0 {
            return 0.0;
        }
        posts / (people * IN_WORK)
    }
}
