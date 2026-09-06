//! **What a person is when nobody is looking at them.**
//!
//! Slice 9 of `docs/mind-spec.md`, sections 25 and 26. One claim carries
//! the file: **changing fidelity must not change the person.** Where it
//! cannot hold exactly, the divergence is named, its direction is
//! asserted, and it is bounded.

use scale_sim::coping::{
    Acute, ActualControl, ControlAppraisal, Coping, Defence, Demands, FunctionalDomain,
    FunctionalState,
};
use scale_sim::growth::{ShapesPersonality, ShapesWellbeing};
use scale_sim::id::{Arena, Id};
use scale_sim::mind::{Facet, Value, ADAPTATION_LIMIT};
use scale_sim::person::{Person, Trade};
use scale_sim::scaling::{
    demote, promote, Change, Coarse, Fidelity, Habits, PersonOrigin, Standing, What,
    GENERATION_SCHEMA,
};

fn who(n: u32) -> Id<Person> {
    let mut folk: Arena<Person> = Arena::new();
    let mut last = folk.add(Person::new("x", Trade::Labourer, 0, 0.0));
    for _ in 0..n {
        last = folk.add(Person::new("x", Trade::Labourer, 0, 0.0));
    }
    last
}

fn culture() -> Vec<(Value, i8)> {
    vec![(Value::Family, 25), (Value::Law, 20)]
}

fn a_trouble(severity: f64) -> Standing {
    Standing {
        severity,
        since: 0,
        worsens_if_ignored: 0.5,
        actual: ActualControl::default(),
    }
}

/// Somebody with one standing trouble, at day zero.
fn a_life(seed: u64, severity: f64) -> Coarse {
    let mut c = Coarse::new(who(3), seed, 0);
    c.standing.push(a_trouble(severity));
    c.perceived_control = ControlAppraisal { source: 0.2, consequences: 0.3, own_response: 0.5 };
    c
}

// =====================================================================
// the invariant
// =====================================================================

/// **Two years in detail and two years in one step are the same two
/// years.**
///
/// The headline. Without it a man's mind depends on whether the engine
/// happened to be looking at him.
#[test]
fn a_coarse_advance_matches_a_life_lived_day_by_day() {
    for (days, severity) in [(730u64, 0.8), (400, 0.5), (3000, 0.95), (120, 0.25)] {
        let cult = culture();
        let start = a_life(7, severity);

        let mut fine = promote(&start, &cult, 0);
        for _ in 0..days {
            fine.a_day_passes(0.2);
        }

        let mut coarse = start.clone();
        let reference = promote(&start, &cult, 0);
        coarse.advance_to(days, &reference.mind, 0.2);

        assert_eq!(
            coarse.strain.state, fine.strain.state,
            "state diverged over {days} days at severity {severity}"
        );
        assert!(
            (coarse.strain.debt - fine.strain.debt).abs() < 1e-9,
            "debt diverged: {} against {}",
            coarse.strain.debt,
            fine.strain.debt
        );
        assert!(
            (coarse.strain.history.lifetime_days as i64
                - fine.strain.history.lifetime_days as i64)
                .abs()
                <= 2
        );
    }
}

/// **And the habits come out the same**, which is what makes a distant
/// person still recognisably themselves when they are picked up again.
#[test]
fn habits_do_not_depend_on_how_often_anybody_looked() {
    let cult = culture();
    let start = a_life(9, 0.75);
    let mut fine = promote(&start, &cult, 0);
    for _ in 0..900 {
        fine.a_day_passes(0.2);
    }
    let mut coarse = start.clone();
    let reference = promote(&start, &cult, 0);
    coarse.advance_to(900, &reference.mind, 0.2);

    assert_eq!(coarse.habits.strongest(), fine.habits.strongest());
    for k in Coping::ALL {
        assert!(
            (coarse.habits.of(k) - fine.habits.of(k)).abs() < 1e-4,
            "{k:?} came out at {} coarse against {} fine",
            coarse.habits.of(k),
            fine.habits.of(k)
        );
    }
}

/// **A record can be put down and picked up at any point** without that
/// showing in the result.
#[test]
fn putting_somebody_down_and_picking_them_up_changes_nothing() {
    let cult = culture();
    let start = a_life(11, 0.7);

    let mut straight = start.clone();
    let reference = promote(&start, &cult, 0);
    straight.advance_to(1000, &reference.mind, 0.2);

    // The same thousand days, interrupted three times.
    let mut broken = start.clone();
    for at in [250u64, 600, 1000] {
        let d = promote(&broken, &cult, broken.last_update);
        let mut c = demote(&d);
        c.advance_to(at, &d.mind, 0.2);
        broken = c;
    }

    assert_eq!(straight.strain.state, broken.strain.state);
    assert!((straight.strain.debt - broken.strain.debt).abs() < 1e-6);
    assert_eq!(straight.habits.strongest(), broken.habits.strongest());
}

/// **A round trip is the identity.**
#[test]
fn demoting_a_promotion_gives_the_record_back() {
    let cult = culture();
    let mut c = a_life(13, 0.6);
    c.growth
        .shaped_personality(Facet::Anxiety, ShapesPersonality::Trauma, 1.0, 1.0, 10);
    c.growth.took_a_role(Facet::Dutifulness, 0.25, 20);
    c.advance_to(500, &promote(&c, &cult, 0).mind, 0.2);

    let there_and_back = demote(&promote(&c, &cult, 500));
    assert_eq!(there_and_back.strain, c.strain);
    assert_eq!(there_and_back.origin, c.origin);
    assert_eq!(there_and_back.standing.len(), c.standing.len());
    assert_eq!(there_and_back.growth.episodics.len(), c.growth.episodics.len());
    assert_eq!(there_and_back.growth.roles.len(), c.growth.roles.len());
    assert_eq!(there_and_back.habits.strongest(), c.habits.strongest());
}

// =====================================================================
// generated, never stored
// =====================================================================

/// **A personality is redrawn from its seed, not saved.** That is what
/// makes a distant person cheap, and it is the project's oldest rule
/// arriving at the mind.
#[test]
fn a_personality_is_regenerated_and_not_carried() {
    let cult = culture();
    let c = a_life(17, 0.4);
    let first = promote(&c, &cult, 0);
    let second = promote(&c, &cult, 0);
    for f in Facet::ALL {
        assert_eq!(first.z(f), second.z(f), "{f:?} came back different");
    }
    // A different seed is a different man.
    let other = promote(&a_life(18, 0.4), &cult, 0);
    assert!(
        Facet::ALL.iter().any(|&f| (other.z(f) - first.z(f)).abs() > 0.1),
        "two different seeds produced the same person"
    );
}

/// **What life did comes back with them**, because growth is dated and
/// analytic rather than a running total.
#[test]
fn what_happened_to_somebody_survives_being_put_away() {
    let cult = culture();
    let mut c = a_life(19, 0.3);
    let untouched = promote(&c, &cult, 0).z(Facet::Anxiety);

    c.growth
        .shaped_personality(Facet::Anxiety, ShapesPersonality::Trauma, 1.0, 1.0, 100);
    let after = promote(&c, &cult, 200).z(Facet::Anxiety);
    assert!(after > untouched + 0.05, "a trauma did not survive the journey");

    // And it is still fading correctly years later, without anybody
    // having ticked it.
    let much_later = promote(&c, &cult, 200 + 365 * 6).z(Facet::Anxiety);
    assert!(much_later < after, "nothing faded while nobody was looking");
    assert!(much_later > untouched, "it faded away to nothing");
}

/// **Well-being travels too**, and stays in its own layer.
#[test]
fn a_wellbeing_injury_is_carried_and_not_confused_with_a_trait() {
    let cult = culture();
    let mut c = a_life(21, 0.3);
    let traits_before: Vec<f32> = Facet::ALL.iter().map(|&f| promote(&c, &cult, 0).z(f)).collect();

    c.growth.shaped_wellbeing(ShapesWellbeing::LostWork, 1.0, -1.0, 50);
    let d = promote(&c, &cult, 100);
    assert!(d.mind.wellbeing_baseline < -0.1, "the injury did not come back with him");
    for (i, &f) in Facet::ALL.iter().enumerate() {
        assert!(
            (d.z(f) - traits_before[i]).abs() < 1e-6,
            "losing work moved {f:?} across a demotion"
        );
    }
}

// =====================================================================
// the named divergence
// =====================================================================

/// **Where it cannot be exact, say so and bound it.**
///
/// A coarse record holds one pressure for the whole interval. Strain
/// accumulates faster than it recovers, so a man whose pressure swung
/// either side of his tolerance is *worse off* than the same average
/// steadily applied — and the coarse path therefore always understates
/// the damage. The direction never varies, which is what makes it
/// safe to name rather than a lurking bug.
#[test]
fn pressure_is_averaged_and_that_understates_it() {
    let cult = culture();
    let start = a_life(23, 0.5);

    // Lived: alternating a hard fortnight and an easy one.
    let mut fine = promote(&start, &cult, 0);
    for day in 0..1460u32 {
        let hard = (day / 14) % 2 == 0;
        fine.standing[0].severity = if hard { 0.9 } else { 0.1 };
        fine.a_day_passes(0.5);
    }

    // Recorded: the mean of the two, which is exactly his tolerance.
    let mut coarse = start.clone();
    coarse.standing[0].severity = 0.5;
    let reference = promote(&start, &cult, 0);
    coarse.advance_to(1460, &reference.mind, 0.5);

    assert!(
        fine.strain.debt > coarse.strain.debt,
        "averaging the pressure made him worse off, which is the wrong direction"
    );
    assert!(
        coarse.strain.debt < 1e-9,
        "an average exactly at tolerance still accumulated"
    );
    assert!(
        fine.strain.debt > 0.3,
        "a swinging life did him no harm at all, so there is nothing to understate"
    );
}

// =====================================================================
// what is kept and what is thrown away
// =====================================================================

/// **The record holds what a promotion needs and no more.**
#[test]
fn the_coarse_record_carries_the_named_list() {
    let c = a_life(29, 0.4);
    // Debt and severe-duration; state and how long in it.
    let _ = (c.strain.debt, c.strain.history.lifetime_days);
    let _ = (c.strain.state, c.strain.days_in_state);
    // Habits, unresolved stressors, outstanding attempts, perceived
    // control, expected support, and when this was last brought up.
    let _: Habits = c.habits;
    let _: &Vec<Standing> = &c.standing;
    let _: u16 = c.attempts_outstanding;
    let _: ControlAppraisal = c.perceived_control;
    let _: f64 = c.support_expected;
    let _: u64 = c.last_update;
    // The origin, and the saved baseline the seed cannot be trusted
    // to reproduce.
    let _: PersonOrigin = c.origin;
    let _: &Vec<f32> = &c.baseline;
}

/// **A save grows with what mattered in a life, not with its length.**
#[test]
fn a_long_life_does_not_grow_the_record_without_bound() {
    let cult = culture();
    let mut c = a_life(31, 0.3);
    for year in 0..70u64 {
        c.growth.shaped_personality(
            Facet::Anxiety,
            ShapesPersonality::CoreMemory,
            0.5,
            if year % 2 == 0 { 1.0 } else { -1.0 },
            year * 365,
        );
    }
    let before = c.growth.episodics.len();
    let today = 70 * 365;
    let z_before = promote(&c, &cult, today).z(Facet::Anxiety);

    c.compact(today);
    assert!(c.growth.episodics.len() < before, "seventy years compacted to nothing");
    let z_after = promote(&c, &cult, today).z(Facet::Anxiety);
    assert!(
        (z_after - z_before).abs() < 0.02,
        "compacting changed who he is now: {z_before:.3} -> {z_after:.3}"
    );
}

/// **Demotion is lossy, and honestly so.** Particular afternoons go; what
/// shaped him stays, because a core memory is already a growth entry.
#[test]
fn a_distant_person_keeps_what_shaped_them_and_not_the_afternoons() {
    let cult = culture();
    let mut c = a_life(37, 0.3);
    c.growth
        .shaped_personality(Facet::Anxiety, ShapesPersonality::Trauma, 1.0, 1.0, 10);

    let mut d = promote(&c, &cult, 20);
    // A loaded person accumulates episodes; a record does not carry them.
    d.mind.episodes.push(scale_sim::mind::Episode {
        what: scale_sim::mind::Emotion::Joy,
        strength: 0.8,
        activation: 0.5,
        age_days: 0.0,
        about: None,
    });
    let back = demote(&d);
    let again = promote(&back, &cult, 20);

    assert!(again.mind.episodes.is_empty(), "a passing mood survived a demotion");
    assert!(
        !back.growth.episodics.is_empty(),
        "the thing that shaped him did not survive"
    );
    assert!(again.z(Facet::Anxiety) > 0.05, "he came back unmarked by it");
}

/// The three tiers are ordered, so a caller can compare them.
#[test]
fn fidelity_is_ordered_from_closest_to_furthest() {
    assert!(Fidelity::Loaded < Fidelity::Settlement);
    assert!(Fidelity::Settlement < Fidelity::Distant);
}

/// **A distant person still gets worse**, which is the whole reason to
/// keep a record rather than freeze them.
#[test]
fn nobody_is_preserved_by_being_ignored() {
    let cult = culture();
    let mut c = a_life(41, 0.9);
    let reference = promote(&c, &cult, 0);
    assert_eq!(c.strain.state, FunctionalState::Regulated);
    c.advance_to(365 * 3, &reference.mind, 0.15);
    assert!(
        c.strain.state >= FunctionalState::Depleted,
        "three years of it happened to somebody nobody was watching and did nothing"
    );
}

/// **And a distant person recovers**, for the same reason.
#[test]
fn somebody_out_of_sight_can_also_get_better() {
    let cult = culture();
    let mut c = a_life(43, 0.9);
    let reference = promote(&c, &cult, 0);
    c.advance_to(365 * 4, &reference.mind, 0.15);
    assert!(c.strain.state >= FunctionalState::Depleted);

    c.standing[0].severity = 0.05;
    c.advance_to(365 * 12, &reference.mind, 0.4);
    assert_eq!(
        c.strain.state,
        FunctionalState::Regulated,
        "his troubles ended and nobody told him"
    );
    assert!(
        c.strain.history.lifetime_days > 365,
        "the years he lost left no record"
    );
}

// =====================================================================
// piecewise events — the semigroup property
// =====================================================================

/// **Constant-pressure invariance is necessary and not sufficient.**
///
/// An analytic step is only valid while nothing changes, so the interval
/// has to be split at every discontinuity. This is the real requirement:
/// a life with things happening in it, lived day by day, must equal the
/// same life advanced through the same schedule in a handful of jumps.
#[test]
fn a_life_with_events_in_it_advances_the_same_either_way() {
    let cult = culture();
    let start = a_life(101, 0.7);
    let reference = promote(&start, &cult, 0);

    let schedule = vec![
        Change { day: 100, what: What::SupportChanges(0.9) },
        Change {
            day: 180,
            what: What::StressorBegins(Standing {
                severity: 0.5,
                since: 180,
                worsens_if_ignored: 0.8,
                actual: ActualControl::default(),
            }),
        },
        Change { day: 240, what: What::StressorEnds },
        Change {
            day: 300,
            what: What::ControlChanges(ControlAppraisal {
                source: 0.8,
                consequences: 0.8,
                own_response: 0.6,
            }),
        },
        Change { day: 365, what: What::SupportChanges(0.1) },
    ];

    // Lived: a day at a time, applying each change on its own day.
    let mut fine = start.clone();
    for day in 1..=500u64 {
        for c in schedule.iter().filter(|c| c.day == day) {
            fine.advance_to(day, &reference.mind, 0.2);
            let mut one = vec![*c];
            one[0].day = day;
            fine.advance_through(&reference.mind, 0.2, &one);
        }
        fine.advance_to(day, &reference.mind, 0.2);
    }

    // Advanced: through the schedule in jumps, then on to the same day.
    let mut coarse = start.clone();
    coarse.advance_through(&reference.mind, 0.2, &schedule);
    coarse.advance_to(500, &reference.mind, 0.2);

    assert_eq!(coarse.strain.state, fine.strain.state, "an eventful year diverged");
    assert!(
        (coarse.strain.debt - fine.strain.debt).abs() < 1e-9,
        "debt diverged: {} against {}",
        coarse.strain.debt,
        fine.strain.debt
    );
    assert_eq!(coarse.standing.len(), fine.standing.len());
    assert!((coarse.support_expected - fine.support_expected).abs() < 1e-12);
}

/// **The semigroup property**, stated directly: advancing by `a + b` is
/// advancing by `a` and then by `b`, given the same schedule.
#[test]
fn advancing_composes() {
    let cult = culture();
    let start = a_life(103, 0.65);
    let reference = promote(&start, &cult, 0);
    let schedule = vec![
        Change { day: 90, what: What::SupportChanges(0.8) },
        Change { day: 400, what: What::SupportChanges(0.2) },
    ];

    let mut whole = start.clone();
    whole.advance_through(&reference.mind, 0.2, &schedule);
    whole.advance_to(900, &reference.mind, 0.2);

    for cut in [1u64, 90, 240, 400, 899] {
        let mut split = start.clone();
        let early: Vec<Change> = schedule.iter().copied().filter(|c| c.day <= cut).collect();
        let late: Vec<Change> = schedule.iter().copied().filter(|c| c.day > cut).collect();
        split.advance_through(&reference.mind, 0.2, &early);
        split.advance_to(cut, &reference.mind, 0.2);
        split.advance_through(&reference.mind, 0.2, &late);
        split.advance_to(900, &reference.mind, 0.2);

        assert_eq!(split.strain.state, whole.strain.state, "splitting at {cut} diverged");
        assert!(
            (split.strain.debt - whole.strain.debt).abs() < 1e-9,
            "splitting at {cut} gave {} against {}",
            split.strain.debt,
            whole.strain.debt
        );
    }
}

/// **A fresh trouble is felt through the one relapse consumer**, and
/// only there. Somebody with four bad years behind them takes a new blow
/// harder than somebody who has never been down.
#[test]
fn a_new_stressor_is_appraised_through_the_history() {
    let cult = culture();
    let mut veteran = a_life(105, 0.9);
    let reference = promote(&veteran, &cult, 0);
    veteran.advance_to(1500, &reference.mind, 0.1);
    veteran.standing.clear();
    veteran.advance_to(1500 + 2000, &reference.mind, 0.5);

    let mut newcomer = a_life(105, 0.0);
    newcomer.standing.clear();
    newcomer.last_update = veteran.last_update;

    let fresh = Standing {
        severity: 0.5,
        since: veteran.last_update,
        worsens_if_ignored: 0.5,
        actual: ActualControl::default(),
    };
    let at = veteran.last_update + 1;
    veteran.advance_through(&reference.mind, 0.4, &[Change { day: at, what: What::StressorBegins(fresh) }]);
    newcomer.advance_through(&reference.mind, 0.4, &[Change { day: at, what: What::StressorBegins(fresh) }]);

    assert!(
        veteran.standing[0].severity > newcomer.standing[0].severity,
        "a man with a history felt a fresh blow no harder than anybody else"
    );
}

/// **A crisis reaches a distant person too**, without promoting their
/// chronic state.
#[test]
fn a_crisis_can_strike_somebody_nobody_is_watching() {
    let cult = culture();
    let mut c = a_life(107, 0.2);
    let reference = promote(&c, &cult, 0);
    c.advance_through(
        &reference.mind,
        0.5,
        &[Change { day: 50, what: What::Crisis { kind: Acute::Panic, activation: 0.9, because_of: 3 } }],
    );
    assert!(c.strain.crisis.is_some(), "nothing happened to him at all");
    assert_eq!(c.strain.state, FunctionalState::Regulated, "one bad hour broke him");

    c.advance_to(60, &reference.mind, 0.5);
    assert!(c.strain.crisis.is_none(), "it was still running ten days later");
}

/// **The domains come back with them.** A distant person's record is
/// enough to say which parts of their life are going badly.
#[test]
fn a_record_can_still_say_which_part_of_a_life_is_failing() {
    let cult = culture();
    let mut c = a_life(109, 0.35);
    let reference = promote(&c, &cult, 0);
    c.advance_to(350, &reference.mind, 0.15);
    let d = Demands { work: 0.9, caregiving: 0.5, social: 0.4, self_care: 0.4 };
    assert!(
        c.strain.functioning_in(FunctionalDomain::Work, &d, &Defence::default())
            > c.strain.functioning_in(FunctionalDomain::Caregiving, &d, &Defence::default()),
        "out of sight, every part of his life failed together"
    );
}

// =====================================================================
// the second gate review
// =====================================================================

/// **Compaction must preserve the future, not only today.**
///
/// A sum of decays with different half-lives is not one decay, so an
/// entry that agrees now can disagree tomorrow. What is exact is
/// splitting each contribution into the part that will never move again
/// and the part still fading, and aggregating them separately.
#[test]
fn compacting_preserves_every_future_value() {
    let cult = culture();
    let mut c = a_life(201, 0.2);
    // Mixed causes, mixed signs, mixed half-lives and residuals.
    for (i, &m) in [
        ShapesPersonality::Trauma,
        ShapesPersonality::CoreMemory,
        ShapesPersonality::SustainedTreatment,
    ]
    .iter()
    .enumerate()
    {
        for k in 0..6u64 {
            c.growth.shaped_personality(
                Facet::Anxiety,
                m,
                0.6,
                if (i as u64 + k) % 2 == 0 { 1.0 } else { -1.0 },
                k * 400 + i as u64 * 90,
            );
        }
    }
    c.growth.shaped_wellbeing(ShapesWellbeing::LostWork, 1.0, -1.0, 300);
    c.growth.shaped_wellbeing(ShapesWellbeing::Bereavement, 1.0, -1.0, 900);

    let today = 365 * 12;
    let before: Vec<(u64, f32, f32)> = [0u64, 1, 365, 365 * 20, 365 * 60]
        .iter()
        .map(|&d| {
            let t = today + d;
            (t, c.growth.raw_for(Facet::Anxiety, t), c.growth.wellbeing(t))
        })
        .collect();

    let n_before = c.growth.episodics.len();
    c.compact(today);
    assert!(c.growth.episodics.len() < n_before, "nothing was coalesced at all");

    for (t, anx, well) in before {
        assert!(
            (c.growth.raw_for(Facet::Anxiety, t) - anx).abs() < 1e-4,
            "the trait diverged at day {t}: {} against {anx}",
            c.growth.raw_for(Facet::Anxiety, t)
        );
        assert!(
            (c.growth.wellbeing(t) - well).abs() < 1e-4,
            "well-being diverged at day {t}"
        );
    }
    let _ = cult;
}

/// **Compacting twice is compacting once**, and it does not creep.
#[test]
fn repeated_compaction_is_stable() {
    let mut c = a_life(203, 0.2);
    for k in 0..10u64 {
        c.growth
            .shaped_personality(Facet::Gloom, ShapesPersonality::Trauma, 0.5, 1.0, k * 300);
    }
    let today = 365 * 10;
    c.compact(today);
    let once: Vec<f32> = (0..5).map(|i| c.growth.raw_for(Facet::Gloom, today + i * 700)).collect();
    let n = c.growth.episodics.len();
    for _ in 0..5 {
        c.compact(today);
    }
    assert_eq!(c.growth.episodics.len(), n, "compaction kept growing the record");
    for (i, v) in once.iter().enumerate() {
        assert!((c.growth.raw_for(Facet::Gloom, today + i as u64 * 700) - v).abs() < 1e-5);
    }
}

/// **What is hidden behind the ceiling survives compaction**, or the
/// saturation-reveal bug comes back through the back door.
#[test]
fn compaction_keeps_what_the_clamp_is_hiding() {
    let mut c = a_life(205, 0.2);
    c.growth.took_a_role(Facet::Dutifulness, 2.5, 0);
    for k in 0..8u64 {
        c.growth
            .shaped_personality(Facet::Dutifulness, ShapesPersonality::Trauma, 1.0, 1.0, k * 200);
    }
    let today = 365 * 12;
    let raw_before = c.growth.raw_for(Facet::Dutifulness, today);
    assert!(raw_before > ADAPTATION_LIMIT, "nothing was hidden to begin with");
    c.compact(today);
    assert!(
        (c.growth.raw_for(Facet::Dutifulness, today) - raw_before).abs() < 1e-4,
        "compaction threw away what the clamp was hiding"
    );
}

/// **An ordinary life does not end pinned against the bound.**
///
/// Every durable change leaves a permanent residue, so without
/// diminishing plasticity an ordinary century eventually presses almost
/// everybody to ±1.5 — and the safety clamp becomes the mechanism again.
#[test]
fn an_ordinary_life_does_not_saturate_a_trait() {
    let mut pinned = 0;
    let people = 40;
    for seed in 0..people as u64 {
        let mut c = a_life(300 + seed, 0.2);
        // Sixty adult years, a few durable things per decade, mostly in
        // one direction because life is not symmetrical.
        for year in 0..60u64 {
            if year % 3 == 0 {
                let up = (seed + year) % 4 != 0;
                c.growth.shaped_personality(
                    Facet::Anxiety,
                    ShapesPersonality::CoreMemory,
                    0.8,
                    if up { 1.0 } else { -1.0 },
                    year * 365,
                );
            }
        }
        let end = 60 * 365;
        if c.growth.expressed_for(Facet::Anxiety, end).abs() > ADAPTATION_LIMIT - 0.05 {
            pinned += 1;
        }
    }
    assert!(
        pinned * 4 < people,
        "{pinned} of {people} ordinary lives ended pinned at the personality bound"
    );
}

/// **A seed is not sufficient save state.** The baseline is saved, so a
/// record reloads the same person even if the generator that first drew
/// them would now draw somebody else.
#[test]
fn a_saved_baseline_survives_a_changed_generator() {
    let cult = culture();
    let mut c = a_life(207, 0.2);
    // What the world drew for him, kept.
    let drawn = promote(&c, &cult, 0);
    let baseline: Vec<f32> = Facet::ALL.iter().map(|&f| drawn.mind.person.baseline_of(f)).collect();
    c.baseline = baseline.clone();

    // The generator changes under us: a different seed is a different
    // draw, which stands in for a new facet or a changed loading.
    c.origin = PersonOrigin { seed: 999_999, schema: GENERATION_SCHEMA + 1, born: 0 };
    let reloaded = promote(&c, &cult, 0);
    for (i, &f) in Facet::ALL.iter().enumerate() {
        assert!(
            (reloaded.mind.person.baseline_of(f) - baseline[i]).abs() < 1e-9,
            "{f:?} changed when the generator did"
        );
    }
}

/// **And the record says which generator made somebody**, so a save can
/// tell whether its seed still means anything.
#[test]
fn a_record_knows_which_generator_made_it() {
    let c = a_life(209, 0.2);
    assert_eq!(c.origin.schema, GENERATION_SCHEMA);
    assert_eq!(c.origin.seed, 209);
}

/// **A demotion carries the baseline out with it**, so a round trip
/// never re-derives who somebody was.
#[test]
fn demotion_saves_the_baseline() {
    let cult = culture();
    let c = a_life(211, 0.3);
    let d = promote(&c, &cult, 0);
    let back = demote(&d);
    assert_eq!(back.baseline.len(), Facet::ALL.len());
    for (i, &f) in Facet::ALL.iter().enumerate() {
        assert!((back.baseline[i] - d.mind.person.baseline_of(f)).abs() < 1e-9);
    }
}

/// **Same-time events must not depend on storage order.**
#[test]
fn two_things_happening_at_once_are_order_invariant() {
    let cult = culture();
    let start = a_life(213, 0.4);
    let reference = promote(&start, &cult, 0);
    let a = Change { day: 200, what: What::SupportChanges(0.9) };
    let b = Change {
        day: 200,
        what: What::ControlChanges(ControlAppraisal {
            source: 0.7,
            consequences: 0.7,
            own_response: 0.5,
        }),
    };

    let mut one = start.clone();
    one.advance_through(&reference.mind, 0.2, &[a, b]);
    one.advance_to(600, &reference.mind, 0.2);
    let mut other = start.clone();
    other.advance_through(&reference.mind, 0.2, &[b, a]);
    other.advance_to(600, &reference.mind, 0.2);

    assert!((one.strain.debt - other.strain.debt).abs() < 1e-12);
    assert!((one.support_expected - other.support_expected).abs() < 1e-12);
    assert_eq!(one.perceived_control, other.perceived_control);
}

/// **An event at the very start, at the very end, and no time at all.**
#[test]
fn the_boundaries_of_an_interval_behave() {
    let cult = culture();
    let start = a_life(215, 0.4);
    let reference = promote(&start, &cult, 0);

    // At the starting instant.
    let mut at_start = start.clone();
    at_start.advance_through(
        &reference.mind,
        0.2,
        &[Change { day: 0, what: What::SupportChanges(0.11) }],
    );
    assert!((at_start.support_expected - 0.11).abs() < 1e-12, "an event on day zero was lost");

    // Zero days.
    let mut still = start.clone();
    let before = still.strain;
    still.advance_to(still.last_update, &reference.mind, 0.2);
    assert_eq!(still.strain, before, "advancing by nothing changed something");

    // At the ending instant, then no further.
    let mut at_end = start.clone();
    at_end.advance_through(
        &reference.mind,
        0.2,
        &[Change { day: 300, what: What::SupportChanges(0.22) }],
    );
    assert_eq!(at_end.last_update, 300);
    assert!((at_end.support_expected - 0.22).abs() < 1e-12);
}

/// **Coping has to actually do something.**
///
/// Found by running the thing rather than by reading it: the coarse
/// advance chose a strategy every chunk and never settled it, so coping
/// was decorative — six very different people met one bad year and all
/// six ended at the ceiling, identical. The whole of slice 8 sat unused
/// behind slice 9's loop.
#[test]
fn what_somebody_reaches_for_changes_where_they_end_up() {
    let cult = culture();
    // One trouble, real but not fixable by force: what differs is the
    // person meeting it.
    let trouble = Standing {
        severity: 0.8,
        since: 0,
        worsens_if_ignored: 0.8,
        actual: ActualControl { source: 0.1, consequences: 0.5, exit: 0.1, means: 0.4 },
    };

    let mut ends: Vec<f64> = Vec::new();
    for seed in 0..12u64 {
        let mut c = Coarse::new(who(3), 700 + seed, 0);
        c.standing.push(trouble);
        c.perceived_control =
            ControlAppraisal { source: 0.5, consequences: 0.5, own_response: 0.5 };
        let me = promote(&c, &cult, 0);
        // **Their own tolerance**, which is drawn from who they are. A
        // fixed one for everybody puts the whole cast at the ceiling and
        // hides exactly what this is asking about.
        let tol = me.mind.stress.tolerance;
        c.advance_to(365 * 3, &me.mind, tol);
        ends.push(c.strain.debt);
    }
    let worst = ends.iter().cloned().fold(f64::MIN, f64::max);
    let best = ends.iter().cloned().fold(f64::MAX, f64::min);
    assert!(
        worst - best > 0.3,
        "twelve different people met one bad year and came out within {:.2} of each other",
        worst - best
    );
}

/// **And what it did teaches them something**, at a rate that does not
/// depend on how often anybody looked.
#[test]
fn what_coping_taught_does_not_depend_on_the_camera() {
    let cult = culture();
    let mut c = Coarse::new(who(3), 4242, 0);
    c.standing.push(Standing {
        severity: 0.8,
        since: 0,
        worsens_if_ignored: 0.6,
        actual: ActualControl { source: 0.0, consequences: 0.05, exit: 0.0, means: 0.5 },
    });
    c.perceived_control = ControlAppraisal { source: 0.9, consequences: 0.9, own_response: 0.5 };
    let me = promote(&c, &cult, 0);

    let mut daily = c.clone();
    for day in 1..=730u64 {
        daily.advance_to(day, &me.mind, 0.3);
    }
    let mut coarse = c.clone();
    coarse.advance_to(730, &me.mind, 0.3);

    assert!(
        (daily.perceived_control.source - coarse.perceived_control.source).abs() < 1e-9,
        "belief drifted apart: {} against {}",
        daily.perceived_control.source,
        coarse.perceived_control.source
    );
    assert!((daily.strain.debt - coarse.strain.debt).abs() < 1e-9);
}
