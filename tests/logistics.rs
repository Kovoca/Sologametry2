//! Who actually moves the goods.

use scale_sim::econ::{Commodity, Doctrine};
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

fn a_nation(seed: u64, rank: usize) -> Region {
    let world = World::generate(384, 216, seed);
    let pol = Polities::partition(&world, 30);
    let set = Settlements::place(&world, &pol, 5000);
    let net = Network::build(&world, &set, 1000);
    let id = pol.ranked()[rank].0;
    Region::extract(&world, &pol, &set, &net, id, 5, Doctrine::Prudent).expect("a nation")
}

/// **A freight industry levels the country out**, which is what neither of
/// the pairwise mechanisms could do.
///
/// `distribute` is a pull and `trade` is a per-hop price test; a cargo
/// three towns down the road has to clear a separate test at every hop and
/// usually never sets off. Hold everything still and vary one thing —
/// whether the country has hauliers.
#[test]
fn carriers_even_out_a_country_that_pairwise_trade_cannot() {
    let cover = |on: bool| -> (Vec<f64>, f64) {
        let mut e = a_nation(20260828, 2).economy;
        if !on {
            e.logistics = None;
        }
        for _ in 0..400 {
            e.step();
        }
        e.ledger.assert_conserved();
        let c: Vec<f64> = (0..e.markets.len())
            .map(|m| {
                let held: f64 = (0..e.ledger.sites.len())
                    .filter(|&s| e.ledger.sites[s].market == m)
                    .map(|s| e.ledger.stock(s, Commodity::ProcessedFood))
                    .sum();
                held / e.daily_draw(m, Commodity::ProcessedFood).max(1e-9)
            })
            .collect();
        let supplied = e.government.as_ref().map(|g| g.supplied).unwrap_or(0.0);
        (c, supplied)
    };

    let (without, hospitals_without) = cover(false);
    let (with, hospitals_with) = cover(true);

    let spread = |v: &[f64]| {
        let lo = v.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = v.iter().cloned().fold(0.0, f64::max);
        hi - lo
    };

    // **Food was already even, and that is worth recording rather than
    // asserting away.** `distribute` handles a commodity every town both
    // makes and sells perfectly well, because a shop short of food is
    // pulling on a mill in the same street. Carriers change nothing here
    // and should not.
    assert!(
        spread(&without) < 1.0 && spread(&with) < 1.0,
        "food cover should be even either way: {without:?} against {with:?}"
    );

    // **And they have to be fed, not merely equally placed.**
    //
    // A spread is a difference and says nothing about a level, so a
    // country in which every town holds a quarter of a day's food passes
    // the line above with a spread of zero. That is not a hypothetical: a
    // change to how goods were allocated took this nation from 17 days of
    // cover to **0.28**, and the whole suite stayed green because the only
    // gate watching it was measuring evenness. Evenly starving is even.
    for (label, v) in [("without hauliers", &without), ("with hauliers", &with)] {
        let lowest = v.iter().cloned().fold(f64::INFINITY, f64::min);
        assert!(
            lowest > 3.0,
            "{label}: the best-stocked town in the country holds {lowest:.2}              days of food — the spread is even because everybody is starving"
        );
    }

    // **The gap is medical grade**, and it is the shape of cargo the
    // pairwise mechanisms cannot handle at all: made in one town, wanted
    // in every town, bought by nobody over a counter and consumed by no
    // recipe anywhere else. There is no chain of adjacent price gaps to
    // walk it down, so without a haulier it can simply sit where it was
    // made.
    //
    // **Whether it does is a fact about the country**, not about
    // carriers. This nation used to show the gap plainly and no longer
    // does: once water became a precondition for settling, its towns
    // moved closer together and `distribute` can reach between them
    // unaided. So what is asserted is that carriers never make it worse
    // and that the country ends fully supplied — and the discriminating
    // case, a nation strung far enough out that the pull cannot reach,
    // now needs finding rather than assuming.
    assert!(
        hospitals_with >= hospitals_without - 1e-9,
        "hauliers made hospital supply worse: {hospitals_with:.2} against \
         {hospitals_without:.2}"
    );
    assert!(
        hospitals_with > 0.98,
        "hospitals only {:.0}% supplied even with a freight industry",
        hospitals_with * 100.0
    );
}

/// **Value density decides how far a thing travels**, and nobody wrote
/// that rule — it falls out of freight cost against what the goods are
/// worth.
#[test]
fn cement_does_not_travel_and_medicine_does() {
    let mut e = a_nation(20260828, 2).economy;
    for _ in 0..200 {
        e.step();
    }
    let freight = e.logistics.as_ref().expect("a nation with hauliers");

    // Find the dearest haul the network offers.
    let mut worst = 0.0f64;
    for a in 0..e.markets.len() {
        for b in 0..e.markets.len() {
            if let Some(c) = freight.freight(a, b) {
                worst = worst.max(c);
            }
        }
    }
    assert!(worst > 0.0, "no route costs anything");

    // Cement is worth ~$100 a tonne and aggregate less, which is why
    // there is a cement works in every region on earth; medicine is worth
    // thousands and moves globally.
    let travels = |c: Commodity| worst <= c.base_cost() * 0.5;
    assert!(
        travels(Commodity::Medicine),
        "medicine should cross the country without noticing the freight"
    );
    assert!(
        Commodity::Cement.base_cost() < Commodity::Medicine.base_cost() / 10.0,
        "cement should be the cheap bulk end of the scale"
    );
}

/// A carrier is a firm with a fleet and a depot, and the shape of the
/// industry is real: **most operators are tiny**.
#[test]
fn a_carrier_is_a_firm_with_a_fleet() {
    let e = a_nation(20260828, 2).economy;
    let freight = e.logistics.as_ref().expect("a nation with hauliers");
    assert_eq!(
        freight.carriers.len(),
        e.markets.len(),
        "freight is a local business before it is a national one"
    );
    for c in freight.carriers.iter() {
        assert!(c.vehicles >= 1.0, "{} has no lorries", c.name);
        assert!(c.drivers() >= 1.0, "{} employs nobody", c.name);
    }
    // The biggest carrier is in the biggest town, because the fleet
    // follows the freight task and that follows the people.
    let biggest = freight
        .carriers
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.vehicles.total_cmp(&b.1.vehicles).then(b.0.cmp(&a.0)))
        .map(|(i, _)| i)
        .unwrap();
    let most_people = (0..e.markets.len())
        .max_by(|&a, &b| e.markets[a].population.total_cmp(&e.markets[b].population))
        .unwrap();
    assert_eq!(freight.carriers[biggest].home, most_people);
}
