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
    assert_eq!(
        a.render(),
        b.render(),
        "the same town generated twice differed"
    );

    let c = Plan::lay_out(20260828, 4243, 250_000.0, 48);
    assert_ne!(
        a.render(),
        c.render(),
        "two different towns came out identical"
    );
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

    // **Every store has a frontage — the store, not every plot of it.**
    //
    // A big store is a block: three plots of frontage and two of depth.
    // The back half is the sales floor, the backroom and the dock, and
    // none of that wants a window. What has to front a street is the
    // front of the building, so walk to it first.
    for y in 0..size {
        for x in 0..size {
            if plan.at(x, y) != Lot::Shop {
                continue;
            }
            if y > 0 && plan.at(x, y - 1) == Lot::Shop {
                continue; // the back of a store, behind its own frontage
            }
            let touches = [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ]
            .iter()
            .any(|&(nx, ny)| nx < size && ny < size && plan.at(nx, ny) == Lot::Street);
            assert!(
                touches,
                "a store fronting at {x},{y} with no street to open onto"
            );
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

    // **Two questions, and they take two different widths.**
    //
    // The law measures a vehicle over its *body* — 2.55 m is the European
    // legal maximum and mirrors are excluded — so an artic is ordinary
    // traffic and nobody's special case. What physically clips is the
    // width over the mirrors, and that is what decides whether anything
    // can get past you.
    let artic = Vehicle::artic();
    assert!(
        artic.body_width_m() < artic.width_m,
        "an artic should be wider over its mirrors than over its body"
    );
    for road in [Lane, Road, Dual, Motorway] {
        assert_eq!(
            road.clearance_for(artic.body_width_m()).permission,
            Permission::Ordinary,
            "an artic is ordinary traffic on a {}",
            road.name()
        );
        assert!(
            !road.clearance_for(artic.width_m).will_not_fit,
            "an artic does not fit on a {}",
            road.name()
        );
    }
    // **But it does block a village lane**, and that is true: 2.8 m over
    // the mirrors on 5.5 m of surface leaves too little to pass, which is
    // why you wait in a gateway for a lorry and never do on a dual
    // carriageway.
    assert!(
        Lane.clearance_for(artic.width_m).blocks_the_road,
        "an artic down a village lane and the traffic still flowing"
    );
    for road in [Dual, Motorway] {
        assert!(
            !road.clearance_for(artic.width_m).blocks_the_road,
            "an artic should not block a {}",
            road.name()
        );
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
    assert!(
        !Lane.clearance_for(4.5).will_not_fit,
        "4.5 m fits on 5.5 m of surface"
    );
}

#[test]
fn the_tiles_agree_with_the_real_width() {
    // **Width is read off the tiles, but not linearly**, and that is the
    // settled contract. A metre to the tile is too coarse to carry both
    // jobs at once: two squares is the honest width of a car and cannot
    // hold two seats, two doors and the bodywork round them. So the grid
    // is spent on interior resolution and the real width comes from a
    // compression table.
    //
    // What this holds together is that the two never drift apart: every
    // vehicle's width must be exactly what its tile count says it is.
    use scale_sim::vehicle::{modelled_width_m, Vehicle};
    for v in [
        Vehicle::bicycle(),
        Vehicle::van(),
        Vehicle::box_truck(),
        Vehicle::artic(),
    ] {
        let tiles = v.footprint().1;
        assert_eq!(
            v.width_m,
            modelled_width_m(tiles),
            "{} is {:.2} m across {tiles} tiles, which is not what the table says",
            v.name,
            v.width_m
        );
        // And a grid wide enough to lay a cab out in is still a vehicle
        // narrow enough for the road it drives on.
        assert!(
            v.width_m < 2.9,
            "{} would need an abnormal-load notice at {:.2} m",
            v.name,
            v.width_m
        );
    }
    // A reefer is the exception that proves the table is a default rather
    // than a law: insulation costs real width, and a refrigerated body is
    // allowed 2.6 m where a dry one is held to 2.55.
    let reefer = Vehicle::reefer();
    assert!(reefer.width_m < modelled_width_m(reefer.footprint().1));
}

#[test]
fn a_village_is_a_street_and_a_city_is_a_grid() {
    // **What a place looks like turns on whether it was surveyed at once
    // or simply grew.** A grid is what an authority lays out before
    // anybody builds — Roman colonies, the Laws of the Indies, the US Land
    // Ordinance, Manhattan's Commissioners' Plan. An irregular town is
    // what you get when the routes came first. Size correlates because a
    // large city has almost certainly been planned or replanned: you
    // cannot run water, sewers, trams and freight through a tangle.
    use scale_sim::townplan::{Pattern, Plan};
    use scale_sim::world::Biome;

    let of = |pop: f64| Plan::lay_out_on(20260828, 4242, pop, 40, Biome::Grassland);
    let village = of(900.0);
    let town = of(18_000.0);
    let city = of(400_000.0);

    assert_eq!(village.pattern, Pattern::Linear);
    assert_eq!(town.pattern, Pattern::Organic);
    assert_eq!(city.pattern, Pattern::Grid);

    // How many full-length through-routes there are is the whole
    // difference: a village has one, a grid has one every few plots.
    let through = |p: &Plan| {
        (0..p.width).filter(|&x| p.col_class(x).is_some()).count()
            + (0..p.height).filter(|&y| p.row_class(y).is_some()).count()
    };
    assert_eq!(through(&village), 1, "a village with more than one street");
    assert!(
        through(&town) < through(&city) / 2,
        "a town that grew has {} through-routes against a grid's {}",
        through(&town),
        through(&city)
    );

    // **A grid is anisotropic.** Manhattan's blocks are 80 m by 274,
    // Chicago's about 100 by 200: a block wants a long side of frontage
    // and a short walk across. Square blocks are the giveaway of a grid
    // nobody measured.
    let cols = (0..city.width)
        .filter(|&x| city.col_class(x).is_some())
        .count();
    let rows = (0..city.height)
        .filter(|&y| city.row_class(y).is_some())
        .count();
    assert!(
        cols as f64 > rows as f64 * 1.8 || rows as f64 > cols as f64 * 1.8,
        "{cols} streets one way and {rows} the other is a square grid"
    );
}

#[test]
fn nobody_builds_upward_where_land_is_cheap() {
    // Flats need a central site *and* a place big enough for land to be
    // worth anything. Judging on centrality alone put twenty-one blocks of
    // flats in a village of nine hundred people. Real apartment blocks are
    // all but absent below about 20,000 and dominant over half a million.
    use scale_sim::townplan::{Lot, Plan};
    use scale_sim::world::Biome;
    let of = |pop: f64| Plan::lay_out_on(20260828, 4242, pop, 40, Biome::Grassland);

    let village = of(900.0);
    let flats = village.count(Lot::Flats) as f64;
    let houses = village.count(Lot::House) as f64;
    assert!(
        flats < houses * 0.05,
        "{flats} blocks of flats against {houses} houses in a village"
    );

    let city = of(2_000_000.0);
    assert!(
        city.count(Lot::Flats) > city.count(Lot::House),
        "a city of two million housed mostly in detached houses"
    );
}

#[test]
fn a_bigger_place_is_denser_and_not_just_wider() {
    // A fixed people-per-km² meant the radius was the only thing that
    // changed with population, so a village and a megacity had identical
    // central density. Real mean densities: Los Angeles 3,200/km², London
    // 5,700, New York 11,000, Paris 20,000 — they are not the same number.
    use scale_sim::townplan::{Plan, METRES_PER_PLOT};
    use scale_sim::world::Biome;
    let density = |pop: f64| {
        let p = Plan::lay_out_on(20260828, 4242, pop, 32, Biome::Grassland);
        p.housed() / (32.0 * METRES_PER_PLOT / 1000.0).powi(2)
    };
    let (village, town, city) = (density(900.0), density(18_000.0), density(400_000.0));
    assert!(
        village < town && town < city,
        "village {village:.0}, town {town:.0}, city {city:.0} per km² is not a gradient"
    );
    // Against the real figures: a town in the low thousands, a city centre
    // in the high thousands.
    assert!(
        (1_500.0..6_000.0).contains(&town),
        "a town of 18,000 at {town:.0} per km²"
    );
    assert!(
        (4_000.0..15_000.0).contains(&city),
        "the middle of a city of 400,000 at {city:.0} per km²"
    );
}
