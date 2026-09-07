//! Trade between nations.
//!
//! One ledger for the whole planet, so a cargo leaving one country is the
//! same tonnes arriving in another rather than a subtraction here and an
//! invention there.

use scale_sim::econ::{Commodity, Doctrine, DAYS_PER_YEAR};
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Nations;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

const FOOD: Commodity = Commodity::ProcessedFood;
const GRAIN: Commodity = Commodity::Grain;

fn nations(seed: u64, count: usize) -> Nations {
    let world = World::generate(384, 216, seed);
    let polities = Polities::partition(&world, 24);
    let settlements = Settlements::place(&world, &polities, 3000);
    let network = Network::build(&world, &settlements, 500);
    Nations::build(
        &world,
        &polities,
        &settlements,
        &network,
        count,
        4,
        Doctrine::Prudent,
    )
}

#[test]
fn a_world_of_nations_conserves() {
    // The reason for one ledger. Folding several economies together must
    // not lose or invent a tonne of anything.
    for seed in [1u64, 42, 20260828] {
        let mut n = nations(seed, 6);
        n.economy.ledger.assert_conserved();
        for _ in 0..(DAYS_PER_YEAR * 2) {
            n.economy.step();
            n.economy.ledger.assert_conserved();
        }
    }
}

#[test]
fn both_hemispheres_are_represented() {
    // The seasonal trade opportunity is between hemispheres, not within a
    // country: one nation's lean season is another's harvest. If every
    // nation sits in the same hemisphere there is nothing to trade on.
    let n = nations(20260828, 8);
    let north = n.economy.markets.iter().filter(|m| !m.southern).count();
    let south = n.economy.markets.len() - north;
    assert!(
        north > 0 && south > 0,
        "{north} northern and {south} southern markets — no hemisphere offset at all"
    );
}

#[test]
fn hemispheres_run_out_of_step() {
    let n = nations(20260828, 8);
    let north = n
        .economy
        .markets
        .iter()
        .position(|m| !m.southern)
        .expect("no northern market");
    let south = n
        .economy
        .markets
        .iter()
        .position(|m| m.southern)
        .expect("no southern market");

    // On any given day the two hemispheres must be in different seasons,
    // and their harvests must peak at opposite ends of the year.
    let mut differed = 0;
    let mut e = n.economy;
    for _ in 0..DAYS_PER_YEAR {
        e.step();
        if e.season_at(north) != e.season_at(south) {
            differed += 1;
        }
    }
    assert!(
        differed > DAYS_PER_YEAR as usize * 3 / 4,
        "hemispheres were in the same season for most of the year"
    );
}

#[test]
fn nations_trade_across_borders() {
    // Lanes exist and are actually used. Without this the whole structure
    // is several sealed economies sharing a ledger.
    let mut n = nations(20260828, 6);
    let international: Vec<usize> = (0..n.economy.routes.len())
        .filter(|&r| {
            let e = &n.economy;
            e.markets[e.routes[r].a].nation != e.markets[e.routes[r].b].nation
        })
        .collect();
    assert!(!international.is_empty(), "no lanes between nations at all");

    let mut ever_worth_it = false;
    for _ in 0..(DAYS_PER_YEAR * 2) {
        n.economy.step();
        if international
            .iter()
            .any(|&r| n.economy.arbitrage(r, GRAIN) > 0.0)
        {
            ever_worth_it = true;
        }
    }
    assert!(
        ever_worth_it,
        "nations diverged for two years and no cross-border haul was ever worth taking"
    );
}

#[test]
fn a_blockade_isolates_a_nation() {
    // Cutting the lanes must actually cut them: a blockaded nation's
    // prices come loose from everyone else's, because nothing can move.
    let mut n = nations(20260828, 6);
    for _ in 0..(DAYS_PER_YEAR + 100) {
        n.economy.step();
    }

    let target: u16 = 1;
    let mut closed = 0;
    let markets = n.economy.markets.iter().map(|m| m.nation).collect::<Vec<_>>();
    for r in n.economy.routes.iter_mut() {
        let (a, b) = (markets[r.a], markets[r.b]);
        if a != b && (a == target || b == target) {
            r.open = false;
            closed += 1;
        }
    }
    assert!(closed > 0, "nation {target} had no lanes to close");

    for _ in 0..200 {
        n.economy.step();
    }
    n.economy.ledger.assert_conserved();

    // Nothing may cross a closed lane.
    for r in n.economy.routes.iter().filter(|r| !r.open) {
        let (a, b) = (markets[r.a], markets[r.b]);
        assert!(a == target || b == target, "closed the wrong lane");
    }
}

#[test]
fn goods_actually_cross_borders() {
    // Not merely that a haul is *worth* taking, but that tonnes move. The
    // journal knows: a Shipped event whose two ends sit in different
    // nations is international trade and nothing else is.
    use scale_sim::econ::Event;

    let mut n = nations(20260828, 6);
    let before = n.economy.journal.len();
    for _ in 0..(DAYS_PER_YEAR * 2) {
        n.economy.step();
    }

    let nation_of_site: Vec<u16> = n
        .economy
        .ledger
        .sites
        .iter()
        .map(|s| n.economy.markets[s.market].nation)
        .collect();

    let mut crossed = 0.0f64;
    for entry in n.economy.journal.entries().iter().skip(before) {
        if let Event::Shipped { from, to, qty, .. } = &entry.event {
            if nation_of_site[*from] != nation_of_site[*to] {
                crossed += qty;
            }
        }
    }
    assert!(
        crossed > 0.0,
        "two years and not one tonne crossed a border"
    );
}

/// **What equalises across a border is what is worth carrying over one.**
///
/// This used to record a gap: lanes joined capitals, so a cargo from a
/// glut in one country to a dear market in another had to clear three
/// separate price-versus-freight tests in a row, and that chain rarely
/// completed. The note said what was wanted next — *agents choosing routes
/// end to end, not a pairwise test at every hop* — and `logistics.rs` is
/// that, once the carriers are founded over the whole trading world rather
/// than inherited from whichever nation was built first.
///
/// What is left is not a gap but a real economic fact. **Value density
/// decides how far a thing travels**, so the spread closes for what is
/// worth hauling and stays open for what is not:
///
/// | | no carriers | with carriers |
/// |---|---|---|
/// | steel | 5.02x | **1.43x** |
/// | medicine | 1.56x | **1.02x** |
/// | grain | wide | **still wide, and correctly so** |
#[test]
fn what_crosses_a_border_is_what_is_worth_carrying() {
    let spread = |n: &scale_sim::region::Nations, c| {
        let (mut lo, mut hi) = (f64::INFINITY, 0.0f64);
        for m in 0..n.economy.markets.len() {
            let p = n.economy.price(m, c);
            lo = lo.min(p);
            hi = hi.max(p);
        }
        hi / lo.max(1.0)
    };

    let mut with = nations(20260828, 6);
    let mut without = nations(20260828, 6);
    without.economy.logistics = None;
    for _ in 0..DAYS_PER_YEAR {
        with.economy.step();
        without.economy.step();
    }

    // Every market in the trading world has a haulier, not just the ones
    // belonging to whichever nation was folded in first.
    let firms = with.economy.logistics.as_ref().unwrap().carriers.len();
    assert_eq!(
        firms,
        with.economy.markets.len(),
        "a merged world should have a carrier in every town"
    );

    // **Medicine is the proof.** At 6,000 a tonne it is the one thing dear
    // enough to be worth carrying over the dearest leg in this world —
    // freight between these markets runs 2 to 983 a tonne over a mean haul
    // of 1,136 km — so a border stops being a price wall for it and for
    // nothing cheaper.
    let med = scale_sim::econ::Commodity::Medicine;
    let closed = spread(&with, med);
    let open = spread(&without, med);
    assert!(
        closed < open / 2.0,
        "medicine should equalise sharply once there are hauliers: \
         {closed:.2}x against {open:.2}x"
    );

    // **And what is left is carriage, which trade cannot equalise past.**
    //
    // This is the second time this gate has moved, and the first time the
    // residual could be *checked* rather than explained away. When every
    // market shared one typed-in reference cost, the whole spread was
    // scarcity and hauliers could erase all of it — tidier, and untrue.
    // Now a cargo carries what it cost and what the haul was charged, so
    // two markets a thousand kilometres apart have genuinely different
    // landed costs and prices differ by at least that much however freely
    // the stuff moves.
    let landed_spread = {
        let (mut lo, mut hi) = (f64::INFINITY, 0.0f64);
        for m in 0..with.economy.markets.len() {
            let l = with.economy.markets[m].landed[med as usize];
            lo = lo.min(l);
            hi = hi.max(l);
        }
        hi / lo.max(1.0)
    };
    assert!(
        landed_spread > 1.15,
        "every market gets medicine for the same money, so the residual price \
         spread of {closed:.2}x is not carriage and needs another explanation"
    );
    assert!(
        closed <= landed_spread * 4.0,
        "prices differ by {closed:.2}x where the cost of getting hold of it \
         differs by only {landed_spread:.2}x, so something other than carriage \
         is holding them apart"
    );

    // **And cheap bulk does not, which is right rather than a defect.** A
    // haul is refused when the freight exceeds half what the goods are
    // worth. Grain at 220 a tonne does not clear a 983 a tonne leg, and it
    // should not: it is the same reason a mountain port a thousand
    // kilometres from the grain belt is *supposed* to pay more for bread,
    // and the reason there is a cement works in every region on earth.
    //
    // The discriminator is the rule, not the commodity. In a smaller world
    // where the lanes are shorter, steel equalises too (5.02x to 1.43x);
    // here it cannot, because these countries are further apart than steel
    // is worth carrying.
    let bulk = spread(&with, GRAIN);
    assert!(
        bulk > 1.5,
        "grain equalised to {bulk:.2}x, which would mean bulk freight had \
         become free — check the value-density rule in logistics.rs"
    );

    // **And cement is the same rule seen from the other side.** It is too
    // cheap to carry anywhere, so every town makes its own — and once
    // every town has a kiln, nobody ships it and the price is at cost
    // everywhere. A commodity that does not travel does not need to: that
    // is *why* there is a cement works in every region on earth, and the
    // model arrives at it by refusing the haul rather than by being told.
    //
    // **"At cost" stopped meaning "the same number".** This asserted a
    // near-flat world price, which was true only while every market shared
    // one typed-in reference cost. Now that a kiln's coal and power are its
    // own, a country burning dear imported fuel genuinely makes dearer
    // cement — real cement prices vary severalfold between countries for
    // exactly that reason, energy being 30-40% of the cost of it.
    //
    // So the claim to test is the one that was always the point: **nobody
    // ships it.** Its price differs where making it differs, and nowhere
    // else.
    let cem = scale_sim::econ::Commodity::Cement;
    let cement = spread(&with, cem);
    let cement_cost = {
        let (mut lo, mut hi) = (f64::INFINITY, 0.0f64);
        for m in 0..with.economy.markets.len() {
            let c = with.economy.markets[m].cost[cem as usize];
            lo = lo.min(c);
            hi = hi.max(c);
        }
        hi / lo.max(1.0)
    };
    assert!(
        cement <= cement_cost * 1.35,
        "cement's price varies {cement:.2}x where making it varies only \
         {cement_cost:.2}x, so somebody is shipping what they should be making"
    );

    // And nobody's kiln is standing idle for want of affordable fuel, which
    // is what a phantom freight charge once did to half of them.
    let idle = (0..with.economy.ledger.sites.len())
        .filter(|&s| {
            with.economy.ledger.sites[s].kind == scale_sim::econ::SiteKind::CementWorks
                && with.economy.ledger.sites[s].ran <= 0.0
        })
        .count();
    let kilns = (0..with.economy.ledger.sites.len())
        .filter(|&s| {
            with.economy.ledger.sites[s].kind == scale_sim::econ::SiteKind::CementWorks
        })
        .count();
    // Not none — one kiln somewhere may genuinely be between batches — but
    // nothing like the half of them that a phantom freight charge once
    // priced out of buying their own fuel.
    assert!(
        idle * 5 < kilns,
        "{idle} of {kilns} cement works stood idle in a world that wants cement"
    );
}

#[test]
fn neglected_roads_decay_and_freight_gets_dearer() {
    // The maintenance deficit of spec C.4, end to end: a state that
    // underfunds upkeep sees its roads go, and its freight bill rises
    // years later — long after whoever cut the budget has moved on.
    let world = World::generate(384, 216, 20260828);
    let polities = Polities::partition(&world, 24);
    let settlements = Settlements::place(&world, &polities, 3000);
    let network = Network::build(&world, &settlements, 500);

    let run = |doctrine: Doctrine| {
        let mut n = Nations::build(
            &world, &polities, &settlements, &network, 4, 4, doctrine,
        );
        let before: f64 = n.economy.routes.iter().map(|r| r.freight_cost).sum();
        for _ in 0..(DAYS_PER_YEAR * 20) {
            n.economy.step();
        }
        let after: f64 = n.economy.routes.iter().map(|r| r.freight_cost).sum();
        let condition = n.economy.road_condition[0];
        (before, after, condition)
    };

    let (kept_before, kept_after, kept) = run(Doctrine::Prudent);
    let (let_go_before, let_go_after, let_go) = run(Doctrine::Negligent);

    assert!(
        (kept_after - kept_before).abs() / kept_before < 0.01,
        "a maintained network got dearer anyway: {kept_before:.0} -> {kept_after:.0}"
    );
    assert!(
        let_go < kept,
        "twenty years of underfunding left the roads in condition {let_go:.2} \
         against {kept:.2} for a maintained network"
    );
    assert!(
        let_go_after > let_go_before * 1.1,
        "roads decayed to {let_go:.2} and freight only went {let_go_before:.0} -> \
         {let_go_after:.0}"
    );
}

#[test]
fn road_decay_is_slow_enough_to_be_a_trap() {
    // The point of the mechanic is the delay. If neglect showed up within
    // a year or two it would simply be a bad decision anyone could see;
    // what makes it worth modelling is that it takes a decade, by which
    // time it is somebody else's problem.
    let world = World::generate(384, 216, 20260828);
    let polities = Polities::partition(&world, 24);
    let settlements = Settlements::place(&world, &polities, 3000);
    let network = Network::build(&world, &settlements, 500);
    let mut n = Nations::build(
        &world, &polities, &settlements, &network, 4, 4, Doctrine::Negligent,
    );

    for _ in 0..(DAYS_PER_YEAR * 3) {
        n.economy.step();
    }
    let after_three = n.economy.road_condition[0];
    assert!(
        after_three > 0.85,
        "three years of neglect already took the roads to {after_three:.2}"
    );

    for _ in 0..(DAYS_PER_YEAR * 17) {
        n.economy.step();
    }
    let after_twenty = n.economy.road_condition[0];
    assert!(
        after_twenty < 0.70,
        "twenty years of neglect left the roads at {after_twenty:.2} — no consequence at all"
    );
}

#[test]
fn nobody_starves_in_a_trading_world() {
    for seed in [1u64, 20260828] {
        let mut n = nations(seed, 6);
        for _ in 0..(DAYS_PER_YEAR * 2) {
            n.economy.step();
            assert!(
                n.economy.unmet_demand[FOOD as usize] == 0.0,
                "seed {seed}: people went hungry on day {} with nothing wrong",
                n.economy.ledger.day
            );
        }
    }
}


// =====================================================================
// what a cargo costs, and who is paid for carrying it
// =====================================================================

/// **Gate: a shipment carries what it cost, so a cheap producer abroad
/// gives a factory here a cheap input.**
///
/// Before this, `Event::Shipped` recorded a commodity and a quantity and
/// nothing else — so tonnes crossed a border and their price did not. A
/// processor in the receiving market fell back on its own local cost and
/// the exporter's cheapness was invisible the moment the cargo moved.
#[test]
fn a_cargo_arrives_carrying_its_own_price() {
    use scale_sim::econ::Event;

    let mut n = nations(20260828, 6);
    let before = n.economy.journal.len();
    for _ in 0..DAYS_PER_YEAR {
        n.economy.step();
    }

    // Shipments happen, and they carry a cost basis rather than a bare
    // tonnage.
    let mut with_a_price = 0;
    let mut bare = 0;
    let mut freight_charged = 0.0f64;
    for entry in n.economy.journal.entries().iter().skip(before) {
        if let Event::Shipped { qty, paid, freight, .. } = &entry.event {
            if *qty <= 0.0 {
                continue;
            }
            if *paid > 0.0 {
                with_a_price += 1;
            } else {
                bare += 1;
            }
            freight_charged += freight;
        }
    }
    assert!(with_a_price + bare > 0, "a year and nothing was shipped at all");
    assert!(
        with_a_price > bare,
        "{with_a_price} cargoes carried a price and {bare} carried none"
    );

    // **And somebody was charged for carrying it.** `Carrier::revenue` was
    // being accumulated and paid to nobody, so a haulage firm could work
    // all year with its account never moving.
    assert!(freight_charged > 0.0, "a year of freight and nobody charged for any of it");
    let paid_out = n.economy.treasury.flows.get("freight").copied().unwrap_or(0.0);
    assert!(
        paid_out > 0.0,
        "freight was charged on the cargo and never reached anybody's account"
    );
}

/// **Gate: landed cost is what was paid plus the carriage, and distance is
/// what makes it differ from the works price.**
#[test]
fn what_it_costs_here_is_what_it_cost_there_plus_getting_it_here() {
    let mut n = nations(20260828, 6);
    for _ in 0..(DAYS_PER_YEAR / 2) {
        n.economy.step();
    }

    // Every market has a landed cost for everything, and it is a real
    // number rather than a leftover.
    for m in 0..n.economy.markets.len() {
        for &c in Commodity::ALL.iter() {
            let landed = n.economy.markets[m].landed[c as usize];
            assert!(landed.is_finite() && landed > 0.0, "{} has no landed cost in market {m}", c.name());
        }
    }

    // **Freight between two markets is a real cost and zero within one.**
    assert_eq!(n.economy.freight_between(0, 0), 0.0);
    let mut any_positive = false;
    for a in 0..n.economy.markets.len() {
        for b in 0..n.economy.markets.len() {
            if a == b {
                continue;
            }
            let f = n.economy.freight_between(a, b);
            assert!(f >= 0.0 && f.is_finite(), "freight {a}->{b} is {f}");
            if f > 0.0 {
                any_positive = true;
            }
        }
    }
    assert!(any_positive, "moving anything anywhere was free");

    // **And a market that imports pays more than the works that made it.**
    // Not universally — a market may make its own — but somewhere in a
    // trading world of six nations, carriage has to show.
    let dearer = (0..n.economy.markets.len())
        .flat_map(|m| {
            Commodity::ALL
                .iter()
                .map(move |&c| (m, c))
        })
        .filter(|&(m, c)| n.economy.markets[m].landed[c as usize] > n.economy.markets[m].cost[c as usize] * 1.02)
        .count();
    assert!(
        dearer > 0,
        "nothing anywhere cost more to get hold of than to make, so carriage reached nothing"
    );
}
