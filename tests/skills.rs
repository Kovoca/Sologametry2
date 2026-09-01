//! Skills, the way DF and CDDA do them.
//!
//! Both arrive at the same model from different ends: a person holds
//! *many* skills at *levels*, work names a level it wants, and the level
//! moves with practice. DF grades Novice to Legendary and lets skill
//! decide speed and quality; CDDA runs 0-10, refuses a recipe above your
//! level, and makes each level cost more than the last.
//!
//! **A qualification is a licence and a skill is competence**, and they
//! come apart in both directions.

use scale_sim::econ::{Doctrine, DAYS_PER_YEAR};
use scale_sim::network::Network;
use scale_sim::person::{self, Person, Skill, Trade};
use scale_sim::polity::Polities;
use scale_sim::populace::Populace;
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

fn a_nation() -> Region {
    let world = World::generate(384, 216, 20260828);
    let pol = Polities::partition(&world, 30);
    let set = Settlements::place(&world, &pol, 5000);
    let net = Network::build(&world, &set, 1000);
    let id = pol.ranked()[2].0;
    Region::extract(&world, &pol, &set, &net, id, 5, Doctrine::Prudent).expect("a nation")
}

/// **Each level costs more than the last**, which is CDDA's shape and the
/// real one — anchored on the familiar figure of about ten thousand hours
/// to mastery.
#[test]
fn a_level_costs_more_than_the_one_below() {
    let mut last = 0.0;
    for l in 1..=10u8 {
        let cost = Skill::days_to_reach(l) - Skill::days_to_reach(l - 1);
        assert!(
            cost > last,
            "level {l} costs {cost:.0} days against {last:.0} for the one below"
        );
        last = cost;
    }
    // Ten thousand hours is five or six years of doing nothing else. At a
    // working year of about 220 days actually at the trade, the top of
    // the scale should be a career rather than a season.
    let years = Skill::days_to_reach(10) / 220.0;
    assert!(
        (4.0..12.0).contains(&years),
        "mastery takes {years:.1} years of practice"
    );
    // And the bottom of it should be quick, because it is.
    assert!(Skill::days_to_reach(1) < 30.0);

    // Levels read back off the practice that bought them.
    for l in 0..=10u8 {
        assert_eq!(Skill::level_from_days(Skill::days_to_reach(l)), l);
    }
}

/// **Use raises a skill and disuse lowers it.**
#[test]
fn practice_makes_a_tradesman_and_neglect_unmakes_one() {
    let mut p = Person::new("Nell", Trade::Electrician, 0, 100.0);
    p.aptitude = 0.9;
    p.diligence = 0.8;
    assert_eq!(p.competence(), 0, "she starts knowing nothing");

    for _ in 0..600 {
        p.practise(true);
    }
    let trained = p.competence();
    assert!(
        trained >= 5,
        "six hundred days at the trade and she is only {} ({})",
        trained,
        Skill::grade(trained)
    );

    // **Skill fades slowly.** A trade you have not worked for years is one
    // you are rusty at, not one you have forgotten.
    for _ in 0..(365 * 3) {
        p.practise(false);
    }
    let rusty = p.competence();
    assert!(rusty < trained, "three years away and no rust at all");
    assert!(
        rusty >= trained.saturating_sub(3),
        "three years away took her from {trained} to {rusty}, which is amnesia \
         rather than rust"
    );
}

/// **Aptitude is the ceiling and practice is the climb.**
///
/// The same axis that decides whether a course is out of reach decides
/// where practice stops paying — so legendary is rare because the ability
/// to get there is rare, not because the hours are unavailable.
#[test]
fn how_far_somebody_gets_is_not_only_how_long_they_try() {
    let mut dull = Person::new("Dull", Trade::Builder, 0, 0.0);
    dull.aptitude = 0.1;
    dull.diligence = 1.0;
    let mut able = Person::new("Able", Trade::Builder, 0, 0.0);
    able.aptitude = 0.95;
    able.diligence = 1.0;

    for _ in 0..4000 {
        dull.practise(true);
        able.practise(true);
    }
    assert!(
        able.competence() > dull.competence(),
        "a lifetime each and the same result: {} against {}",
        able.competence(),
        dull.competence()
    );
    assert_eq!(dull.competence(), dull.ceiling(), "he should reach his ceiling");
    assert!(dull.ceiling() < able.ceiling());
}

/// **A deep trade rewards depth and a shallow one does not.** Real
/// earnings profiles are steep in the professions and flat in low-skill
/// work: a doctor of fifty earns far more than one of thirty, and a shop
/// worker of fifty does not.
#[test]
fn experience_pays_more_in_a_trade_that_has_room_for_it() {
    let top = |t: Trade| person::skill_premium(10, t.wants_level());
    assert!(top(Trade::Doctor) > 1.0);
    assert!(top(Trade::Electrician) > 1.0);
    // Nobody is worth twice the going rate for being very good at a till.
    assert!(
        top(Trade::Shopworker) < 1.5,
        "a legendary shop worker came out at {:.2}x",
        top(Trade::Shopworker)
    );
    // At exactly what the work wants, you are worth exactly the rate.
    for t in [Trade::Doctor, Trade::Nurse, Trade::Labourer] {
        assert_eq!(person::skill_premium(t.wants_level(), t.wants_level()), 1.0);
    }
    // **And below it, it shows in the pay packet** — but never to nothing,
    // because somebody is still doing the work.
    assert!(person::skill_premium(0, 7) < 0.6);
    assert!(person::skill_premium(0, 7) > 0.3);
}

/// **The training a trade needs matches the level it wants.** A doctor is
/// expected to be expert because that is what six years and a foundation
/// programme buy.
#[test]
fn the_ticket_and_the_competence_line_up() {
    use scale_sim::person::{qualification_for, Qualification};
    for t in [
        Trade::Doctor,
        Trade::Nurse,
        Trade::Electrician,
        Trade::Shopworker,
        Trade::Labourer,
    ] {
        let gated = qualification_for(t) != Qualification::School;
        if gated {
            assert!(
                t.wants_level() >= 3,
                "{} needs a qualification but wants only level {}",
                t.name(),
                t.wants_level()
            );
        }
    }
    assert!(Trade::Doctor.wants_level() > Trade::Nurse.wants_level());
    assert!(Trade::Nurse.wants_level() > Trade::CareAssistant.wants_level());
    assert!(Trade::Electrician.wants_level() > Trade::Labourer.wants_level());
}

/// **A country starts with people who have already worked.**
#[test]
fn a_world_begins_with_experienced_people_in_it() {
    let e = a_nation().economy;
    let folk = Populace::seed(&e, 200, 20260828);
    let n = folk.people.len() as f64;

    let untrained = folk.people.iter().filter(|p| p.competence() == 0).count() as f64 / n;
    assert!(
        untrained < 0.10,
        "{:.0}% of a starting population cannot do their own job",
        untrained * 100.0
    );

    let mean: f64 = folk.people.iter().map(|p| p.competence() as f64).sum::<f64>() / n;
    // **Most people are competent to skilled at their work**, which is
    // the honest shape of a workforce. A country of masters is a country
    // that has flattered itself.
    assert!(
        (3.5..6.5).contains(&mean),
        "the average worker is level {mean:.1} ({})",
        Skill::grade(mean.round() as u8)
    );
    let masters = folk.people.iter().filter(|p| p.competence() >= 8).count() as f64 / n;
    assert!(
        masters < 0.10,
        "{:.0}% of the country is a master of its trade",
        masters * 100.0
    );
}

/// It survives a decade of the economy running.
#[test]
fn skills_hold_up_over_a_working_life() {
    let mut e = a_nation().economy;
    let mut folk = Populace::seed(&e, 60, 20260828);
    for day in 0..(DAYS_PER_YEAR * 10) {
        e.step();
        folk.live_a_day(&mut e, day);
    }
    e.ledger.assert_conserved();
    for p in folk.people.iter() {
        assert!(p.competence() <= p.ceiling(), "{} exceeded their ceiling", p.name);
        assert!(p.competence() <= 10);
    }
    let mean: f64 =
        folk.people.iter().map(|p| p.competence() as f64).sum::<f64>() / folk.people.len() as f64;
    assert!(
        mean > 3.0,
        "after a decade the average worker is only level {mean:.1}"
    );
}
