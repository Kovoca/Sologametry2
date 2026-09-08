//! **What sizes a world actually has in it.**
//!
//! The generator used to give every stored settlement a share of the
//! whole urban population by weight, which produced a planet whose median
//! settlement was a city of 360,000, with two small towns on it and no
//! villages at all.

use scale_sim::locality::{villages_in, KM2_PER_SETTLED_PLACE};
use scale_sim::polity::Polities;
use scale_sim::settlement::{population_at_rank, Band, Settlements};
use scale_sim::world::World;

fn a_world() -> (World, Polities, Settlements) {
    let world = World::generate(384, 216, 20260828);
    let pol = Polities::partition(&world, 24);
    let set = Settlements::place(&world, &pol, 6000);
    (world, pol, set)
}

// =====================================================================
// the stored tier: cities and towns
// =====================================================================

/// **The curve is Earth's, at Earth's ranks.**
///
/// No single power law fits: strict Zipf off Tokyo's 37 million would put
/// the hundredth city at 370,000 when it is nearer six million. So the
/// real figures are the anchors and the model interpolates between them.
#[test]
fn the_rank_size_curve_matches_the_real_one() {
    assert!((population_at_rank(1) - 37.0e6).abs() < 1.0);
    assert!((population_at_rank(100) - 6.0e6).abs() < 1.0);
    assert!((population_at_rank(1000) - 800.0e3).abs() < 1.0);
    assert!((population_at_rank(2000) - 400.0e3).abs() < 1.0);

    // Monotone, and it keeps falling past the last anchor rather than
    // stopping dead.
    for r in [1usize, 7, 60, 300, 900, 3000, 9000, 40_000, 200_000] {
        assert!(
            population_at_rank(r) > population_at_rank(r * 2),
            "rank {r} was not larger than rank {}",
            r * 2
        );
    }
    assert!(population_at_rank(200_000) > 0.0);
}

/// **It is far steeper than what it replaced.** The old model's implied
/// exponent was about 0.51 against a real one near 1 — a planet of
/// identical mid-sized cities.
#[test]
fn the_curve_is_steep_enough_to_be_a_real_one() {
    let implied = |a: usize, b: usize| {
        (population_at_rank(a) / population_at_rank(b)).ln() / (b as f64 / a as f64).ln()
    };
    let mid = implied(100, 2000);
    assert!(
        mid > 0.85 && mid < 1.15,
        "the middle of the curve came out at an exponent of {mid:.2}"
    );
}

/// **A generated world comes out with the right shape.**
#[test]
fn a_generated_world_has_the_sizes_earth_has() {
    let (_, _, set) = a_world();
    assert!(set.list.len() > 1000, "too few settlements to say anything");

    let mut pops: Vec<u32> = set.list.iter().map(|s| s.population).collect();
    pops.sort_unstable_by(|a, b| b.cmp(a));

    // The largest is a real largest city, not a continent's worth.
    assert!(
        pops[0] > 20_000_000 && pops[0] <= 40_000_000,
        "largest {}",
        pops[0]
    );

    // **The median is wherever the curve says the middle rank is**, and
    // that is the invariant worth asserting: the absolute figure depends
    // on how many sites the land will support, which is exactly what
    // should differ between worlds. Asserting a number instead only
    // tested how big this particular world happened to be.
    let median = pops[pops.len() / 2] as f64;
    let expected = population_at_rank(pops.len() / 2);
    assert!(
        (median - expected).abs() < expected * 0.05,
        "median {median:.0} against the curve's {expected:.0} at that rank"
    );

    // And every place falls on the curve, not merely the middle one.
    for r in [1usize, pops.len() / 4, pops.len() / 2, pops.len() - 1] {
        let want = population_at_rank(r + 1);
        assert!(
            (pops[r] as f64 - want).abs() < want * 0.05,
            "rank {} holds {} against the curve's {want:.0}",
            r + 1,
            pops[r]
        );
    }
}

/// **The stored places are not the whole world.** Most people do not
/// live in the largest few thousand settlements, and the model has to
/// leave room for them.
#[test]
fn most_people_do_not_live_in_the_stored_settlements() {
    let (_, _, set) = a_world();
    let stored = set.stored_population();
    let country = set.countryside_population();
    assert!(country > 0.0, "the countryside held nobody at all");
    assert!(
        country > stored * 0.5,
        "the stored towns held {stored:.0} and the whole countryside {country:.0}"
    );
}

/// **Size is not status.** A city in Britain is a rank granted by
/// charter — St Davids has 1,600 people and is one; Reading has 175,000
/// and is not — so the two are separate questions here.
#[test]
fn how_big_it_is_and_what_it_is_are_different_questions() {
    assert_eq!(Band::of(60), Band::Hamlet);
    assert_eq!(Band::of(400), Band::Village);
    assert_eq!(Band::of(1_800), Band::LargeVillage);
    assert_eq!(Band::of(6_000), Band::SmallTown);
    assert_eq!(Band::of(30_000), Band::Town);
    assert_eq!(Band::of(70_000), Band::LargeTown);
    assert_eq!(Band::of(250_000), Band::City);

    let (_, _, set) = a_world();
    // **Status lags size**, which is the whole point of separating them:
    // plenty of places here are cities by any headcount and are still
    // filed as towns, exactly as Reading is. Asserting it through
    // capitals would not work - with two dozen nations on a planet every
    // capital really is a city, and that is correct rather than a fault.
    let town_sized_city = set
        .list
        .iter()
        .filter(|s| s.kind == scale_sim::settlement::Kind::Town)
        .any(|s| s.band() == Band::City);
    assert!(
        town_sized_city,
        "nothing was a city by size and a town by status"
    );
}

// =====================================================================
// the countryside: generated, never stored
// =====================================================================

/// **There are villages now**, and they are not in any list.
#[test]
fn the_countryside_is_full_of_places_nobody_wrote_down() {
    let v = villages_in(4242, 900, 6_000.0);
    assert!(!v.is_empty(), "six thousand country people lived nowhere");

    // About one place per thirteen square kilometres, which is what
    // long-settled farming country carries.
    let expected = (16.384 * 16.384 / KM2_PER_SETTLED_PLACE).round() as usize;
    assert!(
        v.len() >= expected / 2 && v.len() <= expected + 2,
        "{} places in a region cell against about {expected}",
        v.len()
    );

    // **Mostly hamlets and villages**, with one bigger place — which is
    // what a parish looks like.
    let bands: Vec<Band> = v.iter().map(|x| Band::of(x.population)).collect();
    assert!(bands.iter().filter(|b| **b <= Band::Village).count() > v.len() / 2);
    assert!(
        bands.iter().any(|b| *b >= Band::LargeVillage),
        "no village had a church"
    );
    assert!(
        !bands.iter().any(|b| *b >= Band::Town),
        "a town appeared in the countryside"
    );
}

/// **Generated, never stored.** Walking away and coming back finds the
/// same hamlets in the same fields.
#[test]
fn the_same_cell_produces_the_same_villages() {
    let a = villages_in(7, 1234, 4_000.0);
    let b = villages_in(7, 1234, 4_000.0);
    assert_eq!(a, b);
    // A different cell is a different parish.
    assert_ne!(villages_in(7, 1235, 4_000.0), a);
    // And a different world is a different countryside.
    assert_ne!(villages_in(8, 1234, 4_000.0), a);
}

/// **Empty country is empty.** Nobody is placed where nobody lives, and
/// there is no minimum village.
#[test]
fn nothing_is_placed_where_nobody_lives() {
    assert!(villages_in(1, 5, 0.0).is_empty());
    assert!(villages_in(1, 5, 3.0).is_empty());
    // And what is placed adds up to roughly what was given.
    let people = 9_000.0;
    let v = villages_in(1, 5, people);
    let held: f64 = v.iter().map(|x| x.population as f64).sum();
    assert!(
        (held - people).abs() < people * 0.15,
        "nine thousand country people came out as {held:.0}"
    );
}

/// **A thinly peopled cell has hamlets; a well peopled one has a
/// village.** The land decides, not a table.
#[test]
fn how_many_people_there_are_decides_what_the_places_are() {
    let thin = villages_in(3, 77, 300.0);
    let full = villages_in(3, 77, 30_000.0);
    let biggest =
        |v: &[scale_sim::locality::Village]| v.iter().map(|x| x.population).max().unwrap_or(0);
    assert!(biggest(&full) > biggest(&thin) * 10);
    assert!(Band::of(biggest(&thin)) <= Band::Village);
    assert!(Band::of(biggest(&full)) >= Band::SmallTown);
}
