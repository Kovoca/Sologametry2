//! **What a thing is, and what has happened to it.**
//!
//! The gates here are the ones a crafting menu quietly fails: that a
//! well-made worn knife and a badly-made new one are different objects,
//! that a part taken out of a lorry is the part that went in, and that
//! folding a warehouse into a number and unfolding it again gives back the
//! same warehouse.

use scale_sim::item::{
    standard_catalogue, Condition, Family, ItemInstance, ItemLot, Lifecycle, Quality, Refusal,
    Store,
};
use scale_sim::material::{Composition, Dims, Material, Quantity, Recovers};

// =====================================================================
// quantities
// =====================================================================

/// **Not everything is a charge.** Rope is a length, flour is a mass, fuel
/// is a volume at a temperature, and a cartridge is one of a count — and
/// the reason to keep them apart is that half a rope is two ropes while
/// half a cartridge is nothing at all.
#[test]
fn a_quantity_is_in_the_unit_the_thing_is_measured_in() {
    let rope = Quantity::Length { metres: 10.0, kg: 3.2 };
    let flour = Quantity::Mass { kg: 25.0 };
    let rounds = Quantity::Count(20);
    let diesel = Quantity::Fluid { litres: 40.0, kg: 33.6, celsius: 12.0 };

    assert!(rope.divisible() && flour.divisible() && diesel.divisible());
    assert!(!rounds.divisible(), "a cartridge came apart into fractions");

    // Every one of them has a mass, because the conservation check rests
    // on it. Energy is the one deliberate exception.
    assert!((rope.mass_kg(0.0) - 3.2).abs() < 1e-9);
    assert!((rounds.mass_kg(0.0124) - 0.248).abs() < 1e-9);
    assert_eq!(Quantity::Energy { kwh: 5.0 }.mass_kg(0.0), 0.0);
}

/// **Two stacks merge only if what is being discarded does not matter.**
/// Silently merging is how ammunition of two loadings becomes one
/// indistinguishable pile.
#[test]
fn stacks_do_not_merge_when_the_difference_matters() {
    let cold = Quantity::Fluid { litres: 10.0, kg: 10.0, celsius: 4.0 };
    let warm = Quantity::Fluid { litres: 10.0, kg: 10.0, celsius: 60.0 };
    let cool = Quantity::Fluid { litres: 5.0, kg: 5.0, celsius: 6.0 };
    assert!(!cold.mergeable_with(warm), "hot and cold water became one tank");
    assert!(cold.mergeable_with(cool));

    // And mixing carries the temperature with the mass rather than losing
    // it, which is the whole reason it sits on the quantity.
    let mixed = cold.merged(cool).expect("two cool fluids would not mix");
    match mixed {
        Quantity::Fluid { kg, celsius, .. } => {
            assert!((kg - 15.0).abs() < 1e-9);
            assert!(celsius > 4.0 && celsius < 6.0, "temperature came out at {celsius}");
        }
        _ => panic!("mixing two fluids gave something else"),
    }

    let a = Quantity::Stock { count: 4, each: Dims::new(2.4, 0.15, 0.025), kg: 27.0 };
    let b = Quantity::Stock { count: 4, each: Dims::new(3.0, 0.15, 0.025), kg: 33.75 };
    assert!(!a.mergeable_with(b), "boards of two lengths became one pile");
    assert!(Quantity::Count(3).merged(Quantity::Count(4)) == Some(Quantity::Count(7)));
}

/// A composition is proportions, and its density follows from them — which
/// is what stops a wooden chair weighing what a steel one does.
#[test]
fn what_a_thing_is_made_of_decides_what_it_weighs() {
    let drill = Composition::of(&[
        (Material::Abs, 0.45),
        (Material::MildSteel, 0.35),
        (Material::Copper, 0.20),
    ]);
    assert_eq!(drill.chiefly(), Some(Material::Abs));
    assert!((drill.fraction_of(Material::Copper) - 0.20).abs() < 1e-6);
    let masses = drill.masses(1.6);
    assert!((masses.iter().map(|m| m.1).sum::<f64>() - 1.6).abs() < 1e-9);

    // Density is volume-weighted, so a mostly-plastic tool is far lighter
    // than its metal content suggests.
    assert!(drill.density() > Material::Abs.density());
    assert!(drill.density() < Material::MildSteel.density());
}

/// **What a material can ever come back as** is a property of the
/// material, not of how carefully somebody took it apart.
#[test]
fn cured_and_cooked_things_do_not_come_back() {
    assert_eq!(Material::MildSteel.recovers(), Recovers::Feedstock);
    assert_eq!(Material::Glass.recovers(), Recovers::Feedstock);
    assert_eq!(Material::Concrete.recovers(), Recovers::Downcycled);
    assert_eq!(Material::Oak.recovers(), Recovers::Fuel);
    assert_eq!(Material::Adhesive.recovers(), Recovers::Nothing);
    assert_eq!(Material::Flour.recovers(), Recovers::Nothing);
}

// =====================================================================
// quality against condition
// =====================================================================

/// **Gate: quality is not condition.**
///
/// Four objects that a single 0..1 score cannot tell apart, and the one
/// consequence that matters — repairing the worn one does not make it well
/// made.
#[test]
fn a_well_made_worn_thing_is_not_a_badly_made_new_one() {
    let cat = standard_catalogue();
    let id = cat.must("wooden chair");

    let fine = Quality { workmanship: 0.95, structural_integrity: 0.95,
                         dimensional_accuracy: 0.95, finish: 0.9, ..Quality::default() };
    let poor = Quality { workmanship: 0.25, structural_integrity: 0.35,
                         dimensional_accuracy: 0.3, finish: 0.2, ..Quality::default() };

    let mut fine_and_worn = ItemInstance::one(&cat, id);
    fine_and_worn.quality = fine;
    fine_and_worn.condition = Condition { wear: 0.7, ..Condition::fresh() };

    let mut poor_and_new = ItemInstance::one(&cat, id);
    poor_and_new.quality = poor;

    // They are different objects, and neither dimension is recoverable
    // from the other.
    assert!(fine_and_worn.quality.overall() > poor_and_new.quality.overall());
    assert!(fine_and_worn.condition.serviceability() < poor_and_new.condition.serviceability());

    // **Repair moves condition and leaves workmanship exactly alone.**
    let was = fine_and_worn.quality.workmanship;
    fine_and_worn.repair(1.0);
    assert_eq!(fine_and_worn.quality.workmanship, was, "mending it improved the joinery");
    assert!(fine_and_worn.condition.serviceability() > 0.6, "a full repair fixed nothing");

    let was = poor_and_new.quality.workmanship;
    poor_and_new.repair(1.0);
    assert_eq!(poor_and_new.quality.workmanship, was, "a cheap chair stopped being cheap");
}

/// A damaged part does not stop being a well-made part, and a fault that
/// disables the thing is not the same as wear.
#[test]
fn a_disabling_fault_is_not_a_worn_tool() {
    let cat = standard_catalogue();
    let mut drill = ItemInstance::one(&cat, cat.must("cordless drill"));
    drill.condition.wear = 0.4;
    let worn = drill.effectiveness();
    assert!(worn > 0.0 && worn < 1.0);

    drill.faults.push(scale_sim::item::Fault {
        what: "burnt-out motor",
        severity: 0.9,
        disabling: true,
        since_day: 3,
    });
    assert_eq!(drill.effectiveness(), 0.0, "a burnt-out drill still drilled");
}

// =====================================================================
// installation
// =====================================================================

/// **Gate: a removed part is the part that was installed.**
///
/// Fitting an alternator must not destroy an item and create a statistic.
/// Take it out again and you get back the same object, with its hours, its
/// wear and its maker.
#[test]
fn a_part_taken_out_is_the_part_that_went_in() {
    let cat = standard_catalogue();
    let mut store = Store::new();

    let washer = store.add(ItemInstance::one(&cat, cat.must("washing machine")));
    let mut alt = ItemInstance::one(&cat, cat.must("alternator"));
    alt.condition.wear = 0.31;
    alt.quality.workmanship = 0.88;
    alt.given_name = Some("the one off the old machine".into());
    let alt_id = store.add(alt);

    let fitting = store.install(washer, alt_id, &cat).expect("the bracket would not take it");
    assert!(store.get(alt_id).unwrap().is_installed());
    // The mass of the machine now includes it.
    assert!(store.laden_mass(washer) > 70.0);

    // Nothing else may take the same point while it is occupied.
    let second = store.add(ItemInstance::one(&cat, cat.must("alternator")));
    assert_eq!(store.install(washer, second, &cat), Err(Refusal::PointTaken));

    let back = store.uninstall(washer, fitting).expect("it would not come off");
    assert_eq!(back, alt_id, "a different alternator came off than went on");
    let a = store.get(back).unwrap();
    assert!((a.condition.wear - 0.31).abs() < 1e-9, "its hours were forgotten");
    assert_eq!(a.quality.workmanship, 0.88);
    assert_eq!(a.given_name.as_deref(), Some("the one off the old machine"));
    assert!(!a.is_installed());
}

/// A thing only goes where it fits, and a thing cannot contain itself.
#[test]
fn a_fitting_is_a_shape_and_not_a_permission() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let rifle = store.add(ItemInstance::one(&cat, cat.must("rifle")));
    let mag = store.add(ItemInstance::one(&cat, cat.must("magazine")));
    let loaf = store.add(ItemInstance::one(&cat, cat.must("loaf")));

    assert!(store.install(rifle, mag, &cat).is_ok());
    assert_eq!(store.install(rifle, loaf, &cat), Err(Refusal::DoesNotFit));
    assert_eq!(store.put_in(rifle, rifle, &cat), Err(Refusal::WouldContainItself));
}

/// A worn tool is slower and less precise, which is the whole reason
/// maintenance is worth paying for.
#[test]
fn a_worn_tool_holds_a_worse_tolerance() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let good = store.add(ItemInstance::one(&cat, cat.must("bandsaw")));
    let mut tired = ItemInstance::one(&cat, cat.must("bandsaw"));
    tired.condition.wear = 0.75;
    let tired = store.add(tired);

    let a = store.capability(good, scale_sim::item::Capability::CutWood, &cat).unwrap();
    let b = store.capability(tired, scale_sim::item::Capability::CutWood, &cat).unwrap();
    assert!(b.speed < a.speed, "a worn saw cut just as fast");
    assert!(b.precision_mm > a.precision_mm, "a worn saw held the same tolerance");
}

// =====================================================================
// lifecycle
// =====================================================================

/// **Gate: a durable is not bought again every tick.**
///
/// The distinction the demand model will stand on: a loaf is gone
/// tomorrow, a coat lasts a few years, a washing machine eleven. Treating
/// all three as a flow of tonnes is what made "retail goods" one number.
#[test]
fn a_washing_machine_is_not_a_loaf() {
    let cat = standard_catalogue();
    let loaf = cat.get(cat.must("loaf")).unwrap();
    let coat = cat.get(cat.must("coat")).unwrap();
    let washer = cat.get(cat.must("washing machine")).unwrap();
    let screws = cat.get(cat.must("wood screw")).unwrap();

    assert_eq!(loaf.lifecycle, Lifecycle::Perishable);
    assert_eq!(coat.lifecycle, Lifecycle::SemiDurable);
    assert_eq!(washer.lifecycle, Lifecycle::Durable);
    assert_eq!(screws.lifecycle, Lifecycle::Consumable);

    // The purchase rate is the reciprocal of the life, and the spread
    // across the families is three orders of magnitude.
    let per_year = |l: Lifecycle| 365.0 / l.typical_life_days();
    assert!(per_year(Lifecycle::Perishable) > 50.0);
    assert!(per_year(Lifecycle::Durable) < 0.15, "a washing machine a year");
    assert!(per_year(Lifecycle::Perishable) > 400.0 * per_year(Lifecycle::Durable));
}

/// Every family knows what kind of thing it is, so the basket does not
/// have to be told twice.
#[test]
fn every_family_has_a_lifecycle_and_the_catalogue_covers_the_basket() {
    let cat = standard_catalogue();
    for f in [Family::Stock, Family::Fastening, Family::Tool, Family::Machine,
              Family::Furniture, Family::Clothing, Family::Appliance, Family::Ammunition,
              Family::Firearm, Family::SparePart, Family::Foodstuff] {
        assert!(cat.of_family(f).next().is_some(), "nothing in the catalogue is {f:?}");
    }
}

// =====================================================================
// lots
// =====================================================================

/// **Gate: folding stock into a number and unfolding it conserves it.**
#[test]
fn a_lot_expands_to_what_was_folded_into_it() {
    let cat = standard_catalogue();
    let id = cat.must("wood screw");

    let mut boxes = Vec::new();
    for k in 0..40 {
        let mut s = ItemInstance::fresh(&cat, id, Quantity::Count(1));
        s.quality.workmanship = 0.5 + (k % 7) as f64 * 0.05;
        s.condition.wear = (k % 5) as f64 * 0.05;
        boxes.push(s);
    }
    let mass_before: f64 = boxes.iter().map(|i| i.mass_kg).sum();

    let (lot, left) = ItemLot::aggregate(id, boxes, 100);
    let lot = lot.expect("nothing was folded up");
    assert!(left.is_empty());
    assert_eq!(lot.count, 40);
    assert!((lot.total_mass_kg - mass_before).abs() < 1e-9);
    assert!(lot.quality_spread > 0.0, "forty different examples came out identical");

    let back = lot.expand(&cat, 7);
    assert_eq!(back.len() as u32, lot.count, "count was not conserved");
    let mass_after: f64 = back.iter().map(|i| i.mass_kg).sum();
    assert!((mass_after - mass_before).abs() < 1e-9, "mass was not conserved");

    // And expansion is deterministic: looking at a warehouse twice does
    // not give two warehouses.
    let again = lot.expand(&cat, 7);
    for (a, b) in back.iter().zip(&again) {
        assert_eq!(a.quality.workmanship, b.quality.workmanship);
        assert_eq!(a.condition.wear, b.condition.wear);
    }
    // Nor does it flatten them all to the mean.
    let spread = back.iter().map(|i| i.quality.overall()).fold(0.0f64, f64::max)
        - back.iter().map(|i| i.quality.overall()).fold(1.0f64, f64::min);
    assert!(spread > 0.02, "expanding gave forty identical screws");
}

/// **What must never be folded up.** A named thing, somebody's property, a
/// loaded gun, a modified tool, work in progress — those are exactly the
/// objects a story is made of.
#[test]
fn particular_things_refuse_to_become_a_statistic() {
    let cat = standard_catalogue();
    let id = cat.must("rifle");
    let mut store = Store::new();

    let plain = ItemInstance::one(&cat, id);
    assert!(plain.aggregatable());

    let mut named = ItemInstance::one(&cat, id);
    named.given_name = Some("Bess".into());

    let mut owned = ItemInstance::one(&cat, id);
    owned.ownership.accounted_for = true;

    let mut faulty = ItemInstance::one(&cat, id);
    faulty.faults.push(scale_sim::item::Fault {
        what: "cracked stock", severity: 0.4, disabling: false, since_day: 1,
    });

    let mut marked = ItemInstance::one(&cat, id);
    marked.provenance.marked = true;

    // A loaded gun: the magazine is a fitted part, so it is modified.
    let host = store.add(ItemInstance::one(&cat, id));
    let mag = store.add(ItemInstance::one(&cat, cat.must("magazine")));
    store.install(host, mag, &cat).unwrap();
    let loaded = store.get(host).unwrap().clone();

    for (what, item) in [("named", named), ("owned", owned), ("faulty", faulty),
                         ("marked", marked), ("loaded", loaded)] {
        assert!(!item.aggregatable(), "a {what} rifle was folded into a lot");
    }

    let (lot, kept) = ItemLot::aggregate(id, vec![plain, ItemInstance::one(&cat, id)], 10);
    assert_eq!(lot.unwrap().count, 2);
    assert!(kept.is_empty());
}
