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

use crate::building::SPAN_OF_CONTROL;
use crate::econ::{Commodity, Economy, SiteKind, RECIPES};

/// Days over which pay catches up with the cost of living *(real)*.
///
/// Wages are reset annually in most of the world, and studies of nominal
/// rigidity put the typical adjustment at nine to eighteen months. Four
/// months is the fast end of honest, and keeps a shock legible inside a
/// single year of play.
const WAGE_CATCHUP_DAYS: f64 = 120.0;

/// What one person eats a day, in tonnes. Same real figure the markets and
/// `person` use: 0.40 t of processed food a year.
const FOOD_PER_DAY: f64 = 0.40 / 365.0;

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
    /// **Posts that are somebody being in charge of others**, as against
    /// posts on the floor. Real span of control is 8-15, so about a tenth
    /// of a workforce — and that is the whole supply of promotions there
    /// is. There is no ladder with room for everybody on it.
    pub supervisory_posts: f64,
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
    /// **The cost of living that wages are actually set against.**
    ///
    /// A slow average of what a day's food costs, not today's price.
    /// Nominal wages are famously sticky — they are renegotiated once a
    /// year, not every morning — so when prices jump, pay lags and the
    /// *real* wage falls. That lag is the whole mechanism by which a
    /// supply shock makes people poorer, and anchoring pay to the
    /// current price erased it: bread went up fivefold in a blackout and
    /// wages went up fivefold the same week, so nobody felt a thing.
    pub food_anchor: f64,
}

impl Default for Workforce {
    fn default() -> Self {
        Workforce {
            supervisory_posts: 0.0,
            posts: 0.0,
            working: 0.0,
            hands: 0.0,
            unemployment: NATURAL_UNEMPLOYMENT,
            wage_index: 1.0,
            food_anchor: 0.0,
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
    let mut supervisory = vec![0.0f64; n];

    let grid_capacity = econ.grid.capacity();
    for site in econ.ledger.sites.iter() {
        // **Shops employ people, and used to employ none.**
        //
        // Retail is around a tenth of all jobs in a developed economy
        // against about 1.5% in food manufacturing, so a model that
        // counted mills and canneries and not shops was counting the
        // small half. The headcount comes off the fixtures: somebody has
        // to work each till, fill each shelf and unload each lorry.
        if let Some(b) = &site.fitted {
            posts[site.market] += b.staff();
            // **Staffed to the trade it is doing today.**
            //
            // A supermarket has thirty checkouts and opens eight of them
            // on a wet Tuesday. Roughly a third of the floor's hours are
            // fixed and the rest are rostered against the till receipts,
            // which is exactly why shop work is part-time and the hours
            // are never guaranteed. The managers are in whether anybody
            // comes through the door or not.
            // **Against the checkouts, which is what selling is.**
            //
            // Summing every fixture that moves tonnage counted the same
            // goods twice — once through the loading bay on the way in and
            // once through a till on the way out — so a shop trading flat
            // out read as half idle, rostered two thirds of its people and
            // could not keep a cashier housed.
            let rated = b.staff_at(crate::building::Fixture::Till)
                / crate::building::Fixture::Till.staff()
                * crate::building::Fixture::Till.throughput_t();
            let busy = if rated > 0.0 { site.ran / rated } else { 0.0 };
            let stocked = Commodity::ALL
                .iter()
                .any(|&c| site.stock[c as usize] > 0.0);
            working[site.market] += if stocked { b.staff_today(busy) } else { 0.0 };
        }
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
                // The floor is high on purpose. `posts` is rated at the
                // *peak* of the harvest, and a farm keeps very nearly all
                // of its people the rest of the year — real agricultural
                // employment swings by something like a quarter between
                // slack season and harvest, not by half. A floor of 0.45
                // read as a permanent agricultural depression: a farm
                // labourer was turned away well over half the days of the
                // year and could not feed himself working full time.
                let floor = site.throughput * 0.80;
                (site.throughput, site.ran.max(floor))
            }
            _ => (site.throughput, site.ran),
        };

        // **Somebody has to see that the work is being done.**
        //
        // A works is not a heap of hands; it has chargehands over the
        // shifts and a manager over them, at a span of about ten. Leaving
        // them out understated industrial employment by a seventh and left
        // nowhere for anybody to be promoted to.
        let with_charge = |floor: f64| {
            if floor < 6.0 {
                floor
            } else {
                let sup = (floor / SPAN_OF_CONTROL).ceil();
                floor + sup + (sup / SPAN_OF_CONTROL).ceil().max(1.0)
            }
        };
        posts[site.market] += with_charge(hands_for(rated, labour));
        working[site.market] += with_charge(hands_for(actual, labour));
        // **The supply of promotions**, counted separately, because it is
        // not a share of employment — it is a fixed number of posts and
        // most people will never hold one.
        let floor = hands_for(rated, labour);
        if floor >= 6.0 {
            let sup = (floor / SPAN_OF_CONTROL).ceil();
            supervisory[site.market] += sup + (sup / SPAN_OF_CONTROL).ceil().max(1.0);
        }
    }

    let food_price: Vec<f64> = (0..n)
        .map(|m| econ.price(m, Commodity::ProcessedFood) * FOOD_PER_DAY)
        .collect();
    let drift = WORKFORCE_DRIFT_PER_YEAR / 365.0;
    for m in 0..n {
        let w = &mut econ.workforce[m];
        w.posts = posts[m];
        w.supervisory_posts = supervisory[m];

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

        // Pay follows the cost of living at a walk, not a run.
        let today = food_price[m];
        w.food_anchor = if w.food_anchor <= 0.0 {
            today
        } else {
            let a = 1.0 / WAGE_CATCHUP_DAYS;
            w.food_anchor * (1.0 - a) + today * a
        };
    }
}
