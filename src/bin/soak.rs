//! **The whole world, switched on and left alone.**
//!
//!   cargo run --release --bin soak
//!   cargo run --release --bin soak -- --seed 7 --nations 4 --years 5 --each 40
//!
//! Every other diagnostic here composes a slice of the model by hand and
//! reports on it once, at the end. That is how each piece was built, and it
//! is exactly what cannot show the failure the owner named: systems that
//! each pass their own tests and **go wrong only when they are all on at
//! once**, often in the first weeks, while everything settles into each
//! other.
//!
//! So this does three things none of the others do:
//!
//! - **It runs the root.** `GameState` owns the clock and the day's order,
//!   so this is the world as it will actually run — several nations with a
//!   border, an exchange rate and a state each, and a sampled population
//!   living in them — rather than a pair of calls somebody remembered to
//!   put in the right order.
//! - **It commands nothing.** No transformer is failed by name, nothing is
//!   thrown away on cue, no event is scripted. Whatever happens, happened.
//! - **It watches over time.** A figure that is fine at the end of five
//!   years and absurd in month two is a real failure, and a report taken
//!   only at the end cannot see it.
//!
//! Every figure is read against a **real band**, taken from the calibration
//! this project already records, and the report says which ones leave it,
//! when, and whether they came back. **The bands are for reading, not yet
//! for failing**: a gate asserted before anybody has seen what the whole
//! world does would be a threshold fitted to a guess.

use scale_sim::econ::{Commodity, Doctrine, DAYS_PER_YEAR};
use scale_sim::game::GameState;
use scale_sim::money::Account;
use scale_sim::network::Network;
use scale_sim::person::{Housing, Trade};
use scale_sim::polity::Polities;
use scale_sim::populace::Populace;
use scale_sim::region::Nations;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

/// A figure read off the world at one moment.
#[derive(Clone, Copy, Default)]
struct Reading {
    day: u64,
    /// Sampled people alive.
    people: usize,
    /// Share of the sample who went hungry at any point this month.
    hungry: f64,
    /// Share of the sample with no roof tonight.
    homeless: f64,
    /// Share of person-days this month that were paid work.
    worked: f64,
    /// The workforce statistics' own unemployment, averaged over towns.
    unemployed: f64,
    /// Days of food held in the worst-supplied town.
    worst_food_cover: f64,
    /// Food's price against what it costs to make, across the world.
    food_price_to_cost: f64,
    /// What a unit of foreign money costs.
    exchange_rate: f64,
    /// Money households hold, across the world.
    household_money: f64,
    /// Money owed and not paid, this month.
    unpaid: f64,
    /// Prices that are not a number.
    broken_prices: usize,
    /// **Who holds the money.** Money is conserved, so anything that
    /// leaves households shows up somewhere else — and where it goes is
    /// the whole diagnosis.
    held: Holders,
}

#[derive(Clone, Copy, Default)]
struct Holders {
    households: f64,
    firms: f64,
    services: f64,
    states: f64,
    abroad: f64,
    banks: f64,
}

/// **What a figure ought to look like, and where that comes from.**
struct Band {
    name: &'static str,
    lo: f64,
    hi: f64,
    source: &'static str,
    read: fn(&Reading) -> f64,
    percent: bool,
}

fn bands() -> Vec<Band> {
    vec![
        Band {
            name: "went hungry this month",
            lo: 0.0,
            hi: 0.05,
            source: "a working economy absorbs ordinary failures; hunger should be rare",
            read: |r| r.hungry,
            percent: true,
        },
        Band {
            name: "homeless",
            lo: 0.0,
            hi: 0.02,
            source: "UK rough sleeping and temporary accommodation, well under 1%",
            read: |r| r.homeless,
            percent: true,
        },
        Band {
            name: "days that were paid work",
            lo: 0.30,
            hi: 0.90,
            source: "cohort mixes full-time, part-time, casual and rota work",
            read: |r| r.worked,
            percent: true,
        },
        Band {
            name: "unemployment (workforce stats)",
            lo: 0.02,
            hi: 0.15,
            source: "developed economies 3-10%, a bad recession past 12",
            read: |r| r.unemployed,
            percent: true,
        },
        Band {
            name: "worst town's food cover (days)",
            lo: 2.0,
            hi: 400.0,
            source: "below a couple of days is a famine waiting for one bad week",
            read: |r| r.worst_food_cover,
            percent: false,
        },
        Band {
            name: "food price / cost",
            lo: 0.7,
            hi: 1.8,
            source: "a staple sells near its cost; 4x cost was the recorded famine",
            read: |r| r.food_price_to_cost,
            percent: false,
        },
        Band {
            name: "exchange rate",
            lo: 0.25,
            hi: 4.0,
            source: "the model clamps at 0.2 and 5.0; sitting on a clamp means it is broken",
            read: |r| r.exchange_rate,
            percent: false,
        },
    ]
}

fn main() {
    let mut seed = 7u64;
    let mut nations = 4usize;
    let mut years = 5u64;
    let mut each = 40usize;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => seed = it.next().and_then(|v| v.parse().ok()).unwrap_or(seed),
            "--nations" => nations = it.next().and_then(|v| v.parse().ok()).unwrap_or(4).max(1),
            "--years" => years = it.next().and_then(|v| v.parse().ok()).unwrap_or(5).max(1),
            "--each" => each = it.next().and_then(|v| v.parse().ok()).unwrap_or(40).max(4),
            "--help" | "-h" => {
                println!("usage: soak [--seed N] [--nations N] [--years N] [--each N]");
                return;
            }
            other => {
                eprintln!("unknown argument: {other:?}");
                std::process::exit(2);
            }
        }
    }

    println!("generating a world (seed {seed}, {nations} nations)...");
    let world = World::generate(384, 216, seed);
    let polities = Polities::partition(&world, 24);
    let settlements = Settlements::place(&world, &polities, 3000);
    let network = Network::build(&world, &settlements, 500);
    let n = Nations::build(
        &world,
        &polities,
        &settlements,
        &network,
        nations,
        4,
        Doctrine::Prudent,
    );
    let folk = Populace::seed(&n.economy, each, seed);
    let sampled = folk.people.len();
    {
        let n = sampled.max(1) as f64;
        let mix: Vec<String> = Trade::ALL
            .iter()
            .filter_map(|&t| {
                let k = folk.people.values().filter(|p| p.trade == t).count();
                (k > 0).then(|| format!("{} {:.0}%", t.name(), k as f64 / n * 100.0))
            })
            .collect();
        println!("the sample at the start: {}", mix.join(", "));
    }
    let markets = n.economy.markets.len();

    let mut g = GameState::new(seed).with_economy(n.economy).with_folk(folk);

    println!(
        "{markets} towns in {nations} nations, {sampled} sampled people. \
         Running {years} years, nothing commanded.\n"
    );

    let days = years * DAYS_PER_YEAR;
    let month = 30u64;
    let mut readings: Vec<Reading> = Vec::new();

    // Per-person counters at the start of the month, so a month's figure is
    // a month's rather than everything since the beginning.
    let mut worked_at: std::collections::BTreeMap<_, u64> = Default::default();
    let mut hungry_at: std::collections::BTreeMap<_, u64> = Default::default();
    let mut unpaid_at = 0.0f64;
    snapshot(&g, &mut worked_at, &mut hungry_at);

    // **Who does what, and who changed it.** A figure for the whole sample
    // cannot say whether the people out of work are stuck in a trade with
    // none, which is the question the planner exists to answer.
    let mut trade_at: std::collections::BTreeMap<_, Trade> = Default::default();
    if let Some(folk) = g.folk.as_ref() {
        for (id, p) in folk.people.iter() {
            trade_at.insert(id, p.trade);
        }
    }
    let mut changes = Changes {
        pairs: vec![vec![0; Trade::ALL.len()]; Trade::ALL.len()],
        ..Default::default()
    };
    let final_year_from = days.saturating_sub(DAYS_PER_YEAR);
    let mut worked_final_year: std::collections::BTreeMap<_, u64> = Default::default();

    for d in 1..=days {
        g.a_day();

        // **The hard invariants, every day.** These are not bands: a world
        // that creates money or tonnes out of nothing is not a world that
        // went wrong gradually, it is broken, and knowing which day it broke
        // is worth more than any average.
        if let Some(e) = g.economy.as_ref() {
            e.ledger.assert_conserved();
            e.treasury.assert_conserved();
        }

        if d == final_year_from {
            if let Some(folk) = g.folk.as_ref() {
                for (id, p) in folk.people.iter() {
                    worked_final_year.insert(id, p.days_worked);
                }
            }
        }

        if d % month == 0 || d == days {
            if let Some(folk) = g.folk.as_ref() {
                for (id, p) in folk.people.iter() {
                    match trade_at.insert(id, p.trade) {
                        Some(was) if was != p.trade => {
                            if p.trade == Trade::Supervisor {
                                changes.promoted += 1;
                            } else {
                                changes.switched += 1;
                                changes.pairs[was.index()][p.trade.index()] += 1;
                            }
                        }
                        _ => {}
                    }
                }
            }
            let r = read(&g, d, &worked_at, &hungry_at, unpaid_at);
            unpaid_at = g.economy.as_ref().map(|e| e.treasury.unpaid).unwrap_or(0.0);
            snapshot(&g, &mut worked_at, &mut hungry_at);
            readings.push(r);
        }
    }

    report(&readings, years);
    by_trade(&g, &worked_final_year, &changes, years, final_year_from, days);
}

/// Changes of trade over the run, counted monthly.
#[derive(Default)]
struct Changes {
    /// Made up to supervisor: a promotion, not a change of work.
    promoted: u64,
    /// Went to work at a different trade.
    switched: u64,
    /// From which trade to which.
    pairs: Vec<Vec<u64>>,
}

/// **The sample by trade at the end**, and how much of the final year each
/// trade actually worked.
fn by_trade(
    g: &GameState,
    worked_from: &std::collections::BTreeMap<scale_sim::id::Id<scale_sim::person::Person>, u64>,
    changes: &Changes,
    years: u64,
    from_day: u64,
    to_day: u64,
) {
    let Some(folk) = g.folk.as_ref() else { return };
    let span = to_day.saturating_sub(from_day).max(1) as f64;
    println!("
  by trade at the end:
");
    println!(
        "  {:<16} {:>6} {:>12} {:>9}",
        "", "people", "worked, yr 5", "homeless"
    );
    let n = folk.people.len().max(1) as f64;
    for t in Trade::ALL {
        let mine: Vec<_> = folk.people.iter().filter(|(_, p)| p.trade == t).collect();
        if mine.is_empty() {
            continue;
        }
        let worked: f64 = mine
            .iter()
            .map(|(id, p)| {
                let w0 = worked_from.get(id).copied().unwrap_or(p.days_worked);
                p.days_worked.saturating_sub(w0) as f64 / span
            })
            .sum::<f64>()
            / mine.len() as f64;
        let homeless = mine
            .iter()
            .filter(|(_, p)| p.housing == Housing::Homeless)
            .count() as f64
            / mine.len() as f64;
        println!(
            "  {:<16} {:>5.1}% {:>11.1}% {:>8.1}%",
            t.name(),
            mine.len() as f64 / n * 100.0,
            worked.min(1.0) * 100.0,
            homeless * 100.0
        );
    }
    let per_year = changes.switched as f64 / n / years.max(1) as f64;
    println!(
        "
  changed trade: {} times over {years} years, {:.1}% of the sample a year          (real occupational mobility runs roughly 10% a year); promoted {} times",
        changes.switched,
        per_year * 100.0,
        changes.promoted
    );
    // **How often people actually thought about it**, which A4.8's think
    // budget bounds, and how often thinking came to nothing.
    let thoughts: u64 = folk.people.values().map(|p| p.planner.thoughts).sum();
    let failed: u64 = folk.people.values().map(|p| p.planner.failures).sum();
    let took_up: u64 = folk.people.values().map(|p| p.planner.taken_up).sum();
    println!(
        "  thought again {:.1} times a person a year; {} plans to take up a trade          came to nothing and {} were carried out (people living at the end)",
        thoughts as f64 / n / years.max(1) as f64,
        failed,
        took_up
    );
    let mut top: Vec<(u64, usize, usize)> = Vec::new();
    for a in 0..scale_sim::occupation::N_OCCUPATIONS {
        for b in 0..scale_sim::occupation::N_OCCUPATIONS {
            if changes.pairs[a][b] > 0 {
                top.push((changes.pairs[a][b], a, b));
            }
        }
    }
    top.sort_by(|x, y| y.0.cmp(&x.0));
    for (count, a, b) in top.into_iter().take(8) {
        println!(
            "    {:>4}  {} -> {}",
            count,
            Trade::ALL[a].name(),
            Trade::ALL[b].name()
        );
    }
}

fn snapshot(
    g: &GameState,
    worked_at: &mut std::collections::BTreeMap<scale_sim::id::Id<scale_sim::person::Person>, u64>,
    hungry_at: &mut std::collections::BTreeMap<scale_sim::id::Id<scale_sim::person::Person>, u64>,
) {
    worked_at.clear();
    hungry_at.clear();
    if let Some(folk) = g.folk.as_ref() {
        for (id, p) in folk.people.iter() {
            worked_at.insert(id, p.days_worked);
            hungry_at.insert(id, p.days_hungry);
        }
    }
}

fn read(
    g: &GameState,
    day: u64,
    worked_at: &std::collections::BTreeMap<scale_sim::id::Id<scale_sim::person::Person>, u64>,
    hungry_at: &std::collections::BTreeMap<scale_sim::id::Id<scale_sim::person::Person>, u64>,
    unpaid_at: f64,
) -> Reading {
    let mut r = Reading {
        day,
        ..Default::default()
    };
    let (Some(e), Some(folk)) = (g.economy.as_ref(), g.folk.as_ref()) else {
        return r;
    };

    // ---- the people ---------------------------------------------------
    let mut worked_days = 0u64;
    let mut lived_days = 0u64;
    let mut hungry = 0usize;
    let mut homeless = 0usize;
    for (id, p) in folk.people.iter() {
        r.people += 1;
        // Somebody born or promoted into the sample this month has no
        // starting figure, and counts from nought rather than inventing one.
        let w0 = worked_at.get(&id).copied().unwrap_or(p.days_worked);
        let h0 = hungry_at.get(&id).copied().unwrap_or(p.days_hungry);
        worked_days += p.days_worked.saturating_sub(w0);
        lived_days += 30;
        if p.days_hungry > h0 {
            hungry += 1;
        }
        if p.housing == Housing::Homeless {
            homeless += 1;
        }
    }
    let n = r.people.max(1) as f64;
    r.hungry = hungry as f64 / n;
    r.homeless = homeless as f64 / n;
    r.worked = worked_days as f64 / lived_days.max(1) as f64;

    // ---- the economy --------------------------------------------------
    let towns = e.markets.len().max(1);
    r.unemployed = e.workforce.iter().map(|w| w.unemployment).sum::<f64>() / towns as f64;

    let food = Commodity::ProcessedFood as usize;
    // Read off the morning's position, which is what every decision that
    // day was taken on.
    r.worst_food_cover = match e.opening() {
        Some(o) => (0..e.markets.len())
            .map(|m| o.cover(m, Commodity::ProcessedFood))
            .filter(|c| c.is_finite())
            .fold(f64::INFINITY, f64::min),
        None => f64::INFINITY,
    };
    if !r.worst_food_cover.is_finite() {
        r.worst_food_cover = 0.0;
    }

    let (mut price, mut cost) = (0.0f64, 0.0f64);
    for m in e.markets.iter() {
        price += m.price[food];
        cost += m.cost[food];
    }
    r.food_price_to_cost = if cost > 1e-9 { price / cost } else { 0.0 };

    r.exchange_rate = e.exchange.foreign_money();
    r.household_money = (0..e.markets.len())
        .map(|m| e.treasury.balance(Account::Households(m)))
        .sum();
    r.unpaid = e.treasury.unpaid - unpaid_at;
    for (a, &v) in e.treasury.accounts() {
        match a {
            Account::Households(_) => r.held.households += v,
            Account::Firm(_) => r.held.firms += v,
            Account::ServiceSector(_) => r.held.services += v,
            Account::State(_) => r.held.states += v,
            Account::Abroad => r.held.abroad += v,
            _ => r.held.banks += v,
        }
    }
    r.broken_prices = e
        .markets
        .iter()
        .flat_map(|m| m.price.iter())
        .filter(|p| !p.is_finite())
        .count();
    r
}

fn report(readings: &[Reading], years: u64) {
    // ---- a quarter at a time, so a transient is visible -----------------
    println!(
        "{:>6} {:>6} {:>7} {:>8} {:>7} {:>7} {:>8} {:>7} {:>7} {:>11} {:>10}",
        "day", "people", "hungry", "homeless", "worked", "unempl", "foodcov", "fp/fc", "fx",
        "hh money", "unpaid/mo"
    );
    println!("{}", "-".repeat(98));
    for (i, r) in readings.iter().enumerate() {
        // Every month for the first six, where things settle, then quarterly.
        if i >= 6 && (i + 1) % 3 != 0 && i + 1 != readings.len() {
            continue;
        }
        println!(
            "{:>6} {:>6} {:>6.1}% {:>7.1}% {:>6.1}% {:>6.1}% {:>8.1} {:>7.2} {:>7.2} {:>11.3e} {:>10.2e}",
            r.day,
            r.people,
            r.hungry * 100.0,
            r.homeless * 100.0,
            r.worked * 100.0,
            r.unemployed * 100.0,
            r.worst_food_cover,
            r.food_price_to_cost,
            r.exchange_rate,
            r.household_money,
            r.unpaid,
        );
    }

    // ---- what left its band, when, and whether it came back -------------
    println!("\n{years} years against real bands:\n");
    let mut clean = true;
    for b in bands() {
        let outside: Vec<&Reading> = readings
            .iter()
            .filter(|r| {
                let v = (b.read)(r);
                !v.is_finite() || v < b.lo || v > b.hi
            })
            .collect();
        let show = |v: f64| {
            if b.percent {
                format!("{:.1}%", v * 100.0)
            } else {
                format!("{v:.2}")
            }
        };
        let first = readings.first().map(|r| (b.read)(r)).unwrap_or(0.0);
        let last = readings.last().map(|r| (b.read)(r)).unwrap_or(0.0);
        let (lo, hi) = readings
            .iter()
            .map(|r| (b.read)(r))
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, z), v| (a.min(v), z.max(v)));
        let verdict = if outside.is_empty() {
            "ok".to_string()
        } else {
            clean = false;
            let ends_outside = readings
                .last()
                .map(|r| {
                    let v = (b.read)(r);
                    !v.is_finite() || v < b.lo || v > b.hi
                })
                .unwrap_or(false);
            format!(
                "OUTSIDE {} of {} months, first on day {}{}",
                outside.len(),
                readings.len(),
                outside[0].day,
                if ends_outside {
                    ", and still out at the end"
                } else {
                    ", came back"
                }
            )
        };
        println!(
            "  {:<32} band {}..{}   start {}  range {}..{}  end {}   {}",
            b.name,
            show(b.lo),
            show(b.hi),
            show(first),
            show(lo),
            show(hi),
            show(last),
            verdict
        );
        println!("  {:<32} ({})", "", b.source);
    }

    // ---- where the money went -------------------------------------------
    //
    // **Conserved, so it went somewhere.** A drain out of one holder is a
    // rise in another, and reading that off is a measurement rather than a
    // theory about the cause.
    if let (Some(a), Some(z)) = (readings.first(), readings.last()) {
        println!("
  who holds the money:
");
        println!("  {:<14} {:>12} {:>12} {:>12}", "", "start", "end", "moved");
        let rows = [
            ("households", a.held.households, z.held.households),
            ("firms", a.held.firms, z.held.firms),
            ("services", a.held.services, z.held.services),
            ("states", a.held.states, z.held.states),
            ("abroad", a.held.abroad, z.held.abroad),
            ("banks, other", a.held.banks, z.held.banks),
        ];
        for (name, s0, s1) in rows {
            println!("  {:<14} {:>12.3e} {:>12.3e} {:>+12.3e}", name, s0, s1, s1 - s0);
        }
        let net: f64 = rows.iter().map(|r| r.2 - r.1).sum();
        println!("  {:<14} {:>12} {:>12} {:>+12.3e}", "net", "", "", net);
        println!(
            "
  (net should be what banks lent less what was repaid; a large figure
                with no bank lending is money appearing from nowhere)"
        );
    }

    // ---- the things that are simply broken ------------------------------
    let broken: usize = readings.iter().map(|r| r.broken_prices).max().unwrap_or(0);
    let lost_people = readings.first().map(|r| r.people).unwrap_or(0) as i64
        - readings.last().map(|r| r.people).unwrap_or(0) as i64;
    println!();
    println!(
        "  prices that are not a number: {}",
        if broken == 0 {
            "none".to_string()
        } else {
            clean = false;
            format!("{broken} at worst")
        }
    );
    println!(
        "  sampled people: {} at the start, {} at the end",
        readings.first().map(|r| r.people).unwrap_or(0),
        readings.last().map(|r| r.people).unwrap_or(0)
    );
    if lost_people != 0 {
        println!(
            "    (the sample replaces the dead, so a moving figure here is itself worth a look)"
        );
    }
    println!("  money and tonnage conserved every day: yes (checked daily; a failure stops the run)");
    println!();
    println!(
        "{}",
        if clean {
            "Nothing left its band."
        } else {
            "Some figures left their band — see above. These are readings, not failures: \
             nothing here is asserted until the whole world has been seen."
        }
    );
}
