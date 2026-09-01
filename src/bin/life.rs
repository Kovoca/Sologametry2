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
use scale_sim::person::{self, Housing, Person, State, Trade};
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
    fault: Option<&'static str>,
    fault_on: u64,
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
        fault: None,
        fault_on: 40,
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
                    Some("shop") => Trade::Shopworker,
                    _ => Trade::Haulier,
                }
            }
            "--doctrine" => {
                a.doctrine = match it.next().as_deref() {
                    Some("negligent") => Doctrine::Negligent,
                    _ => Doctrine::Prudent,
                }
            }
            "--fault" => {
                a.fault = match it.next().as_deref() {
                    Some("line") => Some("line"),
                    Some("transformer") => Some("transformer"),
                    _ => None,
                }
            }
            "--fault-on" => {
                a.fault_on = it.next().and_then(|v| v.parse().ok()).unwrap_or(40)
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
    println!(
        "He sleeps in {}; the rent is {:.1} a day against {:.1} for food.",
        hal.housing.name(),
        person::rent_per_day(&region.economy, start),
        food_day,
    );
    println!(
        "He travels {} and can carry {:.0} kg of his own.",
        hal.conveyance.name(),
        hal.conveyance.payload() * 1000.0,
    );
    println!();
    println!("the roads out of here");
    for r in &region.economy.routes {
        if r.a != start && r.b != start {
            continue;
        }
        let s = r.surface;
        let on_foot = scale_sim::travel::Conveyance::OnFoot
            .km_per_day(s)
            .map(|v| format!("{:.0} days on foot", (r.km / v).max(1.0)))
            .unwrap_or_else(|| "impassable on foot".into());
        let lorry = scale_sim::travel::Conveyance::Artic
            .km_per_day(s)
            .map(|v| format!("{:.0} by lorry", (r.km / v).max(1.0)))
            .unwrap_or_else(|| "no lorry gets through".into());
        println!(
            "  {} to {}: {:.0} km of {} ({:.0}/t, {:.3}/t-km) — {on_foot}, {lorry}",
            region.economy.markets[r.a].name,
            region.economy.markets[r.b].name,
            r.km,
            s.name(),
            r.sound_cost,
            r.sound_cost / r.km.max(1.0),
        );
    }
    println!();

    // Prime the labour market so the opening picture is not day zero.
    region.economy.step();
    println!("who works here");
    for m in 0..region.economy.markets.len() {
        let w = &region.economy.workforce[m];
        if w.posts <= 0.0 {
            continue;
        }
        println!(
            "  {:<10} {:>8} posts in these trades, {:>5.1}% of them idle",
            region.economy.markets[m].name,
            fmt_pop(w.posts),
            w.unemployment * 100.0,
        );
    }
    println!();

    println!("yr:day | state    | money | food | cond | bread | happening");
    println!("-------+----------+-------+------+------+-------+-------------------------");

    let mut last_log = 0;
    let mut last_shape = String::new();
    let mut prev_shape = String::new();
    let mut repeats = 0usize;
    let mut last_unemployment = region.economy.workforce[hal.market].unemployment;
    for elapsed in 0..args.days {
        if elapsed == args.fault_on {
            match args.fault {
                Some("line") => {
                    region.economy.grid.fail_line("main line");
                }
                Some("transformer") => {
                    region.economy.grid.fail_transformer("main line");
                }
                _ => {}
            }
            if args.fault.is_some() {
                println!(
                    "{:>6} |          |       |      |      |       | *** {} fails ***",
                    "",
                    args.fault.unwrap()
                );
            }
        }
        region.economy.step();
        let day = region.economy.ledger.day;
        person::live_a_day(&mut hal, &mut region.economy, day);

        // Say so when the ground shifts under him, because that is the
        // thing the workforce was built to make visible.
        let u = region.economy.workforce[hal.market].unemployment;
        if (u - last_unemployment).abs() > 0.05 {
            println!(
                "{:>6} |          |       |      |      |       | {} is {:.0}% out of work",
                format!("{}:{:03}", day / DAYS_PER_YEAR, day % DAYS_PER_YEAR),
                region.economy.markets[hal.market].name,
                u * 100.0,
            );
            last_unemployment = u;
        }

        // Print only when something happened to them — and **not the same
        // thing over and over**. A decade of an honest working life is
        // four thousand identical lines of "hauled grain, paid 5", which
        // buries every event that matters in it.
        let happened: Vec<String> = hal.log[last_log..].to_vec();
        last_log = hal.log.len();
        for line in &happened {
            let what = line.splitn(2, ": ").nth(1).unwrap_or(line);
            // Strip the running total so repeats of the same job match.
            let shape: String = what
                .split(" — ")
                .next()
                .unwrap_or(what)
                .chars()
                .filter(|c| !c.is_ascii_digit())
                .collect();
            // Work and rest alternate, so a repeating life is a repeating
            // *pair* of lines, not a repeating one.
            if shape == last_shape || shape == prev_shape {
                repeats += 1;
                prev_shape = std::mem::replace(&mut last_shape, shape);
                continue;
            }
            if repeats > 0 {
                println!(
                    "       |          |       |      |      |       |                      ... and {repeats} more like that"
                );
                repeats = 0;
            }
            prev_shape = std::mem::replace(&mut last_shape, shape);
            println!(
                "{:>6} | {:<8} | {:>5.0} | {:>4.1} | {:>4.2} | {:>5.2} | {}",
                format!("{}:{:03}", day / DAYS_PER_YEAR, day % DAYS_PER_YEAR),
                match hal.state {
                    State::Idle => "idle",
                    State::Working { .. } => "working",
                    State::Dead => "dead",
                },
                hal.money,
                hal.larder,
                hal.condition,
                region.economy.price(hal.market, FOOD) * person::FOOD_PER_DAY,
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
            "After {} days {} is alive with {:.0} in hand, having worked {} days, \
             earned {:.0} and spent {:.0} on food and vehicles.",
            args.days, hal.name, hal.money, hal.days_worked, hal.earned, hal.spent,
        );
        // **In days of food, because money moves.**
        //
        // A blackout year ends with more cash in his pocket and him very
        // much worse off, since bread quintupled underneath it. Nominal
        // balances are not comparable across a price shock; what a purse
        // is worth is what it buys.
        let bread = region.economy.price(hal.market, FOOD) * person::FOOD_PER_DAY;
        println!(
            "  which is {:.0} days of food at today's price of {bread:.2}.",
            hal.money / bread.max(1e-9),
        );
        println!(
            "He sleeps in {}{}.",
            hal.housing.name(),
            if hal.housing == Housing::Homeless {
                format!(", and has for {} days", hal.days_homeless)
            } else {
                String::new()
            },
        );
        println!(
            "He ends up with {} — {:.0} kg at a time.",
            hal.conveyance.name(),
            hal.conveyance.payload() * 1000.0,
        );
        // What he is holding is not all he is worth: a trader caught
        // mid-journey has his money in the cargo, and the books only
        // balance once that is counted back in.
        let staked = hal.job.as_ref().map(|c| c.stake()).unwrap_or(0.0);
        if staked > 0.0 {
            println!("  and {staked:.0} more tied up in a load still on the road.");
        }
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
