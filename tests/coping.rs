//! **Coping and breakdown, staged rather than a tantrum table.**
//!
//! Slice 8 of `docs/mind-spec.md`, section 10.
//!
//! The most important test in the file is
//! `a_coarse_advance_matches_a_run_of_days`: without it a person's
//! mental state depends on whether they happened to be simulated in
//! detail, which would make slice 9 meaningless.

use scale_sim::coping::{
    attempt, choose, propensities, regulatory_capacity, resolve, restrain, ActualControl,
    Acute, Attempt, AvoidanceKind, Burnout, Circumstances, ControlAppraisal, Coping, Demands,
    Family, FunctionalDomain, FunctionalState, Strain, SupportGiven, ENTER, LEAVE,
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

fn thinks_they_can() -> ControlAppraisal {
    ControlAppraisal { source: 0.9, consequences: 0.9, own_response: 0.5 }
}
fn thinks_they_cannot() -> ControlAppraisal {
    ControlAppraisal { source: 0.05, consequences: 0.05, own_response: 0.5 }
}

// =====================================================================
// timestep invariance — the slice 9 gate
// =====================================================================

/// **One stage at a time means chronologically, not one per call.**
///
/// A person simulated day by day and the same person advanced once over
/// the same span must end in the same place. Otherwise a detailed
/// character traverses several stages while a distant one updated after
/// two years moves only one, and somebody's mind depends on whether the
/// engine happened to be looking at them.
#[test]
fn a_coarse_advance_matches_a_run_of_days() {
    for (days, pressure, tolerance) in [
        (730u32, 0.9, 0.2),   // a long descent
        (400, 0.55, 0.2),     // a slow one
        (1500, 1.0, 0.1),     // all the way to the bottom
        (90, 0.3, 0.2),       // barely anything
    ] {
        let mut daily = Strain::default();
        for _ in 0..days {
            daily.a_day_passes(pressure, tolerance);
        }
        let mut coarse = Strain::default();
        coarse.advance(days, pressure, tolerance);

        assert_eq!(coarse.state, daily.state, "descent diverged at {days} days");
        assert!((coarse.debt - daily.debt).abs() < 1e-9);
        assert!(
            (coarse.history.lifetime_days as i64 - daily.history.lifetime_days as i64).abs()
                <= 1,
            "severe-duration diverged: {} against {}",
            coarse.history.lifetime_days,
            daily.history.lifetime_days
        );
        assert!((coarse.days_in_state as i64 - daily.days_in_state as i64).abs() <= 1);
    }
}

/// **And on the way back up, too**, which is the case a descent-only
/// test would miss entirely.
#[test]
fn a_coarse_advance_matches_a_run_of_days_recovering() {
    let sunk = |mut s: Strain| {
        s.advance(1500, 1.0, 0.1);
        s
    };
    for days in [200u32, 800, 2000] {
        let mut daily = sunk(Strain::default());
        for _ in 0..days {
            daily.a_day_passes(0.0, 0.3);
        }
        let mut coarse = sunk(Strain::default());
        coarse.advance(days, 0.0, 0.3);

        assert_eq!(coarse.state, daily.state, "recovery diverged over {days} days");
        assert!((coarse.debt - daily.debt).abs() < 1e-9);
        assert!(
            (coarse.history.lifetime_days as i64 - daily.history.lifetime_days as i64).abs()
                <= 1
        );
    }
}

/// **And under continued exposure at the ceiling**, where nothing moves
/// but the clock.
#[test]
fn a_coarse_advance_matches_a_run_of_days_at_the_bottom() {
    let mut daily = Strain::default();
    daily.advance(2000, 1.0, 0.0);
    let mut coarse = daily;
    for _ in 0..1000 {
        daily.a_day_passes(1.0, 0.0);
    }
    coarse.advance(1000, 1.0, 0.0);
    assert_eq!(coarse.state, daily.state);
    assert_eq!(coarse.history.lifetime_days, daily.history.lifetime_days);
    assert!((coarse.debt - daily.debt).abs() < 1e-9);
}

/// **Splitting an advance anywhere gives the same answer.**
#[test]
fn an_advance_can_be_split_at_any_point() {
    for cut in [1u32, 137, 400, 729] {
        let mut whole = Strain::default();
        whole.advance(730, 0.8, 0.2);
        let mut split = Strain::default();
        split.advance(cut, 0.8, 0.2);
        split.advance(730 - cut, 0.8, 0.2);
        assert_eq!(split.state, whole.state, "splitting at {cut} changed the outcome");
        assert!((split.debt - whole.debt).abs() < 1e-9);
    }
}

// =====================================================================
// control, twice
// =====================================================================

/// **Selection reads what they believe; resolution reads what is so.**
/// Both important errors follow from the gap.
#[test]
fn believing_you_can_fix_it_is_not_being_able_to() {
    let doer = a_person(1, &[(Facet::Perseverance, 2.0), (Facet::Assertiveness, 1.5)]);
    let c = Circumstances { severity: 0.7, ..Default::default() };

    // He is certain he can sort it out. He cannot.
    let chose = choose(&doer, &thinks_they_can(), &c, 0.0);
    assert_eq!(chose, Coping::Active, "a determined man did not try");
    let nothing_to_be_done = ActualControl { source: 0.0, consequences: 0.0, exit: 0.0, means: 0.2 };
    let out = resolve(
        attempt(chose),
        &nothing_to_be_done,
        &SupportGiven::default(),
        0.7,
        0.0,
    );
    assert!(out.the_problem_moved < 0.05, "the unfixable got fixed");
    assert!(out.deferred > 0.0, "the wasted effort cost nothing");
    assert!(
        out.helplessness > 0.3,
        "trying hard and failing taught him nothing about his own reach"
    );
}

/// **The other error: nothing was tried, and something could have been.**
#[test]
fn believing_nothing_can_be_done_forgoes_what_could() {
    let m = a_person(2, &[(Facet::Tolerance, 1.5)]);
    let c = Circumstances { severity: 0.7, ..Default::default() };
    let chose = choose(&m, &thinks_they_cannot(), &c, 0.0);
    assert!(!chose.is(Family::Problem), "he tried anyway: {chose:?}");

    // Had he tried, it would have worked.
    let could_have = ActualControl { source: 0.9, consequences: 0.9, exit: 0.5, means: 0.9 };
    let taken = resolve(attempt(chose), &could_have, &SupportGiven::default(), 0.7, 0.6);
    let forgone = resolve(
        attempt(Coping::Active),
        &could_have,
        &SupportGiven::default(),
        0.7,
        0.6,
    );
    assert!(
        forgone.the_problem_moved > taken.the_problem_moved + 0.5,
        "giving it up cost him nothing he could have had"
    );
}

/// **Effort costs because the attempt failed, not because its family
/// carries a penalty.** The same strategy against real control is not
/// punished at all.
#[test]
fn the_cost_is_the_failure_and_not_the_family() {
    let a = attempt(Coping::Active);
    let can = ActualControl { source: 0.95, consequences: 0.95, exit: 0.5, means: 0.95 };
    let cannot = ActualControl { source: 0.0, consequences: 0.0, exit: 0.0, means: 0.1 };
    let won = resolve(a, &can, &SupportGiven::default(), 0.7, 0.0);
    let lost = resolve(a, &cannot, &SupportGiven::default(), 0.7, 0.0);

    assert!(won.deferred < 0.02, "succeeding at something still cost him");
    assert!(lost.deferred > won.deferred);
    assert!(won.relief > lost.relief * 3.0);
}

/// **Control is not one number.** A man who cannot stop the thing can
/// still change what it does to him, and coping should see that.
#[test]
fn control_over_the_source_and_over_the_consequences_are_different() {
    let terminal = ControlAppraisal { source: 0.0, consequences: 0.8, own_response: 0.6 };
    assert!(terminal.instrumental() > 0.7, "nothing at all could be done about anything");
    let m = a_person(3, &[(Facet::Orderliness, 1.5)]);
    let chose = choose(&m, &terminal, &Circumstances::default(), 0.0);
    assert!(
        chose.is(Family::Problem),
        "a man who could arrange his affairs sat and did nothing: {chose:?}"
    );
}

// =====================================================================
// strategies and families
// =====================================================================

/// **A family is a tag and they overlap**, which is the honest reading
/// of an instrument whose author says it has no overall score.
#[test]
fn families_are_overlapping_tags_and_not_a_partition() {
    let multi: Vec<Coping> =
        Coping::ALL.into_iter().filter(|c| c.families().len() > 1).collect();
    assert!(
        multi.len() >= 4,
        "every strategy fell in exactly one family, which is a partition and not a tag"
    );
    // Faith is the clearest case: meaning, or a way of not looking.
    assert!(Coping::Faith.is(Family::Emotion) && Coping::Faith.is(Family::Avoidant));
    // Asking for help can do both jobs at once.
    assert!(
        Coping::InstrumentalSupport.is(Family::Problem)
            && Coping::InstrumentalSupport.is(Family::Emotion)
    );
}

/// **All fourteen are first class**, and each is reachable by somebody.
#[test]
fn every_strategy_is_somebody_s() {
    assert_eq!(Coping::ALL.len(), 14);
    let mut ever = std::collections::BTreeSet::new();
    for seed in 0..40u64 {
        for control in [thinks_they_can(), thinks_they_cannot()] {
            for company in [true, false] {
                for debt in [0.0, 1.0] {
                    let m = a_person(
                        seed,
                        &[
                            (Facet::ALL[(seed % 25) as usize], 2.2),
                            (Facet::ALL[((seed + 7) % 25) as usize], 1.4),
                        ],
                    );
                    let c = Circumstances { company, ..Default::default() };
                    ever.insert(choose(&m, &control, &c, debt));
                }
            }
        }
    }
    assert!(ever.len() >= 8, "only {} strategies were ever reached", ever.len());
}

// =====================================================================
// avoidance: respite and entrenchment
// =====================================================================

/// **Avoidance relieves most today, or nobody would do it.**
#[test]
fn avoidance_gives_the_most_relief_today() {
    let s = 0.7;
    let a = ActualControl::default();
    let avoided = resolve(attempt(Coping::Denial), &a, &SupportGiven::default(), s, 0.8);
    let faced = resolve(attempt(Coping::Acceptance), &a, &SupportGiven::default(), s, 0.8);
    assert!(avoided.relief > faced.relief, "avoiding it felt worse on the day");
}

/// **But it is not uniformly a trap**, and saying so was too strong.
/// A night off from something that was not going to get worse anyway
/// costs almost nothing; drinking about an approaching eviction costs a
/// great deal.
#[test]
fn respite_is_cheap_and_escape_from_a_worsening_thing_is_not() {
    let a = ActualControl::default();
    let s = SupportGiven::default();
    let rest_from_the_unfixable =
        resolve(attempt(Coping::Distraction), &a, &s, 0.7, 0.0);
    let drink_about_the_eviction =
        resolve(attempt(Coping::SubstanceUse), &a, &s, 0.7, 1.0);

    assert!(
        rest_from_the_unfixable.deferred < rest_from_the_unfixable.relief * 0.35,
        "an evening off was charged as though it were denial"
    );
    assert!(
        drink_about_the_eviction.deferred > drink_about_the_eviction.relief,
        "drinking through an eviction came out cheap"
    );
    assert!(drink_about_the_eviction.deferred > rest_from_the_unfixable.deferred * 3.0);
}

/// **Putting down a goal that genuinely cannot be reached is not a
/// failure of nerve.** The same act, when the goal was reachable, is.
#[test]
fn giving_up_on_the_impossible_is_different_from_giving_up() {
    let s = SupportGiven::default();
    let hopeless = ActualControl { source: 0.0, consequences: 0.1, exit: 0.5, means: 0.2 };
    let winnable = ActualControl { source: 0.95, consequences: 0.95, exit: 0.5, means: 0.9 };
    let wise = resolve(attempt(Coping::Disengagement), &hopeless, &s, 0.7, 0.5);
    let premature = resolve(attempt(Coping::Disengagement), &winnable, &s, 0.7, 0.5);
    assert!(
        premature.deferred > wise.deferred * 2.0,
        "walking away from a winnable fight cost the same as from a lost one"
    );
}

/// **Avoiding something that will not get worse does not make it
/// worse.** The debt is a consequence of a concrete thing happening.
#[test]
fn avoidance_of_the_unchangeable_carries_little_debt() {
    let s = SupportGiven::default();
    let a = ActualControl { source: 0.0, consequences: 0.0, exit: 0.0, means: 0.0 };
    let grief = resolve(attempt(Coping::Distraction), &a, &s, 0.8, 0.0);
    let eviction = resolve(attempt(Coping::Distraction), &a, &s, 0.8, 1.0);
    assert!(eviction.deferred > grief.deferred * 2.0);
}

/// Every avoidant strategy says what kind it is.
#[test]
fn avoidance_is_not_one_thing() {
    let kinds: std::collections::BTreeSet<AvoidanceKind> =
        Coping::ALL.into_iter().filter_map(|c| c.avoidance()).collect();
    assert!(kinds.len() >= 4, "avoidance came out as one undifferentiated act");
    assert_eq!(Coping::Distraction.avoidance(), Some(AvoidanceKind::TemporaryRespite));
    assert_eq!(Coping::SubstanceUse.avoidance(), Some(AvoidanceKind::SubstanceEscape));
    assert!(Coping::Active.avoidance().is_none());
}

// =====================================================================
// support is an exchange, not a multiplier
// =====================================================================

/// **Offered is not received.** Help nobody wanted makes things worse,
/// which a generic support bonus cannot express at all.
#[test]
fn an_unwanted_lecture_is_support_that_costs() {
    let a = ActualControl::default();
    let welcome = SupportGiven {
        practical: 0.0,
        emotional: 0.9,
        read_as_helpful: 0.9,
        obligation: 0.0,
    };
    let lecture = SupportGiven {
        practical: 0.0,
        emotional: 0.9,
        read_as_helpful: 0.05,
        obligation: 0.0,
    };
    let heard = resolve(attempt(Coping::EmotionalSupport), &a, &welcome, 0.7, 0.3);
    let lectured = resolve(attempt(Coping::EmotionalSupport), &a, &lecture, 0.7, 0.3);
    assert!(heard.relief > lectured.relief * 5.0);
    assert!(lectured.deferred > 0.0, "being lectured at came out free");
}

/// **Practical help can move the problem; being listened to cannot** —
/// and help that puts you under an obligation carries that cost.
#[test]
fn the_kind_of_support_decides_what_it_can_do() {
    let a = ActualControl { source: 0.8, consequences: 0.8, exit: 0.3, means: 0.9 };
    let listening = SupportGiven {
        practical: 0.0,
        emotional: 1.0,
        read_as_helpful: 1.0,
        obligation: 0.0,
    };
    let money = SupportGiven {
        practical: 1.0,
        emotional: 0.0,
        read_as_helpful: 1.0,
        obligation: 0.9,
    };
    let talked = resolve(attempt(Coping::EmotionalSupport), &a, &listening, 0.7, 0.5);
    let lent = resolve(attempt(Coping::InstrumentalSupport), &a, &money, 0.7, 0.5);

    assert!(talked.the_problem_moved < 0.01, "a sympathetic ear paid the rent");
    assert!(lent.the_problem_moved > 0.2);
    assert!(lent.deferred > 0.0, "being lent money left him owing nothing");
}

/// **Nobody there, nothing to ask.**
#[test]
fn you_cannot_ask_somebody_who_is_not_there() {
    let sociable = a_person(6, &[(Facet::Gregariousness, 2.5)]);
    let alone = Circumstances {
        company: false,
        substance_available: false,
        ..Default::default()
    };
    let chose = choose(&sociable, &thinks_they_cannot(), &alone, 0.4);
    assert!(!chose.needs_company(), "he asked somebody who was not there: {chose:?}");
    assert_ne!(chose, Coping::SubstanceUse);
}

// =====================================================================
// the ladder
// =====================================================================

/// **The states arrive in order and none is skipped.**
#[test]
fn the_states_arrive_in_order() {
    let mut st = Strain::default();
    let mut seen = vec![st.state];
    for _ in 0..4000 {
        st.a_day_passes(1.0, 0.2);
        if *seen.last().unwrap() != st.state {
            seen.push(st.state);
        }
    }
    assert_eq!(
        seen,
        vec![
            FunctionalState::Regulated,
            FunctionalState::Strained,
            FunctionalState::Depleted,
            FunctionalState::Impaired
        ]
    );
}

/// **A bad afternoon is not chronic impairment, and a bad year is.**
#[test]
fn one_terrible_day_is_not_a_breakdown() {
    let mut st = Strain::default();
    st.a_day_passes(1.0, 0.2);
    assert_eq!(st.state, FunctionalState::Regulated);
    for _ in 0..200 {
        st.a_day_passes(0.55, 0.2);
    }
    assert!(st.state >= FunctionalState::Strained);
}

/// **Recovery requires the demands to actually fall below the
/// resources.** A capped accumulator must not drain toward health while
/// the conditions that caused it are unchanged.
#[test]
fn nothing_recovers_while_the_conditions_hold() {
    let mut st = Strain::default();
    st.advance(1500, 0.9, 0.2);
    let sunk = st.debt;
    st.advance(2000, 0.9, 0.2);
    assert!(st.debt >= sunk, "he got better while nothing about his life changed");
    assert_eq!(st.state, FunctionalState::Impaired);

    st.advance(2000, 0.05, 0.4);
    assert!(st.debt < sunk, "relief did nothing");
}

/// **Getting out takes more than getting back under the line.**
#[test]
fn hysteresis_holds_people_in_a_state() {
    for i in 0..3 {
        assert!(LEAVE[i] < ENTER[i]);
    }
    let mut st = Strain::default();
    while st.state < FunctionalState::Strained {
        st.a_day_passes(0.8, 0.2);
    }
    st.a_day_passes(0.2, 0.2);
    assert_eq!(st.state, FunctionalState::Strained);
}

/// **Coming back is slower than going under**, but two years is a
/// *designed bound under favourable conditions* — not a claim about what
/// severe exhaustion generally takes. Recovery evidence is heterogeneous
/// and one clinical cohort still had substantial residual symptoms after
/// seven years.
#[test]
fn recovery_is_slower_than_the_descent() {
    assert!(STRAIN_PER_DAY > RECOVERY_PER_DAY * 2.0);
    let mut down = Strain::default();
    let mut fell = 0;
    while down.state < FunctionalState::Depleted && fell < 10_000 {
        down.a_day_passes(0.6, 0.2);
        fell += 1;
    }
    let mut back = 0;
    while down.state > FunctionalState::Regulated && back < 20_000 {
        down.a_day_passes(0.0, 0.2);
        back += 1;
    }
    assert!(back > fell);
    assert!(back > 365);
}

/// **The hole has a bottom** — but capping the debt must not erase how
/// long somebody was down there.
#[test]
fn the_debt_saturates_and_the_duration_does_not() {
    let mut brief = Strain::default();
    brief.advance(1200, 1.0, 0.0);
    let mut long = Strain::default();
    long.advance(1200 + 3650, 1.0, 0.0);

    assert!((brief.debt - long.debt).abs() < 1e-9, "the debt kept deepening");
    assert!(brief.debt <= STRAIN_CEILING + 1e-9);
    assert!(
        long.history.lifetime_days > brief.history.lifetime_days * 2,
        "ten extra years at the bottom left no trace at all"
    );
    // **And the sensitivity saturates**, which is the point of it: ten
    // further years at the bottom do not make somebody arbitrarily more
    // fragile, or the unbounded accumulator is back by another road.
    assert!(long.history.relapse_sensitivity() <= 1.0);
    assert!(
        (long.history.relapse_sensitivity() - brief.history.relapse_sensitivity()).abs() < 1e-9,
        "sensitivity was still climbing after ten extra years"
    );
}

/// **Impairment is not global.** A capacity of 0.15 is functioning
/// badly, not being a vegetable — which is why the state is not called
/// "broken".
#[test]
fn impairment_is_a_degree_and_not_an_absence() {
    assert!(FunctionalState::Regulated.capacity() > FunctionalState::Strained.capacity());
    assert!(FunctionalState::Strained.capacity() > FunctionalState::Depleted.capacity());
    assert!(FunctionalState::Depleted.capacity() > FunctionalState::Impaired.capacity());
    assert!(FunctionalState::Impaired.capacity() > 0.0);
}

/// **Burnout keeps its three dimensions**, which is what lets the
/// dutiful worker exist honestly: worn out, not visibly cynical, still
/// performing — at a cost.
#[test]
fn burnout_is_three_axes_and_not_a_rung() {
    let dutiful = Burnout { exhaustion: 0.95, cynicism: 0.10, reduced_efficacy: 0.15 };
    let checked_out = Burnout { exhaustion: 0.40, cynicism: 0.55, reduced_efficacy: 0.25 };
    assert!(dutiful.exhaustion > checked_out.exhaustion);
    assert!(dutiful.cynicism < checked_out.cynicism);
    // A single number could not tell these two apart, which is the point.
    let flat = |b: &Burnout| b.exhaustion + b.cynicism + b.reduced_efficacy;
    assert!(
        (flat(&dutiful) - flat(&checked_out)).abs() < 0.01,
        "the two men differ sharply and any single score puts them together"
    );
}

// =====================================================================
// crisis: a repertoire, not a class
// =====================================================================

/// **Personality constrains a repertoire; circumstance picks from it.**
/// The same violent man does not lash out at somebody who can ruin him,
/// and holds himself together in front of a child.
#[test]
fn the_same_man_does_different_things_in_different_rooms() {
    let mut violent = a_person(10, &[(Facet::Violence, 2.5), (Facet::Anger, 2.0)]);
    // Rested and sober, so the capacity to hold back is there to spend.
    violent.willpower = 1.0;
    let plain = Circumstances::default();
    let before_a_child = Circumstances {
        someone_to_protect: true,
        they_matter: 1.0,
        ..Default::default()
    };
    let before_a_magistrate =
        Circumstances { other_has_authority: true, ..Default::default() };

    assert_eq!(Strain::crisis_propensities(&violent, &plain)[0].0, Acute::Aggression);
    assert_ne!(
        Strain::crisis_propensities(&violent, &before_a_child)[0].0,
        Acute::Aggression,
        "he went for somebody in front of his own child"
    );
    assert_ne!(
        Strain::crisis_propensities(&violent, &before_a_magistrate)[0].0,
        Acute::Aggression,
        "he swung at a man who could hang him"
    );
}

/// **Stable given a genuinely identical state**, which is the invariant
/// that matters — not that one man always does one thing.
#[test]
fn identical_circumstances_give_identical_propensities() {
    let m = a_person(11, &[(Facet::Anxiety, 1.8)]);
    let c = Circumstances::default();
    let first = Strain::crisis_propensities(&m, &c);
    for _ in 0..50 {
        assert_eq!(Strain::crisis_propensities(&m, &c), first);
    }
}

/// **More than one response is live at once**, because withdrawal,
/// drinking and a row are a sequence in a bad stretch rather than three
/// character classes.
#[test]
fn several_responses_are_available_at_once() {
    let m = a_person(12, &[(Facet::Anger, 1.2), (Facet::Anxiety, 1.0)]);
    let ranked = Strain::crisis_propensities(&m, &Circumstances::default());
    assert_eq!(ranked.len(), 5, "the repertoire collapsed to one answer");
    assert!(
        ranked[1].1 > ranked[4].1,
        "everything below the top was equally impossible"
    );
}

/// **Coping is not a stage**, and people go on doing it in every state —
/// including the worst one, where what is left is mostly avoidance.
#[test]
fn people_cope_in_every_state_including_the_worst() {
    let m = a_person(13, &[(Facet::Perseverance, 1.0)]);
    let c = Circumstances::default();
    for debt in [0.0, 0.5, 1.0, STRAIN_CEILING] {
        let p = propensities(&m, &thinks_they_cannot(), &c, debt);
        assert!(!p.is_empty(), "at a debt of {debt} he had no way of coping at all");
    }
    // And at the bottom what he reaches for is avoidance.
    let sunk = choose(&m, &thinks_they_cannot(), &c, STRAIN_CEILING);
    assert!(sunk.is(Family::Avoidant), "at the very bottom he was still coping well");
}

/// **An attempt is raised, not a stress subtraction.** Nothing relieves
/// anything until it has been settled against the world.
#[test]
fn a_strategy_raises_an_attempt_and_does_not_pay_out_by_itself() {
    let a: Attempt = attempt(Coping::Active);
    assert!(a.aims_at_the_world);
    assert!(!a.asks_somebody);
    let asked = attempt(Coping::InstrumentalSupport);
    assert!(asked.asks_somebody);
    // With nobody actually helping, asking achieves nothing.
    let nothing = resolve(
        asked,
        &ActualControl::default(),
        &SupportGiven::default(),
        0.7,
        0.5,
    );
    assert!(nothing.relief < 0.01, "asking for help worked with nobody answering");
}

// =====================================================================
// the gate-review follow-ups
// =====================================================================

/// **Restraint is motive times capacity, not a share of the drive.**
///
/// Scaling the suppression with the impulse gave a violent man
/// self-control in exact proportion to his violence, so he could never
/// fail to hold back. The two have to be able to come apart.
#[test]
fn a_man_can_want_to_stop_and_fail() {
    // Same motive, same drive, different capacity.
    let strong = restrain(2.5, 0.9, 0.9);
    let spent = restrain(2.5, 0.9, 0.15);
    assert!(spent > strong, "being worn out made him more restrained");

    // And a bigger drive is bigger after restraint, which is the error
    // the proportional form hid.
    assert!(restrain(3.0, 0.8, 0.8) > restrain(1.0, 0.8, 0.8));
}

/// **Capacity is spent.** Exhaustion and mounting debt take it without
/// touching the motive at all.
#[test]
fn holding_back_gets_harder_as_somebody_comes_apart() {
    let m = a_person(50, &[]);
    let fresh = regulatory_capacity(&m, 0.0);
    let worn = regulatory_capacity(&m, 1.2);
    assert!(fresh > worn * 1.5, "a man at the end of himself held on as well as ever");
}

/// **A child in the room is not a restraint on somebody it is nothing
/// to.** The motive comes from the tie, not the geometry.
#[test]
fn the_person_to_be_protected_has_to_matter() {
    let mut m = a_person(51, &[(Facet::Violence, 2.0), (Facet::Anger, 2.0)]);
    m.willpower = 1.0;
    let his_own = Circumstances { someone_to_protect: true, they_matter: 1.0, ..Default::default() };
    let a_stranger =
        Circumstances { someone_to_protect: true, they_matter: 0.0, ..Default::default() };
    assert!(his_own.motive_to_hold_back() > a_stranger.motive_to_hold_back());
    assert_eq!(
        Strain::crisis_propensities(&m, &a_stranger)[0].0,
        Acute::Aggression,
        "somebody else's child in the room stopped him"
    );
}

/// **Impairment is measured against a domain**, or the claim that a man
/// impaired at work may be a competent parent is only prose.
#[test]
fn somebody_impaired_at_work_can_still_be_a_parent() {
    // **Read where there is something to see.** At the ceiling every
    // part of a life is impaired and correctly so, which tells you
    // nothing about whether the domain is doing any work.
    let mut st = Strain::default();
    st.advance(350, 0.35, 0.15);
    let d = Demands { work: 0.9, caregiving: 0.5, social: 0.4, self_care: 0.4 };

    let work = st.functioning_in(FunctionalDomain::Work, &d);
    let care = st.functioning_in(FunctionalDomain::Caregiving, &d);
    assert!(
        work > care,
        "work and caregiving came out the same, so the domain does nothing"
    );
    assert!(work >= FunctionalState::Depleted);
}

/// **Self-care goes first**, which is what people actually drop.
#[test]
fn what_gets_dropped_first_is_the_person_themselves() {
    let mut st = Strain::default();
    st.advance(250, 0.35, 0.15);
    let d = Demands { work: 0.5, caregiving: 0.5, social: 0.5, self_care: 0.5 };
    let order: Vec<FunctionalState> = FunctionalDomain::ALL
        .iter()
        .map(|&x| st.functioning_in(x, &d))
        .collect();
    let care = st.functioning_in(FunctionalDomain::Caregiving, &d);
    let self_care = st.functioning_in(FunctionalDomain::SelfCare, &d);
    assert!(self_care >= care, "he stopped minding the children before he stopped sleeping");
    assert!(order.iter().any(|s| *s != order[0]), "every part of his life went at once");
}

/// **Relapse sensitivity has exactly one consumer.** `advance` must not
/// consult it, or one caller applying it daily and another once a year
/// diverge however invariant the interval arithmetic is.
#[test]
fn relapse_sensitivity_is_applied_at_appraisal_and_nowhere_else() {
    let mut veteran = Strain::default();
    veteran.advance(1500, 1.0, 0.1);
    veteran.advance(3000, 0.0, 0.5);
    assert!(veteran.history.relapse_sensitivity() > 0.0);

    // Two people, one with a history and one without, under identical
    // pressure: the ladder itself does not know the difference.
    let fresh = Strain::default();
    let mut a = veteran;
    let mut b = fresh;
    a.debt = 0.0;
    a.state = FunctionalState::Regulated;
    a.advance(300, 0.6, 0.2);
    b.advance(300, 0.6, 0.2);
    assert!(
        (a.debt - b.debt).abs() < 1e-12,
        "a history changed a running interval, which is the timestep leak"
    );

    // It bites where it should: on something new.
    assert!(
        veteran.felt_severity(0.5) > fresh.felt_severity(0.5),
        "a man with four bad years behind him felt a fresh blow no harder"
    );
}

/// **Sensitivity saturates and decays**, or an unbounded duration
/// reintroduces the unbounded accumulator through the back door.
#[test]
fn relapse_sensitivity_does_not_grow_for_ever() {
    let mut long = Strain::default();
    long.advance(20_000, 1.0, 0.0);
    assert!(long.history.relapse_sensitivity() <= 1.0);

    let at_the_time = long.history.relapse_sensitivity();
    long.advance(20_000, 0.0, 0.5);
    assert!(
        long.history.relapse_sensitivity() < at_the_time * 0.5,
        "fifty years of keeping well left him as fragile as the day he stopped"
    );
}

/// **Duration has episode structure.** One unbroken stretch is not
/// twenty short ones.
#[test]
fn one_long_episode_is_not_many_short_ones() {
    let mut once = Strain::default();
    once.advance(900, 0.9, 0.15);

    let mut often = Strain::default();
    for _ in 0..6 {
        often.advance(260, 0.9, 0.15);
        often.advance(900, 0.0, 0.5);
    }
    assert!(often.history.episode_count > once.history.episode_count);
    assert!(
        once.history.current_episode_days > 0,
        "an unbroken episode was not being counted as one"
    );
}

/// **A crisis is a separate mechanism.** It can strike anybody, does not
/// promote the chronic state, and resolving it leaves the chronic strain
/// exactly where it was.
#[test]
fn a_crisis_does_not_touch_the_chronic_state() {
    let mut calm = Strain::default();
    assert_eq!(calm.state, FunctionalState::Regulated);
    calm.crisis_strikes(Acute::Panic, 0.9, 10, 77);
    assert_eq!(calm.state, FunctionalState::Regulated, "one bad hour broke a settled man");
    assert!(calm.crisis.is_some());
    assert!(calm.debt < 1e-9);

    // It fades on its own clock, in days.
    calm.advance(7, 0.1, 0.5);
    assert!(calm.crisis.is_none(), "a panic was still running a week later");
    assert_eq!(calm.state, FunctionalState::Regulated);

    // And on somebody already down, it leaves the chronic state alone.
    let mut sunk = Strain::default();
    sunk.advance(1500, 1.0, 0.1);
    let (was, debt) = (sunk.state, sunk.debt);
    sunk.crisis_strikes(Acute::Freeze, 0.8, 20, 78);
    assert_eq!(sunk.state, was);
    assert!((sunk.debt - debt).abs() < 1e-12);
    sunk.advance(10, 1.0, 0.1);
    assert!(sunk.crisis.is_none());
    assert!(sunk.debt >= debt, "a crisis passing undid four years of it");
}
