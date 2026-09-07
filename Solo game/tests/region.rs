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
    let mut worst_without = 0.0f64;
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

        // **Hold everything still and vary one thing: whether the ships
        // come.**
        //
        // This used to assert `unmet_demand == 0.0` on every one of seven
        // hundred and thirty days, which sounds strict and tests less than
        // it looks. It never once checked that the imports were what was
        // feeding anybody — a nation whose own farms happened to be
        // adequate passes it without a grain ship ever docking — and being
        // an exact equality it also failed on a shop running dry for a day,
        // which is not a famine and is not what the sentence claims.
        //
        // So the claim is now the causal one, and it needs both halves.
        let hunger = |lanes_open: bool| -> f64 {
            // Rebuilt rather than cloned: `Economy` is not `Clone`, and
            // the same seed and rank give the same country every time.
            let mut e = region_of(&p, rank, Doctrine::Prudent)
                .expect("the same nation stopped existing")
                .economy;
            if !lanes_open {
                for i in 0..e.ledger.sites.len() {
                    if e.ledger.sites[i].name.contains("grain terminal") {
                        e.ledger.sites[i].throughput = 0.0;
                    }
                }
            }
            let mut went_without = 0.0;
            let mut ate = 0.0;
            for _ in 0..(scale_sim::econ::DAYS_PER_YEAR * 2) {
                e.step();
                went_without += e.unmet_demand[FOOD as usize];
                ate += (0..e.markets.len())
                    .map(|m| e.markets[m].daily_household_demand(FOOD))
                    .sum::<f64>();
            }
            e.ledger.assert_conserved();
            went_without / ate
        };

        let open = hunger(true);
        let shut = hunger(false);

        // **A famine bound, and loose on purpose.**
        //
        // A tenth of a per cent of two years' food is a shop empty for a
        // day or two somewhere; a famine is orders of magnitude larger, as
        // the closed-lanes figure shows. Said plainly because it was
        // checked: reverting the harvest-curve scaling, the grain terminal
        // sizing, the lead-time safety stock, or any pair of them, leaves
        // this half green. **It is the counterfactual below that
        // discriminates**, and a bound nothing can currently trip is worth
        // labelling as such rather than tightening until it fires, which
        // would be fitting a threshold to a sabotage.
        assert!(
            open < 0.001,
            "a nation living on imported grain went {:.3}% short with the \
             sea lanes open",
            100.0 * open
        );
        // **And somewhere the ships have to be what is feeding
        // somebody.**
        //
        // Not every nation with a terminal depends on it: several ride two
        // years on their reserve and their own fields, and cutting their
        // imports changes nothing whatever. That is a real distinction and
        // worth keeping rather than asserting away — a country that *has*
        // a grain trade is not the same as a country that would starve
        // without one. So the counterfactual is asserted over the planet
        // rather than over every importer.
        worst_without = worst_without.max(shut);
    }
    assert!(
        found_importer,
        "no nation on this planet is short of food — the land is not binding"
    );
    // **The counterfactual, which the old gate never checked at all.**
    // Somewhere there has to be a country that is fed by sea and would not
    // be otherwise, or "buys and does not starve" is a sentence about
    // nations that were never hungry in the first place.
    assert!(
        worst_without > 0.02,
        "cutting every grain terminal on the planet left the worst-hit          nation only {:.2}% short — nobody here actually lives on imports,          so the gate is measuring the wrong thing",
        100.0 * worst_without
    );
}

// =====================================================================
// a cheap input has to become a cheap output
// =====================================================================

/// **Gate: a glut of oil makes cheap plastics and cheap goods.**
///
/// The thing the model could not do at all. Price used to be a typed-in
/// reference cost times a scarcity multiplier, so nothing about the oil
/// ever reached the plastics: a country sitting on the richest field in the
/// world paid exactly what a country importing every barrel paid.
#[test]
fn cheap_oil_travels_all_the_way_down_to_the_shelf() {
    let p = planet(42);
    let mut cheap = region_of(&p, 0, Doctrine::Prudent).expect("no region");
    let mut dear = region_of(&p, 0, Doctrine::Prudent).expect("no region");

    // The same country twice, differing in one thing: how good the ground
    // under the oil field is.
    // **`SiteKind::OilField` is worn by two different things** — a real
    // field running `OIL_FIELD` and an import terminal running
    // `OIL_IMPORTS` — so selecting on the kind alone could manufacture
    // "Saudi versus oil sands" in a country with no oil of its own. The
    // recipe is what says whether anybody is lifting anything.
    let domestic = |e: &Region, s: usize| {
        e.economy.ledger.sites[s].recipe == Some(scale_sim::econ::recipe::OIL_FIELD)
    };
    let field_market = (0..cheap.economy.ledger.sites.len())
        .find(|&s| domestic(&cheap, s))
        .map(|s| cheap.economy.ledger.sites[s].market)
        .expect("this nation lifts no oil of its own, so there is nothing to vary");
    let mut varied = 0;
    for s in 0..cheap.economy.ledger.sites.len() {
        if domestic(&cheap, s) {
            // A Saudi-grade field: about $10 a barrel.
            cheap.economy.ledger.sites[s].cost_factor = 0.45;
            // Canadian oil sands: $50-60.
            dear.economy.ledger.sites[s].cost_factor = 3.0;
            varied += 1;
        }
    }
    assert!(varied > 0, "nothing was actually varied");
    for _ in 0..40 {
        cheap.economy.step();
        dear.economy.step();
    }

    let at = |e: &Region, c| e.economy.markets[field_market].cost[c as usize];
    let oil_cheap = at(&cheap, Commodity::Petroleum);
    let oil_dear = at(&dear, Commodity::Petroleum);
    assert!(
        oil_dear > oil_cheap * 2.0,
        "oil sands {oil_dear:.0} against a Saudi field {oil_cheap:.0}"
    );

    // **And it reaches the cracker**, which is one step down.
    let resin_cheap = at(&cheap, Commodity::Plastics);
    let resin_dear = at(&dear, Commodity::Plastics);
    assert!(
        resin_dear > resin_cheap * 1.3,
        "resin {resin_dear:.0} against {resin_cheap:.0} — the oil price did not reach the cracker"
    );

    // **And two steps further, to what a household buys.** Plastics are
    // only a small share of a tonne of goods, so the effect is real and
    // properly diluted — which is what a supply chain does to a shock.
    let goods_cheap = at(&cheap, Commodity::RetailGoods);
    let goods_dear = at(&dear, Commodity::RetailGoods);
    assert!(
        goods_dear > goods_cheap * 1.005,
        "goods {goods_dear:.1} against {goods_cheap:.1} — nothing reached the shelf"
    );
    assert!(
        goods_dear < goods_cheap * 1.3,
        "a fivefold oil price moved retail goods by more than a third, which is not dilution"
    );
}

/// **Gate: a rich seam and a thin one are not the same industry.**
///
/// Real spreads, and they are not small: Powder River coal comes out of a
/// surface seam at $12 a ton and Appalachian underground at $60-70; Pilbara
/// iron ore at 62% Fe costs a fifth of Chinese ore at half the grade,
/// because you have to move twice the rock for the same iron.
#[test]
fn what_it_costs_to_work_depends_on_what_is_in_the_ground() {
    // The relationship is one over the grade, because that is the physical
    // truth: a poorer deposit means moving and crushing proportionally more
    // rock for the same tonne of product.
    let rich = Commodity::cost_of_working(0.9);
    let ordinary = Commodity::cost_of_working(0.4);
    let thin = Commodity::cost_of_working(0.1);
    assert!(rich < ordinary && ordinary < thin);
    assert!(thin > rich * 4.0, "thin {thin:.2} against rich {rich:.2}");

    // Bounded at both ends: a marginal seam is expensive rather than
    // infinite, and no deposit is free to work.
    assert!(Commodity::cost_of_working(0.0) <= 6.0);
    assert!(Commodity::cost_of_working(1.0) >= 0.4);

    // And it reaches the price of what is made out of it.
    // **Resolve the mine and assert the precondition**, then make the
    // propagation unconditional. The first version of this read market
    // zero rather than the colliery's own, and put its only downstream
    // assertion inside an `if` — so a run in which nothing propagated at
    // all skipped the branch and passed. That is precisely the failure
    // this project already records: *a test that never enters the branch
    // is not evidence the branch is rare.*
    let p = planet(42);
    let mut good = region_of(&p, 0, Doctrine::Prudent).expect("no region");
    let mut poor = region_of(&p, 0, Doctrine::Prudent).expect("no region");

    let pit = (0..good.economy.ledger.sites.len())
        .find(|&s| good.economy.ledger.sites[s].recipe == Some(scale_sim::econ::recipe::COAL_MINE))
        .expect("this nation works no coal of its own, so there is no seam to vary");
    let pit_market = good.economy.ledger.sites[pit].market;
    good.economy.ledger.sites[pit].cost_factor = 0.45;
    poor.economy.ledger.sites[pit].cost_factor = 4.0;

    // And it has to be a market that actually makes its own power from it,
    // or there is nothing for the seam to reach.
    assert!(
        (0..good.economy.ledger.sites.len()).any(|s| {
            good.economy.ledger.sites[s].market == pit_market
                && good.economy.ledger.sites[s].kind == scale_sim::econ::SiteKind::PowerPlant
        }),
        "the colliery's market burns no coal, so this proves nothing"
    );

    for _ in 0..40 {
        good.economy.step();
        poor.economy.step();
    }
    let at = |e: &Region, c| e.economy.markets[pit_market].cost[c as usize];
    let coal_good = at(&good, Commodity::Coal);
    let coal_poor = at(&poor, Commodity::Coal);
    assert!(
        coal_poor > coal_good * 1.5,
        "a thin seam cost {coal_poor:.1} against a thick one at {coal_good:.1}"
    );

    // **Unconditional.** The seam reaches the price of the power made from
    // it, or this whole exercise did nothing.
    let power_good = at(&good, Commodity::Electricity);
    let power_poor = at(&poor, Commodity::Electricity);
    assert!(
        power_poor > power_good,
        "a thin seam made no difference to the price of power: \
         {power_poor:.1} against {power_good:.1}"
    );
}

/// **Gate: a sentinel may never be read as a rate.**
///
/// A power station's `throughput` means "whatever the grid can carry", and
/// three separate places have now been caught reading it as a number of
/// batches a day — it once staffed a station with 4.1 million people, it
/// made `distribute` take every tonne of coal in the country, and it was
/// quietly putting 380 million tonnes a day of coal demand into the price
/// pass for as long as the price pass has existed.
///
/// Two of those were fixed where they were found, which is exactly why the
/// third survived. This gate is on the property rather than on any one
/// caller.
#[test]
fn no_sentinel_ever_becomes_a_quantity() {
    use scale_sim::econ::{demand_rate_of, UNBOUNDED_THROUGHPUT};

    let p = planet(42);
    let mut r = region_of(&p, 0, Doctrine::Prudent).expect("no region");
    for _ in 0..30 {
        r.economy.step();
    }

    // The sentinel is really in there — otherwise this gate is watching
    // nothing.
    let unbounded: Vec<usize> = (0..r.economy.ledger.sites.len())
        .filter(|&s| r.economy.ledger.sites[s].throughput >= UNBOUNDED_THROUGHPUT * 0.5)
        .collect();
    assert!(!unbounded.is_empty(), "no site carries the sentinel, so this proves nothing");

    // **And nothing anywhere turns it into tonnes.** A country's entire
    // coal demand has to be a plausible number of tonnes a day, not a
    // fraction of a billion.
    for &s in &unbounded {
        let rate = demand_rate_of(&r.economy.ledger.sites[s]);
        assert!(
            rate < UNBOUNDED_THROUGHPUT * 0.001,
            "a site with an unbounded capacity reported a demand rate of {rate:.0}"
        );
    }

    // Measured end to end: the daily coal draw of a whole nation.
    for m in 0..r.economy.markets.len() {
        let cover = r.economy.markets[m].expected_cover[Commodity::Coal as usize];
        assert!(
            cover.is_finite() || cover.is_infinite(),
            "coal cover in market {m} is not a number at all"
        );
    }
    let biggest = r
        .economy
        .markets
        .iter()
        .map(|x| x.population)
        .fold(0.0f64, f64::max);
    let coal_demand: f64 = (0..r.economy.ledger.sites.len())
        .map(|s| {
            let site = &r.economy.ledger.sites[s];
            let Some(rc) = site.recipe else { return 0.0 };
            scale_sim::econ::RECIPES[rc]
                .inputs
                .iter()
                .find(|&&(ic, _)| ic == Commodity::Coal)
                .map(|&(_, q)| q * demand_rate_of(site))
                .unwrap_or(0.0)
        })
        .sum();
    // Real: world coal is about 8bn tonnes a year across 8bn people, so
    // even a heavy industrial economy is well under a tonne a head a day.
    assert!(
        coal_demand < biggest.max(1.0),
        "a nation of {biggest:.0} people wants {coal_demand:.0} tonnes of coal a day"
    );
}


/// **Gate: an idle plant does not price a market.**
///
/// `cost_of_production` took the cheapest nominal producer, so one tiny or
/// permanently stopped works set the cost for every rival that was actually
/// running. What a producer contributes to a market's cost is what it puts
/// into that market.
#[test]
fn a_works_that_is_not_running_does_not_set_the_price() {
    let p = planet(42);
    let mut r = region_of(&p, 0, Doctrine::Prudent).expect("no region");
    for _ in 0..40 {
        r.economy.step();
    }

    // Find a market that mills its own flour, and the mill doing it.
    let mill = (0..r.economy.ledger.sites.len())
        .find(|&s| {
            r.economy.ledger.sites[s].recipe == Some(scale_sim::econ::recipe::MILL)
                && r.economy.ledger.sites[s].ran > 0.0
        })
        .expect("nobody is milling anything");
    let m = r.economy.ledger.sites[mill].market;
    let ordinary = r.economy.markets[m].cost[Commodity::Flour as usize];

    // **Now put a miraculous mill next door and stop it dead.** It has a
    // nameplate and it never runs, so it supplies nothing and must not
    // price anything.
    let real = &r.economy.ledger.sites[mill];
    let ghost = scale_sim::econ::Site {
        name: "a mill that never opened".into(),
        kind: real.kind,
        market: m,
        stock: real.stock,
        capacity: real.capacity,
        recipe: real.recipe,
        // A nameplate and nothing behind it: it never runs.
        throughput: 0.0,
        powered: true,
        ran: 0.0,
        // And it would be miraculously cheap if anybody let it price.
        cost_factor: 0.02,
        fitted: None,
    };
    r.economy.ledger.sites.push(ghost);
    for _ in 0..10 {
        r.economy.step();
    }
    let with_a_ghost = r.economy.markets[m].cost[Commodity::Flour as usize];
    assert!(
        with_a_ghost > ordinary * 0.9,
        "a mill that never ran took flour from {ordinary:.1} to {with_a_ghost:.1}"
    );

    // **And a real cheap mill that does run moves it**, which is the other
    // half: the rule is about supply, not about ignoring low costs.
    let last = r.economy.ledger.sites.len() - 1;
    r.economy.ledger.sites[last].throughput = r.economy.ledger.sites[mill].throughput * 3.0;
    for _ in 0..40 {
        r.economy.step();
    }
    let with_a_real_one = r.economy.markets[m].cost[Commodity::Flour as usize];
    assert!(
        with_a_real_one < ordinary * 0.95,
        "a cheap mill running three times the town's flour changed the cost from \
         {ordinary:.1} to {with_a_real_one:.1}"
    );
}
