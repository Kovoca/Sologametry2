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
    // Every nation's public posts, because there is a state per country.
    let state: f64 = e
        .governments
        .values()
        .map(|g| g.posts.iter().sum::<f64>())
        .sum();
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

    // ---- who does what, against the whole United States ------------------
    //
    // Every industry's jobs spread over the occupations it employs, set
    // beside the national mix from the same survey. The national figures
    // leave out farms and the armed forces, so both are shown and neither
    // is expected to match.
    use scale_sim::occupation::{jobs_by_occupation, published, Occupation, N_OCCUPATIONS};
    let mut mine = [0.0f64; N_OCCUPATIONS];
    for town in jobs_by_occupation(&e) {
        for (o, v) in town.iter().enumerate() {
            mine[o] += v;
        }
    }
    let modelled: f64 = mine.iter().sum();
    let national = published().get("national").expect("the national table");
    let us_total = national.get("00-0000").copied().unwrap_or(1.0);
    let us = |o: Occupation| -> f64 {
        let g = |c: &str| national.get(c).copied().unwrap_or(0.0);
        let less = |grp: &str, parts: &[&str]| g(grp) - parts.iter().map(|p| g(p)).sum::<f64>();
        use Occupation::*;
        let n = match o {
            Manager => g("11-0000"),
            Farmer | Fisher | Soldier => 0.0,
            Accountant => g("13-2011"),
            BusinessSpecialist => less("13-0000", &["13-2011"]),
            ComputingSpecialist => g("15-0000"),
            Engineer => g("17-0000"),
            Scientist => g("19-0000"),
            SocialWorker => g("21-0000"),
            Legal => g("23-0000"),
            Teacher => g("25-0000"),
            ArtsAndMedia => g("27-0000"),
            Doctor => g("29-1210") + g("29-1240"),
            Nurse => g("29-1141"),
            HealthTechnician => less("29-0000", &["29-1210", "29-1240", "29-1141"]),
            CareAssistant => g("31-0000"),
            ProtectiveService => g("33-0000"),
            FoodService => g("35-0000"),
            Cleaner => g("37-0000"),
            PersonalCare => g("39-0000"),
            Sales => g("41-0000"),
            OfficeClerk => g("43-0000"),
            FarmWorker => g("45-0000"),
            Builder => less("47-0000", &["47-2111", "47-2152", "47-5000"]),
            Electrician => g("47-2111"),
            Pipefitter => g("47-2152"),
            Miner => g("47-5000"),
            Mechanic => g("49-0000"),
            ProductionWorker => g("51-0000"),
            Driver => g("53-3032") + g("53-3033"),
            MaterialMover => less("53-0000", &["53-3032", "53-3033"]),
        };
        n / us_total
    };
    println!(
        "
who does what: {modelled:.0} jobs in this nation, against the United States          (OEWS May 2023, which leaves out farms and the armed forces)
"
    );
    println!("  {:<22} {:>8} {:>8}", "", "here", "US");
    for o in Occupation::ALL {
        let here = mine[o.index()] / modelled.max(1.0) * 100.0;
        println!("  {:<22} {:>7.1}% {:>7.1}%", o.name(), here, us(o) * 100.0);
    }
}
