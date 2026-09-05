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
use scale_sim::fitted::{
    serviceable_parts, FittedVehicle, InstallationFailure, Stage, WallAssembly, WontFit,
};
use scale_sim::item::{
    standard_catalogue, Catalogue, Host, ItemInstance, ItemLot, JointMethod, Placement, Store,
    WorkStatus,
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

    let crate_ = store.add(ItemInstance::one(&cat, cat.must("washing machine")), Placement::anywhere());
    let alt = store.add(ItemInstance::one(&cat, cat.must("alternator")), Placement::anywhere());
    store.place(alt, Placement::Ground { locality: 3, x: 4, y: 5 });

    // Onto the host item first, to prove the same rule holds there.
    store.install(crate_, alt, &cat).expect("the bracket would not take it");
    assert!(matches!(store.placement(alt), Some(Placement::Installed { .. })));
    assert!(store.get(crate_).unwrap().attachments.iter().any(|a| a.1 == alt));

    // And now into the van. It must leave the washing machine as it goes.
    let bracket = van.mount_named("alternator bracket").unwrap();
    // Fitted to something else, it is not available.
    assert!(matches!(
        van.install(&mut store, bracket, alt, &cat, &[], None, 1),
        Err(WontFit::NotAvailable(_))
    ));

    store.place(alt, Placement::Ground { locality: 0, x: 0, y: 0 });
    assert!(
        !store.get(crate_).unwrap().attachments.iter().any(|a| a.1 == alt),
        "it was still bolted to the washing machine after being taken off"
    );
    van.install(&mut store, bracket, alt, &cat, &[], None, 1).unwrap();
    assert_eq!(
        store.placement(alt),
        Some(Placement::Installed { host: Host::Vehicle(1), mount: bracket })
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
    let spare = store.add(ItemInstance::one(&cat, cat.must("alternator")), Placement::anywhere());
    store.place(spare, Placement::Ground { locality: 0, x: 0, y: 0 });

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

    // **Where it is and what it is spoken for are two different
    // refusals.** A board reserved for a job has not moved off its rack;
    // a panel clamped in a press has not been claimed by anybody. Both
    // stop you fitting it, for different reasons.
    for (state, expect) in [
        (Placement::Installed { host: Host::Vehicle(77), mount: 0 }, "fitted"),
        (Placement::Fixtured { resource: 3, slot: 1, clamped: true }, "clamped"),
    ] {
        let alt = store.add(ItemInstance::one(&cat, cat.must("alternator")), Placement::anywhere());
        store.place(alt, state);
        match van.install(&mut store, bracket, alt, &cat, &[], None, 1) {
            Err(WontFit::NotAvailable(why)) => {
                assert!(why.contains(expect), "the refusal said {why:?} for {state:?}")
            }
            other => panic!("{state:?} was accepted: {other:?}"),
        }
    }
    for (status, expect) in [
        (WorkStatus::Reserved { order: 9 }, "committed"),
        (WorkStatus::Wip { order: 9, operation: 2 }, "being worked on"),
        (WorkStatus::AwaitingUnload { order: 9 }, "not yet collected"),
    ] {
        let alt = store.add(ItemInstance::one(&cat, cat.must("alternator")), Placement::anywhere());
        store.set_status(alt, status);
        // **It has not moved.** A reserved board is still on its rack.
        assert!(matches!(store.placement(alt), Some(Placement::Ground { .. })));
        match van.install(&mut store, bracket, alt, &cat, &[], None, 1) {
            Err(WontFit::NotAvailable(why)) => {
                assert!(why.contains(expect), "the refusal said {why:?} for {status:?}")
            }
            other => panic!("{status:?} was accepted: {other:?}"),
        }
    }
    // And a panel sitting in an output tray is reachable: somebody can
    // pick it up and walk off with it.
    let tray = store.add(ItemInstance::one(&cat, cat.must("alternator")), Placement::anywhere());
    store.place(tray, Placement::Fixtured { resource: 3, slot: 0, clamped: false });
    assert!(store.available(tray), "a cool part in a tray could not be picked up");

    // **And a thing that has ended is not somewhere else — it is not
    // anywhere, because it is not an item any more.** That is a different
    // question from where a live item is, and it used to be the same
    // field.
    let doomed = store.add(ItemInstance::one(&cat, cat.must("alternator")), Placement::anywhere());
    store.end(doomed, scale_sim::item::ItemEnd::Destroyed, 4);
    assert_eq!(store.placement(doomed), None);
    assert!(matches!(
        van.install(&mut store, bracket, doomed, &cat, &[], None, 1),
        Err(WontFit::NotAvailable(_))
    ));
}

/// **Gate: a fitting is a shape.** A loaf does not go on an alternator
/// bracket however much anybody wants it to.
#[test]
fn what_fits_is_decided_by_the_fitting() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (mut van, _) = a_van(&cat, &mut store);
    let bracket = van.mount_named("alternator bracket").unwrap();
    let loaf = store.add(ItemInstance::one(&cat, cat.must("loaf")), Placement::anywhere());
    store.place(loaf, Placement::Ground { locality: 0, x: 0, y: 0 });
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
    assert!(matches!(store.placement(alt), Some(Placement::Ground { .. })));
    assert!(!van.mounts[bracket].occupied());
    assert_eq!(van.uninstall(&mut store, bracket), Err(WontFit::Empty));
}

/// **Gate: a wrecked mount settles its occupant, it does not annihilate
/// it.**
///
/// The old rule — destroying the mount destroys what was in it — stopped
/// identity leaking and bought universal annihilation instead. A component
/// in a crash may stay bolted to the wreck, come off intact, come off
/// bent, be jammed where nobody can reach it, or be broken up. What must
/// hold is that **exactly one of those happens and nothing is
/// duplicated**.
#[test]
fn a_wrecked_mount_settles_its_occupant_one_way_or_another() {
    let cat = standard_catalogue();
    let mut seen: Vec<&str> = Vec::new();

    for event in 0..200u64 {
        for severity in [0.2, 0.5, 0.9] {
            let mut store = Store::new();
            let (mut van, loose) = a_van(&cat, &mut store);
            let alt = loose[0];
            let bracket = van.mount_named("alternator bracket").unwrap();
            van.install(&mut store, bracket, alt, &cat, &[], None, 1).unwrap();
            let before = store.live();

            let out = van
                .wreck_mount(&mut store, bracket, severity, &cat, event, 9)
                .expect("nothing was in the mount");
            let label = match &out {
                InstallationFailure::RemainsAttached { .. } => "attached",
                InstallationFailure::Detached { .. } => "detached",
                InstallationFailure::Inaccessible => "jammed",
                InstallationFailure::ContentsReleased => "spilt",
                InstallationFailure::Destroyed { .. } => "destroyed",
            };
            if !seen.contains(&label) {
                seen.push(label);
            }

            // Exactly one outcome, and the object is in exactly one state.
            match &out {
                InstallationFailure::Destroyed { .. } => {
                    assert!(store.get(alt).is_none(), "a destroyed part was still an object");
                    assert_eq!(store.live(), before - 1, "something leaked");
                    assert_eq!(store.graves().len(), 1, "no tombstone was left");
                    assert!(!van.mounts[bracket].occupied());
                }
                InstallationFailure::Detached { .. } => {
                    assert!(matches!(store.placement(alt), Some(Placement::Ground { .. })));
                    assert!(!van.mounts[bracket].occupied());
                    assert_eq!(store.live(), before, "detaching it duplicated it");
                }
                InstallationFailure::RemainsAttached { .. }
                | InstallationFailure::Inaccessible
                | InstallationFailure::ContentsReleased => {
                    assert!(van.mounts[bracket].occupied(), "it fell out of a mount it is in");
                    assert!(matches!(
                        store.placement(alt),
                        Some(Placement::Installed { .. })
                    ));
                    assert_eq!(store.live(), before);
                }
            }
            // Never in two places.
            let fitted = van.mounts.iter().filter(|m| m.occupant == Some(alt)).count();
            let on_ground =
                matches!(store.placement(alt), Some(Placement::Ground { .. })) as usize;
            assert!(fitted + on_ground <= 1, "the alternator was in two places");
        }
    }

    // **All of them are reachable.** A settlement with one outcome is the
    // old rule with more words.
    assert!(seen.len() >= 3, "only {} distinct outcomes ever happened", seen.len());
}

/// **A hard knock is worse than a light one**, and a weak joint lets go
/// where a welded one holds. Measured over the population, because one
/// crash is one draw.
#[test]
fn a_harder_impact_costs_more() {
    let cat = standard_catalogue();
    let count = |severity: f64| {
        let (mut destroyed, mut attached) = (0, 0);
        for event in 0..300u64 {
            let mut store = Store::new();
            let (mut van, loose) = a_van(&cat, &mut store);
            let bracket = van.mount_named("alternator bracket").unwrap();
            van.install(&mut store, bracket, loose[0], &cat, &[], None, 1).unwrap();
            match van.wreck_mount(&mut store, bracket, severity, &cat, event, 1) {
                Some(InstallationFailure::Destroyed { .. }) => destroyed += 1,
                Some(InstallationFailure::RemainsAttached { .. }) => attached += 1,
                _ => {}
            }
        }
        (destroyed, attached)
    };
    let (gentle_d, gentle_a) = count(0.1);
    let (hard_d, hard_a) = count(0.95);
    assert!(hard_d > gentle_d, "a heavy impact broke no more than a nudge");
    assert!(gentle_a > hard_a, "a nudge shook as much loose as a heavy impact");
    assert!(
        gentle_d * 3 < hard_d.max(1),
        "severity barely mattered: {gentle_d} destroyed at a nudge against {hard_d} at a crash"
    );
    // And a nudge is overwhelmingly "still bolted on", which is what
    // makes it a nudge.
    assert!(gentle_a > 200, "only {gentle_a} of 300 survived a light knock in place");
}

/// **Structural breakup moves what survives.** A hole in the floor is not
/// a total loss: what was bolted to a section that is still standing is
/// still bolted to it.
#[test]
fn breaking_a_lorry_in_half_does_not_destroy_the_far_end() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (mut van, loose) = a_van(&cat, &mut store);
    for (m, &item) in loose.iter().enumerate() {
        let takes = van.mounts[m].takes;
        if cat.get(store.get(item).unwrap().definition).unwrap().fits == Some(takes) {
            van.install(&mut store, m, item, &cat, &[], None, 1).unwrap();
        }
    }
    let fitted_before = van.fitted().count();
    assert!(fitted_before >= 4);

    // The nose is torn off: everything at x = 0.
    let lost: Vec<(i32, i32)> =
        van.mounts.iter().map(|m| m.at).filter(|&(x, _)| x == 0).collect();
    let settled = van.break_up(&mut store, &lost, 0.8, &cat, 4242, 12);

    assert!(!settled.is_empty(), "tearing the nose off settled nothing");
    // What was not on a lost tile is untouched.
    for (i, m) in van.mounts.iter().enumerate() {
        if !lost.contains(&m.at) {
            assert!(
                !settled.iter().any(|(k, _)| *k == i),
                "a mount at {:?} was settled and the tile is still there",
                m.at
            );
        }
    }
    assert!(van.fitted().count() > 0, "the whole van was written off by losing its nose");
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

    // **Where it is, is the store's question.** The instance on its own
    // cannot know it is bolted into a van, which is exactly why the check
    // lives where the placement does.
    assert!(!store.aggregatable(alt), "an alternator in a van was folded into a lot");
    let spare = store.add_loose(ItemInstance::one(&cat, cat.must("alternator")));
    assert!(store.aggregatable(spare), "an alternator on a shelf refused to be counted");

    let fitted = store.get(alt).unwrap().clone();
    let (lot, _) = ItemLot::aggregate(cat.must("alternator"), vec![fitted], 10);
    assert!(lot.is_some(), "the instance alone has no reason to refuse");
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
    assert!(matches!(store.placement(wheels[0]), Some(Placement::Ground { .. })));
}

// =====================================================================
// the building adapter
// =====================================================================

fn a_wall(cat: &Catalogue, store: &mut Store) -> (WallAssembly, scale_sim::id::Id<ItemInstance>) {
    let mut wall = WallAssembly::timber_framed(10, cat);
    let door = store.add(ItemInstance::one(cat, cat.must("door")), Placement::anywhere());
    store.place(door, Placement::Ground { locality: 0, x: 0, y: 0 });
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
    let bricks = WallAssembly::brick(11, &cat, JointMethod::CementMortared);
    let down = bricks.take_down(Teardown::Deconstruct, &cat, &store, 0.9, 3);
    assert_eq!(down.material(Material::Mortar), 0.0, "the mortar was recovered as mortar");
}

/// **Gate: which mortar it was built in decides whether the bricks come
/// back.**
///
/// Not "mortar is why nobody saves bricks" — that is too broad. Lime is
/// softer than the brick, so the joint gives way and the brick survives;
/// reclamation yards are full of lime-mortared stock. Cement is harder
/// than the brick, so the brick gives way, and the reclamation literature
/// names cement-based mortar as *the* barrier — a slow problem with known
/// techniques rather than an impossibility.
#[test]
fn the_bond_decides_whether_the_bricks_come_back() {
    let cat = standard_catalogue();
    let store = Store::new();
    let lime = WallAssembly::brick(11, &cat, JointMethod::LimeMortared);
    let cement = WallAssembly::brick(12, &cat, JointMethod::CementMortared);

    let share = |w: &WallAssembly| {
        let mut got = 0.0f64;
        for event in 0..25u64 {
            let r = w.take_down(Teardown::Deconstruct, &cat, &store, 0.85, event);
            got += back(&r, &cat, "brick") as f64 / 430.0;
        }
        got / 25.0
    };
    let (lime_back, cement_back) = (share(&lime), share(&cement));

    assert!(
        lime_back > 0.5,
        "lime-mortared brick came back at only {:.0}%",
        lime_back * 100.0
    );
    assert!(
        cement_back < lime_back * 0.6,
        "cement made no difference: {:.0}% against {:.0}%",
        cement_back * 100.0,
        lime_back * 100.0
    );
    // And it is a barrier, not a wall of its own: some come back.
    assert!(cement_back > 0.0, "cement mortar made recovery flatly impossible");
}

/// **Gate: taking a wall down is a sequence, not a verb.**
///
/// Isolate, strip the fittings, take the door out, strip the finishes,
/// expose the frame, separate it, sort it. Each stage takes out of the
/// wall what that stage is *for*.
#[test]
fn deconstruction_is_a_plan_and_it_has_an_order() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (mut wall, door) = a_wall(&cat, &mut store);

    let plan = WallAssembly::deconstruction_plan();
    assert_eq!(plan[0], Stage::IsolateUtilities, "somebody started before killing the power");
    assert!(
        plan.iter().position(|s| *s == Stage::RemoveOpenings)
            < plan.iter().position(|s| *s == Stage::SeparateStructure),
        "the frame came down before the door came out"
    );

    let boards = cat.must("plasterboard sheet");
    assert!(wall.courses.iter().any(|c| c.def == boards));

    for stage in plan {
        let out = wall.perform(*stage, &mut store, &cat);
        if *stage == Stage::RemoveOpenings {
            assert_eq!(out, vec![door], "taking the openings out did not produce the door");
        }
        assert!(wall.done(*stage));
    }
    // The finish is off the wall and the door is on the floor.
    assert!(!wall.courses.iter().any(|c| c.def == boards));
    assert!(matches!(store.placement(door), Some(Placement::Ground { .. })));
    assert!(!wall.fixtures.iter().any(|m| m.occupied()));
}

/// **Gate: a sledgehammer only destroys what is still in the wall.**
///
/// "Demolition never saves the door" is true of demolishing an *occupied*
/// wall and is not a property of demolition. Take the door out first and
/// the sledgehammer has nothing to say about it.
#[test]
fn a_door_taken_out_first_survives_the_demolition() {
    let cat = standard_catalogue();

    // Occupied: smashing it destroys the door, every time.
    let mut store = Store::new();
    let (wall, _) = a_wall(&cat, &mut store);
    let mut saved = 0;
    for event in 0..40u64 {
        saved += back(&wall.take_down(Teardown::Smash, &cat, &store, 0.8, event), &cat, "door");
    }
    assert_eq!(saved, 0, "a sledgehammer through an occupied wall spared the door");

    // Stripped first: the door is a separate object on the floor and the
    // demolition cannot touch it.
    let mut store = Store::new();
    let (mut wall, door) = a_wall(&cat, &mut store);
    wall.perform(Stage::IsolateUtilities, &mut store, &cat);
    wall.perform(Stage::RemoveFittings, &mut store, &cat);
    wall.perform(Stage::RemoveOpenings, &mut store, &cat);

    let after = wall.take_down(Teardown::Smash, &cat, &store, 0.8, 1);
    assert_eq!(back(&after, &cat, "door"), 0, "the wall still contained a door");
    assert!(store.get(door).is_some(), "the door was destroyed with the wall it had left");
    assert!(matches!(store.placement(door), Some(Placement::Ground { .. })));
    assert_eq!(store.get(door).unwrap().condition.damage, 0.0, "it was damaged from a distance");
}

/// **Gate: what a teardown returns is knocked about, dirty, and sometimes
/// has something wrong with it nobody can see.**
///
/// Four separate questions and four separate named draws, plus an
/// event-level one so that a job which went badly went badly for
/// everything in it.
#[test]
fn separability_and_damage_and_dirt_are_different_questions() {
    let cat = standard_catalogue();
    let store = Store::new();
    let wall = WallAssembly::brick(20, &cat, JointMethod::LimeMortared);

    let survey = |how: Teardown| {
        let (mut n, mut damage, mut dirt, mut hidden) = (0.0f64, 0.0f64, 0.0f64, 0);
        for event in 0..60u64 {
            for c in &wall.take_down(how, &cat, &store, 0.8, event).components {
                n += 1.0;
                damage += c.condition.damage;
                dirt += c.condition.contamination;
                hidden += c.hidden_defect as u32;
            }
        }
        (damage / n.max(1.0), dirt / n.max(1.0), hidden as f64 / n.max(1.0))
    };

    let careful = survey(Teardown::Deconstruct);
    let rough = survey(Teardown::Salvage);

    assert!(rough.0 > careful.0, "rough work damaged nothing more than careful work");
    assert!(rough.1 > careful.1, "rough work returned no dirtier bricks");
    assert!(rough.2 > careful.2, "rough work hid no more defects");
    // And they are separate: a careful job still has some of each, and
    // none of them is simply a copy of another.
    assert!(careful.0 > 0.0 && careful.1 > 0.0);
    assert!((careful.0 - careful.1).abs() > 1e-6, "damage and dirt were the same number");
    assert!(careful.2 < 0.5, "half of a careful teardown came out secretly cracked");
}

/// **Gate: a fixture taken out of a wall is the same fixture.**
#[test]
fn the_door_that_comes_out_is_the_door_that_went_in() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let (mut wall, door) = a_wall(&cat, &mut store);
    store.get_mut(door).unwrap().condition.wear = 0.35;
    store.get_mut(door).unwrap().given_name = Some("the one off the old house".into());

    assert!(matches!(store.placement(door), Some(Placement::Installed { host: Host::Building(10), .. })));
    let opening = wall.fixture_named("doorway").unwrap();
    let back = wall.uninstall(&mut store, opening).unwrap();
    assert_eq!(back, door);
    let d = store.get(back).unwrap();
    assert!((d.condition.wear - 0.35).abs() < 1e-9);
    assert_eq!(d.given_name.as_deref(), Some("the one off the old house"));
    assert!(matches!(store.placement(back), Some(Placement::Ground { .. })));
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
        assert!(matches!(store.placement(i), Some(Placement::Ground { .. })));
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

/// **Gate: the crash model is asserted, not sampled.**
///
/// Three hundred crashes are a diagnostic. A proportion over 300 draws can
/// pass or fail by luck even when the model underneath is exactly right,
/// which is the same small-sample mistake this project already had to
/// unlearn over first-pass yield. So the probabilities themselves are the
/// gate.
#[test]
fn the_crash_probabilities_are_the_model_and_they_are_checked_directly() {
    use scale_sim::fitted::{chance_destroyed, chance_detached, chance_jammed, grip_of};

    // Every probability is a probability.
    for severity in [0.0, 0.05, 0.3, 0.5, 0.9, 1.0] {
        for robustness in [0.0, 0.25, 0.5, 0.8, 1.0] {
            let p = chance_destroyed(severity, robustness);
            assert!((0.0..=1.0).contains(&p), "P(destroyed) = {p} at {severity}/{robustness}");
        }
        assert!((0.0..=1.0).contains(&chance_jammed(severity)));
        for j in [JointMethod::Welded, JointMethod::Clipped, JointMethod::Bolted] {
            assert!((0.0..=1.0).contains(&chance_detached(j, severity)));
        }
    }

    // **A heavier impact breaks more**, monotonically, at every strength.
    for robustness in [0.1, 0.5, 0.9] {
        let mut last = -1.0;
        for severity in [0.0, 0.1, 0.3, 0.5, 0.7, 0.9, 1.0] {
            let p = chance_destroyed(severity, robustness);
            assert!(p >= last, "breaking got less likely as the impact got harder");
            last = p;
        }
    }
    assert!(chance_destroyed(0.9, 0.5) > chance_destroyed(0.1, 0.5));
    // And a nudge breaks almost nothing, which is what makes it a nudge.
    assert!(chance_destroyed(0.1, 0.8) < 0.02, "a light knock destroyed things");

    // **A stronger thing survives what a weaker one does not.**
    assert!(chance_destroyed(0.6, 0.2) > chance_destroyed(0.6, 0.9));

    // **A clip lets go where a weld holds** — which is why a crash strips
    // the trim off a car and leaves the engine mounts alone.
    assert!(grip_of(JointMethod::Welded) > grip_of(JointMethod::Bolted));
    assert!(grip_of(JointMethod::Bolted) > grip_of(JointMethod::Clipped));
    for severity in [0.2, 0.5, 0.9] {
        assert!(
            chance_detached(JointMethod::Clipped, severity)
                > chance_detached(JointMethod::Welded, severity),
            "a weld gave way as readily as a clip at severity {severity}"
        );
    }
    // Nothing comes off in a crash that did not happen.
    assert_eq!(chance_detached(JointMethod::Clipped, 0.0), 0.0);
    assert_eq!(chance_destroyed(0.0, 0.0), 0.0);

    // **Jamming is conditional**, and it never happens below the point at
    // which the structure is folding at all.
    assert_eq!(chance_jammed(0.5), 0.0, "a light knock jammed something in");
    assert!(chance_jammed(0.95) > 0.0);
}

/// The sampled run is kept as a **diagnostic** rather than as proof, and
/// it is asserted loosely enough that sampling noise cannot fail it.
#[test]
fn a_population_of_crashes_looks_like_the_model_says_it_should() {
    use scale_sim::fitted::chance_destroyed;
    let cat = standard_catalogue();
    let mut destroyed = 0;
    let n = 400;
    for event in 0..n as u64 {
        let mut store = Store::new();
        let (mut van, loose) = a_van(&cat, &mut store);
        let bracket = van.mount_named("alternator bracket").unwrap();
        van.install(&mut store, bracket, loose[0], &cat, &[], None, 1).unwrap();
        if let Some(InstallationFailure::Destroyed { .. }) =
            van.wreck_mount(&mut store, bracket, 0.9, &cat, event, 1)
        {
            destroyed += 1;
        }
    }
    // A fresh alternator: structural integrity 0.8, sound, so robustness
    // is 0.8 and the model says about a fifth of them.
    let expected = chance_destroyed(0.9, 0.8);
    let seen = destroyed as f64 / n as f64;
    // Three standard errors, which is wide on purpose: this is a check
    // that the sampler agrees with the model, not a calibration.
    let se = (expected * (1.0 - expected) / n as f64).sqrt();
    assert!(
        (seen - expected).abs() < 3.0 * se + 0.02,
        "the sample says {seen:.3} and the model says {expected:.3}"
    );
}

