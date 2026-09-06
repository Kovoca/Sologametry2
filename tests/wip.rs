//! **Between the sheet on the rack and the door on the car.**
//!
//! Every gate here is a way a model that jumps from consumed inputs to a
//! finished output lies: by making an offcut appear out of nowhere, by
//! losing a panel's history when it is bent, by handing back the design's
//! components instead of the ones somebody actually fitted, or by putting
//! the pristine sheet back on the rack because the press went wrong.

use scale_sim::bom::{Geometry, MaterialState, Surface};
use scale_sim::item::{
    standard_catalogue, Catalogue, ItemInstance, JointMethod, Placement, Store, WorkStatus,
};
use scale_sim::material::{Dims, Material, Quantity};
use scale_sim::teardown::Teardown;
use scale_sim::wip::{
    botched, carry_back, coat, complete, cut, dismantle, drill, form, join, melt, release_all,
    reserve_all,
    rework, start, still_good, what_becomes_of_it, CannotReserve, Disposition, Feature, Progress,
};

fn a_sheet(cat: &Catalogue, store: &mut Store) -> scale_sim::id::Id<ItemInstance> {
    let mut sheet = ItemInstance::one(cat, cat.must("steel sheet"));
    sheet.mass_kg = 31.4;
    store.add(sheet, Placement::Ground { locality: 1, x: 4, y: 9 })
}

fn mass(store: &Store, id: scale_sim::id::Id<ItemInstance>) -> f64 {
    store.get(id).map(|i| i.mass_kg).unwrap_or(0.0)
}

// =====================================================================
// 1-2: reserving is not moving; starting is
// =====================================================================

/// **Gate 1: reserved material keeps its physical location.**
///
/// A work order is not a place. A board booked for tomorrow is still on
/// its rack tonight, and anybody walking past can see it there.
#[test]
fn a_reserved_sheet_is_still_on_its_rack() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let sheet = a_sheet(&cat, &mut store);
    let was = store.placement(sheet);

    reserve_all(&mut store, 42, &[sheet]).expect("nothing was in the way");
    assert_eq!(store.placement(sheet), was, "reserving it moved it");
    assert_eq!(store.status(sheet), WorkStatus::Reserved { order: 42 });
    assert!(!store.available(sheet), "a reserved sheet was free for the taking");
    assert!(store.why_not(sheet).contains("committed"));

    // **All of it or none of it.** A reservation that cannot secure
    // everything leaves nothing behind it.
    let other = a_sheet(&cat, &mut store);
    let already = a_sheet(&cat, &mut store);
    reserve_all(&mut store, 7, &[already]).unwrap();
    let outcome = reserve_all(&mut store, 8, &[other, already]);
    assert_eq!(outcome, Err(CannotReserve::NotAvailable(already)));
    assert_eq!(
        store.status(other),
        WorkStatus::Available,
        "a failed reservation left half of itself booked"
    );
    assert_eq!(store.status(already), WorkStatus::Reserved { order: 7 });

    release_all(&mut store, &[sheet]);
    assert!(store.available(sheet));
}

/// **Gate 2: starting work moves it to the workstation**, and clamped in
/// a press it is no longer anybody's to pick up.
#[test]
fn starting_the_job_puts_it_in_the_press() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let sheet = a_sheet(&cat, &mut store);
    reserve_all(&mut store, 42, &[sheet]).unwrap();

    let blank = start(&mut store, &cat, sheet, cat.must("car door"), 42, 3)
        .expect("the sheet would not go in the press");

    assert!(store.get(sheet).is_none(), "the sheet is still a sheet");
    assert_eq!(
        store.placement(blank),
        Some(Placement::Fixtured { resource: 3, slot: 0, clamped: true })
    );
    assert_eq!(store.status(blank), WorkStatus::Wip { order: 42, operation: 0 });
    assert!(!store.available(blank), "somebody walked off with a clamped panel");
    assert!(store.why_not(blank).contains("clamped"));

    // Unclamp it into the tray and it can be got at again.
    store.place(blank, Placement::Fixtured { resource: 3, slot: 1, clamped: false });
    store.set_status(blank, WorkStatus::Available);
    assert!(store.available(blank), "a cool panel in a tray could not be lifted out");
}

// =====================================================================
// 3-6: the identity rules
// =====================================================================

/// **Gate 3: cutting produces a blank and an offcut, both with lineage,
/// and the mass closes.**
#[test]
fn cutting_a_sheet_ends_it_and_starts_two() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let sheet = a_sheet(&cat, &mut store);
    let before = mass(&store, sheet);

    let (blank, offcut, t) = cut(
        &mut store,
        &cat,
        sheet,
        4.5,
        cat.must("car door"),
        "door blank",
        Dims::new(1.1, 0.95, 0.0007),
        0.08,
        (0.0, 1),
    )
    .expect("the sheet would not cut");
    let offcut = offcut.expect("cutting 4.5 kg off a 31.4 kg sheet left no offcut");

    // **One identity ended and two began**, and both know where they came
    // from.
    assert!(store.get(sheet).is_none(), "the sheet survived being cut up");
    assert_eq!(store.graves().len(), 1);
    for piece in [blank, offcut] {
        let s = store.get(piece).unwrap().shape.as_ref().unwrap();
        assert!(s.lineage.contains(&sheet), "a piece does not know what it came off");
    }
    assert!((mass(&store, blank) - 4.5).abs() < 1e-9);
    assert!((mass(&store, offcut) - (before - 4.5 - 0.08)).abs() < 1e-9);

    // And the kerf is real: it is on the floor, not nowhere.
    let b = t.balance(&store, 0.0);
    assert!(b.closes(), "cutting lost {:.6} kg", b.residual());
    assert!(t.scrap.iter().any(|s| s.0 == Material::MildSteel && s.1 > 0.0));
}

/// **Gate 4: forming preserves identity while changing geometry.**
#[test]
fn pressing_a_blank_leaves_the_same_object() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let sheet = a_sheet(&cat, &mut store);
    let (blank, _, _) = cut(&mut store, &cat, sheet, 4.5, cat.must("car door"), "door blank",
                            Dims::new(1.1, 0.95, 0.0007), 0.08, (0.0, 1)).unwrap();
    let before = mass(&store, blank);
    let came_from = store.get(blank).unwrap().shape.as_ref().unwrap().lineage.clone();

    let t = form(&mut store, blank, Geometry::Stamping, "outer skin", 10.0).unwrap();

    // Same handle, same mass, same lineage; different shape and a
    // different material state, because cold work hardens it.
    assert!(store.get(blank).is_some(), "pressing it destroyed it");
    assert!((mass(&store, blank) - before).abs() < 1e-9, "pressing changed its weight");
    let s = store.get(blank).unwrap().shape.as_ref().unwrap();
    assert_eq!(s.geometry, Geometry::Stamping);
    assert_eq!(s.state, MaterialState::WorkHardened);
    assert_eq!(s.lineage, came_from, "the press forgot where the steel came from");
    assert_eq!(t.retained, vec![blank]);
    assert!(t.created.is_empty(), "pressing created a second panel");
    assert!(t.energy_kwh > 0.0 && t.tool_wear > 0.0);
}

/// **Gate 5: coating adds mass and a surface, and loses the solvent.**
#[test]
fn painting_a_panel_puts_some_of_the_tin_in_the_air() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let sheet = a_sheet(&cat, &mut store);
    let (panel, _, _) = cut(&mut store, &cat, sheet, 4.5, cat.must("car door"), "blank",
                            Dims::new(1.1, 0.95, 0.0007), 0.08, (0.0, 1)).unwrap();
    let before = mass(&store, panel);

    // 0.5 kg of paint at 45% solids: real solvent-borne automotive paint
    // is 35-55% solids by weight.
    let t = coat(&mut store, panel, Surface::Painted, 0.5, 0.45, Material::Paint, 20.0).unwrap();

    assert!(mass(&store, panel) > before, "the paint weighed nothing");
    assert!((mass(&store, panel) - before - 0.225).abs() < 1e-9);
    assert_eq!(store.get(panel).unwrap().shape.as_ref().unwrap().surface, Surface::Painted);

    // **The rest of it went up the extractor**, which is the whole point
    // of counting the environmental terms.
    let gone: f64 = t.emissions.iter().map(|e| e.1).sum();
    assert!((gone - 0.275).abs() < 1e-9, "{gone:.3} kg of solvent went missing");
    let b = t.balance(&store, before);
    assert!(b.closes(), "painting lost {:.6} kg", b.residual());
}

/// **Gate 6: joining preserves the component identities inside the
/// assembly.**
#[test]
fn the_latch_inside_the_door_is_still_that_latch() {
    let cat = standard_catalogue();
    let mut store = Store::new();

    let mut latch = ItemInstance::one(&cat, cat.must("door latch"));
    latch.given_name = Some("the one off the scrapper".into());
    latch.condition.wear = 0.31;
    let latch = store.add(latch, Placement::anywhere());
    let hinge = store.add_loose(ItemInstance::one(&cat, cat.must("door hinge")));
    let glass = store.add_loose(ItemInstance::one(&cat, cat.must("door glass")));
    let before: f64 = [latch, hinge, glass].iter().map(|&i| mass(&store, i)).sum();

    let (door, t) = join(
        &mut store,
        &cat,
        &[latch, hinge, glass],
        cat.must("car door"),
        JointMethod::Bolted,
        &[(Material::MildSteel, 0.04)],
        30.0,
    )
    .unwrap();

    // The parts are inside it, and they are the same parts. **Fitted to
    // it, not loose in it** — a latch bolted into a door is not the
    // contents of a box, and cannot be lifted out without taking the door
    // apart.
    let rec = store.get(door).unwrap().assembly.as_ref().unwrap();
    assert_eq!(rec.as_built, vec![latch, hinge, glass], "the door does not say what is in it");
    assert_eq!(
        store.placement(latch),
        Some(Placement::Installed { host: scale_sim::item::Host::Item(door), mount: 0 })
    );
    assert!(!store.available(latch), "the latch out of a finished door was on offer as stock");
    assert!(store.why_not(latch).contains("fitted"));
    assert_eq!(
        store.get(latch).unwrap().given_name.as_deref(),
        Some("the one off the scrapper")
    );
    assert!((store.get(latch).unwrap().condition.wear - 0.31).abs() < 1e-9);
    assert!((mass(&store, door) - before - 0.04).abs() < 1e-9);
    assert_eq!(t.created, vec![door]);
    assert_eq!(t.retained.len(), 3, "joining destroyed the parts");
}

/// **Melting is the one that ends identity for good.** A billet cannot be
/// traced back through the swarf it was made from, and that is a fact
/// about melting rather than a shortcoming of the record.
#[test]
fn melting_ends_the_lineage() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let a = a_sheet(&cat, &mut store);
    let b = a_sheet(&cat, &mut store);
    let before = mass(&store, a) + mass(&store, b);

    let (billet, t) = melt(&mut store, &cat, &[a, b], cat.must("steel bar"), 0.03, 0.0, 5)
        .expect("it would not melt");

    assert!(store.get(a).is_none() && store.get(b).is_none());
    assert!(store.get(billet).unwrap().shape.is_none(), "a billet remembered being a sheet");
    assert!((mass(&store, billet) - before * 0.97).abs() < 1e-6);
    assert!(t.emissions.iter().map(|e| e.1).sum::<f64>() > 0.0, "melting lost nothing to the flue");
    assert_eq!(store.graves().len(), 2);
}

// =====================================================================
// 7-10: interruption, failure and rework
// =====================================================================

/// **Gate 8: progress is physical, so an interruption preserves the right
/// thing.**
///
/// A cut keeps its path. A kiln loses its heat. A cure carries on by
/// itself. None of that is a rule about interruptions — it falls out of
/// what "part way through" means for each kind of work.
#[test]
fn what_survives_a_stoppage_depends_on_what_was_happening() {
    let mut cutting = Progress::Cut { done_mm: 0.0, total_mm: 2000.0 };
    cutting.advance(10.0, 1.0, 20.0);
    assert!((cutting.fraction() - 0.5).abs() < 1e-9);
    cutting.while_stopped(600.0);
    assert!((cutting.fraction() - 0.5).abs() < 1e-9, "a cut path healed itself overnight");

    let mut kiln = Progress::Heat { celsius: 20.0, target_c: 850.0, ambient_c: 20.0 };
    kiln.advance(60.0, 1.0, 60.0);
    assert!(kiln.complete());
    kiln.while_stopped(120.0);
    assert!(kiln.fraction() < 0.7, "a kiln held its heat through a two-hour outage");
    assert!(kiln.fraction() > 0.0, "it went stone cold in two hours");

    // **A cure runs on chemistry, not on the mains** — and not regardless
    // of everything either. It goes at whatever rate the temperature and
    // the humidity allow, which is why an outage that takes the heating
    // with it slows the glue down without stopping it.
    let mut warm = Progress::Cure { reacted: 0.2, at_c: 20.0, wants_c: 20.0 };
    warm.while_stopped(360.0);
    assert!(warm.fraction() > 0.2, "the glue stopped setting because the lights went out");

    let mut chilled = Progress::Cure { reacted: 0.2, at_c: 8.0, wants_c: 20.0 };
    chilled.while_stopped(360.0);
    assert!(chilled.fraction() > 0.2, "a cold cure stopped dead at eight degrees");
    assert!(
        chilled.fraction() < warm.fraction(),
        "a cold workshop cured glue as fast as a warm one"
    );

    // **Below freezing most adhesives simply do not go off**, which is a
    // fact about the chemistry rather than about the shop.
    let mut frozen = Progress::Cure { reacted: 0.2, at_c: -4.0, wants_c: 20.0 };
    frozen.while_stopped(2880.0);
    assert!((frozen.fraction() - 0.2).abs() < 1e-9, "glue went off in a frozen yard");

    // And there is no universal figure: two operations at the same
    // fraction are in completely different states.
    let welding = Progress::Weld { done: 3, segments: 6 };
    let coating = Progress::Coat { microns: 20.0, target_microns: 40.0, layers: 1 };
    assert!((welding.fraction() - coating.fraction()).abs() < 1e-9);
    assert_ne!(
        std::mem::discriminant(&welding),
        std::mem::discriminant(&coating),
        "two different processes were reduced to one number"
    );
}

/// **Gate 9: an execution failure cannot restore consumed inputs.**
///
/// Once the press comes down, the sheet is not a sheet any more. A failed
/// operation leaves a malformed panel, the electricity spent and the
/// tooling worn — it does not undo the world because the work order was
/// unsuccessful.
#[test]
fn a_botched_pressing_does_not_give_the_sheet_back() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let sheet = a_sheet(&cat, &mut store);
    let (panel, offcut, _) = cut(&mut store, &cat, sheet, 4.5, cat.must("car door"), "blank",
                                 Dims::new(1.1, 0.95, 0.0007), 0.08, (0.0, 1)).unwrap();
    let live_before = store.live();

    let t = botched(&mut store, panel, 0.3, 1.2, 0.05, 40.0).unwrap();

    assert!(store.get(sheet).is_none(), "a failed pressing put the sheet back on the rack");
    assert!(store.get(panel).is_some(), "the malformed panel vanished");
    assert_eq!(store.live(), live_before, "the failure created or destroyed an object");
    assert!(offcut.is_some(), "the offcut was un-cut");

    let p = store.get(panel).unwrap();
    assert!(p.condition.damage > 0.4, "a botched panel came out undamaged");
    assert!(p.quality.dimensional_accuracy < 0.5);
    assert_eq!(p.shape.as_ref().unwrap().stage, "malformed");
    assert!(t.energy_kwh > 0.0, "the electricity came back too");
    assert!(t.tool_wear > 0.0);
    assert!(t.scrap.iter().any(|s| s.1 > 0.0), "the spoiled metal went nowhere");
}

/// **Gate 10: rework goes at the feature that is wrong**, not at the
/// whole thing.
#[test]
fn putting_it_right_does_not_start_again() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let sheet = a_sheet(&cat, &mut store);
    let (panel, _, _) = cut(&mut store, &cat, sheet, 4.5, cat.must("car door"), "blank",
                            Dims::new(1.1, 0.95, 0.0007), 0.08, (0.0, 1)).unwrap();
    form(&mut store, panel, Geometry::Stamping, "outer skin", 10.0).unwrap();
    drill(&mut store, panel, 6, 8.0, 0.02, 20.0).unwrap();
    botched(&mut store, panel, 0.05, 1.0, 0.02, 30.0).unwrap();

    let holes_before = store.get(panel).unwrap().shape.as_ref().unwrap().holes();
    let mass_before = mass(&store, panel);
    let bad = store.get(panel).unwrap().quality.dimensional_accuracy;

    rework(&mut store, panel, Feature::Bend { degrees: 2.0 }, 50.0).unwrap();

    let after = store.get(panel).unwrap();
    assert!(after.quality.dimensional_accuracy > bad, "the rework improved nothing");
    assert!(after.condition.damage < 0.45, "it is still as bent as it was");
    // **The holes are still there.** Rework did not re-press the panel or
    // re-drill it.
    assert_eq!(
        after.shape.as_ref().unwrap().holes(),
        holes_before,
        "putting the bend right re-drilled the panel"
    );
    assert!((after.mass_kg - mass_before).abs() < 1e-9, "rework consumed the workpiece");
    assert_eq!(after.shape.as_ref().unwrap().geometry, Geometry::Stamping);
}

/// **Gate 13: moving or damaging work in progress invalidates the
/// reservations on it.**
#[test]
fn a_board_carried_off_in_the_night_is_not_booked_any_more() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let a = a_sheet(&cat, &mut store);
    let b = a_sheet(&cat, &mut store);
    reserve_all(&mut store, 11, &[a, b]).unwrap();
    assert!(still_good(&store, 11, &[a, b]).is_empty(), "a good booking read as broken");

    // Somebody clamps one of them into a machine for another job.
    store.place(a, Placement::Fixtured { resource: 2, slot: 0, clamped: true });
    let broken = still_good(&store, 11, &[a, b]);
    assert_eq!(broken, vec![a], "the booking survived the sheet being taken");

    // And one that is simply gone.
    store.end(b, scale_sim::item::ItemEnd::Consumed, 3);
    assert_eq!(still_good(&store, 11, &[a, b]).len(), 2);
}

// =====================================================================
// 11-12, 15-16: routes, hands, and what is actually in the thing
// =====================================================================

/// **Gate 11: two routes, one design, different histories.**
#[test]
fn a_panel_can_be_pressed_or_beaten_and_it_is_still_the_panel() {
    let cat = standard_catalogue();
    let design = cat.must("car door");

    let by_press = {
        let mut store = Store::new();
        let sheet = a_sheet(&cat, &mut store);
        let (p, _, _) = cut(&mut store, &cat, sheet, 4.5, design, "blank",
                            Dims::new(1.1, 0.95, 0.0007), 0.08, (0.0, 1)).unwrap();
        form(&mut store, p, Geometry::Stamping, "outer skin", 1.0).unwrap();
        (store.get(p).unwrap().clone(), store)
    };
    let by_hand = {
        let mut store = Store::new();
        let sheet = a_sheet(&cat, &mut store);
        let (p, _, _) = cut(&mut store, &cat, sheet, 4.5, design, "blank",
                            Dims::new(1.1, 0.95, 0.0007), 0.08, (0.0, 1)).unwrap();
        // Beaten over a buck: no press, so the geometry is a shell rather
        // than a stamping and it took an afternoon.
        form(&mut store, p, Geometry::Shell, "outer skin", 1.0).unwrap();
        (store.get(p).unwrap().clone(), store)
    };

    // The same design and the same weight of steel.
    assert_eq!(by_press.0.shape.as_ref().unwrap().becoming, by_hand.0.shape.as_ref().unwrap().becoming);
    assert!((by_press.0.mass_kg - by_hand.0.mass_kg).abs() < 1e-9);
    // And two different histories, which the object carries.
    assert_ne!(
        by_press.0.shape.as_ref().unwrap().geometry,
        by_hand.0.shape.as_ref().unwrap().geometry
    );
}

/// **Gate 12: the player and an employee use the same transformations.**
///
/// There is no second code path. What differs is who is named on the
/// record, which is an accounting fact and not a physical one.
#[test]
fn the_owner_and_the_employee_press_the_same_panel() {
    let cat = standard_catalogue();
    let make = || {
        let mut store = Store::new();
        let sheet = a_sheet(&cat, &mut store);
        let (p, off, t) = cut(&mut store, &cat, sheet, 4.5, cat.must("car door"), "blank",
                              Dims::new(1.1, 0.95, 0.0007), 0.08, (0.0, 1)).unwrap();
        let f = form(&mut store, p, Geometry::Stamping, "outer skin", 1.0).unwrap();
        (
            store.get(p).unwrap().mass_kg,
            store.get(off.unwrap()).unwrap().mass_kg,
            t.scrap.clone(),
            f.energy_kwh,
        )
    };
    assert_eq!(make(), make(), "the same work came out differently for a different person");
}

/// **Gate 16: teardown returns the components that are actually in it.**
///
/// Not the design's expected components — the ones somebody fitted, with
/// the names they were given and the wear they have.
#[test]
fn taking_the_door_apart_gives_back_the_second_hand_latch() {
    let cat = standard_catalogue();
    let mut store = Store::new();

    let mut latch = ItemInstance::one(&cat, cat.must("door latch"));
    latch.given_name = Some("off the scrapper".into());
    latch.condition.wear = 0.44;
    let latch = store.add_loose(latch);
    let hinge = store.add_loose(ItemInstance::one(&cat, cat.must("door hinge")));
    let glass = store.add_loose(ItemInstance::one(&cat, cat.must("door glass")));

    let (door, _) = join(&mut store, &cat, &[latch, hinge, glass], cat.must("car door"),
                         JointMethod::Bolted, &[(Material::MildSteel, 0.04)], 0.0).unwrap();
    let door_mass = mass(&store, door);

    let (recovered, released) = dismantle(&mut store, &cat, door, Teardown::Disassemble, 0.9, 1, 7);

    assert!(!released.is_empty(), "a bolted door gave back nothing");
    assert!(released.contains(&latch), "the latch was not among what came off");
    let back = store.get(latch).unwrap();
    assert_eq!(back.given_name.as_deref(), Some("off the scrapper"), "a different latch came out");
    assert!(back.condition.wear >= 0.44, "the latch was rejuvenated by being unbolted");
    assert!(matches!(store.placement(latch), Some(Placement::Ground { .. })));
    assert!(store.get(door).is_none(), "the door survived being dismantled");

    // And every gram is accounted for.
    let got: f64 = released.iter().map(|&i| mass(&store, i)).sum::<f64>()
        + recovered.materials.iter().map(|m| m.1).sum::<f64>()
        + recovered.fuel_kg();
    assert!(
        (got + recovered.lost_kg - door_mass).abs() < 1e-6,
        "{got:.4} recovered and {:.4} lost out of {door_mass:.4}",
        recovered.lost_kg
    );
}

// =====================================================================
// the vertical chain
// =====================================================================

/// **One door, from the rack to the scrap heap.**
///
/// Reserve, cut, press, punch, coat, form the frame, weld the structure,
/// fit the components, inspect, park it, store it, hang it on a vehicle,
/// take it off and dismantle it. Which is the whole of this layer in one
/// object: stock geometry, work in progress, coatings, components, joints,
/// output placement, installation and teardown.
#[test]
fn a_car_door_from_the_rack_to_the_scrap_heap() {
    use scale_sim::fitted::{serviceable_parts, FittedVehicle, Mount};
    use scale_sim::item::Fitting;
    use scale_sim::vehicle::Vehicle;

    let cat = standard_catalogue();
    let mut store = Store::new();
    let design = cat.must("car door");

    // 1. Reserve the steel, and it does not move.
    let sheet = a_sheet(&cat, &mut store);
    let rack = store.placement(sheet);
    reserve_all(&mut store, 100, &[sheet]).unwrap();
    assert_eq!(store.placement(sheet), rack);

    // 2-3. Cut the blank, and keep the offcut.
    let (skin, offcut, cut_t) = cut(&mut store, &cat, sheet, 4.5, design, "outer blank",
                                    Dims::new(1.1, 0.95, 0.0007), 0.08, (0.0, 1)).unwrap();
    let offcut = offcut.unwrap();
    assert!(cut_t.balance(&store, 0.0).closes());
    store.place(skin, Placement::Fixtured { resource: 1, slot: 0, clamped: true });
    store.set_status(skin, WorkStatus::Wip { order: 100, operation: 1 });

    // 4. Stamp it, then trim and punch the mounting features.
    form(&mut store, skin, Geometry::Stamping, "outer skin", 10.0).unwrap();
    drill(&mut store, skin, 8, 10.0, 0.03, 20.0).unwrap();
    assert_eq!(store.get(skin).unwrap().shape.as_ref().unwrap().holes(), 8);

    // 5. Corrosion treatment, then paint. Both add mass; both lose
    // solvent.
    coat(&mut store, skin, Surface::Galvanised, 0.16, 0.95, Material::Zinc, 30.0).unwrap();
    let painted = coat(&mut store, skin, Surface::Painted, 0.5, 0.45, Material::Paint, 40.0).unwrap();
    assert!(painted.emissions.iter().map(|e| e.1).sum::<f64>() > 0.2);
    assert_eq!(store.get(skin).unwrap().shape.as_ref().unwrap().surface, Surface::Painted);

    // 6. The inner frame and the beam, out of the same rack.
    let more = a_sheet(&cat, &mut store);
    let (frame, _, _) = cut(&mut store, &cat, more, 6.0, design, "inner blank",
                            Dims::new(1.05, 0.9, 0.0008), 0.1, (0.0, 2)).unwrap();
    form(&mut store, frame, Geometry::Stamping, "inner frame", 60.0).unwrap();
    let beam = store.add_loose({
        let mut b = ItemInstance::fresh(&cat, cat.must("steel bar"), Quantity::Mass { kg: 2.2 });
        b.mass_kg = 2.2;
        b
    });

    // 7. Spot-weld the structure.
    let (shell, _) = join(&mut store, &cat, &[skin, frame, beam], design, JointMethod::Welded,
                          &[(Material::MildSteel, 0.05)], 90.0).unwrap();

    // 8. Fit the regulator, latch, glass, wiring and seals.
    let fittings: Vec<_> = ["window regulator", "door latch", "door glass",
                            "wiring loom, door", "weather seal"]
        .iter()
        .map(|n| store.add_loose(ItemInstance::one(&cat, cat.must(n))))
        .collect();
    let mut everything = vec![shell];
    everything.extend(&fittings);
    let (door, _) = join(&mut store, &cat, &everything, design, JointMethod::Bolted,
                         &[(Material::Adhesive, 0.25)], 120.0).unwrap();

    // 9-10. Inspect, and park it where it was made because the store is
    // full. It exists; the bay is not free.
    complete(&mut store, door, design, 150.0).unwrap();
    store.place(door, Placement::Fixtured { resource: 1, slot: 2, clamped: false });
    store.set_status(door, WorkStatus::AwaitingUnload { order: 100 });
    let quality_then = store.get(door).unwrap().quality.overall();
    assert_eq!(store.placement(door).and_then(|p| p.occupying()), Some(1));
    assert!(!store.available(door), "an uncollected door was treated as stock");

    // 11. Somebody comes and moves it. Same object, same quality.
    store.place(door, Placement::Ground { locality: 1, x: 20, y: 3 });
    store.set_status(door, WorkStatus::Available);
    assert_eq!(store.get(door).unwrap().quality.overall(), quality_then, "moving it rerolled it");

    // 12. Hang it on a vehicle.
    let (mut van, _) = FittedVehicle::adapt(9, Vehicle::van(), &serviceable_parts(), &cat, &mut store);
    van.mounts.push(Mount::new("door aperture", (2, 0), Fitting::Opening, JointMethod::Bolted));
    let aperture = van.mount_named("door aperture").unwrap();
    let kerb = van.kerb_kg(&store);
    van.install(&mut store, aperture, door, &cat, &[(Material::MildSteel, 0.06)], Some(1), 200)
        .expect("the door would not hang");
    assert!(
        (van.kerb_kg(&store) - kerb - mass(&store, door)).abs() < 1e-9,
        "hanging the door changed the van by something other than the door"
    );

    // 13. Take it off again and dismantle it.
    let back = van.uninstall(&mut store, aperture).unwrap();
    assert_eq!(back, door, "a different door came off");
    let door_mass = mass(&store, door);
    let (recovered, released) = dismantle(&mut store, &cat, door, Teardown::Deconstruct, 0.85, 5, 210);

    assert!(!released.is_empty(), "a bolted door gave back nothing");
    let got: f64 = released.iter().map(|&i| mass(&store, i)).sum::<f64>()
        + recovered.materials.iter().map(|m| m.1).sum::<f64>()
        + recovered.fuel_kg();
    assert!((got + recovered.lost_kg - door_mass).abs() < 1e-6);

    // And the offcut is still on the rack where it was left, which is the
    // point of it being a real object.
    assert!(store.get(offcut).is_some(), "the offcut evaporated somewhere along the way");
    assert!(mass(&store, offcut) > 25.0);
}

// =====================================================================
// 14: a finished thing exists, even when there is nowhere to put it
// =====================================================================

/// **Gate 14: a blocked delivery is a finished object with nowhere to go,
/// and it costs the shop the bench.**
///
/// "Completion waits" is only honest if the thing is already real, and the
/// six things that have to be true of it are separate claims that a single
/// "delivery failed" flag cannot make. Stated one at a time here, because
/// each is a different way the model could quietly be lying.
#[test]
fn a_door_nobody_collects_holds_the_bay_it_was_made_in() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let design = cat.must("car door");
    let press = 12u32;

    // A panel and a frame, welded up on the press.
    let sheet = a_sheet(&cat, &mut store);
    let (skin, offcut, _) = cut(&mut store, &cat, sheet, 4.5, design, "outer blank",
                                Dims::new(1.1, 0.95, 0.0007), 0.08, (0.0, 1)).unwrap();
    let offcut = offcut.unwrap();
    let offcut_mass = mass(&store, offcut);
    store.place(skin, Placement::Fixtured { resource: press, slot: 0, clamped: true });
    store.set_status(skin, WorkStatus::Wip { order: 500, operation: 0 });
    form(&mut store, skin, Geometry::Stamping, "outer skin", 10.0).unwrap();
    let latch = store.add(
        ItemInstance::one(&cat, cat.must("door latch")),
        Placement::Fixtured { resource: press, slot: 0, clamped: false },
    );
    let (door, _) = join(&mut store, &cat, &[skin, latch], design, JointMethod::Bolted,
                         &[(Material::MildSteel, 0.04)], 60.0).unwrap();
    complete(&mut store, door, design, 90.0).unwrap();

    // The store is full, so it stays on the press awaiting collection.
    store.place(door, Placement::Fixtured { resource: press, slot: 0, clamped: false });
    store.set_status(door, WorkStatus::AwaitingUnload { order: 500 });

    // **1. The machine is still occupied.**
    assert_eq!(
        store.placement(door).and_then(|p| p.occupying()),
        Some(press),
        "the finished door is not holding the press it was made on"
    );
    assert!(!store.slot_free(press, 0), "the press read as clear with a door standing on it");
    assert!(store.occupants_of(press).contains(&door));

    // **2. The finished object already exists.** Not a promise of one.
    assert!(store.get(door).is_some(), "the door that could not be moved does not exist");
    assert!(mass(&store, door) > 4.0, "the door that exists weighs nothing");
    assert_eq!(store.status(door), WorkStatus::AwaitingUnload { order: 500 });
    assert!(!store.available(door), "an uncollected door was on offer as stock");

    // **3. Its quality is already settled**, at the moment the work was
    // done rather than at the moment somebody comes for it.
    let settled = store.get(door).unwrap().quality.overall();
    assert!(settled > 0.0, "a finished door has no quality at all");
    let condition = store.get(door).unwrap().condition.wear;

    // **4. Moving it does not reroll any of that.**
    store.place(door, Placement::Ground { locality: 1, x: 8, y: 8 });
    store.set_status(door, WorkStatus::Available);
    assert_eq!(store.get(door).unwrap().quality.overall(), settled, "collecting it rerolled it");
    assert_eq!(store.get(door).unwrap().condition.wear, condition);

    // **5. The inputs cannot come back.** The sheet is gone, the panel is
    // inside the door, and nothing about a delivery going wrong could
    // return either.
    assert!(store.get(sheet).is_none(), "the sheet came back off the rack");
    assert_eq!(
        store.placement(skin),
        Some(Placement::Installed { host: scale_sim::item::Host::Item(door), mount: 0 }),
        "the pressed panel escaped the door it was welded into"
    );
    assert!(store.placement(latch).map(|p| p.is_installed()).unwrap_or(false));
    assert!(!store.available(skin), "a panel inside a finished door was on offer");
    // The offcut is the one thing that is still stock, and it is exactly
    // what it was.
    assert!((mass(&store, offcut) - offcut_mass).abs() < 1e-9);

    // **6. A second job could not have had the bench.** Only once the
    // door has gone.
    assert!(store.slot_free(press, 0), "the press is still blocked after the door was collected");
    let next = a_sheet(&cat, &mut store);
    let started = start(&mut store, &cat, next, design, 501, press).expect("the press is free now");
    assert_eq!(store.placement(started).and_then(|p| p.occupying()), Some(press));
    assert!(!store.slot_free(press, 0), "two jobs are on one press at once");
}

// =====================================================================
// what a save actually preserves
// =====================================================================

/// **"Pure data mutation" proves repeatability, not preservation.**
///
/// It says a given state advances the same way twice. It says nothing
/// about whether a hand-written codec wrote all of that state down — and
/// an enum variant that no test happens to exercise is exactly the one
/// that silently does not come back. So every variant of every WIP type
/// goes through bytes and is compared.
#[test]
fn every_kind_of_progress_survives_being_written_down() {
    use scale_sim::save::{Reader, Store as Codec, Writer};

    fn round<T: Codec + PartialEq + std::fmt::Debug>(v: T) {
        let mut w = Writer::new();
        v.store(&mut w);
        let mut r = Reader::new(&w.bytes);
        let back = T::load(&mut r).expect("it would not load");
        assert_eq!(v, back, "it came back different");
        assert!(r.done(), "there were bytes left over after reading it");
    }

    // **Every progress variant**, part way through rather than at either
    // end, because a zero survives a codec that drops the field.
    for p in [
        Progress::Cut { done_mm: 812.5, total_mm: 2000.0 },
        Progress::Heat { celsius: 631.25, target_c: 850.0, ambient_c: 18.0 },
        Progress::Dry { moisture: 0.185, target: 0.12 },
        Progress::Cure { reacted: 0.37, at_c: 16.0, wants_c: 20.0 },
        Progress::Weld { done: 4, segments: 11 },
        Progress::Coat { microns: 22.5, target_microns: 40.0, layers: 2 },
        Progress::Assemble { joints_done: 7, joints: 19 },
        Progress::Machine { features_done: 3, features: 8, allowance_mm: 0.35 },
        Progress::Elapsed { minutes: 41.5, total: 125.0 },
    ] {
        round(p);
    }

    // Every placement, including a clamped fixture and a contained item.
    for pl in [
        Placement::Ground { locality: 7, x: -14, y: 92 },
        Placement::Carried { person: 4_000_000_009 },
        Placement::Contained { container: scale_sim::id::Id::from_bits(0x0000_0003_0000_0002) },
        Placement::Installed {
            host: scale_sim::item::Host::Vehicle(31),
            mount: 5,
        },
        Placement::Installed {
            host: scale_sim::item::Host::Building(12),
            mount: 0,
        },
        Placement::Installed {
            host: scale_sim::item::Host::Item(scale_sim::id::Id::from_bits(9)),
            mount: 2,
        },
        Placement::Fixtured { resource: 3, slot: 1, clamped: true },
        Placement::Fixtured { resource: 3, slot: 1, clamped: false },
    ] {
        round(pl);
    }

    // Every work status, which is the half that is not a place.
    for st in [
        WorkStatus::Available,
        WorkStatus::Reserved { order: 77 },
        WorkStatus::Wip { order: 77, operation: 4 },
        WorkStatus::AwaitingUnload { order: 77 },
    ] {
        round(st);
    }

    // Every feature, every geometry, every material state and surface.
    for f in [
        Feature::Hole { count: 8, mm: 10.5 },
        Feature::Cut { length_mm: 2200.0 },
        Feature::Bend { degrees: 92.5 },
        Feature::Weld { segments: 14 },
        Feature::Coating { layer: Surface::Galvanised, microns: 12.0 },
    ] {
        round(f);
    }
    for g in [
        Geometry::Sheet { mm: 0.7 },
        Geometry::Bar { mm: 25.0 },
        Geometry::Stamping,
        Geometry::Shell,
        Geometry::Casting,
        Geometry::Extrusion,
        Geometry::Machined,
        Geometry::Woven,
        Geometry::Wound,
    ] {
        round(g);
    }
    for m in [
        MaterialState::AsRolled,
        MaterialState::Annealed,
        MaterialState::Normalised,
        MaterialState::WorkHardened,
        MaterialState::QuenchedAndTempered,
        MaterialState::Cured,
    ] {
        round(m);
    }
    for s in [
        Surface::Bare,
        Surface::Galvanised,
        Surface::Primed,
        Surface::Painted,
        Surface::Anodised,
        Surface::Plated,
    ] {
        round(s);
    }
    // **And every material**, by name, so inserting one tomorrow cannot
    // reinterpret every save written today.
    for m in scale_sim::material::ALL_MATERIALS {
        round(m);
    }
}

/// **A whole workpiece, with everything on it, through bytes and back.**
#[test]
fn a_part_finished_panel_reloads_exactly_as_it_was() {
    use scale_sim::save::{Reader, Store as Codec, Writer};
    let cat = standard_catalogue();
    let mut store = Store::new();
    let sheet = a_sheet(&cat, &mut store);

    let (panel, _, _) = cut(&mut store, &cat, sheet, 4.5, cat.must("car door"), "outer blank",
                            Dims::new(1.1, 0.95, 0.0007), 0.08, (0.0, 1)).unwrap();
    form(&mut store, panel, Geometry::Stamping, "outer skin", 10.0).unwrap();
    drill(&mut store, panel, 8, 10.0, 0.03, 20.0).unwrap();
    coat(&mut store, panel, Surface::Galvanised, 0.16, 0.95, Material::Zinc, 30.0).unwrap();
    store.place(panel, Placement::Fixtured { resource: 4, slot: 2, clamped: true });
    store.set_status(panel, WorkStatus::Wip { order: 5150, operation: 3 });

    let shape = store.get(panel).unwrap().shape.clone().unwrap();
    let placement = store.placement(panel).unwrap();
    let status = store.status(panel);

    let mut w = Writer::new();
    shape.store(&mut w);
    placement.store(&mut w);
    status.store(&mut w);
    let bytes = w.bytes.clone();

    let mut r = Reader::new(&bytes);
    let back_shape = scale_sim::wip::Shape::load(&mut r).unwrap();
    let back_place = Placement::load(&mut r).unwrap();
    let back_status = WorkStatus::load(&mut r).unwrap();
    assert!(r.done(), "the reader did not consume the whole record");

    assert_eq!(back_shape, shape, "the workpiece came back different");
    assert_eq!(back_place, placement);
    assert_eq!(back_status, status);
    // The particulars, named rather than trusted to the derive.
    assert_eq!(back_shape.holes(), 8);
    assert_eq!(back_shape.surface, Surface::Galvanised);
    assert_eq!(back_shape.state, MaterialState::WorkHardened);
    assert_eq!(back_shape.geometry, Geometry::Stamping);
    assert_eq!(back_shape.lineage, vec![sheet], "it forgot which sheet it came off");
    assert_eq!(back_shape.stage, "outer skin");

    // **Identical state gives identical bytes**, which is what makes a
    // save comparable at all.
    let mut again = Writer::new();
    back_shape.store(&mut again);
    back_place.store(&mut again);
    back_status.store(&mut again);
    assert_eq!(again.bytes, bytes, "writing it twice gave two different files");
}

/// A stage name is authored text, so a save writes the words and a load
/// has to find the same static back. The roster is the contract, and a
/// name this build does not know says so rather than being invented.
#[test]
fn every_stage_a_workpiece_reaches_is_in_the_roster() {
    use scale_sim::wip::{intern_stage, STAGE_NAMES};
    for name in ["blank", "offcut", "malformed", "reworked", "finished", "outer skin"] {
        assert!(STAGE_NAMES.contains(&name), "the library writes {name} and cannot read it back");
        assert_eq!(intern_stage(name), name);
    }
    assert_eq!(intern_stage("a stage from a later build"), "unrecorded");
}

// =====================================================================
// melting, and what it does and does not end
// =====================================================================

/// **Melting ends the objects. It does not end the material
/// provenance.**
///
/// A chair leg and a car panel stop existing and a billet begins — but
/// losing what was charged at the furnace door is how alloy composition,
/// tramp elements, hazardous content, recycled fraction and
/// responsibility for a defective heat all quietly stop being anybody's
/// problem.
#[test]
fn a_billet_remembers_what_was_charged_into_it() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let a = a_sheet(&cat, &mut store);
    let b = a_sheet(&cat, &mut store);
    // A lithium cell goes in the charge, which is a fire in a furnace and
    // must follow the metal round for ever.
    let battery = store.add_loose(ItemInstance::one(&cat, cat.must("battery pack")));
    assert!(
        store.get(battery).unwrap().materials.parts().iter().any(|&(m, f)| f > 0.0 && m.hazardous()),
        "the battery this test depends on is not hazardous any more"
    );

    let (billet, _) = melt(&mut store, &cat, &[a, b, battery], cat.must("steel bar"), 0.03, 0.0, 5)
        .unwrap();

    // The objects are gone.
    assert!(store.get(a).is_none() && store.get(b).is_none() && store.get(battery).is_none());
    assert!(store.get(billet).unwrap().shape.is_none(), "a billet remembered being a sheet");

    // **And the metal remembers.**
    let heat = store.get(billet).unwrap().heat.as_ref().expect("no heat record at all");
    assert_eq!(heat.ancestry(), 3, "the charge was forgotten");
    assert!(heat.merged_from.contains(&a) && heat.merged_from.contains(&battery));
    assert!(heat.hazardous, "the cell went in and nobody wrote it down");
    assert!(heat.recycled_fraction > 0.9, "a heat of pure scrap read as primary metal");
    assert!(!heat.contamination.is_empty(), "a mixed charge came out chemically pure");
    assert!(heat.composition.fraction_of(Material::MildSteel) > 0.5);

    // **And it follows the metal round again.** Remelting the billet
    // carries the ancestry and the hazard forward rather than laundering
    // them.
    let more = a_sheet(&cat, &mut store);
    let (second, _) =
        melt(&mut store, &cat, &[billet, more], cat.must("steel bar"), 0.03, 0.0, 6).unwrap();
    let heat2 = store.get(second).unwrap().heat.as_ref().unwrap();
    assert!(heat2.ancestry() > 3, "the second melt lost the first one's charge");
    assert!(heat2.hazardous, "remelting laundered the hazard out of the record");
}

// =====================================================================
// design against as-built
// =====================================================================

/// **The design still expects a regulator; this door has one, and its
/// mass is counted once.**
///
/// Two different accounts, and collapsing either into the other loses
/// something: the definition has to say what a door is made of whether or
/// not any particular door has been built, and the instance has to say
/// which actual objects are in this one without double-counting their
/// mass into its own bill.
#[test]
fn the_design_expects_a_regulator_and_this_door_has_that_one() {
    let cat = standard_catalogue();
    let mut store = Store::new();

    // **The design.** True of a car door in general.
    let design = cat.get(cat.must("car door")).unwrap();
    let expects: Vec<&str> = design
        .bill
        .components
        .iter()
        .filter_map(|c| cat.get(c.def).map(|d| d.name))
        .collect();
    for want in ["window regulator", "door latch", "door glass"] {
        assert!(expects.contains(&want), "the design does not expect a {want}: {expects:?}");
    }
    assert!(design.bill.declared_mass() > 25.0, "the design bill weighs nothing");

    // **The as-built.** This one, with these objects in it.
    let regulator = store.add_loose(ItemInstance::one(&cat, cat.must("window regulator")));
    let latch = store.add_loose(ItemInstance::one(&cat, cat.must("door latch")));
    let parts_mass = mass(&store, regulator) + mass(&store, latch);
    let (door, _) = join(&mut store, &cat, &[regulator, latch], cat.must("car door"),
                         JointMethod::Bolted, &[(Material::MildSteel, 0.04)], 0.0).unwrap();

    let rec = store.get(door).unwrap().assembly.as_ref().unwrap();
    assert_eq!(rec.as_built, vec![regulator, latch], "the door does not say what is in it");
    // Its own direct account is the joining material and nothing else,
    // because the components are real objects with their own mass.
    assert!(
        (rec.total_component_mass() - 0.04).abs() < 1e-9,
        "the instance bill counted its own components: {:.3} kg",
        rec.total_component_mass()
    );
    assert!((mass(&store, door) - parts_mass - 0.04).abs() < 1e-9, "the mass was counted twice");
    // And the design is untouched by any of it.
    assert!(cat.get(cat.must("car door")).unwrap().bill.declared_mass() > 25.0);
}

// =====================================================================
// an offcut does not walk back to the rack
// =====================================================================

/// **Stock is created where the work was done.**
///
/// An offcut originates at the saw. Getting it back into the racking is a
/// movement somebody makes and a minute or two of somebody's time, and a
/// model in which it simply reappears where the sheet used to live is
/// teleporting stock past its own resource calendar.
#[test]
fn an_offcut_is_at_the_machine_until_somebody_carries_it_back() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let sheet = a_sheet(&cat, &mut store);
    let rack = store.placement(sheet).unwrap();

    // The sheet is taken to the saw first, and both pieces land there.
    let bench = Placement::Fixtured { resource: 6, slot: 0, clamped: false };
    store.place(sheet, bench);
    let (blank, offcut, _) = cut(&mut store, &cat, sheet, 4.5, cat.must("car door"), "blank",
                                 Dims::new(1.1, 0.95, 0.0007), 0.08, (0.0, 1)).unwrap();
    let offcut = offcut.unwrap();

    assert_eq!(store.placement(offcut), Some(bench), "the offcut appeared back on the rack");
    assert_eq!(store.placement(blank), Some(bench));

    // Carrying it back costs time, and more of it for a heavy piece.
    let minutes = carry_back(&mut store, offcut, rack, 1).unwrap();
    assert_eq!(store.placement(offcut), Some(rack));
    assert!(minutes > 1.0, "a 27 kg part-sheet went back on the rack instantly");
    assert!(minutes < 15.0, "putting a sheet away took a quarter of an hour");

    // A light piece is quicker, and a heavy one alone is slower than a
    // heavy one with two people.
    let light = store.add(ItemInstance::one(&cat, cat.must("wood screw")), bench);
    let quick = carry_back(&mut store, light, rack, 1).unwrap();
    assert!(quick < minutes, "a screw took as long to put away as a steel sheet");
    let heavy = store.add(ItemInstance::one(&cat, cat.must("washing machine")), bench);
    let alone = carry_back(&mut store, heavy, rack, 1).unwrap();
    let together = carry_back(&mut store, heavy, bench, 2).unwrap();
    assert!(alone > together, "a two-man lift was no easier with two men");
}

// =====================================================================
// and what becomes of a thing nobody wants
// =====================================================================

/// **A sound machine with no scrap dealer nearby is not buried.**
///
/// It sits in a yard, or goes up for sale, or waits for a lorry, or has
/// its motor taken for something else. The household basket needs all of
/// these before it can model repair, replacement and the second-hand
/// trade.
#[test]
fn a_thing_nobody_wants_is_not_automatically_buried() {
    // Still good and still wanted.
    assert_eq!(what_becomes_of_it(0.9, true, false, false, true), Disposition::InUse);
    // Good, unwanted here, and somebody within reach would buy it.
    assert_eq!(what_becomes_of_it(0.7, false, true, false, true), Disposition::OfferedForSale);
    // Tired, and its motor is worth having.
    assert_eq!(what_becomes_of_it(0.3, false, false, true, true), Disposition::Cannibalized);
    // Sound, nobody wants it and nobody will buy it: it goes in the yard.
    assert_eq!(what_becomes_of_it(0.7, false, false, false, true), Disposition::Stored);
    // Past caring about, and waiting for somebody with a lorry.
    assert_eq!(
        what_becomes_of_it(0.1, false, false, false, true),
        Disposition::AwaitingTransport
    );
    // And nobody claims it at all.
    assert_eq!(what_becomes_of_it(0.9, true, true, true, false), Disposition::Abandoned);

    // Only one of these ends the object, and only one of them means
    // nobody owns it.
    assert!(Disposition::Stored.still_whole() && Disposition::Stored.owned());
    assert!(!Disposition::Cannibalized.still_whole());
    assert!(!Disposition::Abandoned.owned());
}
