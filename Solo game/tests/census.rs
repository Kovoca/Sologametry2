//! **Who is counted, and against what.**
//!
//! A denominator error produces a plausible percentage, which is what
//! makes it the hardest mistake here to see. It has already happened
//! twice — a workforce that counted only the works, and a retail
//! benchmark that was really a whole sector — so these guard the
//! boundaries rather than the arithmetic.

use scale_sim::census::{
    retail_share_of_fte, whole_distribution_sector, Base, Census, DISTRIBUTION,
};

fn a_nation() -> Census {
    Census::of_a_nation(80_000_000.0)
}

/// **Four counts, and they are not each other.** One person may hold two
/// jobs; one job may be half a week.
#[test]
fn jobs_and_people_and_hours_are_different_numbers() {
    let c = a_nation();
    assert!(c.jobs > c.employed_people, "nobody held a second job");
    assert!(
        c.full_time_equivalents < c.employed_people,
        "everybody worked a full week"
    );
    assert!(c.employed_people < c.labour_force, "nobody was out of work");
    assert!(c.labour_force < c.working_age, "every adult was in the labour force");
    assert!(c.working_age < c.population, "nobody was a child or retired");
}

/// **A share carries what it is a share of**, and two on different bases
/// cannot be compared at all.
#[test]
fn a_share_knows_its_denominator() {
    let c = a_nation();
    let n = 3_000_000.0;
    let of_work = c.share(n, Base::EmployedPeople);
    let of_fte = c.share(n, Base::FullTimeEquivalents);

    assert_eq!(of_work.of, Base::EmployedPeople);
    assert_ne!(of_work.value, of_fte.value);
    assert!(
        of_work.compare(&of_fte).is_none(),
        "two shares on different bases compared as though they were alike"
    );
    assert!(of_work.compare(&c.share(1.0, Base::EmployedPeople)).is_some());
}

/// **The same headcount is a bigger share of hours than of heads**,
/// because a full-time equivalent is scarcer than a person.
#[test]
fn the_base_changes_the_answer_and_that_is_the_point() {
    let c = a_nation();
    let n = 3_000_000.0;
    assert!(
        c.share(n, Base::FullTimeEquivalents).percent()
            > c.share(n, Base::EmployedPeople).percent()
    );
    assert!(
        c.share(n, Base::EmployedPeople).percent() > c.share(n, Base::Population).percent()
    );
}

// =====================================================================
// the boundary the benchmark was drawn at
// =====================================================================

/// **14% is not shops.**
///
/// "Wholesale and retail trade; repair of motor vehicles" is one
/// statistical section, and it is the figure everybody quotes. Shops are
/// a little under two thirds of it — so calibrating a shop model against
/// the section total would have put half as many people again behind a
/// counter as belong there, and done it while looking right.
#[test]
fn the_quoted_figure_is_a_whole_sector_and_not_the_shops() {
    let whole = whole_distribution_sector();
    let shops = DISTRIBUTION[0].of_employed;

    assert!(
        (0.13..=0.16).contains(&whole),
        "the whole distribution sector came out at {:.1}%",
        whole * 100.0
    );
    assert!(
        (0.08..=0.10).contains(&shops),
        "shops alone came out at {:.1}%",
        shops * 100.0
    );
    assert!(
        shops < whole * 0.7,
        "shops were most of the sector, so the boundary is doing no work"
    );

    // The other two are real sectors this model does not have at all,
    // and they are not a retail shortfall.
    let wholesale = DISTRIBUTION[1].of_employed;
    let motor = DISTRIBUTION[2].of_employed;
    assert!(wholesale > 0.03 && motor > 0.01);
    assert!((whole - shops - wholesale - motor).abs() < 1e-9);
}

/// **And each one says what it leaves out**, so the boundary travels
/// with the number.
#[test]
fn every_benchmark_names_what_it_excludes() {
    for b in DISTRIBUTION {
        assert!(!b.excludes.is_empty(), "{} did not say what it leaves out", b.what);
        assert!(b.of_employed > 0.0 && b.of_employed < 0.2);
    }
    assert!(DISTRIBUTION[0].excludes.contains("wholesale"));
    assert!(DISTRIBUTION[1].excludes.contains("household"));
}

/// **Retail's share of hours is smaller than its share of heads**, and a
/// model whose staffing comes out of labour-hours should be aiming at
/// the smaller one. That is a second boundary error waiting behind the
/// first.
#[test]
fn measuring_retail_in_hours_gives_a_smaller_target_than_in_heads() {
    let heads = DISTRIBUTION[0].of_employed;
    let hours = retail_share_of_fte();
    assert!(hours < heads, "part-time work made no difference to the target");
    assert!(
        (0.055..=0.075).contains(&hours),
        "retail in hours came out at {:.1}%",
        hours * 100.0
    );
    // About 60% part-time at roughly half a week is a third off.
    assert!(hours > heads * 0.6 && hours < heads * 0.85);
}

/// **The gap is not all one thing.**
///
/// The missing employment used to be attributed wholly to consumer goods
/// nobody buys. Against the corrected boundaries, the shops' own
/// shortfall is under half of it: wholesale and the motor trade are
/// whole sectors this model does not have, and they were never retail's
/// to make up.
#[test]
fn the_shortfall_splits_into_more_than_one_missing_thing() {
    let model_retail = 0.015; // measured by `cargo run --bin jobs`
    let retail_gap = retail_share_of_fte() - model_retail;
    let absent_sectors = DISTRIBUTION[1].of_employed + DISTRIBUTION[2].of_employed;

    assert!(retail_gap > 0.0, "the shops are not short at all");
    assert!(
        absent_sectors > retail_gap * 0.8,
        "the sectors that do not exist are a rounding error beside the retail gap"
    );
    // And the old single-number story overstated the retail half by more
    // than double.
    let old_story = 0.141 - model_retail;
    assert!(old_story > retail_gap * 2.0);
}
