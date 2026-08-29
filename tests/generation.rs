//! Regression guards for world generation.
//!
//! The prototype history is clear that retuning one field can silently wreck
//! the whole map (the classic failure: everything turns to desert). These
//! tests make that loud instead of silent.

use scale_sim::geology::Rock;
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
fn every_rock_type_appears() {
    // A world made entirely of one rock type means the classifier's cuts
    // have drifted — the failure mode that made everything sedimentary.
    for seed in [1u64, 42, 20260828, 7, 3] {
        let w = World::generate(256, 144, seed);
        let land: Vec<usize> = (0..w.biomes.len())
            .filter(|&i| w.elevation.data[i] >= w.sea_level)
            .collect();
        let land_n = land.len() as f32;

        for rock in Rock::ALL {
            let share =
                land.iter().filter(|&&i| w.geology.rock[i] == rock).count() as f32 / land_n;
            assert!(
                share > 0.02,
                "seed {seed}: {} is only {:.1}% of land",
                rock.name(),
                share * 100.0
            );
            assert!(
                share < 0.90,
                "seed {seed}: {} swallowed {:.0}% of land",
                rock.name(),
                share * 100.0
            );
        }
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
