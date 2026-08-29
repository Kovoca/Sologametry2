//! Regression guards for world generation.
//!
//! The prototype history is clear that retuning one field can silently wreck
//! the whole map (the classic failure: everything turns to desert). These
//! tests make that loud instead of silent.

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
}

#[test]
fn different_seeds_differ() {
    let a = World::generate(160, 90, 1);
    let b = World::generate(160, 90, 2);
    assert_ne!(a.biomes, b.biomes, "two seeds produced an identical map");
}
