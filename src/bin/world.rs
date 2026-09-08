//! Generate a planet and run several of its nations as one trading world.
//!
//!   cargo run --release --bin world
//!   cargo run --release --bin world -- --seed 20260828 --nations 6 --days 1100
//!   cargo run --release --bin world -- --blockade 1
//!
//! Unlike `--bin region`, the nations here can buy from each other. That
//! is what turns the hemisphere offset, the coal-rich/coal-poor split and
//! the ore/fuel complementarity from facts about the map into commerce —
//! and into something that can be cut.

use std::time::{SystemTime, UNIX_EPOCH};

use scale_sim::econ::{Commodity, Doctrine, DAYS_PER_YEAR};
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Nations;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

const FOOD: Commodity = Commodity::ProcessedFood;
const GRAIN: Commodity = Commodity::Grain;

struct Args {
    seed: u64,
    nations: usize,
    markets: usize,
    days: u64,
    doctrine: Doctrine,
    /// Close every lane touching this nation, on `blockade_on`.
    blockade: Option<usize>,
    blockade_on: u64,
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
        nations: 6,
        markets: 4,
        days: 800,
        doctrine: Doctrine::Prudent,
        blockade: None,
        blockade_on: 400,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--seed" => a.seed = it.next().and_then(|v| v.parse().ok()).unwrap_or(a.seed),
            "--nations" => {
                a.nations = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(6)
                    .clamp(2, 20)
            }
            "--markets" => a.markets = it.next().and_then(|v| v.parse().ok()).unwrap_or(4).max(1),
            "--days" => a.days = it.next().and_then(|v| v.parse().ok()).unwrap_or(800),
            "--blockade" => a.blockade = it.next().and_then(|v| v.parse().ok()),
            "--blockade-on" => {
                a.blockade_on = it.next().and_then(|v| v.parse().ok()).unwrap_or(400)
            }
            "--doctrine" => {
                a.doctrine = match it.next().as_deref() {
                    Some("negligent") => Doctrine::Negligent,
                    _ => Doctrine::Prudent,
                }
            }
            "--help" | "-h" => {
                println!(
                    "usage: world [--seed N] [--nations K] [--markets N] [--days N]\n\
                     \x20            [--doctrine prudent|negligent]\n\
                     \x20            [--blockade NATION] [--blockade-on DAY]\n\
                     \n\
                     --blockade K   close every trade lane touching nation K"
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

    let mut nations = Nations::build(
        &world,
        &polities,
        &settlements,
        &network,
        args.nations,
        args.markets,
        args.doctrine,
    );

    println!();
    println!("nations");
    for (n, name) in nations.names.iter().enumerate() {
        let ms = &nations.markets_of[n];
        let pop: f64 = ms
            .iter()
            .map(|&m| nations.economy.markets[m].population)
            .sum();
        let southern = nations.economy.markets[ms[0]].southern;
        println!(
            "  {n}. {:<12} {:>8} people   {:<8}   {} markets",
            name,
            fmt_pop(pop),
            if southern { "southern" } else { "northern" },
            ms.len(),
        );
    }
    println!("  {} in all", fmt_pop(nations.population()));
    println!();

    // Lanes between nations are the ones whose ends are in different
    // countries.
    let e = &nations.economy;
    let international: Vec<usize> = (0..e.routes.len())
        .filter(|&r| e.markets[e.routes[r].a].nation != e.markets[e.routes[r].b].nation)
        .collect();
    println!(
        "trade lanes ({} of {} routes cross a border)",
        international.len(),
        e.routes.len()
    );
    for &r in international.iter().take(14) {
        println!(
            "  {:<46} {:>7.0} /t",
            e.routes[r].name, e.routes[r].freight_cost
        );
    }
    if international.len() > 14 {
        println!("  ... and {} more", international.len() - 14);
    }
    println!();

    // What each nation's roads cost to have and to keep. The trap this
    // exposes is the point: hard country costs more per kilometre AND
    // holds fewer people to pay for it.
    let accounts = scale_sim::infrastructure::all_accounts(&world, &network, &polities);
    println!("roads          km      hard      capital      upkeep/yr   per head/yr");
    for (n, name) in nations.names.iter().enumerate() {
        let a = &accounts[nations.polity[n] as usize];
        // Everyone the roads serve, not merely the handful of cities
        // modelled as markets. A nation's network is paid for by all of
        // it, and dividing by the modelled few would flatter the sparse
        // countries this figure exists to expose.
        let pid = nations.polity[n];
        let people: f64 = settlements
            .list
            .iter()
            .filter(|s| s.polity == pid)
            .map(|s| s.population as f64)
            .sum();
        println!(
            "  {:<11} {:>7.0} {:>7.0}%   {:>10.0}M   {:>10.0}M   {:>10.2}",
            name,
            a.total_km(),
            a.hard_going_km / a.total_km().max(1.0) * 100.0,
            a.capital,
            a.upkeep_per_year,
            a.upkeep_per_capita(people),
        );
    }
    println!();

    for n in &nations.notes {
        println!("  {n}");
    }
    println!();

    println!(
        "yr:day | seasons N/S    | grain: cheapest  dearest | hauls | roads | freight | event"
    );
    println!(
        "-------+----------------+--------------------------+-------+-------+---------+-------"
    );

    for day in 0..args.days {
        let mut note = String::new();
        if day == args.blockade_on {
            if let Some(target) = args.blockade {
                let e = &mut nations.economy;
                let mut closed = 0;
                for r in e.routes.iter_mut() {
                    let (na, nb) = (e.markets[r.a].nation, e.markets[r.b].nation);
                    if na != nb && (na as usize == target || nb as usize == target) {
                        r.open = false;
                        closed += 1;
                    }
                }
                note = format!("nation {target} blockaded, {closed} lanes closed");
            }
        }

        nations.economy.step();
        let e = &nations.economy;

        // Cheapest and dearest grain anywhere, and who is buying abroad.
        let mut lo = f64::INFINITY;
        let mut hi: f64 = 0.0;
        for m in 0..e.markets.len() {
            let p = e.price(m, GRAIN);
            lo = lo.min(p);
            hi = hi.max(p);
        }
        let hauls = international
            .iter()
            .filter(|&&r| e.arbitrage(r, GRAIN) > 0.0 || e.arbitrage(r, FOOD) > 0.0)
            .count();

        // Seasons in each hemisphere, to show the offset.
        let north = e
            .markets
            .iter()
            .position(|m| !m.southern)
            .map(|m| e.season_at(m).name())
            .unwrap_or("-");
        let south = e
            .markets
            .iter()
            .position(|m| m.southern)
            .map(|m| e.season_at(m).name())
            .unwrap_or("-");

        if !note.is_empty() || day % 365 == 0 || day + 1 == args.days {
            let yr = e.ledger.day / DAYS_PER_YEAR;
            let doy = e.ledger.day % DAYS_PER_YEAR;
            // Worst-kept network on the planet, and what the world's
            // freight bill has come to because of it.
            let worst = e.road_condition.iter().copied().fold(1.0f64, f64::min);
            let freight: f64 = e.routes.iter().map(|r| r.freight_cost).sum();
            println!(
                "{:>6} | {:<6} {:<7} | {:>15.0} {:>8.0} | {:>5} | {:>5.2} | {:>7.0} | {}",
                format!("{yr}:{doy:03}"),
                north,
                south,
                lo,
                hi,
                hauls,
                worst,
                freight,
                note,
            );
        }
    }

    println!();
    summary(&nations);
}

fn summary(nations: &Nations) {
    let e = &nations.economy;
    println!("after {} days:", e.ledger.day);
    for (n, name) in nations.names.iter().enumerate() {
        let ms = &nations.markets_of[n];
        let food = e.price(ms[0], FOOD);
        let grain = e.price(ms[0], GRAIN);
        let hungry: f64 = e.unmet_demand[FOOD as usize];
        println!(
            "  {:<12} grain {:>6.0}   food {:>6.0}   harvest {:>4.2}",
            name,
            grain,
            food,
            e.harvest_at(ms[0]),
        );
        let _ = hungry;
    }
    let short = e.unmet_demand[FOOD as usize];
    if short > 0.01 {
        println!("  {short:.0} t of food demand unmet today across the world");
    }
    println!("  {} journal entries", e.journal.len());
    e.ledger.assert_conserved();
    println!("  conservation: OK");
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
