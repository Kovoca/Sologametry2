//! Ore, steel, and the things made of steel.
//!
//! Until this chain existed the metal `geology.rs` had been placing since
//! it was written had no consumer at all, and every manufactured article a
//! country used appeared at a depot from nowhere.

use scale_sim::econ::{recipe, Commodity, Doctrine, SiteKind, RECIPES};
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

/// **Coal is the reductant, not the fuel.**
///
/// A blast furnace uses carbon to strip the oxygen off iron oxide, which
/// is why the real BF-BOF recipe is 1.4 t of ore and 0.8 t of coal per
/// tonne of crude steel but only 200-300 kWh of *electricity*. A country
/// with unlimited power and no coal cannot make primary steel — which is
/// the whole reason steel is hard to decarbonise.
#[test]
fn steel_needs_coal_and_not_merely_power() {
    let r = &RECIPES[recipe::STEELWORKS];
    let ore = r
        .inputs
        .iter()
        .find(|&&(c, _)| c == Commodity::IronOre)
        .expect("a steelworks with no ore")
        .1;
    let coal = r
        .inputs
        .iter()
        .find(|&&(c, _)| c == Commodity::Coal)
        .expect("coal is an input, not a utility bill")
        .1;
    assert!((ore - 1.4).abs() < 0.01, "real BF-BOF is 1.4 t of ore");
    assert!((coal - 0.8).abs() < 0.01, "real BF-BOF is 0.8 t of coal");
    // The electricity is the small part: 24 GJ a tonne of total energy,
    // almost all of it the coal itself.
    assert!(
        r.power < 0.4,
        "a steelworks draws {:.2} MWh a tonne; real is 0.2-0.3",
        r.power
    );

    // And fabrication is where the people are, not the metal.
    assert!(
        RECIPES[recipe::FACTORY].labour > RECIPES[recipe::STEELWORKS].labour * 10.0,
        "basic materials are capital-intensive and fabrication is not"
    );
}

/// **A can is made of steel**, and canning was born of the tinplate
/// industry rather than alongside it.
#[test]
fn a_tin_of_food_contains_a_tin() {
    let steel = RECIPES[recipe::CANNERY]
        .inputs
        .iter()
        .find(|&&(c, _)| c == Commodity::Steel)
        .expect("a cannery that makes cans out of nothing")
        .1;
    // 35 kg a tonne of canned food puts metal packaging at ~14 kg a head a
    // year, which is what developed countries actually get through.
    let per_head = steel * Commodity::ProcessedFood.per_capita_annual() * 1000.0;
    assert!(
        (8.0..25.0).contains(&per_head),
        "metal packaging comes to {per_head:.0} kg a head a year; real is 10-15"
    );
}

/// The whole chain turns, and the country eats.
#[test]
fn the_chain_runs_from_the_orefield_to_the_shop() {
    for (seed, rank) in [(20260828u64, 2usize), (424242, 0)] {
        let mut e = a_nation(seed, rank).economy;
        for _ in 0..400 {
            e.step();
        }
        e.ledger.assert_conserved();

        let ran = |k: SiteKind| -> f64 {
            e.ledger
                .sites
                .iter()
                .filter(|s| s.kind == k)
                .map(|s| s.ran)
                .sum()
        };
        assert!(ran(SiteKind::IronMine) > 0.0, "no ore was raised (seed {seed})");
        assert!(ran(SiteKind::Steelworks) > 0.0, "nothing was smelted (seed {seed})");
        assert!(ran(SiteKind::Works) > 0.0, "nothing was made (seed {seed})");

        // **A routine economy must not produce a famine.** This chain
        // starved a nation twice while it was being built: once because
        // the cannery had nowhere to put the tinplate it now needed, and
        // once because a works a thousand kilometres from the only
        // steelworks could not get any metal. Both read as a food crisis
        // and both were a shortage of cans.
        assert_eq!(
            e.unmet_demand[Commodity::ProcessedFood as usize].round(),
            0.0,
            "seed {seed}: people went without food in a working nation"
        );
        let food = e.price(0, Commodity::ProcessedFood);
        assert!(
            food < Commodity::ProcessedFood.base_cost() * 1.5,
            "seed {seed}: food settled at {food:.0}, against a cost of {:.0}",
            Commodity::ProcessedFood.base_cost()
        );
    }
}

/// **Households take about a quarter of a country's electricity**, not all
/// of it. `per_capita_annual` stood in for the whole economy's demand back
/// when no industry was modelled, and the moment factories existed the
/// country counted them twice — household demand alone came to 271,000 MWh
/// a day against industry's 4,000, so the station burnt every tonne raised
/// to supply it and the steelworks on the same coalfield never smelted
/// anything.
#[test]
fn household_power_is_residential_only() {
    let per_head = Commodity::Electricity.per_capita_annual();
    assert!(
        (0.6..1.4).contains(&per_head),
        "households take {per_head} MWh a head a year; real residential is ~0.9 \
         against ~3.5 for the whole economy"
    );
}

/// **Every good leads back to something somebody dug up or cut down.**
///
/// Peter's requirement: recipes for each good, so primary resources go in
/// and finished goods come out. A single undifferentiated `RetailGoods`
/// made at a depot said nothing about what a country could make and what
/// it had to buy.
#[test]
fn every_good_traces_back_to_a_primary_resource() {
    /// A commodity nothing produces from anything — it comes out of the
    /// ground, off the land, or off a ship.
    fn primary(c: Commodity) -> bool {
        matches!(
            c,
            Commodity::IronOre
                | Commodity::Coal
                | Commodity::Petroleum
                | Commodity::Timber
                | Commodity::Grain
                | Commodity::Livestock
                | Commodity::Electricity
        )
    }

    // Walk the tree from retail goods down, and prove it terminates in
    // primaries rather than in a depot.
    fn roots(c: Commodity, depth: usize, seen: &mut Vec<Commodity>) {
        assert!(depth < 8, "the goods tree does not terminate at {c}");
        if primary(c) {
            if !seen.contains(&c) {
                seen.push(c);
            }
            return;
        }
        // Anything made must be made by a recipe that is not pure import.
        let made_from: Vec<Commodity> = RECIPES
            .iter()
            .filter(|r| r.outputs.iter().any(|&(oc, _)| oc == c))
            .filter(|r| !r.inputs.is_empty())
            .flat_map(|r| r.inputs.iter().map(|&(ic, _)| ic))
            .collect();
        assert!(
            !made_from.is_empty(),
            "{c} is conjured out of nothing by every recipe that makes it"
        );
        for input in made_from {
            roots(input, depth + 1, seen);
        }
    }

    let mut seen = Vec::new();
    roots(Commodity::RetailGoods, 0, &mut seen);
    for want in [
        Commodity::IronOre,
        Commodity::Coal,
        Commodity::Petroleum,
        Commodity::Timber,
    ] {
        assert!(
            seen.contains(&want),
            "a tonne of goods does not lead back to {want}; it reaches {seen:?}"
        );
    }

    // And the quantities are the real ones, per tonne of finished goods.
    let f = &RECIPES[recipe::FACTORY];
    let per = |c: Commodity| {
        f.inputs
            .iter()
            .find(|&&(ic, _)| ic == c)
            .map(|&(_, q)| q)
            .unwrap_or(0.0)
    };
    let machinery = per(Commodity::Machinery);
    let steel_in_machinery = RECIPES[recipe::MACHINE_WORKS]
        .inputs
        .iter()
        .find(|&&(c, _)| c == Commodity::Steel)
        .unwrap()
        .1;
    // World crude steel is ~230 kg a head a year and households take a
    // tonne of goods each.
    let steel_per_head = machinery * steel_in_machinery * 1000.0;
    assert!(
        (150.0..280.0).contains(&steel_per_head),
        "a tonne of goods carries {steel_per_head:.0} kg of steel; real is ~230"
    );
    // Real industrial roundwood is ~180 kg a head a year.
    let timber = per(Commodity::Timber) * 1000.0;
    assert!(
        (120.0..280.0).contains(&timber),
        "{timber:.0} kg of timber a head a year; real is ~180"
    );
}

/// **Oil is the most capital-intensive extraction there is**, which is why
/// oil wealth does not become employment — a field worth billions is run
/// by a few hundred people. It is a real and consequential fact about
/// petro-states, and it falls out of the labour figure.
#[test]
fn an_oil_field_employs_almost_nobody() {
    let oil = RECIPES[recipe::OIL_FIELD].labour;
    let works = RECIPES[recipe::MACHINE_WORKS].labour;
    assert!(
        works > oil * 100.0,
        "a machine works is {works} person-hours a tonne against an oil field's {oil}; \
         the gap should be two orders of magnitude"
    );
}
