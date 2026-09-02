//! **How somebody comes to be different from who they were.**
//!
//! Slice 7 of `docs/mind-spec.md` — sections 8 and 20. What is guarded
//! here is that change is *rare*, *cumulative*, *mechanism-tagged*, and
//! that **doubt comes before change**: an argument does not move a
//! conviction, it makes somebody less sure.

use scale_sim::growth::{Argued, Doubts, Growth, Mechanism, CONVICTION_STEP, DOUBT_TO_MOVE};
use scale_sim::mind::{Facet, Mind, Value, ADAPTATION_LIMIT};
use scale_sim::rng::Rng;

fn a_person(seed: u64) -> Mind {
    let mut m = Mind::draw(&mut Rng::new(seed), &[(Value::Family, 25), (Value::Law, 20)]);
    for f in Facet::ALL {
        m.person.set_baseline(f, 0.0);
    }
    m
}

/// A conviction set exactly, so a test varies one thing.
fn holding(m: &mut Mind, topic: Value, held: i8) {
    if let Some(c) = m.values.iter_mut().find(|c| c.topic == topic) {
        c.held = held;
    }
}

// =====================================================================
// section 8 — rare, cumulative, mechanism-tagged
// =====================================================================

/// **Rare.** Living does not change anybody. A year of ordinary days
/// leaves a personality exactly where it was, because nothing in an
/// ordinary day calls `happened` at all.
#[test]
fn an_ordinary_year_changes_nobody() {
    let mut m = a_person(1);
    let mut g = Growth::new();
    let was = m.person.z(Facet::Anxiety);
    for day in 0..365 {
        g.settle_into(&mut m, day);
    }
    assert!(g.changes.is_empty(), "an uneventful year recorded {} changes", g.changes.len());
    assert!((m.person.z(Facet::Anxiety) - was).abs() < 1e-6);
}

/// **Cumulative.** One instance of anything is small; it is the
/// repetition that makes somebody different.
#[test]
fn one_event_is_small_and_a_thousand_are_not() {
    let mut m = a_person(2);
    let mut g = Growth::new();
    let was = m.person.z(Facet::Dutifulness);

    g.happened(Facet::Dutifulness, Mechanism::RoleDemand, 1.0, 1.0, 0);
    g.settle_into(&mut m, 0);
    let after_one = m.person.z(Facet::Dutifulness) - was;
    assert!(
        after_one < 0.01,
        "one day in a job moved a man by {after_one:.3} z"
    );

    // Two years of it — about 500 working days.
    for day in 1..500 {
        g.happened(Facet::Dutifulness, Mechanism::RoleDemand, 1.0, 1.0, day);
    }
    g.settle_into(&mut m, 500);
    let after_years = m.person.z(Facet::Dutifulness) - was;
    assert!(
        after_years > 0.1 && after_years < 0.45,
        "two years in a demanding role moved a man by {after_years:.3} z, \
         against a real role effect of 0.1-0.3"
    );
}

/// **Mechanism-tagged**, and the tag is not decoration: it is what the
/// record is *for*. A total says a man is anxious; the list says why.
#[test]
fn what_shaped_somebody_can_be_named() {
    let mut g = Growth::new();
    g.happened(Facet::Anxiety, Mechanism::Trauma, 1.0, 1.0, 0);
    g.happened(Facet::Anxiety, Mechanism::Bereavement, 0.8, 1.0, 100);
    for day in 200..400 {
        g.happened(Facet::Anxiety, Mechanism::RoleDemand, 0.5, -1.0, day);
    }

    let shaped = g.what_shaped(Facet::Anxiety, 400);
    assert!(!shaped.is_empty());
    assert_eq!(
        shaped[0].0,
        Mechanism::Trauma,
        "the largest thing that happened to him was not the first thing named"
    );
    // Every change carries a mechanism; none is anonymous.
    assert!(g.changes.iter().all(|c| Mechanism::ALL.contains(&c.mechanism)));
    // And a steadying role shows as pushing the other way.
    let role = shaped.iter().find(|(m, _)| *m == Mechanism::RoleDemand).unwrap();
    assert!(role.1 < 0.0, "a settling job read as making him worse");
}

/// **The heart of the mechanism tag, and it is not intuitive.**
///
/// The *larger* blow leaves less behind. People substantially recover
/// from being widowed and largely do not recover from losing their work
/// — and that difference is not explained by which hurt more at the
/// time.
#[test]
fn a_bereavement_fades_and_losing_your_work_does_not() {
    let mut g = Growth::new();
    g.happened(Facet::Gloom, Mechanism::Bereavement, 1.0, 1.0, 0);
    let mut h = Growth::new();
    h.happened(Facet::Gloom, Mechanism::LostWork, 1.0, 1.0, 0);

    let grief_then = g.standing(Facet::Gloom, 0);
    let work_then = h.standing(Facet::Gloom, 0);
    assert!(
        grief_then > work_then,
        "losing a job hit harder on the day than losing somebody"
    );

    // Four years on.
    let grief_now = g.standing(Facet::Gloom, 1460);
    let work_now = h.standing(Facet::Gloom, 1460);
    assert!(
        work_now > grief_now,
        "after four years grief still outweighed unemployment: {grief_now:.3} vs {work_now:.3}"
    );
    assert!(
        grief_now / grief_then < 0.4,
        "a bereavement left {:.0}% of itself after four years",
        100.0 * grief_now / grief_then
    );
    assert!(
        work_now / work_then > 0.8,
        "unemployment faded like an ordinary misfortune"
    );
}

/// **A fading change really does come back down**, and the personality
/// takes it — which is the reviewer's point about the clamp made
/// concrete: a state bound is not a lifetime budget.
#[test]
fn a_person_can_be_moved_back_toward_who_they_were() {
    let mut m = a_person(4);
    let mut g = Growth::new();
    let was = m.person.z(Facet::Gloom);

    g.happened(Facet::Gloom, Mechanism::Bereavement, 1.0, 1.0, 0);
    g.settle_into(&mut m, 0);
    let at_the_time = m.person.z(Facet::Gloom) - was;
    assert!(at_the_time > 0.2);

    for year in 1..=5 {
        g.settle_into(&mut m, year * 365);
    }
    let years_later = m.person.z(Facet::Gloom) - was;
    assert!(
        years_later < at_the_time * 0.45,
        "five years on he was still {years_later:.3} of the {at_the_time:.3} he was"
    );
    assert!(
        years_later > 0.0,
        "he got over it completely, which is not what bereavement does"
    );
}

/// **A life of small demands outweighs one dramatic day**, which is the
/// whole reason the role mechanism exists and the reason personality
/// change is described as cumulative rather than eventful.
#[test]
fn a_role_practised_for_years_beats_one_terrible_day() {
    let mut role = Growth::new();
    for day in 0..1250 {
        role.happened(Facet::Dutifulness, Mechanism::RoleDemand, 1.0, 1.0, day);
    }
    let mut once = Growth::new();
    once.happened(Facet::Dutifulness, Mechanism::Trauma, 1.0, 1.0, 0);

    let after = 1825; // five years
    assert!(
        role.standing(Facet::Dutifulness, after) > once.standing(Facet::Dutifulness, after),
        "five years of doing the job mattered less than one bad afternoon"
    );
}

/// **A working lifetime saturates and does not overflow.**
///
/// The bug this catches is specific: if what has been pushed is recorded
/// unclamped, the first day anything fades subtracts a difference from a
/// value that never got there, and walks the whole adaptation off the far
/// side.
#[test]
fn a_lifetime_in_one_role_saturates_cleanly() {
    let mut m = a_person(6);
    let mut g = Growth::new();
    let was = m.person.z(Facet::Dutifulness);
    for day in 0..12_000 {
        g.happened(Facet::Dutifulness, Mechanism::RoleDemand, 1.0, 1.0, day);
        if day % 500 == 0 {
            g.settle_into(&mut m, day);
        }
    }
    g.settle_into(&mut m, 12_000);
    let moved = m.person.z(Facet::Dutifulness) - was;
    assert!(
        (moved - ADAPTATION_LIMIT).abs() < 1e-3,
        "a working lifetime came out at {moved:.3}, not the {ADAPTATION_LIMIT} ceiling"
    );

    // And it stays there rather than walking backwards as things fade.
    for year in 34..44 {
        g.settle_into(&mut m, year * 365);
        let now = m.person.z(Facet::Dutifulness) - was;
        assert!(
            now > 1.0,
            "adaptation walked back to {now:.3} while the role was still being done"
        );
    }
}

// =====================================================================
// section 20 — doubt before change
// =====================================================================

/// **The headline.** An argument does not change a mind. It makes
/// somebody less sure, and that is a different object.
#[test]
fn an_argument_produces_doubt_and_not_a_change_of_mind() {
    let mut m = a_person(10);
    holding(&mut m, Value::Law, 30);
    let mut d = Doubts::new();

    let before = m.conviction(Value::Law);
    let what = d.argued(&mut m, Value::Law, -30, 0.9, 0.9, 1);
    assert_eq!(what, Argued::Doubted, "one argument settled it on the spot");
    assert_eq!(
        m.conviction(Value::Law),
        before,
        "a single conversation moved a man's convictions"
    );
    assert!(d.about(Value::Law) > 0.0, "an argument left no doubt at all");
    assert!(
        d.about(Value::Law) < DOUBT_TO_MOVE,
        "one argument produced enough doubt to convert somebody"
    );
}

/// **Preaching to the converted does nothing**, or every conversation
/// between people who agree is a persuasion event.
#[test]
fn agreeing_with_somebody_is_not_persuading_them() {
    let mut m = a_person(11);
    holding(&mut m, Value::Law, 30);
    let mut d = Doubts::new();
    assert_eq!(d.argued(&mut m, Value::Law, 32, 1.0, 1.0, 1), Argued::Preaching);
    assert_eq!(d.about(Value::Law), 0.0);
    assert_eq!(m.conviction(Value::Law), 30);
}

/// **Doubt decays**, which is the whole difference between the person
/// you argue with every week and the one you argue with once a year.
#[test]
fn one_conversation_a_year_converts_nobody() {
    let mut yearly = a_person(12);
    holding(&mut yearly, Value::Law, 30);
    let mut dy = Doubts::new();
    for year in 0..25 {
        dy.argued(&mut yearly, Value::Law, -40, 1.0, 0.9, year * 365 + 1);
    }

    let mut weekly = a_person(12);
    holding(&mut weekly, Value::Law, 30);
    let mut dw = Doubts::new();
    for week in 0..25 {
        dw.argued(&mut weekly, Value::Law, -40, 1.0, 0.9, week * 7 + 1);
    }

    assert_eq!(
        yearly.conviction(Value::Law),
        30,
        "twenty-five arguments spread over twenty-five years converted a man"
    );
    assert!(
        weekly.conviction(Value::Law) < 30,
        "twenty-five arguments in half a year moved nobody"
    );
}

/// **Sustained argument from somebody credible does eventually work** —
/// slowly, and by a little. If it never worked, nobody would ever change
/// their mind about anything.
#[test]
fn kept_up_it_does_move_somebody() {
    let mut m = a_person(13);
    holding(&mut m, Value::Law, 20);
    let mut d = Doubts::new();
    let before = m.conviction(Value::Law);

    let mut moved = 0;
    for day in 0..(365 * 3) {
        if day % 7 == 0 && d.argued(&mut m, Value::Law, -40, 1.0, 0.9, day) == Argued::Moved {
            moved += 1;
        }
    }
    assert!(moved > 0, "three years of weekly argument never once moved him");
    assert!(
        m.conviction(Value::Law) < before,
        "he ended where he began after three years of it"
    );
    // **And values are stable.** Even this is a shift, not a personality
    // transplant: real value measures report test-retest around 0.7-0.8
    // over years.
    let shift = (before - m.conviction(Value::Law)) as f32;
    assert!(
        shift <= moved as f32 * CONVICTION_STEP + 0.01,
        "convictions moved further than the number of times doubt cashed out"
    );
}

/// **A conviction held hard resists; a weak one gives.** Which is why
/// argument works on the undecided and rarely on the committed.
#[test]
fn a_deeply_held_conviction_resists_far_more() {
    let run = |held: i8| {
        let mut m = a_person(14);
        holding(&mut m, Value::Law, held);
        let mut d = Doubts::new();
        for week in 0..30 {
            d.argued(&mut m, Value::Law, -45, 1.0, 0.9, week * 7 + 1);
        }
        d.about(Value::Law) + (held - m.conviction(Value::Law)).abs() as f64
    };
    let weak = run(6);
    let strong = run(48);
    assert!(
        weak > strong,
        "a lifelong believer gave as much ground as somebody with no view: {weak:.2} vs {strong:.2}"
    );
}

/// **People update toward evidence.** Backfire is the exception, not the
/// rule — the effect is far rarer and weaker than its fame, and large
/// replications find updating toward the argument in almost every
/// condition tested. A model where argument entrenches people would be
/// both wrong and much more cynical than the research.
#[test]
fn hardening_is_the_exception_and_not_the_rule() {
    let mut hardened = 0;
    let mut budged = 0;
    for seed in 0..60u64 {
        let mut m = a_person(20 + seed);
        holding(&mut m, Value::Law, 25);
        let mut d = Doubts::new();
        // An ordinary spread of speakers: mostly people taken as roughly
        // honest, a few not.
        let credible = 0.15 + 0.85 * ((seed % 7) as f64 / 6.0);
        match d.argued(&mut m, Value::Law, -30, 0.9, credible, 1) {
            Argued::Hardened => hardened += 1,
            Argued::Doubted | Argued::Moved => budged += 1,
            Argued::Preaching => {}
        }
    }
    assert!(budged > hardened * 4, "{budged} moved toward against {hardened} away");

    // **But it does exist**, and a test that never reaches the branch is
    // not evidence that the branch is rare — it is evidence of nothing.
    // A conviction held hard, pushed by somebody taken for a liar.
    let mut zealot = a_person(99);
    holding(&mut zealot, Value::Law, 46);
    let before = zealot.conviction(Value::Law);
    let mut d = Doubts::new();
    assert_eq!(
        d.argued(&mut zealot, Value::Law, -40, 0.9, 0.05, 1),
        Argued::Hardened,
        "a zealot pushed by a man he thinks a liar did not dig in at all"
    );
    assert!(
        zealot.conviction(Value::Law) > before,
        "he hardened without actually moving further into his view"
    );
}

/// **A decay rate and a threshold together decide whether anybody can
/// ever be persuaded**, and the number that settles it is visible in
/// neither of them.
///
/// This is the gate that was missing when doubt was set to fade by half
/// in three weeks: weekly argument from somebody wholly credible
/// converged on 0.55 against a bar of 0.75, so the model asserted that
/// nobody is ever talked round by anybody they see every week — and it
/// asserted it silently, because both constants read perfectly sensibly
/// on their own.
#[test]
fn repeated_influence_has_a_ceiling_and_it_must_clear_the_bar() {
    // The most one credible weekly arguer can raise in somebody with a
    // moderately held view.
    let added = 0.7 * 0.9 * 0.18;
    let weekly = Doubts::ceiling_at(7.0, added);
    assert!(
        weekly > DOUBT_TO_MOVE,
        "weekly argument tops out at {weekly:.2} against a bar of {DOUBT_TO_MOVE}:          nobody in this world can be talked round by anybody"
    );

    // And the other end has to fail, or doubt does not decay usefully.
    let yearly = Doubts::ceiling_at(365.0, added);
    assert!(
        yearly < DOUBT_TO_MOVE,
        "one argument a year tops out at {yearly:.2} and would eventually convert everybody"
    );
}

/// **A brilliant argument from a man taken for a liar moves nothing**,
/// and that is section 17's rule arriving where it decides whether
/// anybody's mind changes: what matters is the listener's reading, never
/// the speaker's intent or skill.
#[test]
fn credibility_does_more_than_the_argument_does() {
    let doubt_after = |credible: f64| {
        let mut m = a_person(15);
        holding(&mut m, Value::Law, 15);
        let mut d = Doubts::new();
        for week in 0..10 {
            d.argued(&mut m, Value::Law, -40, 1.0, credible, week * 7 + 1);
        }
        d.about(Value::Law) + (15 - m.conviction(Value::Law)).abs() as f64
    };
    assert!(
        doubt_after(0.95) > doubt_after(0.25) * 2.0,
        "being believed barely mattered"
    );
}

/// **A convert becomes a heretic**, and nothing has to say so: moving a
/// conviction moves its distance from the culture, which already exists.
#[test]
fn changing_your_mind_makes_you_a_heretic() {
    let mut m = a_person(16);
    holding(&mut m, Value::Family, 25);
    if let Some(c) = m.values.iter_mut().find(|c| c.topic == Value::Family) {
        c.cultural = 25;
    }
    let orthodox = m.values.iter().find(|c| c.topic == Value::Family).unwrap().heterodoxy();
    assert_eq!(orthodox, 0);

    let mut d = Doubts::new();
    for week in 0..40 {
        d.argued(&mut m, Value::Family, -40, 1.0, 0.9, week * 7 + 1);
    }
    let now = m.values.iter().find(|c| c.topic == Value::Family).unwrap().heterodoxy();
    assert!(
        now > orthodox,
        "a man argued out of his culture's view was still perfectly orthodox"
    );
}

/// **Growth writes a personality only through `adapt`.** The module holds
/// no route to `adaptation`, which is private — so the ceiling cannot be
/// stepped around by the one thing that pushes on it hardest.
#[test]
fn growth_cannot_step_around_the_ceiling() {
    let mut m = a_person(17);
    let mut g = Growth::new();
    let was = m.person.z(Facet::Anxiety);
    for day in 0..200 {
        // Absurd: a catastrophe every day for most of a year.
        g.happened(Facet::Anxiety, Mechanism::Trauma, 1.0, 1.0, day);
        g.settle_into(&mut m, day);
    }
    let moved = m.person.z(Facet::Anxiety) - was;
    assert!(
        moved <= ADAPTATION_LIMIT + 1e-4,
        "two hundred catastrophes moved a man {moved:.3} z, past the ceiling"
    );
}
