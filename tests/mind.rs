//! **A person is not one happiness number.**
//!
//! `docs/mind-spec.md`, slice 1. What is asserted here is not that the
//! arithmetic works — it is that the *separations* hold, because they are
//! the argument.

use scale_sim::mind::{
    Appraisal, ConcernKind, ConcernState, Emotion, Episode, Facet, Happening, LoadingSource, Mind,
    Personality, Value, RELIABILITY,
};
use scale_sim::rng::Rng;

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

/// Somebody built to order: every facet middling except the named ones.
fn with(seed: u64, set: &[(Facet, f32)]) -> Mind {
    let mut m = a_mind(seed);
    for f in Facet::ALL {
        m.person.set_baseline(f, 0.0);
    }
    for &(f, z) in set {
        m.person.set_baseline(f, z);
    }
    m.willpower = 0.0;
    m
}

fn of(v: &[Episode], what: Emotion) -> f64 {
    v.iter().find(|e| e.what == what).map(|e| e.strength).unwrap_or(0.0)
}

fn corr(a: &[f32], b: &[f32]) -> f64 {
    let n = a.len() as f64;
    let ma = a.iter().map(|x| *x as f64).sum::<f64>() / n;
    let mb = b.iter().map(|x| *x as f64).sum::<f64>() / n;
    let (mut cov, mut va, mut vb) = (0.0, 0.0, 0.0);
    for i in 0..a.len() {
        let (x, y) = (a[i] as f64 - ma, b[i] as f64 - mb);
        cov += x * y;
        va += x * x;
        vb += y * y;
    }
    cov / (va * vb).sqrt()
}

// ---------------------------------------------------------------------
// 1. Big Five underneath, behavioural facets above
// ---------------------------------------------------------------------

/// **Twenty-five independent sliders are not a five-factor model.**
///
/// The whole content of the model is that facets covary *through their
/// parent domain*. Anger, anxiety, gloom and vulnerability to stress are
/// not four coin flips; they are four expressions of one thing. Drawing
/// them independently — which is what the first version did — produces a
/// list with five-factor names on it and none of the structure.
///
/// Real NEO-PI-R facet loadings run 0.5–0.75, so facets within a domain
/// correlate substantially and facets across domains barely at all.
#[test]
fn facets_covary_through_their_domain_or_it_is_not_a_five_factor_model() {
    let mut rng = Rng::new(20260902);
    let folk: Vec<Personality> = (0..800).map(|_| Personality::draw(&mut rng)).collect();
    let col = |f: Facet| -> Vec<f32> { folk.iter().map(|p| p.z(f)).collect() };

    let within = corr(&col(Facet::Anxiety), &col(Facet::Gloom));
    assert!(
        (0.35..0.80).contains(&within),
        "anxiety and gloom correlate at {within:+.2}; at zero they are two \
         unrelated sliders, at one they are the same slider twice"
    );

    let across = corr(&col(Facet::Anxiety), &col(Facet::Curiosity));
    assert!(
        across.abs() < 0.15,
        "anxiety and curiosity correlate at {across:+.2}, which is not two \
         different factors"
    );

    // **A negative loading is the model working.** Cruelty is low
    // agreeableness, so it runs against altruism without anybody wiring
    // it by hand.
    let opposed = corr(&col(Facet::Altruism), &col(Facet::Cruelty));
    assert!(opposed < -0.3, "altruism and cruelty correlate at {opposed:+.2}");

    // And a facet keeps variance of its own, or it is its domain under
    // another name: an anxious but even-tempered person exists.
    let odd = folk
        .iter()
        .filter(|p| p.z(Facet::Anxiety) > 1.0 && p.z(Facet::Anger) < 0.0)
        .count();
    assert!(
        odd > 15,
        "only {odd} of 800 people are anxious without being bad-tempered, \
         so the facet has no life of its own"
    );
}

/// **Four things change and only one of them is who they grew up to be.**
/// A core memory moves *durable adaptation*, not the baseline.
#[test]
fn life_bends_a_person_without_rewriting_them() {
    let mut p = Personality::draw(&mut Rng::new(5));
    let was = p.z(Facet::Anxiety);
    p.adapt(Facet::Anxiety, -0.4);
    assert!((p.z(Facet::Anxiety) - (was - 0.4)).abs() < 1e-5);

    for _ in 0..50 {
        p.adapt(Facet::Anxiety, -1.0);
    }
    assert!(
        p.z(Facet::Anxiety) > was - 1.6,
        "a life of adaptation replaced the person rather than bending them"
    );
}

// ---------------------------------------------------------------------
// 2. calibration, read carefully
// ---------------------------------------------------------------------

/// **Latent inside, presentation outside, and "extreme" defined.**
///
/// The first version stored the bounded 0–100 score and asserted a game's
/// neutral band on it. Three averaged uniforms put 43.2% inside 40–60, a
/// matched normal 45.1%, and the test failed at 44% while the code was
/// right — it was checking a number nobody had measured.
#[test]
fn personality_is_standard_normal_and_extreme_means_two_sigma() {
    let mut rng = Rng::new(1234);
    let z: Vec<f32> = (0..4000)
        .map(|_| Personality::draw(&mut rng).z(Facet::Anger))
        .collect();
    let n = z.len() as f64;
    let mean = z.iter().map(|x| *x as f64).sum::<f64>() / n;
    let sd = (z.iter().map(|x| (*x as f64 - mean).powi(2)).sum::<f64>() / n).sqrt();
    assert!(mean.abs() < 0.07, "mean z is {mean:+.3}");
    assert!((0.92..1.08).contains(&sd), "sd is {sd:.3}, so it is not standardised");

    let share = |k: f64| z.iter().filter(|x| (**x as f64).abs() <= k).count() as f64 / n;
    assert!(
        (share(1.0) - 0.683).abs() < 0.035,
        "{:.1}% within one sigma, not 68.3%",
        share(1.0) * 100.0
    );
    assert!(
        (share(2.0) - 0.954).abs() < 0.025,
        "{:.1}% within two sigma, not 95.4%",
        share(2.0) * 100.0
    );
    // **Extreme is |z| > 2.** Defined, so it is reproducible — "under 6%"
    // was not.
    let extreme = 1.0 - share(2.0);
    assert!((extreme - 0.046).abs() < 0.025, "{:.1}% extreme, not 4.6%", extreme * 100.0);
}

/// **Heritability is a population variance ratio, not a share of one
/// person.**
///
/// `personality = 0.5 × parents + 0.5 × environment` is a claim about an
/// individual and is meaningless. What twin estimates of 41–61% *(Jang et
/// al.)* license is a breeding value: mid-parent plus segregation noise,
/// phenotype on top, and then a **population correlation between
/// relatives** as the check. For heritability h², parent-offspring is
/// h²/2 — about 0.22 here, against measured figures of 0.15–0.20.
#[test]
fn relatives_correlate_at_about_half_the_heritability() {
    let mut rng = Rng::new(77);
    let (mut parent, mut child, mut sib_a, mut sib_b) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for _ in 0..3000 {
        let a = Personality::draw(&mut rng);
        let b = Personality::draw(&mut rng);
        let one = Personality::inherit(&mut rng, &a, &b);
        let two = Personality::inherit(&mut rng, &a, &b);
        parent.push(a.z(Facet::Anxiety));
        child.push(one.z(Facet::Anxiety));
        sib_a.push(one.z(Facet::Anxiety));
        sib_b.push(two.z(Facet::Anxiety));
    }
    let po = corr(&parent, &child);
    assert!(
        (0.12..0.32).contains(&po),
        "parent and child correlate at {po:+.2}; h²/2 is about 0.22 and \
         measured figures run 0.15-0.20"
    );
    let ss = corr(&sib_a, &sib_b);
    assert!(
        (0.12..0.36).contains(&ss),
        "siblings correlate at {ss:+.2}, which is either clones or strangers"
    );

    let n = child.len() as f64;
    let m = child.iter().map(|x| *x as f64).sum::<f64>() / n;
    let sd = (child.iter().map(|x| (*x as f64 - m).powi(2)).sum::<f64>() / n).sqrt();
    assert!((0.85..1.15).contains(&sd), "children's spread is {sd:.2}");
}

/// **r ≈ 0.6–0.7 is an observed coefficient over an interval, not an
/// annual retention rate.**
///
/// Applying 0.65 every year would destroy stability inside a decade. And
/// the published figures carry **measurement error**: observed = true ×
/// reliability, so a latent model held directly against them makes people
/// far less stable than they are. At a reliability of 0.80, a true
/// stability near 0.85 shows up as the ~0.68 the literature reports.
#[test]
fn rank_order_is_stable_across_twenty_years_at_the_measured_rate() {
    let mut rng = Rng::new(31415);
    let mut folk: Vec<Personality> = (0..2000).map(|_| Personality::draw(&mut rng)).collect();

    let before: Vec<f32> = folk.iter().map(|p| p.z(Facet::Anxiety)).collect();
    let seen_before: Vec<f32> = folk
        .iter()
        .map(|p| p.observed(Facet::Anxiety, &mut rng))
        .collect();

    for p in folk.iter_mut() {
        for _ in 0..20 {
            p.a_year_passes();
        }
        // Twenty years of whatever life durably did.
        for f in Facet::ALL {
            p.adapt(f, (rng.next_f32() - 0.5) * 2.0 * 0.62);
        }
    }

    let after: Vec<f32> = folk.iter().map(|p| p.z(Facet::Anxiety)).collect();
    let seen_after: Vec<f32> = folk
        .iter()
        .map(|p| p.observed(Facet::Anxiety, &mut rng))
        .collect();

    let latent = corr(&before, &after);
    assert!(
        (0.72..0.96).contains(&latent),
        "latent stability over twenty years is {latent:+.2}; people are \
         either unrecognisable or unchangeable"
    );

    // **The comparable number**, because it is the same quantity the
    // literature reports.
    let observed = corr(&seen_before, &seen_after);
    assert!(
        (0.52..0.80).contains(&observed),
        "observed stability is {observed:+.2}, against a measured 0.6-0.7"
    );
    assert!(
        observed < latent,
        "measurement error made people look *more* consistent than they are"
    );
    assert!(RELIABILITY < 1.0, "a perfect instrument does not exist");
}

/// **The maturity principle is a population tendency, and some people
/// must move against it.**
///
/// Sixteen longitudinal samples analysed together *(Graham et al.)* found
/// substantial heterogeneity, flattening and late-life reversals — so a
/// rule every actor obeys is not a tendency, it is a script.
#[test]
fn people_mature_on_average_and_plenty_do_not() {
    let mut rng = Rng::new(808);
    let mut folk: Vec<Personality> = (0..1500).map(|_| Personality::draw(&mut rng)).collect();
    for p in folk.iter_mut() {
        p.age_years = 25.0;
    }
    let before: Vec<f32> = folk.iter().map(|p| p.z(Facet::Dutifulness)).collect();
    let before_n: Vec<f32> = folk.iter().map(|p| p.z(Facet::Anxiety)).collect();
    for p in folk.iter_mut() {
        p.age_years = 55.0;
    }
    let after: Vec<f32> = folk.iter().map(|p| p.z(Facet::Dutifulness)).collect();
    let after_n: Vec<f32> = folk.iter().map(|p| p.z(Facet::Anxiety)).collect();

    let mean = |v: &Vec<f32>| v.iter().map(|x| *x as f64).sum::<f64>() / v.len() as f64;
    assert!(mean(&after) > mean(&before), "dutifulness did not rise across adulthood");
    assert!(mean(&after_n) < mean(&before_n), "anxiety did not fall across adulthood");

    let against =
        (0..folk.len()).filter(|&i| after[i] < before[i]).count() as f64 / folk.len() as f64;
    assert!(
        (0.12..0.46).contains(&against),
        "{:.0}% of people became less dutiful with age: either everybody \
         obeys the average, which is a script, or there is no tendency",
        against * 100.0
    );
}

// ---------------------------------------------------------------------
// 3. appraisal is the bridge
// ---------------------------------------------------------------------

/// **The trait does not say what somebody is angry about; the value does
/// not guarantee anger.** Their interaction produces the disposition.
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
    let mut pacifist = with(7, &[(Facet::Anger, 1.6), (Facet::Violence, -1.6)]);
    let mut brute = with(7, &[(Facet::Anger, 1.6), (Facet::Violence, 1.6)]);
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

    assert!(of(&p, Emotion::Anger) > 0.3, "the hot-tempered pacifist felt nothing");
    assert!(
        (of(&p, Emotion::Anger) - of(&b, Emotion::Anger)).abs() < 0.05,
        "the same temper produced different anger"
    );
    assert!(
        of(&p, Emotion::Outrage) > of(&b, Emotion::Outrage) + 0.2,
        "breaking a principle somebody holds produced no more outrage than \
         breaking one they do not"
    );
}

/// **One event, several emotions, and they may disagree.**
///
/// A promotion that went to a friend can produce at once: gladness that
/// they succeeded, envy at the relative status, frustration at a blocked
/// goal, resentment if the process was crooked, shame if it confirms
/// something already feared. Those stay separate even when their
/// valences fight.
#[test]
fn one_event_produces_several_emotions_and_they_may_disagree() {
    let promotion = Appraisal {
        severity: -0.3,
        to_me: 0.8,
        to_mine: 0.6,
        someone_gained: true,
        blocks_a_goal: true,
        control: 0.2,
        unexpected: 0.6,
        ..Default::default()
    };

    let envious = with(1, &[(Facet::Envy, 1.9)]);
    let driven = with(1, &[(Facet::Ambition, 1.9)]);
    let generous = with(
        1,
        &[(Facet::Envy, -1.9), (Facet::Ambition, -1.9), (Facet::Altruism, 1.9)],
    );

    let e = envious.appraise(&promotion);
    let d = driven.appraise(&promotion);
    let g = generous.appraise(&promotion);

    assert!(of(&e, Emotion::Envy) > of(&d, Emotion::Envy));
    assert!(of(&d, Emotion::Frustration) > of(&e, Emotion::Frustration));
    assert!(
        of(&g, Emotion::Joy) > of(&e, Emotion::Joy),
        "the generous one is not the gladder for their friend"
    );
    assert!(e.len() >= 3, "a happiness bar with extra steps");

    let crooked = Appraisal { unfair: 0.8, ..promotion };
    let fearful = Appraisal { confirms_a_fear: 0.8, ..promotion };
    assert!(
        of(&envious.appraise(&crooked), Emotion::Outrage) > of(&e, Emotion::Outrage) + 0.2,
        "a crooked process produced no more outrage than a fair one"
    );
    assert!(
        of(&envious.appraise(&fearful), Emotion::Shame) > of(&e, Emotion::Shame) + 0.2,
        "a defeat that confirms a fear produced no shame"
    );
}

// ---------------------------------------------------------------------
// 4. acute activation fades; the concern remains
// ---------------------------------------------------------------------

/// **Rage's activation is gone within days; the grievance is not.**
///
/// Duration is not a property of arousal. What lengthens an emotion is
/// its importance, its intensity, and the eliciting situation coming back
/// *(Verduyn et al.)* — so the model separates the burst from the thing
/// that keeps producing bursts.
#[test]
fn activation_fades_and_the_grievance_stays() {
    let mut rng = Rng::new(2);
    let mut m = with(11, &[(Facet::Anger, 1.5), (Facet::Vengefulness, 1.5)]);
    let insult = Appraisal {
        severity: -0.8,
        to_me: 1.0,
        deliberate: true,
        control: 0.1,
        unfair: 0.8,
        ..Default::default()
    };
    let felt = m.appraise(&insult);
    m.feel(felt);
    m.take_on(ConcernKind::Grievance, 0.9);

    let day_one = m.feeling_of(Emotion::Anger);
    assert!(day_one > 0.3, "an insult produced no anger");

    for _ in 0..7 {
        m.a_day_passes(&mut rng);
    }
    assert!(
        m.feeling_of(Emotion::Anger) < day_one,
        "the original rage never subsided"
    );

    // A year on, the grievance is still throwing off fresh anger.
    let mut angry_days = 0;
    for _ in 0..365 {
        m.a_day_passes(&mut rng);
        if m.feeling_of(Emotion::Anger) > 0.05 || m.feeling_of(Emotion::Resentment) > 0.05 {
            angry_days += 1;
        }
    }
    assert!(
        angry_days > 20,
        "only {angry_days} days of anger in a year from an unresolved \
         grievance, so nothing is being re-made"
    );
    assert!(
        angry_days < 330,
        "{angry_days} days of anger in a year, which is one long emotion \
         and not a grievance"
    );
    assert!(m.concerns[0].adaptation > 0.2, "no habituation at all in a year");
    // **"It hurts less and does not leave" is about depth, not daily
    // load.** A year on it may be going quiet — which is what
    // accommodation is — but the grievance itself is still there to be
    // touched, and a cue still reaches it.
    assert!(m.concerns[0].depth() > 0.15, "the grievance simply expired");
    let reminded = m.cued(0, 0.9).expect("nothing left to remind him of");
    assert!(
        reminded.strength > 0.1,
        "being reminded of it produced {:.2}",
        reminded.strength
    );
}

/// **Bereavement is recurrent waves, not one uninterrupted year of
/// sadness.**
#[test]
fn grief_comes_in_waves() {
    let mut rng = Rng::new(9);
    let mut m = with(13, &[(Facet::StressVulnerability, 0.8)]);
    m.take_on(ConcernKind::Bereavement, 0.95);

    let (mut heavy, mut quiet) = (0, 0);
    for _ in 0..365 {
        m.a_day_passes(&mut rng);
        if m.feeling_of(Emotion::Grief) > 0.15 || m.feeling_of(Emotion::Yearning) > 0.15 {
            heavy += 1;
        } else {
            quiet += 1;
        }
    }
    assert!(heavy > 15, "only {heavy} hard days in the year after a death");
    assert!(
        quiet > 100,
        "only {quiet} bearable days in the year after a death, which is one \
         permanent emotion rather than grief"
    );
}

/// **Stress, mood and focus are three different things** — and focus is
/// not untouched by chronic load either.
///
/// Both cases have to survive: grieving and functional, delighted and
/// temporarily useless. What takes focus first is acute activation and
/// intrusive recollection; chronic load exerts a smaller indirect penalty
/// through vigilance, rumination and exhaustion, because carrying
/// something indefinitely is not free.
#[test]
fn focus_is_taken_by_activation_first_and_by_load_a_little() {
    let mut rng = Rng::new(3);
    let mut bereaved = with(3, &[(Facet::StressVulnerability, 0.9)]);
    bereaved.willpower = 1.2;
    let loss = Appraisal {
        severity: -0.9,
        to_mine: 1.0,
        to_me: 0.3,
        control: 0.1,
        unexpected: 0.9,
        ..Default::default()
    };
    let felt = bereaved.appraise(&loss);
    assert!(felt.iter().any(|e| e.what == Emotion::Grief));
    bereaved.feel(felt);
    bereaved.take_on(ConcernKind::Bereavement, 0.9);
    for _ in 0..14 {
        bereaved.a_day_passes(&mut rng);
    }
    assert!(bereaved.stress.load > 0.05, "a fortnight and the grief cost nothing");
    assert!(
        bereaved.focus.current > 0.40,
        "a grieving person can concentrate on nothing, which is the case \
         this separation exists to allow"
    );

    // Delighted, unstressed, and briefly good for nothing.
    let mut elated = with(3, &[]);
    elated.willpower = 0.0;
    let good = Appraisal {
        severity: 0.95,
        to_me: 1.0,
        unexpected: 1.0,
        ..Default::default()
    };
    let felt = elated.appraise(&good);
    elated.feel(felt);
    elated.a_day_passes(&mut rng);
    assert!(
        elated.stress.load < bereaved.stress.load,
        "good news cost more than a death"
    );
    assert!(
        elated.focus.current < 0.85,
        "the best news of somebody's life did not distract them at all"
    );
}

// ---------------------------------------------------------------------

/// **An individual departs from their culture**, or nothing can change.
#[test]
fn people_do_not_all_agree_with_their_own_culture() {
    let mut rng = Rng::new(4242);
    let culture = a_settled_culture();
    let folk: Vec<Mind> = (0..400).map(|_| Mind::draw(&mut rng, &culture)).collect();
    let mean_peace: f64 = folk
        .iter()
        .map(|m| m.conviction(Value::Peace) as f64)
        .sum::<f64>()
        / folk.len() as f64;
    assert!(
        (mean_peace - 30.0).abs() < 7.0,
        "a peaceable culture averaged {mean_peace:.0} on peace"
    );
    let dissenters = folk
        .iter()
        .filter(|m| m.values.iter().any(|c| c.heterodoxy() > 20))
        .count();
    assert!(dissenters > folk.len() / 10, "only {dissenters} disagree with anything");
}

/// A seed rebuilds the same head.
#[test]
fn the_same_seed_draws_the_same_person() {
    let a = a_mind(99);
    let b = a_mind(99);
    assert_eq!(a.person, b.person);
    assert_eq!(a.values, b.values);
    let ev = Appraisal {
        severity: -0.5,
        to_me: 1.0,
        deliberate: true,
        ..Default::default()
    };
    assert_eq!(a.appraise(&ev), b.appraise(&ev));
}

// ---------------------------------------------------------------------
// gate tests
// ---------------------------------------------------------------------

/// **A designed mapping must never be mistaken for a measured
/// coefficient.**
///
/// NEO-PI-R has thirty facets, six to a domain; this is a subset plus
/// extensions. Cruelty, violence, vengefulness and greed are not NEO
/// facets at all — their negative agreeableness loadings are sensible
/// modelling and nothing more. Recording that costs nothing now and stops
/// a later document claiming otherwise.
#[test]
fn every_loading_says_where_it_came_from() {
    use LoadingSource::*;
    assert_eq!(Facet::Anxiety.provenance(), MeasuredNeo);
    assert_eq!(Facet::Trust.provenance(), MeasuredNeo);
    for f in [Facet::Cruelty, Facet::Violence, Facet::Vengefulness, Facet::Greed] {
        assert_eq!(
            f.provenance(),
            DesignedExtension,
            "{} is not a NEO facet and must not be recorded as one",
            f.name()
        );
    }
    // The subset is smaller than the instrument it draws on.
    assert!(
        Facet::ALL.len() < 30,
        "this claims to be all thirty NEO facets, which it is not"
    );
}

/// **Two independent observations of an unchanged person correlate at the
/// reliability**, which checks the error generator itself rather than
/// letting the twenty-year result come out right by accident.
///
/// The standardised form is what makes this exact:
/// `observed = √R × latent + √(1−R) × noise`. Adding raw noise to the
/// latent score instead inflates the variance and yields 0.83 where 0.80
/// was wanted.
#[test]
fn two_observations_of_the_same_person_correlate_at_the_reliability() {
    let mut rng = Rng::new(5150);
    let folk: Vec<Personality> = (0..4000).map(|_| Personality::draw(&mut rng)).collect();
    let once: Vec<f32> = folk.iter().map(|p| p.observed(Facet::Anxiety, &mut rng)).collect();
    let twice: Vec<f32> = folk.iter().map(|p| p.observed(Facet::Anxiety, &mut rng)).collect();

    let r = corr(&once, &twice);
    assert!(
        (r - RELIABILITY as f64).abs() < 0.04,
        "two readings of an unchanged population correlate at {r:.3}, not \
         the {RELIABILITY} the instrument claims"
    );

    // An observation against the truth: √R.
    let latent: Vec<f32> = folk.iter().map(|p| p.z(Facet::Anxiety)).collect();
    let against_truth = corr(&once, &latent);
    assert!(
        (against_truth - (RELIABILITY as f64).sqrt()).abs() < 0.04,
        "a reading correlates with the truth at {against_truth:.3}, not √R"
    );

    // And it is standardised, or none of the above is exact.
    let n = once.len() as f64;
    let m = once.iter().map(|x| *x as f64).sum::<f64>() / n;
    let sd = (once.iter().map(|x| (*x as f64 - m).powi(2)).sum::<f64>() / n).sqrt();
    assert!((0.94..1.06).contains(&sd), "observations have spread {sd:.3}, not 1");
}

/// **Two people assign different unfairness to the same event.**
///
/// The perception boundary, and the reason it has to hold *before* slice
/// 2 rather than after. If the happening itself carried `unfair`, then
/// everybody who heard about it would inherit the same moral conclusion —
/// and there would be no room for two witnesses to disagree, for a rumour
/// to be wrong, or for the person who made the decision to think it
/// perfectly proper.
#[test]
fn two_people_read_the_same_event_differently() {
    // The manager chose Alice. Bob was also a candidate; Carol was not.
    let choice = Happening {
        severity: -0.4,
        someone_gained: true,
        by_a_decision: true,
        deliberate: true,
        control: 0.1,
        unexpected: 0.5,
        to_me: 0.0,
        ..Default::default()
    };
    let passed_over = Happening { to_me: 0.9, blocks_a_goal: true, ..choice };

    let bob = with(21, &[(Facet::Gloom, 1.2), (Facet::Trust, -1.2)]);
    let mut carol = with(21, &[]);
    for c in carol.values.iter_mut() {
        if c.topic == Value::Fairness {
            c.held = 20;
        }
    }

    let bobs = bob.read(&passed_over);
    let carols = carol.read(&choice);

    assert!(
        bobs.unfair > 0.3,
        "the man passed over reads no unfairness at all ({:.2})",
        bobs.unfair
    );
    assert!(
        carols.unfair < 0.1,
        "a bystander with no stake reads the promotion as crooked ({:.2})",
        carols.unfair
    );
    assert!(
        bobs.confirms_a_fear > carols.confirms_a_fear,
        "a defeat confirmed nothing for the man who lost and something for \
         the woman who was not in the running"
    );

    // **The facts are shared and the verdict is not.**
    assert_eq!(bobs.severity, carols.severity);
    assert_ne!(bobs.unfair, carols.unfair);

    // And the same man, less inclined to think ill of himself, reads it
    // differently again — which is the point of it being his and not the
    // event's.
    let sanguine = with(21, &[(Facet::Gloom, -1.5), (Facet::Pride, 1.5), (Facet::Trust, 1.2)]);
    assert!(
        sanguine.read(&passed_over).confirms_a_fear < bobs.confirms_a_fear,
        "a confident man read his own defeat as confirming a fear just as \
         much as a gloomy one"
    );
}

/// **A long life does not accumulate unbounded permanent stress.**
///
/// The failure this guards is quiet and would only show up decades in:
/// every loss, grievance and abandoned goal staying live for ever, so an
/// elderly person carries fifty of them at once and is permanently at
/// breaking point. What actually happens is accommodation — the
/// attachment and the memory stay, and the *daily burden* goes.
///
/// A dormant loss must still answer a cue, or nothing would ever hurt on
/// an anniversary.
#[test]
fn old_accommodated_losses_go_quiet_without_going_away() {
    let mut rng = Rng::new(1919);
    let mut m = with(31, &[(Facet::StressVulnerability, 0.5)]);

    // A long life: a dozen serious losses and grievances, spread out.
    for k in 0..12 {
        m.take_on(
            if k % 2 == 0 { ConcernKind::Bereavement } else { ConcernKind::Grievance },
            0.8,
        );
        for day in 0..600 {
            // **An ordinary life, lived.** Before needs existed this loop
            // was twenty years of a man doing literally nothing — no
            // company, no family, no rest, no work — and once neglect
            // took focus that quite correctly left him unable to
            // concentrate. What the test is about is old *losses*, so the
            // ordinary living has to be held still.
            use scale_sim::needs::{Circumstances, Doing};
            if let Some(n) = m.needs.as_mut() {
                let known = Circumstances { with_somebody_known: true, ..Default::default() };
                let kin = Circumstances { with_kin: true, ..Default::default() };
                let taking_part = Circumstances { taking_part: true, ..Default::default() };
                n.did(Doing::TalkWithAFriend, 1.0, known);
                n.did(Doing::SitWithFamily, 2.0, kin);
                n.did(Doing::WorkTheLoom, 8.0, Circumstances::default());
                n.did(Doing::Sleep, 8.0, Circumstances::default());
                n.did(Doing::WalkInTheFields, 0.5, Circumstances::default());
                if day % 7 == 0 {
                    n.did(Doing::AttendTheService, 1.0, taking_part);
                }
                if day % 30 == 0 {
                    n.did(Doing::ReadABook, 2.0, Circumstances::default());
                }
                if day % 90 == 0 {
                    n.did(Doing::Feast, 4.0, taking_part);
                }
            }
            m.a_day_passes(&mut rng);
        }
    }

    let dormant = m
        .concerns
        .iter()
        .filter(|c| c.state == ConcernState::Dormant)
        .count();
    assert!(
        dormant >= 10,
        "only {dormant} of twelve old losses ever went quiet, so a long \
         life is a permanently rising burden"
    );

    // Carrying them all costs something, and not everything.
    let daily: f64 = m.concerns.iter().map(|c| c.pressure()).sum();
    assert!(
        daily < 1.0,
        "twelve accommodated losses still weigh {daily:.2} every day"
    );
    assert!(
        !m.overloaded(),
        "an ordinary long life left somebody permanently past breaking point"
    );
    assert!(m.focus.current > 0.5, "and unable to concentrate on anything, for ever");

    // **But nothing was deleted.** The oldest loss still answers.
    assert!(m.concerns[0].depth() > 0.1, "an old loss stopped existing");
    let anniversary = m.cued(0, 1.0).expect("an anniversary reached nothing");
    assert!(
        anniversary.strength > 0.1,
        "the anniversary of a death produced a feeling of {:.2}",
        anniversary.strength
    );
    assert_eq!(
        m.concerns[0].state,
        ConcernState::Active,
        "a strong cue did not wake a dormant loss"
    );
}
