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
use scale_sim::person::{Housing, Rank, Trade};
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
    /// **The same, over heads**: idle hands over all hands. A national
    /// rate is this — a village at sixty per cent is not a city.
    unemployed_by_head: f64,
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
            name: "unemployment, by head",
            lo: 0.02,
            hi: 0.15,
            source: "idle hands over all hands; the way a national rate is counted",
            read: |r| r.unemployed_by_head,
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
    let mut trade_at: std::collections::BTreeMap<_, (Trade, Rank)> = Default::default();
    if let Some(folk) = g.folk.as_ref() {
        for (id, p) in folk.people.iter() {
            trade_at.insert(id, (p.trade, p.rank));
        }
    }
    let mut changes = Changes {
        pairs: vec![vec![0; Trade::ALL.len()]; Trade::ALL.len()],
        ..Default::default()
    };
    let final_year_from = days.saturating_sub(DAYS_PER_YEAR);
    let mut worked_final_year: std::collections::BTreeMap<_, u64> = Default::default();
    // **Each kind of works' books over the final year**: what came in and
    // what went out, by reason.
    let mut books: std::collections::BTreeMap<(String, bool, String), f64> = Default::default();
    // **And what went unpaid, by what it was for**, over the same year.
    let mut unpaid_year: std::collections::BTreeMap<&'static str, f64> = Default::default();

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

        if d > final_year_from {
            if let Some(e) = g.economy.as_ref() {
                use scale_sim::money::Account;
                for (why, v) in e.treasury.unpaid_why.iter() {
                    *unpaid_year.entry(why).or_default() += v;
                }
                for t in e.treasury.today.iter() {
                    if let Account::State(_) = t.to {
                        *books
                            .entry(("~State".into(), true, format!("{:?}", t.why)))
                            .or_default() += t.amount;
                    }
                    if let Account::State(_) = t.from {
                        *books
                            .entry(("~State".into(), false, format!("{:?}", t.why)))
                            .or_default() += t.amount;
                    }
                    if let Account::Firm(s) = t.to {
                        if let Some(site) = e.ledger.sites.get(s) {
                            *books
                                .entry((format!("{:?}", site.kind), true, format!("{:?}", t.why)))
                                .or_default() += t.amount;
                        }
                    }
                    if let Account::Firm(s) = t.from {
                        if let Some(site) = e.ledger.sites.get(s) {
                            *books
                                .entry((format!("{:?}", site.kind), false, format!("{:?}", t.why)))
                                .or_default() += t.amount;
                        }
                    }
                }
            }
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
                    match trade_at.insert(id, (p.trade, p.rank)) {
                        Some((was, was_rank)) if was != p.trade => {
                            // A supervisor who becomes a manager has moved up
                            // a rung, not changed work.
                            if p.trade == Trade::Manager && was_rank == Rank::Supervisor {
                                changes.given_a_branch += 1;
                            } else {
                                changes.switched += 1;
                                changes.pairs[was.index()][p.trade.index()] += 1;
                            }
                        }
                        Some((_, Rank::Hand)) if p.rank == Rank::Supervisor => {
                            changes.promoted += 1;
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
    idle_jobs(&g);
    firm_books(&books);
    println!(
        "
  owed and not paid over the final year, by what for:
"
    );
    for (why, v) in unpaid_year.iter() {
        println!("  {:<24} {:>10.2e}", why, v);
    }
    by_trade(
        &g,
        &worked_final_year,
        &changes,
        years,
        final_year_from,
        days,
    );
}

/// Changes of trade over the run, counted monthly.
#[derive(Default)]
struct Changes {
    /// Made up to supervisor: a promotion, not a change of work.
    promoted: u64,
    /// A supervisor given a branch to manage.
    given_a_branch: u64,
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
    println!(
        "
  by trade at the end:
"
    );
    println!(
        "  {:<16} {:>6} {:>12} {:>9} {:>9} {:>9} {:>10} {:>10}",
        "",
        "people",
        "worked, yr 5",
        "homeless",
        "run crews",
        "own home",
        "pay, food",
        "held, food"
    );
    // **What a day's pay and what they hold are worth in days of food**,
    // at the price of food where each person lives: the medians.
    let food_day = |m: usize| {
        g.economy
            .as_ref()
            .map(|e| e.price(m, Commodity::ProcessedFood) * scale_sim::person::FOOD_PER_DAY)
            .unwrap_or(1.0)
            .max(1e-9)
    };
    let median = |mut v: Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v.get(v.len() / 2).copied().unwrap_or(0.0)
    };
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
        let running = mine
            .iter()
            .filter(|(_, p)| p.rank == Rank::Supervisor)
            .count() as f64
            / mine.len() as f64;
        let crews = if t.crew().is_some() {
            format!("{:>8.1}%", running * 100.0)
        } else {
            "        -".to_string()
        };
        let owned = mine
            .iter()
            .filter(|(_, p)| p.housing == Housing::Owned)
            .count() as f64
            / mine.len() as f64;
        let (pay, held) = match g.economy.as_ref() {
            Some(e) => (
                median(
                    mine.iter()
                        .map(|(_, p)| {
                            scale_sim::person::day_rate_for(e, p.market, p) / food_day(p.market)
                        })
                        .collect(),
                ),
                median(
                    mine.iter()
                        .map(|(_, p)| p.money / food_day(p.market))
                        .collect(),
                ),
            ),
            None => (0.0, 0.0),
        };
        println!(
            "  {:<16} {:>5.1}% {:>11.1}% {:>8.1}% {} {:>8.1}% {:>10.1} {:>10.0}",
            t.name(),
            mine.len() as f64 / n * 100.0,
            worked.min(1.0) * 100.0,
            homeless * 100.0,
            crews,
            owned * 100.0,
            pay,
            held
        );
    }
    // **The rungs, against what they really are.** First-line supervisors
    // are 5.1% of US jobs and managers 6.9% (OEWS, May 2023).
    let supervisors = folk
        .people
        .values()
        .filter(|p| p.rank == Rank::Supervisor)
        .count() as f64;
    let managers = folk
        .people
        .values()
        .filter(|p| p.trade == Trade::Manager)
        .count() as f64;
    // And what this world's own employers staff, which is what the rungs
    // are filled against: each town's people over its jobs.
    let (mut crew_room, mut manager_room) = (0.0f64, 0.0f64);
    if let Some(e) = g.economy.as_ref() {
        let jobs = scale_sim::occupation::jobs_by_occupation(e);
        for (m, town) in jobs.iter().enumerate() {
            let total: f64 = town.iter().sum();
            let here = folk.people.values().filter(|p| p.market == m).count() as f64;
            if total <= 0.0 {
                continue;
            }
            for t in Trade::ALL {
                crew_room += here * town[t.index()] / total * t.supervisor_share();
            }
            manager_room += here * town[Trade::Manager.index()] / total;
        }
    }
    // **What went wrong, and who was covered for it.** Real: 4.16% of
    // insured vehicles have a collision claim in a year and 3.3% damage
    // somebody else (ISO, 2024), and about 14% of American drivers carry
    // no insurance at all.
    {
        let owners: Vec<_> = folk
            .people
            .values()
            .filter(|p| p.conveyance.price_in_wage_days() > 0.0)
            .collect();
        let all = folk.people.values().count().max(1) as f64;
        let n = owners.len().max(1) as f64;
        let covered = owners.iter().filter(|p| p.insured).count() as f64;
        let a_year = years.max(1) as f64;
        let mishaps: f64 = folk.people.values().map(|p| p.mishaps as f64).sum();
        let ruined: f64 = folk.people.values().map(|p| p.ruined_vehicles as f64).sum();
        println!(
            "
  vehicles: {:.0}% of the sample keep one, {:.1}% of those insured (real ~86%);          {:.1} mishaps a year per hundred owners, {:.0} vehicles lost for want of the repair",
            n / all * 100.0,
            covered / n * 100.0,
            mishaps / n / a_year * 100.0,
            ruined
        );
    }

    // **What the rent ladder did**, against the real one: 6.1% of American
    // renter households were filed on in 2016 and 2.3% were put out, so
    // about three notices in five end some other way (Eviction Lab).
    {
        let renting = folk
            .people
            .values()
            .filter(|p| p.housing != Housing::Owned)
            .count()
            .max(1) as f64;
        let a_year = years.max(1) as f64;
        let sum = |f: fn(&scale_sim::person::Person) -> u32| {
            folk.people.values().map(|p| f(p) as f64).sum::<f64>() / renting / a_year * 100.0
        };
        println!(
            "\n  the rent, a year: {:.1}% served notice, {:.1}% put out, {:.1}% worked it off          (real 6.1% filed on, 2.3% evicted)",
            sum(|p| p.notices),
            sum(|p| p.evictions),
            sum(|p| p.worked_off)
        );
    }

    // **Where people live, and what a house costs against pay.**
    let tenure =
        |h: Housing| folk.people.values().filter(|p| p.housing == h).count() as f64 / n * 100.0;
    if let Some(e) = g.economy.as_ref() {
        let (mut weight, mut years_of_pay) = (0.0f64, 0.0f64);
        for m in 0..e.markets.len() {
            let pay = scale_sim::person::day_rate(e, m, Trade::ProductionWorker) * 260.0;
            if pay > 0.0 {
                weight += e.markets[m].population;
                years_of_pay += e.markets[m].population * e.house_price(m) / pay;
            }
        }
        println!(
            "\n  housing: owned {:.1}%, rented {:.1}%, lodging {:.1}%, homeless {:.1}%; a house costs {:.1} years of a production worker's pay",
            tenure(Housing::Owned),
            tenure(Housing::Rented),
            tenure(Housing::Lodging),
            tenure(Housing::Homeless),
            years_of_pay / weight.max(1.0)
        );
    }
    println!(
        "\n  supervisors {:.1}% of the sample; this world's jobs have room for {:.1}% (real 5.1% of jobs)\n  managers {:.1}%; room for {:.1}% (real 6.9%)",
        supervisors / n * 100.0,
        crew_room / n * 100.0,
        managers / n * 100.0,
        manager_room / n * 100.0
    );
    // **What the reviews did**, a year at a time. All layoffs and
    // discharges together run 1.1% of jobs a month in the United States
    // (JOLTS, 2023-2025), which is the ceiling; federal removals for
    // misconduct about 0.35% a year, a protected workforce's floor. A
    // demotion is far rarer than a dismissal: 114 against 7,411 federally
    // in 2016 (GAO-18-48).
    // How the work actually spreads, among those on the books.
    {
        let mut perf: Vec<f64> = folk
            .people
            .values()
            .filter(|p| p.employment != scale_sim::person::Employment::None)
            .map(|p| p.performance())
            .collect();
        let mut stand: Vec<f64> = folk
            .people
            .values()
            .filter(|p| p.employment != scale_sim::person::Employment::None)
            .map(|p| p.standing)
            .collect();
        perf.sort_by(f64::total_cmp);
        stand.sort_by(f64::total_cmp);
        let q = |v: &[f64], f: f64| {
            v.get(((v.len() as f64 - 1.0) * f) as usize)
                .copied()
                .unwrap_or(0.0)
        };
        println!(
            "  performance on the books: p2 {:.2} p10 {:.2} p50 {:.2} p90 {:.2}; standing p2 {:.2} p10 {:.2} p50 {:.2} p90 {:.2}",
            q(&perf, 0.02), q(&perf, 0.10), q(&perf, 0.5), q(&perf, 0.9),
            q(&stand, 0.02), q(&stand, 0.10), q(&stand, 0.5), q(&stand, 0.9)
        );
    }
    let r = folk.reviews;
    let a_year = years.max(1) as f64;
    println!(
        "  reviews {:.0} a year: {:.1}% commended, {:.1}% warned; let go {:.1}% of the sample a year,          supervisors put back {:.1}% of them a year",
        r.held as f64 / a_year,
        r.commended as f64 / r.held.max(1) as f64 * 100.0,
        r.warned as f64 / r.held.max(1) as f64 * 100.0,
        r.let_go as f64 / n / a_year * 100.0,
        r.demoted as f64 / supervisors.max(1.0) / a_year * 100.0
    );
    let per_year = changes.switched as f64 / n / years.max(1) as f64;
    println!(
        "
  changed trade: {} times over {years} years, {:.1}% of the sample a year          (real occupational mobility runs roughly 10% a year); made supervisor {} times, given a branch {} times",
        changes.switched,
        per_year * 100.0,
        changes.promoted,
        changes.given_a_branch
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
    top.sort_by_key(|x| std::cmp::Reverse(x.0));
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
    let hands: f64 = e.workforce.iter().map(|w| w.hands).sum();
    r.unemployed_by_head = if hands > 0.0 {
        e.workforce
            .iter()
            .map(|w| w.hands * w.unemployment)
            .sum::<f64>()
            / hands
    } else {
        0.0
    };

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
        "day",
        "people",
        "hungry",
        "homeless",
        "worked",
        "unempl",
        "foodcov",
        "fp/fc",
        "fx",
        "hh money",
        "unpaid/mo"
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
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, z), v| {
                (a.min(v), z.max(v))
            });
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
        println!(
            "
  who holds the money:
"
        );
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
            println!(
                "  {:<14} {:>12.3e} {:>12.3e} {:>+12.3e}",
                name,
                s0,
                s1,
                s1 - s0
            );
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
    println!(
        "  money and tonnage conserved every day: yes (checked daily; a failure stops the run)"
    );
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

/// **Where the idle jobs are, and why.** For each kind of employer at the
/// end of the run: the jobs it is rated for, the people on today, how much
/// of its rating it actually ran, and what share of its payroll it could
/// meet — which separates a works with nothing to do from a works that
/// cannot pay the people to do it.
fn idle_jobs(g: &GameState) {
    let Some(e) = g.economy.as_ref() else { return };
    println!(
        "
  towns at the end:
"
    );
    println!(
        "  {:<18} {:>12} {:>12} {:>12} {:>8}",
        "", "people", "hands", "working", "unempl"
    );
    let grid = e.grid.capacity();
    for (m, w) in e.workforce.iter().enumerate() {
        // Where the idle jobs are: the three kinds of works furthest short
        // of their rated staff in this town.
        let mut short: std::collections::BTreeMap<String, f64> = Default::default();
        for (i, site) in e.ledger.sites.iter().enumerate() {
            if site.market != m {
                continue;
            }
            let Some(r) = site.recipe else { continue };
            let rated = match site.kind {
                scale_sim::econ::SiteKind::PowerPlant => grid,
                _ => site.throughput,
            };
            let posts =
                scale_sim::labour::rated_headcount(rated, scale_sim::econ::RECIPES[r].labour);
            let on = e.staff_today.get(i).copied().unwrap_or(0.0);
            *short.entry(format!("{:?}", site.kind)).or_default() += posts - on;
        }
        let mut worst: Vec<(String, f64)> = short.into_iter().collect();
        worst.sort_by(|a, b| b.1.total_cmp(&a.1));
        let worst: Vec<String> = worst
            .iter()
            .take(3)
            .map(|(k, v)| format!("{k} {:.0}k", v / 1e3))
            .collect();
        println!(
            "  {:<18} {:>12.0} {:>12.0} {:>12.0} {:>7.1}%  n{} {}",
            e.markets[m].name.chars().take(18).collect::<String>(),
            e.markets[m].population,
            w.hands,
            w.working,
            w.unemployment * 100.0,
            e.markets[m].nation,
            worst.join(", ")
        );
    }
    let mut rows: std::collections::BTreeMap<String, (f64, f64, f64, f64, f64)> =
        Default::default();
    let grid = e.grid.capacity();
    for (i, site) in e.ledger.sites.iter().enumerate() {
        let Some(r) = site.recipe else { continue };
        let rated = match site.kind {
            scale_sim::econ::SiteKind::PowerPlant => grid,
            _ => site.throughput,
        };
        let posts = scale_sim::labour::rated_headcount(rated, scale_sim::econ::RECIPES[r].labour);
        let on = e.staff_today.get(i).copied().unwrap_or(0.0);
        let met = e.payroll_met.get(i).copied().unwrap_or(1.0);
        let running = if rated > 0.0 && site.kind != scale_sim::econ::SiteKind::PowerPlant {
            site.ran / rated
        } else if site.ran > 0.0 {
            1.0
        } else {
            0.0
        };
        let x = rows.entry(format!("{:?}", site.kind)).or_default();
        x.0 += posts;
        x.1 += on;
        x.2 += running * posts;
        x.3 += met * posts;
        x.4 += 1.0;
    }
    println!(
        "
  idle jobs at the end, by kind of works:
"
    );
    println!(
        "  {:<14} {:>6} {:>12} {:>12} {:>8} {:>10}",
        "", "sites", "rated jobs", "on today", "running", "pay met"
    );
    for (kind, (posts, on, run, met, n)) in rows {
        if posts < 1.0 {
            continue;
        }
        println!(
            "  {:<14} {:>6.0} {:>12.0} {:>12.0} {:>7.0}% {:>9.0}%",
            kind,
            n,
            posts,
            on,
            run / posts * 100.0,
            met / posts * 100.0
        );
    }
}

/// **The final year's books for each kind of works**: income, and each
/// outgoing by reason, and what was left.
fn firm_books(books: &std::collections::BTreeMap<(String, bool, String), f64>) {
    let mut kinds: Vec<String> = books.keys().map(|k| k.0.clone()).collect();
    kinds.sort();
    kinds.dedup();
    println!(
        "
  the final year's books, by kind of works (money in, and out by reason):
"
    );
    println!(
        "  {:<14} {:>10} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>10}",
        "", "income", "payroll", "supply", "purchase", "freight", "tax", "profit", "left"
    );
    for k in kinds {
        let get = |inc: bool, why: &str| {
            books
                .get(&(k.clone(), inc, why.to_string()))
                .copied()
                .unwrap_or(0.0)
        };
        let income: f64 = books
            .iter()
            .filter(|(key, _)| key.0 == k && key.1)
            .map(|(_, v)| v)
            .sum();
        let out: f64 = books
            .iter()
            .filter(|(key, _)| key.0 == k && !key.1)
            .map(|(_, v)| v)
            .sum();
        if income + out <= 0.0 {
            continue;
        }
        println!(
            "  {:<14} {:>10.2e} {:>9.2e} {:>9.2e} {:>9.2e} {:>9.2e} {:>9.2e} {:>9.2e} {:>+10.2e}",
            k,
            income,
            get(false, "Payroll"),
            get(false, "Supply"),
            get(false, "Purchase"),
            get(false, "Freight"),
            get(false, "Tax"),
            get(false, "Profit"),
            income - out
        );
    }
}
