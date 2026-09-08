//! **What was done to the ground, kept so it still means the same thing
//! tomorrow.**
//!
//! The gate is `the_identified_base_plus_the_overlay_is_the_same_world`:
//! regenerate exactly the base an overlay was cut against, apply it once,
//! and get the same complete local state.

use scale_sim::patch::{
    BaseChunk, Boundary, ChunkAt, Construction, Field, Fluid, Material, Materialised, ObjectId,
    Overlay, Rebase, Terrain, TileChange, Vegetation, CHUNK,
};
use scale_sim::save::{Journal, Reader, Save, SaveError, Store, Writer};

/// A stand-in generator. Deterministic, and cheap to change so the
/// "somebody rebuilt the world" case can actually be tested.
fn generator(salt: u64) -> impl Fn(i64, i64, i64) -> u64 {
    move |x, y, z| {
        let h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (y as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
            ^ (z as u64).wrapping_mul(0x94D0_49BB_1331_11EB)
            ^ salt;
        h ^ (h >> 29)
    }
}

/// What that generator would say a tile is.
fn base_tile(gen: &impl Fn(i64, i64, i64) -> u64, at: (i64, i64, i64)) -> Materialised {
    let v = gen(at.0, at.1, at.2);
    Materialised {
        terrain: Some(match v % 3 {
            0 => Terrain::Solid,
            1 => Terrain::Floor,
            _ => Terrain::Open,
        }),
        material: Some(match (v >> 3) % 3 {
            0 => Material::Soil,
            1 => Material::Sedimentary,
            _ => Material::Igneous,
        }),
        construction: if (v >> 7).is_multiple_of(5) {
            Some(Construction::Wall)
        } else {
            None
        },
        ceiling: if (v >> 11).is_multiple_of(4) {
            Some(Boundary::Solid)
        } else {
            None
        },
        fluid: None,
        vegetation: if (v >> 13).is_multiple_of(6) {
            Some(Vegetation::Tree)
        } else {
            None
        },
        object: None,
    }
}

// =====================================================================
// the gate
// =====================================================================

/// **Regenerate the identified base, apply the overlay once, and get the
/// same world.**
#[test]
fn the_identified_base_plus_the_overlay_is_the_same_world() {
    let gen = generator(1);
    let mut o = Overlay::new();

    // Somebody knocks a hole in a wall, floods a cellar, fells a tree
    // and puts a hatch in a floor.
    let edits: Vec<((i64, i64, i64), TileChange)> = vec![
        (
            (4, 5, 0),
            TileChange {
                construction: Field::Remove,
                ..Default::default()
            },
        ),
        (
            (6, 5, 0),
            TileChange {
                construction: Field::Set(Construction::Door),
                material: Field::Set(Material::Timber),
                ..Default::default()
            },
        ),
        (
            (7, 9, -1),
            TileChange {
                fluid: Field::Set(Fluid::Water),
                ..Default::default()
            },
        ),
        (
            (11, 2, 0),
            TileChange {
                vegetation: Field::Remove,
                ..Default::default()
            },
        ),
        (
            (3, 3, 0),
            TileChange {
                ceiling: Field::Set(Boundary::Hatch),
                ..Default::default()
            },
        ),
    ];
    for (at, c) in &edits {
        o.record(*at, *c, 1, &gen);
    }

    // What the world looks like now.
    let live: Vec<Materialised> = edits
        .iter()
        .map(|(at, _)| o.materialise(*at, base_tile(&gen, *at)))
        .collect();

    // Written down, read back, and applied to a freshly regenerated base.
    let bytes = Save {
        world_seed: 1,
        day: 0,
        people: vec![],
        journal: Journal::new(),
        overlay: o.clone(),
        ..Default::default()
    }
    .to_bytes();
    let back = Save::from_bytes(&bytes).unwrap().overlay;

    for (i, (at, _)) in edits.iter().enumerate() {
        let chunk = ChunkAt::of(at.0, at.1, at.2);
        assert_eq!(
            back.check(chunk, 1, &gen),
            Rebase::Matches,
            "the base moved"
        );
        assert_eq!(
            back.materialise(*at, base_tile(&gen, *at)),
            live[i],
            "the tile at {at:?} came back different"
        );
    }
    assert_eq!(back, o);
}

/// **Applying it twice is applying it once.** An overlay is a statement
/// about the world, not a sequence of operations on it.
#[test]
fn applying_an_overlay_twice_changes_nothing() {
    let gen = generator(2);
    let mut o = Overlay::new();
    let at = (1, 1, 0);
    o.record(
        at,
        TileChange {
            construction: Field::Remove,
            ..Default::default()
        },
        1,
        &gen,
    );
    let once = o.materialise(at, base_tile(&gen, at));
    let twice = o.materialise(at, once);
    assert_eq!(once, twice);
}

// =====================================================================
// the base is part of the meaning
// =====================================================================

/// **An overlay means nothing without the base it was cut against.**
///
/// "Remove the wall at (x, y, z)" is a sentence about a wall. If a newer
/// generator puts a road there, the sentence no longer means what it
/// meant — and a bare version number does not catch that, because the
/// version can be unchanged while the world is not.
#[test]
fn a_changed_base_is_detected_and_not_silently_applied() {
    let old = generator(1);
    let mut o = Overlay::new();
    let at = (4, 5, 0);
    o.record(
        at,
        TileChange {
            construction: Field::Remove,
            ..Default::default()
        },
        1,
        &old,
    );

    // The generator is rebuilt. Same version number, different world.
    let rebuilt = generator(999);
    let chunk = ChunkAt::of(at.0, at.1, at.2);
    match o.check(chunk, 1, &rebuilt) {
        Rebase::BaseChanged { was, now } => assert_ne!(was, now),
        other => panic!("the ground moved under an overlay and nothing noticed: {other:?}"),
    }
    // And the original still matches, so this is not simply always
    // failing.
    assert_eq!(o.check(chunk, 1, &old), Rebase::Matches);
}

/// **A save from a generator this build does not have** is refused
/// rather than misread.
#[test]
fn an_overlay_from_a_newer_generator_is_refused() {
    let gen = generator(3);
    let mut o = Overlay::new();
    let at = (2, 2, 0);
    o.record(
        at,
        TileChange {
            terrain: Field::Set(Terrain::Open),
            ..Default::default()
        },
        7,
        &gen,
    );
    let chunk = ChunkAt::of(at.0, at.1, at.2);
    assert_eq!(o.check(chunk, 3, &gen), Rebase::UnknownGenerator(7));
}

/// **Only touched chunks carry a base**, so the identity costs what was
/// changed rather than what was generated.
#[test]
fn identity_is_kept_for_what_was_changed_and_nothing_else() {
    let gen = generator(4);
    let mut o = Overlay::new();
    o.record(
        (0, 0, 0),
        TileChange {
            vegetation: Field::Remove,
            ..Default::default()
        },
        1,
        &gen,
    );
    o.record(
        (1, 0, 0),
        TileChange {
            vegetation: Field::Remove,
            ..Default::default()
        },
        1,
        &gen,
    );
    // Same chunk, so one identity.
    assert_eq!(o.base.len(), 1);
    o.record(
        (CHUNK + 1, 0, 0),
        TileChange {
            vegetation: Field::Remove,
            ..Default::default()
        },
        1,
        &gen,
    );
    assert_eq!(
        o.base.len(),
        2,
        "a change in another chunk brought no identity with it"
    );
    assert_eq!(o.len(), 3);
}

// =====================================================================
// layers, and the third state
// =====================================================================

/// **Remove is not `Set(nothing)`.** A door the generator put there has
/// to be removable, and a layer nobody touched has to stay the
/// generator's business.
#[test]
fn a_layer_has_three_states_and_needs_all_three() {
    let gen = generator(5);
    let at = (3, 7, 0);
    let base = Materialised {
        terrain: Some(Terrain::Floor),
        material: Some(Material::Brick),
        construction: Some(Construction::Door),
        ceiling: Some(Boundary::Solid),
        fluid: None,
        vegetation: None,
        object: None,
    };

    let mut o = Overlay::new();
    o.record(
        at,
        TileChange {
            construction: Field::Remove,
            ..Default::default()
        },
        1,
        &gen,
    );
    let after = o.materialise(at, base);
    assert_eq!(after.construction, None, "the door could not be taken away");
    assert_eq!(
        after.material,
        Some(Material::Brick),
        "removing a door changed the brickwork"
    );
    assert_eq!(after.terrain, Some(Terrain::Floor));
    assert_eq!(after.ceiling, Some(Boundary::Solid));
}

/// **Changing one layer does not overwrite the others**, which is the
/// whole reason a stored change is not a `Tile`.
#[test]
fn editing_one_layer_leaves_the_rest_to_the_generator() {
    let gen = generator(6);
    let at = (8, 8, 0);
    let base = base_tile(&gen, at);
    let mut o = Overlay::new();
    o.record(
        at,
        TileChange {
            fluid: Field::Set(Fluid::Sewage),
            ..Default::default()
        },
        1,
        &gen,
    );
    let after = o.materialise(at, base);
    assert_eq!(after.fluid, Some(Fluid::Sewage));
    assert_eq!(after.terrain, base.terrain);
    assert_eq!(after.material, base.material);
    assert_eq!(after.construction, base.construction);
    assert_eq!(after.vegetation, base.vegetation);
}

/// **A change with nothing in it is not stored**, or a save grows with
/// what was looked at rather than what was done.
#[test]
fn looking_at_the_world_does_not_write_to_it() {
    let gen = generator(7);
    let mut o = Overlay::new();
    for x in 0..200i64 {
        o.record((x, 0, 0), TileChange::default(), 1, &gen);
    }
    assert!(
        o.is_empty(),
        "two hundred untouched tiles were written down"
    );

    // And reverting a change removes it rather than recording the
    // reversion.
    o.record(
        (5, 0, 0),
        TileChange {
            vegetation: Field::Remove,
            ..Default::default()
        },
        1,
        &gen,
    );
    assert_eq!(o.len(), 1);
    o.record((5, 0, 0), TileChange::default(), 1, &gen);
    assert!(o.is_empty(), "putting it back left a change behind");
}

/// **A boundary belongs to one tile.** A floor between two levels is
/// stored as the lower tile's ceiling, once, so it cannot disagree with
/// itself.
#[test]
fn a_floor_between_two_levels_is_one_fact() {
    let gen = generator(8);
    let mut o = Overlay::new();
    // Knocking a hole in the floor of the first storey.
    let lower = Overlay::boundary_between(4, 4, 0);
    assert_eq!(lower, (4, 4, 0));
    o.record(
        lower,
        TileChange {
            ceiling: Field::Set(Boundary::Open),
            ..Default::default()
        },
        1,
        &gen,
    );

    // The level above records nothing about it — there is nowhere for it
    // to, which is what makes the boundary unambiguous.
    assert_eq!(o.len(), 1);
    assert!(!o.tiles.contains_key(&(4, 4, 1)));
    let after = o.materialise(lower, base_tile(&gen, lower));
    assert_eq!(after.ceiling, Some(Boundary::Open));
}

/// **A thing that spans tiles keeps its identity**, rather than being
/// inferred back from adjacency — which is how two halves of one
/// staircase become two staircases.
#[test]
fn something_spanning_tiles_is_one_thing_after_a_reload() {
    let gen = generator(9);
    let mut o = Overlay::new();
    let stair = ObjectId(77);
    for z in 0..4i64 {
        o.record(
            (2, 2, z),
            TileChange {
                terrain: Field::Set(Terrain::Open),
                ceiling: Field::Set(Boundary::Open),
                object: Field::Set(stair),
                ..Default::default()
            },
            1,
            &gen,
        );
    }
    let bytes = Save {
        world_seed: 1,
        day: 0,
        people: vec![],
        journal: Journal::new(),
        overlay: o.clone(),
        ..Default::default()
    }
    .to_bytes();
    let back = Save::from_bytes(&bytes).unwrap().overlay;
    for z in 0..4i64 {
        assert_eq!(
            back.materialise((2, 2, z), Materialised::default()).object,
            Some(stair),
            "level {z} of the stair came back as something else"
        );
    }
}

// =====================================================================
// the format
// =====================================================================

/// Every layer value survives the trip.
#[test]
fn every_layer_round_trips() {
    let all = TileChange {
        terrain: Field::Set(Terrain::Ramp),
        material: Field::Set(Material::Glass),
        construction: Field::Remove,
        ceiling: Field::Set(Boundary::Grate),
        fluid: Field::Set(Fluid::Sewage),
        vegetation: Field::Remove,
        object: Field::Set(ObjectId(9)),
    };
    let mut w = Writer::new();
    all.store(&mut w);
    assert_eq!(TileChange::load(&mut Reader::new(&w.bytes)).unwrap(), all);
}

/// **The same overlay writes the same bytes.**
#[test]
fn an_overlay_is_written_canonically() {
    let gen = generator(10);
    let build = |order: &[i64]| {
        let mut o = Overlay::new();
        for &x in order {
            o.record(
                (x, 0, 0),
                TileChange {
                    vegetation: Field::Remove,
                    ..Default::default()
                },
                1,
                &gen,
            );
        }
        let mut w = Writer::new();
        o.store(&mut w);
        w.bytes
    };
    assert_eq!(build(&[1, 2, 3]), build(&[3, 1, 2]));
}

/// **A file claiming one tile was changed twice is broken**, not newer.
#[test]
fn a_duplicated_tile_is_rejected() {
    let mut w = Writer::new();
    w.len(0); // no base chunks
    w.len(2); // two changes...
    for _ in 0..2 {
        w.i64(1);
        w.i64(1);
        w.i64(0);
        TileChange {
            vegetation: Field::Remove,
            ..Default::default()
        }
        .store(&mut w);
    }
    assert!(matches!(
        Overlay::load(&mut Reader::new(&w.bytes)),
        Err(SaveError::Conflict(_))
    ));
}

/// A base chunk identity survives with the version that made it.
#[test]
fn a_base_identity_round_trips() {
    let b = BaseChunk {
        at: ChunkAt {
            cx: -3,
            cy: 7,
            cz: -1,
        },
        worldgen_version: 4,
        base_hash: 0xDEAD_BEEF,
    };
    let mut w = Writer::new();
    b.store(&mut w);
    assert_eq!(BaseChunk::load(&mut Reader::new(&w.bytes)).unwrap(), b);
}

/// Negative coordinates — cellars and the ground west of the origin —
/// address the right chunk.
#[test]
fn chunks_are_addressed_correctly_below_and_left_of_nothing() {
    assert_eq!(
        ChunkAt::of(-1, -1, -1),
        ChunkAt {
            cx: -1,
            cy: -1,
            cz: -1
        }
    );
    assert_eq!(
        ChunkAt::of(0, 0, 0),
        ChunkAt {
            cx: 0,
            cy: 0,
            cz: 0
        }
    );
    assert_eq!(
        ChunkAt::of(CHUNK, CHUNK - 1, 2),
        ChunkAt {
            cx: 1,
            cy: 0,
            cz: 2
        }
    );
    assert_eq!(
        ChunkAt::of(-CHUNK, 0, 0),
        ChunkAt {
            cx: -1,
            cy: 0,
            cz: 0
        }
    );
}
