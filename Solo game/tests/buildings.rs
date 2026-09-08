//! What a building is, what it is for, and what it costs to put up.
//!
//! Construction and use are separate axes. A supermarket, a distribution
//! warehouse and a sports hall are the *same shed*.

use scale_sim::building::{Structure, Use};

/// **The same shed serves several trades**, which is the whole reason the
/// axes are split. Collapsed into one enum, a "warehouse" could only ever
/// be a warehouse and every new use meant a new construction type.
#[test]
fn construction_and_use_are_different_questions() {
    let shed = Structure::Warehouse;
    let same: Vec<Use> = Use::ALL
        .iter()
        .copied()
        .filter(|u| u.construction() == shed)
        .collect();
    assert!(
        same.len() >= 4,
        "only {} uses share the portal-frame shed: {:?}",
        same.len(),
        same
    );
    assert!(same.contains(&Use::Supermarket));
    assert!(same.contains(&Use::SportsHall));

    // And the high street is terraces with trade in the bottom of them,
    // which is why town centres have people living over the shops.
    assert_eq!(Use::CornerShop.construction(), Structure::Terrace);
    assert_eq!(Use::Pub.construction(), Structure::Terrace);

    // Every use must name a construction, or its materials are unknowable.
    for u in Use::ALL {
        let bill = u.materials(10.0);
        assert!(bill.total() > 0.0, "{} costs nothing to build", u.name());
    }
}

/// **Real space standards are a fixed part plus so much per occupant**,
/// because a school needs a hall and a kitchen whether it has 200 children
/// or 400.
#[test]
fn floor_areas_are_the_real_standards() {
    // Building Bulletin 103: 350 m2 + 4.1 m2 a pupil.
    let small = Use::School.floor_area_m2(200.0);
    let large = Use::School.floor_area_m2(400.0);
    assert!((small - (350.0 + 4.1 * 200.0)).abs() < 1.0);
    // Doubling the roll does not double the school — that is what the
    // fixed part means, and it is why small schools cost more per pupil.
    assert!(large < small * 2.0);
    assert!(
        small / 200.0 > large / 400.0,
        "a small school should cost more floor area per pupil"
    );

    // 47.5 m2 an inpatient bed, all departments counted. A hospital is
    // the largest building most towns have and this is why.
    assert!((Use::Hospital.floor_area_m2(100.0) - 4_750.0).abs() < 1.0);
    assert!(
        Use::Hospital.floor_area_m2(300.0) > Use::Supermarket.floor_area_m2(0.0) * 4.0,
        "a district hospital should dwarf a superstore"
    );

    // BCO 2024: 10 m2 a work setting.
    assert!((Use::Office.floor_area_m2(50.0) - 500.0).abs() < 1.0);

    // A convenience store against a superstore: real 250-1,000 against
    // 2,800-4,650, so better than tenfold.
    assert!(Use::Supermarket.floor_area_m2(0.0) > Use::CornerShop.floor_area_m2(0.0) * 10.0);
}

/// **Where a building is allowed to stand** follows from what it needs.
#[test]
fn what_a_building_needs_decides_where_it_can_go() {
    // A shop that cannot be seen is not a shop.
    assert!(Use::CornerShop.wants_frontage());
    assert!(Use::Pub.wants_frontage());
    assert!(!Use::Warehouse.wants_frontage());
    assert!(!Use::Factory.wants_frontage());

    // And an artic has to reach the door, which is most of why these end
    // up on the edge of town on cheap land.
    assert!(Use::Warehouse.wants_lorry_access());
    assert!(Use::Supermarket.wants_lorry_access());
    assert!(!Use::Office.wants_lorry_access());

    // **A supermarket wants both**, and that tension is exactly why they
    // are hard to fit into an old town centre and why they ended up on
    // bypasses instead.
    assert!(Use::Supermarket.wants_frontage() && Use::Supermarket.wants_lorry_access());

    // The cold chain, and the loads a grid will black out a district to
    // keep.
    assert!(Use::ColdStore.needs_cold());
    assert!(Use::Hospital.critical_supply());
    assert!(Use::FireStation.critical_supply());
    assert!(!Use::Cinema.critical_supply());
}

/// **A shelter is small and enormously dear**, which is the whole
/// economics of civil defence: you can afford to harden a cupboard and
/// not a town.
#[test]
fn hardened_buildings_are_small_because_they_are_dear() {
    // Real shelter standards run about a square metre a head — far
    // tighter than anywhere anybody lives.
    let for_fifty = Use::Shelter.floor_area_m2(50.0);
    assert!(for_fifty < Use::Dwelling.floor_area_m2(0.0) * 1.2);

    // And per square metre it costs many times a house.
    let shelter_per_m2 = Use::Shelter.construction().materials().total();
    let house_per_m2 = Use::Dwelling.construction().materials().total();
    assert!(
        shelter_per_m2 > house_per_m2 * 5.0,
        "hardening came out at only {:.1}x a house",
        shelter_per_m2 / house_per_m2
    );

    // Housing fifty people properly costs far more than sheltering them,
    // even so — which is why shelters exist and hardened housing does not.
    assert!(
        Use::Dwelling.materials(0.0).total() * 20.0 > Use::Shelter.materials(50.0).total(),
        "twenty houses should still outweigh one shelter for fifty"
    );
}

/// **A village has a pub; a university needs a city.**
///
/// Nobody decides that — it falls out of threshold populations, which is
/// how it works in reality. Real UK counts against 67 million people:
/// ~46,000 pubs, ~20,800 primary schools, ~800 cinemas, ~165 universities.
#[test]
fn what_a_place_contains_follows_from_how_big_it_is() {
    let has = |pop: f64, u: Use| {
        Use::amenities_for(pop)
            .iter()
            .any(|&(w, n)| w == u && n > 0)
    };

    // A hamlet supports nothing at all, and that is right: you drive to
    // the next village for a pint.
    assert!(Use::amenities_for(300.0).is_empty());

    // A village: a pub and a shop, and not much else.
    assert!(has(1_500.0, Use::Pub));
    assert!(has(1_500.0, Use::CornerShop));
    assert!(!has(1_500.0, Use::Supermarket));
    assert!(!has(1_500.0, Use::Hospital));

    // A market town adds the things a village sends you elsewhere for.
    assert!(has(20_000.0, Use::Supermarket));
    assert!(has(20_000.0, Use::Chemist));
    assert!(has(20_000.0, Use::Library));
    assert!(!has(20_000.0, Use::University));
    assert!(!has(20_000.0, Use::Cinema));

    // A city has everything, including the two that need a region behind
    // them.
    assert!(has(500_000.0, Use::Cinema));
    assert!(has(500_000.0, Use::University));
    assert!(has(500_000.0, Use::Hospital));

    // And the ladder is monotonic: a bigger place never loses an amenity.
    let town = Use::amenities_for(50_000.0);
    let city = Use::amenities_for(500_000.0);
    for (u, _) in &town {
        assert!(
            city.iter().any(|(w, _)| w == u),
            "a city lost the {} a town had",
            u.name()
        );
    }

    // Commonest first, and it should be the pub or the corner shop —
    // which is exactly what a real high street looks like.
    let (most, n) = city[0];
    assert!(
        matches!(most, Use::Pub | Use::CornerShop),
        "the commonest building in a city came out as {} ({n})",
        most.name()
    );
}
