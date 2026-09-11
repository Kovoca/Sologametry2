//! **The national accounts**, and where the money goes when it does not
//! come back.
//!
//!   cargo run --release --bin accounts
//!   cargo run --release --bin accounts -- --seed 7 --nations 4 --days 300
//!
//! Total income is total value added is total spending — which says the
//! wage *level* cannot by itself leave households unable to pay for what
//! the country makes: whatever labour does not get, profit does. So when
//! they cannot pay, the money has leaked somewhere, and this says where:
//! into firms' reserves, into the state, abroad, or from one town into
//! another.

use scale_sim::econ::{Commodity, Doctrine};
use scale_sim::money::Account;
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Nations;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;
use std::collections::BTreeMap;

fn class(a: Account) -> &'static str {
    match a {
        Account::Firm(_) => "firms",
        Account::Households(_) => "households",
        Account::ServiceSector(_) => "services",
        Account::State => "state",
        Account::Abroad => "abroad",
        _ => "other",
    }
}

fn main() {
    let mut seed = 7u64;
    let mut nations = 4usize;
    let mut days = 300u64;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let v = it.next().and_then(|v| v.parse::<u64>().ok());
        match (a.as_str(), v) {
            ("--seed", Some(v)) => seed = v,
            ("--nations", Some(v)) => nations = v as usize,
            ("--days", Some(v)) => days = v,
            _ => {
                eprintln!("usage: accounts [--seed N] [--nations N] [--days N]");
                return;
            }
        }
    }
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

    let holdings = |e: &scale_sim::econ::Economy| {
        let mut h: BTreeMap<&'static str, f64> = BTreeMap::new();
        for (a, v) in e.treasury.accounts() {
            *h.entry(class(*a)).or_default() += *v;
        }
        h
    };
    let start = holdings(&n.economy);

    // Each town's households: what came in and what went out, by why.
    let towns = n.economy.markets.len();
    let mut town_in: Vec<BTreeMap<String, f64>> = vec![BTreeMap::new(); towns];
    let mut town_out: Vec<BTreeMap<String, f64>> = vec![BTreeMap::new(); towns];
    // flows[from class -> to class, why]
    let mut flows: BTreeMap<(String, String, String), f64> = BTreeMap::new();
    let mut unpaid_why: BTreeMap<&'static str, f64> = BTreeMap::new();
    let mut snapshots = Vec::new();
    for d in 0..days {
        n.economy.step();
        for t in n.economy.treasury.today.iter() {
            *flows
                .entry((
                    class(t.from).to_string(),
                    class(t.to).to_string(),
                    format!("{:?}", t.why),
                ))
                .or_default() += t.amount;
            if let Account::Households(m) = t.to {
                *town_in[m]
                    .entry(format!("{:?} from {}", t.why, class(t.from)))
                    .or_default() += t.amount;
            }
            if let Account::Households(m) = t.from {
                *town_out[m]
                    .entry(format!("{:?} to {}", t.why, class(t.to)))
                    .or_default() += t.amount;
            }
        }
        for (k, v) in n.economy.treasury.unpaid_why.iter() {
            *unpaid_why.entry(k).or_default() += v;
        }
        if d % 50 == 49 {
            snapshots.push((d + 1, holdings(&n.economy)));
        }
    }
    let e = &n.economy;

    println!(
        "seed {seed}, {nations} nations, {} towns, {days} days\n",
        e.markets.len()
    );
    println!("who holds the money");
    print!("  {:<10}", "day 0");
    for (k, v) in start.iter() {
        print!("  {k} {v:.3e}");
    }
    println!();
    for (d, h) in snapshots.iter() {
        print!("  day {d:<6}");
        for (k, v) in h.iter() {
            print!("  {k} {v:.3e}");
        }
        println!();
    }

    println!("\nflows over the run, by who paid whom and why");
    for ((f, t, w), v) in flows.iter() {
        if f == t && f != "firms" {
            continue;
        }
        println!("  {f:<10} -> {t:<10} {w:<16} {v:.3e}");
    }
    println!("\nowed and not paid");
    for (k, v) in unpaid_why.iter() {
        println!("  {k:<16} {v:.3e}");
    }

    // Household income against what the basket costs, per head a year.
    let pop: f64 = e.markets.iter().map(|m| m.population).sum();
    let get = |f: &str, t: &str, w: &str| {
        flows
            .get(&(f.to_string(), t.to_string(), w.to_string()))
            .copied()
            .unwrap_or(0.0)
    };
    let year = 365.0 / days as f64;
    let wages = (get("firms", "households", "Payroll")
        + get("services", "households", "Payroll")
        + get("state", "households", "Payroll")
        + get("state", "households", "PublicSpending"))
        * year
        / pop;
    let profit = (get("firms", "households", "Profit") + get("services", "households", "Profit"))
        * year
        / pop;
    let spend =
        (get("households", "firms", "Purchase") + get("households", "services", "Purchase")) * year
            / pop;
    println!("\nper head a year: wages {wages:.0}, profit {profit:.0}, spending {spend:.0}");
    let basket: f64 = Commodity::ALL
        .iter()
        .map(|&c| c.per_capita_annual() * c.base_cost())
        .sum();
    println!("the basket at reference prices: {basket:.0} a head a year");
    let labour_share = wages / (wages + profit).max(1e-9);
    println!(
        "wages as a share of household income: {:.0}%",
        labour_share * 100.0
    );

    // Each town's households, a head a year: in and out.
    println!("\na head a year, by town: what households took in and paid out");
    for m in 0..e.markets.len() {
        let pop = e.markets[m].population.max(1.0);
        let fmt = |b: &BTreeMap<String, f64>| {
            b.iter()
                .map(|(k, v)| format!("{k} {:.0}", v * year / pop))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let tin: f64 = town_in[m].values().sum::<f64>() * year / pop;
        let tout: f64 = town_out[m].values().sum::<f64>() * year / pop;
        println!(
            "  {:<12} in {tin:>6.0} [{}]",
            e.markets[m].name,
            fmt(&town_in[m])
        );
        println!("  {:<12} out {tout:>5.0} [{}]", "", fmt(&town_out[m]));
    }

    // Towns whose households have run dry.
    println!("\nhouseholds by town at the end");
    for m in 0..e.markets.len() {
        let bal = e.treasury.balance(Account::Households(m));
        let per_head = bal / e.markets[m].population.max(1.0);
        let firms: f64 = (0..e.ledger.sites.len())
            .filter(|&s| e.ledger.sites[s].market == m)
            .map(|s| e.treasury.balance(Account::Firm(s)))
            .sum();
        println!(
            "  {:<16} pop {:>10.0}  households {:>10.3e} ({:>7.1} a head)  firms {:>10.3e}  services {:>10.3e}  nation {}",
            e.markets[m].name,
            e.markets[m].population,
            bal,
            per_head,
            firms,
            e.treasury.balance(Account::ServiceSector(m)),
            e.markets[m].nation
        );
    }
}
