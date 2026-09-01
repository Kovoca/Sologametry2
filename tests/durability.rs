//! Things wear out, and that is what a service sector is for.

use scale_sim::econ::{Doctrine, SiteKind, DAYS_PER_YEAR};
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

fn a_nation() -> Region {
    let world = World::generate(384, 216, 20260828);
    let pol = Polities::partition(&world, 30);
    let set = Settlements::place(&world, &pol, 5000);
    let net = Network::build(&world, &set, 1000);
    let id = pol.ranked()[2].0;
    Region::extract(&world, &pol, &set, &net, id, 5, Doctrine::Prudent).expect("a nation")
}

fn run(years: u64, builders: bool) -> scale_sim::econ::Economy {
    let mut e = a_nation().economy;
    if !builders {
        for s in e.ledger.sites.iter_mut() {
            if s.kind == SiteKind::Builders {
                s.throughput = 0.0;
            }
        }
    }
    for _ in 0..(DAYS_PER_YEAR * years) {
        e.step();
    }
    e
}

/// **A building that is not kept up does not vanish; it degrades.**
///
/// This is what the service sector is *for*, and until now nothing in the
/// model had a condition for anyone to maintain — construction was 6.4% of
/// employment producing an abstraction. Real maintenance runs 1-2% of a
/// building's capital value a year: a roof lasts 25-50 years, wiring
/// 30-40, a boiler 15.
#[test]
fn a_town_that_is_not_maintained_falls_apart() {
    let kept = run(20, true);
    let neglected = run(20, false);

    let a = kept.fabric_condition(0);
    let b = neglected.fabric_condition(0);

    assert!(
        b < a,
        "twenty years without a builder left the fabric at {b:.2} against \
         {a:.2} with one, which is no difference at all"
    );
    // **A decade of neglect is visible and a year is not**, the same shape
    // as the road network's maintenance deficit and for the same reason:
    // the shortfall compounds slowly enough that whoever let it happen is
    // long gone before it shows.
    assert!(
        b < 0.5,
        "twenty years of no upkeep whatever and the fabric is still {b:.2}"
    );
    let early = run(2, false);
    assert!(
        early.fabric_condition(0) > 0.8,
        "two years of neglect should barely show, not take it to {:.2}",
        early.fabric_condition(0)
    );
}

/// **Degraded stock is cheap stock**, which is how under-maintained
/// housing becomes the only housing some people can afford — and why
/// letting it go is a decision somebody makes rather than an accident.
#[test]
fn a_worn_out_town_is_a_cheap_town() {
    let kept = run(20, true);
    let neglected = run(20, false);

    // The building is worth less. The land under it is not, which is why
    // the price falls by less than the condition does.
    assert!(
        neglected.house_price(0) < kept.house_price(0),
        "a town left to rot costs the same to buy into as one kept up"
    );
    let ratio = neglected.house_price(0) / kept.house_price(0);
    assert!(
        (0.2..0.95).contains(&ratio),
        "houses in the neglected town are {ratio:.2} of the kept one, which is \
         either no effect or the land has stopped counting"
    );
}

/// The stock is a real quantity, not a multiplier.
#[test]
fn the_fabric_is_measured_in_tonnes_of_building() {
    let e = run(1, true);
    for m in 0..e.markets.len() {
        let stock = e.building_stock[m];
        let per_head = stock / e.markets[m].population;
        // Real building stock is 50-60 tonnes a head once dwellings,
        // shops, works and civic buildings are counted — a 76 m² house is
        // about 150 tonnes and holds 2.4 people.
        assert!(
            (30.0..90.0).contains(&per_head),
            "{} holds {per_head:.0} tonnes of building a head",
            e.markets[m].name
        );
        assert!((0.0..=1.0).contains(&e.fabric_condition(m)));
    }
}

/// **Known: a maintained country still settles below new.**
///
/// With its building trade working flat out this nation's fabric settles
/// near 0.78 rather than 1.00, because the trade is materials-limited
/// about half the time even with a kiln in every town and the coal for
/// them mined. Some of that is real — building stock does age — and some
/// of it is a supply chain that has not been chased all the way down.
///
/// Asserted at the level it actually reaches so that a change either way
/// is visible, rather than pretending the equilibrium is where the
/// arithmetic says it ought to be.
#[test]
fn maintenance_holds_the_line_without_quite_restoring_it() {
    let kept = run(20, true);
    let c = kept.fabric_condition(0);
    assert!(
        (0.6..0.95).contains(&c),
        "a maintained town settled at {c:.2}; if this has moved, the \
         building trade's supply chain has changed"
    );
}
