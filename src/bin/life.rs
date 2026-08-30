//! One person trying to make a living in a generated nation.
//!
//!   cargo run --release --bin life
//!   cargo run --release --bin life -- --seed 20260828 --rank 3 --days 400
//!   cargo run --release --bin life -- --trade labourer --money 200
//!
//! Nothing is arranged for them. Work exists when the economy wants
//! something done and not otherwise; prices are the market's; and if there
//! is no work and no money, they go hungry and eventually die of it.

use std::time::{SystemTime, UNIX_EPOCH};

use scale_sim::econ::{Commodity, Doctrine, DAYS_PER_YEAR};
use scale_sim::network::Network;
use scale_sim::person::{self, Person, State, Trade};
use scale_sim::polity::Polities;
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

const FOOD: Commodity = Commodity::ProcessedFood;

struct Args {
    seed: u64,
    rank: usize,
    days: u64,
    trade: Trade,
    money: f64,
    doctrine: Doctrine,
}

fn random_seed() -> u64 {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mut z = n ^ 0x9E3779B97F4A7C15;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z ^ (z >> 31)
}

fn parse_args() -> Args {
    let mut a = Args {
        seed: random_seed(),
        rank: 1,
        days: 365,
        trade: Trade::Haulier,
        money: 60.0,
        doctrine: Doctrine::Prudent,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--seed" => a.seed = it.next().and_then(|v| v.parse().ok()).unwrap_or(a.seed),
            "--rank" => a.rank = it.next().and_then(|v| v.parse().ok()).unwrap_or(1).max(1),
            "--days" => a.days = it.next().and_then(|v| v.parse().ok()).unwrap_or(365),
            "--money" => a.money = it.next().and_then(|v| v.parse().ok()).unwrap_or(60.0),
            "--trade" => {
                a.trade = match it.next().as_deref() {
                    Some("labourer") => Trade::Labourer,
                    _ => Trade::Haulier,
                }
            }
            "--doctrine" => {
                a.doctrine = match it.next().as_deref() {
                    Some("negligent") => Doctrine::Negligent,
                    _ => Doctrine::Prudent,
                }
            }
            "--help" | "-h" => {
                println!(
                    "usage: life [--seed N] [--rank K] [--days N] [--money N]\n\
                     \x20           [--trade haulier|labourer] [--doctrine prudent|negligent]"
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument: {other:?}");
                std::process::exit(2);
            }
        }
    }
    a
}

fn main() {
    let args = parse_args();

    println!("generating world (seed {})...", args.seed);
    let world = World::generate(768, 432, args.seed);
    let polities = Polities::partition(&world, 30);
    let settlements = Settlements::place(&world, &polities, 9000);
    let network = Network::build(&world, &settlements, 1400);

    let ranked = polities.ranked();
    let Some(&(id, _)) = ranked.get(args.rank - 1) else {
        eprintln!("no nation of rank {}", args.rank);
        std::process::exit(1);
    };
    let Some(mut region) =
        Region::extract(&world, &polities, &settlements, &network, id, 5, args.doctrine)
    else {
        eprintln!("that nation has no settlements to model");
        std::process::exit(1);
    };

    // Start them in the smallest market, which is where somebody with
    // nothing usually is.
    let start = region
        .economy
        .markets
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.population.total_cmp(&b.1.population))
        .map(|(m, _)| m)
        .unwrap_or(0);

    let mut hal = Person::new("Hal", args.trade, start, args.money);

    println!();
    println!(
        "{} the {}, of {} ({} people), with {:.0} in hand and three days' food.",
        hal.name,
        hal.trade.name(),
        region.economy.markets[start].name,
        fmt_pop(region.economy.markets[start].population),
        hal.money,
    );
    let wage = person::day_rate(&region.economy, start, hal.trade);
    let food_day = region.economy.price(start, FOOD) * person::FOOD_PER_DAY;
    println!(
        "A day's work pays about {wage:.1}; a day's food costs {food_day:.1}.",
    );
    println!();

    println!("yr:day | state    | money | food | cond | happening");
    println!("-------+----------+-------+------+------+--------------------------------");

    let mut last_log = 0;
    for _ in 0..args.days {
        region.economy.step();
        let day = region.economy.ledger.day;
        person::live_a_day(&mut hal, &mut region.economy, day);

        // Print only when something happened to them.
        let happened: Vec<String> = hal.log[last_log..].to_vec();
        last_log = hal.log.len();
        for line in &happened {
            let what = line.splitn(2, ": ").nth(1).unwrap_or(line);
            println!(
                "{:>6} | {:<8} | {:>5.0} | {:>4.1} | {:>4.2} | {}",
                format!("{}:{:03}", day / DAYS_PER_YEAR, day % DAYS_PER_YEAR),
                match hal.state {
                    State::Idle => "idle",
                    State::Working { .. } => "working",
                    State::Dead => "dead",
                },
                hal.money,
                hal.larder,
                hal.condition,
                what,
            );
        }
        if !hal.alive() {
            break;
        }
    }

    println!();
    let day = region.economy.ledger.day;
    if hal.alive() {
        println!(
            "After {} days {} is alive with {:.0} in hand, having worked {} days and \
             earned {:.0}.",
            args.days, hal.name, hal.money, hal.days_worked, hal.earned,
        );
        if hal.money < 20.0 {
            println!("Not much of a living.");
        }
    } else {
        println!(
            "{} is dead on day {day}, having worked {} days and earned {:.0}.",
            hal.name, hal.days_worked, hal.earned,
        );
    }
    println!("  {} entries in his log", hal.log.len());
}

fn fmt_pop(p: f64) -> String {
    if p >= 1.0e6 {
        format!("{:.1}M", p / 1.0e6)
    } else if p >= 1.0e3 {
        format!("{:.0}k", p / 1.0e3)
    } else {
        format!("{p:.0}")
    }
}
