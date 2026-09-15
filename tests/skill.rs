//! **How good the hands are, and what that makes.**
//!
//! `person.rs` has had a full skill model since it was written: eleven
//! occupational skills, levels 0-10 on a quadratic anchored at ten
//! thousand hours to mastery, a ceiling set by aptitude so *legendary is
//! rare because the ability to get there is rare*, and rust at a rate that
//! costs a decade away a couple of levels rather than all of them.
//!
//! **Its only consumer was that person's own wage.** A skilled man earned
//! more and produced exactly what an unskilled one did, because `econ.rs`
//! had no skill term anywhere in it. `person.rs` states the claim that
//! made false: *labour share of output is 50-60%, so a day's pay tracks a
//! day's production value* — which cannot be true with only the pay side
//! varying.

use scale_sim::econ::{Commodity, Doctrine, SiteKind};
use scale_sim::person::Trade;
use scale_sim::slice;

/// Run a country for a while with its workforce pegged at one level, and
/// report what it made.
fn made_with_hands(level: Option<f64>, days: u64) -> f64 {
    let mut e = slice::build(Doctrine::Prudent);
    if let Some(l) = level {
        for m in 0..e.markets.len() {
            for t in Trade::ALL {
                e.set_hands(m, t, l);
            }
        }
    }
    // **Batches actually run**, summed over the whole period. The direct
    // measure of what a country made, and exactly the quantity skill is
    // supposed to move — stock on hand would net off consumption and say
    // something else entirely.
    let mut ran = 0.0;
    for _ in 0..days {
        e.step();
        ran += e.ledger.sites.iter().map(|s| s.ran).sum::<f64>();
        // The economy is stepped, not the populace, so nothing overwrites
        // what was set above.
        if let Some(l) = level {
            for m in 0..e.markets.len() {
                for t in Trade::ALL {
                    e.set_hands(m, t, l);
                }
            }
        }
    }
    ran
}

/// **A works staffed to standard makes exactly what it made before any of
/// this existed.**
///
/// The gate the whole change hangs on. Every recipe in this model is
/// calibrated against rated throughput, so if a workforce at the level its
/// trade wants produced anything other than the rating, this would have
/// silently moved the calibration of the entire economy — and the way to
/// find that out is not to run the suite and hope.
///
/// That is why production is a **ratio** against an ordinary hand
/// rather than `craft::Maker::pace` itself: `pace` at a competent hand is
/// 0.74, so using it directly would have cut every works in the world by a
/// quarter and called it a skill model.
#[test]
fn a_workforce_at_standard_leaves_the_calibration_alone() {
    let untouched = made_with_hands(None, 60);

    // Pegging every trade at exactly what it wants must be the same world.
    let mut e = slice::build(Doctrine::Prudent);
    for m in 0..e.markets.len() {
        for t in Trade::ALL {
            e.set_hands(m, t, 5.0);
        }
    }
    let mut pegged = 0.0;
    for _ in 0..60 {
        e.step();
        pegged += e.ledger.sites.iter().map(|s| s.ran).sum::<f64>();
        for m in 0..e.markets.len() {
            for t in Trade::ALL {
                e.set_hands(m, t, 5.0);
            }
        }
    }

    let drift = (pegged - untouched).abs() / untouched.max(1e-9);
    assert!(
        drift < 1e-9,
        "a workforce at the level its trade wants made {pegged:.3} against \
         {untouched:.3} untouched — {:.4}% adrift. The default is supposed \
         to *be* the wanted level, so these are the same world.",
        drift * 100.0
    );
}

/// **A country of novices makes less than a country of masters.**
///
/// The claim, and the direction is the whole of it. Sabotage: drop the
/// `hands_work_at` factor out of `produce` and the three readings collapse
/// to one.
#[test]
fn a_better_workforce_makes_more() {
    let novice = made_with_hands(Some(1.0), 60);
    let standard = made_with_hands(Some(5.0), 60);
    let master = made_with_hands(Some(8.0), 60);

    assert!(
        novice < standard && standard < master,
        "novice {novice:.0}, standard {standard:.0}, master {master:.0} — \
         skill is not reaching production"
    );

    // And the size of it is real rather than merely ordered. `pace` runs
    // 0.35 + 0.65 x skill against a competent hand, so the spread between
    // a dabbler and a master is substantial and is nowhere near a factor
    // of ten: a master is better, not superhuman.
    let spread = master / novice;
    assert!(
        (1.05..=3.0).contains(&spread),
        "a master workforce made {spread:.2}x what a novice one did, which \
         is not a plausible range for the same plant and the same inputs"
    );
}

/// **Nobody works at a trade the site has no use for.**
///
/// `SiteKind::worked_by` is an exhaustive match precisely so a new kind of
/// works cannot compile until somebody has said who works there. This
/// checks the answers are sane rather than merely present — a hospital run
/// by hauliers would compile perfectly.
#[test]
fn every_kind_of_works_is_staffed_by_somebody_plausible() {
    use SiteKind::*;
    let cases = [
        (Farm, Trade::Labourer),
        (Mill, Trade::Labourer),
        (Steelworks, Trade::Labourer),
        (PowerPlant, Trade::Labourer),
        (Builders, Trade::Builder),
        (Hospital, Trade::Nurse),
        (Shop, Trade::Shopworker),
        (Depot, Trade::Haulier),
    ];
    for (kind, want) in cases {
        assert_eq!(
            kind.worked_by(),
            want,
            "{kind:?} is worked by {:?}, which is not the trade that does \
             that job",
            kind.worked_by()
        );
    }

    // And every trade named must be one whose skill is the one practised
    // by doing the work — or a works would raise a skill nobody there has.
    for t in Trade::ALL {
        assert_eq!(
            Trade::ALL[t.index()],
            t,
            "the trade roster and the exhaustive index disagree at {t:?}, \
             so a variant has been added to one and not the other"
        );
    }
}

/// **The roster cannot quietly lose a trade.**
///
/// `Trade::ALL` is a hand-written list and `Trade::index` is an exhaustive
/// match. The match is what the compiler enforces; this is what holds the
/// list to it.
#[test]
fn the_trade_roster_is_complete() {
    assert_eq!(Trade::ALL.len(), 13, "a trade has been added or removed");
    for (i, t) in Trade::ALL.iter().enumerate() {
        assert_eq!(
            t.index(),
            i,
            "{t:?} sits at {i} in the roster and says its index is {}",
            t.index()
        );
    }
}

/// **A town with no individuated people behind it is not a town of
/// novices.**
///
/// The default matters as much as the mechanism: an economy built without
/// a populace — every hand-built fixture, and a bare region — has nothing
/// to read a skill level off, and reading that as zero would have made
/// every such world produce a fraction of what it does.
#[test]
fn an_economy_with_no_people_in_it_is_staffed_to_standard() {
    let e = slice::build(Doctrine::Prudent);
    for t in Trade::ALL {
        assert_eq!(
            e.hands_at(0, t),
            5.0,
            "{t:?} in a world with no sampled people reads as {} rather \
             than the ordinary practised hand a rated throughput assumes",
            e.hands_at(0, t)
        );
    }
    // And it is genuinely producing, or the assertion above is about a
    // world where nothing happens.
    let mut e = e;
    let mut ran = 0.0;
    for _ in 0..10 {
        e.step();
        ran += e.ledger.sites.iter().map(|s| s.ran).sum::<f64>();
    }
    assert!(ran > 0.0, "this fixture produced nothing at all in ten days");
    // Food is what the fixture is for.
    assert!(
        e.markets
            .iter()
            .any(|m| m.price[Commodity::ProcessedFood as usize] > 0.0),
        "food is not priced, so this is not the fixture it is meant to be"
    );
}
