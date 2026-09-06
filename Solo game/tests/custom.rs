//! **What is done here.**
//!
//! A value is a fact about a person; a norm is a fact about a place. The
//! interesting cases live in the gap, and the sharpest of them is that
//! **a witness cannot see that you did not know.**

use scale_sim::coping::{regulatory_capacity, Strain};
use scale_sim::custom::{
    did_they_know, judged_here, norms_of, places, will_bend, would_keep, Conditions, Custom,
    Norm,
};
use scale_sim::mind::{Facet, Mind, Value};
use scale_sim::rng::Rng;

fn a_polite_man(seed: u64) -> Mind {
    let mut m = Mind::draw(&mut Rng::new(seed), &[(Value::Law, 30)]);
    for f in Facet::ALL {
        m.person.set_baseline(f, 0.0);
    }
    m.person.set_baseline(Facet::Dutifulness, 1.2);
    m.willpower = 0.5;
    m.mood.valence = 0.0;
    m
}

/// **The same act is a breach in one place and correct in another.**
#[test]
fn one_act_two_verdicts() {
    let formal = places::old_country();
    let city = places::the_city();

    // Speaking to a stranger: expected in one, not done in the other.
    let spoke = 1.0;
    let here = judged_here(&formal, Norm::GreetStrangers, spoke);
    let there = judged_here(&city, Norm::GreetStrangers, spoke);
    assert!(here.against_them < 0.05, "greeting somebody was held against him");
    assert!(there.against_them > 0.3, "speaking to a stranger passed unremarked in the city");
}

/// **And staying silent flips it.**
#[test]
fn the_verdict_reverses_with_the_place() {
    let said_nothing = -1.0;
    assert!(judged_here(&places::old_country(), Norm::GreetStrangers, said_nothing).against_them > 0.3);
    assert!(judged_here(&places::the_city(), Norm::GreetStrangers, said_nothing).against_them < 0.05);
}

/// **Where nobody minds, there is nothing to breach.** Most places have
/// no opinion about most things, and a model where every norm is live
/// everywhere would make travel unbearable rather than interesting.
#[test]
fn a_norm_nobody_holds_cannot_be_broken() {
    let indifferent = Custom::new(&[(Norm::AcceptHospitality, 0.0)]);
    assert_eq!(
        judged_here(&indifferent, Norm::AcceptHospitality, -1.0).against_them,
        0.0
    );
}

/// **A witness cannot see that you did not know.**
///
/// The perception boundary, arriving at manners: a foreigner's innocent
/// breach and a local's deliberate rudeness are the same act, and the
/// person who took offence has no way to tell them apart.
#[test]
fn innocence_is_invisible_from_outside() {
    let here = places::the_city();
    let home = places::the_market();

    // He greets a stranger, as he would at home.
    let judged = judged_here(&here, Norm::GreetStrangers, 1.0);
    assert!(judged.against_them > 0.3, "it passed unremarked");

    // He had no idea: at home it is expected.
    assert!(did_they_know(&home, Norm::GreetStrangers));
    // And the verdict is identical either way, because the verdict is
    // reached from the act.
    let local_doing_the_same = judged_here(&here, Norm::GreetStrangers, 1.0);
    assert_eq!(judged, local_doing_the_same);
}

/// **Where two places differ is what makes somebody a foreigner** rather
/// than merely a stranger.
#[test]
fn two_places_can_be_named_as_different() {
    let d = places::old_country().differs_from(&places::the_city());
    assert!(!d.is_empty(), "two very different places agreed about everything");
    let names: Vec<Norm> = d.iter().map(|(n, _)| *n).collect();
    assert!(names.contains(&Norm::GreetStrangers));
    assert!(names.contains(&Norm::Directness));
    // And a place does not differ from itself.
    assert!(places::the_city().differs_from(&places::the_city()).is_empty());
}

/// **Haggling.** Expected in a market, insulting in a shop — which is
/// exactly the sort of thing that gets a traveller into trouble.
#[test]
fn the_price_is_a_conversation_in_some_places_and_not_others() {
    let market = places::the_market();
    let city = places::the_city();
    assert!(judged_here(&market, Norm::Haggle, 1.0).against_them < 0.05);
    assert!(judged_here(&city, Norm::Haggle, 1.0).against_them > 0.3);
    assert!(judged_here(&market, Norm::Haggle, -1.0).against_them > 0.3);
}

/// **Only a real breach is worth remarking on.** Somebody slightly off
/// is not somebody who insulted you.
#[test]
fn being_slightly_wrong_is_not_an_insult() {
    let formal = places::old_country();
    let slightly_off = judged_here(&formal, Norm::DeferToElders, 0.4);
    let openly_rude = judged_here(&formal, Norm::DeferToElders, -1.0);
    assert!(!slightly_off.worth_mentioning);
    assert!(openly_rude.worth_mentioning);
    assert!(openly_rude.against_them > slightly_off.against_them);
}

// =====================================================================
// a norm says what is expected; a person decides what they do
// =====================================================================

/// **A normally polite man in a foul mood is not polite**, and he has
/// not become a different person.
#[test]
fn a_bad_day_costs_somebody_their_manners() {
    let m = a_polite_man(1);
    let cap = regulatory_capacity(&m, 0.0);
    let d = m.person.z(Facet::Dutifulness) as f64;

    let ordinary = would_keep(0.9, d, 0.0, cap);
    let foul = would_keep(0.9, d, -1.0, cap);
    let cheerful = would_keep(0.9, d, 1.0, cap);

    assert!(foul < ordinary, "a terrible mood cost him nothing");
    assert!(cheerful > ordinary);
    // But it does not turn him rude. What erodes is the margin.
    assert!(foul > 0.3, "one bad day made a courteous man discourteous: {foul:.2}");
}

/// **Manners fail for the same reason tempers do.** Keeping a norm is an
/// act of self-control, so it runs on the capacity strain spends — which
/// is why somebody a year into a bad stretch is short with people who
/// have done nothing to them.
///
/// Two separate costs, and they compound. Which is the larger depends on
/// how bad the day and how long the year, and the model does not need an
/// opinion about that — only that both are real.
#[test]
fn strain_and_mood_both_cost_manners_and_they_compound() {
    let m = a_polite_man(3);
    let d = m.person.z(Facet::Dutifulness) as f64;

    let mut worn = Strain::default();
    worn.advance(1500, 0.9, 0.15);
    let fresh_cap = regulatory_capacity(&m, 0.0);
    let worn_cap = regulatory_capacity(&m, worn.debt);
    assert!(worn_cap < fresh_cap, "a wrecking year left his self-command untouched");

    let ordinary = would_keep(0.9, d, 0.0, fresh_cap);
    let bad_day = would_keep(0.9, d, -1.0, fresh_cap);
    let bad_year = would_keep(0.9, d, 0.0, worn_cap);
    let both = would_keep(0.9, d, -1.0, worn_cap);

    assert!(bad_day < ordinary, "a terrible day cost him nothing");
    assert!(bad_year < ordinary, "a wrecking year cost him nothing");
    assert!(
        both < bad_day && both < bad_year,
        "the two did not compound: {both:.2} against {bad_day:.2} and {bad_year:.2}"
    );
    // And still not rudeness. He is a courteous man having an awful time.
    assert!(both > 0.15, "he became a boor: {both:.2}");
}

/// **A good mood does not invent a custom.** Somebody who does not hold
/// a norm is not made to keep it by cheerfulness.
#[test]
fn cheerfulness_does_not_produce_manners_nobody_has() {
    let m = a_polite_man(5);
    let d = m.person.z(Facet::Dutifulness) as f64;
    assert!(would_keep(-0.8, d, 1.0, 1.0) <= 0.0);
}

/// **And the witness cannot tell the difference.** A man curt because he
/// had just heard something terrible is judged exactly as a man who is
/// simply curt.
#[test]
fn a_bad_day_and_a_bad_character_look_the_same() {
    let here = places::old_country();
    let m = a_polite_man(7);
    let d = m.person.z(Facet::Dutifulness) as f64;
    let cap = regulatory_capacity(&m, 0.0);

    // A courteous man having the worst day of his life.
    let curt_today = would_keep(0.9, d, -1.0, 0.15);
    // Somebody who simply does not bother.
    let just_rude = would_keep(-0.2, d, 0.0, cap);

    let a = judged_here(&here, Norm::ThankForCourtesy, curt_today);
    let b = judged_here(&here, Norm::ThankForCourtesy, just_rude);
    // They need not be equal — he was ruder or less so — but neither
    // carries any note about why, and the good man is still marked down.
    assert!(a.against_them > 0.0, "having a terrible day was a free pass");
    assert!(b.against_them > 0.0);
}

// =====================================================================
// ethics is a disposition, not a switch
// =====================================================================

/// **Somebody with less regard for the law bends more rules**, other
/// things equal.
#[test]
fn scruple_is_what_varies_between_people() {
    let straight = will_bend(45, 1.0, 0.6, 0.3);
    let loose = will_bend(-30, -1.0, 0.6, 0.3);
    assert!(loose > straight, "conviction made no difference at all");
    assert!(straight < 0.15, "a man of firm principle bent a rule for very little");
}

/// **What it is worth matters**, which is why almost nobody is honest
/// about everything and almost everybody is honest about most things.
#[test]
fn the_gain_is_part_of_it() {
    let trifle = will_bend(10, 0.0, 0.1, 0.3);
    let fortune = will_bend(10, 0.0, 1.0, 0.3);
    assert!(fortune > trifle);
}

/// **Certainty deters; severity mostly does not.**
///
/// One of the more robust findings in criminology, and the reason the
/// penalty is not a term in this at all. Being watched is what stops
/// people.
#[test]
fn being_seen_is_what_deters() {
    let unseen = will_bend(0, 0.0, 0.7, 0.0);
    let watched = will_bend(0, 0.0, 0.7, 1.0);
    assert!(
        unseen > watched * 3.0,
        "whether anybody was looking barely mattered: {unseen:.2} vs {watched:.2}"
    );
    // There is no penalty argument to pass, which is the point.
}

/// **The scrupulous are deterred least**, because they were not going to
/// anyway — so a watchman changes the behaviour of the people who were
/// wavering.
#[test]
fn a_watchman_changes_the_mind_of_the_undecided() {
    let scrupulous_change = will_bend(45, 1.0, 0.7, 0.0) - will_bend(45, 1.0, 0.7, 1.0);
    let wavering_change = will_bend(0, 0.0, 0.7, 0.0) - will_bend(0, 0.0, 0.7, 1.0);
    assert!(
        wavering_change > scrupulous_change,
        "watching had the same effect on a saint as on somebody in two minds"
    );
}

/// **Nobody is bent by nothing.** With no gain there is nothing to bend
/// a rule for, whatever somebody is like.
#[test]
fn there_has_to_be_something_in_it() {
    assert_eq!(will_bend(-50, -2.0, 0.0, 0.0), 0.0);
}

// =====================================================================
// nobody decides what is done here
// =====================================================================

/// **The custom follows the settlement.** Nothing is typed in: change
/// how many people live somewhere and what is done there changes with
/// it.
#[test]
fn how_many_people_there_are_decides_whether_you_greet_them() {
    let village = norms_of(&places::a_village());
    let city = norms_of(&places::a_metropolis());

    assert!(village.holds(Norm::GreetStrangers) > 0.3, "a village of four hundred kept to itself");
    assert!(city.holds(Norm::GreetStrangers) < -0.3, "eight million people all said good morning");

    // And it is the size doing it, not anything else about the place:
    // hold everything still and move only the population.
    let base = Conditions { population: 300.0, ..places::a_village() };
    let grown = Conditions { population: 900_000.0, ..base };
    assert!(norms_of(&base).holds(Norm::GreetStrangers) > norms_of(&grown).holds(Norm::GreetStrangers));
}

/// **A stranger is remarkable where anybody could know everybody**, and
/// the line is where the real one is: a hundred and fifty people is
/// about the most anybody keeps up with, and by fifty thousand it is
/// certainly gone.
#[test]
fn the_line_is_drawn_where_relationships_actually_stop() {
    let at = |pop: f64| {
        Conditions { population: pop, ..places::a_village() }.everybody_knows_everybody()
    };
    assert!(at(150.0) > 0.95, "a hamlet where nobody knew anybody");
    assert!(at(400.0) > 0.7);
    assert!(at(50_000.0) < 0.05);
    assert!(at(2_000_000.0) < 1e-9);
    // Monotone, which a divisor picked for convenience need not be.
    assert!(at(500.0) > at(5_000.0) && at(5_000.0) > at(40_000.0));
}

/// **Guest-right is strongest where travel is dangerous and there is no
/// inn** — which is a fact about remoteness and thin ground, not about
/// anybody's generosity.
#[test]
fn hospitality_comes_from_being_a_long_way_from_anywhere() {
    let outpost = norms_of(&places::a_desert_outpost());
    let city = norms_of(&places::a_metropolis());
    assert!(
        outpost.holds(Norm::AcceptHospitality) > city.holds(Norm::AcceptHospitality) + 0.5,
        "a desert outpost was no more hospitable than a metropolis"
    );

    // Hold the place still and move it closer to everything.
    let far = places::a_desert_outpost();
    let near = Conditions { remoteness: 0.0, ..far };
    assert!(norms_of(&far).holds(Norm::AcceptHospitality) > norms_of(&near).holds(Norm::AcceptHospitality));
}

/// **Personal space is larger where it is cold**, which is measured
/// across forty-two countries and tracks temperature rather than
/// character.
#[test]
fn the_climate_decides_how_close_anybody_stands() {
    let cold = Conditions { mean_temp_c: -5.0, ..places::a_market_town() };
    let hot = Conditions { mean_temp_c: 30.0, ..places::a_market_town() };
    assert!(norms_of(&cold).holds(Norm::KeepDistance) > norms_of(&hot).holds(Norm::KeepDistance));
}

/// **Haggling is what happens where the price is not posted**, and fixed
/// prices are an invention of scale retail that ends it wherever they
/// arrive.
#[test]
fn the_price_is_a_conversation_until_somebody_prints_it() {
    let market = norms_of(&places::a_market_town());
    let city = norms_of(&places::a_metropolis());
    assert!(market.holds(Norm::Haggle) > 0.3);
    assert!(city.holds(Norm::Haggle) < -0.3);

    // The same market town, grown into a city with department stores.
    let grown = Conditions { population: 3_000_000.0, ..places::a_market_town() };
    assert!(
        norms_of(&grown).holds(Norm::Haggle) < market.holds(Norm::Haggle),
        "haggling survived the arrival of a posted price"
    );
}

/// **Rules are held harder where the ground is thin and the people are
/// close together** — the tightness–looseness finding across thirty-three
/// nations, and it is ecology rather than preference.
#[test]
fn threat_and_crowding_tighten_the_rules() {
    let easy = Conditions { density: 100.0, scarcity: 0.05, remoteness: 0.1, ..Default::default() };
    let hard = Conditions { density: 5_000.0, scarcity: 0.9, remoteness: 0.9, ..Default::default() };
    assert!(hard.tightness() > easy.tightness() * 1.8);

    // Each of the three pulls on its own.
    let crowded = Conditions { density: 5_000.0, ..easy };
    let barren = Conditions { scarcity: 0.9, ..easy };
    let cut_off = Conditions { remoteness: 0.9, ..easy };
    for tighter in [crowded, barren, cut_off] {
        assert!(tighter.tightness() > easy.tightness());
    }
}

/// **Pace follows size and cold**, measured by walking speed, clock
/// accuracy and how long it takes to buy a stamp across thirty-one
/// countries.
#[test]
fn a_big_cold_place_runs_faster_than_a_small_warm_one() {
    let big_cold = Conditions { population: 5_000_000.0, mean_temp_c: 2.0, ..Default::default() };
    let small_warm = Conditions { population: 800.0, mean_temp_c: 28.0, ..Default::default() };
    assert!(big_cold.pace() > small_warm.pace() * 2.0);
    assert!(
        norms_of(&big_cold).holds(Norm::Punctuality) > norms_of(&small_warm).holds(Norm::Punctuality)
    );
}

/// **Age is deferred to where what an old person knows is still worth
/// knowing** — which is farming and craft, not a mobile industrial city.
#[test]
fn elders_are_deferred_to_where_their_knowledge_still_works() {
    let farming = norms_of(&places::a_village());
    let industrial = norms_of(&places::a_metropolis());
    assert!(farming.holds(Norm::DeferToElders) > industrial.holds(Norm::DeferToElders));
}

/// **And no two derived places are the same place**, which is the whole
/// point of deriving them.
#[test]
fn different_ground_produces_different_custom() {
    let all = [
        norms_of(&places::a_village()),
        norms_of(&places::a_metropolis()),
        norms_of(&places::a_market_town()),
        norms_of(&places::a_desert_outpost()),
    ];
    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            assert!(
                !all[i].differs_from(&all[j]).is_empty(),
                "two quite different places came out with identical custom"
            );
        }
    }
}
