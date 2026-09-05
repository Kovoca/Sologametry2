//! **Watch something get made, and then take it apart.**
//!
//!   cargo run --release --bin make

use scale_sim::craft::{
    hand_tools, machine_shop, standard_recipes, Halt, Maker, RecipeBook, WorkOrder, Workplace,
};
use scale_sim::bom::{audit, depth_of, explode, plan_coverage, tree, validate, Origin};
use scale_sim::item::{standard_catalogue, Catalogue, Family, ItemInstance};
use scale_sim::teardown::{heat_mj, take_apart, Teardown};

fn run(o: &mut WorkOrder, book: &RecipeBook, cat: &Catalogue, place: &Workplace, m: Maker) {
    for _ in 0..2000 {
        if o.finished() {
            break;
        }
        let before = o.elapsed_min;
        o.advance(15.0, book, cat, place, m);
        if o.elapsed_min == before && !matches!(o.state, Halt::Running | Halt::Unattended) {
            break;
        }
    }
}

fn main() {
    let cat = standard_catalogue();
    let book = standard_recipes(&cat);
    let hand = Maker { skill: 0.8, proficiency: 0.75, knows_recipe: true, tool_familiarity: 0.9,
                       focus: 0.85, fatigue: 0.15 };

    println!("{} item definitions, {} plans\n", cat.len(), book.recipes.len());
    for f in [Family::Stock, Family::Fastening, Family::Tool, Family::Machine, Family::Furniture,
              Family::Clothing, Family::Appliance, Family::SparePart, Family::Firearm,
              Family::Ammunition, Family::Foodstuff] {
        let names: Vec<&str> = cat.of_family(f).map(|d| d.name).take(6).collect();
        println!("  {:<12?} {}", f, names.join(", "));
    }

    // ---- what a thing is made of, all the way down ------------------
    println!("\nWHAT IS IN A CORDLESS DRILL");
    let drill = cat.must("cordless drill");
    let mut s = String::new();
    tree(&cat, drill, cat.get(drill).unwrap().nominal_mass_kg, 1, &mut s);
    print!("{s}");
    println!("  flattened to materials:");
    for (m, kg) in explode(&cat, drill, 1.6) {
        println!("    {:>7.4} kg  {}", kg, m.name());
    }

    println!("\nHOW DEEP THE TREES GO");
    for name in ["oak board", "wooden chair", "toaster", "cordless drill", "rifle",
                 "car door", "washing machine"] {
        let id = cat.must(name);
        let d = cat.get(id).unwrap();
        let flat = explode(&cat, id, d.nominal_mass_kg);
        println!(
            "  {name:<18} {:>7.2} kg  {} levels  {} parts  {} materials",
            d.nominal_mass_kg,
            depth_of(&cat, id),
            d.bill.components.iter().map(|c| c.count).sum::<u32>(),
            flat.len()
        );
    }

    println!("
WHAT IS IN A CAR DOOR");
    let cd = cat.must("car door");
    let mut s = String::new();
    tree(&cat, cd, cat.get(cd).unwrap().nominal_mass_kg, 1, &mut s);
    print!("{s}");
    let worst = audit(&cat, cd, 28.1)
        .iter()
        .map(|n| n.residual().abs())
        .fold(0.0f64, f64::max);
    println!("  worst residual anywhere in the tree: {worst:.6} kg");

    println!("
WHAT ANYBODY HERE CAN ACTUALLY MAKE");
    for (family, made, all) in plan_coverage(&cat) {
        println!("  {:<12?} {made:>3} / {all:<3}  {:>5.0}%", family, 100.0 * made as f64 / all as f64);
    }

    let findings = validate(&cat, &book.recipes.iter().map(|r| r.name).collect::<Vec<_>>());
    let industrial = cat
        .iter()
        .filter(|d| d.origin.iter().any(|o| matches!(o, Origin::Industrial { .. })))
        .count();
    println!(
        "\n  {} definitions, {} content errors, {} still awaiting a written plan",
        cat.len(),
        findings.len(),
        industrial
    );

    // ---- the same chair, three ways ---------------------------------
    println!("\nA WOODEN CHAIR, THREE WAYS");
    println!("  {:<22} {:>8} {:>9} {:>8} {:>7} {:>8}", "", "labour", "on the", "power", "mass", "quality");
    println!("  {:<22} {:>8} {:>9} {:>8} {:>7} {:>8}", "", "hours", "clock", "kWh", "kg", "");
    let cases: [(&str, Workplace); 3] = [
        ("a man at a bench", Workplace::a_workshop(hand_tools(&cat))),
        ("a cabinet shop", Workplace::a_factory(machine_shop(&cat), 2.0, 1)),
        ("a furniture works", Workplace::a_factory(machine_shop(&cat), 8.0, 6)),
    ];
    for (name, place) in &cases {
        let mut o = WorkOrder::begin(1, book.must("chair, hand tools"), 1, 0, 1);
        run(&mut o, &book, &cat, place, hand);
        let chair = o.deliver(&book, &cat, 1);
        let (labour, elapsed) = o.labour_vs_elapsed();
        println!(
            "  {name:<22} {:>8.1} {:>8.1}h {:>8.1} {:>7.2} {:>8.2}",
            labour / 60.0,
            elapsed / 60.0,
            o.power_kwh,
            chair.as_ref().map(|c| c.mass_kg).unwrap_or(0.0),
            chair.as_ref().map(|c| c.quality.overall()).unwrap_or(0.0),
        );
    }
    println!("  Twelve hours of the clock is glue curing, and none of it is anybody's day.");

    // ---- what a batch buys ------------------------------------------
    println!("\nWHAT A BATCH BUYS");
    let f = book.get(book.must("chair, factory")).unwrap();
    for n in [1u32, 5, 20, 100, 500] {
        println!("  {n:>4} chairs   {:>7.2} labour-hours each", f.labour_for_batch(n) / 60.0 / n as f64);
    }
    println!("  It saves the setup, once. It never saves making the chair.");

    // ---- labour against the clock -----------------------------------
    println!("\nLABOUR AGAINST THE CLOCK");
    println!("  {:<24} {:>8} {:>8} {:>8}", "", "labour", "machine", "clock");
    for name in ["loaf", "chair, hand tools", "work trousers", "rifle, assembled",
                 "cartridge, handloaded"] {
        let r = book.get(book.must(name)).unwrap();
        println!(
            "  {name:<24} {:>8.0} {:>8.0} {:>8.0}   min",
            r.labour_minutes(),
            r.machine_minutes(),
            r.span_minutes()
        );
    }
    println!("  Staff a bakery off the clock and you hire three and a half times too many.");

    // ---- oak against particleboard ----------------------------------
    println!("\nTWO CHAIRS, AND WHAT COMES BACK OUT OF THEM");
    let bench = Workplace::a_workshop(hand_tools(&cat));
    let make = |cheap: bool| -> ItemInstance {
        let mut o = WorkOrder::begin(9, book.must("chair, hand tools"), 1, 0, 1);
        if cheap {
            o.substituted(cat.must("oak board"), cat.must("particleboard sheet"), &cat, &book)
                .expect("a particleboard sheet will not do for a chair");
        }
        run(&mut o, &book, &cat, &bench, hand);
        o.deliver(&book, &cat, 1).unwrap()
    };
    for (label, chair) in [("oak", make(false)), ("particleboard", make(true))] {
        let rec = chair.assembly.as_ref().unwrap();
        let parts: Vec<String> = rec
            .components
            .iter()
            .map(|c| {
                format!(
                    "{} ({})",
                    cat.get(c.definition).map(|d| d.name).unwrap_or("?"),
                    c.materials.chiefly().map(|m| m.name()).unwrap_or("?")
                )
            })
            .collect();
        println!("\n  a chair of {label}, {:.2} kg", chair.mass_kg);
        println!("    made of: {}", parts.join(", "));
        let apart = take_apart(&chair, Teardown::Deconstruct, &cat, 0.8, 1);
        let back: Vec<String> = apart
            .components
            .iter()
            .map(|c| format!("{} x{}", cat.get(c.definition).map(|d| d.name).unwrap_or("?"), c.count))
            .collect();
        println!("    taken apart: {}", if back.is_empty() { "nothing whole".into() } else { back.join(", ") });
        for (m, kg) in &apart.materials {
            println!("      {:>6.3} kg of {} for the furnace", kg, m.name());
        }
        for (m, kg) in &apart.fuel {
            println!("      {:>6.3} kg of {} to burn ({:.0} MJ)", kg, m.name(), heat_mj(*m, *kg));
        }
        println!("      {:>6.3} kg lost", apart.lost_kg);
    }

    // ---- nine ways to take one thing apart --------------------------
    println!("\nNINE INTENTIONS, ONE CHAIR");
    let chair = make(false);
    println!("  {:<14} {:>6} {:>9} {:>9} {:>8}", "", "parts", "material", "to burn", "lost");
    for how in [Teardown::Disassemble, Teardown::Deconstruct, Teardown::Salvage,
                Teardown::Recycle, Teardown::CutUp, Teardown::Smash] {
        let r = take_apart(&chair, how, &cat, 0.8, 1);
        println!(
            "  {:<14} {:>6} {:>8.3} {:>9.3} {:>8.3}   {:.0} min",
            how.name(),
            r.components.iter().map(|c| c.count).sum::<u32>(),
            r.materials.iter().map(|m| m.1).sum::<f64>(),
            r.fuel_kg(),
            r.lost_kg,
            r.minutes,
        );
    }

    // ---- and a blackout ---------------------------------------------
    println!("\nTHE GRID GOES DOWN");
    for (name, mut place) in [
        ("a man at a bench", Workplace::a_workshop(hand_tools(&cat))),
        ("a furniture works", Workplace::a_factory(machine_shop(&cat), 8.0, 6)),
    ] {
        place.power = false;
        let mut o = WorkOrder::begin(3, book.must("chair, hand tools"), 1, 0, 1);
        run(&mut o, &book, &cat, &place, hand);
        println!("  {name:<22} {:?}", o.state);
    }
    let mut dark = Workplace::a_factory(machine_shop(&cat), 4.0, 1);
    dark.power = false;
    let mut o = WorkOrder::begin(4, book.must("loaf"), 1, 0, 1);
    run(&mut o, &book, &cat, &dark, hand);
    println!("  {:<22} {:?} at step {} of 4 — the dough is proved and the oven is cold",
             "a bakery", o.state, o.step);
}
