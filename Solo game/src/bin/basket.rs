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
    let mut m = Market {
        repairer: Some(45.0),
        power: Some(0.41),
        ..Default::default()
    };
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
    m.finance = Some(Finance {
        rates: scale_sim::bank::Rates::ordinary(),
        lending: true,
    });
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
            Means {
                income: 230.0,
                savings: 20.0,
                credit: 0.0,
            },
        ),
        (
            "a couple with two children",
            Roster::of(2).with(1, 1),
            Climate::temperate(),
            Means {
                income: 640.0,
                savings: 900.0,
                credit: 500.0,
            },
        ),
        (
            "the same family, in Minnesota",
            Roster::of(2).with(1, 1),
            Climate::cold(),
            Means {
                income: 640.0,
                savings: 900.0,
                credit: 500.0,
            },
        ),
        (
            "an office couple",
            Roster::of(2),
            Climate::temperate(),
            Means {
                income: 1_400.0,
                savings: 9_000.0,
                credit: 4_000.0,
            },
        ),
    ];

    println!(
        "  {:<28} {:>8} {:>8} {:>8} {:>7} {:>10}",
        "", "spent/wk", "food %", "power %", "transport %", "short"
    );
    for (k, (name, who, where_, means)) in cases.iter().enumerate() {
        let mut home = Household::new(
            *who,
            *where_,
            Placement::Ground {
                locality: 1,
                x: 0,
                y: 0,
            },
        );
        let mut market = a_town(&cat);
        let mut spent = 0.0;
        let mut food = 0.0;
        let mut transport = 0.0;
        let mut debt = 0.0;
        let mut power = 0.0;
        let mut hours = 0.0;
        let mut short = 0usize;
        let mut dark = 0usize;
        let mut cats: Vec<(Category, f64)> = Vec::new();
        // **What is not spent is put by**, which is how anybody ever gets
        // to own the machine that gives them their evenings back.
        let mut purse = *means;
        for week in 0..52u64 {
            let out = a_period(
                &mut home,
                &mut store,
                &cat,
                &mut market,
                purse,
                7,
                1_000 + k as u64 * 100 + week,
            );
            purse = Means {
                income: means.income,
                ..out.left
            };
            spent += out.spent;
            power += out.utilities;
            debt += out.debt_service;
            transport += out.debt_service;
            hours += out.hours;
            short += out.without.len() + out.unmet.len();
            if out.cut_off {
                dark += 1;
            }
            for (o, paid) in &out.bought {
                if o.need.category() == Category::Food {
                    food += paid;
                }
                if o.need.category() == Category::Transport {
                    transport += paid;
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
        let _ = (&top, dark, hours, debt);
        println!(
            "  {:<28} {:>8.0} {:>7.0}% {:>7.0}% {:>11.0}% {:>7}",
            name,
            spent / 52.0,
            food / spent.max(1.0) * 100.0,
            power / spent.max(1.0) * 100.0,
            transport / spent.max(1.0) * 100.0,
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
            println!("  (BLS 2023, normalised over what this module covers — shelter, pensions");
            println!("  and insurance are 43% of a real budget and are somebody else's line)");
        }
    }

    // ---- what a machine is worth -------------------------------------
    println!("\nWHAT AN APPLIANCE IS ACTUALLY WORTH");
    let home = Household::new(
        Roster::of(4),
        Climate::temperate(),
        Placement::Ground {
            locality: 1,
            x: 0,
            y: 0,
        },
    );
    for (need, machine) in [
        (Need::CleanClothes, "washing machine"),
        (Need::CookedFood, "cooking stove"),
    ] {
        let level = requirement_for(need, home.who, home.where_);
        let by_hand = hours_to_do_it_by_hand(need, level);
        let leaves = what_it_does(machine)
            .iter()
            .find(|s| s.need == need)
            .unwrap()
            .leaves;
        let kwh = what_it_does(machine)
            .iter()
            .find(|s| s.need == need)
            .unwrap()
            .kwh_a_week;
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
    println!(
        "  {:<20} {:>7} {:>7} {:>7} {:>7}",
        "", "1 adult", "2", "4", "2+2 kids"
    );
    for need in [
        Need::Nutrition,
        Need::Warmth,
        Need::CookedFood,
        Need::Clothed,
    ] {
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
    println!(
        "  {:<24} {:>10} {:>8} {:>8}",
        "", "customers", "FTE", "people"
    );
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

    // ---- the bill ----------------------------------------------------
    println!("\nWHEN THEY CANNOT PAY THE BILL");
    println!("  a household using 40 kWh a day and paying nothing at all, from 1 November\n");
    println!(
        "  {:<24} {:>10} {:>10} {:>10} {:>12}",
        "", "notice", "cut off", "owed", "protected"
    );
    for (name, climate) in [
        ("a mild state", Climate::temperate()),
        ("Minnesota", Climate::cold()),
    ] {
        let mut account = scale_sim::utility::Account::new(scale_sim::utility::Tariff::ordinary());
        let (mut noticed, mut cut, mut protected) = (None, None, 0u32);
        for d in 0..365u32 {
            let e = scale_sim::utility::a_day(&mut account, 40.0, 0.0, climate, 305 + d);
            if e.notice && noticed.is_none() {
                noticed = Some(d);
            }
            if e.cut_off && cut.is_none() {
                cut = Some(d);
            }
            if e.protected {
                protected += 1;
            }
        }
        println!(
            "  {:<24} {:>10} {:>10} {:>10.0} {:>12}",
            name,
            noticed
                .map(|d| format!("day {d}"))
                .unwrap_or_else(|| "-".into()),
            cut.map(|d| format!("day {d}"))
                .unwrap_or_else(|| "-".into()),
            account.arrears,
            if protected > 0 {
                format!("{protected} days")
            } else {
                "-".into()
            },
        );
    }
    let t = scale_sim::utility::Tariff::ordinary();
    println!(
        "\n  a month of nothing at all costs {:.0}; 855 kWh costs {:.0}; 1,600 kWh costs {:.0}",
        t.bill_for(0.0),
        t.bill_for(855.0),
        t.bill_for(1_600.0)
    );

    // ---- and what it is worth when it is finished ----------------------
    println!("\nWHAT IT IS WORTH AT THE SCRAPYARD");
    println!("  {:<22} {:>12} {:>16}", "", "$/tonne", "worth hauling");
    for m in [
        scale_sim::material::Material::Copper,
        scale_sim::material::Material::Aluminium,
        scale_sim::material::Material::Lead,
        scale_sim::material::Material::MildSteel,
        scale_sim::material::Material::Paperboard,
        scale_sim::material::Material::Glass,
        scale_sim::material::Material::Abs,
        scale_sim::material::Material::Rubber,
        scale_sim::material::Material::Lithium,
    ] {
        let range = scale_sim::scrap::economic_range_km(m);
        println!(
            "  {:<22} {:>12.0} {:>16}",
            m.name(),
            scale_sim::scrap::price_a_tonne(m),
            if range <= 0.0 {
                "nowhere".to_string()
            } else if scale_sim::scrap::goes_anywhere(m) {
                "anywhere".to_string()
            } else {
                format!("{range:.0} km")
            }
        );
    }

    println!("\n  and a whole thing, at a yard five kilometres away:");
    let washer = [
        (scale_sim::material::Material::MildSteel, 0.0315),
        (scale_sim::material::Material::Concrete, 0.0210),
        (scale_sim::material::Material::Abs, 0.0084),
        (scale_sim::material::Material::Copper, 0.0035),
    ];
    let fridge = [
        (scale_sim::material::Material::MildSteel, 0.0273),
        (scale_sim::material::Material::Abs, 0.0149),
        (scale_sim::material::Material::Polyethylene, 0.0087),
        (scale_sim::material::Material::Copper, 0.0050),
    ];
    println!(
        "  {:<22} {:>12} {:>12} {:>16}",
        "", "as found", "stripped", "worth stripping?"
    );
    for (what, bill) in [
        ("a washing machine", &washer[..]),
        ("a refrigerator", &fridge[..]),
    ] {
        let name = if what.contains("frig") {
            "refrigerator"
        } else {
            "washing machine"
        };
        println!(
            "  {:<22} {:>12.2} {:>12.2} {:>16}",
            what,
            scale_sim::scrap::worth_as_scrap(name, bill, 5.0, false),
            scale_sim::scrap::worth_as_scrap(name, bill, 5.0, true),
            // Three quarters of an hour with a spanner, at an ordinary wage.
            if scale_sim::scrap::worth_stripping(bill, 0.75, 22.0) {
                "yes"
            } else {
                "no"
            }
        );
    }

    // A car through the shredder.
    let car = scale_sim::scrap::Load::of(&[
        (scale_sim::material::Material::MildSteel, 0.870),
        (scale_sim::material::Material::Aluminium, 0.135),
        (scale_sim::material::Material::Abs, 0.165),
        (scale_sim::material::Material::Polyethylene, 0.060),
        (scale_sim::material::Material::Rubber, 0.090),
        (scale_sim::material::Material::Glass, 0.060),
        (scale_sim::material::Material::Copper, 0.045),
        (scale_sim::material::Material::Polyester, 0.030),
        (scale_sim::material::Material::Lubricant, 0.030),
        (scale_sim::material::Material::Lead, 0.015),
    ]);
    let total = car.tonnes();
    let (back, fluff) = scale_sim::scrap::shred(&car);
    let recovered: f64 = back.iter().map(|b| b.1).sum();
    println!(
        "\n  a {:.1} t car through a shredder: {:.0}% back as metal, {:.0} kg of fluff to landfill",
        total,
        recovered / total * 100.0,
        fluff * 1000.0
    );

    // And the same load with a cell left in it.
    let mut with_a_cell = car.clone();
    with_a_cell
        .materials
        .push((scale_sim::material::Material::Lithium, 0.010));
    for (what, yard) in [
        (
            "the yard down the road",
            scale_sim::scrap::Yard::ordinary(1, 5.0),
        ),
        (
            "a licensed processor",
            scale_sim::scrap::Yard::licensed(2, 90.0),
        ),
    ] {
        let s = scale_sim::scrap::weigh_in(&with_a_cell, &yard, 1_000.0);
        let said = match s {
            scale_sim::scrap::Settlement::Taken { paid, .. } => format!("took it, paid {paid:.0}"),
            scale_sim::scrap::Settlement::Refused(r) => format!("turned it away: {}", r.name()),
        };
        println!("  the same car with a cell left in it, {what:<24} {said}");
    }
    // ---- where the money for it came from ------------------------------
    {
        use scale_sim::bank::*;
        use scale_sim::money::{Account, Treasury};

        println!("\nWHERE THE MONEY FOR THE CAR CAME FROM");
        let mut sys = System::new(Rates::ordinary());
        let bk = sys.add_bank(400_000.0, 150_000.0);
        let mut t = Treasury::new();
        t.open(Account::Households(0), 20_000.0);
        t.open(Account::Bank(bk), 400_000.0);

        let before = (t.total(), sys.bank(bk).reserves, sys.bank(bk).deposits);
        let who = Applicant {
            account: Account::Households(0),
            income: 6_100.0,
            existing_payments: 0.0,
            savings: 4_000.0,
            standing: 0.72,
            settled: true,
        };
        let offer = underwrite(
            sys.bank(bk),
            &sys.rates,
            &who,
            Credit::CarLoan,
            9_000.0,
            9_000.0,
        )
        .expect("refused");
        sys.advance(bk, Account::Households(0), &offer, &mut t, 1, None);
        let after = (t.total(), sys.bank(bk).reserves, sys.bank(bk).deposits);

        println!("  {:<34} {:>12} {:>12}", "", "before", "after");
        println!(
            "  {:<34} {:>12.0} {:>12.0}",
            "money in the world", before.0, after.0
        );
        println!(
            "  {:<34} {:>12.0} {:>12.0}",
            "the bank's reserves", before.1, after.1
        );
        println!(
            "  {:<34} {:>12.0} {:>12.0}",
            "the bank's deposits", before.2, after.2
        );
        println!(
            "\n  the loan created {:.0} and moved no reserves at all, and no saver is a penny",
            offer.principal
        );
        println!("  worse off — which is what a bank actually does, whatever the textbooks say");

        println!(
            "\n  {:.0} over {} months at {:.1}%, so {:.0} a month and {:.2} times over",
            offer.principal,
            offer.months,
            offer.rate * 100.0,
            offer.monthly,
            offer.times_over()
        );

        println!("\n  AND WHAT THE SAME CAR COSTS DIFFERENT PEOPLE");
        println!(
            "  {:<26} {:>8} {:>10} {:>12}",
            "", "rate", "a month", "paid in all"
        );
        // The real credit tiers, at the midpoint of each FICO band
        // flattened onto 0..1 by (score - 300) / 550.
        for (what, standing) in [
            ("super prime  781-850", 0.87),
            ("prime        661-780", 0.81),
            ("nonprime     601-660", 0.65),
            ("subprime     501-600", 0.50),
            ("deep subprime  <500", 0.18),
        ] {
            let a = Applicant { standing, ..who };
            match underwrite(
                sys.bank(bk),
                &sys.rates,
                &a,
                Credit::UsedCarLoan,
                9_000.0,
                9_000.0,
            ) {
                Ok(o) => println!(
                    "  {:<26} {:>7.1}% {:>10.0} {:>12.0}",
                    what,
                    o.rate * 100.0,
                    o.monthly,
                    o.monthly * o.months as f64
                ),
                Err(e) => println!("  {:<26} {:>31}", what, e.name()),
            }
        }
        println!("\n  real used-car finance runs 7.1% super prime to 21.6% deep subprime");
    }

    // ---- and how they actually get rid of it --------------------------
    use scale_sim::material::Material as M;
    use scale_sim::scrap::{Carrying, Circumstances, Council};

    println!("\nHOW A DEAD FRIDGE ACTUALLY LEAVES THE HOUSE");
    println!("  62 kg, and worth less than nothing at the yard once the refrigerant is out\n");
    let dead_fridge = [
        (M::MildSteel, 0.0273),
        (M::Abs, 0.0149),
        (M::Polyethylene, 0.0087),
        (M::Copper, 0.0050),
    ];
    let town = Council::a_town();
    let country = Council::out_in_the_country();
    let base = Circumstances {
        name: "refrigerator",
        materials: &dead_fridge,
        kg: 62.0,
        bulky: true,
        still_works: false,
        carrying: Carrying::ACar,
        council: &town,
        collections_used: 9,
        others_going: 0,
        hourly_worth: 15.0,
        scruple: 0.6,
        room_to_store: false,
        somebody_wants_it: false,
        charity_nearby: false,
        being_replaced: false,
        scrappers_about: false,
        somewhere_to_burn: false,
        event: 7,
    };
    println!("  {:<42} {:>28} {:>7}", "", "what they do", "cost");
    for k in 0..8 {
        let mut c = base;
        let what = match k {
            0 => {
                c.being_replaced = true;
                "buying a new one, delivered"
            }
            1 => {
                c.collections_used = 0;
                "the council still owes them a collection"
            }
            2 => {
                c.room_to_store = true;
                "somewhere to put it and forget about it"
            }
            3 => "no truck, no room, in town",
            4 => {
                c.carrying = Carrying::ATruck;
                "a truck, and one fridge"
            }
            5 => {
                c.carrying = Carrying::ATruck;
                c.others_going = 5;
                "a truck, and a yard full of junk"
            }
            6 => {
                c.carrying = Carrying::ATruck;
                c.others_going = 5;
                c.hourly_worth = 300.0;
                "the same, for somebody on 300 an hour"
            }
            _ => {
                c.still_works = true;
                c.somebody_wants_it = true;
                "it still works, and somebody wants it"
            }
        };
        let did = scale_sim::scrap::how_to_get_rid_of_it(&c);
        println!(
            "  {:<42} {:>28} {:>7}",
            what,
            did.name(),
            if did.cost().abs() < 0.005 {
                "-".to_string()
            } else {
                format!("{:.0}", did.cost())
            }
        );
    }

    // The fly-tipping case is a rate over many people, not one man.
    let rate = |council: &Council| {
        let mut tipped = 0;
        for k in 0..300u64 {
            let mut c = base;
            c.council = council;
            c.scruple = -0.9;
            c.event = 40_000 + k;
            if !scale_sim::scrap::how_to_get_rid_of_it(&c).legal() {
                tipped += 1;
            }
        }
        tipped * 100 / 300
    };
    println!(
        "\n  of 300 people with no scruples and no truck, {}% leave it in the woods out in the",
        rate(&country)
    );
    println!(
        "  country against {}% in town — and what makes the difference is being seen",
        rate(&town)
    );

    // ---- a broken machine --------------------------------------------
    println!("\nWHEN THE MACHINE BREAKS");
    let mut store = Store::new();
    let mut home = Household::new(
        Roster::of(2),
        Climate::temperate(),
        Placement::Ground {
            locality: 1,
            x: 0,
            y: 0,
        },
    );
    let id = store.add(
        ItemInstance::one(&cat, cat.must("washing machine")),
        home.home,
    );
    home.owns.push(id);
    let market = a_town(&cat);
    let mut none = a_town(&cat);
    none.repairer = None;
    let purse = Means {
        income: 640.0,
        savings: 900.0,
        credit: 500.0,
    };
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
