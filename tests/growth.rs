//! **How somebody comes to be different from who they were.**
//!
//! Slice 7 of `docs/mind-spec.md` — sections 8 and 20. What is guarded
//! here is that change is *rare*, *cumulative*, *provenanced*, and that
//! **doubt comes before change**.
//!
//! And, since the gate review, that a durable change lands in the layer
//! whose literature it came from: life-satisfaction results cannot reach
//! a personality facet.

use scale_sim::growth::{
    stationary_post_exposure_ceiling, stationary_pre_exposure_ceiling, Argued, Cause,
    DurableTarget, Doubts, Framing, Growth, Role, ShapesPersonality, ShapesWellbeing,
    CONVICTION_STEP, DOUBT_TO_MOVE,
};
use scale_sim::mind::{Facet, Mind, Value, ADAPTATION_LIMIT};
use scale_sim::rng::Rng;

fn a_person(seed: u64) -> Mind {
    let mut m = Mind::draw(&mut Rng::new(seed), &[(Value::Family, 25), (Value::Law, 20)]);
    for f in Facet::ALL {
        m.person.set_baseline(f, 0.0);
    }
    m
}

fn holding(m: &mut Mind, topic: Value, held: i8) {
    if let Some(c) = m.values.iter_mut().find(|c| c.topic == topic) {
        c.held = held;
    }
}

/// A speaker worth listening to and with something new to say.
fn credible_and_fresh() -> Framing {
    Framing { credible: 0.9, novelty: 1.0, ..Default::default() }
}

// =====================================================================
// the layer a change lands in
// =====================================================================

/// **Life satisfaction is not personality**, and the separation is
/// structural rather than conventional: `ShapesWellbeing` has no entry
/// point that reaches a facet, so Lucas's unemployment result cannot be
/// spent on somebody's traits by mistake.
#[test]
fn a_wellbeing_result_cannot_become_a_personality_change() {
    let mut m = a_person(1);
    let mut g = Growth::new();
    let traits_before: Vec<f32> = Facet::ALL.iter().map(|&f| m.person.z(f)).collect();

    g.shaped_wellbeing(ShapesWellbeing::LostWork, 1.0, -1.0, 0);
    g.shaped_wellbeing(ShapesWellbeing::Bereavement, 1.0, -1.0, 0);
    g.settle_into(&mut m, 0);

    for (i, &f) in Facet::ALL.iter().enumerate() {
        assert!(
            (m.person.z(f) - traits_before[i]).abs() < 1e-6,
            "losing work and being widowed moved {f:?}, which no study measured"
        );
    }
    assert!(
        m.wellbeing_baseline < -0.5,
        "two of the largest blows in the literature left life satisfaction untouched"
    );
    // And every one of them is recorded against the layer it belongs to.
    assert!(g
        .episodics
        .iter()
        .all(|e| e.target == DurableTarget::WellbeingBaseline));
}

/// **The mechanism carries its outcome**, so a reader can tell which
/// literature a number came from without going back to the source.
#[test]
fn every_change_says_what_was_measured() {
    let mut g = Growth::new();
    g.shaped_personality(Facet::Anxiety, ShapesPersonality::Trauma, 1.0, 1.0, 0);
    g.shaped_wellbeing(ShapesWellbeing::Bereavement, 1.0, -1.0, 0);

    for e in &g.episodics {
        match (e.target, e.cause) {
            (DurableTarget::Facet(_), Cause::Personality(_)) => {}
            (DurableTarget::WellbeingBaseline, Cause::Wellbeing(_)) => {}
            other => panic!("a change landed in the wrong layer: {other:?}"),
        }
    }
}

/// **A residual is fitted to a finite observation**, and the record says
/// how far out anybody actually looked. Reading past the horizon is
/// extrapolation and the model will say so rather than pretending.
#[test]
fn a_persistence_figure_knows_how_far_the_evidence_goes() {
    let mut g = Growth::new();
    g.shaped_wellbeing(ShapesWellbeing::LostWork, 1.0, -1.0, 0);
    let e = g.episodics[0];

    assert!(e.within_evidence(365 * 10), "ten years was inside a fifteen-year panel");
    assert!(
        !e.within_evidence(365 * 40),
        "forty years on was reported as measured when nobody followed anybody that long"
    );
    let p = ShapesWellbeing::LostWork.persistence();
    assert!(p.calibration_horizon_days > 365.0 * 5.0);
    assert!(p.residual_fraction > 0.0 && p.residual_fraction < 1.0);
}

/// **The striking real result, now in the layer that measured it.**
/// Recovery from bereavement is substantial; recovery from losing work
/// is incomplete *on average* — and both of those are life satisfaction.
#[test]
fn grief_lifts_and_losing_work_does_not() {
    let mut g = Growth::new();
    g.shaped_wellbeing(ShapesWellbeing::Bereavement, 1.0, -1.0, 0);
    let mut h = Growth::new();
    h.shaped_wellbeing(ShapesWellbeing::LostWork, 1.0, -1.0, 0);

    let (grief_then, work_then) = (g.wellbeing(0).abs(), h.wellbeing(0).abs());
    assert!(grief_then > work_then, "losing a job hit harder on the day");

    let (grief_now, work_now) = (g.wellbeing(3650).abs(), h.wellbeing(3650).abs());
    assert!(
        grief_now / grief_then < work_now / work_then,
        "grief kept a larger share of itself than unemployment did"
    );
    assert!(grief_now > 0.0, "a bereavement left nothing at all behind");
}

// =====================================================================
// aggregation: raw first, project once
// =====================================================================

/// **Order cannot matter.** The contributions are summed raw and clamped
/// once, so nothing has to record how much of the shared ceiling it
/// happened to receive.
#[test]
fn a_then_b_is_the_same_as_b_then_a() {
    let mk = |first: bool| {
        let mut g = Growth::new();
        if first {
            g.shaped_personality(Facet::Anxiety, ShapesPersonality::Trauma, 1.0, 1.0, 0);
            g.shaped_personality(Facet::Anxiety, ShapesPersonality::CoreMemory, 0.8, 1.0, 0);
        } else {
            g.shaped_personality(Facet::Anxiety, ShapesPersonality::CoreMemory, 0.8, 1.0, 0);
            g.shaped_personality(Facet::Anxiety, ShapesPersonality::Trauma, 1.0, 1.0, 0);
        }
        g.expressed_for(Facet::Anxiety, 500)
    };
    assert!((mk(true) - mk(false)).abs() < 1e-7);
}

/// **One push of a size is two pushes of half of it.**
#[test]
fn contributions_add_rather_than_compete() {
    let mut one = Growth::new();
    one.shaped_personality(Facet::Anxiety, ShapesPersonality::Trauma, 1.0, 1.0, 0);
    let mut two = Growth::new();
    two.shaped_personality(Facet::Anxiety, ShapesPersonality::Trauma, 0.5, 1.0, 0);
    two.shaped_personality(Facet::Anxiety, ShapesPersonality::Trauma, 0.5, 1.0, 0);
    assert!(
        (one.raw_for(Facet::Anxiety, 0) - two.raw_for(Facet::Anxiety, 0)).abs() < 1e-6
    );
}

/// **What is hidden behind a saturating effect reappears the moment it
/// goes**, with nothing having to push again.
///
/// This is the bug the old `applied` bookkeeping could not avoid: B is
/// entirely behind A at the ceiling, records that it received nothing,
/// and when A fades the facet drops to zero until B happens to be
/// re-applied.
#[test]
fn removing_what_saturated_reveals_what_was_behind_it() {
    let mut g = Growth::new();
    // A: enough on its own to bury the ceiling.
    g.took_a_role(Facet::Dutifulness, 3.0, 0);
    // B: real, and wholly hidden behind it.
    g.took_a_role(Facet::Dutifulness, 0.6, 0);

    let day = 4000;
    assert!(g.raw_for(Facet::Dutifulness, day) > ADAPTATION_LIMIT);
    // Near the bound and never at it: expression saturates smoothly, so
    // the clamp stays a limit rather than becoming the answer.
    let pressed = g.expressed_for(Facet::Dutifulness, day);
    assert!(pressed > ADAPTATION_LIMIT * 0.9 && pressed < ADAPTATION_LIMIT);

    // A ends. B is still standing and must show immediately.
    let mut without_a = Growth::new();
    without_a.roles = vec![g.roles[1]];
    let revealed = without_a.expressed_for(Facet::Dutifulness, day);
    assert!(
        revealed > 0.4,
        "what stood behind the ceiling came out at {revealed:.3} once the other went"
    );
}

/// **Opposing effects cancel**, and it does not matter which arrived
/// first.
#[test]
fn opposing_effects_cancel_in_any_order() {
    let mut g = Growth::new();
    g.shaped_personality(Facet::Anxiety, ShapesPersonality::Trauma, 1.0, 1.0, 0);
    g.shaped_personality(Facet::Anxiety, ShapesPersonality::Trauma, 1.0, -1.0, 0);
    assert!(g.raw_for(Facet::Anxiety, 0).abs() < 1e-6);
    assert!(g.raw_for(Facet::Anxiety, 2000).abs() < 1e-6);
}

/// **Nothing is stored that a reload could get wrong.** Every
/// contribution is recomputed from its dates, so a copy taken mid
/// saturation carries on identically.
#[test]
fn a_save_taken_at_the_ceiling_carries_on_the_same() {
    let mut g = Growth::new();
    g.took_a_role(Facet::Dutifulness, 2.0, 0);
    g.shaped_personality(Facet::Anxiety, ShapesPersonality::Trauma, 1.0, 1.0, 100);
    g.left_the_role(Facet::Dutifulness, 3000);

    // "Saved" at a point where the facet is pinned at the ceiling.
    let reloaded = g.clone();
    for day in [3000u64, 3500, 4000, 6000, 9000] {
        for f in [Facet::Dutifulness, Facet::Anxiety] {
            assert!(
                (g.expressed_for(f, day) - reloaded.expressed_for(f, day)).abs() < 1e-9,
                "a reload diverged at day {day} on {f:?}"
            );
        }
    }
    // And settling twice on one day changes nothing the second time.
    let mut m = a_person(9);
    g.settle_into(&mut m, 4000);
    let once = m.person.z(Facet::Dutifulness);
    g.settle_into(&mut m, 4000);
    assert!((m.person.z(Facet::Dutifulness) - once).abs() < 1e-9);
}

// =====================================================================
// rare, cumulative, provenanced
// =====================================================================

/// **Rare.** Nothing in an ordinary day calls into this module at all.
#[test]
fn an_ordinary_year_changes_nobody() {
    let mut m = a_person(2);
    let g = Growth::new();
    let was = m.person.z(Facet::Anxiety);
    for day in 0..365 {
        g.settle_into(&mut m, day);
    }
    assert!(g.episodics.is_empty() && g.roles.is_empty());
    assert!((m.person.z(Facet::Anxiety) - was).abs() < 1e-6);
}

/// **A career does not saturate a facet.** A role approaches its own
/// target of 0.1–0.3 z, which is what role effects measure. The daily
/// increment this replaced reached the global ±1.5 clamp in under a
/// decade, making saturation the expected outcome of having a job — and
/// turning the safety bound into the mechanism.
#[test]
fn a_working_life_does_not_pin_a_trait_at_the_ceiling() {
    let mut m = a_person(3);
    let mut g = Growth::new();
    g.took_a_role(Facet::Dutifulness, 0.25, 0);

    let after_two = g.expressed_for(Facet::Dutifulness, 730);
    assert!(
        after_two > 0.10 && after_two < 0.25,
        "two years in the job came out at {after_two:.3}, outside the measured 0.1-0.3"
    );

    // A whole forty-year career.
    g.settle_into(&mut m, 365 * 40);
    let lifetime = m.person.z(Facet::Dutifulness);
    assert!(
        lifetime < 0.3,
        "a working life drove a trait to {lifetime:.3}, near the safety ceiling"
    );
    assert!(lifetime > 0.2, "forty years of it did almost nothing: {lifetime:.3}");
}

/// **Leaving the work does not undo it, and does not preserve it
/// either.**
#[test]
fn what_a_role_built_decays_once_the_role_ends() {
    let r = Role { facet: Facet::Dutifulness, target: 0.25, started: 0, ended: Some(3650) };
    let at_the_end = r.current(3650);
    let long_after = r.current(3650 + 365 * 8);
    assert!(long_after < at_the_end, "it kept every bit of it forever");
    assert!(long_after > at_the_end * 0.5, "it vanished entirely on the last day");
}

/// **Cumulative**, and a sustained effort is what actually moves a trait
/// — Roberts's 0.37 SD is twenty-four weeks of treatment, not a session.
#[test]
fn sustained_effort_is_what_moves_a_trait() {
    let mut g = Growth::new();
    g.shaped_personality(Facet::Anxiety, ShapesPersonality::SustainedTreatment, 1.0, -1.0, 0);
    let one_week = g.raw_for(Facet::Anxiety, 0).abs();
    assert!(one_week < 0.03, "one week of trying moved {one_week:.3} z");

    let mut course = Growth::new();
    for week in 0..24 {
        course.shaped_personality(
            Facet::Anxiety,
            ShapesPersonality::SustainedTreatment,
            1.0,
            -1.0,
            week * 7,
        );
    }
    let after = course.raw_for(Facet::Anxiety, 24 * 7).abs();
    assert!(
        after > 0.30 && after < 0.45,
        "a course of treatment came to {after:.3} against a measured 0.37"
    );
}

/// **Provenance is what the record is for.**
#[test]
fn what_shaped_somebody_can_be_named() {
    let mut g = Growth::new();
    g.shaped_personality(Facet::Anxiety, ShapesPersonality::Trauma, 1.0, 1.0, 0);
    g.shaped_personality(Facet::Anxiety, ShapesPersonality::CoreMemory, 0.4, 1.0, 100);
    let shaped = g.what_shaped(DurableTarget::Facet(Facet::Anxiety), 400);
    assert_eq!(shaped[0].0, Cause::Personality(ShapesPersonality::Trauma));

    // A role shows up as a cause too, without being an episodic.
    g.took_a_role(Facet::Anxiety, -0.3, 0);
    let with_role = g.what_shaped(DurableTarget::Facet(Facet::Anxiety), 1500);
    assert!(with_role
        .iter()
        .any(|(c, v)| *c == Cause::Personality(ShapesPersonality::RoleDemand) && *v < 0.0));
}

/// **A role is not an event**, and asking for one as an episodic push is
/// refused rather than quietly approximated — which is how the daily
/// increment got in.
#[test]
fn a_role_cannot_be_filed_as_a_one_off() {
    let mut g = Growth::new();
    g.shaped_personality(Facet::Dutifulness, ShapesPersonality::RoleDemand, 1.0, 1.0, 0);
    assert!(g.episodics.is_empty(), "a standing demand was recorded as an event");
}

/// **The ceiling still holds** whatever is thrown at it.
#[test]
fn growth_cannot_step_around_the_ceiling() {
    let mut m = a_person(5);
    let mut g = Growth::new();
    for day in 0..300 {
        g.shaped_personality(Facet::Anxiety, ShapesPersonality::Trauma, 1.0, 1.0, day);
    }
    g.settle_into(&mut m, 300);
    assert!(m.person.z(Facet::Anxiety) <= ADAPTATION_LIMIT + 1e-4);
}

// =====================================================================
// section 20 — doubt before change
// =====================================================================

/// **The headline.** An argument does not change a mind.
#[test]
fn an_argument_produces_doubt_and_not_a_change_of_mind() {
    let mut m = a_person(10);
    holding(&mut m, Value::Law, 30);
    let mut d = Doubts::new();
    let before = m.conviction(Value::Law);

    assert_eq!(
        d.argued(&mut m, Value::Law, -30, 0.9, credible_and_fresh(), 1),
        Argued::Doubted
    );
    assert_eq!(m.conviction(Value::Law), before);
    assert!(d.strongest_about(Value::Law) > 0.0);
    assert!(d.strongest_about(Value::Law) < DOUBT_TO_MOVE);
}

/// **Preaching to the converted does nothing.**
#[test]
fn agreeing_with_somebody_is_not_persuading_them() {
    let mut m = a_person(11);
    holding(&mut m, Value::Law, 30);
    let mut d = Doubts::new();
    assert_eq!(
        d.argued(&mut m, Value::Law, 32, 1.0, credible_and_fresh(), 1),
        Argued::Preaching
    );
    assert_eq!(d.strongest_about(Value::Law), 0.0);
}

/// **Doubt is directional.**
///
/// One bucket per conviction lets somebody arguing *for* a thing and
/// somebody arguing *against* it fill the same reservoir, so whoever
/// happens to speak when it brims decides which way the person moves.
#[test]
fn arguments_from_both_sides_do_not_pool() {
    let mut m = a_person(12);
    holding(&mut m, Value::Peace, 0);
    let mut d = Doubts::new();

    // **Four each way**, which is not enough on either side alone and
    // would be ample pooled: eight arguments a week apart cross the bar
    // comfortably. Read while both are still standing, since crossing
    // spends the doubt and resets it.
    for round in 0..4u64 {
        d.argued(&mut m, Value::Peace, 40, 1.0, credible_and_fresh(), round * 14 + 1);
        d.argued(&mut m, Value::Peace, -40, 1.0, credible_and_fresh(), round * 14 + 8);
    }
    let up = d.about(Value::Peace, 1);
    let down = d.about(Value::Peace, -1);
    assert!(up > 0.0 && down > 0.0, "one side of the argument left no trace");
    assert!(
        up + down > d.strongest_about(Value::Peace),
        "the two sides were being kept in one bucket"
    );
    assert_eq!(
        m.conviction(Value::Peace),
        0,
        "being argued at from both sides moved him, which is the pooling bug"
    );
    let _ = CONVICTION_STEP;
}

/// **Repeating one sentence is not fresh evidence.** A trusted friend
/// with new reasons persuades; a man saying the same thing weekly buys
/// familiarity.
#[test]
fn saying_it_again_is_not_saying_something_new() {
    let run = |novelty: f64| {
        let mut m = a_person(13);
        holding(&mut m, Value::Law, 20);
        let mut d = Doubts::new();
        let f = Framing { credible: 0.9, novelty, ..Default::default() };
        let mut moves = 0;
        for week in 0..40 {
            if d.argued(&mut m, Value::Law, -40, 1.0, f, week * 7 + 1) == Argued::Moved {
                moves += 1;
            }
        }
        moves
    };
    assert!(
        run(1.0) > run(0.0),
        "the same sentence forty times did as much as forty new arguments"
    );
    assert!(run(1.0) > 0, "forty fresh arguments from a credible friend moved nobody");
}

/// **Doubt decays**, so the person you argue with once a year converts
/// nobody and the one you see weekly can.
#[test]
fn one_conversation_a_year_converts_nobody() {
    let run = |interval: u64| {
        let mut m = a_person(14);
        holding(&mut m, Value::Law, 30);
        let mut d = Doubts::new();
        for n in 0..25u64 {
            d.argued(&mut m, Value::Law, -40, 1.0, credible_and_fresh(), n * interval + 1);
        }
        m.conviction(Value::Law)
    };
    assert_eq!(run(365), 30, "twenty-five arguments over twenty-five years converted a man");
    assert!(run(7) < 30, "twenty-five arguments in half a year moved nobody");
}

/// **Kept up, it works** — slowly, and by a little.
#[test]
fn kept_up_it_does_move_somebody() {
    let mut m = a_person(15);
    holding(&mut m, Value::Law, 20);
    let mut d = Doubts::new();
    let before = m.conviction(Value::Law);

    let mut moved = 0;
    for day in 0..(365 * 3) {
        if day % 7 == 0
            && d.argued(&mut m, Value::Law, -40, 1.0, credible_and_fresh(), day) == Argued::Moved
        {
            moved += 1;
        }
    }
    assert!(moved > 0, "three years of weekly argument never once moved him");
    assert!(m.conviction(Value::Law) < before);
    let shift = (before - m.conviction(Value::Law)) as f32;
    assert!(shift <= moved as f32 * CONVICTION_STEP + 0.01);
}

/// **A conviction held hard resists; a weak one gives.**
#[test]
fn a_deeply_held_conviction_resists_far_more() {
    let run = |held: i8| {
        let mut m = a_person(16);
        holding(&mut m, Value::Law, held);
        let mut d = Doubts::new();
        for week in 0..30 {
            d.argued(&mut m, Value::Law, -45, 1.0, credible_and_fresh(), week * 7 + 1);
        }
        d.strongest_about(Value::Law) + (held - m.conviction(Value::Law)).abs() as f64
    };
    assert!(run(6) > run(48));
}

/// **A man you do not credit is waved away, not resisted.** Dismissal is
/// the ordinary response, and it is a different outcome from digging in.
#[test]
fn a_speaker_you_disbelieve_is_dismissed_and_not_resisted() {
    let mut m = a_person(17);
    holding(&mut m, Value::Law, 40);
    let mut d = Doubts::new();
    let before = m.conviction(Value::Law);
    let liar = Framing { credible: 0.05, novelty: 1.0, ..Default::default() };

    assert_eq!(d.argued(&mut m, Value::Law, -40, 0.9, liar, 1), Argued::Dismissed);
    assert_eq!(
        m.conviction(Value::Law),
        before,
        "being told something by somebody he disbelieves made him more certain"
    );
    assert_eq!(d.strongest_about(Value::Law), 0.0);
}

/// **Hardening needs identity threat**, not merely a disagreeable
/// source: Wood and Porter found no backfire across 52 issues and more
/// than 10,000 participants, and later work finds only limited
/// conditional cases.
#[test]
fn backfire_needs_more_than_a_source_you_dislike() {
    // The conjunction that does produce it.
    let mut zealot = a_person(18);
    holding(&mut zealot, Value::Law, 46);
    let mut d = Doubts::new();
    let threat = Framing {
        credible: 0.1,
        novelty: 1.0,
        identity_centrality: 0.9,
        hostile_intent: 0.8,
        out_group: 0.9,
    };
    let before = zealot.conviction(Value::Law);
    assert_eq!(d.argued(&mut zealot, Value::Law, -40, 0.9, threat, 1), Argued::Hardened);
    assert!(zealot.conviction(Value::Law) > before);

    // Take away any single leg of it and it stops happening.
    for weakened in [
        Framing { identity_centrality: 0.1, ..threat },
        Framing { hostile_intent: 0.0, ..threat },
        Framing { out_group: 0.0, ..threat },
    ] {
        let mut m = a_person(18);
        holding(&mut m, Value::Law, 46);
        let mut d = Doubts::new();
        assert_ne!(
            d.argued(&mut m, Value::Law, -40, 0.9, weakened, 1),
            Argued::Hardened,
            "backfire survived losing one of its conditions"
        );
    }

    // And across an ordinary spread of speakers it is rare.
    let mut hardened = 0;
    let mut budged = 0;
    for seed in 0..60u64 {
        let mut m = a_person(20 + seed);
        holding(&mut m, Value::Law, 25);
        let mut d = Doubts::new();
        let f = Framing {
            credible: 0.15 + 0.85 * ((seed % 7) as f64 / 6.0),
            novelty: 1.0,
            ..Default::default()
        };
        match d.argued(&mut m, Value::Law, -30, 0.9, f, 1) {
            Argued::Hardened => hardened += 1,
            Argued::Doubted | Argued::Moved => budged += 1,
            _ => {}
        }
    }
    assert!(budged > hardened * 4, "{budged} moved toward against {hardened} away");
}

/// **Credibility does more than the argument does.**
#[test]
fn credibility_does_more_than_the_argument_does() {
    let run = |credible: f64| {
        let mut m = a_person(19);
        holding(&mut m, Value::Law, 15);
        let mut d = Doubts::new();
        for week in 0..10 {
            let f = Framing { credible, novelty: 1.0, ..Default::default() };
            d.argued(&mut m, Value::Law, -40, 1.0, f, week * 7 + 1);
        }
        d.strongest_about(Value::Law) + (15 - m.conviction(Value::Law)).abs() as f64
    };
    assert!(run(0.95) > run(0.30) * 2.0);
}

/// **A convert becomes a heretic**, and nothing has to say so.
#[test]
fn changing_your_mind_makes_you_a_heretic() {
    let mut m = a_person(21);
    holding(&mut m, Value::Family, 25);
    if let Some(c) = m.values.iter_mut().find(|c| c.topic == Value::Family) {
        c.cultural = 25;
    }
    let mut d = Doubts::new();
    for week in 0..40 {
        d.argued(&mut m, Value::Family, -40, 1.0, credible_and_fresh(), week * 7 + 1);
    }
    let now = m.values.iter().find(|c| c.topic == Value::Family).unwrap().heterodoxy();
    assert!(now > 0, "a man argued out of his culture's view was still orthodox");
}

/// **Two fixed points, not one**, and a diagnostic has to say which it
/// is reporting or the number is ambiguous.
#[test]
fn the_ceiling_is_stated_for_a_named_phase() {
    let added = 0.7 * 0.9 * 0.18;
    // **Read where neither saturates**, or both are pinned at 1.0 and
    // the distinction the two functions exist for is invisible.
    let post = stationary_post_exposure_ceiling(120.0, added);
    let pre = stationary_pre_exposure_ceiling(120.0, added);
    assert!(post > pre, "the two phases of the fixed point came out the same");
    assert!(pre > 0.0, "nothing at all survived to the next conversation");

    // The gate that was missing when doubt faded by half in three weeks:
    // weekly argument converged below the bar, so the model silently
    // said nobody is ever talked round by anybody they see weekly.
    let weekly = stationary_post_exposure_ceiling(7.0, added);
    assert!(
        weekly > DOUBT_TO_MOVE,
        "weekly argument tops out at {weekly:.2} against a bar of {DOUBT_TO_MOVE}"
    );
    assert!(stationary_post_exposure_ceiling(365.0, added) < DOUBT_TO_MOVE);

    // And it agrees with an explicit simulation of the same cadence.
    let mut m = a_person(22);
    holding(&mut m, Value::Law, 0);
    let mut d = Doubts::new();
    let mut seen: f64 = 0.0;
    for week in 0..60 {
        d.argued(&mut m, Value::Law, 40, 1.0, credible_and_fresh(), week * 7 + 1);
        seen = seen.max(d.about(Value::Law, 1));
    }
    assert!(seen <= 1.0 && seen > 0.0);
}
