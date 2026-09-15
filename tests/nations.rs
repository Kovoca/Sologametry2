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
    let markets = n
        .economy
        .markets
        .iter()
        .map(|m| m.nation)
        .collect::<Vec<_>>();
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
        .filter(|&s| with.economy.ledger.sites[s].kind == scale_sim::econ::SiteKind::CementWorks)
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
        let mut n = Nations::build(&world, &polities, &settlements, &network, 4, 4, doctrine);
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
        &world,
        &polities,
        &settlements,
        &network,
        4,
        4,
        Doctrine::Negligent,
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
        if let Event::Shipped {
            qty, paid, freight, ..
        } = &entry.event
        {
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
    assert!(
        with_a_price + bare > 0,
        "a year and nothing was shipped at all"
    );
    assert!(
        with_a_price > bare,
        "{with_a_price} cargoes carried a price and {bare} carried none"
    );

    // **And somebody was charged for carrying it.** `Carrier::revenue` was
    // being accumulated and paid to nobody, so a haulage firm could work
    // all year with its account never moving.
    assert!(
        freight_charged > 0.0,
        "a year of freight and nobody charged for any of it"
    );
    let paid_out = n
        .economy
        .treasury
        .flows
        .get("freight")
        .copied()
        .unwrap_or(0.0);
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
            assert!(
                landed.is_finite() && landed > 0.0,
                "{} has no landed cost in market {m}",
                c.name()
            );
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
        .flat_map(|m| Commodity::ALL.iter().map(move |&c| (m, c)))
        .filter(|&(m, c)| {
            n.economy.markets[m].landed[c as usize] > n.economy.markets[m].cost[c as usize] * 1.02
        })
        .count();
    assert!(
        dearer > 0,
        "nothing anywhere cost more to get hold of than to make, so carriage reached nothing"
    );
}

/// **Goods from outside the country are paid for.**
///
/// A depot is where goods from beyond the modelled world arrive, and its
/// recipe has **no inputs at all** — so a tonne of imported steel was made
/// out of nothing and nobody was billed for it. `Account::Abroad` exists
/// precisely so a trade deficit has somewhere to go, and the only line
/// that touched it was the one that opened it: **nothing had ever moved
/// money across a border.**
///
/// The consequence was not small. A country could run an unlimited trade
/// deficit at no cost, which made every import-dependent nation
/// artificially rich — and it is why a depot's balance sat at ten thousand
/// against a shop's four hundred and sixty million. It received goods free
/// and handed them on free, so it was a conduit rather than a business.
#[test]
fn what_comes_from_abroad_is_bought_from_abroad() {
    use scale_sim::money::Account;

    // **A country with no quay can only buy**, which is what isolates the
    // claim. The first version ran a four-nation world and asserted the
    // outside world ends up richer — and once exports existed that stopped
    // being true, because those countries are net *exporters* and the money
    // flows the other way. That is not the claim. The claim is that goods
    // from outside are paid for, not that the trade balance has a sign.
    //
    // The two-town fixture is on no map, so no town is coastal and nothing
    // can be sold abroad. Every crossing is an import.
    let mut e = scale_sim::slice::build(Doctrine::Prudent);
    assert!(
        e.markets.iter().all(|m| !m.port),
        "the fixture has grown a quay, so this no longer isolates imports"
    );
    let abroad_before = e.treasury.balance(Account::Abroad);

    let importers = (0..e.ledger.sites.len())
        .filter(|&s| e.buys_abroad(s))
        .count();
    assert!(
        importers > 0,
        "this fixture imports nothing, so the gate is watching a closed border"
    );

    // **Extraction is not an import**, and that distinction is the whole
    // of the rule. A farm, a colliery and an oil field also have recipes
    // with no inputs — they are taking from the land the world generator
    // actually put there, not from outside the model, and nobody abroad is
    // owed for it.
    //
    // **Watched by who pays, not by a balance.** This asserted that no
    // extractive site ended with a negative balance, which the treasury can
    // never produce — it caps every payment at what the payer holds — so
    // it could not fail whatever the model did. What says a farm was billed
    // for its own harvest is a transfer from it to the outside world, and
    // the recipe, not the kind of site, says who imports: a grain terminal
    // stands on a `Mine`.
    let mut billed_for_their_own_ground = Vec::new();
    for _ in 0..120 {
        e.step();
        for t in e.treasury.today.iter() {
            if let (Account::Firm(s), Account::Abroad) = (t.from, t.to) {
                if !e.buys_abroad(s) {
                    billed_for_their_own_ground.push(e.ledger.sites[s].name.clone());
                }
            }
        }
    }

    // **Money has left the country**, which is what an import is.
    let abroad_after = e.treasury.balance(Account::Abroad);
    assert!(
        abroad_after > abroad_before,
        "a hundred and twenty days of importing and the outside world is no better off: \
         {abroad_before:.0} to {abroad_after:.0}"
    );

    billed_for_their_own_ground.sort();
    billed_for_their_own_ground.dedup();
    assert!(
        billed_for_their_own_ground.is_empty(),
        "these take from their own ground and have been billed by the outside world: \
         {billed_for_their_own_ground:?}"
    );

    // And the books still balance, which they must: money moved between
    // named accounts and none was made.
    e.treasury.assert_conserved();
    e.ledger.assert_conserved();
}

/// **The border is a place, and it trades on a price like anywhere else.**
///
/// Whether a town buys from abroad or sells to it is decided the way the
/// trade itself decides it, and the way famine early-warning systems
/// compute it market by market: against **import parity** — the world
/// price, plus the voyage, the duty, the dockers and the road up from the
/// coast, plus a trader's margin — and **export parity**, the world price
/// less all of that. Above the first a town imports, below the second it
/// exports, and between them it does neither.
///
/// So a money printer at the quay is not something a gate has to catch any
/// more; it is arithmetic. One side adds the costs and the other takes them
/// away, and no price can sit above the first and below the second.
#[test]
fn a_country_buys_what_it_is_short_of_and_sells_what_it_is_long_of() {
    use scale_sim::money::{Account, Why};

    let mut n = nations(7, 4);
    let ports = (0..n.economy.markets.len())
        .filter(|&m| n.economy.quay(m))
        .count();
    assert!(
        ports > 0 && ports < n.economy.markets.len(),
        "{ports} of {} towns have a quay — a port is a fact about a town, not about a \
         country, and an inland town trades through the coast",
        n.economy.markets.len()
    );

    let mut to_abroad = 0.0f64;
    let mut from_abroad = 0.0f64;
    let mut growers_paid = 0.0f64;
    let mut unpaid_landings = Vec::new();
    for _ in 0..300 {
        // **The rate the day's landings were priced at**, which is the one
        // standing when the day began: the exchange settles at the close,
        // so reading it afterwards is a day out and the identity misses by
        // exactly the overnight move.
        let rate = n.economy.exchange.foreign_money();
        n.economy.step();
        let e = &n.economy;
        let today = &e.treasury.today;
        // Who the world paid today for goods it took.
        let exporters: std::collections::BTreeSet<usize> = today
            .iter()
            .filter(|t| t.from == Account::Abroad && t.why == Why::Trade)
            .filter_map(|t| match t.to {
                Account::Firm(s) => Some(s),
                _ => None,
            })
            .collect();
        for t in today.iter() {
            if t.to == Account::Abroad && t.from != Account::Abroad {
                to_abroad += t.amount;
                // **Only an importer is billed by the outside world.** A
                // farm, a colliery and an oil field also have recipes with
                // no inputs; they are taking from the ground the world
                // generator put there, and nobody abroad is owed for it.
                match t.from {
                    Account::Firm(s) => assert!(
                        e.buys_abroad(s),
                        "{} was billed by the outside world and imports nothing",
                        e.ledger.sites[s].name
                    ),
                    other => panic!("{other:?} paid the outside world"),
                }
            }
            if t.from == Account::Abroad && t.to != Account::Abroad {
                from_abroad += t.amount;
            }
            // **The exporter pays whoever grew it.** It used to take the
            // goods off the farms for nothing and be paid by the world for
            // goods it never owned.
            if t.why == Why::Supply {
                if let (Account::Firm(q), Account::Firm(_)) = (t.from, t.to) {
                    if exporters.contains(&q) {
                        growers_paid += t.amount;
                    }
                }
            }
        }
        // **Nothing lands without a bill**, whatever kind of site it stands
        // on: what the ships brought today is exactly what the outside
        // world was paid today plus what went down as owed to it.
        //
        // Five of the twelve import terminals stood on the same kinds of
        // site as the farms and mines that really are digging, and landed
        // their full tonnage every day with no bill raised at all — which
        // no count of payments can see, because there was nothing to count.
        // An identity over the tonnage can.
        //
        // **Billed, not necessarily paid**, and that is deliberate: an
        // importer pays on the terms every firm here does, and on the first
        // run of this gate twenty-five of them were landing cargoes with
        // empty tills. Their customers were not paying them either; that is
        // the wage-scale gap, and it is named there.
        //
        // Which sites landed goods from abroad is read off the recipe
        // directly, not through `buys_abroad` — the function whose getting
        // it wrong is the defect this watches for. Asked through the same
        // function, both sides of the identity would move together.
        //
        // **The rate is read as a number from before the day**, not through
        // `Economy::world_price`,
        // for the same reason: what is being checked is that every landing
        // is billed, and rebuilding the bill out of the very function that
        // computes it would check nothing. Whether the rate itself is right
        // is a different claim with its own gates.
        let mut landed_value = 0.0f64;
        for s in 0..e.ledger.sites.len() {
            let Some(r) = e.ledger.sites[s].recipe else {
                continue;
            };
            if !scale_sim::econ::RECIPES[r].from_abroad || e.ledger.sites[s].ran <= 1e-9 {
                continue;
            }
            for &(c, out) in scale_sim::econ::RECIPES[r].outputs {
                let voyage = c.sea_freight().expect("landed something that cannot sail");
                landed_value += scale_sim::econ::Economy::WHOLESALE_MARGIN
                    * out
                    * e.ledger.sites[s].ran
                    * c.world_price()
                    * rate
                    * (1.0 + voyage);
            }
        }
        let billed = today
            .iter()
            .filter(|t| t.to == Account::Abroad && t.why == Why::Trade)
            .map(|t| t.amount)
            .sum::<f64>()
            + e.treasury.unpaid_why.get("trade").copied().unwrap_or(0.0);
        if (landed_value - billed).abs() > 1e-6 * landed_value.max(1.0) {
            unpaid_landings.push(format!(
                "day {}: landed {landed_value:.0} and billed {billed:.0}",
                e.ledger.day
            ));
        }
    }
    let e = &n.economy;

    assert!(
        to_abroad > 0.0 && from_abroad > 0.0,
        "money crossed the border only one way: {to_abroad:.0} out, {from_abroad:.0} in"
    );
    assert!(
        growers_paid > 0.0,
        "the country exported and the people who made the goods were paid nothing"
    );
    unpaid_landings.sort();
    unpaid_landings.dedup();
    assert!(
        unpaid_landings.is_empty(),
        "goods landed from abroad without a bill: {:?}",
        unpaid_landings.iter().take(3).collect::<Vec<_>>()
    );

    // **An inland town cannot load a ship.** It exports the way it
    // imports, by road through the coast.
    for m in 0..e.markets.len() {
        if e.quay(m) {
            continue;
        }
        for &c in Commodity::ALL.iter() {
            assert!(
                !e.worth_exporting(m, c),
                "{} is inland and is loading ships",
                e.markets[m].name
            );
        }
    }

    // **And the direction follows the price**, both ways, somewhere in
    // this world — otherwise the gate is watching a country that never
    // trades and proves nothing.
    let mut importing = 0;
    let mut exporting = 0;
    for m in 0..e.markets.len() {
        for &c in Commodity::ALL.iter() {
            let p = e.markets[m].price[c as usize];
            if e.worth_importing(m, c) {
                importing += 1;
                assert!(p > e.export_parity(m, c));
            }
            if e.worth_exporting(m, c) {
                exporting += 1;
                assert!(p < e.import_parity(m, c));
            }
        }
    }
    assert!(
        importing > 0 && exporting > 0,
        "{importing} imports and {exporting} exports across four nations"
    );

    e.treasury.assert_conserved();
    e.ledger.assert_conserved();
}

/// **The band between the two parities cannot close**, at any town, for
/// any cargo that goes by sea.
///
/// Import parity adds every cost of getting a tonne here; export parity
/// takes every cost of getting one away. Whatever the world price, the
/// duty or the distance, the first is above the second — which is the
/// whole of why a town cannot buy abroad and sell abroad in the same breath
/// and keep the difference.
#[test]
fn the_border_is_a_band_and_it_cannot_close() {
    let mut n = nations(7, 4);
    for _ in 0..5 {
        n.economy.step();
    }
    let e = &n.economy;
    let mut checked = 0;
    for m in 0..e.markets.len() {
        for &c in Commodity::ALL.iter() {
            if !c.will_go_on_a_ship() {
                continue;
            }
            let (i, x) = (e.import_parity(m, c), e.export_parity(m, c));
            assert!(
                i > x,
                "{c} at {}: import parity {i:.1} is not above export parity {x:.1}",
                e.markets[m].name
            );
            checked += 1;
        }
    }
    assert!(checked > 100, "only {checked} bands were checked");
}

/// **A town up-country pays the road both ways.**
///
/// Its imports come up from the quay and its exports go down to it, so its
/// band is wider than the port's on both sides — by exactly the haul. That
/// is why a landlocked town pays more for imported grain than the port it
/// comes through, and gets less for its own than the port does.
#[test]
fn a_town_up_country_pays_the_road_both_ways() {
    let mut n = nations(7, 4);
    for _ in 0..5 {
        n.economy.step();
    }
    let e = &n.economy;
    let mut inland = 0;
    for m in 0..e.markets.len() {
        let Some(q) = e.nearest_quay(m) else { continue };
        if q == m {
            continue;
        }
        inland += 1;
        for &c in [Commodity::Grain, Commodity::Steel, Commodity::Medicine].iter() {
            assert!(
                e.import_parity(m, c) > e.import_parity(q, c),
                "{c} is no dearer to import at {} than at its quay {}",
                e.markets[m].name,
                e.markets[q].name
            );
            assert!(
                e.export_parity(m, c) < e.export_parity(q, c),
                "{c} fetches as much for export at {} as at its quay {}",
                e.markets[m].name,
                e.markets[q].name
            );
        }
    }
    assert!(inland > 0, "no town in this world is inland of a quay");
}

/// **What crosses an ocean is what is worth carrying.**
///
/// Carriage is charged by weight, so what makes it bite is how much a tonne
/// is worth: a sixth of the price of bulk grain, a third of cement, and
/// under a hundredth of medicine. So the band a town neither imports nor
/// exports in is wide for cheap bulk and narrow for dear goods — which is
/// why about 3-4% of the world's cement crosses a border and a
/// pharmaceutical plant supplies whole continents.
#[test]
fn what_crosses_an_ocean_is_what_is_worth_carrying() {
    let mut n = nations(7, 4);
    for _ in 0..5 {
        n.economy.step();
    }
    let e = &n.economy;
    let q = (0..e.markets.len())
        .find(|&m| e.quay(m))
        .expect("no quay in this world");
    let band = |c: Commodity| (e.import_parity(q, c) - e.export_parity(q, c)) / c.world_price();
    let by_value = [
        Commodity::Cement,
        Commodity::Steel,
        Commodity::Machinery,
        Commodity::Medicine,
    ];
    for w in by_value.windows(2) {
        assert!(
            band(w[0]) > band(w[1]),
            "{} ({:.2} of its price) has no wider a band than {} ({:.2}), though a tonne of \
             it is worth far less",
            w[0],
            band(w[0]),
            w[1],
            band(w[1])
        );
    }
}

/// **Every town in a merged world has its services and its state.**
///
/// Both are posts against population, sized when a region is built, and
/// folding several regions into one economy left a service sector and a
/// public one for the first nation's towns and none for anybody else's —
/// 37% of employment in private services and another sixth in the public
/// sector, unpaid in three nations of four. Money still flowed *in*: the
/// freight every guest firm paid landed in service accounts that never
/// paid it out, so the guests' households drained to under one unit a
/// head while their service accounts held billions. The whole suite was
/// green throughout, because nothing asked.
#[test]
fn every_town_in_a_merged_world_has_its_services_and_its_state() {
    use scale_sim::money::{Account, Why};
    let mut n = nations(7, 4);
    {
        let e = &n.economy;
        let svc = e.services.as_ref().expect("a world with no service sector");
        for m in 0..e.markets.len() {
            // **And it is that town's own state**, one per nation: a world
            // with a single exchequer paid a poor nation's teachers out of
            // a rich one's tax and could not hold a weak state beside a
            // strong one at all.
            let gov = e
                .government(m)
                .unwrap_or_else(|| panic!("{} has no state over it", e.markets[m].name));
            assert!(
                svc.total_in(m) > 0.0 && gov.posts_in(m) > 0.0,
                "{} (nation {}) has {:.0} service posts and {:.0} public ones",
                e.markets[m].name,
                e.markets[m].nation,
                svc.total_in(m),
                gov.posts_in(m)
            );
        }
    }

    // **And the money that goes into them comes out.** What each town's
    // service sector paid its people over the run, against what it holds.
    let mut paid_out = vec![0.0f64; n.economy.markets.len()];
    for _ in 0..200 {
        n.economy.step();
        for t in n.economy.treasury.today.iter() {
            if let Account::ServiceSector(m) = t.from {
                if t.why == Why::Payroll || t.why == Why::Profit {
                    paid_out[m] += t.amount;
                }
            }
        }
    }
    let e = &n.economy;
    for m in 0..e.markets.len() {
        let held = e.treasury.balance(Account::ServiceSector(m));
        assert!(
            paid_out[m] > 0.0 && held < paid_out[m],
            "{}'s service sector paid out {:.3e} in 200 days and is sitting on {held:.3e}",
            e.markets[m].name,
            paid_out[m]
        );
    }
}

/// **A company's profit goes to its shareholders, and they do not all live
/// next to the works.**
///
/// It was paid to the households of the firm's own town, so a town that
/// consumes more than it makes sent money out through its shops and nothing
/// brought it back — five towns drained to under three units a head, the
/// largest city in the world among them. The form follows the size: under
/// about six hands the owner works the till and the profit is his, in his
/// town; past fifty the owners "generally do not work there at all", and
/// pension funds and share registers spread them across the country.
#[test]
fn a_companys_profit_reaches_the_nation_and_a_proprietors_stays_home() {
    use scale_sim::building::Ownership;
    use scale_sim::money::{Account, Why};
    let mut n = nations(7, 4);
    // Who each firm's profit reached, and how big the firm was that day.
    let mut reached: std::collections::BTreeMap<usize, std::collections::BTreeSet<usize>> =
        Default::default();
    let mut companies = std::collections::BTreeSet::new();
    let mut proprietors = std::collections::BTreeSet::new();
    for _ in 0..120 {
        n.economy.step();
        let e = &n.economy;
        for t in e.treasury.today.iter() {
            if let (Account::Firm(s), Account::Households(m), Why::Profit) = (t.from, t.to, t.why) {
                reached.entry(s).or_default().insert(m);
                let staff = e.staff_today.get(s).copied().unwrap_or(0.0);
                if Ownership::for_size(staff).owner_works_there() {
                    proprietors.insert(s);
                } else {
                    companies.insert(s);
                }
            }
        }
    }
    let e = &n.economy;
    assert!(
        !companies.is_empty(),
        "no company paid a dividend in 120 days, so the gate proves nothing"
    );
    for &s in companies.iter() {
        let home = e.markets[e.ledger.sites[s].market].nation;
        let nation: std::collections::BTreeSet<usize> = (0..e.markets.len())
            .filter(|&m| e.markets[m].nation == home)
            .collect();
        assert!(
            reached[&s] == nation,
            "{} is a company and its profit reached {} of its nation's {} towns",
            e.ledger.sites[s].name,
            reached[&s].len(),
            nation.len()
        );
    }
    for &s in proprietors.iter() {
        if companies.contains(&s) {
            continue; // grew or shrank across the line during the run
        }
        let own = e.ledger.sites[s].market;
        assert!(
            reached[&s].iter().all(|&m| m == own),
            "{} is run by its owner and paid profit outside its own town",
            e.ledger.sites[s].name
        );
    }
}

/// **Somewhere is always harvesting**, so a town fed by ships does not have
/// to carry a whole crop year in its sheds.
///
/// The northern crop comes in from May to September and the southern from
/// October to February. The world holds about 30% of a year's cereal use
/// *(FAO, 2024/25)*, the FAO's minimum safe level is 17-18% — two months —
/// and Egypt, the largest wheat importer, keeps four to six months counting
/// cargoes afloat. Aiming a town that lands grain from abroad at the 150
/// days a single-harvest country carries left every such town reading half
/// its target with a full terminal, and pricing grain at three times the
/// world.
#[test]
fn a_town_fed_by_ships_can_hold_what_it_aims_at() {
    let mut n = nations(7, 4);
    for _ in 0..300 {
        n.economy.step();
    }
    let e = &n.economy;
    let grain = Commodity::Grain;
    let mut fed_by_ships = Vec::new();
    for m in 0..e.markets.len() {
        let days = e.stock_days(m, grain);
        // Never more than the season, never less than the resupplied
        // figure — a blend and not an extrapolation.
        assert!(
            days <= grain.target_cover_days() + 1e-9
                && days >= grain.stock_days_if_resupplied().unwrap() - 1e-9,
            "{} aims at {days:.0} days of grain",
            e.markets[m].name
        );
        // **Which towns live on ships is read off their terminals**, not off
        // `stock_days` — the function whose getting it wrong is what this
        // watches for. Asked through it, the sabotage empties the list and
        // the gate fails for the wrong reason.
        let can_land: f64 = (0..e.ledger.sites.len())
            .filter(|&s| {
                let site = &e.ledger.sites[s];
                site.market == m
                    && site.recipe.is_some_and(|r| {
                        scale_sim::econ::RECIPES[r].from_abroad
                            && scale_sim::econ::RECIPES[r]
                                .outputs
                                .iter()
                                .any(|&(oc, _)| oc == grain)
                    })
            })
            .map(|s| e.ledger.sites[s].throughput)
            .sum();
        let draw = e.daily_draw(m, grain);
        if draw > 1e-9 && can_land >= 0.7 * draw {
            fed_by_ships.push(m);
        }
    }
    assert!(
        !fed_by_ships.is_empty(),
        "no town in this world lives on imported grain, so the gate proves nothing"
    );

    // **And it can reach it.** What a ship-fed town holds against what it
    // aims at; against a crop year's target the same terminals read half.
    let reaching = fed_by_ships
        .iter()
        .filter(|&&m| e.markets[m].expected_cover[grain as usize] >= 0.8 * e.target_cover(m, grain))
        .count();
    assert!(
        reaching * 4 >= fed_by_ships.len() * 3,
        "only {reaching} of {} ship-fed towns hold what they aim at: {:?}",
        fed_by_ships.len(),
        fed_by_ships
            .iter()
            .map(|&m| (
                e.markets[m].name.clone(),
                (e.markets[m].expected_cover[grain as usize] / e.target_cover(m, grain) * 100.0)
                    .round()
            ))
            .collect::<Vec<_>>()
    );
}

/// **A fishing village is not a container port, and the water decides.**
///
/// A quay was a flag, so any coastal town could ship a country's whole
/// harvest in a morning — and it did: exports outran the price signal,
/// because a stored staple is priced off a deliberately slow average of
/// cover, so the drain never told anybody to stop. `trade` then pulled the
/// rest of the country's surplus to the coast to follow it out.
///
/// The ceiling is physical and it comes from the sea. The elevation field
/// has always run below sea level — that is what makes a cell ocean — and
/// **nothing had ever read it as water.** Real draughts: an inshore boat
/// wants 2-3 m, a coaster 5-7, a Panamax 12, a capesize bulk carrier
/// 17-18. So a bay eight metres deep can load timber and cannot load ore,
/// which is a real reason ore ports are few.
#[test]
fn what_can_tie_up_is_decided_by_the_water() {
    use scale_sim::world::{Berth, World, SHELF_M};

    let world = World::generate(384, 216, 7);

    // **Dry land has no depth**, and the sea does. Both halves matter: a
    // depth on a hillside would make every town a port.
    let mut wet = 0usize;
    let mut dry = 0usize;
    let mut deepest = 0.0f64;
    for cell in 0..world.width * world.height {
        match world.depth_m(cell) {
            Some(d) => {
                assert!(d >= 0.0, "water {d} m deep");
                deepest = deepest.max(d);
                wet += 1;
            }
            None => dry += 1,
        }
    }
    assert!(wet > 0 && dry > 0, "{wet} wet cells and {dry} dry ones");
    assert!(
        deepest > SHELF_M,
        "the deepest water on this planet is {deepest:.0} m, which is still on the shelf"
    );

    // **The shelf is where everything is.** Real continental shelf is
    // about 130 m at its outer edge and it is the ground every port on
    // earth stands on, so most of the sea near land must be shallow rather
    // than most of it being abyss.
    let mut on_the_shelf = 0usize;
    let mut abyss = 0usize;
    for cell in 0..world.width * world.height {
        if let Some(d) = world.depth_m(cell) {
            if d <= SHELF_M {
                on_the_shelf += 1;
            } else {
                abyss += 1;
            }
        }
    }
    assert!(
        on_the_shelf > 0 && abyss > 0,
        "shelf {on_the_shelf}, deep {abyss} — a planet is not all one or the other"
    );

    // **And the berth follows the draught, in the order the ships do.**
    assert_eq!(Berth::for_depth(1.0), Berth::None);
    assert_eq!(Berth::for_depth(4.0), Berth::Fishing);
    assert_eq!(Berth::for_depth(8.0), Berth::Coaster);
    assert_eq!(Berth::for_depth(14.0), Berth::Ocean);
    assert_eq!(Berth::for_depth(25.0), Berth::Deep);
    let mut last = -1.0;
    for m in [1.0, 4.0, 8.0, 14.0, 25.0] {
        let takes = Berth::for_depth(m).a_days_loading();
        assert!(
            takes > last,
            "deeper water at {m} m loads no faster than shallower"
        );
        last = takes;
    }

    // **A country's ports are the towns on the water, and what they can
    // take varies.** If every quay in a world were the same class, the
    // water would be decorative.
    let mut n = nations(7, 4);
    for _ in 0..40 {
        n.economy.step();
    }
    let classes: std::collections::BTreeSet<Berth> = n
        .economy
        .markets
        .iter()
        .filter(|m| m.port)
        .map(|m| m.berth)
        .collect();
    assert!(
        !classes.is_empty(),
        "no town in four nations is on the water"
    );
    for m in n.economy.markets.iter().filter(|m| !m.port) {
        assert_eq!(m.berth, Berth::None, "{} is inland and has a berth", m.name);
    }
}

/// **A country's taxes pay its own teachers.**
///
/// `Account::State` carried nothing on it, so a merged world had **one
/// exchequer for however many countries were in it**: it collected at every
/// till on the planet and staffed every town, which quietly paid a poor
/// nation's schools out of a rich neighbour's tax. That is not a currency
/// union — a union has one money and many governments — it is one country,
/// and this world is supposed to hold several.
///
/// The gate is the identity rather than a level: every penny of tax reaches
/// the state of the nation the till stands in, and every penny of public
/// spending leaves the state of the nation the wage is paid in. A single
/// crossing is a defect and there is no tolerance to set.
#[test]
fn a_countrys_taxes_pay_its_own_teachers() {
    use scale_sim::money::Account;
    let mut n = nations(7, 4);
    let mut taxed = 0.0f64;
    let mut spent = 0.0f64;
    let mut crossings = Vec::new();
    for _ in 0..200 {
        n.economy.step();
        let e = &n.economy;
        let nation_of_site = |s: usize| e.markets[e.ledger.sites[s].market].nation;
        for t in e.treasury.today.iter() {
            match (t.from, t.to) {
                (Account::Firm(s), Account::State(g)) => {
                    taxed += t.amount;
                    if nation_of_site(s) != g {
                        crossings.push(format!(
                            "day {}: nation {} paid {:.0} of tax to the state of {g}",
                            e.ledger.day,
                            nation_of_site(s),
                            t.amount
                        ));
                    }
                }
                (Account::State(g), Account::Households(m)) => {
                    spent += t.amount;
                    if e.markets[m].nation != g {
                        crossings.push(format!(
                            "day {}: the state of {g} paid {:.0} to households in nation {}",
                            e.ledger.day, t.amount, e.markets[m].nation
                        ));
                    }
                }
                (Account::State(g), Account::Firm(s)) => {
                    spent += t.amount;
                    if nation_of_site(s) != g {
                        crossings.push(format!(
                            "day {}: the state of {g} paid {:.0} to a firm in nation {}",
                            e.ledger.day,
                            t.amount,
                            nation_of_site(s)
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    assert!(
        taxed > 0.0 && spent > 0.0,
        "nothing was taxed or spent at all, so the gate proves nothing: \
         {taxed:.0} in, {spent:.0} out"
    );
    crossings.truncate(4);
    assert!(
        crossings.is_empty(),
        "public money crossed a border: {crossings:?}"
    );
    // And every nation actually has one, rather than one of them having all
    // the tills.
    let e = &n.economy;
    for nation in e.nations() {
        assert!(
            e.governments.contains_key(&nation),
            "nation {nation} has no state"
        );
        assert!(
            e.treasury.balance(Account::State(nation)) != 0.0,
            "the state of nation {nation} never held a penny"
        );
        // **And it employs its own people, not the planet's.** `govern`
        // walked every market in the economy, so each nation's government
        // staffed every town on it and all four came out with an identical
        // establishment — the whole world's, four times over. The money
        // stayed straight because the spending filtered again on the way
        // out, which is how the first filter being missing stayed hidden.
        //
        // Real staffing sums to about 8.7% of the population across
        // health, education, administration, safety and defence, and a
        // fully funded state reaches it.
        let people: f64 = e
            .markets
            .iter()
            .filter(|m| m.nation == nation)
            .map(|m| m.population)
            .sum();
        let posts: f64 = e.governments[&nation].posts.iter().sum();
        let share = posts / people.max(1.0);
        assert!(
            (0.02..0.12).contains(&share),
            "the state of nation {nation} employs {:.1}% of its people, against a real \
             8.7% for a fully funded one",
            100.0 * share
        );
    }
}

/// **A weak state can stand next to a strong one.**
///
/// The thing a single exchequer made impossible. `Capacity` is this
/// project's account of why weak states stay weak — effective tax takes run
/// 35-50% for a high-capacity developed state and 10-18% where control is
/// thin, and under-funding shows up as **fewer people**, not a worse
/// multiplier — and with one government for the whole world there was only
/// ever one capacity, so the comparison could not be made inside a world at
/// all.
#[test]
fn a_weak_state_can_stand_next_to_a_strong_one() {
    use scale_sim::money::Account;
    use scale_sim::state::{Capacity, Government};
    let mut n = nations(7, 4);
    let all = n.economy.nations();
    assert!(all.len() >= 2, "this fixture has only one nation in it");
    let (rich, poor) = (all[0], all[1]);
    for (nation, capacity) in [(rich, Capacity::Developed), (poor, Capacity::Weak)] {
        let gov = Government::govern(&n.economy, capacity, nation);
        n.economy.governments.insert(nation, gov);
    }
    for _ in 0..200 {
        n.economy.step();
    }
    let e = &n.economy;
    let people = |g: u16| -> f64 {
        e.markets
            .iter()
            .filter(|m| m.nation == g)
            .map(|m| m.population)
            .sum::<f64>()
            .max(1.0)
    };
    // **Under-funding shows up as fewer people**, not as a worse
    // multiplier, which is this project's own account of what a weak state
    // is. Both states meet nearly all of their own payroll — 0.983 against
    // 0.989 — because a state that cannot collect hires fewer teachers and
    // then pays the ones it has, so what share of the bill was met
    // discriminates nothing. The establishment does.
    let posts = |g: u16| {
        e.governments
            .get(&g)
            .map(|gov| gov.posts.iter().sum::<f64>())
            .unwrap_or(0.0)
            / people(g)
    };
    assert!(
        posts(rich) > posts(poor) * 1.2,
        "a developed state employs {:.4} of its people against a weak one's {:.4}",
        posts(rich),
        posts(poor)
    );
    // And it is poorer in the plainest sense: less in its treasury for
    // everybody it has to serve.
    let per_head = |g: u16| e.treasury.balance(Account::State(g)) / people(g);
    assert!(
        per_head(rich) > per_head(poor),
        "the developed state holds {:.4} a head against the weak one's {:.4}",
        per_head(rich),
        per_head(poor)
    );
}
