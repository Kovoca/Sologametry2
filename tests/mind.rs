//! **A person is not one happiness number.**
//!
//! `docs/mind-spec.md`, slice 1. What is being asserted here is not that
//! the arithmetic works â€” it is that the *separations* hold, because they
//! are the whole argument. A facet is not a value; stress is not mood is
//! not focus; and one event produces several emotions that may disagree.

use scale_sim::mind::{Appraisal, Emotion, Facets, Mind, Value};
use scale_sim::rng::Rng;

/// A culture that holds peace and law dear, so heterodoxy has something
/// to be heterodox about.
fn a_settled_culture() -> Vec<(Value, i8)> {
    vec![
        (Value::Peace, 30),
        (Value::Law, 30),
        (Value::Family, 25),
        (Value::Craftsmanship, 20),
    ]
}

fn a_mind(seed: u64) -> Mind {
    Mind::draw(&mut Rng::new(seed), &a_settled_culture())
}

/// Give a mind the facets a test needs and leave the rest middling.
fn with(seed: u64, edit: impl Fn(&mut Facets)) -> Mind {
    let mut m = a_mind(seed);
    for v in [
        &mut m.facets.anger,
        &mut m.facets.anxiety,
        &mut m.facets.envy,
        &mut m.facets.ambition,
        &mut m.facets.pride,
        &mut m.facets.vengefulness,
        &mut m.facets.violence,
        &mut m.facets.cheerfulness,
        &mut m.facets.gratitude,
        &mut m.facets.dutifulness,
        &mut m.facets.depression_propensity,
        &mut m.facets.stress_vulnerability,
        &mut m.facets.excitement_seeking,
        &mut m.facets.curiosity,
    ] {
        *v = 50;
    }
    edit(&mut m.facets);
    m
}

/// **The same event, three people, three different heads.**
///
/// The specification's own example: a friend is promoted over you. There
/// is no correct emotion to have about that. An envious person is
/// envious, an ambitious one is frustrated, and one who is neither is
/// mildly pleased for their friend â€” and a real person is often all
/// three at once, which is what mixed feelings *are*.
///
/// A single happiness number cannot express any of this. It can only go
/// down by ten.
#[test]
fn one_event_produces_several_emotions_and_they_may_disagree() {
    let promotion = Appraisal {
        severity: -0.3,
        to_me: 0.8,
        to_mine: 0.4,
        someone_gained: true,
        control: 0.2,
        unexpected: 0.6,
        ..Default::default()
    };

    let envious = with(1, |f| f.envy = 92);
    let driven = with(1, |f| f.ambition = 95);
    let easy = with(1, |f| {
        f.envy = 8;
        f.ambition = 10;
    });

    let e = envious.appraise(&promotion);
    let d = driven.appraise(&promotion);
    let g = easy.appraise(&promotion);

    let of = |v: &Vec<scale_sim::mind::Felt>, what: Emotion| {
        v.iter().find(|f| f.what == what).map(|f| f.strength).unwrap_or(0.0)
    };

    assert!(
        of(&e, Emotion::Envy) > of(&d, Emotion::Envy),
        "the envious person is not the more envious one"
    );
    assert!(
        of(&d, Emotion::Frustration) > of(&e, Emotion::Frustration),
        "the ambitious person is not the more frustrated one"
    );
    assert!(
        of(&g, Emotion::Envy) < 0.3 && of(&g, Emotion::Frustration) < 0.4,
        "somebody neither envious nor ambitious is eaten up about it anyway"
    );

    // **Mixed, not resolved.** The envious one feels more than one thing
    // about it, and that is the point of the whole exercise.
    assert!(
        e.len() >= 2,
        "one event produced exactly one emotion, which is a happiness bar \
         with extra steps"
    );
}

/// **A facet is a weight, not a command.**
///
/// Two people with identical anger and opposite convictions do not do the
/// same thing. The angry one who holds peace and law dear is *outraged* â€”
/// they will shout, threaten to report you, walk out â€” and the one who
/// holds neither has nothing pulling the other way.
///
/// This is why values and facets have to be separate layers. Collapse
/// them and a hot-tempered pacifist is not expressible.
#[test]
fn identical_tempers_and_opposite_convictions_are_different_people() {
    let struck = Appraisal {
        severity: -0.7,
        to_me: 1.0,
        deliberate: true,
        control: 0.3,
        touches: Some((Value::Peace, false)),
        ..Default::default()
    };

    let mut pacifist = with(7, |f| {
        f.anger = 85;
        f.violence = 15;
    });
    let mut brute = with(7, |f| {
        f.anger = 85;
        f.violence = 90;
    });
    for c in pacifist.values.iter_mut() {
        if c.topic == Value::Peace {
            c.held = 45;
        }
    }
    for c in brute.values.iter_mut() {
        if c.topic == Value::Peace {
            c.held = -20;
        }
    }

    let p = pacifist.appraise(&struck);
    let b = brute.appraise(&struck);
    let of = |v: &Vec<scale_sim::mind::Felt>, what: Emotion| {
        v.iter().find(|f| f.what == what).map(|f| f.strength).unwrap_or(0.0)
    };

    // Both are angry â€” the temper is the same and the model must not
    // pretend the pacifist is serene.
    assert!(of(&p, Emotion::Anger) > 0.3, "the hot-tempered pacifist felt nothing");
    assert!(
        (of(&p, Emotion::Anger) - of(&b, Emotion::Anger)).abs() < 0.05,
        "the same temper produced different anger"
    );
    // What differs is that one of them has had a principle broken.
    assert!(
        of(&p, Emotion::Outrage) > of(&b, Emotion::Outrage) + 0.2,
        "breaking a value the person actually holds produced no more \
         outrage than breaking one they do not"
    );
}

/// **Stress, mood and focus are three different things.**
///
/// The structural claim the specification is most emphatic about, and the
/// one that most separates this from a happiness bar. Both of these are
/// possible and neither is expressible with one number:
///
/// - **heavily loaded and completely focused** â€” a grieving parent
///   nursing a sick child;
/// - **content and unable to concentrate** â€” somebody stirred up by good
///   news.
///
/// Grief is low arousal and lasts; rage is high arousal and does not.
/// That asymmetry is why the first case exists at all.
#[test]
fn stress_and_focus_are_not_the_same_axis() {
    let loss = Appraisal {
        severity: -0.9,
        to_mine: 1.0,
        to_me: 0.3,
        control: 0.1,
        unexpected: 0.9,
        ..Default::default()
    };
    let mut bereaved = with(3, |f| f.stress_vulnerability = 70);
    bereaved.attributes.willpower = 80;
    let felt = bereaved.appraise(&loss);
    assert!(
        felt.iter().any(|f| f.what == Emotion::Grief),
        "a death in the family produced no grief"
    );
    bereaved.feel(felt);

    // A fortnight on: still carrying it, and able to work.
    for _ in 0..14 {
        bereaved.a_day_passes();
    }
    assert!(
        bereaved.stress.load > 0.05,
        "a fortnight and the grief has cost nothing at all"
    );
    assert!(
        bereaved.feeling_of(Emotion::Grief) > 0.02,
        "grief evaporated in a fortnight; only high-arousal feelings do that"
    );
    assert!(
        bereaved.focus.current > 0.5,
        "a grieving person cannot concentrate on anything, which is not \
         true and is the case this separation exists for"
    );

    // And the other way: good news, no stress, badly distracted.
    let mut elated = with(3, |_| {});
    let good = Appraisal {
        severity: 0.9,
        to_me: 1.0,
        unexpected: 1.0,
        ..Default::default()
    };
    let felt = elated.appraise(&good);
    elated.feel(felt);
    elated.a_day_passes();
    assert!(
        elated.stress.load < bereaved.stress.load,
        "good news cost more stress than a death"
    );
    assert!(
        elated.focus.current < 0.8,
        "somebody who has just had the best news of their life is \
         concentrating perfectly"
    );
}

/// **Most people are middling.** A world where everybody has three
/// extreme traits is a collection of caricatures.
///
/// Real personality is approximately normally distributed, which is what
/// the three-uniform draw gives: about two thirds inside 40-60 and an
/// extreme trait genuinely uncommon.
#[test]
fn a_population_is_mostly_unremarkable() {
    let mut rng = Rng::new(20260902);
    let culture = a_settled_culture();
    let folk: Vec<Mind> = (0..400).map(|_| Mind::draw(&mut rng, &culture)).collect();

    let angers: Vec<f64> = folk.iter().map(|m| m.facets.anger as f64).collect();
    let n = angers.len() as f64;
    let mean = angers.iter().sum::<f64>() / n;
    let sd = (angers.iter().map(|a| (a - mean).powi(2)).sum::<f64>() / n).sqrt();
    assert!(
        (mean - 50.0).abs() < 4.0,
        "the population's average temper is {mean:.0}, not the middle"
    );

    // **Measured in standard deviations, not in a band off a game's
    // tables.** "Most people are 40-60" describes DF's own scale; the
    // real statistical claim is that personality is approximately normal,
    // so about two thirds sit within one SD of the mean and an extreme
    // trait is genuinely uncommon rather than merely less likely.
    //
    // Asserting 40-60 was asserting a *tighter* distribution than exists:
    // at SD 16.7 that band is 0.6 SD wide and holds 45% of a normal, and
    // the test failed at 44% while the code was right.
    assert!(
        (12.0..22.0).contains(&sd),
        "the spread of tempers is {sd:.1}: either everybody is the same \
         person or the world is a collection of caricatures"
    );
    let within_one = angers
        .iter()
        .filter(|a| (**a - mean).abs() <= sd)
        .count() as f64
        / n;
    assert!(
        (0.60..0.78).contains(&within_one),
        "{:.0}% of people are within one standard deviation, so the \
         distribution is not normal",
        within_one * 100.0
    );
    let extreme = angers.iter().filter(|a| **a > 85.0 || **a < 15.0).count() as f64 / n;
    assert!(
        extreme < 0.06,
        "{:.0}% of the population has an extreme temper",
        extreme * 100.0
    );
}

/// **An individual departs from their culture**, or there is no room for
/// a heretic, a reformer, or a criminal who thinks they are in the right.
#[test]
fn people_do_not_all_agree_with_their_own_culture() {
    let mut rng = Rng::new(4242);
    let culture = a_settled_culture();
    let folk: Vec<Mind> = (0..300).map(|_| Mind::draw(&mut rng, &culture)).collect();

    // The culture is reproduced on average...
    let mean_peace: f64 = folk
        .iter()
        .map(|m| m.conviction(Value::Peace) as f64)
        .sum::<f64>()
        / folk.len() as f64;
    assert!(
        (mean_peace - 30.0).abs() < 6.0,
        "a peaceable culture averaged {mean_peace:.0} on peace"
    );

    // ...and somebody in it does not hold with it at all.
    let dissenters = folk
        .iter()
        .filter(|m| m.values.iter().any(|c| c.heterodoxy() > 20))
        .count();
    assert!(
        dissenters > folk.len() / 10,
        "only {dissenters} of {} people disagree with their culture about \
         anything, so nothing can ever change",
        folk.len()
    );
}

/// A seed rebuilds the same head, like everything else here.
#[test]
fn the_same_seed_draws_the_same_person() {
    let a = a_mind(99);
    let b = a_mind(99);
    assert_eq!(a.facets, b.facets);
    assert_eq!(a.attributes, b.attributes);
    assert_eq!(a.values, b.values);

    let ev = Appraisal {
        severity: -0.5,
        to_me: 1.0,
        deliberate: true,
        ..Default::default()
    };
    assert_eq!(a.appraise(&ev), b.appraise(&ev));
}

