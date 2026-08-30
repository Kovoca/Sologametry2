//! A town's ground plan.
//!
//! The rung between the world map and a building's interior: a market used
//! to be a point with a population hung off it, and the shop the economy
//! fitted out with tills and shelving stood nowhere in particular.

use scale_sim::townplan::{Lot, Plan, METRES_PER_PLOT};

#[test]
fn a_town_is_the_same_town_every_time() {
    // Spec A1.5: generated, never stored. Walk away and come back and it
    // has to be the place you left, or nothing above it can be trusted.
    let a = Plan::lay_out(20260828, 4242, 250_000.0, 48);
    let b = Plan::lay_out(20260828, 4242, 250_000.0, 48);
    assert_eq!(a.render(), b.render(), "the same town generated twice differed");

    let c = Plan::lay_out(20260828, 4243, 250_000.0, 48);
    assert_ne!(a.render(), c.render(), "two different towns came out identical");
}

#[test]
fn a_city_centre_is_dense_and_a_small_town_is_not() {
    // **Density comes from building upward, not from smaller plots.**
    // Houses alone gave a capital of sixteen million the population
    // density of an American suburb.
    let size = 44;
    let km2 = (size as f64 * METRES_PER_PLOT / 1000.0).powi(2);

    let city = Plan::lay_out(1, 100, 16_000_000.0, size);
    let village = Plan::lay_out(1, 100, 3_000.0, size);

    let city_density = city.housed() / km2;
    let village_density = village.housed() / km2;

    assert!(
        (5_000.0..25_000.0).contains(&city_density),
        "the middle of a city of sixteen million runs at {city_density:.0} a \
         square kilometre — central London is about 10,000"
    );
    assert!(
        village_density < city_density * 0.5,
        "a village of three thousand is as dense as a capital: \
         {village_density:.0} against {city_density:.0}"
    );
    assert!(
        city.count(Lot::Flats) > city.count(Lot::House) / 2,
        "a city centre of {} houses and {} blocks of flats",
        city.count(Lot::House),
        city.count(Lot::Flats)
    );
}

#[test]
fn shops_face_the_street_and_crowd_the_middle() {
    // A shop nobody walks past is not a shop, and the middle is where
    // everybody's walk crosses. Sprinkled evenly they gave one shop to
    // every five houses everywhere, which is a bazaar and not a town.
    let size = 60;
    let plan = Plan::lay_out(7, 55, 2_000_000.0, size);
    let mid = size / 2;

    let count_in = |x0: usize, x1: usize, y0: usize, y1: usize| {
        let mut shops = 0;
        let mut plots = 0;
        for y in y0..y1 {
            for x in x0..x1 {
                if plan.at(x, y) == Lot::Street {
                    continue;
                }
                plots += 1;
                if plan.at(x, y) == Lot::Shop {
                    shops += 1;
                }
            }
        }
        shops as f64 / plots.max(1) as f64
    };

    let centre = count_in(mid - 6, mid + 6, mid - 6, mid + 6);
    let edge = count_in(0, 10, 0, 10);
    assert!(
        centre > edge * 1.5,
        "shops are {:.0}% of the middle and {:.0}% of the edge — that is a \
         sprinkle, not a high street",
        centre * 100.0,
        edge * 100.0
    );

    // Every shop has a frontage.
    for y in 0..size {
        for x in 0..size {
            if plan.at(x, y) != Lot::Shop {
                continue;
            }
            let touches = [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ]
            .iter()
            .any(|&(nx, ny)| nx < size && ny < size && plan.at(nx, ny) == Lot::Street);
            assert!(touches, "a shop at {x},{y} with no street to open onto");
        }
    }
}

#[test]
fn a_lorry_is_ordinary_traffic_and_a_tank_is_not() {
    // **What you must arrange is a property of the load; whether anything
    // gets past you is a property of the road.** Folding those into one
    // scale gave every class of street the same verdict, which is the tell
    // that the road had stopped mattering.
    use scale_sim::townplan::{Permission, StreetClass::*};
    use scale_sim::vehicle::Vehicle;

    // A lorry is 2.55 m — the European legal maximum — and is nobody's
    // special case. It nearly fills its 3.65 m lane, with 55 cm either
    // side, which is real and is why lorries feel enormous beside you.
    for road in [Lane, Road, Dual, Motorway] {
        let c = road.clearance_for(Vehicle::artic().width_m);
        assert_eq!(c.permission, Permission::Ordinary, "an artic is ordinary traffic");
        assert!(!c.will_not_fit && !c.blocks_the_road, "an artic on a {}", road.name());
    }

    // A main battle tank is 3.5-3.9 m: escorted wherever it goes...
    let tank = 3.9;
    for road in [Lane, Road, Dual, Motorway] {
        assert_eq!(
            road.clearance_for(tank).permission,
            Permission::Escorted,
            "a tank on a {} is not ordinary traffic",
            road.name()
        );
    }
    // ...but the *consequence* depends entirely on the road it is on.
    assert!(
        Lane.clearance_for(tank).blocks_the_road,
        "a tank down a village street and the traffic still flowing"
    );
    assert!(
        !Motorway.clearance_for(tank).blocks_the_road,
        "three lanes each way and a tank stops all of them"
    );

    // A large grid transformer is 3.5-4.5 m, so it needs an order that
    // takes weeks. **This is a real reason a substation stays dark**, on
    // top of the twelve to eighteen months to build the thing.
    assert_eq!(
        Motorway.clearance_for(4.5).permission,
        Permission::SpecialOrder
    );

    // And some things are not a paperwork problem at all.
    assert!(
        Lane.clearance_for(12.0).will_not_fit,
        "twelve metres down a five-and-a-half metre lane"
    );
    assert!(!Lane.clearance_for(4.5).will_not_fit, "4.5 m fits on 5.5 m of surface");
}

#[test]
fn the_tiles_agree_with_the_real_width() {
    // A vehicle's width is the one measurement not read off the tile grid,
    // because a metre is too coarse: an artic is 2.55 m and rounds up to
    // three tiles, and 3.0 m would put an ordinary lorry over the 2.9 m
    // line where the police want notice. This holds the two together so a
    // layout cannot quietly drift from the real figure.
    use scale_sim::vehicle::Vehicle;
    for v in [
        Vehicle::bicycle(),
        Vehicle::van(),
        Vehicle::box_truck(),
        Vehicle::artic(),
        Vehicle::reefer(),
    ] {
        let tiles = v.footprint().1 as f64;
        assert_eq!(
            v.width_m.ceil(),
            tiles,
            "{} is {:.2} m but takes up {tiles} tiles",
            v.name,
            v.width_m
        );
    }
}
