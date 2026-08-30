//! Who does the work, and what happens to them when there is none.
//!
//! Until now a city was a population and a set of works, with no
//! connection between them: the mill produced flour whether or not anybody
//! was employed to mill it, and a person looking for a job met an economy
//! that had never heard of employment. This is the join.
//!
//! **The point of building it is one causal chain.** A transformer fails,
//! so the mill has no power, so the mill does not run, so the people who
//! work at the mill are not working, so they cannot buy food. Every link
//! in that already existed except the last two, and without them a
//! blackout was an inconvenience to a stockpile rather than something that
//! happened to anybody.
//!
//! ## What is and is not modelled
//!
//! This models the labour market **for the trades the economy contains** —
//! farm, mill, cannery, mine, power station, depot, and the haulage
//! between them. It does not model a city's whole employment, and it must
//! not pretend to: six commodities is a thin slice of a real economy, and
//! the works here account for well under a percent of a city's labour
//! force. Reporting the rest as unemployed would be nonsense.
//!
//! So `unemployment` here means *within these trades*. A man who cannot
//! get a shift at the cannery is unemployed in the sense that matters to
//! him, and that is the sense the simulation can honestly speak to.

use crate::econ::{Economy, SiteKind, RECIPES};

/// Hours in a working year *(real)*. The OECD average is about 1,750;
/// the US runs 1,791 and Germany nearer 1,340. 1,800 is an honest figure
/// for an industrialising economy with a six-day week in the works.
pub const HOURS_PER_WORKING_YEAR: f64 = 1_800.0;

/// Unemployment a healthy labour market carries anyway *(real)*.
///
/// Nobody moves between jobs instantly, so even a booming economy runs at
/// four to six percent. This is the floor a market returns to, not a
/// target anybody achieves.
pub const NATURAL_UNEMPLOYMENT: f64 = 0.055;

/// How fast a workforce follows the jobs, per year.
///
/// **Slowly, and that is the whole point.** People do not leave the town
/// they grew up in the month the works closes; they wait, and draw down
/// savings, and hope it reopens. Real regional unemployment after a plant
/// closure persists for years, which is why a shock here has to outlive
/// the shock. At 35% a year a disturbance half-clears in about two.
const WORKFORCE_DRIFT_PER_YEAR: f64 = 0.35;

/// The labour market of one town, in the trades this economy has.
#[derive(Clone, Debug)]
pub struct Workforce {
    /// Posts the town's works would fill running flat out.
    pub posts: f64,
    /// Posts actually being worked today, from what the works actually
    /// ran. A mill with no power employs nobody today however many it
    /// employed last week.
    pub working: f64,
    /// People who follow these trades here. Follows `posts` slowly.
    pub hands: f64,
    /// Share of `hands` with nothing to do.
    pub unemployment: f64,
    /// What a day's work fetches here, relative to a market at its natural
    /// rate. Below 1 where hands are idle, above 1 where they are scarce.
    pub wage_index: f64,
}

impl Default for Workforce {
    fn default() -> Self {
        Workforce {
            posts: 0.0,
            working: 0.0,
            hands: 0.0,
            unemployment: NATURAL_UNEMPLOYMENT,
            wage_index: 1.0,
        }
    }
}

impl Workforce {
    /// Posts going begging today — what somebody looking for work is
    /// actually competing for.
    pub fn vacancies(&self) -> f64 {
        (self.working - self.employed()).max(0.0)
    }

    /// Hands with a post today.
    pub fn employed(&self) -> f64 {
        self.hands.min(self.working)
    }

    /// The chance somebody looking for work in these trades finds any.
    ///
    /// Rationing by availability and not only by price is what a slack
    /// labour market actually feels like: the wage on offer barely moves,
    /// and you simply do not get taken on.
    pub fn chance_of_work(&self) -> f64 {
        if self.hands <= 1e-9 {
            return 1.0;
        }
        (self.working / self.hands).clamp(0.0, 1.0)
    }
}

/// Headcount a works needs to run at the rate it is running.
///
/// Straight from the recipe's labour-hours, which are real: eight hours to
/// bring in a tonne of grain (US agriculture runs about nine), a fifth of
/// an hour to mill a tonne of flour, an hour and a bit to can it. That
/// spread is why a country industrialises off its farm labour.
fn hands_for(batches_per_day: f64, labour_hours_per_batch: f64) -> f64 {
    batches_per_day * labour_hours_per_batch * 365.0 / HOURS_PER_WORKING_YEAR
}

/// Bring every town's labour market up to date from what its works did
/// today. Runs inside the economy's day, after production.
pub fn update(econ: &mut Economy) {
    let n = econ.markets.len();
    // Markets can be added after the economy is built — folding several
    // nations into one world does exactly that — so the labour market
    // follows the market list rather than assuming it was sized once.
    if econ.workforce.len() != n {
        econ.workforce.resize(n, Workforce::default());
    }
    let mut posts = vec![0.0f64; n];
    let mut working = vec![0.0f64; n];

    let grid_capacity = econ.grid.capacity();
    for site in econ.ledger.sites.iter() {
        let Some(r) = site.recipe else { continue };
        let labour = RECIPES[r].labour;

        let (rated, actual) = match site.kind {
            // A power station's `throughput` is a sentinel meaning "as
            // much as the grid can carry", so headcount has to come off
            // the grid instead — reading it off the sentinel staffed one
            // coal station with four million people. And **staffing does
            // not follow load**: nobody sends half the shift home because
            // demand dipped. It is all of them or, when the plant is down,
            // none of them.
            SiteKind::PowerPlant => {
                let r = grid_capacity;
                (r, if site.ran > 0.0 { r } else { 0.0 })
            }
            // **A farm is not idle out of harvest.**
            //
            // Output collapses between harvests and employment does not:
            // the ground still has to be ploughed, sown and tended, and
            // the beasts fed. Tying headcount straight to tonnage put a
            // farming town at 89% unemployment for ten months of the year.
            // Real agricultural labour swings by something like a factor
            // of two between slack season and harvest, with extra hands
            // taken on to get the crop in — so a floor, not a collapse.
            SiteKind::Farm => {
                let floor = site.throughput * 0.45;
                (
                    site.throughput,
                    if site.powered {
                        site.ran.max(floor)
                    } else {
                        site.ran
                    },
                )
            }
            _ => (site.throughput, site.ran),
        };

        posts[site.market] += hands_for(rated, labour);
        working[site.market] += hands_for(actual, labour);
    }

    let drift = WORKFORCE_DRIFT_PER_YEAR / 365.0;
    for m in 0..n {
        let w = &mut econ.workforce[m];
        w.posts = posts[m];

        // **Nobody is hired and fired by the day.**
        //
        // Firms hoard labour through a short interruption: the shift is
        // kept on through a fortnight's stoppage because losing trained
        // hands costs more than paying them to sweep up. Tying employment
        // straight to today's output made a town flicker between full
        // employment and half idle from one day to the next, which is not
        // what a labour market does. Shedding takes weeks, and so does
        // taking people back on.
        const STICKINESS_DAYS: f64 = 21.0;
        let a = 1.0 / STICKINESS_DAYS;
        w.working = if w.working <= 0.0 && w.posts > 0.0 {
            working[m]
        } else {
            w.working * (1.0 - a) + working[m] * a
        };

        // A town's stock of tradesmen follows the work, slowly. The target
        // carries the natural rate because a market with exactly as many
        // hands as posts has nobody free to fill the next vacancy.
        let target = w.posts * (1.0 + NATURAL_UNEMPLOYMENT);
        if w.hands <= 0.0 {
            w.hands = target;
        } else {
            w.hands += (target - w.hands) * drift;
        }

        w.unemployment = if w.hands > 1e-9 {
            (1.0 - w.working / w.hands).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // **Wages sag when hands are idle, and not by much.**
        //
        // Nominal wages are famously sticky: a doubling of unemployment
        // does not halve anybody's pay, and pretending it does would be as
        // wrong as pretending nothing happens. What actually gives is
        // hiring, which is handled by rationing the work itself. This is
        // the smaller, second-order squeeze on top.
        let slack = (NATURAL_UNEMPLOYMENT / w.unemployment.max(1e-4)).powf(0.35);
        w.wage_index = slack.clamp(0.75, 1.40);
    }
}
