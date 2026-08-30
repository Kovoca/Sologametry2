//! The bottom of the ladder: ground at a metre to the tile, where a
//! person stands.
//!
//! Everything built above this has been furniture for a place nobody could
//! be in. A shop had tills and shelving but no floor to put them on; a
//! lorry was seventeen metres of parts with no road under it; a man had
//! money, hunger and a trade but no position finer than which market he
//! was in. This is the floor.
//!
//! **Generated, never stored** (spec A1.5, rule R5). A patch of ground is
//! a pure function of the world seed, the region cell, the town's plan and
//! the tile coordinates. Walk away and it is discarded; come back and it
//! regenerates identically. Only things that actually happen to it — a
//! fire, a wreck, a grave, a sale — would ever be written down, and that
//! is what keeps a save bounded.
//!
//! **Only near the person** (spec A1.6). A hundred and sixty tiles on foot,
//! two hundred and eighty-eight in a vehicle, because a fixed hundred-tile
//! bubble is under a hundred metres and far too short for a road ambush to
//! be legible.

use crate::building::{Building, Fixture};
use crate::townplan::{Lot, Plan, TILES_PER_PLOT};
use crate::vehicle::{Part, Vehicle};
use crate::world::Biome;

/// How far the world is real around somebody *(spec A1.6)*.
pub const BUBBLE_ON_FOOT: usize = 160;
/// Further in a vehicle, because you cover ground faster and need to see
/// the ambush before you are in it.
pub const BUBBLE_IN_VEHICLE: usize = 288;

/// One square metre of the world.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Tile {
    // --- open country ---
    Grass,
    Scrub,
    Sand,
    Rock,
    Snow,
    Water,
    Tree,

    // --- made ground ---
    Road,
    Pavement,

    // --- built ---
    Wall,
    Floor,
    Door,
    Window,

    /// A piece of a building's fitting-out, standing where it stands.
    Fitting(Fixture),
    /// A piece of a vehicle. **You can walk onto this** — that is the
    /// point of it being a tile rather than a mode.
    Vehicle(Part),
}

impl Tile {
    pub fn glyph(self) -> char {
        match self {
            Tile::Grass => '"',
            Tile::Scrub => '*',
            Tile::Sand => ',',
            Tile::Rock => '^',
            Tile::Snow => 'A',
            Tile::Water => '~',
            Tile::Tree => 'T',
            Tile::Road => '=',
            Tile::Pavement => '-',
            Tile::Wall => '#',
            Tile::Floor => '.',
            Tile::Door => '/',
            Tile::Window => 'o',
            Tile::Fitting(f) => match f {
                Fixture::Till => '$',
                Fixture::Shelving => 'S',
                Fixture::StockRack => 'R',
                Fixture::LoadingBay => 'L',
                Fixture::Counter => 'C',
            },
            Tile::Vehicle(p) => p.glyph(),
        }
    }

    /// Whether somebody can stand here.
    ///
    /// **A vehicle is walkable** — you stand on the seat tile to drive and
    /// on the cargo bed to load it — which is the whole reason vehicles
    /// are tiles and not a state you enter.
    ///
    /// Furniture mostly is not. You stand *at* a till and *in front of*
    /// shelving; you do not stand in a shelf bay, and letting people walk
    /// through the fittings would make a shop one open room with pictures
    /// of shelves on the floor.
    pub fn walkable(self) -> bool {
        match self {
            Tile::Wall | Tile::Water | Tile::Tree | Tile::Window => false,
            Tile::Fitting(f) => matches!(
                f,
                Fixture::Till | Fixture::Counter | Fixture::LoadingBay
            ),
            _ => true,
        }
    }
}

/// A window of ground, real for as long as somebody is looking at it.
pub struct Ground {
    pub w: usize,
    pub h: usize,
    /// Tile coordinates of the top-left corner, within the town.
    pub origin: (i64, i64),
    pub tiles: Vec<Tile>,
}

impl Ground {
    pub fn at(&self, x: usize, y: usize) -> Tile {
        self.tiles[y * self.w + x]
    }

    /// Generate the ground around a point, from the town above it.
    ///
    /// `centre` is in tiles. The plan supplies what is on each plot and the
    /// biome supplies what the unbuilt ground is made of, so a town in
    /// forest has trees between the houses and one in badlands has scrub.
    pub fn around(seed: u64, plan: &Plan, centre: (i64, i64), radius: usize) -> Self {
        let w = radius;
        let h = radius / 2; // terminals are twice as tall as they are wide
        let origin = (centre.0 - w as i64 / 2, centre.1 - h as i64 / 2);
        let mut tiles = Vec::with_capacity(w * h);

        for ty in 0..h {
            for tx in 0..w {
                let gx = origin.0 + tx as i64;
                let gy = origin.1 + ty as i64;
                tiles.push(tile_at(seed, plan, gx, gy));
            }
        }
        Ground {
            w,
            h,
            origin,
            tiles,
        }
    }

    /// Park a vehicle with its top-left corner here, laying its parts out
    /// on the ground the way `vehicle.rs` arranges them.
    pub fn park(&mut self, v: &Vehicle, at: (i64, i64)) {
        for &(part, px, py) in v.parts.iter() {
            let gx = at.0 + px as i64;
            let gy = at.1 + py as i64;
            let (tx, ty) = (gx - self.origin.0, gy - self.origin.1);
            if tx < 0 || ty < 0 || tx as usize >= self.w || ty as usize >= self.h {
                continue;
            }
            let i = ty as usize * self.w + tx as usize;
            // The most telling part on a tile wins, same as when a vehicle
            // is drawn on its own.
            let keep = match self.tiles[i] {
                Tile::Vehicle(old) => old.prominence() >= part.prominence(),
                _ => false,
            };
            if !keep {
                self.tiles[i] = Tile::Vehicle(part);
            }
        }
    }

    pub fn render(&self, person: Option<(i64, i64)>) -> String {
        let mut out = String::with_capacity((self.w + 1) * self.h);
        for y in 0..self.h {
            for x in 0..self.w {
                let here = (self.origin.0 + x as i64, self.origin.1 + y as i64);
                out.push(if person == Some(here) {
                    '@'
                } else {
                    self.at(x, y).glyph()
                });
            }
            out.push('\n');
        }
        out
    }
}

/// What is on one square metre.
///
/// **The whole thing is a pure function of where it is**, which is what
/// makes "generate on demand, store nothing" possible: no chunk has to
/// exist before its neighbour, and the same coordinates always give the
/// same tile.
fn tile_at(seed: u64, plan: &Plan, gx: i64, gy: i64) -> Tile {
    let t = TILES_PER_PLOT as i64;
    let (px, py) = (gx.div_euclid(t), gy.div_euclid(t));
    let (ix, iy) = (gx.rem_euclid(t), gy.rem_euclid(t));

    let lot = if px < 0 || py < 0 || px >= plan.width as i64 || py >= plan.height as i64 {
        Lot::Open
    } else {
        plan.at(px as usize, py as usize)
    };

    match lot {
        // **A street is not 32 m of tarmac.** A residential carriageway is
        // five or six metres with pavements either side; the rest of the
        // plot is verge and frontage. Paving the whole width gave every
        // lane the footprint of a dual carriageway.
        Lot::Street => {
            // **Which way does this street run?**
            //
            // From its neighbours: a plot with street above and below
            // carries a north-south road, one with street either side
            // carries an east-west road, and one with both is a
            // crossroads. Taking the nearer of the two centrelines
            // regardless put a crossroads in every single street plot,
            // which paves three quarters of the town.
            let lot_at = |dx: i64, dy: i64| -> Lot {
                let (nx, ny) = (px + dx, py + dy);
                if nx < 0 || ny < 0 || nx >= plan.width as i64 || ny >= plan.height as i64 {
                    Lot::Street // the road carries on out of town
                } else {
                    plan.at(nx as usize, ny as usize)
                }
            };
            let runs_ns = lot_at(0, -1) == Lot::Street || lot_at(0, 1) == Lot::Street;
            let runs_ew = lot_at(-1, 0) == Lot::Street || lot_at(1, 0) == Lot::Street;
            let mid = t / 2;
            let across = match (runs_ns, runs_ew) {
                (true, true) => (iy - mid).abs().min((ix - mid).abs()),
                (true, false) => (ix - mid).abs(),
                (false, true) => (iy - mid).abs(),
                // A stub of road going nowhere: still a bit of surface.
                (false, false) => (ix - mid).abs().max((iy - mid).abs()),
            };
            // **A residential carriageway is 5-6 m** with a 2 m pavement
            // either side. The rest of the plot is verge and frontage.
            if across <= 3 {
                Tile::Road
            } else if across <= 5 {
                Tile::Pavement
            } else {
                open_ground(seed, plan.ground, gx, gy)
            }
        }
        Lot::Open => open_ground(seed, plan.ground, gx, gy),
        Lot::Park => {
            if hash(seed, gx, gy, 3) < 0.18 {
                Tile::Tree
            } else {
                Tile::Grass
            }
        }
        Lot::House | Lot::Flats | Lot::Shop | Lot::Works => {
            building_tile(seed, plan, lot, gx, gy, ix, iy)
        }
    }
}

/// The shell of a building on its plot, and what is inside it.
fn building_tile(
    seed: u64,
    plan: &Plan,
    lot: Lot,
    gx: i64,
    gy: i64,
    ix: i64,
    iy: i64,
) -> Tile {
    let t = TILES_PER_PLOT as i64;
    // Set back from the plot edge — a house does not fill its garden, and
    // a shop leaves room for a pavement and a delivery yard.
    let inset = match lot {
        Lot::Shop | Lot::Works => 3,
        _ => 6,
    };
    let (lo, hi) = (inset, t - 1 - inset);
    if ix < lo || iy < lo || ix > hi || iy > hi {
        return open_ground(seed, plan.ground, gx, gy);
    }

    let on_wall = ix == lo || iy == lo || ix == hi || iy == hi;
    if on_wall {
        // The door faces the street, which here means the low side.
        let mid = (lo + hi) / 2;
        if iy == lo && (ix - mid).abs() <= 1 {
            return Tile::Door;
        }
        // Windows, but not on the corners.
        let corner = (ix == lo || ix == hi) && (iy == lo || iy == hi);
        if !corner && hash(seed, gx, gy, 4) < 0.35 {
            return Tile::Window;
        }
        return Tile::Wall;
    }

    // --- inside ---
    match lot {
        Lot::Shop => shop_interior(lo, hi, ix, iy),
        Lot::Works => {
            if hash(seed, gx, gy, 5) < 0.10 {
                Tile::Fitting(Fixture::StockRack)
            } else {
                Tile::Floor
            }
        }
        _ => Tile::Floor,
    }
}

/// **The inside of a shop, laid out the way one is.**
///
/// Tills across the front by the door, because that is where you pay on
/// the way out. Aisles of shelving through the middle. Stockroom racking
/// along the back wall, where the lorries come to. This is `building.rs`'s
/// fixture list given somewhere to stand.
fn shop_interior(lo: i64, hi: i64, ix: i64, iy: i64) -> Tile {
    let depth = hi - lo;
    let from_front = iy - lo;
    if from_front <= 1 {
        // The checkouts, with gaps to walk through.
        return if ix % 3 == 0 {
            Tile::Fitting(Fixture::Till)
        } else {
            Tile::Floor
        };
    }
    if from_front >= depth - 3 {
        // Goods in, and the racking behind it.
        return if from_front == depth - 1 && (ix - lo) % 5 == 2 {
            Tile::Fitting(Fixture::LoadingBay)
        } else if (ix - lo) % 2 == 0 {
            Tile::Fitting(Fixture::StockRack)
        } else {
            Tile::Floor
        };
    }
    // Aisles: a run of shelving, then a gangway wide enough for a trolley.
    if (iy - lo) % 3 != 0 && (ix - lo) % 8 != 0 {
        Tile::Fitting(Fixture::Shelving)
    } else {
        Tile::Floor
    }
}

/// Unbuilt ground, made of whatever country the town stands in.
fn open_ground(seed: u64, biome: Biome, gx: i64, gy: i64) -> Tile {
    let n = hash(seed, gx, gy, 1);
    use Biome::*;
    match biome {
        Ocean | Shallows => Tile::Water,
        Beach => {
            if n < 0.05 {
                Tile::Scrub
            } else {
                Tile::Sand
            }
        }
        Desert => {
            if n < 0.04 {
                Tile::Scrub
            } else {
                Tile::Sand
            }
        }
        Savanna => {
            if n < 0.05 {
                Tile::Tree
            } else {
                Tile::Grass
            }
        }
        Grassland => {
            if n < 0.02 {
                Tile::Tree
            } else {
                Tile::Grass
            }
        }
        Shrubland => {
            if n < 0.30 {
                Tile::Scrub
            } else {
                Tile::Grass
            }
        }
        Forest | Taiga => {
            if n < 0.45 {
                Tile::Tree
            } else if n < 0.55 {
                Tile::Scrub
            } else {
                Tile::Grass
            }
        }
        Rainforest => {
            if n < 0.65 {
                Tile::Tree
            } else {
                Tile::Scrub
            }
        }
        Swamp => {
            if n < 0.25 {
                Tile::Water
            } else if n < 0.40 {
                Tile::Tree
            } else {
                Tile::Grass
            }
        }
        Tundra => {
            if n < 0.10 {
                Tile::Rock
            } else {
                Tile::Scrub
            }
        }
        Mountain => {
            if n < 0.55 {
                Tile::Rock
            } else {
                Tile::Scrub
            }
        }
        Snowcap => Tile::Snow,
    }
}

fn hash(seed: u64, x: i64, y: i64, layer: u64) -> f32 {
    let mut h = seed
        ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ layer.wrapping_mul(0x2545_F491_4F6C_DD1D);
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    (h >> 11) as f32 / (1u64 << 53) as f32
}

/// Where a `Building`'s fixtures would stand, for anything that needs to
/// know rather than draw — a person walking to their till, say.
pub fn fixture_positions(b: &Building, lo: i64, hi: i64) -> Vec<(Fixture, i64, i64)> {
    let mut out = Vec::new();
    for iy in lo + 1..hi {
        for ix in lo + 1..hi {
            if let Tile::Fitting(f) = shop_interior(lo, hi, ix, iy) {
                out.push((f, ix, iy));
            }
        }
    }
    let _ = b;
    out
}
