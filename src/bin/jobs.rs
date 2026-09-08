//! **Who actually works at what**, measured against the real shares.
//!
//!   cargo run --release --bin jobs -- --seed N --rank K

use scale_sim::census::{
    retail_share_of_fte, whole_distribution_sector, Base, Census, DISTRIBUTION,
};
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
    // **Not `Workforce::hands`.** That counts the trades the works
    // employ and nothing else, which this project already records — so
    // dividing the state and the private services by it gives shares
    // over 100%. `Census` exists so the mistake is hard to make twice:
    // every share names what it is a share of.
    let c = Census::of_a_nation(people);
    println!(
        "nation of {people:.0}\n  labour force {:.0}, in work {:.0}, jobs {:.0}, FTE {:.0}\n",
        c.labour_force, c.employed_people, c.jobs, c.full_time_equivalents
    );

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
    let state = e
        .government
        .as_ref()
        .map(|g| g.posts.iter().sum::<f64>())
        .unwrap_or(0.0);
    let services = e
        .services
        .as_ref()
        .map(|s| s.posts.iter().flatten().sum::<f64>())
        .unwrap_or(0.0);

    let row = |name: &str, n: f64, real: f64| {
        let s = c.share(n, Base::EmployedPeople);
        println!(
            "  {name:<30} {n:>12.0}  {:>5.1}% of {:<14} real {real:>4.1}%",
            s.percent(),
            s.of.name()
        );
    };
    // **Shops, not the whole distribution sector.** 14.1% is wholesale
    // and retail and the motor trade together; shops are a little under
    // two thirds of it.
    row("retail (shops)", shop, DISTRIBUTION[0].of_employed * 100.0);
    row("works, mines, farms", works, 10.0);
    row("the state", state, 17.0);
    row("private services", services, 36.8);

    let total = shop + works + state + services;
    let all = c.share(total, Base::EmployedPeople);
    println!(
        "\n  {:<30} {total:>12.0}  {:>5.1}% of {}",
        "accounted for",
        all.percent(),
        all.of.name()
    );

    println!("\nthe distribution sector, at the boundaries the statistics use");
    for b in DISTRIBUTION {
        println!("  {:<62} {:>4.1}%", b.what, b.of_employed * 100.0);
    }
    println!(
        "  {:<62} {:>4.1}%",
        "all three together — the figure usually quoted",
        whole_distribution_sector() * 100.0
    );
    println!(
        "\n  and shops measured in hours rather than heads: {:.1}% of employment,\n  \
         because about 60% of retail work is part-time — which is what a model\n  \
         whose staffing comes out of labour-hours should be aiming at.",
        retail_share_of_fte() * 100.0
    );
}
