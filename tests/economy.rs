//! The vertical slice's acceptance test, as executable assertions.
//!
//! `docs/state-and-economy-spec.md` Part D ends with an eight-step chain
//! that must run with no special-case code. These are those steps.

use scale_sim::econ::{Commodity, Economy};
use scale_sim::slice::{self, Doctrine};

const FOOD: Commodity = Commodity::ProcessedFood;

fn run(econ: &mut Economy, days: u64) {
    for _ in 0..days {
        econ.step();
    }
}

#[test]
fn conservation_holds_over_a_long_run() {
    // Rule R1. Nothing may enter or leave except through the journal â€”
    // this is the property the whole ledger/journal structure exists to
    // guarantee, and the one that must never regress.
    let mut econ = slice::build(Doctrine::Negligent);
    for day in 0..400 {
        econ.step();
        econ.ledger.assert_conserved();
        assert!(
            econ.ledger.day == day + 1,
            "day counter drifted at {day}"
        );
    }
}

#[test]
fn stock_never_goes_negative() {
    let mut econ = slice::build(Doctrine::Negligent);
    for _ in 0..200 {
        econ.step();
        for site in 0..econ.ledger.sites.len() {
            for &c in Commodity::ALL.iter() {
                let q = econ.ledger.sites[site].stock[c as usize];
                assert!(
                    q > -1e-6,
                    "{} holds {q:.6} {c} â€” stock went negative",
                    econ.ledger.sites[site].name
                );
            }
        }
    }
}

#[test]
fn undisturbed_economy_settles_at_cost() {
    // With supply comfortably meeting demand, price should sit at the cost
    // of production and cover at its target. If the baseline drifts, every
    // disruption measurement downstream is meaningless.
    let mut econ = slice::build(Doctrine::Negligent);
    run(&mut econ, 60);

    for m in [slice::ASHFORD, slice::BEXLEY] {
        let price = econ.price(m, FOOD);
        let base = FOOD.base_cost();
        assert!(
            (price - base).abs() / base < 0.05,
            "{}: settled at {price:.0}, expected about {base:.0}",
            econ.markets[m].name
        );
        let cover = econ.markets[m].cover[FOOD as usize];
        assert!(
            (cover - FOOD.target_cover_days()).abs() < 0.5,
            "{}: cover settled at {cover:.1} days",
            econ.markets[m].name
        );
    }
}

#[test]
fn losing_the_only_line_starves_both_towns() {
    // Steps 1-4 of the acceptance test: no N-1 spare, so the line failure
    // stops the cannery, food stock runs down within days, and the price
    // rises steeply because food is inelastic.
    let mut econ = slice::build(Doctrine::Negligent);
    // Nobody available to fix it, so the outage persists and the full
    // cascade can be measured. The repair chain is tested separately.
    econ.response.crews = 0;
    run(&mut econ, 20);
    let before = econ.price(slice::ASHFORD, FOOD);

    assert!(econ.grid.fail_line("Kelling line A"));
    run(&mut econ, 3);
    assert!(
        econ.unserved_power > 0.0,
        "line is down but nothing is being shed"
    );

    // The 3-5 day retail buffer should carry them briefly, then fail.
    run(&mut econ, 12);
    let after = econ.price(slice::ASHFORD, FOOD);
    assert!(
        after > before * 2.0,
        "food went from {before:.0} to only {after:.0} after two weeks without power"
    );
    assert!(
        econ.unmet_demand[FOOD as usize] > 0.0,
        "stock is gone but nobody is recorded as going without"
    );
}

#[test]
fn a_redundant_grid_absorbs_the_same_failure() {
    // The doctrine trait made legible: identical shock, prudent grid, no
    // consequence at all. This is what a scouting player is reading.
    let mut econ = slice::build(Doctrine::Prudent);
    run(&mut econ, 20);
    let before = econ.price(slice::ASHFORD, FOOD);

    assert!(econ.grid.fail_line("Kelling line A"));
    run(&mut econ, 25);

    assert_eq!(
        econ.unserved_power, 0.0,
        "redundant grid shed load it should have carried"
    );
    let after = econ.price(slice::ASHFORD, FOOD);
    assert!(
        (after - before).abs() / before < 0.05,
        "price moved from {before:.0} to {after:.0} despite the spare line"
    );
}

#[test]
fn the_world_repairs_itself_without_being_told() {
    // Steps 6-8. Nothing here restores the line; the fault is noticed,
    // reported, assigned to a crew who travel to it, fixed, and the
    // economy recovers on its own.
    let mut econ = slice::build(Doctrine::Negligent);
    run(&mut econ, 20);
    let baseline = econ.price(slice::ASHFORD, FOOD);

    econ.grid.fail_line("Kelling line A");
    run(&mut econ, 40);

    let inc = econ.response.incidents.first().expect("no incident raised");
    assert!(inc.reported.is_some(), "the fault was never reported");
    assert!(inc.dispatched.is_some(), "no crew was ever sent");
    let resolved = inc.resolved.expect("the line was never repaired");
    let outage = resolved - inc.occurred;

    // A downed line is a matter of days even for a badly-run region —
    // crews carry what they need. Anything approaching a month means the
    // repair chain has broken.
    assert!(
        (3..=20).contains(&outage),
        "line was out for {outage} days, which is not a plausible line repair"
    );

    let recovered = econ.price(slice::ASHFORD, FOOD);
    assert!(
        (recovered - baseline).abs() / baseline < 0.10,
        "after repair price is {recovered:.0}, baseline was {baseline:.0}"
    );
}

#[test]
fn a_prudent_region_never_notices_the_outage() {
    // Fast response plus a spare line means the failure never reaches the
    // shops at all.
    let mut econ = slice::build(Doctrine::Prudent);
    run(&mut econ, 20);
    econ.grid.fail_line("Kelling line A");
    run(&mut econ, 20);

    let inc = econ.response.incidents.first().expect("no incident raised");
    let outage = inc.resolved.expect("never repaired") - inc.occurred;
    assert!(outage <= 5, "a well-run region took {outage} days");
    assert!(
        econ.unmet_demand[FOOD as usize] == 0.0,
        "nobody should have gone without"
    );
}

#[test]
fn a_spare_transformer_is_the_difference_between_days_and_never() {
    // The most consequential prudence decision on the grid. A transformer
    // is built to order with a lead time measured in months; holding one
    // in store turns a catastrophe into an inconvenience.
    let mut with_spare = slice::build(Doctrine::Prudent);
    run(&mut with_spare, 10);
    with_spare.grid.fail_transformer("Kelling line A");
    run(&mut with_spare, 40);
    let quick = with_spare.response.incidents[0]
        .resolved
        .expect("prudent region never replaced its transformer")
        - with_spare.response.incidents[0].occurred;
    assert!(
        (3..=30).contains(&quick),
        "swapping a spare transformer took {quick} days"
    );
    assert_eq!(with_spare.response.spare_transformers, 0, "spare not consumed");

    let mut without = slice::build(Doctrine::Negligent);
    run(&mut without, 10);
    without.grid.fail_transformer("Kelling line A");
    run(&mut without, 120);
    assert!(
        without.response.incidents[0].resolved.is_none(),
        "a transformer was replaced in four months with none in store"
    );
    assert!(
        without.unmet_demand[FOOD as usize] > 0.0,
        "four months without power and nobody went hungry"
    );
}

#[test]
fn nothing_is_fixed_when_nobody_can_report_it() {
    // Response is not automatic: it needs a witness with a working way to
    // tell someone. Cutting comms is an attack in its own right.
    let mut econ = slice::build(Doctrine::Negligent);
    econ.response.comms_up = false;
    run(&mut econ, 20);
    econ.grid.fail_line("Kelling line A");
    run(&mut econ, 60);

    assert_eq!(econ.response.unreported(), 1, "the fault got reported somehow");
    assert!(
        econ.response.incidents[0].dispatched.is_none(),
        "a crew was sent for a fault nobody had reported"
    );
    assert!(
        econ.unmet_demand[FOOD as usize] > 0.0,
        "two months of blackout and nobody went hungry"
    );
}

#[test]
fn a_haul_contract_appears_because_the_arithmetic_changed() {
    // Step 5, and the point of the whole exercise. No quest system
    // generates this; it exists when the price gap between two markets
    // exceeds the freight cost between them, and not before.
    let mut econ = slice::build(Doctrine::Negligent);
    run(&mut econ, 20);
    assert!(
        econ.arbitrage(0, FOOD) <= 0.0,
        "a profitable haul exists in an undisturbed economy â€” \
         the markets should be within freight cost of each other"
    );

    econ.grid.fail_line("Kelling line A");
    let mut appeared = false;
    for _ in 0..25 {
        econ.step();
        if econ.arbitrage(0, FOOD) > 0.0 {
            appeared = true;
            break;
        }
    }
    assert!(
        appeared,
        "the towns diverged but no profitable haul ever appeared"
    );
}

#[test]
fn cutting_the_road_decouples_the_markets() {
    // Spec A.7: the price gap between two markets cannot exceed the cost
    // of moving goods between them â€” unless nothing can move, in which
    // case they are no longer one market at all.
    let mut econ = slice::build(Doctrine::Prudent);
    run(&mut econ, 40);

    let gap_before = (econ.price(slice::ASHFORD, FOOD)
        - econ.price(slice::BEXLEY, FOOD))
    .abs();
    assert!(
        gap_before <= econ.routes[0].freight_cost + 1.0,
        "connected markets differ by {gap_before:.0}, more than the {:.0} freight cost",
        econ.routes[0].freight_cost
    );

    // Bexley has no works of its own; everything it eats comes down that
    // road.
    econ.routes[0].open = false;
    run(&mut econ, 25);

    let gap_after = (econ.price(slice::ASHFORD, FOOD)
        - econ.price(slice::BEXLEY, FOOD))
    .abs();
    assert!(
        gap_after > econ.routes[0].freight_cost * 5.0,
        "road is cut but the markets are still coupled (gap {gap_after:.0})"
    );
    assert!(
        econ.price(slice::BEXLEY, FOOD) > econ.price(slice::ASHFORD, FOOD),
        "the town with no industry should be the one that suffers"
    );
}

#[test]
fn the_journal_explains_every_change() {
    // Spec A2.3: an unattributed delta is a bug, not data. Every event
    // must carry a site and a quantity that actually moved.
    let mut econ = slice::build(Doctrine::Negligent);
    run(&mut econ, 30);

    assert!(!econ.journal.is_empty(), "nothing was journalled at all");
    for entry in econ.journal.entries() {
        assert!(entry.day < econ.ledger.day, "entry dated in the future");
        let qty = match &entry.event {
            scale_sim::econ::Event::Produced { qty, .. }
            | scale_sim::econ::Event::Consumed { qty, .. }
            | scale_sim::econ::Event::Shipped { qty, .. }
            | scale_sim::econ::Event::Spoiled { qty, .. } => *qty,
        };
        assert!(qty > 0.0, "journalled a non-positive quantity: {qty}");
    }
}

#[test]
fn the_slice_is_deterministic() {
    let mut a = slice::build(Doctrine::Negligent);
    let mut b = slice::build(Doctrine::Negligent);
    for _ in 0..50 {
        a.step();
        b.step();
    }
    a.grid.fail_line("Kelling line A");
    b.grid.fail_line("Kelling line A");
    run(&mut a, 20);
    run(&mut b, 20);

    for &c in Commodity::ALL.iter() {
        assert_eq!(
            a.ledger.total(c).to_bits(),
            b.ledger.total(c).to_bits(),
            "two identical runs hold different amounts of {c}"
        );
    }
    assert_eq!(a.journal.len(), b.journal.len());
}
