//! **What somebody needs that is not food.**
//!
//! Slice 4 of `docs/mind-spec.md`. What is guarded here is that
//! satisfaction is **semantic** — an activity provides what it actually
//! provides, and standing in a building is not doing the thing that
//! happens in it.

use scale_sim::mind::{Emotion, Facet, Mind, Value};
use scale_sim::needs::{Circumstances, Doing, Need, Needs};
use scale_sim::rng::Rng;

fn a_mind(seed: u64) -> Mind {
    Mind::draw(&mut Rng::new(seed), &[(Value::Family, 25), (Value::Craftsmanship, 20)])
}

/// Somebody built to order.
fn person(seed: u64, facets: &[(Facet, f32)], values: &[(Value, i8)]) -> Mind {
    let mut m = a_mind(seed);
    for f in Facet::ALL {
        m.person.set_baseline(f, 0.0);
    }
    for &(f, z) in facets {
        m.person.set_baseline(f, z);
    }
    for &(v, held) in values {
        for c in m.values.iter_mut() {
            if c.topic == v {
                c.held = held;
            }
        }
    }
    m.willpower = 0.0;
    m.needs = Some(Needs::of(&m));
    m
}

fn got(v: &[(Need, f64)], n: Need) -> f64 {
    v.iter().find(|(k, _)| *k == n).map(|(_, a)| *a).unwrap_or(0.0)
}

/// Drain everything so satisfaction has somewhere to go.
fn empty(n: &mut Needs) {
    for _ in 0..60 {
        n.a_day_passes();
    }
}

/// **Proximity is not company.**
///
/// The rule the module turns on. A model that satisfies "socialise"
/// because two people stood near each other has a population whose social
/// needs are met by walking to work — and no reason for anybody to seek
/// anybody out.
#[test]
fn passing_a_stranger_is_not_company() {
    let m = person(1, &[], &[]);
    let mut n = m.needs.clone().unwrap();
    empty(&mut n);

    let passing = n.did(Doing::PassAStranger, 1.0, Circumstances::default());
    let talking = n.did(
        Doing::TalkWithAFriend,
        1.0,
        Circumstances { with_somebody_known: true, ..Default::default() },
    );

    assert!(
        got(&passing, Need::Company) < 0.05,
        "passing somebody in a corridor filled {:.0}% of a day's need for \
         company",
        got(&passing, Need::Company) * 100.0
    );
    assert!(
        got(&talking, Need::Company) > 10.0 * got(&passing, Need::Company).max(0.001),
        "an hour with a friend was worth barely more than passing a stranger"
    );
}

/// **An argument is contact and it is not friendship.**
///
/// It can even be exciting. It does not make anybody less lonely, and a
/// model that scores a generic "sociality" cannot say so.
#[test]
fn arguing_satisfies_excitement_and_not_friendship() {
    let m = person(2, &[], &[]);
    let mut n = m.needs.clone().unwrap();
    empty(&mut n);

    let row = n.did(
        Doing::Argue,
        1.0,
        Circumstances { with_somebody_known: true, ..Default::default() },
    );
    assert!(got(&row, Need::Excitement) > 0.2, "a row was not even exciting");
    assert_eq!(
        got(&row, Need::Friendship),
        0.0,
        "shouting at somebody made him feel closer to them"
    );
    assert!(got(&row, Need::Company) > 0.0, "an argument is contact of a kind");
}

/// **A loom is practice; a design is creation.**
///
/// Doing well what you already know how to do is a real satisfaction and
/// a different one from making something that did not exist. Collapse
/// them and a weaver of forty years has no reason ever to try anything.
#[test]
fn working_a_loom_is_not_making_something_new() {
    let m = person(3, &[(Facet::LoveOfMaking, 1.5)], &[(Value::Craftsmanship, 40)]);
    let mut n = m.needs.clone().unwrap();
    empty(&mut n);

    let weaving = n.did(Doing::WorkTheLoom, 2.0, Circumstances::default());
    assert!(got(&weaving, Need::Craft) > 0.3, "a day at the loom was not craft");
    assert_eq!(
        got(&weaving, Need::Creation),
        0.0,
        "weaving another bolt of the usual cloth counted as creating something"
    );

    let mut n2 = m.needs.clone().unwrap();
    empty(&mut n2);
    let designing = n2.did(
        Doing::DesignSomething,
        2.0,
        Circumstances { something_new: true, ..Default::default() },
    );
    assert!(got(&designing, Need::Craft) > 0.0);
    assert!(
        got(&designing, Need::Creation) > 0.3,
        "designing a new tapestry created nothing"
    );

    // And designing the same thing again is not creation either.
    let mut n3 = m.needs.clone().unwrap();
    empty(&mut n3);
    let copying = n3.did(Doing::DesignSomething, 2.0, Circumstances::default());
    assert_eq!(got(&copying, Need::Creation), 0.0);
}

/// **Being in the building is not the activity.**
///
/// The sharpest of them. A model that counts presence has a population
/// whose spiritual needs are met by walking past a church, and there is
/// then no reason for anybody to attend anything.
#[test]
fn standing_in_a_temple_is_not_worship() {
    let m = person(4, &[], &[(Value::Tradition, 45)]);
    let mut n = m.needs.clone().unwrap();
    empty(&mut n);

    let standing = n.did(Doing::StandInATemple, 2.0, Circumstances::default());
    assert!(standing.is_empty(), "standing about in a temple satisfied something");
    assert_eq!(n.get(Need::Worship).satisfaction, 0.0);

    // At the back, not taking part: still nothing.
    let at_the_back = n.did(Doing::AttendTheService, 1.0, Circumstances::default());
    assert_eq!(
        got(&at_the_back, Need::Worship),
        0.0,
        "standing at the back of a service is worship"
    );
    // Tradition is satisfied by being there, which is honest: some of
    // what a service is for is that it happened at all.
    assert!(got(&at_the_back, Need::Tradition) > 0.0);

    // Taking part is the thing.
    let taking_part = n.did(
        Doing::AttendTheService,
        1.0,
        Circumstances { taking_part: true, ..Default::default() },
    );
    assert!(
        got(&taking_part, Need::Worship) > 0.5,
        "taking part in the service did not satisfy a need to worship"
    );
}

/// **Two people in one town want different lives.**
///
/// Weight comes from what somebody believes and what they are like. A man
/// who does not care about worship is not troubled by never worshipping,
/// which is why an unmet need is two numbers and not one.
#[test]
fn what_somebody_needs_comes_from_who_they_are() {
    let sociable = person(5, &[(Facet::Gregariousness, 2.0), (Facet::Privacy, -1.5)], &[]);
    let solitary = person(5, &[(Facet::Gregariousness, -2.0), (Facet::Privacy, 1.5)], &[]);
    assert!(
        sociable.needs.as_ref().unwrap().get(Need::Company).weight
            > solitary.needs.as_ref().unwrap().get(Need::Company).weight + 0.15,
        "a gregarious man and a private one need company equally"
    );

    let devout = person(5, &[], &[(Value::Tradition, 48)]);
    let godless = person(5, &[], &[(Value::Tradition, -40)]);
    assert!(
        devout.needs.as_ref().unwrap().get(Need::Worship).weight
            > godless.needs.as_ref().unwrap().get(Need::Worship).weight + 0.2,
        "a devout man and a godless one need worship equally"
    );

    let scholar = person(5, &[(Facet::Curiosity, 2.0)], &[(Value::Knowledge, 45)]);
    assert!(
        scholar.needs.as_ref().unwrap().get(Need::Learning).weight > 0.5,
        "a curious scholar has no need to learn anything"
    );

    // **Company and friendship are not the same need**, which is what
    // lets somebody be surrounded all day and lonely.
    let mut n = sociable.needs.clone().unwrap();
    empty(&mut n);
    n.did(Doing::DrinkWithWorkmates, 3.0, Circumstances::default());
    assert!(n.get(Need::Company).satisfaction > 0.5, "an evening out was no company");
    assert_eq!(
        n.get(Need::Friendship).satisfaction,
        0.0,
        "drinking among people he does not know made him less lonely"
    );
}

/// **Content and unable to concentrate.**
///
/// The case slice 1 separated stress from focus in order to allow, and
/// which nothing could produce until needs existed: a man with no stress
/// at all, kept from everything he cares about, cannot settle to
/// anything. No amount of calm fixes it.
#[test]
fn a_contented_man_with_nothing_he_wants_cannot_concentrate() {
    let mut rng = Rng::new(6);
    let mut m = person(6, &[(Facet::Gregariousness, 1.5), (Facet::Curiosity, 1.5)], &[]);
    let settled = m.focus.current;

    // A month of nothing: no disaster, no grief, no stress — and nothing
    // he wanted either.
    for _ in 0..30 {
        m.a_day_passes(&mut rng);
    }

    assert!(
        m.stress.load < 0.15,
        "a quiet month left him stressed ({:.2}), which is not the case \
         being tested",
        m.stress.load
    );
    assert!(
        m.focus.current < settled - 0.15,
        "a month with nothing he cares about cost him no concentration \
         ({:.2} against {settled:.2})",
        m.focus.current
    );
    assert!(m.episodes.is_empty(), "he is not agitated; he is going without");

    // And it can be fixed by giving him what he wants, not by calming
    // him down.
    let n = m.needs.as_mut().unwrap();
    n.did(
        Doing::TalkWithAFriend,
        3.0,
        Circumstances { with_somebody_known: true, ..Default::default() },
    );
    n.did(Doing::ReadABook, 3.0, Circumstances::default());
    let before = m.focus.current;
    m.a_day_passes(&mut rng);
    assert!(
        m.focus.current > before,
        "an evening with a friend and a book helped him not at all"
    );
}

/// **Focus goes before unhappiness, which is the order it happens in.**
///
/// Somebody kept from what they care about is scattered for a fortnight
/// before they are miserable about it — and a grievance is a *different*
/// register from a distraction.
#[test]
fn long_neglect_becomes_a_grievance_and_not_merely_a_distraction() {
    let m = person(7, &[(Facet::Gregariousness, 2.0)], &[(Value::Friendship, 45)]);
    let mut n = m.needs.clone().unwrap();

    // A week: distracting, and not yet a complaint.
    for _ in 0..7 {
        n.a_day_passes();
    }
    assert!(n.debt() > 0.05, "a week of nothing cost nothing");
    assert!(
        n.grievances().is_empty(),
        "a week without company is already a grievance"
    );

    // Two months: now it is a complaint.
    for _ in 0..60 {
        n.a_day_passes();
    }
    let sore = n.grievances();
    assert!(
        sore.contains(&Need::Company) || sore.contains(&Need::Friendship),
        "two months of solitude for a gregarious man is not worth \
         complaining about: {sore:?}"
    );

    // What he goes looking for is the worst of it.
    let (want, _) = n.most_pressing().expect("he wants nothing at all");
    assert!(
        matches!(want, Need::Company | Need::Friendship | Need::Occupation | Need::Rest),
        "the thing he most wants after two months alone is {want:?}"
    );
}

/// **The worst one dominates, and a dozen small wants do not add up to a
/// disaster.**
///
/// Summing them would make somebody with a full life of small
/// dissatisfactions worse off than somebody with one ruinous one, which
/// is the wrong way round. So the test holds the neglect still and varies
/// only *which* need is met: satisfying the heaviest helps far more than
/// satisfying two light ones.
#[test]
fn the_worst_want_weighs_more_than_several_small_ones() {
    let m = person(8, &[(Facet::Gregariousness, 2.0)], &[]);

    let mut fed_the_worst = m.needs.clone().unwrap();
    let mut fed_the_trivial = m.needs.clone().unwrap();
    for _ in 0..60 {
        fed_the_worst.a_day_passes();
        fed_the_trivial.a_day_passes();
    }

    // One satisfies the thing a gregarious man most wants.
    fed_the_worst.did(Doing::DrinkWithWorkmates, 3.0, Circumstances::default());
    // The other satisfies two he barely cares about.
    fed_the_trivial.did(Doing::WalkInTheFields, 3.0, Circumstances::default());
    fed_the_trivial.did(Doing::SitAndThink, 3.0, Circumstances::default());

    assert!(
        fed_the_worst.debt() < fed_the_trivial.debt(),
        "an evening among people helped a lonely man less than a country \
         walk and a sit down ({:.3} against {:.3})",
        fed_the_worst.debt(),
        fed_the_trivial.debt()
    );

    // And it saturates: sixty days of nothing is bad, and six hundred is
    // not ten times worse.
    let mut a_bad_month = m.needs.clone().unwrap();
    for _ in 0..30 {
        a_bad_month.a_day_passes();
    }
    let mut a_wasted_life = m.needs.clone().unwrap();
    for _ in 0..600 {
        a_wasted_life.a_day_passes();
    }
    assert!(a_wasted_life.debt() <= 1.0, "debt has no ceiling");
    assert!(
        a_wasted_life.debt() < a_bad_month.debt() * 2.5,
        "twenty months of neglect is twenty times a month of it"
    );
    assert!(a_bad_month.debt() > 0.1, "a month of nothing at all cost nothing");
}

/// A seed gives the same wants.
#[test]
fn the_same_person_wants_the_same_things() {
    let a = a_mind(99);
    let b = a_mind(99);
    assert_eq!(a.needs, b.needs);
    let _ = Emotion::Joy;
}
