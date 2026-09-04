//! **Installing something is a move, not a copy.**
//!
//! The state these gates exist to make impossible is the same alternator
//! sitting in a stockroom and fitted to a van at once. Everything else
//! follows from that: a removed part is the part that went in, a destroyed
//! mount takes its occupant with it, and a wall gives back a door only if
//! somebody took it down carefully.

use scale_sim::craft::{
    hand_tools, machine_shop, standard_recipes, Halt, Maker, WorkOrder, Workplace,
};
use scale_sim::fitted::{serviceable_parts, FittedVehicle, WallAssembly, WontFit};
use scale_sim::item::{
    standard_catalogue, Catalogue, Host, ItemInstance, ItemLot, JointMethod, Placement, Store,
};
use scale_sim::material::Material;
use scale_sim::teardown::{Recovered, Teardown};
use scale_sim::vehicle::{Part, Vehicle};

fn a_van(cat: &Catalogue, store: &mut Store) -> (FittedVehicle, Vec<scale_sim::id::Id<ItemInstance>>) {
    FittedVehicle::adapt(1, Vehicle::van(), &serviceable_parts(), cat, store)
}

fn a_good_hand() -> Maker {
    Maker { skill: 0.85, proficiency: 0.8, knows_recipe: true, tool_familiarity: 0.9,
            focus: 0.9, fatigue: 0.1 }
}

// =====================================================================
// placement: one item, one place
// =====================================================================

/// **Gate: an item is in exactly one place, and fitting it moves it.**
///
/// It was in a crate; now it is in the van; and the crate no longer lists
/// it. A model with a `location` *and* an `installed_in` admits the state
/// where both are set, and that state is the bug.
#[test]
fn fitting_a_part_takes_it_out_of_the_crate() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (mut van, _) = a_van(&cat, &mut store);

    let crate_ = store.add(ItemInstance::one(&cat, cat.must("washing machine")));
    let alt = store.add(ItemInstance::one(&cat, cat.must("alternator")));
    store.place(alt, Placement::Loose { locality: 3, x: 4, y: 5 });

    // Onto the host item first, to prove the same rule holds there.
    store.install(crate_, alt, &cat).expect("the bracket would not take it");
    assert!(matches!(store.placement(alt), Placement::Installed { .. }));
    assert!(store.get(crate_).unwrap().attachments.iter().any(|a| a.1 == alt));

    // And now into the van. It must leave the washing machine as it goes.
    let bracket = van.mount_named("alternator bracket").unwrap();
    // Fitted to something else, it is not available.
    assert!(matches!(
        van.install(&mut store, bracket, alt, &cat, &[], None, 1),
        Err(WontFit::NotAvailable(_))
    ));

    store.place(alt, Placement::Loose { locality: 0, x: 0, y: 0 });
    assert!(
        !store.get(crate_).unwrap().attachments.iter().any(|a| a.1 == alt),
        "it was still bolted to the washing machine after being taken off"
    );
    van.install(&mut store, bracket, alt, &cat, &[], None, 1).unwrap();
    assert_eq!(
        store.placement(alt),
        Placement::Installed { host: Host::Vehicle(1), mount: bracket }
    );
}

/// **Gate: nothing is fitted twice.**
#[test]
fn one_alternator_cannot_be_in_two_vans() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (mut a, loose) = a_van(&cat, &mut store);
    let (mut b, _) = FittedVehicle::adapt(2, Vehicle::van(), &serviceable_parts(), &cat, &mut store);

    let alt = *loose
        .iter()
        .find(|&&i| store.get(i).unwrap().definition == cat.must("alternator"))
        .unwrap();
    let m1 = a.mount_named("alternator bracket").unwrap();
    let m2 = b.mount_named("alternator bracket").unwrap();

    a.install(&mut store, m1, alt, &cat, &[], None, 1).unwrap();
    assert!(matches!(
        b.install(&mut store, m2, alt, &cat, &[], None, 1),
        Err(WontFit::NotAvailable(_))
    ));
    assert!(!b.mounts[m2].occupied(), "the second van thought it had one");
}

/// **Gate: a mount takes one thing.**
#[test]
fn a_mount_that_is_full_is_full() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (mut van, loose) = a_van(&cat, &mut store);
    let alt = loose[0];
    let spare = store.add(ItemInstance::one(&cat, cat.must("alternator")));
    store.place(spare, Placement::Loose { locality: 0, x: 0, y: 0 });

    let bracket = van.mount_named("alternator bracket").unwrap();
    van.install(&mut store, bracket, alt, &cat, &[], None, 1).unwrap();
    assert_eq!(
        van.install(&mut store, bracket, spare, &cat, &[], None, 1),
        Err(WontFit::Occupied)
    );
}

/// **Gate: what is committed elsewhere cannot be fitted.** A part reserved
/// by a work order or folded into a lot is not on the shelf, and the
/// refusal says which.
#[test]
fn something_spoken_for_is_not_available() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (mut van, _) = a_van(&cat, &mut store);
    let bracket = van.mount_named("alternator bracket").unwrap();

    for (state, expect) in [
        (Placement::InWorkOrder { order: 9 }, "committed"),
        (Placement::Aggregated { lot: 4 }, "lot"),
        (Placement::Nowhere, "nowhere"),
    ] {
        let alt = store.add(ItemInstance::one(&cat, cat.must("alternator")));
        store.place(alt, state);
        match van.install(&mut store, bracket, alt, &cat, &[], None, 1) {
            Err(WontFit::NotAvailable(why)) => {
                assert!(why.contains(expect), "the refusal said {why:?} for {state:?}")
            }
            other => panic!("{state:?} was accepted: {other:?}"),
        }
    }
}

/// **Gate: a fitting is a shape.** A loaf does not go on an alternator
/// bracket however much anybody wants it to.
#[test]
fn what_fits_is_decided_by_the_fitting() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (mut van, _) = a_van(&cat, &mut store);
    let bracket = van.mount_named("alternator bracket").unwrap();
    let loaf = store.add(ItemInstance::one(&cat, cat.must("loaf")));
    store.place(loaf, Placement::Loose { locality: 0, x: 0, y: 0 });
    assert_eq!(
        van.install(&mut store, bracket, loaf, &cat, &[], None, 1),
        Err(WontFit::DoesNotFit)
    );
}

/// **Gate: taking it off puts it back on the ground.**
#[test]
fn uninstalling_leaves_it_loose_and_not_nowhere() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (mut van, loose) = a_van(&cat, &mut store);
    let alt = loose[0];
    let bracket = van.mount_named("alternator bracket").unwrap();

    van.install(&mut store, bracket, alt, &cat, &[], None, 1).unwrap();
    let back = van.uninstall(&mut store, bracket).unwrap();
    assert_eq!(back, alt);
    assert!(matches!(store.placement(alt), Placement::Loose { .. }));
    assert!(!van.mounts[bracket].occupied());
    assert_eq!(van.uninstall(&mut store, bracket), Err(WontFit::Empty));
}

/// **Gate: destroying the mount destroys what was in it.**
///
/// It must not fall out loose, and it must certainly not go on existing in
/// two places. A crash that duplicated components would be a money printer
/// of the exact shape this project keeps finding.
#[test]
fn a_wrecked_mount_takes_its_occupant_with_it() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (mut van, loose) = a_van(&cat, &mut store);
    let alt = loose[0];
    let bracket = van.mount_named("alternator bracket").unwrap();
    van.install(&mut store, bracket, alt, &cat, &[], None, 1).unwrap();

    let before = store.items.len();
    van.destroy_mount(&mut store, bracket);

    assert!(!van.mounts[bracket].occupied());
    assert!(store.get(alt).is_none(), "the alternator survived the crash intact");
    assert_eq!(store.items.len(), before - 1, "something was duplicated or leaked");
    assert!(van.installations.iter().all(|i| i.item != alt));
}

/// **Gate: a fitted thing is not ordinary stock.** It has a history and a
/// place, and folding it into a lot would destroy both.
#[test]
fn a_fitted_part_is_not_folded_into_a_lot() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (mut van, loose) = a_van(&cat, &mut store);
    let alt = loose[0];
    let bracket = van.mount_named("alternator bracket").unwrap();
    van.install(&mut store, bracket, alt, &cat, &[], None, 1).unwrap();

    let fitted = store.get(alt).unwrap().clone();
    assert!(!fitted.aggregatable(), "an alternator in a van was folded into a lot");
    let (lot, kept) = ItemLot::aggregate(cat.must("alternator"), vec![fitted], 10);
    assert!(lot.is_none());
    assert_eq!(kept.len(), 1);
}

// =====================================================================
// the vehicle adapter
// =====================================================================

/// **Gate: a removed part is the part that was installed.**
#[test]
fn the_alternator_that_comes_off_is_the_one_that_went_on() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (mut van, loose) = a_van(&cat, &mut store);
    let alt = loose[0];
    {
        let a = store.get_mut(alt).unwrap();
        a.condition.wear = 0.44;
        a.quality.workmanship = 0.91;
        a.given_name = Some("the rebuilt one".into());
        a.faults.push(scale_sim::item::Fault {
            what: "noisy bearing", severity: 0.3, disabling: false, since_day: 40,
        });
    }
    let bracket = van.mount_named("alternator bracket").unwrap();
    van.install(&mut store, bracket, alt, &cat, &[(Material::MildSteel, 0.04)], Some(7), 12)
        .unwrap();

    // A year of running.
    store.get_mut(alt).unwrap().condition.wear = 0.61;

    let back = van.uninstall(&mut store, bracket).unwrap();
    assert_eq!(back, alt, "a different alternator came off");
    let a = store.get(back).unwrap();
    assert!((a.condition.wear - 0.61).abs() < 1e-9, "its hours were forgotten");
    assert_eq!(a.quality.workmanship, 0.91);
    assert_eq!(a.given_name.as_deref(), Some("the rebuilt one"));
    assert_eq!(a.faults.len(), 1, "the noisy bearing was mended by taking it out");
}

/// **Gate: the fasteners are sacrificed and the component is not.**
///
/// What went into the joint is a separate line from what was installed,
/// which is exactly what lets the door come off and the mastic not.
#[test]
fn what_was_used_installing_it_is_not_part_of_it() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (mut van, loose) = a_van(&cat, &mut store);
    let alt = loose[0];
    let bracket = van.mount_named("alternator bracket").unwrap();
    van.install(&mut store, bracket, alt, &cat, &[(Material::MildSteel, 0.08)], Some(3), 5)
        .unwrap();

    let record = van.installations.iter().find(|i| i.item == alt).unwrap();
    assert_eq!(record.joint, JointMethod::Bolted);
    assert_eq!(record.installer, Some(3));
    assert_eq!(record.installed_on_day, 5);
    assert!((record.consumed.iter().map(|c| c.1).sum::<f64>() - 0.08).abs() < 1e-9);

    // And the alternator itself is unchanged by having been bolted on.
    assert!((store.get(alt).unwrap().mass_kg - 5.5).abs() < 1e-9);
}

/// **Gate: an installed component's mass appears exactly once.**
#[test]
fn the_van_gets_heavier_by_exactly_the_alternator() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (mut van, loose) = a_van(&cat, &mut store);
    let alt = loose[0];
    let its_mass = store.get(alt).unwrap().mass_kg;

    let empty = van.kerb_kg(&store);
    let bracket = van.mount_named("alternator bracket").unwrap();
    van.install(&mut store, bracket, alt, &cat, &[], None, 1).unwrap();
    let fitted = van.kerb_kg(&store);

    assert!(
        (fitted - empty - its_mass).abs() < 1e-9,
        "fitting a {its_mass:.1} kg alternator moved the kerb weight by {:.1} kg",
        fitted - empty
    );

    // And taking it off gives the mass back exactly.
    van.uninstall(&mut store, bracket).unwrap();
    assert!((van.kerb_kg(&store) - empty).abs() < 1e-9);
}

/// **Gate: fitting a part gives the machine a function it did not have.**
///
/// The physics reads the assembled vehicle, so an alternator on a bracket
/// really does mean the van can charge a battery — and taking it off means
/// it cannot.
#[test]
fn a_van_with_no_alternator_generates_nothing() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (mut van, loose) = a_van(&cat, &mut store);

    assert_eq!(
        van.assembled().generation_kwh_per_day(),
        0.0,
        "a van with its alternator on the bench was still charging"
    );

    let alt = *loose
        .iter()
        .find(|&&i| store.get(i).unwrap().definition == cat.must("alternator"))
        .unwrap();
    let bracket = van.mount_named("alternator bracket").unwrap();
    van.install(&mut store, bracket, alt, &cat, &[], None, 1).unwrap();
    assert!(van.assembled().generation_kwh_per_day() > 5.0);
}

/// **Gate: a van on two wheels does not go.** Structure and function are
/// read off what is fitted, not off the name of the vehicle.
#[test]
fn a_van_with_its_wheels_off_is_not_drivable() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (mut van, loose) = a_van(&cat, &mut store);
    assert!(!van.assembled().drivable(), "it drove with every wheel in the store");

    let wheels: Vec<_> = loose
        .iter()
        .copied()
        .filter(|&i| store.get(i).unwrap().definition == cat.must("road wheel"))
        .collect();
    assert!(wheels.len() >= 4, "a van came apart into {} wheels", wheels.len());

    let hubs: Vec<usize> = van
        .mounts
        .iter()
        .enumerate()
        .filter(|(_, m)| m.name == "hub")
        .map(|(i, _)| i)
        .collect();
    for (hub, wheel) in hubs.iter().zip(&wheels) {
        van.install(&mut store, *hub, *wheel, &cat, &[], None, 1).unwrap();
    }
    assert!(van.assembled().drivable(), "a van with four wheels on would not move");

    // Take two off and it is on the ramp again.
    van.uninstall(&mut store, hubs[0]).unwrap();
    van.uninstall(&mut store, hubs[1]).unwrap();
    van.uninstall(&mut store, hubs[2]).unwrap();
    assert!(!van.assembled().drivable());
    // And the wheels are objects on the floor, not gone.
    assert!(matches!(store.placement(wheels[0]), Placement::Loose { .. }));
}

// =====================================================================
// the building adapter
// =====================================================================

fn a_wall(cat: &Catalogue, store: &mut Store) -> (WallAssembly, scale_sim::id::Id<ItemInstance>) {
    let mut wall = WallAssembly::timber_framed(10, cat);
    let door = store.add(ItemInstance::one(cat, cat.must("door")));
    store.place(door, Placement::Loose { locality: 0, x: 0, y: 0 });
    let opening = wall.fixture_named("doorway").unwrap();
    wall.install(store, opening, door, cat, &[(Material::MildSteel, 0.15)], 1).unwrap();
    (wall, door)
}

fn back(r: &Recovered, cat: &Catalogue, name: &str) -> u32 {
    let want = cat.must(name);
    r.components.iter().filter(|c| c.definition == want).map(|c| c.count).sum()
}

/// **Gate: careful deconstruction returns the door; demolition does not.**
///
/// The same wall, the same person, two intentions — and the difference is
/// not a special case for doors, it falls out of the joint table and how
/// much care was taken.
#[test]
fn a_deconstructed_wall_gives_back_its_door() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (wall, _) = a_wall(&cat, &mut store);

    let (mut careful_doors, mut smashed_doors) = (0, 0);
    for event in 0..40u64 {
        let careful = wall.take_down(Teardown::Deconstruct, &cat, &store, 0.8, event);
        let smashed = wall.take_down(Teardown::Smash, &cat, &store, 0.8, event);
        careful_doors += back(&careful, &cat, "door");
        smashed_doors += back(&smashed, &cat, "door");
    }
    assert!(careful_doors > 25, "careful work saved the door {careful_doors} times in 40");
    assert_eq!(smashed_doors, 0, "a sledgehammer through a wall produced an intact door");
}

/// **Gate: mortar and adhesive are joint mass and never come back.**
#[test]
fn what_was_set_into_the_joint_stays_there() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (wall, _) = a_wall(&cat, &mut store);

    let careful = wall.take_down(Teardown::Deconstruct, &cat, &store, 0.95, 3);
    assert_eq!(careful.material(Material::Adhesive), 0.0, "the mastic came back");
    assert_eq!(careful.fuel_of(Material::Adhesive), 0.0);

    // Nor does the jointing compound come back as plasterboard, which is
    // what a model that treated it as another layer would say.
    let bricks = WallAssembly::brick(11, &cat);
    let down = bricks.take_down(Teardown::Deconstruct, &cat, &store, 0.9, 3);
    assert_eq!(down.material(Material::Mortar), 0.0, "the mortar was recovered as mortar");
}

/// **Gate: a brick wall in mortar is worth far less taken down than a
/// screwed timber frame**, however careful anybody is. That is what the
/// joint table is for.
#[test]
fn mortar_is_why_nobody_saves_bricks() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (timber, _) = a_wall(&cat, &mut store);
    let bricks = WallAssembly::brick(11, &cat);

    let (mut studs, mut saved_bricks) = (0.0f64, 0.0f64);
    for event in 0..25u64 {
        let a = timber.take_down(Teardown::Deconstruct, &cat, &store, 0.85, event);
        let b = bricks.take_down(Teardown::Deconstruct, &cat, &store, 0.85, event);
        studs += back(&a, &cat, "stud") as f64 / 7.0;
        saved_bricks += back(&b, &cat, "brick") as f64 / 430.0;
    }
    let (studs, saved_bricks) = (studs / 25.0, saved_bricks / 25.0);
    assert!(studs > 0.5, "a screwed timber frame gave back {:.0}% of its studs", studs * 100.0);
    assert!(
        saved_bricks < studs * 0.8,
        "mortared brick came back as readily as screwed timber: {:.0}% against {:.0}%",
        saved_bricks * 100.0,
        studs * 100.0
    );
}

/// **Gate: a fixture taken out of a wall is the same fixture.**
#[test]
fn the_door_that_comes_out_is_the_door_that_went_in() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (mut wall, door) = a_wall(&cat, &mut store);
    store.get_mut(door).unwrap().condition.wear = 0.35;
    store.get_mut(door).unwrap().given_name = Some("the one off the old house".into());

    assert!(matches!(store.placement(door), Placement::Installed { host: Host::Building(10), .. }));
    let opening = wall.fixture_named("doorway").unwrap();
    let back = wall.uninstall(&mut store, opening).unwrap();
    assert_eq!(back, door);
    let d = store.get(back).unwrap();
    assert!((d.condition.wear - 0.35).abs() < 1e-9);
    assert_eq!(d.given_name.as_deref(), Some("the one off the old house"));
    assert!(matches!(d.placement, Placement::Loose { .. }));
}

/// **Gate: a wall weighs its courses plus its joints plus its fixtures,
/// and taking it down accounts for every kilogram of that.**
#[test]
fn a_wall_is_the_sum_of_what_is_in_it() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (wall, door) = a_wall(&cat, &mut store);

    let mass = wall.mass_kg(&cat, &store);
    // A 2.4 x 3.0 m stud wall with plasterboard runs about 25 kg/m2.
    assert!((150.0..=230.0).contains(&mass), "the wall came to {mass:.0} kg");
    assert!(mass > store.get(door).unwrap().mass_kg);

    let as_one = wall.as_one_object(&cat, &store);
    assert!((as_one.mass_kg - mass).abs() < 1e-9);

    for how in [Teardown::Deconstruct, Teardown::Salvage, Teardown::Recycle, Teardown::Smash] {
        let r = wall.take_down(how, &cat, &store, 0.8, 1);
        assert!(
            (r.accounted_kg() - mass).abs() < 1e-6,
            "{}: {:.3} kg accounted for out of {mass:.3}",
            how.name(),
            r.accounted_kg()
        );
    }
}

// =====================================================================
// and the blackout, which reaches for the other tool
// =====================================================================

/// **Gate: losing power disables the powered provider, not the step.**
///
/// The operation still has to happen; what changes is which tool does it.
/// A shop with a bandsaw *and* a handsaw carries on, more slowly.
#[test]
fn the_shop_reaches_for_the_handsaw() {
    let cat = standard_catalogue();
    let book = standard_recipes(&cat);
    let plan = book.must("chair, hand tools");

    // A shop that has both, which is what a real workshop is.
    let mut everything = machine_shop(&cat);
    everything.extend(hand_tools(&cat));
    let lit = Workplace::a_factory(everything.clone(), 2.0, 1);
    let mut dark = lit.clone();
    dark.power = false;

    let mut with = WorkOrder::begin(5, plan, 1, 0, 1);
    let mut without = WorkOrder::begin(5, plan, 1, 0, 1);
    for _ in 0..400 {
        if !with.finished() {
            with.advance(60.0, &book, &cat, &lit, a_good_hand());
        }
        if !without.finished() {
            without.advance(60.0, &book, &cat, &dark, a_good_hand());
        }
    }

    assert_eq!(without.state, Halt::Done, "the shop stopped rather than picking up a saw");
    assert_eq!(with.state, Halt::Done);
    assert!(without.power_kwh == 0.0, "a handsaw drew current");
    assert!(with.power_kwh > 0.0);
    assert!(
        without.active_labour_min > with.active_labour_min,
        "the handsaw was as quick as the bandsaw"
    );
    // The same chair, out of the same board.
    let a = with.deliver(&book, &cat, 1).unwrap();
    let b = without.deliver(&book, &cat, 1).unwrap();
    assert!((a.mass_kg - b.mass_kg).abs() < 1e-6);
}

/// **Gate: work already finished is not recalculated under the
/// replacement tool.**
///
/// The power goes off half way through. What was cut is cut; the steps
/// already recorded keep the times and the outcomes they had.
#[test]
fn the_power_goes_off_and_yesterday_does_not_change() {
    let cat = standard_catalogue();
    let book = standard_recipes(&cat);
    let plan = book.must("chair, hand tools");
    let mut everything = machine_shop(&cat);
    everything.extend(hand_tools(&cat));
    let lit = Workplace::a_factory(everything.clone(), 2.0, 1);
    let mut dark = lit.clone();
    dark.power = false;

    let mut o = WorkOrder::begin(6, plan, 1, 0, 1);
    while o.completed.len() < 3 && !o.finished() {
        o.advance(20.0, &book, &cat, &lit, a_good_hand());
    }
    let done_so_far = o.completed.clone();
    let elapsed = o.elapsed_min;
    let drawn = o.power_kwh;
    assert!(!done_so_far.is_empty() && drawn > 0.0);

    // The lights go out.
    for _ in 0..400 {
        if o.finished() {
            break;
        }
        o.advance(60.0, &book, &cat, &dark, a_good_hand());
    }
    assert_eq!(o.state, Halt::Done);

    assert_eq!(
        &o.completed[..done_so_far.len()],
        &done_so_far[..],
        "the steps already finished were re-timed under the hand tools"
    );
    assert!(o.elapsed_min > elapsed);
    assert!(
        (o.power_kwh - drawn).abs() < 1e-9,
        "the shop went on drawing current after the supply failed"
    );
}

/// The adapter says what it is: some parts of a vehicle become objects and
/// the rest stays structure. Nothing is duplicated by the split.
#[test]
fn adapting_a_vehicle_moves_parts_out_rather_than_copying_them() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let whole = Vehicle::van();
    let (van, loose) = a_van(&cat, &mut store);

    let alternators_before = whole.kinds().filter(|p| matches!(p, Part::Alternator(_))).count();
    let alternators_after =
        van.base.parts.iter().filter(|(p, _, _)| matches!(p, Part::Alternator(_))).count();
    assert_eq!(alternators_before, 1);
    assert_eq!(alternators_after, 0, "the van kept an alternator it had handed over");
    assert_eq!(van.mounts.iter().filter(|m| m.name == "alternator bracket").count(), 1);

    // Every part that came out is now an object, and none of them is
    // anywhere but on the floor.
    assert_eq!(loose.len(), van.mounts.len());
    for &i in &loose {
        assert!(matches!(store.placement(i), Placement::Loose { .. }));
    }
    // And putting them all back gives the vehicle it started as.
    let mut van = van;
    let mut order = loose.clone();
    for m in 0..van.mounts.len() {
        let takes = van.mounts[m].takes;
        let at = order
            .iter()
            .position(|&i| cat.get(store.get(i).unwrap().definition).unwrap().fits == Some(takes))
            .expect("nothing came out that fits this mount");
        let item = order.remove(at);
        van.install(&mut store, m, item, &cat, &[], None, 1).unwrap();
    }
    let rebuilt = van.assembled();
    assert_eq!(rebuilt.parts.len(), whole.parts.len(), "the van did not go back together");
    assert!((rebuilt.kerb_t() - whole.kerb_t()).abs() < 1e-9);
}
