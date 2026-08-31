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
/// **A Z level is a storey, not a metre.** Floor-to-floor in a real
/// building is 2.5-3 m, so height is measured in a coarser unit than the
/// ground is — which is exactly how Dwarf Fortress does it, and the reason
/// a stairwell occupies one tile of plan and joins two levels.
pub const METRES_PER_LEVEL: f64 = 3.0;

// Box-drawing, so a wall's shape comes from its neighbours rather than
// from a dozen separate terrain types.
const BOX_H: char = '\u{2500}';
const BOX_V: char = '\u{2502}';
const BOX_NW: char = '\u{250C}';
const BOX_NE: char = '\u{2510}';
const BOX_SW: char = '\u{2514}';
const BOX_SE: char = '\u{2518}';
const BOX_TEE_E: char = '\u{251C}';
const BOX_TEE_W: char = '\u{2524}';
const BOX_TEE_S: char = '\u{252C}';
const BOX_TEE_N: char = '\u{2534}';
const BOX_CROSS: char = '\u{253C}';

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

    /// Open air: above a roof, or beside a building on an upper level.
    /// **This is what makes a Z level a level** rather than a second map.
    Sky,

    /// Soil and subsoil: what a spade goes through.
    Earth,
    /// **A slope you can walk up.** DF's rule exactly: a change of level
    /// is either ramped or it is a cliff, and a cliff is not a tile type
    /// but the absence of a ramp.
    Ramp,

    /// A stairwell. **High-density housing is a core with dwellings hung
    /// off it**, not a big room with partitions.
    Stairs,
    /// A lift, which a block needs above about four storeys — that is the
    /// limit anybody will walk up, and the point at which a walk-up stops
    /// being buildable.
    Lift,

    /// Tarmac with bays painted on it. **A car park is bigger than the
    /// thing it serves**, which is real and is why it has to be drawn.
    Parking,

    /// A piece of a building's fitting-out, standing where it stands.
    Fitting(Fixture),
    /// Furniture, which is what makes a room a room rather than a floor.
    Furnishing(Furnishing),
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
            Tile::Sky => ' ',
            Tile::Earth => '&',
            Tile::Ramp => '<',
            Tile::Stairs => '>',
            Tile::Lift => 'V',
            Tile::Parking => '_',
            Tile::Furnishing(f) => f.glyph(),
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
            Tile::Wall | Tile::Water | Tile::Tree | Tile::Window | Tile::Sky => false,
            // Undug ground is not somewhere you can be.
            Tile::Earth | Tile::Rock => false,
            // A ramp is the whole point: it is the walkable step.
            Tile::Ramp => true,
            Tile::Fitting(f) => matches!(
                f,
                Fixture::Till | Fixture::Counter | Fixture::LoadingBay
            ),
            // You stand beside a bed and at a table; you do not stand in
            // the wardrobe.
            Tile::Furnishing(f) => matches!(f, Furnishing::Chair),
            _ => true,
        }
    }
}

/// **One legend for the tile view**, so every binary that draws ground
/// says the same thing about it. The vehicle parts are listed only when
/// there is a vehicle to explain.
pub fn ground_legend(with_vehicle: bool) -> String {
    let mut out = String::new();
    out.push_str("  you       @
");
    out.push_str("  made      = carriageway   : lane marking   ; hard shoulder   - footway
");
    out.push_str("  building  # wall   / door   o window   . floor
");
    out.push_str("  fittings  $ till   S shelving   R racking   L loading bay   C counter
");
    out.push_str("  furniture n bed   m table   h chair   e stove   k wardrobe
");
    out.push_str("  vertical  > stair   V lift   < ramp   ' ' air   & earth   ^ rock
");
    out.push_str("  country   \" grass   T tree   * scrub   , sand   ^ rock   A snow   ~ water");
    if with_vehicle {
        out.push_str("
  vehicle   + frame   E engine   O wheel   B cargo bay   F fuel tank");
        out.push_str("
            % seat   ! controls   b battery   a alternator");
        out.push_str("
            p solar   x refrigeration   w workshop rig   Y land gear");
    }
    out
}

/// **Furniture appropriate to the use**, which is the thing CDDA has and
/// a bare floor does not. A room with nothing in it is not a room, it is
/// an area.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Furnishing {
    Bed,
    Table,
    Chair,
    Stove,
    Wardrobe,
}

impl Furnishing {
    pub fn glyph(self) -> char {
        match self {
            Furnishing::Bed => 'n',
            Furnishing::Table => 'm',
            Furnishing::Chair => 'h',
            Furnishing::Stove => 'e',
            Furnishing::Wardrobe => 'k',
        }
    }
}

/// **What a room is for.** Assigned from where it sits, not chosen at
/// random: the room off the front door is the one you live in, the
/// kitchen backs onto the yard because that is where the drains and the
/// bins are, and the rest are bedrooms.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Room {
    Living,
    Kitchen,
    Bedroom,
    /// Circulation: hall, landing, stair. Deliberately empty.
    Hall,
}

/// A window of ground, real for as long as somebody is looking at it.
pub struct Ground {
    /// Which storey you are looking at. 0 is the ground.
    pub z: i64,
    /// **What is standing on the ground, which is not the ground.**
    ///
    /// A lorry parked on a road does not delete the road, and a bridge
    /// does not delete the river under it. Parking used to overwrite the
    /// terrain, so a street with a vehicle on it had no surface left
    /// underneath. The person was already composited at render time and
    /// vehicles were baked in, which was two mechanisms for one idea.
    pub over: Vec<Option<Part>>,
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
        Self::around_on(seed, plan, centre, radius, 0)
    }

    /// The same, `z` levels above or below the ground at `centre`.
    ///
    /// **Relative, because that is what a level means to somebody in the
    /// world**: 0 is the ground you are standing on, +1 the floor above,
    /// -1 the cellar. Absolute levels are the engine's business — a town
    /// 60 m above the sea has its ground at absolute level 20, and asking
    /// for 0 there gets you sixty metres of rock.
    pub fn around_on(
        seed: u64,
        plan: &Plan,
        centre: (i64, i64),
        radius: usize,
        z: i64,
    ) -> Self {
        // **Square, because a reality bubble is a radius and not a
        // viewport.** This used to halve the height so it fitted a
        // terminal, which meant somebody could see twice as far east as
        // north — and, less obviously, meant anything measured across an
        // east-west street was quietly cut off at 24 m. A motorway's hard
        // shoulders sat outside the window and a test that should have
        // caught it passed instead.
        Self::window_on(seed, plan, centre, radius * 2 + 1, radius * 2 + 1, z)
    }

    /// A window of a given size, for *looking at* rather than standing in.
    /// Terminals are about twice as tall as they are wide, so a view meant
    /// to look square on screen is not square in metres.
    pub fn window(seed: u64, plan: &Plan, centre: (i64, i64), w: usize, h: usize) -> Self {
        Self::window_on(seed, plan, centre, w, h, 0)
    }

    /// The same, `z` levels above or below the ground at `centre`.
    pub fn window_on(
        seed: u64,
        plan: &Plan,
        centre: (i64, i64),
        w: usize,
        h: usize,
        z: i64,
    ) -> Self {
        let z = surface_z(seed, plan, centre.0, centre.1) + z;
        let origin = (centre.0 - w as i64 / 2, centre.1 - h as i64 / 2);
        let mut tiles = Vec::with_capacity(w * h);

        for ty in 0..h {
            for tx in 0..w {
                let gx = origin.0 + tx as i64;
                let gy = origin.1 + ty as i64;
                tiles.push(tile_at(seed, plan, gx, gy, z));
            }
        }
        Ground {
            z,
            over: vec![None; tiles.len()],
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
            let keep = match self.over[i] {
                Some(old) => old.prominence() >= part.prominence(),
                None => false,
            };
            if !keep {
                self.over[i] = Some(part);
            }
        }
    }

    /// **The map is not the simulation; it is a viewport onto it.**
    ///
    /// Composited in priority order — person, then whatever stands on the
    /// ground, then the ground itself — so nothing has to be destroyed in
    /// order to be hidden. Walls take their glyph from their neighbours,
    /// which is presentation and not terrain: the tile is a `Wall` either
    /// way, and one terrain type yields corners, tees and crossings.
    pub fn render(&self, person: Option<(i64, i64)>) -> String {
        let mut out = String::with_capacity((self.w + 1) * self.h);
        for y in 0..self.h {
            for x in 0..self.w {
                let here = (self.origin.0 + x as i64, self.origin.1 + y as i64);
                out.push(if person == Some(here) {
                    '@'
                } else if let Some(part) = self.over[y * self.w + x] {
                    part.glyph()
                } else {
                    self.glyph_at(x, y)
                });
            }
            out.push('\n');
        }
        out
    }

    /// A wall run includes its doors and windows, because those are holes
    /// in a wall and not gaps between two.
    fn glyph_at(&self, x: usize, y: usize) -> char {
        let t = self.at(x, y);
        if t != Tile::Wall {
            return t.glyph();
        }
        let joins = |dx: isize, dy: isize| -> bool {
            let (nx, ny) = (x as isize + dx, y as isize + dy);
            if nx < 0 || ny < 0 || nx as usize >= self.w || ny as usize >= self.h {
                return false;
            }
            matches!(
                self.at(nx as usize, ny as usize),
                Tile::Wall | Tile::Door | Tile::Window
            )
        };
        match (joins(0, -1), joins(0, 1), joins(-1, 0), joins(1, 0)) {
            (true, true, true, true) => BOX_CROSS,
            (true, true, true, false) => BOX_TEE_W,
            (true, true, false, true) => BOX_TEE_E,
            (true, false, true, true) => BOX_TEE_N,
            (false, true, true, true) => BOX_TEE_S,
            (true, true, false, false) => BOX_V,
            (false, false, true, true) => BOX_H,
            (true, false, true, false) => BOX_SE,
            (true, false, false, true) => BOX_SW,
            (false, true, true, false) => BOX_NE,
            (false, true, false, true) => BOX_NW,
            (true, false, false, false) | (false, true, false, false) => BOX_V,
            _ => BOX_H,
        }
    }

    /// What stands on this tile, if anything — a lorry's wheel, say, with
    /// the road still underneath it.
    pub fn standing_on(&self, x: usize, y: usize) -> Option<Part> {
        self.over[y * self.w + x]
    }
}

/// What is on one square metre.
///
/// **The whole thing is a pure function of where it is**, which is what
/// makes "generate on demand, store nothing" possible: no chunk has to
/// exist before its neighbour, and the same coordinates always give the
/// same tile.
fn tile_at(seed: u64, plan: &Plan, gx: i64, gy: i64, gz: i64) -> Tile {
    let t = TILES_PER_PLOT as i64;
    let (px, py) = (gx.div_euclid(t), gy.div_euclid(t));
    let (ix, iy) = (gx.rem_euclid(t), gy.rem_euclid(t));

    let lot = if px < 0 || py < 0 || px >= plan.width as i64 || py >= plan.height as i64 {
        Lot::Open
    } else {
        plan.at(px as usize, py as usize)
    };

    // **Above the ground there is only what somebody built.**
    //
    // This is what a Z level buys, and it is how Dwarf Fortress manages
    // height: a building stops being a floorplate with a number of storeys
    // asserted about it and becomes a stack you can stand on any floor of.
    // Everything else at this height is air.
    // **Everything is relative to the ground here.**
    //
    // Terrain has height now, so level 0 is not a plane through the world
    // — it is wherever this tile's ground happens to be. A building stands
    // on the surface, a cellar is under it, and the sky starts above it.
    let sz = surface_z(seed, plan, gx, gy);
    let gz = gz - sz;

    // **A step in the ground is a ramp or it is a cliff.** Where the
    // neighbours are lower and the drop is one level, the ground slopes
    // and you can walk it; a bigger drop, or hard rock that keeps its
    // edge, and you cannot. Granite makes tors and chalk makes downland.
    if gz == 0 {
        let lower = [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)]
            .iter()
            .map(|&(dx, dy)| sz - surface_z(seed, plan, gx + dx, gy + dy))
            .max()
            .unwrap_or(0);
        if lower == 1 && !plan.rock.keeps_an_edge() {
            return Tile::Ramp;
        }
    }

    // **Below the ground, the same ladder downward.**
    //
    // A cellar, then what a spade goes through, then the rock the planet
    // put there. Returning air below ground was simply wrong: down is a
    // direction like up.
    if gz < 0 {
        return below_ground(seed, plan, lot, gx, gy, ix, iy, gz);
    }

    if gz != 0 {
        if !matches!(lot, Lot::House | Lot::Flats | Lot::Shop | Lot::Works) {
            return Tile::Sky;
        }
        let f = footprint_of(plan, lot, px, py);
        let across = if f.terraced { t } else { t - 2 * f.side };
        let floorplate = (t - f.front - f.back) * across;
        if gz >= storeys_of(lot, floorplate) {
            return Tile::Sky;
        }
        return building_tile(seed, plan, lot, gx, gy, ix, iy, gz);
    }

    match lot {
        // **A street is not 32 m of tarmac.** A residential carriageway is
        // five or six metres with pavements either side; the rest of the
        // plot is verge and frontage. Paving the whole width gave every
        // lane the footprint of a dual carriageway.
        Lot::Street => {
            // **Which way does this street run, and which road wins?**
            //
            // Both come from the plan's through-routes. Reading it off the
            // neighbouring plots instead put a crossroads in every single
            // street plot, which paves three quarters of the town — and it
            // could not tell a lane joining a trunk road from two lanes
            // meeting, so two motorways crossed at grade in the middle of
            // a city.
            let mid = t / 2;
            let mut roads: Vec<(StreetClass, i64, i64)> = Vec::new();
            if let Some(c) = plan.col_class(px as usize) {
                roads.push((c, (ix - mid).abs(), gy)); // runs north-south
            }
            if let Some(c) = plan.row_class(py as usize) {
                roads.push((c, (iy - mid).abs(), gx)); // runs east-west
            }
            roads.sort_by_key(|&(c, _, _)| std::cmp::Reverse(c.size()));

            // **The bigger road runs through and the lesser one stops at
            // it.** That is what severance is, and a motorway's corridor
            // fills the whole plot, so a street meeting one dead-ends
            // against it — which is exactly the claim the cross-sections
            // were already making and nothing was enforcing.
            let equal_crossing =
                roads.len() == 2 && roads[0].0.size() == roads[1].0.size();
            for &(c, across, along) in &roads {
                if let Some(tile) = cross_section(c, across, along, equal_crossing) {
                    return tile;
                }
            }
            // **A lane that is not a through-route**, which in a town
            // that grew rather than being laid out is most of them. It
            // still runs one way: take the direction from the neighbouring
            // plots, because the stub formula makes a square patch of
            // tarmac where a lane should be a strip.
            if roads.is_empty() {
                let lot_at = |dx: i64, dy: i64| -> Lot {
                    let (nx, ny) = (px + dx, py + dy);
                    if nx < 0 || ny < 0 || nx >= plan.width as i64 || ny >= plan.height as i64 {
                        Lot::Open
                    } else {
                        plan.at(nx as usize, ny as usize)
                    }
                };
                let ns = lot_at(0, -1) == Lot::Street || lot_at(0, 1) == Lot::Street;
                let ew = lot_at(-1, 0) == Lot::Street || lot_at(1, 0) == Lot::Street;
                let (across, along) = match (ns, ew) {
                    (true, true) => ((iy - mid).abs().min((ix - mid).abs()), gx),
                    (true, false) => ((ix - mid).abs(), gy),
                    (false, true) => ((iy - mid).abs(), gx),
                    (false, false) => ((ix - mid).abs().max((iy - mid).abs()), gx),
                };
                if let Some(tile) = cross_section(StreetClass::Lane, across, along, ns && ew) {
                    return tile;
                }
            }

            // **Between the kerb and the building line: paved in town,
            // verge in the country.** That is the difference between a
            // street and a road, and without it a 13 m corridor sat in the
            // middle of a 32 m plot with nineteen metres of grass either
            // side — so in a city of forty-six million the buildings stood
            // back from the road like farmhouses and a corner had nothing
            // on it. Which side matters: the edge of town is paved on the
            // built side and grass on the other.
            let built = |dx: i64, dy: i64| -> bool {
                let (nx, ny) = (px + dx, py + dy);
                nx >= 0
                    && ny >= 0
                    && nx < plan.width as i64
                    && ny < plan.height as i64
                    // **Dense frontage only.** A city-centre street is
                    // paved kerb to building line — Oxford Street has no
                    // verges — because the shops and blocks come right out
                    // to the footway. A street of houses does not: it has
                    // verges and front gardens, and paving those made a
                    // residential lane 100% made surface, which is a
                    // runway. The difference is the whole point.
                    && matches!(
                        plan.at(nx as usize, ny as usize),
                        Lot::Flats | Lot::Shop
                    )
            };
            // The side you are standing on, for each road through here: a
            // north-south road has neighbours east and west, an east-west
            // one north and south. Paving both sides regardless would pave
            // the fields at the edge of town.
            let mut built_side = false;
            for &(_, _, along) in &roads {
                built_side |= if along == gy {
                    built(if ix < mid { -1 } else { 1 }, 0)
                } else {
                    built(0, if iy < mid { -1 } else { 1 })
                };
            }
            // **At a junction the buildings are on the diagonals**, since
            // all four orthogonal neighbours are the streets themselves.
            // Miss this and every corner in the town is a patch of grass,
            // which is the one place a player is most likely to stand.
            if roads.len() == 2 {
                built_side |= built(
                    if ix < mid { -1 } else { 1 },
                    if iy < mid { -1 } else { 1 },
                );
            }
            let built = built_side;
            if built {
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
            building_tile(seed, plan, lot, gx, gy, ix, iy, 0)
        }
    }
}

/// **How a building sits on its plot, which is what urban density
/// actually looks like.**
///
/// Density was a number in the plan — Clark's law, flats in the middle,
/// houses outward — and nothing at the tile layer used it, so a city of
/// forty-six million had grass and trees between every building. A city
/// centre has a **street wall**: buildings on the back of the footway,
/// sharing party walls with their neighbours, with whatever open ground
/// there is behind them rather than around them.
///
/// Real site coverage *(building footprint over plot area)*: a dense urban
/// core is 60-80%, inner-city terraces 40-50%, interwar semis 25-30%,
/// detached suburbs 15-25%. What produces that spread is not plot size —
/// it is **setback and party walls**, so that is what is modelled.
///
/// A terrace is not a type here, it is a consequence: a house whose
/// neighbours along the street are also built shares walls with them, and
/// one whose neighbours are fields does not.
struct Footprint {
    front: i64,
    back: i64,
    side: i64,
    /// Whether the wall on the low side is shared with next door, in which
    /// case the high side has none and the neighbour's closes it. Real
    /// party walls are one wall, not two.
    terraced: bool,
}

fn footprint_of(plan: &Plan, lot: Lot, px: i64, py: i64) -> Footprint {
    let neighbours = px >= 0
        && py >= 0
        && px < plan.width as i64
        && py < plan.height as i64
        && plan.terraced(px as usize, py as usize);

    match lot {
        // A shopfront is on the back of the pavement, because a shop set
        // back behind a garden is not a shop anybody walks into. Service
        // yard behind. ~84% coverage.
        Lot::Shop => Footprint { front: 1, back: 4, side: 0, terraced: true },
        // A mansion block: on the street, joined to its neighbours, with
        // the bins and the drying green behind. ~78%.
        Lot::Flats => Footprint { front: 1, back: 6, side: 0, terraced: true },
        // A shed wants lorry access, so the yard is at the front.
        Lot::Works => Footprint { front: 6, back: 2, side: 2, terraced: false },
        // **The gradient.** A terraced house is 10 m deep with a 4 m front
        // garden and a long garden behind — a Victorian street. Detached,
        // it is a 12 m box in the middle of its ground.
        Lot::House if neighbours => {
            Footprint { front: 4, back: 18, side: 0, terraced: true }
        }
        _ => Footprint { front: 8, back: 14, side: 10, terraced: false },
    }
}

/// The shell of a building on its plot, and what is inside it.
#[allow(clippy::too_many_arguments)]
fn building_tile(
    seed: u64,
    plan: &Plan,
    lot: Lot,
    gx: i64,
    gy: i64,
    ix: i64,
    iy: i64,
    gz: i64,
) -> Tile {
    let t = TILES_PER_PLOT as i64;
    let (px, py) = (gx.div_euclid(t), gy.div_euclid(t));
    let f = footprint_of(plan, lot, px, py);

    // The front faces the street, which here means the low side.
    let (lo_y, hi_y) = (f.front, t - 1 - f.back);
    let (lo_x, hi_x) = if f.terraced {
        // Runs the full width and shares the wall on the low side, so
        // between two neighbours there is one wall and not two.
        (0, t - 1)
    } else {
        (f.side, t - 1 - f.side)
    };
    if ix < lo_x || iy < lo_y || ix > hi_x || iy > hi_y {
        return if gz == 0 {
            open_ground(seed, plan.ground, gx, gy)
        } else {
            Tile::Sky
        };
    }

    // **A 32 m frontage is five houses, not one.** A terrace is divided
    // by party walls every 6 m or so — real terraced frontages are 4.5 to
    // 6 m — and without them a street of houses was a single building the
    // width of the plot with one front door.
    let party = f.terraced
        && lot == Lot::House
        && ix % 6 == 0
        && ix != lo_x
        && ix != hi_x;
    let on_wall = party
        || ix == lo_x
        || iy == lo_y
        || iy == hi_y
        // A terrace's high flank is closed by next door's party wall.
        || (ix == hi_x && !f.terraced);
    if on_wall {
        // Every house in the terrace gets its own front door.
        let mid = (lo_x + hi_x) / 2;
        let own_door = if f.terraced && lot == Lot::House {
            iy == lo_y && ix % 6 == 3
        } else {
            iy == lo_y && (ix - mid).abs() <= 1
        };
        // **A door to the street exists on the ground floor only.** Above
        // it, the same wall carries a window — you get in by the stair.
        if own_door && gz == 0 {
            return Tile::Door;
        }
        if party {
            return Tile::Wall;
        }
        let corner = (ix == lo_x || ix == hi_x) && (iy == lo_y || iy == hi_y);
        if !corner && hash(seed, gx, gy, 4) < 0.35 {
            return Tile::Window;
        }
        return Tile::Wall;
    }

    // --- inside ---
    //
    // **A building is more than one room.** A bare floor inside four walls
    // is an area, not a place: what makes an interior legible is
    // partitions, doorways between them, and furniture that says what each
    // room is for.
    match lot {
        // **A high street is shops with flats over them**, which is what
        // a two-storey shop actually is and why town centres have people
        // living in them.
        Lot::Shop if gz == 0 => shop_interior(lo_x, hi_x, lo_y, hi_y, ix, iy),
        Lot::Works => {
            // A shed is one big space on purpose — that is what a shed is
            // for — with racking round the edges.
            if hash(seed, gx, gy, 5) < 0.10 {
                Tile::Fitting(Fixture::StockRack)
            } else {
                Tile::Floor
            }
        }
        // **High-density housing is a core with dwellings off it.**
        //
        // A stairwell, a lift once the block is taller than anybody will
        // walk *(four storeys is the limit of a walk-up, which is exactly
        // where lifts start)*, a landing, and flats opening onto it.
        // Subdividing the floorplate the way a house is subdivided gave
        // one enormous dwelling per block, which is not what a tenement is.
        Lot::Flats => {
            let (w, h) = (hi_x - lo_x + 1, hi_y - lo_y + 1);
            let core_x = lo_x + w / 2 - 2;
            let core_y = lo_y + 1;
            let storeys = storeys_of(Lot::Flats, w * h);
            if iy >= core_y && iy <= core_y + 4 && ix >= core_x && ix <= core_x + 4 {
                // 2.5 x 5 m of stair, a 1.8 m lift shaft beside it, and
                // the landing you step out onto.
                return if ix <= core_x + 1 {
                    Tile::Stairs
                } else if ix == core_x + 2 && storeys > 4 {
                    Tile::Lift
                } else {
                    Tile::Floor
                };
            }
            let key = px * 977 + py * 31;
            let (flat, _fw, _fh, wall) =
                room_at(seed, key, lo_x + 1, lo_y + 1, hi_x - 1, hi_y - 1, ix, iy, FLAT_M2);
            if let Some(door) = wall {
                return if door { Tile::Door } else { Tile::Wall };
            }
            // Rooms inside the flat. Descending again from the same
            // bounds with a smaller target lands in the same subdivision
            // and then keeps going, so a flat gets its own rooms.
            let (room, rw, rh, partition) = room_at(
                seed,
                key,
                lo_x + 1,
                lo_y + 1,
                hi_x - 1,
                hi_y - 1,
                ix,
                iy,
                ROOM_M2,
            );
            match partition {
                Some(true) => Tile::Door,
                Some(false) => Tile::Wall,
                None => furnish(
                    seed,
                    room ^ flat,
                    iy - lo_y,
                    rh,
                    ix,
                    iy,
                    rw <= 3 || rh <= 3 || hash(seed, gx, gy, 14) < 0.45,
                ),
            }
        }
        _ => {
            // A terrace is divided into houses first; each house is then
            // divided into rooms of its own, so the key has to include
            // which house it is.
            let unit = if f.terraced && lot == Lot::House {
                ix.div_euclid(6)
            } else {
                0
            };
            let (ux0, ux1) = if f.terraced && lot == Lot::House {
                (unit * 6 + 1, (unit + 1) * 6 - 1)
            } else {
                (lo_x + 1, hi_x - 1)
            };
            let key = px * 977 + py * 31 + unit;
            let (room, rw, rh, partition) =
                room_at(seed, key, ux0, lo_y + 1, ux1, hi_y - 1, ix, iy, ROOM_M2);
            match partition {
                Some(true) => Tile::Door,
                Some(false) => Tile::Wall,
                None => {
                    // Furniture goes against the walls, the way furniture
                    // does, leaving the middle to walk in.
                    let on_edge = ix == ux0
                        || ix == ux1
                        || iy == lo_y + 1
                        || iy == hi_y - 1
                        || rw <= 3
                        || rh <= 3;
                    furnish(seed, room, iy - lo_y, rh, ix, iy, on_edge)
                }
            }
        }
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
fn shop_interior(lo_x: i64, _hi_x: i64, lo_y: i64, hi_y: i64, ix: i64, iy: i64) -> Tile {
    // **Front and depth are different axes.** They were the same while
    // every building was a square inset in the middle of its plot; once a
    // shop ran the full width of a terrace they came apart, and passing
    // the width where the depth was wanted put the back wall halfway up
    // the shop.
    let depth = hi_y - lo_y;
    let from_front = iy - lo_y;
    let across = ix - lo_x;
    if from_front <= 1 {
        // The checkouts, with gaps to walk through.
        return if across % 3 == 0 {
            Tile::Fitting(Fixture::Till)
        } else {
            Tile::Floor
        };
    }
    if from_front >= depth - 3 {
        // Goods in, and the racking behind it.
        return if from_front == depth - 1 && across % 5 == 2 {
            Tile::Fitting(Fixture::LoadingBay)
        } else if across % 2 == 0 {
            Tile::Fitting(Fixture::StockRack)
        } else {
            Tile::Floor
        };
    }
    // Aisles: a run of shelving, then a gangway wide enough for a trolley.
    if from_front % 3 != 0 && across % 8 != 0 {
        Tile::Fitting(Fixture::Shelving)
    } else {
        Tile::Floor
    }
}

/// **The natural ground, in metres above the sea.**
///
/// Spec A1.3d again — interpolate the coarse field, add higher-frequency
/// variation, store nothing — one rung further down than `locality.rs`
/// does it. The plan carries the locality's relief and everything here is
/// scaled by that single figure, so a floodplain comes out flat and a
/// mountainside does not.
///
/// **What the arithmetic decides for you:** at a metre to the tile and
/// three metres to a level, one level of step is a 300% gradient — a
/// cliff. Real ground rises 5-30%, so natural terrain should cross a
/// level every 10 to 60 metres. That is not a tuned number, it falls out,
/// and it is why gentle country is genuinely flat at this scale and only
/// hard country gets vertical structure.
pub fn terrain_m(seed: u64, plan: &Plan, gx: i64, gy: i64) -> f64 {
    // Three octaves: the shape of the hill, its shoulders, and the
    // roughness underfoot. Wavelengths in metres, which is to say tiles.
    let mut h = 0.0;
    let mut amp = 1.0;
    let mut wave = 900.0;
    let mut norm = 0.0;
    for oct in 0..3 {
        let sx = (gx as f64 / wave).floor() as i64;
        let sy = (gy as f64 / wave).floor() as i64;
        let fx = (gx as f64 / wave) - sx as f64;
        let fy = (gy as f64 / wave) - sy as f64;
        // Bilinear between lattice corners, so the ground is smooth and a
        // hillside is a hillside rather than a field of noise.
        let c = |dx: i64, dy: i64| hash(seed, sx + dx, sy + dy, 20 + oct) as f64;
        let (u, v) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
        let top = c(0, 0) * (1.0 - u) + c(1, 0) * u;
        let bot = c(0, 1) * (1.0 - u) + c(1, 1) * u;
        h += (top * (1.0 - v) + bot * v - 0.5) * amp;
        norm += amp;
        amp *= 0.45;
        wave *= 0.28;
    }
    plan.elevation_m + (h / norm.max(1e-6)) * plan.relief_m
}

/// **The finished ground: what you actually stand on.**
///
/// Natural terrain, except that **anything made stands on a levelled
/// platform**. That is not a simplification, it is what cut and fill is:
/// nobody lays a floor on a slope and nobody builds a street that follows
/// every hummock. A building pad and a graded road are both flat, and the
/// step between a plot and its neighbour is where the retaining wall goes.
pub fn surface_m(seed: u64, plan: &Plan, gx: i64, gy: i64) -> f64 {
    let t = TILES_PER_PLOT as i64;
    let (px, py) = (gx.div_euclid(t), gy.div_euclid(t));
    let lot = if px < 0 || py < 0 || px >= plan.width as i64 || py >= plan.height as i64 {
        Lot::Open
    } else {
        plan.at(px as usize, py as usize)
    };
    if lot == Lot::Open || lot == Lot::Park {
        return terrain_m(seed, plan, gx, gy);
    }
    // Levelled to the middle of its own plot.
    terrain_m(seed, plan, px * t + t / 2, py * t + t / 2)
}

/// Which Z level the ground is at here.
pub fn surface_z(seed: u64, plan: &Plan, gx: i64, gy: i64) -> i64 {
    (surface_m(seed, plan, gx, gy) / METRES_PER_LEVEL).floor() as i64
}

/// **Real room and dwelling sizes** *(UK nationally described space
/// standard and ordinary practice)*. A double bedroom is 12-14 m², a
/// living room 16-20, a kitchen 8-12. A one-bed flat for two is 50 m², a
/// two-bed for four 70; the average new British flat is about 61.
const ROOM_M2: i64 = 9;
const FLAT_M2: i64 = 61;

/// **Whether the buildings here have cellars**, which is not a matter of
/// taste but of two real, opposite constraints.
///
/// A footing has to go below the frost line or it heaves, so where the
/// frost is deep you are digging that hole anyway and a basement is
/// nearly free — real frost depths run 1.5 m in Minnesota, 1.2 m in New
/// York and 0.13 m in Georgia, and basement prevalence follows almost
/// exactly: ~80% across the Midwest and Northeast, under 10% in the South.
///
/// The opposite constraint is water. New Orleans has no basements because
/// the water table is a metre down, and nor does anywhere built on a
/// marsh.
fn has_cellars(ground: Biome) -> bool {
    use Biome::*;
    match ground {
        // Hard winters: the hole is dug before you start.
        Taiga | Tundra | Snowcap | Mountain => true,
        // Temperate: sometimes, and historically often.
        Forest | Shrubland | Grassland => true,
        // Warm, or wet, or both. Nothing to gain and water in the way.
        Desert | Savanna | Rainforest | Swamp | Beach => false,
        Ocean | Shallows => false,
    }
}

/// **What is underneath.**
///
/// One level of cellar under a building where the climate justifies one,
/// a sewer under a made-up street, and below that the ground itself:
/// topsoil and subsoil for a metre or two *(real)*, then the bedrock
/// `geology.rs` decided on when the planet was made.
#[allow(clippy::too_many_arguments)]
fn below_ground(
    seed: u64,
    plan: &Plan,
    lot: Lot,
    gx: i64,
    gy: i64,
    ix: i64,
    iy: i64,
    gz: i64,
) -> Tile {
    let t = TILES_PER_PLOT as i64;
    // Below the first level down, nothing is dug: soil for a metre or two
    // and then rock. At 3 m to the level that puts bedrock at level -2.
    if gz < -1 {
        return Tile::Rock;
    }

    match lot {
        Lot::House | Lot::Flats | Lot::Shop | Lot::Works if has_cellars(plan.ground) => {
            let (px, py) = (gx.div_euclid(t), gy.div_euclid(t));
            let f = footprint_of(plan, lot, px, py);
            let (lo_y, hi_y) = (f.front, t - 1 - f.back);
            let (lo_x, hi_x) = if f.terraced {
                (0, t - 1)
            } else {
                (f.side, t - 1 - f.side)
            };
            if ix < lo_x || iy < lo_y || ix > hi_x || iy > hi_y {
                return Tile::Earth;
            }
            if ix == lo_x || ix == hi_x || iy == lo_y || iy == hi_y {
                return Tile::Wall;
            }
            // A cellar is storage, and it is where the stair comes down.
            let (w, _h) = (hi_x - lo_x + 1, hi_y - lo_y + 1);
            let core_x = lo_x + w / 2 - 2;
            if lot == Lot::Flats && iy >= lo_y + 1 && iy <= lo_y + 5 && ix <= core_x + 1 && ix >= core_x {
                return Tile::Stairs;
            }
            if hash(seed, gx, gy, 15) < 0.22 {
                Tile::Fitting(Fixture::StockRack)
            } else {
                Tile::Floor
            }
        }
        // **A street has a sewer under it**, which is the other thing
        // that is really down there. Victorian brick sewers run 3-10 m
        // down, so one level is about right; services sit shallower.
        Lot::Street => {
            let mid = t / 2;
            let across = (ix - mid).abs().min((iy - mid).abs());
            match across {
                0 => Tile::Water,   // the flow
                1 => Tile::Floor,   // the ledge you walk on
                2 => Tile::Wall,    // brick
                _ => Tile::Earth,
            }
        }
        _ => Tile::Earth,
    }
}

/// **How many floors a building has**, which is the thing a floorplate
/// alone cannot tell you and the reason the arithmetic did not close: a
/// block of eight dwellings on an 800 m² plate over four storeys works out
/// at 400 m² each, which is a mansion, not a tenement.
///
/// Real: a terrace is two storeys, a European tenement four to six, a
/// high-street shop has a floor over it, a shed is one. **Four is the
/// limit of a walk-up** — nobody carries shopping higher — which is
/// exactly where lifts start.
pub fn storeys_of(lot: Lot, floorplate_m2: i64) -> i64 {
    match lot {
        Lot::Flats => {
            if floorplate_m2 >= 600 {
                6
            } else {
                4
            }
        }
        Lot::House => 2,
        Lot::Shop => 2,
        _ => 1,
    }
}

/// **Where the partitions fall inside a building.**
///
/// A recursive split, always across the long way, until the pieces are
/// the size of real rooms. Real figures *(UK)*: a double bedroom is 12-14
/// m², a living room 16-20, a kitchen 8-12, a bathroom 4-6; an average
/// new-build house is 76 m² over about five rooms. So a 60 m² floor wants
/// four or five rooms, not one.
///
/// Computed per tile rather than stored, like everything else at this
/// layer — the descent is a handful of steps and it keeps `tile_at` a
/// pure function of its coordinates (spec A1.5).
///
/// Returns which room the tile is in, how big that room is, and whether
/// the tile is on a partition (and if so whether it is the doorway).
fn room_at(
    seed: u64,
    key: i64,
    mut x0: i64,
    mut y0: i64,
    mut x1: i64,
    mut y1: i64,
    ix: i64,
    iy: i64,
    smallest_m2: i64,
) -> (u64, i64, i64, Option<bool>) {
    let mut id: u64 = 1;
    for _ in 0..8 {
        let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
        if w * h <= smallest_m2 * 2 || w.min(h) < 4 {
            break;
        }
        // Always across the long way, so rooms stay roughly square rather
        // than becoming corridors.
        let vertical = w >= h;
        let span = if vertical { w } else { h };
        // Somewhere in the middle, so no room is a slot.
        let t = 0.35 + 0.30 * hash(seed, key, id as i64, 11) as f64;
        let cut = (span as f64 * t) as i64;
        let (lo, at) = if vertical { (x0, x0 + cut) } else { (y0, y0 + cut) };
        let _ = lo;
        let here = if vertical { ix } else { iy };

        if here == at {
            // On the partition. One doorway through it, placed along the
            // wall — a room with no door is a cupboard.
            let other = if vertical { iy } else { ix };
            let (o0, o1) = if vertical { (y0, y1) } else { (x0, x1) };
            let door = o0 + 1 + ((o1 - o0 - 1) as f64
                * (0.2 + 0.6 * hash(seed, key, id as i64, 12) as f64)) as i64;
            return (id, w, h, Some(other == door));
        }
        if here < at {
            if vertical { x1 = at - 1 } else { y1 = at - 1 }
            id = id * 2;
        } else {
            if vertical { x0 = at + 1 } else { y0 = at + 1 }
            id = id * 2 + 1;
        }
    }
    (id, x1 - x0 + 1, y1 - y0 + 1, None)
}

/// **What a room is for, and what stands in it.**
///
/// The room touching the front door is the one you live in; the kitchen
/// backs onto the yard, because that is where the drains and the bins
/// are; the rest are bedrooms. Furniture goes against the walls, the way
/// furniture actually does, leaving the middle to walk in.
fn furnish(
    seed: u64,
    room: u64,
    depth_from_front: i64,
    room_h: i64,
    ix: i64,
    iy: i64,
    on_edge: bool,
) -> Tile {
    let kind = if depth_from_front <= 1 {
        Room::Living
    } else if room % 3 == 0 {
        Room::Kitchen
    } else if room % 7 == 0 {
        Room::Hall
    } else {
        Room::Bedroom
    };
    if !on_edge {
        // The middle of a room is floor. A table is the one thing that
        // stands away from the wall.
        return if kind == Room::Living && room_h >= 5 && (ix + iy) % 5 == 0 {
            Tile::Furnishing(Furnishing::Table)
        } else {
            Tile::Floor
        };
    }
    let r = hash(seed, ix, iy, 13);
    match kind {
        Room::Hall => Tile::Floor,
        Room::Living => {
            if r < 0.35 {
                Tile::Furnishing(Furnishing::Chair)
            } else {
                Tile::Floor
            }
        }
        Room::Kitchen => {
            if r < 0.18 {
                Tile::Furnishing(Furnishing::Stove)
            } else if r < 0.55 {
                Tile::Fitting(Fixture::Counter)
            } else {
                Tile::Floor
            }
        }
        Room::Bedroom => {
            if r < 0.40 {
                Tile::Furnishing(Furnishing::Bed)
            } else if r < 0.55 {
                Tile::Furnishing(Furnishing::Wardrobe)
            } else {
                Tile::Floor
            }
        }
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
pub fn fixture_positions(
    b: &Building,
    lo_x: i64,
    hi_x: i64,
    lo_y: i64,
    hi_y: i64,
) -> Vec<(Fixture, i64, i64)> {
    let mut out = Vec::new();
    for iy in lo_y + 1..hi_y {
        for ix in lo_x + 1..hi_x {
            if let Tile::Fitting(f) = shop_interior(lo_x, hi_x, lo_y, hi_y, ix, iy) {
                out.push((f, ix, iy));
            }
        }
    }
    let _ = b;
    out
}
