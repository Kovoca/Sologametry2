//! The seam between the generated world and the economy.
//!
//! Everything up to `network` produces a planet; everything in `econ`
//! consumes a scenario. These assert that the first can actually feed the
//! second, on arbitrary nations of arbitrary planets rather than on one
//! hand-tuned case.

use scale_sim::econ::{Commodity, Doctrine, SiteKind};
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

const FOOD: Commodity = Commodity::ProcessedFood;

struct Planet {
    world: World,
    polities: Polities,
    settlements: Settlements,
    network: Network,
}

fn planet(seed: u64) -> Planet {
    // Smaller than the default so the suite stays quick; the pipeline is
    // resolution-independent.
    let world = World::generate(384, 216, seed);
    let polities = Polities::partition(&world, 24);
    let settlements = Settlements::place(&world, &polities, 3000);
    let network = Network::build(&world, &settlements, 500);
    Planet {
        world,
        polities,
        settlements,
        network,
    }
}

fn region_of(p: &Planet, rank: usize, doctrine: Doctrine) -> Option<Region> {
    let ranked = p.polities.ranked();
    let &(id, _) = ranked.get(rank)?;
    Region::extract(
        &p.world,
        &p.polities,
        &p.settlements,
        &p.network,
        id,
        5,
        doctrine,
    )
}

#[test]
fn any_nation_yields_a_working_economy() {
    // The point of the integration: not one hand-tuned region, but any
    // nation of any planet.
    for seed in [1u64, 42, 20260828] {
        let p = planet(seed);
        for rank in 0..6 {
            let Some(mut r) = region_of(&p, rank, Doctrine::Prudent) else {
                continue;
            };
            for _ in 0..90 {
                r.economy.step();
            }
            r.economy.ledger.assert_conserved();

            // Undisturbed, every market should be fed. A region that
            // starves on day one means its economy was derived wrong, not
            // that its geography is hard.
            for m in 0..r.economy.markets.len() {
                let cover = r.economy.markets[m].cover[FOOD as usize];
                assert!(
                    cover > 1.0,
                    "seed {seed} nation {rank}: {} has {cover:.1} days of food \
                     with nothing wrong",
                    r.economy.markets[m].name
                );
            }
        }
    }
}

#[test]
fn every_region_can_get_power_somehow() {
    // A nation with coal mines it; one without buys it. Neither simply
    // goes dark, but the second acquires a dependency that can be cut.
    for seed in [1u64, 42, 20260828] {
        let p = planet(seed);
        for rank in 0..5 {
            let Some(r) = region_of(&p, rank, Doctrine::Prudent) else {
                continue;
            };
            assert!(
                r.has_generation(),
                "seed {seed} nation {rank}: no power station at all"
            );
            let fuel = r
                .economy
                .ledger
                .sites
                .iter()
                .filter(|s| s.kind == SiteKind::Mine)
                .count();
            assert!(
                fuel > 0,
                "seed {seed} nation {rank}: a power station with nothing to burn"
            );
        }
    }
}

#[test]
fn freight_costs_come_from_real_distances() {
    // Routes are priced off the map, so a nation whose towns are far apart
    // must have dearer freight than one whose towns are close. If this
    // stops holding, the arbitrage loop has been cut loose from geography.
    let p = planet(20260828);
    let mut seen = 0;
    for rank in 0..8 {
        let Some(r) = region_of(&p, rank, Doctrine::Prudent) else {
            continue;
        };
        for route in &r.economy.routes {
            assert!(
                route.freight_cost > 0.0,
                "nation {rank}: {} costs nothing to haul along",
                route.name
            );
            // A road haul across a continent should not be pennies, and a
            // hop between neighbouring towns should not cost a fortune.
            assert!(
                route.freight_cost < 3000.0,
                "nation {rank}: {} costs {:.0}/t, which is not a road",
                route.name,
                route.freight_cost
            );
            seen += 1;
        }
    }
    assert!(seen > 0, "no routes were generated on any nation");
}

#[test]
fn populations_are_the_ones_the_world_generated() {
    // The economy must run on the world's numbers, not on numbers of its
    // own. If these drift apart, the two halves are only pretending to be
    // connected.
    let p = planet(20260828);
    let ranked = p.polities.ranked();
    let &(id, _) = ranked.first().expect("no nations");
    let r = Region::extract(
        &p.world,
        &p.polities,
        &p.settlements,
        &p.network,
        id,
        5,
        Doctrine::Prudent,
    )
    .expect("largest nation yielded no region");

    for (m, &s) in r.settlement_of_market.iter().enumerate() {
        assert_eq!(
            r.economy.markets[m].population,
            p.settlements.list[s].population as f64,
            "market {} does not match the settlement it came from",
            r.economy.markets[m].name
        );
    }
}

#[test]
fn the_farming_year_has_a_shape() {
    // Crops arrive in a burst and are eaten all year. If output is flat,
    // seasons are decorative.
    let p = planet(20260828);
    let Some(r) = region_of(&p, 0, Doctrine::Prudent) else {
        panic!("no region");
    };
    let mut peak: f64 = 0.0;
    let mut trough = f64::INFINITY;
    let mut seasons = std::collections::BTreeSet::new();
    let mut e = r.economy;
    for _ in 0..scale_sim::econ::DAYS_PER_YEAR {
        e.step();
        let h = e.harvest_today();
        peak = peak.max(h);
        trough = trough.min(h);
        seasons.insert(e.season().name());
    }
    assert_eq!(seasons.len(), 4, "the year did not pass through four seasons");
    assert!(
        peak > trough * 20.0,
        "harvest peaked at {peak:.2} against a trough of {trough:.2} — too flat to be a crop"
    );
}

#[test]
fn grain_is_dear_before_the_harvest_and_cheap_after() {
    // The seasonal signal a trader would learn. It lives in grain, not in
    // the loaf: shops and mills buffer the processed end, which is why
    // real bread prices barely move while grain prices do.
    let p = planet(20260828);
    let Some(r) = region_of(&p, 0, Doctrine::Prudent) else {
        panic!("no region");
    };
    let mut e = r.economy;
    let grain = Commodity::Grain;

    // Settle through a full year first, then watch the next one.
    for _ in 0..scale_sim::econ::DAYS_PER_YEAR {
        e.step();
    }
    let mut cheapest = f64::INFINITY;
    let mut dearest: f64 = 0.0;
    for _ in 0..scale_sim::econ::DAYS_PER_YEAR {
        e.step();
        let p = e.price(0, grain);
        cheapest = cheapest.min(p);
        dearest = dearest.max(p);
    }
    assert!(
        dearest > cheapest * 1.3,
        "grain ran {cheapest:.0} to {dearest:.0} over a year — no seasonal signal at all"
    );
    assert!(
        dearest < cheapest * 12.0,
        "grain ran {cheapest:.0} to {dearest:.0} — a swing no stored staple has"
    );
}

#[test]
fn seasons_do_not_starve_anyone() {
    // The counterpart. A harvest cycle is a rhythm, not a crisis: granaries
    // exist precisely so that eating is steady while growing is not.
    for seed in [1u64, 42, 20260828] {
        let p = planet(seed);
        let Some(r) = region_of(&p, 0, Doctrine::Prudent) else {
            continue;
        };
        let mut e = r.economy;
        for _ in 0..(scale_sim::econ::DAYS_PER_YEAR * 3) {
            e.step();
            assert!(
                e.unmet_demand[FOOD as usize] == 0.0,
                "seed {seed}: people went hungry on day {} with nothing wrong",
                e.ledger.day
            );
        }
        e.ledger.assert_conserved();
    }
}

#[test]
fn extraction_is_deterministic() {
    let p = planet(777);
    let a = region_of(&p, 0, Doctrine::Negligent).expect("no region");
    let b = region_of(&p, 0, Doctrine::Negligent).expect("no region");

    let names_a: Vec<&str> = a.economy.ledger.sites.iter().map(|s| s.name.as_str()).collect();
    let names_b: Vec<&str> = b.economy.ledger.sites.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names_a, names_b, "the same nation produced different works");

    let f_a: Vec<f64> = a.economy.routes.iter().map(|r| r.freight_cost).collect();
    let f_b: Vec<f64> = b.economy.routes.iter().map(|r| r.freight_cost).collect();
    assert_eq!(f_a, f_b, "freight costs differ between identical extractions");
}

#[test]
fn a_generated_region_cascades_like_the_hand_built_one() {
    // The whole reason for the slice was to prove the cascade. It has to
    // survive being fed real geography instead of hand-typed sites.
    let p = planet(20260828);
    let Some(mut r) = region_of(&p, 0, Doctrine::Negligent) else {
        panic!("largest nation yielded no region");
    };
    for _ in 0..30 {
        r.economy.step();
    }
    let before = r.economy.price(0, FOOD);

    r.economy.grid.fail_transformer("main line");
    for _ in 0..40 {
        r.economy.step();
    }

    let after = r.economy.price(0, FOOD);
    assert!(
        after > before * 2.0,
        "a whole nation lost its generation and food went {before:.0} -> {after:.0}"
    );
    assert!(
        r.economy.unmet_demand[FOOD as usize] > 0.0,
        "nobody went without despite the grid being down for weeks"
    );
    r.economy.ledger.assert_conserved();
}
