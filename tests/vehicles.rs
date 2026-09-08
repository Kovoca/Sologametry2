//! Vehicles as grids of parts, CDDA's way.
//!
//! Two things are being checked here. First that the numbers a vehicle
//! reports are *computed* from what it is made of rather than typed in —
//! that is the whole point of a part list. Second that the structure is
//! real: a frame under everything, and a vehicle that comes apart where
//! it is broken.

use scale_sim::vehicle::{modelled_width_m, Part, Vehicle};

fn fleet() -> Vec<Vehicle> {
    vec![
        Vehicle::bicycle(),
        Vehicle::van(),
        Vehicle::box_truck(),
        Vehicle::artic(),
        Vehicle::reefer(),
    ]
}

/// **Width is compressed, and that is what buys interior resolution.**
///
/// A real car is about 2 m across, which at one metre to the tile is two
/// squares — and two squares cannot hold two seats, two doors and the
/// bodywork round them. Spending four squares and reading the width off
/// the table gets both: a cabin you can lay out and a vehicle that is the
/// right size on the road.
#[test]
fn width_is_read_off_the_tiles_but_not_linearly() {
    // The table itself, which is CDDA's.
    assert_eq!(modelled_width_m(1), 0.90);
    assert_eq!(modelled_width_m(4), 2.10);
    assert_eq!(modelled_width_m(7), 2.80);

    // It rises, but by less and less: the curve flattens because the law
    // flattens it.
    for t in 1..7 {
        let step = modelled_width_m(t + 1) - modelled_width_m(t);
        assert!(step > 0.0, "width should never fall with tiles");
        assert!(step <= 0.45, "a tile should never add half a metre");
    }
    assert!(
        modelled_width_m(7) - modelled_width_m(6) < modelled_width_m(2) - modelled_width_m(1),
        "the curve should flatten, not stay linear"
    );

    // **The widest ordinary vehicle is the widest one that needs no
    // paperwork.** 2.9 m is where you must give the police two days'
    // notice, and nothing in the stock fleet crosses it.
    for v in fleet() {
        assert!(
            v.width_m < 2.9,
            "{} is {:.2} m and would need an abnormal-load notice",
            v.name,
            v.width_m
        );
    }

    // And the grid is wide enough to lay a cab out in: four tiles at
    // least on anything with two seats in it.
    for v in fleet() {
        let seats = v.kinds().filter(|p| *p == Part::Seat).count();
        if seats >= 2 {
            assert!(
                v.footprint().1 >= 4,
                "{} has {seats} seats on a {}-tile width, which cannot hold \
                 two seats and the bodywork round them",
                v.name,
                v.footprint().1
            );
        }
    }
}

/// **Nothing is bolted to thin air.** CDDA's rule, and the one that makes
/// a part list a structure rather than a bag: the frame has to be there
/// before anything can be installed onto it.
#[test]
fn every_part_has_a_frame_under_it() {
    for v in fleet() {
        assert!(v.well_formed(), "{} has a part hanging in mid-air", v.name);
        assert!(v.supported(), "{} does not stand on its own wheels", v.name);
        // One object, not several: a vehicle is one connected structure.
        assert_eq!(
            v.sections().len(),
            1,
            "{} is already in pieces before anything hit it",
            v.name
        );
    }
}

/// **A crash tears the back off a lorry**, rather than subtracting from
/// one global health bar. That is the property the whole part model exists
/// for.
#[test]
fn breaking_the_structure_can_break_the_vehicle_in_two() {
    let artic = Vehicle::artic();
    let (len, _) = artic.footprint();

    // A hole in the floor is a hole in the floor. Five tiles across means
    // losing one disconnects nothing, and a lorry does not come in half
    // for it.
    let holed = artic.destroy_frame(9, 3);
    assert_eq!(holed.len(), 1, "one lost tile should not sever a lorry");

    // Take out a whole cross-section — which is what a serious impact
    // does — and the trailer parts company with the tractor.
    let shear: Vec<(i32, i32)> = (0..7).map(|y| (8, y)).collect();
    let pieces = artic.destroy_frames(&shear);
    assert_eq!(
        pieces.len(),
        2,
        "shearing a lorry across should leave a tractor and a trailer, \
         got {} piece(s)",
        pieces.len()
    );

    // Biggest first, and between them they are the lorry less what was
    // destroyed.
    assert!(pieces[0].footprint().0 >= pieces[1].footprint().0);
    let total: i32 = pieces.iter().map(|p| p.footprint().0).sum();
    assert!(
        total < len,
        "the pieces cannot add up to more than the whole"
    );

    // **The half with the engine and the controls is the half that can be
    // driven away.** The other is wreckage on the carriageway.
    let drivable: Vec<&Vehicle> = pieces.iter().filter(|p| p.drivable()).collect();
    assert_eq!(
        drivable.len(),
        1,
        "exactly one half of a sheared artic should still drive"
    );
    assert!(
        drivable[0].kinds().any(|p| matches!(p, Part::Engine(_))),
        "the half that drives should be the half with the engine in it"
    );
}

/// **Where the weight sits is a fact about where the cargo is**, not a
/// single number hung off the vehicle.
#[test]
fn cargo_sits_somewhere_and_moves_the_centre_of_mass() {
    let mut v = Vehicle::van();
    let (before_x, _) = v.centre_of_mass();

    // Put a tonne right at the back.
    let (len, _) = v.footprint();
    v.parts.push((Part::CargoBay(1000), len - 1, 1));
    let (after_x, _) = v.centre_of_mass();
    assert!(
        after_x > before_x,
        "loading the tail should move the centre of mass back: \
         {before_x:.2} -> {after_x:.2}"
    );

    // Hang enough weight off the back of a short vehicle and it is no
    // longer standing on its wheels, which is a real way to make a
    // perfectly serviceable vehicle undriveable.
    let mut tail_heavy = Vehicle::van();
    for _ in 0..40 {
        tail_heavy.parts.push((Part::CargoBay(1000), 6, 1));
    }
    assert!(
        !tail_heavy.supported(),
        "a van with four tonnes hung off the tailgate should be tipping"
    );
}

/// **Top speed is where the engine runs out of push**, which is what makes
/// the width table matter: frontal area is width times height.
#[test]
fn speed_comes_from_power_against_drag() {
    let artic = Vehicle::artic();
    let van = Vehicle::van();

    // A van is lighter, smaller and less draggy, so it out-runs a lorry
    // on a fraction of the power.
    assert!(van.power_kw() < artic.power_kw());
    assert!(
        van.top_speed_kmh() < artic.top_speed_kmh() * 1.5,
        "the two should be in the same league on a motorway"
    );

    // Nothing in the fleet is absurd. Real: a laden artic tops out around
    // 120-130 and is then held to 90 by law.
    for v in fleet() {
        let top = v.top_speed_kmh();
        assert!(
            (10.0..190.0).contains(&top),
            "{} tops out at {top:.0} km/h",
            v.name
        );
    }

    // **An artic is limited by law, not by power.** An EU speed limiter
    // caps a heavy goods vehicle at 90 km/h, and this one would otherwise
    // sit well above it.
    assert!(
        artic.top_speed_kmh() > 90.0,
        "an artic should have the power to exceed its limiter"
    );
    assert!(
        (artic.cruise_kmh() - 90.0).abs() < 0.01,
        "an artic should be governed to 90, not {:.0}",
        artic.cruise_kmh()
    );

    // A bicycle has no engine, so none of that applies.
    assert_eq!(Vehicle::bicycle().cruise_kmh(), 15.0);
}

/// The capabilities are computed, so changing the parts changes them.
#[test]
fn a_bigger_engine_climbs_better_and_drinks_more() {
    let stock = Vehicle::van();
    let mut tuned = Vehicle::van();
    // Swap the engine for something twice the size.
    for p in tuned.parts.iter_mut() {
        if let Part::Engine(kw) = p.0 {
            p.0 = Part::Engine(kw * 2);
        }
    }
    assert!(tuned.power_kw() > stock.power_kw());
    assert!(
        tuned.kw_per_tonne() > stock.kw_per_tonne(),
        "twice the engine should climb better"
    );
    assert!(
        tuned.litres_per_100km() > stock.litres_per_100km(),
        "and drink more, because both read the same number"
    );
    assert!(
        tuned.top_speed_kmh() > stock.top_speed_kmh(),
        "and go faster"
    );
}
