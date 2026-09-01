//! What a viewer can actually see, and what an enclosure hides.
//!
//! The renderer had no visibility stage at all: every tile was drawn
//! whether or not anything could see it, so a shop's shelving was legible
//! from the middle of the street and a closed box van showed its seats,
//! tanks and cargo to anybody standing beside it. The assembly was leaking
//! into the player's view.
//!
//! The contract, in order: **boundary opacity → line of sight → the
//! visible set → the renderer.** A wall that stops a ray is itself
//! visible; what is behind it is not.

use scale_sim::ground::{Ground, Tile};
use scale_sim::townplan::{Lot, Plan};
use scale_sim::vehicle::{Part, Vehicle};
use scale_sim::world::World;

fn a_town() -> (u64, Plan, (i64, i64)) {
    let seed = 4242u64;
    let world = World::generate(384, 216, seed);
    let pol = scale_sim::polity::Polities::partition(&world, 30);
    let set = scale_sim::settlement::Settlements::place(&world, &pol, 5000);
    let s = set
        .list
        .iter()
        .max_by_key(|s| s.population)
        .expect("a settlement");
    let plan = Plan::lay_out_on(seed, s.cell, s.population as f64, 40, world.biomes[s.cell])
        .on_rock(world.geology.rock[s.cell]);
    // A street tile with a building beside it.
    let mut spot = (0i64, 0i64);
    'find: for py in 1..plan.height - 1 {
        for px in 1..plan.width - 1 {
            if plan.at(px, py) == Lot::Street
                && matches!(plan.at(px + 1, py), Lot::Shop | Lot::Flats | Lot::House)
            {
                spot = (
                    px as i64 * 32 + 16,
                    py as i64 * 32 + 16,
                );
                break 'find;
            }
        }
    }
    (seed, plan, spot)
}

/// **A boundary is opaque and what is behind it is not drawn.**
#[test]
fn opaque_enclosures_hide_building_and_vehicle_interiors() {
    let (seed, plan, spot) = a_town();
    let g = Ground::window(seed, &plan, spot, 80, 40);

    // Park a lorry beside the viewer, closed up.
    let mut with_lorry = Ground::window(seed, &plan, spot, 80, 40);
    if let Some(at) = (0..with_lorry.h).find_map(|y| {
        (0..with_lorry.w).find_map(|x| {
            if with_lorry.at(x, y) == Tile::Road {
                Some((
                    with_lorry.origin.0 + x as i64,
                    with_lorry.origin.1 + y as i64,
                ))
            } else {
                None
            }
        })
    }) {
        with_lorry.park(&Vehicle::artic(), at);
    }

    let seen = g.visible_from(spot);

    // --- a wall that stops a ray is itself visible ---
    let mut walls_seen = 0;
    for y in 0..g.h {
        for x in 0..g.w {
            if g.at(x, y) == Tile::Wall && seen[y * g.w + x] {
                walls_seen += 1;
            }
        }
    }
    assert!(
        walls_seen > 0,
        "standing in a street beside a building and not one wall is visible"
    );

    // --- but what is behind one is not ---
    //
    // **Some of a shop is meant to be visible**: a counter through the
    // window is exactly the sightline the contract allows, and a shop
    // that cannot be seen into is not a shop. What must never be visible
    // is what a wall stands in front of.
    let mut fittings = 0;
    let mut fittings_seen = 0;
    for y in 0..g.h {
        for x in 0..g.w {
            if !matches!(g.at(x, y), Tile::Fitting(_) | Tile::Furnishing(_)) {
                continue;
            }
            fittings += 1;
            if seen[y * g.w + x] {
                fittings_seen += 1;
                // **The back of house is behind a partition**, so a
                // stockroom rack is never on view from the street however
                // many windows the frontage has.
                assert_ne!(
                    g.at(x, y),
                    Tile::Fitting(scale_sim::building::Fixture::StockRack),
                    "stockroom racking at {x},{y} is visible from the street"
                );
            }
        }
    }
    assert!(fittings > 0, "a town with nothing inside any of its buildings");
    assert!(
        (fittings_seen as f64) < fittings as f64 * 0.25,
        "{fittings_seen} of {fittings} fittings in the whole view are visible          from one spot in the street, which is a town made of glass"
    );

    // --- and a closed vehicle shows its hull, not its contents ---
    let view = with_lorry.render(Some(spot));
    for hidden in ['%', '!', 'B', 'F', 'b', 'a', 'x', 'w'] {
        assert!(
            !view.contains(hidden),
            "a closed lorry is showing its '{hidden}' from outside"
        );
    }

    // **Not a patch for lorries.** The rule is the enclosure's, so it is
    // asked of the part rather than of the vehicle.
    for p in [
        Part::Seat,
        Part::Controls,
        Part::CargoBay(1000),
        Part::Tank(500),
        Part::Battery(5),
        Part::Engine(300),
    ] {
        assert!(
            matches!(p.seen_from_outside(), Part::Frame { .. }),
            "{p:?} is on view from outside a closed hull"
        );
    }
    // The boundary and the things mounted through it stay themselves.
    assert!(matches!(
        Part::Wheel { heavy: true }.seen_from_outside(),
        Part::Wheel { .. }
    ));
    assert!(Part::Frame { heavy: true }.opaque());
    assert!(!Part::Wheel { heavy: false }.opaque());
}

/// **Glazing is seen and seen through**, which is what makes it glazing.
#[test]
fn a_window_is_visible_and_transparent() {
    assert!(!Tile::Window.opaque());
    assert!(Tile::Wall.opaque());
    // A door is shut until somebody opens it. Guessing the permissive
    // state would show the inside of every building in the town.
    assert!(Tile::Door.opaque());
    // Ground you walk on never blocks the view along it.
    for t in [Tile::Road, Tile::Pavement, Tile::Floor, Tile::Grass, Tile::Sand] {
        assert!(!t.opaque(), "{t:?} should not stop a line of sight");
    }
}

/// The inspection view exists and is honest about what it is.
#[test]
fn omniscience_is_available_and_clearly_separate() {
    let (seed, plan, spot) = a_town();
    let g = Ground::window(seed, &plan, spot, 60, 30);
    let seen = g.render(Some(spot));
    let all = g.render_omniscient(Some(spot), false);
    assert_ne!(
        seen, all,
        "the player's view and the inspection view are identical, so one of \
         them is wrong"
    );
    // Omniscience draws strictly more.
    let blanks = |s: &str| s.chars().filter(|c| *c == ' ').count();
    assert!(
        blanks(&seen) > blanks(&all),
        "the visible view should have more unseen ground than the omniscient one"
    );
}

/// **A step in the ground is not a hole in the world.**
///
/// A window was one horizontal slice of the world, which is only ever
/// right indoors. Outdoors the ground moves: on the low side of a step the
/// slice sat above it and came back `Sky`, on the high side it was buried
/// and came back solid earth. Standing on a street in hill country, most
/// of what you could see was therefore either blank or walled off — not
/// hidden, *absent* — which is not what happens when you stand on a kerb.
///
/// Three things have to hold together, and each of them was broken:
///
/// - the eye's own level finds the **surface**, up or down;
/// - a boundary only stops a ray **at your own level**, because a wall
///   down in the cutting is not between you and anything;
/// - line of sight is measured in **metres, not levels**. The 3 m level is
///   how the world is drawn; quantising the viewshed to it turned a road
///   climbing at 5% into a flight of three-metre walls, each of which hid
///   everything past it.
#[test]
fn a_step_in_the_ground_is_not_a_hole_in_the_world() {
    let (seed, plan, spot) = a_town();
    let g = Ground::window(seed, &plan, spot, 92, 72);

    // Nothing generated at the level somebody is standing on is open air:
    // where the ground falls away you see the ground, lower down.
    let sky = g.tiles.iter().filter(|t| **t == Tile::Sky).count();
    assert_eq!(
        sky, 0,
        "{sky} tiles of open air at ground level: the view has holes in it"
    );

    // And the ground genuinely moves here, or the test proves nothing —
    // the whole point is a window that spans a step.
    let (lo, hi) = g
        .surf
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), s| (l.min(*s), h.max(*s)));
    assert!(
        hi - lo > 1.0,
        "the ground moves only {:.2} m across 92 m, so this proves nothing \
         about steps in it",
        hi - lo
    );

    // **You can see down the street you are standing in.** Made ground is
    // open ground: a road, its markings and its footways are the one place
    // in a town with nothing in the way, so nearly all of what the window
    // holds of them should be in view. Before the viewshed ran on metres a
    // single levelled plot boundary blacked out the whole road beyond it.
    let seen = g.visible_from(spot);
    let road = |t: Tile| {
        matches!(
            t,
            Tile::Road | Tile::Marking | Tile::Pavement | Tile::Shoulder
        )
    };
    let (mut open, mut in_view) = (0, 0);
    for i in 0..seen.len() {
        if road(g.tiles[i]) && g.rel[i] == 0 {
            open += 1;
            if seen[i] {
                in_view += 1;
            }
        }
    }
    assert!(open > 200, "only {open} tiles of street to look along");
    let share = in_view as f64 / open as f64;
    assert!(
        share > 0.8,
        "only {:.0}% of the street around an open corner is visible \
         ({in_view} tiles of {open})",
        share * 100.0
    );

    // **But something still blocks**, or the viewshed is doing nothing at
    // all — the buildings along it are opaque and stay so.
    let all = seen.iter().filter(|v| **v).count() as f64 / seen.len() as f64;
    assert!(all < 0.95, "everything is visible, so nothing is occluded");
}

/// **A level asked for by name is that level.** Surface-seeking is for the
/// level somebody is standing on; a caller who asks for the sewer under a
/// street wants the sewer, not the street found again one level up.
#[test]
fn asking_for_a_level_by_name_gives_that_level() {
    let (seed, plan, spot) = a_town();
    let cellar = Ground::window_on(seed, &plan, spot, 32, 32, -1);
    assert!(
        cellar.rel.iter().all(|d| *d == 0),
        "an explicitly requested level went looking for the surface"
    );
    assert!(
        cellar.tiles.iter().filter(|t| **t == Tile::Earth).count() > 100,
        "the ground under a street is not solid"
    );
}
