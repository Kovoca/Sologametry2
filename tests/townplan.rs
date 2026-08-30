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
