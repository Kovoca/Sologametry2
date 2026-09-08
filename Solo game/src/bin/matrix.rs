//! **Sixteen combinations, because four changes have six interactions.**
//!
//! Four behaviours were introduced together and starved a country: food
//! cover went from 17 days to 0.28, which drove the scarcity premium to
//! its ceiling, which put food at five times its cost, which took wages
//! with it, which halved the house-price-to-income ratio. Reverting was
//! right. Reintroducing them one at a time by intuition would not be —
//! the one that did the damage may not be the one that looks guiltiest,
//! and a pair may do something neither does alone.
//!
//! ```text
//! L  carrier deliveries update the landed average
//! M  price = marginal delivered + premium on the goods
//! S  buyers draw on the cheapest delivered supplier
//! T  a trader sells the market's surplus, capped at what closes the margin
//! ```
//!
//! `cargo run --release --bin matrix`

use scale_sim::econ::{Commodity, Doctrine, Economy, Experiments};
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

const FOOD: Commodity = Commodity::ProcessedFood;
const DAYS: usize = 400;

/// **Several nations, because one is not evidence.**
///
/// `LM` was the best of all sixteen cases on the first nation measured and
/// broke a gate on another — so a matrix run against a single country says
/// what happens *there*, and a change shipped on it is a change shipped on
/// a sample of one.
const RANKS: [usize; 4] = [0, 2, 4, 6];

struct Reading {
    label: String,
    lowest_cover: f64,
    median_cover: f64,
    food_price: f64,
    food_cost: f64,
    wage_a_year: f64,
    house_to_income: f64,
    food_made: f64,
    idle_for_inputs: usize,
    idle_for_room: usize,
    hungry_days: usize,
    conserves: bool,
}

fn main() {
    let world = World::generate(384, 216, 20260828);
    let pol = Polities::partition(&world, 30);
    let set = Settlements::place(&world, &pol, 5000);
    let net = Network::build(&world, &set, 1000);

    println!(
        "One nation, {DAYS} days, every combination of four changes.\n\
         L landed-from-carriers  M marginal-source pricing  \
         S cheapest-delivered supplier  T market-wide trade\n"
    );
    println!(
        "{:<6} {:>8} {:>8} {:>9} {:>9} {:>9} {:>7} {:>11} {:>5} {:>5} {:>7} cons",
        "case",
        "lo cvr",
        "med cvr",
        "food px",
        "food cost",
        "wage/yr",
        "hse/inc",
        "food made",
        "in?",
        "room",
        "hungry"
    );
    println!("{}", "-".repeat(110));

    let mut rows = Vec::new();
    for x in Experiments::matrix() {
        let mut worst: Option<Reading> = None;
        for rank in RANKS {
            let id = pol.ranked()[rank].0;
            let Some(r) = Region::extract(&world, &pol, &set, &net, id, 5, Doctrine::Prudent)
            else {
                continue;
            };
            let got = run(r.economy, x);
            // Worst is the one that would fail first: least cover, and a
            // leak beats everything.
            let replace = match &worst {
                None => true,
                Some(w) => {
                    !got.conserves && w.conserves
                        || got.conserves == w.conserves && got.lowest_cover < w.lowest_cover
                }
            };
            if replace {
                worst = Some(got);
            }
        }
        let Some(w) = worst else { continue };
        rows.push(w);
        let last = rows.last().unwrap();
        println!(
            "{:<6} {:>8.2} {:>8.2} {:>9.1} {:>9.1} {:>9.0} {:>7.2} {:>11.0} {:>5} {:>5} {:>7} {}",
            last.label,
            last.lowest_cover,
            last.median_cover,
            last.food_price,
            last.food_cost,
            last.wage_a_year,
            last.house_to_income,
            last.food_made,
            last.idle_for_inputs,
            last.idle_for_room,
            last.hungry_days,
            if last.conserves { "ok" } else { "LEAK" }
        );
    }

    // What each switch does on its own, against the baseline.
    let base = &rows[0];
    println!("\nAgainst the baseline (all off), one switch at a time:");
    for (bit, name) in [
        (1usize, "L landed"),
        (2, "M marginal"),
        (4, "S supplier"),
        (8, "T trade"),
    ] {
        let r = &rows[bit];
        println!(
            "  {name:<12} cover {:>7.2} -> {:>7.2}   food price {:>8.1} -> {:>8.1}   \
             wage {:>7.0} -> {:>7.0}   hungry days {} -> {}",
            base.lowest_cover,
            r.lowest_cover,
            base.food_price,
            r.food_price,
            base.wage_a_year,
            r.wage_a_year,
            base.hungry_days,
            r.hungry_days
        );
    }

    // Interactions: is the pair worse than either alone?
    println!("\nPairs, and whether the two together are worse than the worst of them alone:");
    for (a, an) in [(1usize, "L"), (2, "M"), (4, "S")] {
        for (b, bn) in [(2usize, "M"), (4, "S"), (8, "T")] {
            if b <= a {
                continue;
            }
            let (pa, pb, both) = (&rows[a], &rows[b], &rows[a | b]);
            let worst_alone = pa.lowest_cover.min(pb.lowest_cover);
            let flag = if both.lowest_cover < worst_alone * 0.8 {
                "  <-- worse together"
            } else {
                ""
            };
            println!(
                "  {an}+{bn}: cover {:>7.2} against {:>7.2} alone{flag}",
                both.lowest_cover, worst_alone
            );
        }
    }
}

fn run(mut e: Economy, x: Experiments) -> Reading {
    e.experiments = x;
    let mut hungry = 0usize;
    let mut food_made = 0.0f64;
    for _ in 0..DAYS {
        e.step();
        if e.unmet_demand[FOOD as usize] > 0.0 {
            hungry += 1;
        }
    }
    for entry in e.journal.entries() {
        if let scale_sim::econ::Event::Produced { commodity, qty, .. } = &entry.event {
            if *commodity == FOOD {
                food_made += qty;
            }
        }
    }

    let mut covers: Vec<f64> = (0..e.markets.len())
        .map(|m| {
            let held: f64 = (0..e.ledger.sites.len())
                .filter(|&s| e.ledger.sites[s].market == m)
                .map(|s| e.ledger.stock(s, FOOD))
                .sum();
            held / e.daily_draw(m, FOOD).max(1e-9)
        })
        .collect();
    covers.sort_by(f64::total_cmp);

    // Why anything that could run did not: no inputs, or nowhere to put
    // the output. The two are completely different problems and counting
    // them together is what let a market working be mistaken for a famine.
    let (mut no_inputs, mut no_room) = (0usize, 0usize);
    for s in 0..e.ledger.sites.len() {
        let site = &e.ledger.sites[s];
        if site.ran > 1e-9 || site.throughput <= 0.0 {
            continue;
        }
        let Some(r) = site.recipe else { continue };
        let rec = &scale_sim::econ::RECIPES[r];
        let starved = rec
            .inputs
            .iter()
            .any(|&(ic, q)| e.ledger.stock(s, ic) < q * site.throughput * 0.5);
        let full = rec
            .outputs
            .iter()
            .all(|&(oc, _)| site.capacity[oc as usize] - e.ledger.stock(s, oc) < 1e-6);
        if starved {
            no_inputs += 1;
        } else if full {
            no_room += 1;
        }
    }

    let wage = scale_sim::person::day_rate(&e, 0, scale_sim::person::Trade::Labourer) * 260.0;
    let conserves =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| e.ledger.assert_conserved()))
            .is_ok();

    Reading {
        label: x.label(),
        lowest_cover: covers[0],
        median_cover: covers[covers.len() / 2],
        food_price: e.price(0, FOOD),
        food_cost: e.markets[0].cost[FOOD as usize],
        wage_a_year: wage,
        house_to_income: e.house_price(0) / wage.max(1.0),
        food_made,
        idle_for_inputs: no_inputs,
        idle_for_room: no_room,
        hungry_days: hungry,
        conserves,
    }
}
