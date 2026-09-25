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
    let mut supply = false;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => seed = it.next().and_then(|v| v.parse().ok()).unwrap_or(seed),
            "--nations" => nations = it.next().and_then(|v| v.parse().ok()).unwrap_or(4).max(1),
            "--years" => years = it.next().and_then(|v| v.parse().ok()).unwrap_or(5).max(1),
            "--each" => each = it.next().and_then(|v| v.parse().ok()).unwrap_or(40).max(4),
            "--supply-answers-price" => supply = true,
            "--help" | "-h" => {
                println!(
                    "usage: soak [--seed N] [--nations N] [--years N] [--each N] \
                     [--supply-answers-price]"
                );
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
    let mut n = Nations::build(
        &world,
        &polities,
        &settlements,
        &network,
        nations,
        4,
        Doctrine::Prudent,
    );
    n.economy.experiments.supply_answers_price = supply;
    if supply {
        println!("a works makes only as much as is worth making");
    }
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
    // **And what went unpaid, by what it was for and who owed it**, over
    // the same year: a firm short for its supplies and a household short
    // at the till are different failures with different cures.
    let mut unpaid_year: std::collections::BTreeMap<(&'static str, &'static str), f64> =
        Default::default();
    // **And between firms, which kind of works failed to pay which.** A
    // total against "firms" cannot say whether it is a mill short for its
    // grain or a hospital short for its medicine.
    let mut unpaid_works: std::collections::BTreeMap<(&'static str, String, String), f64> =
        Default::default();
    // **What is owed, as against what failed**: the obligations book at the
    // start of the final year, so the year's billing, collection and
    // charge-offs can be read as differences, and what households held back
    // from their shopping to meet bills they knew were coming.
    let mut book_at_year: Option<(
        f64,
        f64,
        f64,
        std::collections::BTreeMap<scale_sim::credit::Origin, f64>,
    )> = None;
    let mut held_back_year = 0.0;
    // **What each kind of works is billed and bills, whether or not the
    // money moved.** A cash book hides a works billed twice what it takes
    // in: the half it cannot pay is a shortfall on the day's tally and never
    // reaches its books. Paid and failed, by who was to be paid.
    let mut accrual: std::collections::BTreeMap<String, [f64; ACCRUAL]> = Default::default();
    // **And the arithmetic the prices set**: at the day's prices, what a
    // works running at its rating pays for its inputs, and for everything,
    // against what its output fetches — sampled monthly over the final year.
    let mut at_prices: std::collections::BTreeMap<String, [f64; 6]> = Default::default();
    // What each commodity fetched and cost, against its reference, over the
    // same samples: which prices are off, and whether it is the cost or the
    // scarcity on top of it.
    let mut quoted: std::collections::BTreeMap<String, [f64; 11]> = Default::default();

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
                held_back_year += e.held_back_today;
            }
            if let Some(e) = g.economy.as_ref() {
                use scale_sim::money::Account;
                for ((from, to, why), v) in e.treasury.unpaid_by.iter() {
                    let who = match from {
                        Account::Firm(_) => "firms",
                        Account::Households(_) => "households",
                        Account::ServiceSector(_) => "service sector",
                        Account::State(_) => "the state",
                        _ => "other",
                    };
                    *unpaid_year.entry((why, who)).or_default() += v;
                    if let Account::Firm(s) = from {
                        let payee = match to {
                            Account::Firm(t) => e
                                .ledger
                                .sites
                                .get(*t)
                                .map(|x| format!("{:?}", x.kind))
                                .unwrap_or_default(),
                            other => other.name(),
                        };
                        let payer = e
                            .ledger
                            .sites
                            .get(*s)
                            .map(|x| format!("{:?}", x.kind))
                            .unwrap_or_default();
                        *unpaid_works.entry((why, payer, payee)).or_default() += v;
                    }
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
        if d > final_year_from {
            if let Some(e) = g.economy.as_ref() {
                for t in e.treasury.today.iter() {
                    let name = scale_sim::money::reason_name(t.why);
                    book_accrual(e, &mut accrual, t.from, t.to, name, t.amount, false);
                }
                for ((from, to, why), v) in e.treasury.unpaid_by.iter() {
                    book_accrual(e, &mut accrual, *from, *to, why, *v, true);
                }
                // **What was owed at the moment of billing**, rather than the
                // settlement that pays it later: counting both would bill it
                // twice, and counting only the settlement books it on the
                // day somebody found the money.
                for ((debtor, creditor, origin), b) in e.bills_today.iter() {
                    if b.owed <= 0.0 {
                        continue;
                    }
                    let reason = match origin.name() {
                        "wages" => "payroll",
                        other => other,
                    };
                    book_accrual(e, &mut accrual, *debtor, *creditor, reason, b.owed, true);
                }
                if d % month == 0 {
                    price_arithmetic(e, &mut at_prices, &mut quoted);
                }
            }
        }
        if d == final_year_from {
            if let Some(e) = g.economy.as_ref() {
                let b = &e.obligations;
                book_at_year = Some((
                    b.billed,
                    b.collected,
                    b.charged_off,
                    b.charged_off_by.clone(),
                ));
            }
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
        "\n  failed and forgotten over the final year — shortfalls on the day's tally,
  owed by nobody the next morning — by what for and who failed:\n"
    );
    for ((why, who), v) in unpaid_year.iter() {
        println!("  {:<14} {:<16} {:>10.2e}", why, who, v);
    }
    println!("\n  and between firms, the ten largest, by who failed to pay whom:\n");
    let mut works: Vec<_> = unpaid_works.iter().collect();
    works.sort_by(|a, b| b.1.total_cmp(a.1));
    for ((why, payer, payee), v) in works.into_iter().take(10) {
        println!("  {:<10} {:<16} -> {:<18} {:>10.2e}", why, payer, payee, v);
    }
    obligations(&g, book_at_year, held_back_year);
    margins(&accrual, &at_prices, &quoted);
    by_trade(
        &g,
        &worked_final_year,
        &changes,
        years,
        final_year_from,
        days,
    );
}

/// Columns of a kind of works' accrual book: what it was billed to its
/// customers and what it was billed for inputs, power, payroll, carriage and
/// goods from abroad — each as paid and as failed on the day's tally.
const ACCRUAL: usize = 12;

/// **Book one payment, or one shortfall, to the works on either end.**
///
/// A cost is sorted by who was to be paid rather than by the reason, because
/// a power station and a mill are both paid under "supply". Profit, tax and
/// capital are not operating costs and are left out; so is capital coming
/// back to a firm, which is not revenue.
fn book_accrual(
    e: &scale_sim::econ::Economy,
    accrual: &mut std::collections::BTreeMap<String, [f64; ACCRUAL]>,
    from: Account,
    to: Account,
    reason: &str,
    amount: f64,
    failed: bool,
) {
    let kind = |s: usize| e.ledger.sites.get(s).map(|x| format!("{:?}", x.kind));
    // A settlement pays a bill already booked when it was owed.
    let skip = matches!(
        reason,
        "profit" | "tax" | "capital" | "lending" | "repayment" | "interest" | "settlements"
    );
    if skip {
        return;
    }
    let f = failed as usize;
    if let Account::Firm(s) = to {
        if let Some(k) = kind(s) {
            accrual.entry(k).or_insert([0.0; ACCRUAL])[f] += amount;
        }
    }
    if let Account::Firm(s) = from {
        let col = if reason == "payroll" {
            Some(6)
        } else {
            match to {
                Account::Firm(t) => {
                    let power = e
                        .ledger
                        .sites
                        .get(t)
                        .map(|x| x.kind == scale_sim::econ::SiteKind::PowerPlant)
                        .unwrap_or(false);
                    Some(if power { 4 } else { 2 })
                }
                Account::ServiceSector(_) => Some(8),
                Account::Abroad => Some(10),
                _ => None,
            }
        };
        if let (Some(c), Some(k)) = (col, kind(s)) {
            accrual.entry(k).or_insert([0.0; ACCRUAL])[c + f] += amount;
        }
    }
}

/// **At the day's prices, does a works running at its rating cover what it
/// buys?** Each works with a recipe, making something, that is not an
/// importer: its output at the price a firm is paid (a power station is paid
/// the clearing price itself), its inputs at the price a firm pays, and its
/// whole planned outlay — inputs, power and payroll at rating.
///
/// Columns: output value, inputs, planned outlay, works sampled, works whose
/// inputs alone cost more than the output fetches, works whose whole outlay
/// does.
fn price_arithmetic(
    e: &scale_sim::econ::Economy,
    at_prices: &mut std::collections::BTreeMap<String, [f64; 6]>,
    quoted: &mut std::collections::BTreeMap<String, [f64; 11]>,
) {
    use scale_sim::econ::{demand_rate_of, Economy, SiteKind, RECIPES};
    for (m, market) in e.markets.iter().enumerate() {
        for &c in Commodity::ALL.iter() {
            let (price, cost) = (market.price[c as usize], market.cost[c as usize]);
            if !price.is_finite() || !cost.is_finite() {
                continue;
            }
            let row = quoted.entry(format!("{c:?}")).or_insert([0.0; 11]);
            row[0] += price / c.base_cost();
            row[1] += cost / c.base_cost();
            row[2] += if cost > 0.0 { price / cost } else { 0.0 };
            row[3] += 1.0;
            // **Cover against the target the price aims at, and the most
            // cover the sheds that are counted could hold.** A commodity
            // whose counted storage cannot hold its target is priced as
            // scarce however much of it there is; one whose working stock
            // alone exceeds the target sits on the floor. The same sheds the
            // price counts: only shops, for a thing households buy and no
            // works uses.
            let cover = market.cover[c as usize];
            if c == Commodity::Electricity || !cover.is_finite() || cover <= 1e-9 {
                continue;
            }
            let industrial: f64 = e
                .ledger
                .sites
                .iter()
                .filter(|site| site.market == m)
                .filter_map(|site| {
                    let r = site.recipe?;
                    let per = RECIPES[r]
                        .inputs
                        .iter()
                        .find(|&&(ic, _)| ic == c)
                        .map(|&(_, q)| q)?;
                    Some(per * demand_rate_of(site))
                })
                .sum();
            let shop_only = c.per_capita_annual() > 0.0 && industrial <= 0.0;
            let counted = |site: &&scale_sim::econ::Site| {
                site.market == m && (!shop_only || site.kind == SiteKind::Shop)
            };
            let (held, room): (f64, f64) = e
                .ledger
                .sites
                .iter()
                .enumerate()
                .filter(|(_, site)| counted(site))
                .map(|(s, site)| (e.ledger.stock(s, c), site.capacity[c as usize]))
                .fold((0.0, 0.0), |a, b| (a.0 + b.0, a.1 + b.1));
            if held <= 1e-9 {
                continue;
            }
            let demand = held / cover;
            let target = e.target_cover(m, c);
            if target <= 1e-9 || demand <= 1e-12 {
                continue;
            }
            let seen = market.expected_cover[c as usize];
            row[4] += seen / target;
            row[5] += room / demand / target;
            row[6] += 1.0;
            // **An average over towns can hide a split** — plenty where it is
            // made and short everywhere else — so the towns short of their
            // target are counted, and each side's price read apart.
            let ratio = if cost > 0.0 { price / cost } else { 0.0 };
            if seen < target {
                row[7] += 1.0;
                row[8] += ratio;
            } else {
                row[9] += ratio;
            }
            row[10] += 1.0;
        }
    }
    for (s, site) in e.ledger.sites.iter().enumerate() {
        let Some(r) = site.recipe else { continue };
        let recipe = &RECIPES[r];
        if recipe.outputs.is_empty() || e.buys_abroad(s) {
            continue;
        }
        if matches!(
            site.kind,
            SiteKind::Shop | SiteKind::Hospital | SiteKind::Builders
        ) {
            continue;
        }
        let rate = demand_rate_of(site);
        if rate <= 1e-9 {
            continue;
        }
        let m = site.market;
        let paid_at = if site.kind == SiteKind::PowerPlant {
            1.0
        } else {
            Economy::WHOLESALE_MARGIN
        };
        let out: f64 = recipe
            .outputs
            .iter()
            .map(|&(c, q)| q * rate * e.markets[m].price[c as usize] * paid_at)
            .sum();
        let inputs: f64 = recipe
            .inputs
            .iter()
            .map(|&(c, q)| q * rate * e.markets[m].price[c as usize] * Economy::WHOLESALE_MARGIN)
            .sum();
        let outlay = e.planned_outlay(s);
        let row = at_prices
            .entry(format!("{:?}", site.kind))
            .or_insert([0.0; 6]);
        row[0] += out;
        row[1] += inputs;
        row[2] += outlay;
        row[3] += 1.0;
        if inputs > out {
            row[4] += 1.0;
        }
        if outlay > out {
            row[5] += 1.0;
        }
    }
}

/// **What each kind of works was billed against what it billed**, over the
/// final year, and what the day's prices say it should have been.
fn margins(
    accrual: &std::collections::BTreeMap<String, [f64; ACCRUAL]>,
    at_prices: &std::collections::BTreeMap<String, [f64; 6]>,
    quoted: &std::collections::BTreeMap<String, [f64; 11]>,
) {
    println!(
        "\n  the final year, by kind of works: billed to its customers against billed
  to it, paid or not (failed = a shortfall on the day's tally, or owed on the
  book when it was billed)\n"
    );
    println!(
        "                    revenue  of it   inputs    power  payroll carriage   abroad  of costs  costs /"
    );
    println!(
        "                     billed  failed   billed   billed   billed   billed   billed    failed  revenue"
    );
    for (k, a) in accrual.iter() {
        let rev = a[0] + a[1];
        let cost: f64 = (2..ACCRUAL).map(|i| a[i]).sum();
        let failed: f64 = (2..ACCRUAL).step_by(2).map(|i| a[i + 1]).sum();
        if rev + cost <= 0.0 {
            continue;
        }
        println!(
            "  {:<16} {:>9.2e} {:>6.0}% {:>8.2e} {:>8.2e} {:>8.2e} {:>8.2e} {:>8.2e} {:>8.0}% {:>8.2}",
            k,
            rev,
            if rev > 0.0 { a[1] / rev * 100.0 } else { 0.0 },
            a[2] + a[3],
            a[4] + a[5],
            a[6] + a[7],
            a[8] + a[9],
            a[10] + a[11],
            if cost > 0.0 { failed / cost * 100.0 } else { 0.0 },
            if rev > 0.0 { cost / rev } else { f64::INFINITY },
        );
    }
    println!("\n  at the day's prices, sampled monthly, a works running at its rating:\n");
    println!("                  inputs /  outlay /   works    inputs alone   whole outlay");
    println!("                    output    output  sampled   cost more      costs more");
    for (k, a) in at_prices.iter() {
        if a[3] <= 0.0 {
            continue;
        }
        println!(
            "  {:<14} {:>9.2} {:>9.2} {:>8.0} {:>11.0}% {:>13.0}%",
            k,
            if a[0] > 0.0 {
                a[1] / a[0]
            } else {
                f64::INFINITY
            },
            if a[0] > 0.0 {
                a[2] / a[0]
            } else {
                f64::INFINITY
            },
            a[3],
            a[4] / a[3] * 100.0,
            a[5] / a[3] * 100.0,
        );
    }
    println!("\n  and what each commodity fetched, over every town and the same samples:\n");
    println!("                  price /    cost /    price /    cover /  storage /  towns   price/cost  price/cost");
    println!("                reference reference     cost     target     target   short    if short   otherwise");
    for (k, a) in quoted.iter() {
        if a[3] <= 0.0 {
            continue;
        }
        let per = |x: f64| if a[6] > 0.0 { x / a[6] } else { f64::NAN };
        let short = a[7];
        let rest = a[10] - a[7];
        println!(
            "  {:<14} {:>9.2} {:>9.2} {:>9.2} {:>10.2} {:>10.2} {:>6.0}% {:>11.2} {:>11.2}",
            k,
            a[0] / a[3],
            a[1] / a[3],
            a[2] / a[3],
            per(a[4]),
            per(a[5]),
            if a[10] > 0.0 {
                short / a[10] * 100.0
            } else {
                f64::NAN
            },
            if short > 0.0 { a[8] / short } else { f64::NAN },
            if rest > 0.0 { a[9] / rest } else { f64::NAN },
        );
    }
}

/// **What is owed, and what became of what was owed**: the year's billing
/// on the book, what was collected and what was charged off by kind, and
/// what stands at the end by who owes it and what for.
fn obligations(
    g: &GameState,
    at_year: Option<(
        f64,
        f64,
        f64,
        std::collections::BTreeMap<scale_sim::credit::Origin, f64>,
    )>,
    held_back: f64,
) {
    let Some(e) = g.economy.as_ref() else {
        return;
    };
    let b = &e.obligations;
    let (billed0, collected0, off0, by0) = at_year.unwrap_or_default();
    println!("\n  obligations over the final year:\n");
    println!(
        "  left owing on bills {:>10.2e}   collected {:>10.2e}   charged off {:>10.2e}",
        b.billed - billed0,
        b.collected - collected0,
        b.charged_off - off0
    );
    for (o, v) in b.charged_off_by.iter() {
        let was = by0.get(o).copied().unwrap_or(0.0);
        if v - was > 0.0 {
            println!("    charged off, {:<16} {:>10.2e}", o.name(), v - was);
        }
    }
    println!(
        "  held back from discretionary shopping for known bills {:>10.2e}",
        held_back
    );
    let day = e.ledger.day;
    let mut standing: std::collections::BTreeMap<(&'static str, &'static str), (f64, f64)> =
        Default::default();
    for (_, i) in b.iter() {
        let who = match i.debtor {
            Account::Firm(_) => "firms",
            Account::Households(_) => "households",
            Account::ServiceSector(_) => "insurers",
            Account::State(_) => "the state",
            _ => "other",
        };
        let row = standing.entry((who, i.origin.name())).or_default();
        row.0 += i.outstanding();
        row.1 += i.overdue_on(day);
    }
    println!(
        "\n  owed at the end, by who owes it and for what ({} live):\n",
        b.len()
    );
    println!("  {:<12} {:<16} {:>10} {:>10}", "", "", "owed", "overdue");
    for ((who, what), (owed, overdue)) in standing.iter() {
        println!(
            "  {:<12} {:<16} {:>10.2e} {:>10.2e}",
            who, what, owed, overdue
        );
    }
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
    // **The insurance book**: what the towns paid in, what came back as
    // claims, and how many people it takes to settle them. Real: 1.12
    // million Americans work in the four insurance occupations, 0.72% of
    // everybody in work (OEWS May 2025).
    if let Some(e) = g.economy.as_ref() {
        let premiums: f64 = (0..e.markets.len()).map(|m| e.premiums_a_day(m)).sum();
        let posts: f64 = e
            .services
            .as_ref()
            .map(|s| {
                (0..e.markets.len())
                    .map(|m| s.posts_in(m, scale_sim::services::Sector::Insurance))
                    .sum()
            })
            .unwrap_or(0.0);
        let people: f64 = e.markets.iter().map(|m| m.population).sum();
        let claims = premiums * scale_sim::person::CLAIMS_SHARE_OF_PREMIUM;
        // How much of the industry is settling claims rather than selling
        // cover: adjusters and their clerks against agents and
        // underwriters, from the same chain the posts come from.
        let adjusters = scale_sim::services::VEHICLES_A_HEAD
            * scale_sim::services::CLAIMS_A_VEHICLE_A_YEAR
            / scale_sim::services::CLAIMS_AN_ADJUSTER_A_YEAR;
        let settling = adjusters * (1.0 + scale_sim::services::CLERKS_TO_AN_ADJUSTER);
        let selling = (scale_sim::services::VEHICLES_A_HEAD + 1.0 / 2.53)
            / scale_sim::services::POLICIES_TO_A_SELLER;
        let share_settling = settling / (settling + selling);
        println!(
            "
  insurance: {:.2e} of premiums a day and {:.2e} back as claims; {:.0} posts,          {:.2}% of everybody in work (real 0.72%), {:.0}% of them settling {:.2e} claims a year",
            premiums,
            claims,
            posts,
            posts / (people * 0.5) * 100.0,
            share_settling * 100.0,
            people * scale_sim::services::VEHICLES_A_HEAD
                * scale_sim::services::CLAIMS_A_VEHICLE_A_YEAR
        );
    }

    // **Who pays for medicine**, nation by nation. Real, as a share of
    // current health expenditure (WHO GHED, 2022): the United Kingdom is
    // 82 / 4 / 14 government, private and out of pocket; the United
    // States 55 / 34 / 11; China 58 / 11 / 32.
    if let Some(e) = g.economy.as_ref() {
        // **What the policy says against what was actually settled.** A
        // shortfall in delivery has a payer behind it, and which one is
        // the whole point of splitting the bill — a state that cannot
        // collect closes a tax-funded ward, and households that cannot
        // pay close one where the money is found at the door.
        let hospitals: std::collections::BTreeSet<usize> = e
            .ledger
            .sites
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind == scale_sim::econ::SiteKind::Hospital)
            .map(|(i, _)| i)
            .collect();
        let mut paid: std::collections::BTreeMap<u16, (f64, f64, f64)> = Default::default();
        for t in e.treasury.today.iter() {
            let scale_sim::money::Account::Firm(site) = t.to else {
                continue;
            };
            if !hospitals.contains(&site) {
                continue;
            }
            let n = e.markets[e.ledger.sites[site].market].nation;
            let row = paid.entry(n).or_default();
            match t.from {
                scale_sim::money::Account::State(_) => row.0 += t.amount,
                scale_sim::money::Account::ServiceSector(_) => row.1 += t.amount,
                scale_sim::money::Account::Households(_) => row.2 += t.amount,
                _ => {}
            }
        }
        println!();
        for (n, gov) in e.governments.iter() {
            let (public, insured, pocket) = gov.health.shares();
            let bill: f64 = (0..e.markets.len())
                .filter(|&m| e.markets[m].nation == *n)
                .map(|m| e.health_premiums_a_day(m))
                .sum();
            let (ps, is, hs) = paid.get(n).copied().unwrap_or((0.0, 0.0, 0.0));
            let took = ps + is + hs;
            let of = |x: f64| if took > 0.0 { x / took * 100.0 } else { 0.0 };
            println!(
                "  medicine, nation {n}: {:<17} policy {:.0}/{:.0}/{:.0} the state, insurers, \
the door; settled {:.0}/{:.0}/{:.0}; delivered {:.0}% (state affords {:.0}%); cover {:.2e} a day",
                gov.health.name(),
                public * 100.0,
                insured * 100.0,
                pocket * 100.0,
                of(ps),
                of(is),
                of(hs),
                gov.health_delivered() * 100.0,
                e.state_affords(*n) * 100.0,
                bill,
            );
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
        // **The figure taken apart, town by town**, because a
        // population-weighted mean of a ratio can be one town. What a house
        // is valued at (`Economy::house_price`: the dwelling's bill of
        // materials at local prices, over the 45% materials are of a build,
        // worn by condition, plus land) against a year of the production
        // worker's day rate at 260 days — the same arithmetic as the line
        // above, with every term printed.
        use scale_sim::econ::Commodity;
        let bill = scale_sim::building::Use::Dwelling.materials(0.0);
        println!(
            "  a dwelling takes {:.1} t of cement, {:.2} t of steel and {:.2} t of timber\n",
            bill.cement, bill.steel, bill.timber
        );
        println!(
            "  {:<14} {:>10} {:>9} {:>9} {:>9} {:>6} {:>6} {:>11} {:>9} {:>7}",
            "town",
            "people",
            "cement",
            "steel",
            "timber",
            "cond",
            "land",
            "house",
            "pay/yr",
            "years"
        );
        let mut by_town: Vec<f64> = Vec::new();
        for m in 0..e.markets.len() {
            let pay = scale_sim::person::day_rate(e, m, Trade::ProductionWorker) * 260.0;
            by_town.push(e.house_price(m) / pay.max(1e-9));
            println!(
                "  {:<14} {:>10.3e} {:>9.1} {:>9.1} {:>9.1} {:>6.2} {:>6.2} {:>11.3e} {:>9.0} {:>7.1}",
                e.markets[m].name,
                e.markets[m].population,
                e.price(m, Commodity::Cement),
                e.price(m, Commodity::Steel),
                e.price(m, Commodity::Timber),
                e.fabric_condition(m),
                e.land_value_ratio(m),
                e.house_price(m),
                pay,
                e.house_price(m) / pay.max(1e-9)
            );
        }
        // **The median town**, beside the mean above. An affordability
        // figure is conventionally a median, because one town can carry a
        // population-weighted mean of a ratio on its own — world 23's 527
        // years is one town at 9,826 over fifteen between 3.8 and 34.9.
        by_town.sort_by(|a, b| a.total_cmp(b));
        let n_towns = by_town.len();
        if n_towns > 0 {
            let median = if n_towns % 2 == 1 {
                by_town[n_towns / 2]
            } else {
                0.5 * (by_town[n_towns / 2 - 1] + by_town[n_towns / 2])
            };
            println!(
                "\n  the median town's house costs {median:.1} years of a production worker's pay"
            );
        }
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
