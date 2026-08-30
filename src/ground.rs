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
use crate::townplan::{Lot, Plan, StreetClass, TILES_PER_PLOT};
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
    /// A painted line. **This is what makes a road two-way** — a centre
    /// line between the two directions, or a divider between lanes.
    Marking,
    /// A motorway's hard shoulder: surface you stop on, not drive on.
    Shoulder,

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
            Tile::Marking => ':',
            Tile::Shoulder => ';',
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
        // **Square, because a reality bubble is a radius and not a
        // viewport.** This used to halve the height so it fitted a
        // terminal, which meant somebody could see twice as far east as
        // north — and, less obviously, meant anything measured across an
        // east-west street was quietly cut off at 24 m. A motorway's hard
        // shoulders sat outside the window and a test that should have
        // caught it passed instead.
        Self::window(seed, plan, centre, radius * 2 + 1, radius * 2 + 1)
    }

    /// A window of a given size, for *looking at* rather than standing in.
    /// Terminals are about twice as tall as they are wide, so a view meant
    /// to look square on screen is not square in metres.
    pub fn window(seed: u64, plan: &Plan, centre: (i64, i64), w: usize, h: usize) -> Self {
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
            let junction = runs_ns && runs_ew;
            let across = match (runs_ns, runs_ew) {
                (true, true) => (iy - mid).abs().min((ix - mid).abs()),
                (true, false) => (ix - mid).abs(),
                (false, true) => (iy - mid).abs(),
                // A stub of road going nowhere: still a bit of surface.
                (false, false) => (ix - mid).abs().max((iy - mid).abs()),
            };
            let class = plan
                .street_class(px as usize, py as usize)
                .unwrap_or(StreetClass::Lane);
            // How far along the street are we? Markings are dashed, and
            // a dash has to line up along the road, not across it.
            let along = if runs_ns { gy } else { gx };
            cross_section(class, across, along, junction)
                .unwrap_or_else(|| open_ground(seed, plan.ground, gx, gy))
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
/// **What you find at a given distance from the centre line.**
///
/// Every figure is a real cross-section, because the alternative is
/// picking widths that look right and getting a town where a country lane
/// and a trunk road are the same object. `None` means you are past the
/// edge of the corridor and standing on whatever the country is.
///
/// The lane widths are the ones that get built: **2.75 m is the narrowest
/// anybody lays**, 3.65 m is standard and what a motorway uses. Which is
/// why a lane's two directions share 5.5 m with no line down it (you do
/// not mark a road that narrow — the two directions just give way), while
/// a motorway needs 11 m a side for its three.
fn cross_section(class: StreetClass, across: i64, along: i64, junction: bool) -> Option<Tile> {
    // **Lines are dashed, and the dash gets longer as the road gets
    // faster** — an ordinary centre line is a 2 m mark with a 7 m gap, and
    // a lane line at motorway speed is a long mark with a short gap,
    // because at 110 km/h a 2 m dash is gone before you have seen it.
    // Solid means something specific (do not cross), so the edge of a
    // carriageway is solid and the line between lanes is not.
    let dashed = |mark: i64, period: i64| {
        if junction {
            // Nothing is painted through a junction; that is where the
            // lines stop and give way.
            Tile::Road
        } else if along.rem_euclid(period) < mark {
            Tile::Marking
        } else {
            Tile::Road
        }
    };
    let centre_line = dashed(2, 9);
    let lane_line = dashed(4, 6);
    let edge_line = if junction { Tile::Road } else { Tile::Marking };
    match class {
        // ~10 m corridor: 5.5 m shared, 2 m footway each side.
        StreetClass::Lane => match across {
            0..=2 => Some(Tile::Road),
            3..=4 => Some(Tile::Pavement),
            _ => None,
        },
        // ~13 m: 7.3 m of two marked lanes, 2.5 m footways.
        StreetClass::Road => match across {
            0 => Some(centre_line),
            1..=3 => Some(Tile::Road),
            4..=6 => Some(Tile::Pavement),
            _ => None,
        },
        // ~25 m: a reserve, then 7.3 m of two lanes each way, then footways.
        StreetClass::Dual => match across {
            0..=1 => Some(Tile::Grass), // central reserve
            2 | 9 => Some(edge_line),   // solid: the edge of the carriageway
            5 => Some(lane_line),       // dashed: between the two lanes
            3..=8 => Some(Tile::Road),
            10..=12 => Some(Tile::Pavement),
            _ => None,
        },
        // ~33 m, which is the whole plot: reserve, 11 m of three lanes
        // each way, and a 3.3 m hard shoulder. **No footway** — you cannot
        // walk on a motorway, and a town it runs through is cut in two.
        StreetClass::Motorway => match across {
            0..=1 => Some(Tile::Grass),
            2 | 12 => Some(edge_line),
            5 | 9 => Some(lane_line),
            3..=11 => Some(Tile::Road),
            13..=16 => Some(Tile::Shoulder),
            _ => None,
        },
    }
}

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
