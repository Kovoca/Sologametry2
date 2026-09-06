//! **What a household actually buys.**
//!
//! `cargo run --release --bin basket`
//!
//! Four households in the same town with the same shops, differing only in
//! who is in them, where they are and what they earn. Nothing here writes
//! down a share of spending; the shares at the bottom are what came out.

use scale_sim::basket::*;
use scale_sim::item::{standard_catalogue, Catalogue, ItemInstance, Placement, Store};

fn a_town(cat: &Catalogue) -> Market {
    let shop = 1u64;
    let mut m = Market { repairer: Some(45.0), power: Some(0.41), ..Default::default() };
    for (name, price) in [
        ("washing machine", 700.0),
        ("cooking stove", 900.0),
        ("refrigerator", 800.0),
        ("space heater", 90.0),
        ("electric light", 25.0),
        ("bed", 500.0),
        ("bicycle", 400.0),
        ("motor car", 9_000.0),
        ("cooking pot", 40.0),
        ("wash tub", 30.0),
        ("water butt", 70.0),
        ("coat", 120.0),
        ("work clothes", 60.0),
        ("telephone", 600.0),
        ("radio set", 80.0),
    ] {
        m.stock(Stall::new(cat.must(name), price, 999, shop));
    }
    m.services = vec![
        (Need::Nutrition, 8.5),
        (Need::Health, 2.0),
        (Need::Water, 0.4),
        (Need::CleanClothes, 6.0),
        (Need::Mobility, 4.0),
        (Need::CookedFood, 9.0),
    ];
    m
}

fn main() {
    let cat = standard_catalogue();
    let mut store = Store::new();

    println!("WHAT FOUR HOUSEHOLDS BUY IN A YEAR");
    println!("  the same town, the same shops, the same prices\n");

    let cases: [(&str, Roster, Climate, Means); 4] = [
        (
            "a labourer, alone",
            Roster::of(1),
            Climate::temperate(),
            Means { income: 230.0, savings: 20.0, credit: 0.0 },
        ),
        (
            "a couple with two children",
            Roster::of(2).with(1, 1),
            Climate::temperate(),
            Means { income: 640.0, savings: 900.0, credit: 500.0 },
        ),
        (
            "the same family, in Minnesota",
            Roster::of(2).with(1, 1),
            Climate::cold(),
            Means { income: 640.0, savings: 900.0, credit: 500.0 },
        ),
        (
            "an office couple",
            Roster::of(2),
            Climate::temperate(),
            Means { income: 1_400.0, savings: 9_000.0, credit: 4_000.0 },
        ),
    ];

    println!(
        "  {:<28} {:>8} {:>8} {:>8} {:>7} {:>10}",
        "", "spent/wk", "food %", "power %", "hours", "short"
    );
    for (k, (name, who, where_, means)) in cases.iter().enumerate() {
        let mut home = Household::new(*who, *where_, Placement::Ground { locality: 1, x: 0, y: 0 });
        let mut market = a_town(&cat);
        let mut spent = 0.0;
        let mut food = 0.0;
        let mut power = 0.0;
        let mut hours = 0.0;
        let mut short = 0usize;
        let mut dark = 0usize;
        let mut cats: Vec<(Category, f64)> = Vec::new();
        // **What is not spent is put by**, which is how anybody ever gets
        // to own the machine that gives them their evenings back.
        let mut purse = *means;
        for week in 0..52u64 {
            let out =
                a_period(&mut home, &mut store, &cat, &mut market, purse, 7, 1_000 + k as u64 * 100 + week);
            purse = Means { income: means.income, ..out.left };
            spent += out.spent;
            power += out.utilities;
            hours += out.hours;
            short += out.without.len() + out.unmet.len();
            if out.cut_off {
                dark += 1;
            }
            for (o, paid) in &out.bought {
                if o.need.category() == Category::Food {
                    food += paid;
                }
                match cats.iter_mut().find(|c| c.0 == o.need.category()) {
                    Some(c) => c.1 += paid,
                    None => cats.push((o.need.category(), *paid)),
                }
            }
        }
        cats.sort_by(|a, b| b.1.total_cmp(&a.1));
        let top: Vec<String> = cats
            .iter()
            .take(3)
            .map(|(c, v)| format!("{} {:.0}%", c.name(), v / spent.max(1.0) * 100.0))
            .collect();
        let _ = (short, &top, dark);
        println!(
            "  {:<28} {:>8.0} {:>7.0}% {:>7.0}% {:>7.0} {:>7}",
            name,
            spent / 52.0,
            food / spent.max(1.0) * 100.0,
            power / spent.max(1.0) * 100.0,
            hours / 52.0,
            short,
        );
        if k == cases.len() - 1 {
            println!(
                "
  the same categories in a real US budget: food 22%, utilities 12%,"
            );
            println!(
                "  transport 30%, healthcare 14%, household goods 9%, recreation 9%, clothing 4%"
            );
            println!(
                "  (BLS 2023, normalised over what this module covers — shelter, pensions"
            );
            println!("  and insurance are 43% of a real budget and are somebody else's line)");
        }
    }

    // ---- what a machine is worth -------------------------------------
    println!("\nWHAT AN APPLIANCE IS ACTUALLY WORTH");
    let home = Household::new(
        Roster::of(4),
        Climate::temperate(),
        Placement::Ground { locality: 1, x: 0, y: 0 },
    );
    for (need, machine) in
        [(Need::CleanClothes, "washing machine"), (Need::CookedFood, "cooking stove")]
    {
        let level = requirement_for(need, home.who, home.where_);
        let by_hand = hours_to_do_it_by_hand(need, level);
        let leaves = what_it_does(machine).iter().find(|s| s.need == need).unwrap().leaves;
        let kwh = what_it_does(machine).iter().find(|s| s.need == need).unwrap().kwh_a_week;
        println!(
            "  {:<16} {:>5.1} h a week by hand, {:>4.1} h with a {} ({:.0} kWh a week to run)",
            need.name(),
            by_hand,
            by_hand * leaves,
            machine,
            kwh,
        );
    }

    // ---- what a household needs, and how it scales -------------------
    println!("\nWHO IS IN IT, AND WHAT THAT COSTS");
    println!("  {:<20} {:>7} {:>7} {:>7} {:>7}", "", "1 adult", "2", "4", "2+2 kids");
    for need in [Need::Nutrition, Need::Warmth, Need::CookedFood, Need::Clothed] {
        let at = |r: Roster| requirement_for(need, r, Climate::temperate());
        println!(
            "  {:<20} {:>7.2} {:>7.2} {:>7.2} {:>7.2}",
            need.name(),
            at(Roster::of(1)),
            at(Roster::of(2)),
            at(Roster::of(4)),
            at(Roster::of(2).with(1, 1)),
        );
    }

    // ---- and the shop on the other side of the counter ---------------
    println!("\nAND THE SHOP THAT SERVES THEM");
    println!("  {:<24} {:>10} {:>8} {:>8}", "", "customers", "FTE", "people");
    for (what, per_day, m2) in [
        ("a corner shop", 260.0, 180.0),
        ("a convenience store", 900.0, 900.0),
        ("a supermarket", 2_857.0, 3_700.0),
        ("a supercentre", 7_000.0, 17_000.0),
    ] {
        let fte = shop_staff_for(per_day, m2);
        println!(
            "  {:<24} {:>10.0} {:>8.0} {:>8.0}",
            what,
            per_day,
            fte,
            heads_from_fte(fte)
        );
    }

    // ---- a broken machine --------------------------------------------
    println!("\nWHEN THE MACHINE BREAKS");
    let mut store = Store::new();
    let mut home = Household::new(
        Roster::of(2),
        Climate::temperate(),
        Placement::Ground { locality: 1, x: 0, y: 0 },
    );
    let id = store.add(ItemInstance::one(&cat, cat.must("washing machine")), home.home);
    home.owns.push(id);
    let market = a_town(&cat);
    let mut none = a_town(&cat);
    none.repairer = None;
    let purse = Means { income: 640.0, savings: 900.0, credit: 500.0 };
    for damage in [0.0, 0.4, 0.6, 0.9] {
        store.get_mut(id).unwrap().condition.damage = damage;
        let with = what_to_do(Need::CleanClothes, 1.0, &home, &store, &cat, &market, purse);
        let without = what_to_do(Need::CleanClothes, 1.0, &home, &store, &cat, &none, purse);
        println!(
            "  damage {:.1}   with a repairer: {:<22} without one: {}",
            damage,
            with.name(),
            without.name()
        );
    }
}
