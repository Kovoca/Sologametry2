//! **What is paid is what is costed.**
//!
//! Real firms mostly price as a markup over their costs — 54% of euro-area
//! firms say so outright *(Fabiani et al.)*, and most of Blinder's American
//! price-setters rate costs as the main driver — and wages are the largest
//! cost there is. Pay is re-set about once a year against the cost of
//! living. So a pay rise reaches prices, in proportion to how much of a
//! thing is labour.
//!
//! The model had two wage scales about thirty-five times apart and they
//! never met: every cost of production was built on a constant, and a
//! labourer was paid on a different scale entirely. Raising wages made
//! everybody richer and nothing dearer.

use scale_sim::econ::{recipe, Commodity, Doctrine, Economy, RECIPES};
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Nations;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

fn a_world() -> Economy {
    let world = World::generate(384, 216, 7);
    let polities = Polities::partition(&world, 24);
    let settlements = Settlements::place(&world, &polities, 3000);
    let network = Network::build(&world, &settlements, 500);
    Nations::build(
        &world,
        &polities,
        &settlements,
        &network,
        4,
        4,
        Doctrine::Prudent,
    )
    .economy
}

/// The first site running `r`, and the commodity it makes.
fn a_site_running(e: &Economy, r: usize) -> (usize, Commodity) {
    let s = (0..e.ledger.sites.len())
        .find(|&s| e.ledger.sites[s].recipe == Some(r) && e.ledger.sites[s].throughput > 0.0)
        .unwrap_or_else(|| panic!("no site in this world runs {}", RECIPES[r].name));
    (s, RECIPES[r].outputs[0].0)
}

/// **A pay rise reaches prices, in proportion to how much of the thing is
/// labour.**
///
/// A tenth on a nation's pay moves the cost of what it makes by labour's
/// share of it: a lot for a factory that puts fifty-five hours into a tonne
/// of goods, hardly at all for a steelworks that puts in one and a half
/// against a furnace, ore and coal.
///
/// **The rise is given to the whole nation, because that is the figure a
/// cost is built on** — pay is settled nationally and by industry, and
/// `Economy::national_wage_level` is what `site_cost` reads. Raising two
/// towns instead moves the national mean by their share of the nation's
/// people, which would make this gate measure how big those two towns
/// happen to be.
#[test]
fn a_pay_rise_reaches_prices_by_labours_share() {
    let mut e = a_world();
    for _ in 0..30 {
        e.step();
    }
    let (factory, goods) = a_site_running(&e, recipe::FACTORY);
    let (steelworks, steel) = a_site_running(&e, recipe::STEELWORKS);

    let before_goods = e.site_cost(factory, goods).unwrap();
    let before_steel = e.site_cost(steelworks, steel).unwrap();
    let nations: Vec<u16> = [factory, steelworks]
        .iter()
        .map(|&s| e.markets[e.ledger.sites[s].market].nation)
        .collect();
    let before_level: Vec<f64> = nations.iter().map(|&n| e.national_wage_level(n)).collect();

    for m in 0..e.markets.len() {
        if nations.contains(&e.markets[m].nation) {
            e.workforce[m].wage_index *= 1.10;
        }
    }
    let goods_rose = e.site_cost(factory, goods).unwrap() / before_goods - 1.0;
    let steel_rose = e.site_cost(steelworks, steel).unwrap() / before_steel - 1.0;

    // The pay rise really was a tenth, measured on the figure the costs
    // actually read, or the gate is measuring nothing.
    for (i, &n) in nations.iter().enumerate() {
        assert!(
            (e.national_wage_level(n) / before_level[i] - 1.10).abs() < 1e-9,
            "a tenth on every wage index in nation {n} was not a tenth on the nation's pay"
        );
    }

    assert!(
        goods_rose > 0.02 && goods_rose < 0.10 * scale_sim::econ::WAGE_SHARE_OF_VALUE_ADDED + 1e-9,
        "a tenth on the wage moved the cost of goods by {:.2}% — labour is most of a \
         factory's added value and a little over half of that is pay",
        goods_rose * 100.0
    );
    assert!(
        steel_rose >= 0.0 && steel_rose < goods_rose / 3.0,
        "steel rose {:.2}% against goods' {:.2}% — a steelworks is a furnace and an ore \
         yard with a few people in it",
        steel_rose * 100.0,
        goods_rose * 100.0
    );
}

/// **And the loop it closes settles.**
///
/// Dearer food raises pay a third of a year later, pay raises the cost of
/// growing, milling and canning the food, and that raises its price again
/// — by about half as much, because labour is a share of a share. A loop
/// that gives back half of each push settles at about twice the first push
/// and never runs away; one that gave back more than it took would be a
/// wage-price spiral with nothing to stop it.
#[test]
fn the_wage_price_loop_settles() {
    let mut e = a_world();
    let level = |e: &Economy| {
        let pop: f64 = e.markets.iter().map(|m| m.population).sum();
        (0..e.markets.len())
            .map(|m| e.wage_level(m) * e.markets[m].population)
            .sum::<f64>()
            / pop
    };
    let mut readings = Vec::new();
    for day in 0..730 {
        e.step();
        if day % 73 == 72 {
            readings.push(level(&e));
        }
    }
    let lo = readings.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = readings.iter().cloned().fold(0.0f64, f64::max);
    assert!(
        lo > 0.6 && hi < 1.5,
        "pay wandered from {lo:.3} to {hi:.3} of the reference over two years: {readings:?}"
    );
    // The second year no further from the first than the band a labour
    // market moves in, which is what settled means.
    let first: f64 = readings[..5].iter().sum::<f64>() / 5.0;
    let second: f64 = readings[5..].iter().sum::<f64>() / readings[5..].len() as f64;
    assert!(
        (second / first - 1.0).abs() < 0.10,
        "pay moved {:.1}% from the first year to the second: {readings:?}",
        (second / first - 1.0) * 100.0
    );
}
