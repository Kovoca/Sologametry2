//! **What a town's households could actually pay for at the counter.**
//!
//!   cargo run --release --bin counter -- --seed 7 --days 400
//!
//! `consume_households` takes the goods off the shelf whatever the
//! balance and puts the shortfall on a counter, so a poor town eats like
//! a rich one on credit nobody extended (`docs/status.md` item 3). Before
//! changing that, this measures the thing that would change: what the
//! day's counter purchases come to, what the pool holds against them, and
//! how much of it the town could have funded.
//!
//! **Over a counter you pay; on a bill you can fall behind.** So this
//! counts only what is handed over a till — food, meat, remedies and
//! goods — and leaves out the power (`utility.rs` bills monthly in
//! arrears) and the hospital, which sends an invoice.

use scale_sim::econ::{Commodity, Doctrine, Economy};
use scale_sim::money::Account;
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Nations;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

/// What is bought at a till rather than billed.
const AT_THE_TILL: [Commodity; 4] = [
    Commodity::ProcessedFood,
    Commodity::Meat,
    Commodity::Remedies,
    Commodity::RetailGoods,
];

/// The day's counter basket for one town, at today's prices.
fn basket_cost(e: &Economy, m: usize) -> f64 {
    AT_THE_TILL
        .iter()
        .map(|&c| e.markets[m].daily_household_demand(c) * e.markets[m].price[c as usize])
        .sum()
}

fn main() {
    let mut seed = 7u64;
    let mut days = 400u64;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let v = it.next().and_then(|v| v.parse::<u64>().ok());
        match (a.as_str(), v) {
            ("--seed", Some(v)) => seed = v,
            ("--days", Some(v)) => days = v,
            _ => {
                eprintln!("usage: counter [--seed N] [--days N]");
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
        4,
        4,
        Doctrine::Prudent,
    );

    let towns = n.economy.markets.len();
    // Days on which the pool could not fund the day's counter basket, and
    // the worst shortfall seen.
    let mut short_days = vec![0u64; towns];
    let mut worst = vec![0.0f64; towns];
    // Averaged over the run, because one morning is not a country.
    let mut sum_cover = vec![0.0f64; towns];

    for _ in 0..days {
        n.economy.step();
        for m in 0..towns {
            let want = basket_cost(&n.economy, m);
            let pool = n.economy.treasury.balance(Account::Households(m)).max(0.0);
            if want <= 1e-9 {
                continue;
            }
            let cover = pool / want;
            sum_cover[m] += cover;
            if cover < 1.0 {
                short_days[m] += 1;
                worst[m] = worst[m].max(1.0 - cover);
            }
        }
    }

    // Outcomes that exist in both versions of the model, so the before
    // and after are the same measurement.
    let food = Commodity::ProcessedFood as usize;
    let o = n.economy.opening().cloned();
    let covers: Vec<f64> = match &o {
        Some(o) => (0..towns)
            .map(|m| o.cover(m, Commodity::ProcessedFood))
            .collect(),
        None => vec![0.0; towns],
    };
    let mut price = 0.0;
    let mut sold = 0.0;
    for m in 0..towns {
        price += n.economy.markets[m].price[food];
    }
    let cover: f64 = covers.iter().sum();
    for s in n.economy.ledger.sites.iter() {
        if s.kind == scale_sim::econ::SiteKind::Shop {
            sold += s.ran;
        }
    }
    let short_towns = (0..towns).filter(|&m| short_days[m] > 0).count();
    let spread = {
        let mut v = covers.clone();
        v.sort_by(f64::total_cmp);
        v[v.len() - 1] - v[0]
    };

    println!(
        "seed {seed}, {towns} towns, {days} days
"
    );
    println!("towns short of counter money   {short_towns} of {towns}");
    println!(
        "days a town was short, worst   {}",
        short_days.iter().max().copied().unwrap_or(0)
    );
    println!("food price, mean over towns    {:.1}", price / towns as f64);
    println!("food cover, mean over towns    {:.2}", cover / towns as f64);
    println!("food cover, spread             {spread:.2}");
    println!("shops sold today               {sold:.4e}");
    println!(
        "household purchases unpaid     {:.4e}",
        *n.economy
            .treasury
            .unpaid_why
            .get("purchases")
            .unwrap_or(&0.0)
    );
    println!(
        "counter basket, last day       {:.4e}",
        (0..towns).map(|m| basket_cost(&n.economy, m)).sum::<f64>()
    );
    let wo: f64 = AT_THE_TILL
        .iter()
        .map(|&c| n.economy.went_without[c as usize] * n.economy.markets[0].price[c as usize])
        .sum();
    println!("went without, last day         {wo:.4e}");
    println!(
        "
went without, by commodity (tonnes, last day):"
    );
    for &c in AT_THE_TILL.iter() {
        let want: f64 = (0..towns)
            .map(|m| n.economy.markets[m].daily_household_demand(c))
            .sum();
        let gone = n.economy.went_without[c as usize];
        println!(
            "  {:<16} wanted {:.4e}  went without {:.4e}  {:>5.1}%",
            format!("{c:?}"),
            want,
            gone,
            if want > 0.0 { gone / want * 100.0 } else { 0.0 }
        );
    }
}
