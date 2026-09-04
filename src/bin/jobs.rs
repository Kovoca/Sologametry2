//! **Who actually works at what**, measured against the real shares.
//!
//!   cargo run --release --bin jobs -- --seed N --rank K

use scale_sim::econ::Doctrine;
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

fn main() {
    let mut seed = 20260828u64;
    let mut rank = 2usize;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => seed = it.next().and_then(|v| v.parse().ok()).unwrap_or(seed),
            "--rank" => rank = it.next().and_then(|v| v.parse().ok()).unwrap_or(rank),
            _ => {}
        }
    }

    let world = World::generate(384, 216, seed);
    let pol = Polities::partition(&world, 30);
    let set = Settlements::place(&world, &pol, 5000);
    let net = Network::build(&world, &set, 1000);
    let Some(id) = pol.ranked().get(rank).map(|r| r.0) else {
        eprintln!("no nation of rank {rank}");
        return;
    };
    let Some(r) = Region::extract(&world, &pol, &set, &net, id, 5, Doctrine::Prudent) else {
        eprintln!("that nation has no settlements to model");
        return;
    };
    let mut e = r.economy;
    for _ in 0..200 {
        e.step();
    }

    let people: f64 = e.markets.iter().map(|m| m.population).sum();
    // **Not Workforce::hands.** That counts the trades the works
    // employ and nothing else, which this file already records — so
    // dividing the state and the services by it gives shares over 100%.
    // The labour force is the population in work: real participation
    // runs 45-55%.
    let hands = people * 0.5;
    println!("nation of {people:.0}, labour force {hands:.0}\n");

    // Everybody the model actually employs, by where they work.
    let mut shop = 0.0;
    let mut works = 0.0;
    for (i, s) in e.ledger.sites.iter().enumerate() {
        let staff = e.staff_today.get(i).copied().unwrap_or(0.0);
        if matches!(s.kind, scale_sim::econ::SiteKind::Shop) {
            shop += staff;
        } else {
            works += staff;
        }
    }
    let state = e.government.as_ref().map(|g| g.posts.iter().sum::<f64>()).unwrap_or(0.0);
    let services = e.services.as_ref().map(|s| s.posts.iter().flatten().sum::<f64>()).unwrap_or(0.0);

    let row = |name: &str, n: f64, real: f64| {
        let share = if hands > 0.0 { 100.0 * n / hands } else { 0.0 };
        println!("  {name:<28} {n:>12.0}  {share:>5.1}%   real {real:>4.1}%");
    };
    row("retail (shops)", shop, 14.1);
    row("works, mines, farms", works, 10.0);
    row("the state", state, 17.0);
    row("private services", services, 36.8);

    let total = shop + works + state + services;
    println!(
        "\n  {:<28} {total:>12.0}  {:>5.1}%",
        "accounted for",
        if hands > 0.0 { 100.0 * total / hands } else { 0.0 }
    );
}
