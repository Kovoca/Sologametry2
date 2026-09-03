//! **Coping and breakdown, staged rather than a tantrum table.**
//!
//! Slice 8 of `docs/mind-spec.md`, section 10. Every test here is aimed
//! at one of the four things a roll-on-a-table cannot do: cope at all,
//! arrive in order, come back, or break *in character*.

use scale_sim::coping::{
    choose, cope, fit, Breaks, Coping, Focus, Situation, Stage, Strain, ENTER, LEAVE,
    RECOVERY_PER_DAY, STRAIN_CEILING, STRAIN_PER_DAY,
};
use scale_sim::mind::{Facet, Mind, Value};
use scale_sim::rng::Rng;

fn a_person(seed: u64, traits: &[(Facet, f32)]) -> Mind {
    let mut m = Mind::draw(&mut Rng::new(seed), &[(Value::Family, 25)]);
    for f in Facet::ALL {
        m.person.set_baseline(f, 0.0);
    }
    for &(f, z) in traits {
        m.person.set_baseline(f, z);
    }
    m.willpower = 0.0;
    m
}

/// Something that can be done about, and something that cannot.
fn controllable() -> Situation {
    Situation { severity: 0.7, control: 0.9, ..Default::default() }
}
fn hopeless() -> Situation {
    Situation { severity: 0.7, control: 0.05, ..Default::default() }
}

// =====================================================================
// coping exists at all
// =====================================================================

/// **Controllability decides which family helps.** The best-replicated
/// result in the coping literature, and the reason `Situation` carries
/// `control` at all: planning your way out of a bereavement does not
/// work, and doing nothing about a leaking roof does not either.
#[test]
fn what_helps_depends_on_whether_anything_can_be_done() {
    assert!(fit(Coping::Active, &controllable()) > fit(Coping::Active, &hopeless()));
    assert!(fit(Coping::Acceptance, &hopeless()) > fit(Coping::Acceptance, &controllable()));
    assert!(fit(Coping::Planning, &controllable()) > fit(Coping::Acceptance, &controllable()));
    assert!(fit(Coping::Reframing, &hopeless()) > fit(Coping::Planning, &hopeless()));
}

/// **A mismatch costs the trying.** It is not merely a nil return: the
/// effort is spent either way, which is why hammering at something you
/// cannot change wears people out.
#[test]
fn the_wrong_kind_of_coping_is_worse_than_a_nil_return() {
    let doer = a_person(1, &[(Facet::Perseverance, 2.0), (Facet::Assertiveness, 1.5)]);
    let matched = cope(&doer, &controllable(), 0.0);
    let mismatched = cope(&doer, &hopeless(), 0.0);

    assert_eq!(matched.chose.focus(), Focus::Problem);
    assert!(
        matched.relief > mismatched.relief * 2.0,
        "trying to fix the unfixable worked nearly as well as fixing something"
    );
    if mismatched.chose.is_approach() {
        assert!(
            mismatched.deferred > 0.0,
            "the effort of trying to fix an unfixable thing cost nothing at all"
        );
    }
}

/// **The person decides what they reach for, and it is not a roll.**
/// The same man in the same trouble reaches for the same thing twice.
#[test]
fn coping_is_chosen_by_the_person_and_not_drawn() {
    let m = a_person(2, &[(Facet::Orderliness, 2.0)]);
    let s = controllable();
    let first = choose(&m, &s, 0.0);
    for _ in 0..20 {
        assert_eq!(choose(&m, &s, 0.0), first, "the same man coped differently twice");
    }
}

/// **And different people reach for different things.** A planner plans,
/// an angry man vents, a sociable one asks for help, a gloomy one blames
/// himself — out of the same trouble.
#[test]
fn two_people_in_one_predicament_do_different_things() {
    let s = hopeless();
    let planner = a_person(3, &[(Facet::Orderliness, 2.5), (Facet::Tolerance, 1.0)]);
    let angry = a_person(3, &[(Facet::Anger, 2.5)]);
    let sociable = a_person(3, &[(Facet::Gregariousness, 2.5), (Facet::Trust, 1.5)]);
    let gloomy = a_person(3, &[(Facet::Gloom, 2.5), (Facet::Anxiety, 1.5)]);

    let picks = [
        choose(&planner, &s, 0.3),
        choose(&angry, &s, 0.3),
        choose(&sociable, &s, 0.3),
        choose(&gloomy, &s, 0.3),
    ];
    let mut distinct = picks.to_vec();
    distinct.sort();
    distinct.dedup();
    assert!(
        distinct.len() >= 3,
        "four very different people coped almost identically: {picks:?}"
    );
    assert_eq!(choose(&angry, &s, 0.3), Coping::Venting);
    assert_eq!(choose(&gloomy, &s, 0.3), Coping::SelfBlame);
}

/// **Avoidance works, and that is exactly why it is a trap.**
///
/// A model where avoidance simply fails cannot explain why anybody
/// avoids anything, and people avoid constantly. It gives the most
/// relief today and puts more than it relieved onto the debt.
#[test]
fn avoidance_gives_the_most_relief_today_and_costs_more_tomorrow() {
    let drinker = a_person(4, &[(Facet::ExcitementSeeking, 2.5), (Facet::Gloom, 2.0)]);
    let steady = a_person(4, &[(Facet::Tolerance, 2.0), (Facet::Cheerfulness, 1.5)]);
    let s = hopeless();

    let avoided = cope(&drinker, &s, 0.5);
    let faced = cope(&steady, &s, 0.5);
    assert!(!avoided.chose.is_approach());
    assert!(faced.chose.is_approach());

    assert!(
        avoided.relief > faced.relief,
        "avoiding it felt worse on the day, which is not why people do it"
    );
    assert!(
        avoided.deferred > avoided.relief,
        "avoidance came out free, so nothing about it is a trap"
    );
    assert!(faced.deferred < avoided.deferred);
}

/// **Support buffers most where it is needed most.** The
/// stress-buffering hypothesis is specifically that company matters at
/// high stress and does little at low — not a flat bonus.
#[test]
fn company_matters_most_when_things_are_worst() {
    let sociable = a_person(5, &[(Facet::Gregariousness, 2.5), (Facet::Trust, 2.0)]);
    let gain_at = |severity: f64| {
        let mut with = Situation { severity, control: 0.2, ..Default::default() };
        with.company = true;
        let mut alone = with;
        alone.company = false;
        cope(&sociable, &with, 0.2).relief - cope(&sociable, &alone, 0.2).relief
    };
    assert!(
        gain_at(0.9) > gain_at(0.2) * 2.0,
        "having somebody to turn to helped as much on a good day as a bad one"
    );
}

/// **Alone, the strategies that need somebody are simply unavailable.**
#[test]
fn you_cannot_ask_for_help_when_there_is_nobody_there() {
    let sociable = a_person(6, &[(Facet::Gregariousness, 2.5)]);
    let alone = Situation { severity: 0.7, control: 0.2, company: false, drink_available: false };
    let chose = choose(&sociable, &alone, 0.4);
    assert!(!chose.needs_company(), "he asked somebody who was not there: {chose:?}");
    assert_ne!(chose, Coping::Drink, "he drank what there was none of");
}

/// **Willpower is what holds somebody to the harder option**, which is
/// most of what it is for.
#[test]
fn resolve_keeps_people_off_the_easy_way_out() {
    let mut weak = a_person(7, &[(Facet::ExcitementSeeking, 1.2), (Facet::Gloom, 1.0)]);
    weak.willpower = -1.5;
    let mut firm = weak.clone();
    firm.willpower = 2.0;

    let s = hopeless();
    assert!(!choose(&weak, &s, 0.8).is_approach());
    assert!(
        choose(&firm, &s, 0.8).is_approach(),
        "a man of real resolve took the easy way out under pressure"
    );
}

/// **And strain itself pushes people toward avoidance**, which is the
/// feedback that makes a bad patch into a spiral.
#[test]
fn the_worse_it_gets_the_more_people_avoid_it() {
    let m = a_person(8, &[(Facet::ExcitementSeeking, 0.8)]);
    let s = hopeless();
    let calm = choose(&m, &s, 0.0);
    let desperate = choose(&m, &s, 1.5);
    if calm.is_approach() {
        assert!(
            !desperate.is_approach(),
            "nothing changed about how he coped as he came apart"
        );
    }
}

// =====================================================================
// staged, not a threshold
// =====================================================================

/// **Nobody goes from fine to broken.** The stages arrive in order and
/// each is visible before the next, which is the whole difference from
/// rolling on a table when a number is crossed.
#[test]
fn the_stages_arrive_in_order_and_none_is_skipped() {
    let mut st = Strain::default();
    let mut seen = vec![st.stage];
    for _ in 0..4000 {
        st.a_day_passes(1.0, 0.2);
        if *seen.last().unwrap() != st.stage {
            seen.push(st.stage);
        }
    }
    assert_eq!(
        seen,
        vec![Stage::Coping, Stage::Strained, Stage::Exhausted, Stage::Broken],
        "somebody skipped a stage on the way down"
    );
}

/// **A bad afternoon is not a breakdown, and a bad year is.**
///
/// The reason the debt accumulates rather than being read off today's
/// stress: no single day of grinding pressure looks dramatic.
#[test]
fn one_terrible_day_is_not_a_breakdown() {
    let mut st = Strain::default();
    st.a_day_passes(1.0, 0.2);
    assert_eq!(st.stage, Stage::Coping, "one bad day broke somebody");

    for _ in 0..200 {
        st.a_day_passes(0.55, 0.2);
    }
    assert!(
        st.stage >= Stage::Strained,
        "seven months of unrelieved pressure left no mark at all"
    );
}

/// **Coming back is slower than going under.** Severe burnout runs one
/// to three years, and exhaustion leave is measured in months — the
/// single most important asymmetry here.
#[test]
fn recovery_is_far_slower_than_the_descent() {
    assert!(STRAIN_PER_DAY > RECOVERY_PER_DAY * 2.0);

    let mut down = Strain::default();
    let mut to_exhausted = 0;
    while down.stage < Stage::Exhausted && to_exhausted < 10_000 {
        down.a_day_passes(0.6, 0.2);
        to_exhausted += 1;
    }
    let mut back = 0;
    while down.stage > Stage::Coping && back < 20_000 {
        down.a_day_passes(0.0, 0.2);
        back += 1;
    }
    assert!(
        back > to_exhausted,
        "he recovered in {back} days from a state that took {to_exhausted} to reach"
    );
    assert!(back > 365, "a year of exhaustion cleared up in {back} days");
}

/// **The hole has a bottom.** Unbounded, a decade under it builds a debt
/// that takes a century to clear, so a man who had a very bad ten years
/// could never recover in a lifetime. Being broken is a state, not a
/// running total.
#[test]
fn the_debt_does_not_deepen_for_ever() {
    let mut st = Strain::default();
    for _ in 0..20_000 {
        st.a_day_passes(1.0, 0.0);
    }
    assert!(st.load <= STRAIN_CEILING + 1e-9, "fifty years under it went to {}", st.load);

    // And from the very bottom, real relief clears it in a couple of
    // years, which is what severe burnout takes.
    let mut days = 0;
    while st.stage > Stage::Coping && days < 20_000 {
        st.a_day_passes(0.0, 0.4);
        days += 1;
    }
    assert!(
        (365..=365 * 4).contains(&days),
        "the worst case took {days} days to come back from"
    );
}

/// **A way back exists at all**, which no tantrum table has.
#[test]
fn people_do_come_back() {
    let mut st = Strain::default();
    for _ in 0..2000 {
        st.a_day_passes(0.9, 0.2);
    }
    assert!(st.stage >= Stage::Exhausted);
    for _ in 0..6000 {
        st.a_day_passes(0.05, 0.35);
    }
    assert_eq!(st.stage, Stage::Coping, "nobody ever recovers from anything");
    assert!(st.load < LEAVE[0]);
}

/// **Burnout does not lift the week the workload does.** Hysteresis: the
/// level you leave a stage at is below the level you entered it at.
#[test]
fn getting_out_takes_more_than_getting_back_under_the_line() {
    for i in 0..3 {
        assert!(LEAVE[i] < ENTER[i], "stage {i} left at the same load it was entered at");
    }

    let mut st = Strain::default();
    while st.stage < Stage::Strained {
        st.a_day_passes(0.8, 0.2);
    }
    // Drop the pressure to just under what caused it: still strained.
    let load_at_entry = st.load;
    st.a_day_passes(0.2, 0.2);
    assert_eq!(st.stage, Stage::Strained, "it lifted the moment the pressure came off");
    assert!(st.load <= load_at_entry);
}

/// **A stage costs capacity**, which is what makes it matter to the rest
/// of the simulation rather than being a label.
#[test]
fn each_stage_takes_something_away() {
    assert!(Stage::Coping.capacity() > Stage::Strained.capacity());
    assert!(Stage::Strained.capacity() > Stage::Exhausted.capacity());
    assert!(Stage::Exhausted.capacity() > Stage::Broken.capacity());
    assert!(Stage::Broken.capacity() > 0.0, "a broken man can do literally nothing");
}

/// **Tolerance is what a person carries for nothing**, so the same
/// pressure breaks one man and not another.
#[test]
fn the_same_pressure_does_not_break_everybody() {
    let run = |tolerance: f64| {
        let mut st = Strain::default();
        for _ in 0..900 {
            st.a_day_passes(0.5, tolerance);
        }
        st.stage
    };
    assert!(run(0.7) < run(0.15), "how much somebody can carry made no difference");
    assert_eq!(run(0.7), Stage::Coping);
}

// =====================================================================
// breaking in character
// =====================================================================

/// **The replacement for the tantrum table.** How somebody breaks is
/// read off who they are: not drawn, so it is the same every time for
/// one person and different between people.
#[test]
fn a_breakdown_takes_the_shape_of_the_person() {
    let violent = a_person(10, &[(Facet::Violence, 2.5), (Facet::Anger, 2.0)]);
    let private = a_person(10, &[(Facet::Privacy, 2.5), (Facet::Gloom, 1.5)]);
    let dutiful = a_person(10, &[(Facet::Dutifulness, 2.5), (Facet::Perseverance, 2.5)]);
    let loud = a_person(10, &[(Facet::Anger, 2.2), (Facet::Gregariousness, 2.2)]);
    let sot = a_person(
        10,
        &[(Facet::ExcitementSeeking, 2.5), (Facet::Gloom, 2.0), (Facet::Dutifulness, -2.0)],
    );

    assert_eq!(Strain::breaks_as(&violent), Breaks::Violently);
    assert_eq!(Strain::breaks_as(&private), Breaks::Withdrawn);
    assert_eq!(Strain::breaks_as(&dutiful), Breaks::StillWorking);
    assert_eq!(Strain::breaks_as(&loud), Breaks::Loudly);
    assert_eq!(Strain::breaks_as(&sot), Breaks::Drinking);
}

/// **And it is stable.** Ask a hundred times, get the same answer — the
/// property a die roll cannot have.
#[test]
fn the_same_man_breaks_the_same_way_every_time() {
    let m = a_person(11, &[(Facet::Privacy, 1.8)]);
    let first = Strain::breaks_as(&m);
    for _ in 0..100 {
        assert_eq!(Strain::breaks_as(&m), first);
    }
}

/// **The one that gets missed.** A dutiful man breaks by carrying on
/// perfectly, so nothing about him looks wrong from the outside — which
/// is precisely why it is the dangerous one, and a tantrum table has no
/// way to express it at all.
#[test]
fn some_people_break_by_showing_nothing() {
    let dutiful = a_person(12, &[(Facet::Dutifulness, 2.5), (Facet::Perseverance, 2.0)]);
    let mut st = Strain::default();
    for _ in 0..3000 {
        st.a_day_passes(0.9, 0.2);
    }
    assert_eq!(st.stage, Stage::Broken);
    assert_eq!(
        Strain::breaks_as(&dutiful),
        Breaks::StillWorking,
        "a thoroughly dutiful man made a scene"
    );
    // He is still broken, whatever it looks like.
    assert!(st.stage.capacity() < 0.2);
}

/// **Days at a stage are counted**, because how long somebody has been
/// there is not the same question as how bad it is.
#[test]
fn how_long_somebody_has_been_there_is_tracked_separately() {
    let mut st = Strain::default();
    for _ in 0..600 {
        st.a_day_passes(0.6, 0.2);
    }
    let stage_then = st.stage;
    let days_then = st.days_here;
    st.a_day_passes(0.6, 0.2);
    if st.stage == stage_then {
        assert_eq!(st.days_here, days_then + 1);
    } else {
        assert_eq!(st.days_here, 0, "the counter did not reset on a change of stage");
    }
}
