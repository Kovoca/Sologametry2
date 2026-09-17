//! **Where each town sits against the world price**, and what crosses the
//! border because of it.
//!
//!   cargo run --release --bin border
//!   cargo run --release --bin border -- --seed 7 --nations 4 --days 300
//!
//! Every tradable commodity in every town has two parity prices: **import
//! parity**, above which bringing a tonne in from abroad pays, and **export
//! parity**, below which sending one out pays. Between them the town
//! neither buys nor sells. This prints where the domestic price actually
//! sits against that band, and how much money went over the border in each
//! direction to get it there.

use scale_sim::econ::{Commodity, Doctrine};
use scale_sim::money::Account;
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Nations;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

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
                eprintln!("usage: border [--seed N] [--nations N] [--days N]");
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

    let (mut paid_abroad, mut earned_abroad) = (0.0f64, 0.0f64);
    let mut worst_food = f64::INFINITY;
    let mut hungry = 0u64;
    // What the import merchants took in and paid out, by reason.
    let mut into_importers: std::collections::BTreeMap<String, f64> = Default::default();
    let mut out_of_importers: std::collections::BTreeMap<String, f64> = Default::default();
    let mut unpaid = 0.0f64;
    let mut unpaid_why: std::collections::BTreeMap<&'static str, f64> = Default::default();
    // **What the country buys abroad, good by good**, against what it makes:
    // tonnes made here, tonnes landed from abroad, and the money each good
    // sends over the border.
    let n_c = Commodity::ALL.len();
    let mut made = vec![0.0f64; n_c];
    let mut landed = vec![0.0f64; n_c];
    let mut paid_for = vec![0.0f64; n_c];
    let mut shipped = vec![0.0f64; n_c];
    let mut earned_for = vec![0.0f64; n_c];
    for _ in 0..days {
        n.economy.step();
        unpaid += n.economy.treasury.unpaid;
        for (k, v) in n.economy.treasury.unpaid_why.iter() {
            *unpaid_why.entry(k).or_default() += v;
        }
        for t in n.economy.treasury.today.iter() {
            if t.to == Account::Abroad && t.from != Account::Abroad {
                paid_abroad += t.amount;
            }
            if t.from == Account::Abroad && t.to != Account::Abroad {
                earned_abroad += t.amount;
            }
            if let Account::Firm(s) = t.to {
                if n.economy.buys_abroad(s) {
                    *into_importers.entry(format!("{:?}", t.why)).or_default() += t.amount;
                }
            }
            if let Account::Firm(s) = t.from {
                if n.economy.buys_abroad(s) {
                    let to = match t.to {
                        Account::Abroad => "abroad".to_string(),
                        Account::Households(_) => "households".to_string(),
                        Account::ServiceSector(_) => "services".to_string(),
                        Account::State(n) => format!("state {n}"),
                        Account::Firm(_) => "firms".to_string(),
                        _ => "other".to_string(),
                    };
                    *out_of_importers
                        .entry(format!("{:?} to {to}", t.why))
                        .or_default() += t.amount;
                }
            }
        }
        let e = &n.economy;
        for site in e.ledger.sites.iter() {
            let Some(r) = site.recipe else { continue };
            let recipe = &scale_sim::econ::RECIPES[r];
            for &(c, q) in recipe.outputs {
                if recipe.from_abroad {
                    landed[c as usize] += site.ran * q;
                } else {
                    made[c as usize] += site.ran * q;
                }
            }
        }
        for &(c, t, money) in e.exported_today.iter() {
            shipped[c as usize] += t;
            earned_for[c as usize] += money;
        }
        for t in e.treasury.today.iter() {
            if t.why != scale_sim::money::Why::Trade || t.to != Account::Abroad {
                continue;
            }
            let Account::Firm(s) = t.from else { continue };
            let Some(r) = e.ledger.sites.get(s).and_then(|x| x.recipe) else {
                continue;
            };
            if let Some(&(c, _)) = scale_sim::econ::RECIPES[r].outputs.first() {
                paid_for[c as usize] += t.amount;
            }
        }
        for m in 0..e.markets.len() {
            let c = Commodity::ProcessedFood;
            let cover = e.markets[m].cover[c as usize] / e.target_cover(m, c);
            worst_food = worst_food.min(cover);
        }
        if e.unmet_demand[Commodity::ProcessedFood as usize] > 1e-6 {
            hungry += 1;
        }
    }
    let e = &n.economy;

    {
        let total: f64 = paid_for.iter().sum();
        println!("what the country buys abroad, by good ({days} days):
");
        println!(
            "  {:<12} {:>12} {:>12} {:>9} {:>11} {:>7} {:>12} {:>11}",
            "", "made (t)", "landed (t)", "imported", "paid abroad", "share", "shipped (t)", "earned"
        );
        let mut order: Vec<usize> = (0..n_c).collect();
        order.sort_by(|&a, &b| (paid_for[b] + earned_for[b]).total_cmp(&(paid_for[a] + earned_for[a])));
        for c in order {
            if made[c] + landed[c] <= 0.0 && paid_for[c] <= 0.0 {
                continue;
            }
            println!(
                "  {:<12} {:>12.3e} {:>12.3e} {:>8.1}% {:>11.3e} {:>6.1}% {:>12.3e} {:>11.3e}",
                Commodity::ALL[c].to_string(),
                made[c],
                landed[c],
                landed[c] / (made[c] + landed[c]).max(1e-9) * 100.0,
                paid_for[c],
                paid_for[c] / total.max(1e-9) * 100.0,
                shipped[c],
                earned_for[c]
            );
        }
        println!();
    }
    println!(
        "seed {seed}, {nations} nations, {} towns, {days} days",
        e.markets.len()
    );
    let quays = (0..e.markets.len()).filter(|&m| e.quay(m)).count();
    println!("{quays} towns have a quay");
    let bought_in = (0..e.markets.len())
        .flat_map(|m| Commodity::ALL.iter().map(move |&c| (m, c)))
        .filter(|&(m, c)| e.daily_draw(m, c) > 1e-9 && !e.supplied_here(m, c))
        .count();
    println!("{bought_in} town-and-commodity pairs are wanted where nobody makes or lands them\n");
    println!("over the border: paid abroad {paid_abroad:.3e}, earned abroad {earned_abroad:.3e}");
    println!(
        "foreign money costs {:.3}, balance {:+.3}, funded {:.0}%, owed abroad {:.3e}",
        e.exchange.foreign_money(),
        e.exchange.imbalance(),
        100.0 * e.exchange.funded_share(),
        e.exchange.owed_abroad(),
    );
    println!(
        "food: worst cover {:.2} of target, hungry on {hungry} of {days} days",
        worst_food
    );
    println!("money owed and not paid, whole run: {unpaid:.3e}, of which");
    for (k, v) in unpaid_why.iter() {
        println!("  {k:<28} {v:.3e}");
    }
    println!();
    println!("import merchants took in:");
    for (k, v) in into_importers.iter() {
        println!("  {k:<28} {v:.3e}");
    }
    println!("and paid out:");
    for (k, v) in out_of_importers.iter() {
        println!("  {k:<28} {v:.3e}");
    }
    let held: f64 = (0..e.ledger.sites.len())
        .filter(|&s| e.buys_abroad(s))
        .map(|s| e.treasury.balance(Account::Firm(s)))
        .sum();
    println!("and hold {held:.3e} at the end\n");

    // Where each town's price sits against its own band.
    println!(
        "{:<14} {:>6} {:>6} {:>6} {:>6}   {:>7} {:>7} {:>7}   {:>5} {:>5}",
        "", "above", "band", "below", "none", "P/W", "IPP/W", "EPP/W", "imp", "exp"
    );
    for &c in Commodity::ALL.iter() {
        if !c.will_go_on_a_ship() {
            continue;
        }
        // **What the world charges here**, which now moves: the rate is one
        // number for the whole modelled world and every parity is built on
        // it, so a ratio against the unconverted reference would read as a
        // shortage after a depreciation when nothing is short.
        let w = e.world_price(c);
        let (mut above, mut band, mut below, mut none) = (0, 0, 0, 0);
        let (mut p, mut ipp, mut epp, mut k) = (0.0, 0.0, 0.0, 0.0);
        let (mut imp, mut exp) = (0, 0);
        for m in 0..e.markets.len() {
            let demand = e.daily_draw(m, c);
            if demand <= 1e-9 {
                none += 1;
                continue;
            }
            let (price, i, x) = (
                e.markets[m].price[c as usize],
                e.import_parity(m, c),
                e.export_parity(m, c),
            );
            if price > i {
                above += 1;
            } else if price < x {
                below += 1;
            } else {
                band += 1;
            }
            p += price / w;
            ipp += i / w;
            epp += x / w;
            k += 1.0;
            if e.worth_importing(m, c) {
                imp += 1;
            }
            if e.worth_exporting(m, c) {
                exp += 1;
            }
        }
        let k = f64::max(k, 1.0);
        println!(
            "{:<14} {:>6} {:>6} {:>6} {:>6}   {:>7.2} {:>7.2} {:>7.2}   {:>5} {:>5}",
            c.to_string(),
            above,
            band,
            below,
            none,
            p / k,
            ipp / k,
            epp / k,
            imp,
            exp
        );
    }

    // Why an importer lands what it lands.
    println!("\nimporters: what limits them");
    println!(
        "  {:<28} {:>8} {:>8} {:>9} {:>9} {:>8} {:>7} {:>7}",
        "", "stock", "room", "cash", "per t", "landed", "P/IPP", "cover"
    );
    for s in 0..e.ledger.sites.len() {
        if !e.buys_abroad(s) {
            continue;
        }
        let site = &e.ledger.sites[s];
        let Some(r) = site.recipe else { continue };
        let Some(&(c, _)) = scale_sim::econ::RECIPES[r].outputs.first() else {
            continue;
        };
        let m = site.market;
        let stock = e.ledger.stock(s, c);
        let room = site.capacity[c as usize] - stock;
        println!(
            "  {:<28} {:>8.0} {:>8.0} {:>9.2e} {:>9.0} {:>8.1} {:>7.2} {:>7.2}",
            site.name,
            stock,
            room,
            e.treasury.balance(Account::Firm(s)),
            0.75 * e.landed_from_abroad(m, c),
            site.ran,
            e.markets[m].price[c as usize] / e.import_parity(m, c),
            e.markets[m].expected_cover[c as usize] / e.target_cover(m, c),
        );
    }

    // A few commodities town by town.
    for c in [Commodity::Grain, Commodity::Steel, Commodity::Cement] {
        println!("\n{c}: world {:.0}", e.world_price(c));
        println!(
            "  {:<22} {:>5} {:>8} {:>8} {:>8} {:>8} {:>7}",
            "town", "quay", "inland", "price", "import", "export", "cover"
        );
        for m in 0..e.markets.len() {
            if e.daily_draw(m, c) <= 1e-9 {
                continue;
            }
            let cover = e.markets[m].expected_cover[c as usize] / e.target_cover(m, c);
            println!(
                "  {:<22} {:>5} {:>8.0} {:>8.0} {:>8.0} {:>8.0} {:>7.2}",
                e.markets[m].name,
                if e.quay(m) { "yes" } else { "" },
                e.inland_leg(m),
                e.markets[m].price[c as usize],
                e.import_parity(m, c),
                e.export_parity(m, c),
                cover
            );
        }
    }
}
