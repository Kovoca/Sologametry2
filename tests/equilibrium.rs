//! **A world with no reason for anything to differ.**
//!
//! In a real country a spread in days of cover between two towns can be
//! perfectly correct — a mountain port 1,200 km from the grain belt is
//! *supposed* to hold less and pay more — so "cover is level" is a claim
//! about the world rather than about the model, and asserting it in an
//! asymmetric fixture tests something nobody has established. That is why
//! the food-cover question went round in circles for two commits.
//!
//! `slice::symmetric` removes the ambiguity. Three towns identical in
//! every respect, a triangle of identical roads, one nation so they share
//! a season. Under rotation the world maps onto itself, so **any mechanism
//! that respects the economics must give each town the same answer.** A
//! spread is then not a signal. It is a bug.

use scale_sim::econ::{Commodity, Doctrine, Economy};
use scale_sim::logistics::Logistics;
use scale_sim::slice;

fn cover(e: &Economy, m: usize, c: Commodity) -> f64 {
    let held: f64 = (0..e.ledger.sites.len())
        .filter(|&s| e.ledger.sites[s].market == m)
        .map(|s| e.ledger.stock(s, c))
        .sum();
    let d = e.daily_draw(m, c);
    if d > 1e-9 {
        held / d
    } else {
        0.0
    }
}

fn covers(e: &Economy, c: Commodity) -> Vec<f64> {
    (0..e.markets.len()).map(|m| cover(e, m, c)).collect()
}

fn run(days: usize, hauliers: bool, reversed: bool) -> Economy {
    let mut e = slice::symmetric(Doctrine::Prudent);
    if reversed {
        // **The same world, scanned the other way round.** Site indices
        // are referenced by nothing else in a freshly built economy —
        // every site carries its own market — so reversing the vector
        // changes nothing whatever about the economics. If the answer
        // moves, the answer was never about the economics.
        e.ledger.sites.reverse();
    }
    if hauliers {
        e.logistics = Some(Logistics::found(&e));
    }
    for _ in 0..days {
        e.step();
    }
    e
}

/// **Three identical towns hold identical stock.**
///
/// The gate that found the bug. Before pro-rata distribution this came out
/// at **57, 33 and 8 days of food** — a monotone gradient by market index,
/// which is the signature of a scan in vector order and cannot be anything
/// else in a world where the three towns are interchangeable.
#[test]
fn three_identical_towns_end_up_in_the_same_place() {
    let e = run(400, false, false);
    for &c in Commodity::ALL.iter() {
        if !c.storable() {
            continue;
        }
        let v = covers(&e, c);
        let lo = v.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = v.iter().cloned().fold(0.0f64, f64::max);
        assert!(
            hi - lo < 0.01,
            "{c} cover differs between towns that are the same in every \
             respect: {v:?} (spread {:.3})",
            hi - lo
        );
    }
    e.ledger.assert_conserved();
}

/// **And the answer does not depend on the order of a `Vec`.**
///
/// The decisive experiment, because it varies exactly one thing that has
/// no economic content at all.
#[test]
fn reversing_the_site_vector_changes_nothing() {
    let forward = run(400, false, false);
    let backward = run(400, false, true);
    for &c in Commodity::ALL.iter() {
        if !c.storable() {
            continue;
        }
        let (a, b) = (covers(&forward, c), covers(&backward, c));
        for m in 0..a.len() {
            assert!(
                (a[m] - b[m]).abs() < 0.01,
                "{c} in town {m}: {} forward against {} reversed — the answer \
                 depends on where the sites sit in a list",
                a[m],
                b[m]
            );
        }
    }
}

/// **Where nobody outbids anybody, a shortage is shared.**
///
/// The gate for the tie-breaking half of the allocation rule, and getting
/// its fixture right took two attempts — both times by making the same
/// mistake this whole file exists to correct.
///
/// **Shutting two mills of the three does not test this.** It leaves one
/// town with a mill and two without, which is not a symmetric world any
/// more, and the answer — the surviving mill's own town takes every sack,
/// because carriage comes out of the seller's netback and the other two
/// are 800 km away — is then perfectly correct. Asserting an even outcome
/// there is asserting symmetry in a world I had just made asymmetric.
///
/// So the shortage is applied symmetrically: **every mill at 30%**. The
/// three towns stay interchangeable, every buyer's netback is identical,
/// the whole component is one band, and there is no fact about the world
/// that could make one of them go short before the others.
#[test]
fn a_shortage_is_shared_out_rather_than_taken_by_whoever_asks_first() {
    let mut e = slice::symmetric(Doctrine::Prudent);
    let mut throttled = 0;
    for s in 0..e.ledger.sites.len() {
        if e.ledger.sites[s].kind == scale_sim::econ::SiteKind::Mill {
            e.ledger.sites[s].throughput *= 0.30;
            throttled += 1;
        }
    }
    assert_eq!(throttled, 3, "the fixture no longer has three mills");

    for _ in 0..200 {
        e.step();
        e.ledger.assert_conserved();
    }

    for &c in &[Commodity::Flour, Commodity::ProcessedFood] {
        let v = covers(&e, c);
        let lo = v.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = v.iter().cloned().fold(0.0f64, f64::max);
        assert!(
            hi - lo < 0.05 * hi.max(1.0),
            "{c}: a shortage fell on {v:?} — somebody was served first"
        );
        // **And it fell on somebody.** A gate where everyone ends at zero
        // passes on an absence, which is the same mistake as a test that
        // never enters its branch.
        assert!(
            hi > 0.01,
            "{c} came out at nothing anywhere, so the gate proved nothing: {v:?}"
        );
    }

    // The shortage has to be real, or the sharing rule never binds and
    // this is measuring an adequate supply.
    let short = covers(&e, Commodity::ProcessedFood);
    let plenty = covers(&run(200, false, false), Commodity::ProcessedFood);
    assert!(
        short[0] < plenty[0] * 0.9,
        "milling at 30% left food at {:.2} against {:.2} — there was no          shortage to share out",
        short[0],
        plenty[0]
    );
}

/// **Hauliers do not disturb a world that is already level.**
///
/// A freight industry exists to move goods to where they are wanted. In a
/// country where every town already has what it needs there is nothing for
/// it to do, and a model whose carriers shuffle stock about to no purpose
/// is inventing work — which shows up as an oscillation rather than as
/// commerce.
///
/// **The tolerance is relative and the reason is worth recording rather
/// than hiding**: carriers rank markets by cover and break exact ties by
/// index, so in a world where three markets are *precisely* equal the
/// tiebreak has to pick one of them. The resulting wobble is a fraction of
/// a per cent and self-correcting, because tomorrow the town that was
/// served is no longer the neediest. What would not be acceptable is a
/// drift that grows, so the bar is a small share of the level rather than
/// an absolute figure that a bigger stockpile would sail through.
#[test]
fn carriers_have_nothing_to_do_in_a_country_that_is_already_even() {
    let e = run(400, true, false);
    for &c in Commodity::ALL.iter() {
        if !c.storable() {
            continue;
        }
        let v = covers(&e, c);
        let lo = v.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = v.iter().cloned().fold(0.0f64, f64::max);
        let level = v.iter().sum::<f64>() / v.len() as f64;
        assert!(
            hi - lo < (0.03 * level).max(0.01),
            "{c}: carriers moved a level country to {v:?} (spread {:.3})",
            hi - lo
        );
    }
    e.ledger.assert_conserved();
}

/// **No unexploited arbitrage in grain, within a country.**
///
/// The claim that is actually true of an *asymmetric* world, and the one
/// that should have been asserted instead of level cover all along. Two
/// towns may legitimately differ in price; what may not survive is a gap
/// wider than the cost of closing it, where the cheap end has stock to
/// spare. That is money left on the table.
///
/// **Grain is clean and the others are not**, and the boundary is
/// informative rather than embarrassing: grain is made in every town and
/// wanted in every town, so `distribute` reaches it. Medical grade is made
/// in one town and wanted in all of them, and the only mechanism that can
/// move it end to end is a haulier — who decides on **days of cover**
/// while `trade` decides on **price**, and only between adjacent towns.
/// So a price gap between two towns that are not neighbours has nothing
/// looking at it. That is a named gap, not a mystery.
#[test]
fn a_country_does_not_leave_grain_money_on_the_table() {
    use scale_sim::network::Network;
    use scale_sim::polity::Polities;
    use scale_sim::region::Nations;
    use scale_sim::settlement::Settlements;
    use scale_sim::world::World;

    let world = World::generate(384, 216, 7);
    let polities = Polities::partition(&world, 24);
    let settlements = Settlements::place(&world, &polities, 3000);
    let network = Network::build(&world, &settlements, 500);
    let mut n =
        Nations::build(&world, &polities, &settlements, &network, 4, 4, Doctrine::Prudent);
    for _ in 0..400 {
        n.economy.step();
    }
    let e = &n.economy;
    let c = Commodity::Grain;

    let mut worst = 0.0f64;
    let mut pair = (0usize, 0usize);
    for dear in 0..e.markets.len() {
        for cheap in 0..e.markets.len() {
            if dear == cheap {
                continue;
            }
            // Same country only. Across a border the lanes are thin and
            // this file already records that as a known gap.
            if !n
                .markets_of
                .iter()
                .any(|ms| ms.contains(&dear) && ms.contains(&cheap))
            {
                continue;
            }
            if e.surplus(cheap, c) <= 1e-6 {
                continue;
            }
            let excess = e.markets[dear].price[c as usize]
                - e.markets[cheap].price[c as usize]
                - e.freight_between(cheap, dear);
            if excess > worst {
                worst = excess;
                pair = (cheap, dear);
            }
        }
    }
    let reference = e.markets[0].price[c as usize].max(1e-9);
    assert!(
        worst / reference < 0.05,
        "{} to {}: {worst:.2} a tonne of grain, {:.0}% of its price, going \
         begging inside one country",
        pair.0,
        pair.1,
        100.0 * worst / reference
    );
}
