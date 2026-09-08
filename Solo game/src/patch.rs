//! **What was done to the ground, kept so it still means the same thing
//! tomorrow.**
//!
//! `ground.rs` generates and never stores; `Changes` records what
//! somebody altered. Writing that overlay to disk is *not* a code table
//! and an iterator, which is what this module exists to say.
//!
//! # An overlay means nothing without the base it was cut against
//!
//! Suppose a save records:
//!
//! ```text
//! remove the brick wall at (x, y, z)
//! ```
//!
//! and a later generator puts a road there. Applying the deletion no
//! longer means what it meant. **Keeping the generator's version number
//! does not fix this unless something actually reads it**, so every
//! modified chunk carries a `BaseChunk`: where it is, which generator
//! drew it, and a hash of what that generator produced. On load the base
//! is regenerated and hashed again, and a mismatch is *detected* rather
//! than silently misapplied.
//!
//! # Layers, because a tile is not one thing
//!
//! This project already holds that terrain, material and what is built
//! on them are three different questions, and that a glyph describes
//! what is seen rather than defining what exists. `ground::Tile` is the
//! **materialised view** — a union convenient for rendering. It is the
//! wrong thing to store: "wall" says nothing about whether the wall is
//! brick or the ground under it is rock, so an edit to one layer cannot
//! be recorded without overwriting the others.
//!
//! A stored change is therefore per layer, and each layer has **three**
//! states rather than two. `Remove` is not `Set(nothing)`: doors, walls,
//! floors, pipes and trees all exist in the generated base, and taking
//! one away has to be expressible as taking it away.
//!
//! # A boundary belongs to one tile
//!
//! A floor is the boundary between the level below and the level above.
//! Written from both sides it is stored twice and can disagree with
//! itself, so by convention it is **always the lower tile's ceiling**.

use std::collections::BTreeMap;

/// A chunk is one plot square, one level deep — the rung the town plan
/// already works in.
pub const CHUNK: i64 = 32;

/// **What a stored value can say.**
///
/// Three states, because two cannot express taking something away that
/// the generator put there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Field<T> {
    /// The generator's answer stands.
    #[default]
    Unchanged,
    Set(T),
    /// **There is deliberately nothing here**, whatever was generated.
    Remove,
}

impl<T: Copy> Field<T> {
    /// Apply this to whatever the generator said.
    pub fn over(self, base: Option<T>) -> Option<T> {
        match self {
            Field::Unchanged => base,
            Field::Set(v) => Some(v),
            Field::Remove => None,
        }
    }
    pub fn is_unchanged(self) -> bool {
        matches!(self, Field::Unchanged)
    }
}

/// What the ground *is*, before anything is built on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Terrain {
    Solid,
    Floor,
    Open,
    Ramp,
}

/// What it is made of. Separate from the terrain, so there is no
/// `GRANITE_WALL` and `LIMESTONE_WALL` to keep in step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Material {
    Soil,
    Sand,
    Sedimentary,
    Igneous,
    Metamorphic,
    Brick,
    Concrete,
    Timber,
    Steel,
    Glass,
    Tarmac,
}

/// What somebody put there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Construction {
    Wall,
    Door,
    Window,
    Partition,
    Fitting,
}

/// **The boundary above this tile.** Only ever the lower tile's, so one
/// physical floor is one stored fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Boundary {
    Solid,
    /// A hole through it — a stairwell, an atrium, a breach. Which is
    /// the whole reason a floor is a boundary rather than a property of
    /// a level.
    Open,
    Grate,
    Hatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fluid {
    Water,
    Sewage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Vegetation {
    Grass,
    Scrub,
    Crop,
    Tree,
}

/// **A stable handle for something that spans tiles.**
///
/// A stairwell, a run of pipe, a vehicle, a building. Inferring these
/// back from adjacency after a load is how two halves of one staircase
/// become two staircases; an id says they were always one thing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObjectId(pub u64);

/// One tile, changed in whichever layers were actually changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TileChange {
    pub terrain: Field<Terrain>,
    pub material: Field<Material>,
    pub construction: Field<Construction>,
    /// The boundary **above** this tile.
    pub ceiling: Field<Boundary>,
    pub fluid: Field<Fluid>,
    pub vegetation: Field<Vegetation>,
    pub object: Field<ObjectId>,
}

impl TileChange {
    /// Whether anything is actually recorded. A change with every layer
    /// unchanged is not a change and must not be stored, or a save grows
    /// with what was *looked at*.
    pub fn is_nothing(&self) -> bool {
        self.terrain.is_unchanged()
            && self.material.is_unchanged()
            && self.construction.is_unchanged()
            && self.ceiling.is_unchanged()
            && self.fluid.is_unchanged()
            && self.vegetation.is_unchanged()
            && self.object.is_unchanged()
    }
}

/// What a tile actually is, once the generator and the overlay are both
/// taken into account.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Materialised {
    pub terrain: Option<Terrain>,
    pub material: Option<Material>,
    pub construction: Option<Construction>,
    pub ceiling: Option<Boundary>,
    pub fluid: Option<Fluid>,
    pub vegetation: Option<Vegetation>,
    pub object: Option<ObjectId>,
}

impl Materialised {
    pub fn with(self, c: &TileChange) -> Self {
        Materialised {
            terrain: c.terrain.over(self.terrain),
            material: c.material.over(self.material),
            construction: c.construction.over(self.construction),
            ceiling: c.ceiling.over(self.ceiling),
            fluid: c.fluid.over(self.fluid),
            vegetation: c.vegetation.over(self.vegetation),
            object: c.object.over(self.object),
        }
    }
}

/// Where a chunk is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChunkAt {
    pub cx: i64,
    pub cy: i64,
    pub cz: i64,
}

impl ChunkAt {
    pub fn of(x: i64, y: i64, z: i64) -> Self {
        ChunkAt {
            cx: x.div_euclid(CHUNK),
            cy: y.div_euclid(CHUNK),
            cz: z,
        }
    }
}

/// **The base an overlay was cut against.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BaseChunk {
    pub at: ChunkAt,
    /// Which generator drew it. Recorded *and read*, which is the part
    /// that a bare version number never is.
    pub worldgen_version: u32,
    /// What that generator produced, hashed.
    pub base_hash: u64,
}

/// What to do when the ground underneath an overlay is not what it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rebase {
    /// The base is exactly what it was. Apply the overlay.
    Matches,
    /// A different generator drew it, and the ground has moved. **The
    /// overlay is not applied**, because "remove the wall" is a sentence
    /// about a wall that may no longer be there.
    BaseChanged { was: u64, now: u64 },
    /// The overlay was written by a generator this build does not have.
    UnknownGenerator(u32),
}

/// **Everything somebody has done to the world.**
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overlay {
    pub base: BTreeMap<ChunkAt, BaseChunk>,
    pub tiles: BTreeMap<(i64, i64, i64), TileChange>,
}

impl Overlay {
    pub fn new() -> Self {
        Overlay::default()
    }

    /// **Hash what the generator says**, so the base can be recognised
    /// again. The caller supplies the generator, because this module has
    /// no business knowing how the world is drawn.
    pub fn hash_base<F>(at: ChunkAt, generated: F) -> u64
    where
        F: Fn(i64, i64, i64) -> u64,
    {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for y in 0..CHUNK {
            for x in 0..CHUNK {
                let v = generated(at.cx * CHUNK + x, at.cy * CHUNK + y, at.cz);
                for b in v.to_le_bytes() {
                    h ^= b as u64;
                    h = h.wrapping_mul(0x1000_0000_01b3);
                }
            }
        }
        h
    }

    /// Record a change, taking the base identity of its chunk at the same
    /// time — because an overlay entry without one is a sentence with no
    /// subject.
    pub fn record<F>(
        &mut self,
        at: (i64, i64, i64),
        change: TileChange,
        worldgen_version: u32,
        generated: F,
    ) where
        F: Fn(i64, i64, i64) -> u64,
    {
        if change.is_nothing() {
            self.tiles.remove(&at);
            return;
        }
        let chunk = ChunkAt::of(at.0, at.1, at.2);
        self.base.entry(chunk).or_insert_with(|| BaseChunk {
            at: chunk,
            worldgen_version,
            base_hash: Overlay::hash_base(chunk, &generated),
        });
        self.tiles.insert(at, change);
    }

    /// **Is the ground still the ground this was cut against?**
    pub fn check<F>(&self, chunk: ChunkAt, worldgen_version: u32, generated: F) -> Rebase
    where
        F: Fn(i64, i64, i64) -> u64,
    {
        let Some(b) = self.base.get(&chunk) else {
            return Rebase::Matches;
        };
        if b.worldgen_version > worldgen_version {
            return Rebase::UnknownGenerator(b.worldgen_version);
        }
        let now = Overlay::hash_base(chunk, generated);
        if now == b.base_hash {
            Rebase::Matches
        } else {
            Rebase::BaseChanged {
                was: b.base_hash,
                now,
            }
        }
    }

    /// **Materialise one tile**: what the generator says, with the
    /// overlay applied once.
    pub fn materialise(&self, at: (i64, i64, i64), base: Materialised) -> Materialised {
        match self.tiles.get(&at) {
            Some(c) => base.with(c),
            None => base,
        }
    }

    /// How much of the world has been touched. **This, and not the size
    /// of the world, is what a save costs.**
    pub fn len(&self) -> usize {
        self.tiles.len()
    }
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// The canonical address of the boundary between `z` and `z + 1`.
    /// **Always the lower tile**, so one physical floor is one fact.
    pub fn boundary_between(x: i64, y: i64, lower_z: i64) -> (i64, i64, i64) {
        (x, y, lower_z)
    }
}
