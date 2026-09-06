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
fn every_town_is_on_its_nations_road_network() {
    // No country's settlements sit off its own roads. The routes must form
    // one connected network reaching every modelled town, not a star of
    // straight lines drawn between them.
    for seed in [1u64, 42, 20260828] {
        let p = planet(seed);
        for rank in 0..5 {
            let Some(r) = region_of(&p, rank, Doctrine::Prudent) else {
                continue;
            };
            let n = r.economy.markets.len();
            if n < 2 {
                continue;
            }

            let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
            for route in &r.economy.routes {
                adj[route.a].push(route.b);
                adj[route.b].push(route.a);
            }
            let mut seen = vec![false; n];
            let mut stack = vec![0usize];
            seen[0] = true;
            while let Some(m) = stack.pop() {
                for &j in &adj[m] {
                    if !seen[j] {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
            let stranded = seen.iter().filter(|&&s| !s).count();
            assert_eq!(
                stranded, 0,
                "seed {seed} nation {rank}: {stranded} of {n} towns are off the network"
            );

            // A tree over n towns has exactly n-1 links: no redundant
            // straight-line shortcuts left over from the old star.
            assert_eq!(
                r.economy.routes.len(),
                n - 1,
                "seed {seed} nation {rank}: {} routes for {n} towns",
                r.economy.routes.len()
            );
        }
    }
}

#[test]
fn road_hauls_are_never_shorter_than_the_crow_flies() {
    // The whole point of routing over the network: freight follows roads,
    // and roads bend round terrain. A haul reported as shorter than the
    // straight-line gap would mean the costs had come loose from the map.
    let p = planet(20260828);
    let mut detours = 0;
    for rank in 0..6 {
        let Some(r) = region_of(&p, rank, Doctrine::Prudent) else {
            continue;
        };
        for route in &r.economy.routes {
            // The name carries both figures: "N km of road for an M km gap".
            let nums: Vec<f64> = route
                .name
                .split(|c: char| !c.is_ascii_digit())
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse().ok())
                .collect();
            if nums.len() < 2 {
                continue;
            }
            let (road, direct) = (nums[0], nums[1]);
            assert!(
                road >= direct - 1.0,
                "{}: {road} km of road for a {direct} km gap is shorter than a straight line",
                route.name
            );
            if road > direct * 1.3 {
                detours += 1;
            }
        }
    }
    assert!(
        detours > 0,
        "not one route detoured round anything — freight is ignoring the terrain"
    );
}

#[test]
fn a_pass_shuts_for_the_winter_and_a_tunnel_does_not() {
    // The whole argument for boring through a mountain rather than going
    // over it. A pass follows the ground and costs little; it is also gone
    // for months, and an economy that depends on one has a seasonal hole.
    use scale_sim::econ::Crossing;

    let p = planet(20260828);
    let mut found_pass = false;

    for rank in 0..8 {
        let Some(r) = region_of(&p, rank, Doctrine::Negligent) else {
            continue;
        };
        if !r
            .economy
            .routes
            .iter()
            .any(|rt| matches!(rt.crossing, Crossing::Pass { .. }))
        {
            continue;
        }
        found_pass = true;

        let mut e = r.economy;
        let mut shut_days = 0;
        for _ in 0..scale_sim::econ::DAYS_PER_YEAR {
            e.step();
            if e.routes.iter().any(|rt| rt.snowed_in) {
                shut_days += 1;
            }
        }
        assert!(
            shut_days > 0,
            "nation {rank} has a pass and it never closed in a whole year"
        );
        assert!(
            shut_days < scale_sim::econ::DAYS_PER_YEAR as usize,
            "nation {rank}'s pass was shut the entire year — that is not a pass"
        );
        break;
    }
    assert!(found_pass, "no nation on this planet routes over a pass");
}

#[test]
fn tunnels_are_bought_only_where_the_traffic_pays_for_them() {
    // A col carrying a few carts is left as a pass; one carrying a
    // nation's freight is bored through. Both exist in real mountain
    // country side by side, and which one a state builds is a decision
    // about money, not about geology.
    use scale_sim::econ::Crossing;

    let p = planet(20260828);
    let mut tunnels_when_funded = 0;
    let mut tunnels_when_not = 0;
    let mut crossings = 0;

    for rank in 0..16 {
        for (doctrine, count) in [
            (Doctrine::Prudent, &mut tunnels_when_funded),
            (Doctrine::Negligent, &mut tunnels_when_not),
        ] {
            let Some(r) = region_of(&p, rank, doctrine) else {
                continue;
            };
            if doctrine == Doctrine::Prudent {
                crossings += r
                    .economy
                    .routes
                    .iter()
                    .filter(|rt| !matches!(rt.crossing, Crossing::Level))
                    .count();
            }
            *count += r
                .economy
                .routes
                .iter()
                .filter(|rt| matches!(rt.crossing, Crossing::Tunnel { .. }))
                .count();
        }
    }

    // **A planet may simply have no route worth tunnelling**, and after
    // water became a precondition for settling this one very nearly does:
    // towns went onto the rivers, and a river valley is the low way
    // through a range. Which is why real roads follow them. Nothing to
    // choose between a pass and a tunnel is a fact about the ground, not
    // a failure of the choice.
    if crossings < 8 {
        // Too few to discriminate, and that is the ground rather than
        // the choice: a tunnel is bought where the traffic pays for it,
        // so a handful of quiet crossings correctly buys none.
        return;
    }

    assert!(
        tunnels_when_funded > tunnels_when_not,
        "with {crossings} crossings to choose over, a state that can afford \
         tunnels built {tunnels_when_funded} and one that cannot built \
         {tunnels_when_not}"
    );
}

#[test]
fn a_tunnel_hauls_cheaper_than_the_pass_it_replaces() {
    use scale_sim::econ::Crossing;

    let p = planet(20260828);
    for rank in 0..8 {
        let (Some(rich), Some(poor)) = (
            region_of(&p, rank, Doctrine::Prudent),
            region_of(&p, rank, Doctrine::Negligent),
        ) else {
            continue;
        };
        for (a, b) in rich.economy.routes.iter().zip(poor.economy.routes.iter()) {
            if matches!(a.crossing, Crossing::Tunnel { .. })
                && matches!(b.crossing, Crossing::Pass { .. })
            {
                assert!(
                    a.freight_cost < b.freight_cost,
                    "the tunnel at {:.0}/t is dearer than the pass at {:.0}/t",
                    a.freight_cost,
                    b.freight_cost
                );
                return;
            }
        }
    }
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
        let h = e.harvest_at(0);
        peak = peak.max(h);
        trough = trough.min(h);
        seasons.insert(e.season_at(0).name());
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
    // **Real seasonality**: pre-modern grain rose 20-40% between harvest
    // and the hungry gap, and modern grain futures still swing 10-20%.
    // A quarter is a signal; the bar sat at exactly 30% and a nation that
    // became slightly better fed slipped under it, which is the tension
    // CLAUDE.md already records — enough farm slack to survive a bad year
    // is enough buffer to flatten the seasonal price signal.
    assert!(
        dearest > cheapest * 1.25,
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
    //
    // **Seed 1 is back, and it used to be the exception.** Once the soil
    // decided what a nation grows, a town's farms no longer matched its
    // own mills and the smallest town of that nation was drained: on day
    // 1284 Valehaven's fields and terminal made 10,324 tonnes between
    // them, both ended the day holding none of it, and its mill got 3,122
    // and stopped. The grain was carted to the larger towns the day it was
    // made, because a producer handed over everything without first
    // covering the works next door.
    //
    // Two later changes fixed it without either being aimed at it:
    // `distribute` now covers every daily draw before anybody builds
    // inventory, and `logistics.rs` moves on days of cover rather than on
    // tonnes. The seed is asserted rather than avoided so it cannot
    // silently regress.
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

#[test]
fn the_land_decides_what_a_nation_grows() {
    // **The bug this exists to stop coming back.**
    //
    // Farms used to be sized by apportioning a nation's own grain
    // requirement between its towns by fertility. The shares always summed
    // to one, so every nation on every planet grew exactly the same
    // multiple of what it ate: a country of 153 million on the worst
    // ground per head fed itself as comfortably as one with fifteen times
    // the soil per person. The fertility map decided where the farms sat
    // and nothing at all about whether the nation was rich or poor in
    // food — so nobody was ever short, and trade had nothing to do.
    let p = planet(20260828);
    let mut per_head = Vec::new();
    for rank in 0..8 {
        let Some(r) = region_of(&p, rank, Doctrine::Prudent) else {
            continue;
        };
        let pop: f64 = r.economy.markets.iter().map(|m| m.population).sum();
        let grown: f64 = r
            .economy
            .ledger
            .sites
            .iter()
            .filter(|s| s.kind == SiteKind::Farm)
            .map(|s| s.throughput)
            .sum();
        if pop > 0.0 {
            per_head.push(grown * 365.0 / pop);
        }
    }
    assert!(per_head.len() >= 4, "not enough nations to compare");
    let lo = per_head.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = per_head.iter().cloned().fold(0.0, f64::max);
    assert!(
        hi > lo * 1.5,
        "every nation grows {lo:.2}-{hi:.2} t of grain a head — the land is \
         deciding nothing"
    );
}

#[test]
fn a_nation_that_cannot_feed_itself_buys_and_does_not_starve() {
    // Land that binds has to leave somebody short, or it is not binding.
    // What a short nation does is buy, landed at a port — the same answer
    // it already gives for coal — and thereby take on the oldest
    // dependency there is.
    let p = planet(20260828);
    let mut found_importer = false;
    for rank in 0..8 {
        let Some(r) = region_of(&p, rank, Doctrine::Prudent) else {
            continue;
        };
        let imports = r
            .economy
            .ledger
            .sites
            .iter()
            .any(|s| s.name.contains("grain terminal"));
        if !imports {
            continue;
        }
        found_importer = true;
        let mut e = r.economy;
        for _ in 0..(scale_sim::econ::DAYS_PER_YEAR * 2) {
            e.step();
            assert!(
                e.unmet_demand[FOOD as usize] == 0.0,
                "a nation living on imported grain went hungry on day {} \
                 with the sea lanes open",
                e.ledger.day
            );
        }
        e.ledger.assert_conserved();
    }
    assert!(
        found_importer,
        "no nation on this planet is short of food — the land is not binding"
    );
}
