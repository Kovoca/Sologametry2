//! **What is done here.**
//!
//! A value is a fact about a person; a norm is a fact about a place. The
//! interesting cases live in the gap, and the sharpest of them is that
//! **a witness cannot see that you did not know.**

use scale_sim::custom::{did_they_know, judged_here, places, Custom, Norm};

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
