//! The middle rung of the scale ladder.
//!
//! One region cell, seen close enough to walk into. Spec A1.3d: interpolate
//! the coarse fields, add higher-frequency variation, store nothing.

use scale_sim::locality::{Locality, LOCALITIES_PER_CELL, METRES_PER_LOCALITY};
use scale_sim::region::KM_PER_CELL;
use scale_sim::world::World;

fn a_world() -> World {
    World::generate(256, 144, 20260828)
}

#[test]
fn the_ladder_multiplies_out() {
    use scale_sim::townplan::{METRES_PER_PLOT, TILES_PER_PLOT};
    let plots_per_locality = METRES_PER_LOCALITY / METRES_PER_PLOT;
    assert_eq!(
        plots_per_locality as usize * TILES_PER_PLOT * LOCALITIES_PER_CELL,
        16_384,
        "16 localities x {plots_per_locality} plots x {TILES_PER_PLOT} tiles          should be 16,384 m"
    );

    // 16 localities x 32 plots x 32 tiles = 16,384 m, which is why the
    // region cell is 16.384 km in the first place. If these drift apart
    // the scales stop nesting and nothing above or below can be trusted.
    let from_cell = KM_PER_CELL * 1000.0;
    let from_localities = LOCALITIES_PER_CELL as f64 * METRES_PER_LOCALITY;
    assert!(
        (from_cell - from_localities).abs() < 1.0,
        "a region cell is {from_cell:.0} m one way and {from_localities:.0} the other"
    );
}

#[test]
fn zooming_in_shows_the_country_it_came_from() {
    // **The coarse pass must not be re-rolled.** Its rain shadows and
    // ranges are computed globally and a 16 km grid is ample to model
    // them; zooming in refines and never overrules. Re-classifying from
    // absolute thresholds turned a mountain cell into flat desert, which
    // is the same trap CLAUDE.md already records for biomes: rank over
    // land, never cut on raw values.
    let w = a_world();
    let mut checked = 0;
    for cell in (0..w.biomes.len()).step_by(311) {
        if w.elevation.data[cell] < w.sea_level {
            continue;
        }
        let loc = Locality::zoom(&w, cell);
        let parent = w.biomes[cell];
        let same = loc
            .patches
            .iter()
            .filter(|p| !p.river && p.biome == parent)
            .count();
        assert!(
            same * 2 > loc.patches.len(),
            "a {parent:?} cell zoomed to {}/{} patches of its own country",
            same,
            loc.patches.len()
        );
        checked += 1;
    }
    assert!(checked > 20, "only {checked} land cells sampled");
}

#[test]
fn a_river_is_a_channel_not_a_puddle() {
    // Marking every low patch put water over a tenth of the cell in a
    // scatter. A river comes in from one neighbour and leaves towards
    // another, and it is connected the whole way — that is most of what
    // makes it a river rather than a marsh.
    let w = a_world();
    let mut found = 0;
    for cell in 0..w.biomes.len() {
        if !w.river[cell] || w.elevation.data[cell] < w.sea_level {
            continue;
        }
        let loc = Locality::zoom(&w, cell);
        let wet = loc.patches.iter().filter(|p| p.river).count();
        assert!(
            wet > 0 && wet < loc.patches.len() / 3,
            "a river cell came out {wet} patches of {} — that is a lake",
            loc.patches.len()
        );
        found += 1;
        if found > 40 {
            break;
        }
    }
    assert!(found > 10, "only {found} river cells to check");
}

#[test]
fn the_same_ground_every_time() {
    // Spec A1.5: generated, never stored. Walk away and come back and it
    // has to be the valley you left.
    let w = a_world();
    let cell = w.biomes.len() / 3;
    assert_eq!(
        Locality::zoom(&w, cell).render(),
        Locality::zoom(&w, cell).render()
    );
}
