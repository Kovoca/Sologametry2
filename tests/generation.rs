//! Regression guards for world generation.
//!
//! The prototype history is clear that retuning one field can silently wreck
//! the whole map (the classic failure: everything turns to desert). These
//! tests make that loud instead of silent.

use scale_sim::geology::Rock;
use scale_sim::network::{Network, Road};
use scale_sim::polity::{Polities, UNCLAIMED};
use scale_sim::settlement::{Kind, Settlements};
use scale_sim::world::{Biome, World};

const OCEAN: usize = Biome::Ocean as usize;
const SHALLOWS: usize = Biome::Shallows as usize;

#[test]
fn land_fraction_stays_near_target() {
    for seed in [1u64, 42, 20260828, 999_999, 7] {
        let world = World::generate(192, 108, seed);
        let land = world.land_fraction();
        assert!(
            (0.28..=0.40).contains(&land),
            "seed {seed}: land fraction {land:.3} drifted outside 0.28..=0.40"
        );
    }
}

#[test]
fn no_single_biome_dominates_land() {
    for seed in [1u64, 42, 20260828, 999_999, 7] {
        let world = World::generate(192, 108, seed);
        let counts = world.biome_counts();

        let land_total: usize = counts
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != OCEAN && *i != SHALLOWS)
            .map(|(_, &n)| n)
            .sum();
        assert!(land_total > 0, "seed {seed}: no land at all");

        for (i, &n) in counts.iter().enumerate() {
            if i == OCEAN || i == SHALLOWS {
                continue;
            }
            let share = n as f32 / land_total as f32;
            assert!(
                share < 0.55,
                "seed {seed}: {} is {:.0}% of all land",
                Biome::ALL[i].name(),
                share * 100.0
            );
        }
    }
}

#[test]
fn some_variety_exists() {
    // A believable planet should show a decent spread of biomes, not two.
    let world = World::generate(192, 108, 20260828);
    let counts = world.biome_counts();
    let present = counts.iter().filter(|&&n| n > 0).count();
    assert!(present >= 7, "only {present} biomes present, expected >= 7");
}

#[test]
fn generation_is_deterministic() {
    let a = World::generate(160, 90, 12345);
    let b = World::generate(160, 90, 12345);
    assert_eq!(a.biomes, b.biomes, "same seed produced different biomes");
    assert_eq!(a.sea_level, b.sea_level, "same seed produced different sea level");
    assert_eq!(a.river, b.river, "same seed produced different rivers");
    assert_eq!(a.lake, b.lake, "same seed produced different lakes");
    assert_eq!(a.geology.rock, b.geology.rock, "same seed produced different rock");
    assert_eq!(
        a.geology.fertility.data, b.geology.fertility.data,
        "same seed produced different fertility"
    );
    assert_eq!(
        a.geology.ore.data, b.geology.ore.data,
        "same seed produced different ore"
    );
}

#[test]
fn sedimentary_rock_covers_most_of_the_land() {
    // **Real: sediment covers about 73% of the continental surface** — it
    // is only ~8% of the crust by volume, but it blankets everything;
    // igneous and metamorphic are exposed in shields, mountain cores and
    // volcanic provinces. The cuts once gave 12% sedimentary and 73%
    // metamorphic, which is the real world exactly inverted, and it
    // stopped being cosmetic the moment the tile layer began asking which
    // rock keeps a cliff face: nearly every hillside came out as crags.
    //
    // Worlds are *allowed* to differ — that is why the cuts are fixed and
    // not percentiles — so this asserts the shape, not a ratio: sediment
    // dominates everywhere, and each crystalline kind is somewhere.
    let mut seen = [false; 3];
    for seed in [1u64, 42, 20260828, 7, 3] {
        let w = World::generate(256, 144, seed);
        let land: Vec<usize> = (0..w.biomes.len())
            .filter(|&i| w.elevation.data[i] >= w.sea_level)
            .collect();
        let land_n = land.len() as f32;
        let share = |rock: Rock| {
            land.iter().filter(|&&i| w.geology.rock[i] == rock).count() as f32 / land_n
        };
        let sed = share(Rock::Sedimentary);
        assert!(
            (0.50..0.97).contains(&sed),
            "seed {seed}: sedimentary is {:.0}% of land, against ~73% on Earth",
            sed * 100.0
        );
        for (k, rock) in Rock::ALL.iter().enumerate() {
            seen[k] |= share(*rock) > 0.02;
        }
    }
    for (k, rock) in Rock::ALL.iter().enumerate() {
        assert!(seen[k], "{} appears on no world at all", rock.name());
    }
}

#[test]
fn deposits_are_localised() {
    // Deposits should be a small share of the map. If a resource covers
    // most of the land it has stopped being a deposit and become terrain.
    for seed in [1u64, 42, 20260828, 7, 3] {
        let w = World::generate(256, 144, seed);
        let land: Vec<usize> = (0..w.biomes.len())
            .filter(|&i| w.elevation.data[i] >= w.sea_level)
            .collect();
        let land_n = land.len() as f32;

        for (name, f) in [
            ("ore", &w.geology.ore),
            ("coal", &w.geology.coal),
            ("petroleum", &w.geology.petroleum),
        ] {
            let workable =
                land.iter().filter(|&&i| f.data[i] >= 0.45).count() as f32 / land_n;
            assert!(
                workable < 0.35,
                "seed {seed}: workable {name} covers {:.0}% of land",
                workable * 100.0
            );
        }

        // ...but the world must not be barren of everything either.
        let any = land
            .iter()
            .filter(|&&i| {
                w.geology.ore.data[i] >= 0.45
                    || w.geology.coal.data[i] >= 0.45
                    || w.geology.petroleum.data[i] >= 0.45
            })
            .count();
        assert!(any > 0, "seed {seed}: no workable deposits anywhere");
    }
}

#[test]
fn polities_claim_most_land_and_leave_a_frontier() {
    for seed in [1u64, 42, 20260828, 7] {
        let w = World::generate(256, 144, seed);
        let p = Polities::partition(&w, 24);

        let land = (0..w.biomes.len())
            .filter(|&i| w.elevation.data[i] >= w.sea_level)
            .count() as f32;
        let claimed = p.owner.iter().filter(|&&o| o != UNCLAIMED).count() as f32;
        let share = claimed / land;

        assert!(
            (0.55..=0.995).contains(&share),
            "seed {seed}: {:.0}% of land claimed — want most settled but some frontier left",
            share * 100.0
        );
    }
}

#[test]
fn polities_never_claim_water() {
    let w = World::generate(256, 144, 20260828);
    let p = Polities::partition(&w, 24);
    for i in 0..w.biomes.len() {
        if matches!(w.biomes[i], Biome::Ocean | Biome::Shallows) {
            assert_eq!(p.owner[i], UNCLAIMED, "cell {i} is water but is owned");
        }
    }
}

#[test]
fn nation_count_tracks_the_request() {
    // The count emerges from terrain, so it need not match exactly — but
    // asking for more must reliably produce more, or the knob is a lie.
    let w = World::generate(256, 144, 20260828);
    let few = Polities::partition(&w, 4).ranked().len();
    let many = Polities::partition(&w, 40).ranked().len();
    assert!(few <= 6, "asked for 4 nations, got {few}");
    assert!(many > few * 3, "asked for 40 vs 4, got {many} vs {few}");
}

#[test]
fn fewer_nations_means_more_concentration() {
    // The whole point of the knob: few polities is a world of great powers,
    // many is a world of peers.
    let w = World::generate(256, 144, 20260828);
    let concentrated = Polities::partition(&w, 4).concentration(3);
    let fragmented = Polities::partition(&w, 40).concentration(3);
    assert!(
        concentrated > fragmented + 0.25,
        "top-3 share barely moved: {concentrated:.2} at 4 nations vs {fragmented:.2} at 40"
    );
}

#[test]
fn state_sizes_are_skewed_not_uniform() {
    // "A few big nations and many smaller ones" is the target shape. If
    // every state comes out the same size, the founding-advantage model
    // has stopped doing anything.
    for seed in [1u64, 42, 20260828, 7] {
        let w = World::generate(256, 144, seed);
        let p = Polities::partition(&w, 30);
        let ranked = p.ranked();
        assert!(ranked.len() >= 10, "seed {seed}: only {} states", ranked.len());

        let largest = ranked[0].1.cells as f32;
        let median = ranked[ranked.len() / 2].1.cells as f32;
        // A uniform partition puts this ratio near 1.5; the skew should
        // push it well clear of that.
        assert!(
            largest > median * 2.4,
            "seed {seed}: largest {largest:.0} vs median {median:.0} — sizes too uniform"
        );

        // ...but no single state should swallow the planet.
        assert!(
            p.concentration(1) < 0.45,
            "seed {seed}: largest holds {:.0}% of claimed land",
            p.concentration(1) * 100.0
        );
    }
}

#[test]
fn every_polity_gets_exactly_one_capital() {
    let w = World::generate(256, 144, 20260828);
    let p = Polities::partition(&w, 24);
    let s = Settlements::place(&w, &p, 2000);

    for (id, _) in p.ranked() {
        let capitals = s
            .list
            .iter()
            .filter(|t| t.polity == id && t.kind == Kind::Capital)
            .count();
        assert_eq!(capitals, 1, "polity {id} has {capitals} capitals");
    }
}

#[test]
fn settlements_stand_on_owned_land() {
    let w = World::generate(256, 144, 20260828);
    let p = Polities::partition(&w, 24);
    let s = Settlements::place(&w, &p, 2000);

    for t in &s.list {
        assert!(
            w.elevation.data[t.cell] >= w.sea_level,
            "settlement at {} is under water",
            t.cell
        );
        assert_ne!(
            p.owner[t.cell], UNCLAIMED,
            "settlement at {} stands on unclaimed land",
            t.cell
        );
        assert!(
            !matches!(w.biomes[t.cell], Biome::Ocean | Biome::Shallows | Biome::Snowcap),
            "settlement at {} is on {:?}",
            t.cell,
            w.biomes[t.cell]
        );
    }
}

#[test]
fn city_sizes_span_orders_of_magnitude() {
    // Real settlement systems run from a handful of megacities down to
    // thousands of small towns. If the largest is only a few times the
    // median, the hinterland model has stopped biting.
    for seed in [1u64, 42, 20260828, 7] {
        let w = World::generate(256, 144, seed);
        let p = Polities::partition(&w, 24);
        let s = Settlements::place(&w, &p, 3000);
        let ranked = s.ranked();
        assert!(ranked.len() > 500, "seed {seed}: only {} placed", ranked.len());

        let largest = ranked[0].population as f64;
        let median = (ranked[ranked.len() / 2].population as f64).max(1.0);
        let ratio = largest / median;
        assert!(
            (15.0..600.0).contains(&ratio),
            "seed {seed}: largest/median is {ratio:.0}x (Earth is about 150x)"
        );
    }
}

#[test]
fn urban_population_is_conserved() {
    // Every person must land in exactly one settlement — the total is
    // shared out, never invented.
    let w = World::generate(256, 144, 20260828);
    let p = Polities::partition(&w, 24);
    let s = Settlements::place(&w, &p, 2000);

    let total = s.total_population() as f64;
    let expected = 8.0e9 * 0.57;
    let drift = (total - expected).abs() / expected;
    assert!(
        drift < 0.01,
        "urban population {total:.0} drifted {:.1}% from {expected:.0}",
        drift * 100.0
    );
}

#[test]
fn settlement_placement_is_deterministic() {
    let w = World::generate(192, 108, 555);
    let p = Polities::partition(&w, 20);
    let a = Settlements::place(&w, &p, 1500);
    let b = Settlements::place(&w, &p, 1500);

    let cells_a: Vec<usize> = a.list.iter().map(|s| s.cell).collect();
    let cells_b: Vec<usize> = b.list.iter().map(|s| s.cell).collect();
    assert_eq!(cells_a, cells_b, "same world placed settlements differently");

    let pop_a: Vec<u32> = a.list.iter().map(|s| s.population).collect();
    let pop_b: Vec<u32> = b.list.iter().map(|s| s.population).collect();
    assert_eq!(pop_a, pop_b, "same world produced different populations");
}

#[test]
fn roads_stay_on_land_and_form_a_hierarchy() {
    for seed in [1u64, 42, 20260828, 7] {
        let w = World::generate(256, 144, seed);
        let p = Polities::partition(&w, 24);
        let s = Settlements::place(&w, &p, 2500);
        let net = Network::build(&w, &s, 500);

        for i in 0..w.biomes.len() {
            if net.road[i] != Road::None {
                assert!(
                    w.elevation.data[i] >= w.sea_level,
                    "seed {seed}: road at {i} runs over water"
                );
            }
        }

        let highway = net.count(Road::Highway);
        assert!(highway > 0, "seed {seed}: no trunk routes at all");

        // **Class must follow the real traffic thresholds, not this map's
        // own percentiles.**
        //
        // Ranking a world's own stretches and cutting at fixed percentiles
        // hands every planet the same proportions of highway and track
        // however rich or empty it is, which is exactly the mistake the
        // geology pass had to unlearn. What is actually true is that
        // paving pays at a few hundred vehicles a day and dualling at
        // about thirteen thousand, and a country either clears those bars
        // or does not.
        //
        // So the invariant is that the rule was applied, and that class
        // never falls as traffic rises.
        const TRIPS: f64 = 0.02;
        for i in 0..net.road.len() {
            let vpd = net.traffic[i] * TRIPS;
            match net.road[i] {
                Road::Highway => assert!(
                    vpd >= 13_000.0,
                    "seed {seed}: motorway standard on {vpd:.0} vehicles a day"
                ),
                Road::Road => assert!(
                    (300.0..13_000.0).contains(&vpd),
                    "seed {seed}: paved single carriageway on {vpd:.0} vehicles a day"
                ),
                Road::Track => assert!(
                    vpd < 300.0,
                    "seed {seed}: left unpaved under {vpd:.0} vehicles a day"
                ),
                Road::None => assert!(net.traffic[i] <= 0.0),
            }
        }

        // Note for whoever reads a road map and wonders where the lanes
        // are: this world has no villages. Its two-thousandth settlement
        // still holds a quarter of a million people, so every link between
        // any two of them earns its pavement honestly. Tracks will appear
        // when the settlement pass grows a tail of hamlets, and not
        // before — which is a gap in `settlement.rs`, not in this rule.

        let paved = (highway + net.count(Road::Road) + net.count(Road::Track)) as f32;
        let land = (w.land_fraction() * w.biomes.len() as f32).max(1.0);
        let share = paved / land;
        assert!(
            (0.01..0.35).contains(&share),
            "seed {seed}: roads cover {:.0}% of land",
            share * 100.0
        );
    }
}

#[test]
fn navigable_water_reaches_the_sea() {
    // A big river draining into a closed basin is not a trade route. Every
    // navigable stretch must connect to the ocean through other navigable
    // cells.
    let w = World::generate(256, 144, 20260828);
    let p = Polities::partition(&w, 24);
    let s = Settlements::place(&w, &p, 2500);
    let net = Network::build(&w, &s, 500);

    let (width, height) = (w.width, w.height);
    let mut seen = vec![false; width * height];
    let mut stack: Vec<usize> = Vec::new();

    // Start from navigable cells touching the sea.
    for i in 0..width * height {
        if !net.navigable[i] {
            continue;
        }
        let (x, y) = (i % width, i / width);
        let mut at_sea = false;
        for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
            let ny = y as i32 + dy;
            if ny < 0 || ny >= height as i32 {
                continue;
            }
            let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
            if matches!(w.biomes[ny as usize * width + nx], Biome::Ocean | Biome::Shallows) {
                at_sea = true;
            }
        }
        if at_sea {
            seen[i] = true;
            stack.push(i);
        }
    }
    while let Some(i) = stack.pop() {
        let (x, y) = (i % width, i / width);
        for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
            let ny = y as i32 + dy;
            if ny < 0 || ny >= height as i32 {
                continue;
            }
            let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
            let j = ny as usize * width + nx;
            if net.navigable[j] && !seen[j] {
                seen[j] = true;
                stack.push(j);
            }
        }
    }

    for i in 0..width * height {
        assert!(
            !net.navigable[i] || seen[i],
            "navigable cell {i} is cut off from the sea"
        );
    }
}

#[test]
fn network_is_deterministic() {
    let w = World::generate(192, 108, 555);
    let p = Polities::partition(&w, 20);
    let s = Settlements::place(&w, &p, 1500);
    let a = Network::build(&w, &s, 400);
    let b = Network::build(&w, &s, 400);
    assert_eq!(a.road, b.road, "same world produced different roads");
    assert_eq!(a.navigable, b.navigable, "same world produced different waterways");
    assert_eq!(a.chokepoints, b.chokepoints, "chokepoints differ between runs");
}

#[test]
fn polity_partition_is_deterministic() {
    let w = World::generate(192, 108, 555);
    let a = Polities::partition(&w, 20);
    let b = Polities::partition(&w, 20);
    assert_eq!(a.owner, b.owner, "same world produced different borders");
}

#[test]
fn fertile_land_exists_and_is_not_everywhere() {
    for seed in [1u64, 42, 20260828, 7, 3] {
        let w = World::generate(256, 144, seed);
        let land: Vec<usize> = (0..w.biomes.len())
            .filter(|&i| w.elevation.data[i] >= w.sea_level)
            .collect();
        let land_n = land.len() as f32;

        let prime =
            land.iter().filter(|&&i| w.geology.fertility.data[i] > 0.55).count() as f32 / land_n;
        assert!(
            (0.005..0.60).contains(&prime),
            "seed {seed}: prime farmland is {:.1}% of land",
            prime * 100.0
        );
    }
}

#[test]
fn hydrology_produces_a_river_network() {
    for seed in [1u64, 42, 20260828, 7] {
        let w = World::generate(256, 144, seed);
        let land = (w.land_fraction() * w.biomes.len() as f32).max(1.0);

        let rivers = w.river_count();
        assert!(rivers > 0, "seed {seed}: no rivers at all");

        // Fresh water should be a minor share of the land, not a flood.
        let fresh = (rivers + w.lake_count()) as f32 / land;
        assert!(
            fresh < 0.20,
            "seed {seed}: rivers+lakes are {:.0}% of land",
            fresh * 100.0
        );
    }
}

#[test]
fn rivers_flow_downhill() {
    // Every river cell must have at least one neighbour that is lower or
    // equal (somewhere for the water to go). A river cell that is a strict
    // local maximum is a routing bug.
    let w = World::generate(256, 144, 20260828);
    let (width, height, e) = (w.width, w.height, &w.elevation.data);
    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            if !w.river[i] {
                continue;
            }
            let mut has_outlet = false;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let ny = y as i32 + dy;
                    if ny < 0 || ny >= height as i32 {
                        continue;
                    }
                    let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                    if e[ny as usize * width + nx] <= e[i] + 1e-4 {
                        has_outlet = true;
                    }
                }
            }
            assert!(has_outlet, "seed 20260828: river cell ({x},{y}) is a local peak");
        }
    }
}

#[test]
fn different_seeds_differ() {
    let a = World::generate(160, 90, 1);
    let b = World::generate(160, 90, 2);
    assert_ne!(a.biomes, b.biomes, "two seeds produced an identical map");
}

#[test]
fn east_west_edges_are_seamless() {
    // The planet is a cylinder (implementation-spec A1.3): the elevation
    // field must wrap in X with no visible seam. Measure the discontinuity
    // between the first and last columns against the typical discontinuity
    // between interior adjacent columns — it should be comparable, not a
    // cliff.
    for seed in [1u64, 42, 20260828, 7] {
        let w = World::generate(256, 144, seed);
        let (width, height) = (w.width, w.height);
        let e = &w.elevation.data;

        let mut seam = 0.0f32;
        let mut interior = 0.0f32;
        for y in 0..height {
            seam += (e[y * width] - e[y * width + (width - 1)]).abs();
            let mid = width / 2;
            interior += (e[y * width + mid] - e[y * width + mid - 1]).abs();
        }
        seam /= height as f32;
        interior /= height as f32;

        assert!(
            seam < interior * 3.0,
            "seed {seed}: east-west seam discontinuity {seam:.4} is a cliff \
             vs typical adjacent-column {interior:.4}"
        );
    }
}

#[test]
fn the_water_table_is_a_subdued_replica_of_the_ground() {
    // **The classic hydrogeological result**, and the whole model:
    // groundwater follows the surface with less relief, standing high
    // under hills and falling toward valleys. Where it meets the surface
    // you get a spring, a marsh or a perennial river — which is *why*
    // those are where they are.
    //
    // Real depths: nil at a watercourse, 1-5 m on a floodplain, 10-50 on a
    // hillside, over 100 in arid uplands; the deepest anywhere ~300 m.
    // A hand-dug well reaches 10-30 m.
    for seed in [1u64, 20260828, 7] {
        let w = World::generate(256, 144, seed);
        let land: Vec<usize> = (0..w.biomes.len())
            .filter(|&i| w.elevation.data[i] >= w.sea_level)
            .collect();

        let mut deepest = 0.0f64;
        for &i in &land {
            let d = w.depth_to_water_m(i);
            // Water never stands above the ground: that would be a lake,
            // and the hydrology pass has already decided about those.
            assert!(d >= 0.0, "seed {seed}: water above the ground");
            deepest = deepest.max(d);
        }
        // 300 m is the limit the generator holds to, and real deepest
        // water tables — the Sahara, the Australian outback, the High
        // Plains — are of that order.
        assert!(
            deepest <= 300.5,
            "seed {seed}: water {deepest:.0} m down — deeper than anywhere on Earth"
        );
        assert!(
            deepest > 40.0,
            "seed {seed}: nowhere on the planet is the water more than {deepest:.0} m down"
        );

        // **A watercourse is the table showing through.** Cells carrying
        // one are far shallower than the land in general.
        let mean = |v: &[usize]| {
            v.iter().map(|&i| w.depth_to_water_m(i)).sum::<f64>() / v.len().max(1) as f64
        };
        let wet: Vec<usize> = land.iter().copied().filter(|&i| w.river[i] || w.lake[i]).collect();
        let dry: Vec<usize> = land.iter().copied().filter(|&i| !w.river[i] && !w.lake[i]).collect();
        if !wet.is_empty() {
            assert!(
                mean(&wet) < mean(&dry),
                "seed {seed}: water is no shallower beside a river ({:.0} m) than away from one ({:.0} m)",
                mean(&wet),
                mean(&dry)
            );
        }

        // **Rain holds the table up.** The wettest quarter of the land
        // carries its water nearer the surface than the driest.
        let mut by_rain: Vec<usize> = dry.clone();
        by_rain.sort_by(|&a, &b| {
            w.rainfall.data[a]
                .total_cmp(&w.rainfall.data[b])
                .then(a.cmp(&b))
        });
        let q = by_rain.len() / 4;
        if q > 10 {
            let driest = mean(&by_rain[..q]);
            let wettest = mean(&by_rain[by_rain.len() - q..]);
            assert!(
                wettest < driest,
                "seed {seed}: the wettest ground holds its water {wettest:.0} m down \
                 against {driest:.0} m in the driest — rain is not recharging anything"
            );
        }
    }
}

#[test]
fn productivity_comes_from_the_climate_and_feeds_what_lives_on_it() {
    // **The Miami model** *(Lieth, 1975)* — a published model rather than
    // a curve picked to look right. Growth is limited by whichever of heat
    // and water is scarcer, which is why a hot desert and a wet tundra are
    // both unproductive for opposite reasons.
    use scale_sim::biota::miami_npp;
    // Real reference points, checked against the model directly.
    assert!(miami_npp(25.0, 2500.0) > 1800.0, "the wet tropics are not productive");
    assert!(miami_npp(25.0, 50.0) < 150.0, "a hot desert grows something");
    assert!(miami_npp(-10.0, 400.0) < 250.0, "tundra is not productive");

    for seed in [1u64, 20260828] {
        let w = World::generate(256, 144, seed);
        let land: Vec<usize> = (0..w.biomes.len())
            .filter(|&i| w.elevation.data[i] >= w.sea_level)
            .collect();
        let mean = |f: &dyn Fn(usize) -> f64| {
            land.iter().map(|&i| f(i)).sum::<f64>() / land.len() as f64
        };

        // **Two real anchors**, which is what the climate fields are
        // calibrated on: Earth's land mean annual temperature is ~8.5 °C
        // and its land mean annual precipitation ~715 mm. The rainfall
        // field was never on a 0..1 scale — its land mean is about 0.064
        // and it never reaches 0.4 — so reading it as one put the whole
        // planet in a drought at 230 mm and dragged productivity with it.
        let t = mean(&|i| w.temperature_c(i) as f64);
        let r = mean(&|i| w.rainfall_mm(i) as f64);
        assert!((-6.0..20.0).contains(&t), "seed {seed}: land mean {t:.1} °C");
        assert!((350.0..1400.0).contains(&r), "seed {seed}: land mean {r:.0} mm of rain");

        // The global land mean of net primary productivity is about
        // 700 g/m²/yr.
        let npp = mean(&|i| w.biota.npp.data[i] as f64);
        assert!(
            (400.0..1100.0).contains(&npp),
            "seed {seed}: land mean productivity {npp:.0} g/m²/yr against a real ~700"
        );

        // **A rainforest is the most productive land there is and carries
        // less game than a savanna half as productive**, because forest
        // production is locked up in wood forty metres overhead. That is
        // the whole reason grazing is not a multiple of productivity.
        let by_biome = |b: scale_sim::world::Biome, f: &dyn Fn(usize) -> f64| {
            let c: Vec<usize> = land.iter().copied().filter(|&i| w.biomes[i] == b).collect();
            if c.is_empty() {
                return None;
            }
            Some(c.iter().map(|&i| f(i)).sum::<f64>() / c.len() as f64)
        };
        use scale_sim::world::Biome::*;
        let npp_of = |b| by_biome(b, &|i| w.biota.npp.data[i] as f64);
        let game_of = |b| by_biome(b, &|i| w.biota.game.data[i] as f64);
        if let (Some(rf_npp), Some(gr_npp), Some(rf_game), Some(gr_game)) =
            (npp_of(Rainforest), npp_of(Grassland), game_of(Rainforest), game_of(Grassland))
        {
            assert!(rf_npp > gr_npp, "seed {seed}: grassland out-grows rainforest");
            assert!(
                gr_game > rf_game,
                "seed {seed}: rainforest carries more game ({rf_game:.0} kg/km²) than \
                 grassland ({gr_game:.0}) — the wood is being counted as fodder"
            );
        }

        // **Timber is accumulated, not annual.** A boreal forest grows
        // slowly and stands for centuries, so it carries 100-200 m³/ha on
        // a fraction of the tropics' productivity; scaling stock straight
        // off productivity gave taiga 42, which is scrub.
        if let Some(taiga) = by_biome(Taiga, &|i| w.biota.timber.data[i] as f64) {
            assert!(
                (60.0..250.0).contains(&taiga),
                "seed {seed}: taiga carries {taiga:.0} m³/ha of standing timber"
            );
        }

        // Two trophic steps is a hundredfold loss, so predators are rare
        // everywhere and not merely where it is cold.
        let g = mean(&|i| w.biota.game.data[i] as f64);
        let p = mean(&|i| w.biota.predators.data[i] as f64);
        assert!(p < g * 0.05 && p > 0.0, "seed {seed}: predators at {p:.0} against {g:.0} of prey");
    }
}
