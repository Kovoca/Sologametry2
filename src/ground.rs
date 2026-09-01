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
use crate::geology::Rock;
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

/// **The sixteen colours, which is what DF actually runs on.**
///
/// The classic interface is CP437 in a 16-colour CGA palette, and the
/// palette is not decoration — it is half the information. Sand, soil and
/// mud are all `.` in DF and are told apart by colour alone. Adopting the
/// glyph conventions without the colour would have collapsed distinctions
/// this already draws separately.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Colour {
    Black,
    Blue,
    Green,
    Cyan,
    Red,
    Magenta,
    Brown,
    Grey,
    DarkGrey,
    LightBlue,
    LightGreen,
    LightCyan,
    LightRed,
    LightMagenta,
    Yellow,
    White,
}

impl Colour {
    /// **The hue, with the two brightnesses folded together.**
    ///
    /// Sixteen colours are really eight hues at two brightnesses, and two
    /// brightnesses of one hue are not enough to tell two *kinds of thing*
    /// apart at a glance — grey and dark grey on the same full block made
    /// a building in mountain country read as a crag. Brightness is
    /// therefore free to carry something else (how close, how lit, whether
    /// it matters); it is the hue that has to carry the class.
    ///
    /// Brown is dark yellow, which is why the two share a family.
    pub fn family(self) -> &'static str {
        match self {
            Colour::Black | Colour::DarkGrey | Colour::Grey | Colour::White => "grey",
            Colour::Green | Colour::LightGreen => "green",
            Colour::Blue | Colour::LightBlue => "blue",
            Colour::Cyan | Colour::LightCyan => "cyan",
            Colour::Red | Colour::LightRed => "red",
            Colour::Magenta | Colour::LightMagenta => "magenta",
            Colour::Brown | Colour::Yellow => "yellow",
        }
    }

    /// The ANSI escape for a foreground colour.
    pub fn ansi(self) -> &'static str {
        match self {
            Colour::Black => "\x1b[30m",
            Colour::Blue => "\x1b[34m",
            Colour::Green => "\x1b[32m",
            Colour::Cyan => "\x1b[36m",
            Colour::Red => "\x1b[31m",
            Colour::Magenta => "\x1b[35m",
            Colour::Brown => "\x1b[33m",
            Colour::Grey => "\x1b[37m",
            Colour::DarkGrey => "\x1b[90m",
            Colour::LightBlue => "\x1b[94m",
            Colour::LightGreen => "\x1b[92m",
            Colour::LightCyan => "\x1b[96m",
            Colour::LightRed => "\x1b[91m",
            Colour::LightMagenta => "\x1b[95m",
            Colour::Yellow => "\x1b[93m",
            Colour::White => "\x1b[97m",
        }
    }
}

/// **What the renderer decides**, kept apart from what the tile *is*.
/// Spec §16 in the terrain note: a glyph describes what is seen, it does
/// not define what exists.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Display {
    pub glyph: char,
    pub fg: Colour,
    /// **Drawn faint, because it is not on your level.** Sixteen colours
    /// cannot carry a second shade of every hue, so the level is carried
    /// by the ANSI faint attribute instead and the hue is left alone.
    /// Flattening everything below the eye to one grey threw away what it
    /// was: a whole town downhill came out as featureless smudge.
    pub dim: bool,
}

impl Tile {
    /// **DF's own conventions**, which are a functional mapping and not
    /// anybody's expression: `.` open ground, `"` grass, `,` loose stone,
    /// `≈` fluid, `█` solid rock, `♣` tree, `░` snow and ice, `+` door,
    /// `<` and `>` stairs, `▲` ramp, `@` the player.
    ///
    /// Where DF has no opinion because it models no such thing — lane
    /// markings, hard shoulders, footways, shop fittings — the glyph stays
    /// what it was.
    pub fn display(self) -> Display {
        let (glyph, fg) = match self {
            Tile::Grass => ('"', Colour::Green),
            Tile::Scrub => ('"', Colour::Brown),
            Tile::Sand => ('.', Colour::Yellow),
            Tile::Rock => ('\u{2588}', Colour::DarkGrey),
            Tile::Snow => ('\u{2591}', Colour::White),
            Tile::Water => ('\u{2248}', Colour::LightBlue),
            Tile::Tree => ('\u{2663}', Colour::LightGreen),
            // **Not the same shade as snow.** Both were the light block
            // and only the colour told them apart, so the moment a
            // terminal had no colour — or somebody looked at a screenshot
            // — a mountain town's snow-covered gardens and the undug
            // ground of a cellar were the same character. The plain table
            // had always distinguished them; the colour one had not.
            Tile::Earth => ('\u{2592}', Colour::Brown),
            Tile::Ramp => ('\u{25B2}', Colour::Grey),
            Tile::Sky => (' ', Colour::Black),
            Tile::Stairs => ('>', Colour::White),
            Tile::Lift => ('V', Colour::LightCyan),
            Tile::Road => ('=', Colour::DarkGrey),
            Tile::Marking => (':', Colour::Yellow),
            Tile::Shoulder => (';', Colour::DarkGrey),
            Tile::Pavement => ('-', Colour::Grey),
            Tile::Parking => ('_', Colour::DarkGrey),
            // **Masonry is not rock.** Both are the full block, so with a
            // wall in grey and a rock face in dark grey the two were a
            // shade apart and a building in mountain country read as a
            // crag. Colour carries the class, the glyph carries which one.
            Tile::Wall => ('\u{2588}', Colour::Brown),
            Tile::Floor => ('.', Colour::Grey),
            // **A way in is the same everywhere.** The wall carries what
            // the building is; a door is the one thing you look for
            // whatever building it belongs to, so it stays neutral and
            // bright rather than joining in.
            Tile::Door => ('+', Colour::White),
            Tile::Window => ('o', Colour::LightCyan),
            // **Not the shop's own colour**, or the fittings vanish into
            // the walls around them: a shop is magenta now, so its tills
            // and shelving cannot be. Cyan is equipment, which nothing
            // indoors competes with.
            Tile::Fitting(f) => (
                match f {
                    Fixture::Till => '$',
                    Fixture::Shelving => 'S',
                    Fixture::StockRack => 'R',
                    Fixture::LoadingBay => 'L',
                    Fixture::Counter => 'C',
                },
                Colour::Cyan,
            ),
            Tile::Furnishing(f) => (f.glyph(), Colour::Brown),
            Tile::Vehicle(p) => (p.glyph(), Colour::LightRed),
        };
        Display { glyph, fg, dim: false }
    }

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
    /// **Does this stop a line of sight?**
    ///
    /// The first stage of a visibility contract the renderer did not
    /// have. Every tile was drawn whether or not anything could see it,
    /// so a shop's shelving was legible from the middle of the street and
    /// a closed box van showed its seats, its tanks and its cargo to
    /// anybody standing beside it. The assembly was leaking into the view.
    ///
    /// **A wall that stops a ray is itself visible** — you see the wall,
    /// not through it — which is why opacity belongs to the boundary and
    /// not to the space behind it.
    pub fn opaque(self) -> bool {
        match self {
            // Boundaries.
            Tile::Wall | Tile::Rock | Tile::Earth => true,
            // **A door is shut until somebody opens it.** There is no open
            // state yet, and guessing the permissive one would show the
            // inside of every building in the town.
            Tile::Door => true,
            // Glazing: you see it, and you see through it.
            Tile::Window => false,
            // A canopy blocks the view along the ground.
            Tile::Tree => true,
            _ => false,
        }
    }

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

pub fn ground_legend(with_vehicle: bool) -> String {
    ground_legend_in(with_vehicle, false)
}

/// **The same key, painted the same way the map is.**
///
/// A legend in plain text against a coloured map is only half a key: it
/// tells you the glyph and leaves you to guess which of the six grey
/// things on the screen it meant. Each entry is drawn in its own colour,
/// so the eye can match it.
pub fn ground_legend_in(with_vehicle: bool, colour: bool) -> String {
    let mut out = String::new();
    let line = |head: &str, items: &[(Tile, &str)]| -> String {
        let mut row = format!("  {head:<9} ");
        for (i, (t, name)) in items.iter().enumerate() {
            if i > 0 {
                row.push_str("   ");
            }
            let d = t.display();
            if colour {
                row.push_str(d.fg.ansi());
            }
            row.push(d.glyph);
            if colour {
                row.push_str(Colour::Grey.ansi());
            }
            row.push(' ');
            row.push_str(name);
        }
        row.push('\n');
        row
    };
    if colour {
        out.push_str(Colour::Grey.ansi());
    }
    out.push_str(&format!(
        "  {:<9} {}@{} you\n",
        "you",
        if colour { Colour::White.ansi() } else { "" },
        if colour { Colour::Grey.ansi() } else { "" }
    ));
    out.push_str(&line(
        "country",
        &[
            (Tile::Grass, "grass"),
            (Tile::Scrub, "scrub"),
            (Tile::Tree, "tree"),
            (Tile::Sand, "sand"),
            (Tile::Rock, "rock"),
            (Tile::Snow, "snow"),
            (Tile::Earth, "earth"),
            (Tile::Water, "water"),
        ],
    ));
    out.push_str(&line(
        "made",
        &[
            (Tile::Road, "carriageway"),
            (Tile::Marking, "marking"),
            (Tile::Shoulder, "hard shoulder"),
            (Tile::Pavement, "footway"),
            (Tile::Parking, "yard"),
        ],
    ));
    out.push_str(&line(
        "building",
        &[
            (Tile::Wall, "wall"),
            (Tile::Door, "door"),
            (Tile::Window, "window"),
            (Tile::Floor, "floor"),
        ],
    ));
    // **A wall's colour says what the building is for**, which is the one
    // thing on the map that a glyph cannot carry.
    if colour {
        out.push_str("  use       ");
        for (c, name) in [
            (fabric_colour(Lot::House, 0, 0, 0), "a dwelling"),
            (fabric_colour(Lot::Shop, 0, 0, 0), "a shop"),
            (fabric_colour(Lot::Works, 0, 0, 0), "works"),
        ] {
            out.push_str(c.ansi());
            out.push('\u{2588}');
            out.push_str(Colour::Grey.ansi());
            out.push(' ');
            out.push_str(name);
            out.push_str("   ");
        }
        out.push('\n');
    }
    out.push_str(&line(
        "vertical",
        &[
            (Tile::Stairs, "stair"),
            (Tile::Lift, "lift"),
            (Tile::Ramp, "ramp"),
            (Tile::Sky, "open air"),
        ],
    ));
    out.push_str(&line(
        "fittings",
        &[
            (Tile::Fitting(Fixture::Till), "till"),
            (Tile::Fitting(Fixture::Shelving), "shelving"),
            (Tile::Fitting(Fixture::StockRack), "racking"),
            (Tile::Fitting(Fixture::LoadingBay), "loading bay"),
            (Tile::Fitting(Fixture::Counter), "counter"),
        ],
    ));
    out.push_str(&line(
        "furniture",
        &[
            (Tile::Furnishing(Furnishing::Bed), "bed"),
            (Tile::Furnishing(Furnishing::Table), "table"),
            (Tile::Furnishing(Furnishing::Chair), "chair"),
            (Tile::Furnishing(Furnishing::Stove), "stove"),
            (Tile::Furnishing(Furnishing::Wardrobe), "wardrobe"),
        ],
    ));
    if with_vehicle {
        // Every part of a vehicle is red, because what you need to know
        // about one is that it is a vehicle. Which part is the glyph's
        // job, and from outside a closed hull you only ever see two of
        // them anyway.
        let parts = |items: &[(Part, &str)]| -> String {
            let mut row = String::from("            ");
            for (i, (p, name)) in items.iter().enumerate() {
                if i > 0 {
                    row.push_str("   ");
                }
                if colour {
                    row.push_str(Colour::LightRed.ansi());
                }
                row.push(p.glyph());
                if colour {
                    row.push_str(Colour::Grey.ansi());
                }
                row.push(' ');
                row.push_str(name);
            }
            row.push('\n');
            row
        };
        let mut head = parts(&[
            (Part::Frame { heavy: false }, "frame"),
            (Part::Engine(300), "engine"),
            (Part::Wheel { heavy: false }, "wheel"),
            (Part::CargoBay(1000), "cargo bay"),
            (Part::Tank(500), "fuel tank"),
        ]);
        head.replace_range(2..9, "vehicle");
        out.push_str(&head);
        out.push_str(&parts(&[
            (Part::Seat, "seat"),
            (Part::Controls, "controls"),
            (Part::Battery(5), "battery"),
            (Part::Alternator(2000), "alternator"),
        ]));
        out.push_str(&parts(&[
            (Part::SolarPanel(200), "solar"),
            (Part::Refrigeration(10_000), "refrigeration"),
            (Part::WorkshopRig, "workshop rig"),
            (Part::LandGear { working_width_m: 6 }, "land gear"),
        ]));
    }
    // **Faint means another level**, which is the one thing on the map
    // that is not a glyph at all.
    if colour {
        out.push_str("  level     ");
        out.push_str("\x1b[2m");
        out.push_str(Colour::Grey.ansi());
        out.push_str("faint");
        out.push_str("\x1b[22m");
        out.push_str(Colour::Grey.ansi());
        out.push_str(" ground above or below the one you stand on\n");
        out.push_str("\x1b[0m");
    }
    out
}

/// **What has actually happened, as against what was generated.**
///
/// Everything below the world map is a pure function of seed and
/// coordinates (spec A1.5, rule R5), which is what keeps a save bounded —
/// and it also meant **nothing could ever change**. Knock a hole in a wall
/// and the wall came back the moment you looked away, because the
/// generator has no memory and it is the generator that answers.
///
/// The resolution is the one CDDA and DF both use: **generation gives the
/// initial state, and the tile wins.** A blueprint says a wall should be
/// here; this says whether it still is. Only tiles somebody actually
/// changed are stored, so a save stays proportional to what was done to
/// the world rather than to how much of it was visited.
///
/// A `BTreeMap` rather than a hash, because a save must write in the same
/// order every time.
#[derive(Default, Clone, Debug)]
pub struct Changes {
    edits: std::collections::BTreeMap<(i64, i64, i64), Tile>,
}

impl Changes {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record what a tile is *now*, whatever it was generated as.
    pub fn set(&mut self, at: (i64, i64, i64), tile: Tile) {
        self.edits.insert(at, tile);
    }

    /// Put a tile back under the generator's authority — which is not the
    /// same as setting it to what the generator currently says, because
    /// the generator can legitimately change when the world does.
    pub fn revert(&mut self, at: (i64, i64, i64)) {
        self.edits.remove(&at);
    }

    pub fn get(&self, at: (i64, i64, i64)) -> Option<Tile> {
        self.edits.get(&at).copied()
    }

    /// How much of the world has been touched. This, and not the size of
    /// the world, is what a save costs.
    pub fn len(&self) -> usize {
        self.edits.len()
    }

    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }
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
    /// **Which level each shown tile is on, relative to the eye's.** 0 is
    /// the level you are standing on, negative is below you, positive
    /// above.
    ///
    /// A window used to be one horizontal slice of the world, which is
    /// only ever right indoors. Outside, the ground goes up and down: on
    /// the low side the slice was above the ground and came back `Sky`, on
    /// the high side it was buried and came back solid earth. Standing on
    /// a levelled street in hill country that left most of the view either
    /// blank or walled off — not hidden, *absent* — which is not what
    /// happens when you stand on a kerb.
    pub rel: Vec<i16>,
    /// **The height of the ground at each cell in metres**, relative to the
    /// ground under the eye. This is what line of sight is measured
    /// against; `rel` is only how it is drawn.
    pub surf: Vec<f32>,
    /// **What colour this building is**, for the cells that are one.
    ///
    /// A tile cannot answer this, because a wall is a wall whatever it
    /// encloses — so the renderer has to, the same way it already picks a
    /// wall's box-drawing character from its neighbours rather than having
    /// a dozen kinds of wall tile.
    pub fabric: Vec<Colour>,
}

/// **A building takes the hue of what it is for.**
///
/// The plan view can tell a shop from a house from a works, and then you
/// walked down onto the street and every building was the same brown wall
/// — the identity vanished exactly where you would use it. A frontage is
/// how you tell a shop from a dwelling in reality, and at a metre to the
/// character there is no room for a sign, so colour does that work.
///
/// **Uniform within a building, varied between them.** The shade is drawn
/// off the plot, so neighbouring shops in a terrace differ slightly —
/// which is what lets you see where one ends and the next begins. Sharing
/// a party wall, they otherwise run together into one long shopfront.
///
/// Dwellings are one class here, not two. A house wall and a tenement wall
/// look alike and the distinction is not the one you need standing in
/// front of them; what you need is home, shop, or works.
pub fn fabric_colour(lot: Lot, px: i64, py: i64, seed: u64) -> Colour {
    let light = hash(seed, px, py, 37) < 0.5;
    match lot {
        Lot::House | Lot::Flats => {
            if light {
                Colour::Yellow
            } else {
                Colour::Brown
            }
        }
        Lot::Shop => {
            if light {
                Colour::LightMagenta
            } else {
                Colour::Magenta
            }
        }
        Lot::Works => {
            if light {
                Colour::LightRed
            } else {
                Colour::Red
            }
        }
        _ => Colour::Grey,
    }
}

/// **How far a view reaches past a step in the ground**, in Z levels: 16
/// is 48 m.
///
/// Local relief runs 2-10 m/km on a floodplain and 300-600 in mountain
/// country, so across a 92 m window the ground can move about 55 m at the
/// very worst. This covers it, and it costs a lookup only where the eye's
/// own level did not land on the ground.
const SIGHT_LEVELS: i64 = 16;

/// **Eye height in metres.** What you can see over a rise is decided by how
/// high off the ground you are looking, so it cannot be left out — at zero
/// the ground you stand on blocks you.
const EYE_M: f64 = 1.6;

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

    /// The same, with whatever has happened to the world laid over it.
    pub fn around_with(
        seed: u64,
        plan: &Plan,
        centre: (i64, i64),
        radius: usize,
        z: i64,
        changes: &Changes,
    ) -> Self {
        let r = radius * 2 + 1;
        Self::window_with(seed, plan, centre, r, r, z, changes)
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
        Self::window_with(seed, plan, centre, w, h, z, &Changes::default())
    }

    /// The same, with whatever has happened to the world laid over it.
    ///
    /// The overlay is applied *here*, where tiles are materialised, and
    /// not inside `tile_at` — the generator stays a pure function of its
    /// coordinates, which is what it is for. What changed is a separate
    /// fact about the world, not a different generator.
    #[allow(clippy::too_many_arguments)]
    pub fn window_with(
        seed: u64,
        plan: &Plan,
        centre: (i64, i64),
        w: usize,
        h: usize,
        z: i64,
        changes: &Changes,
    ) -> Self {
        // **Only the eye's own level goes looking for the ground.** Asking
        // for a level explicitly — the sewer under a street, the third
        // floor of a block — means that level and not a search for the
        // nearest surface to it, or there would be no way to look at a
        // cellar at all.
        let standing = z == 0;
        let z = surface_z(seed, plan, centre.0, centre.1) + z;
        let origin = (centre.0 - w as i64 / 2, centre.1 - h as i64 / 2);
        let mut tiles = Vec::with_capacity(w * h);

        let mut rel = Vec::with_capacity(w * h);
        let mut surf = Vec::with_capacity(w * h);
        let mut fabric = Vec::with_capacity(w * h);
        let eye_m = surface_m(seed, plan, centre.0, centre.1);

        for ty in 0..h {
            for tx in 0..w {
                let gx = origin.0 + tx as i64;
                let gy = origin.1 + ty as i64;
                let look = |lz: i64| {
                    changes
                        .get((gx, gy, lz))
                        .unwrap_or_else(|| tile_at(seed, plan, gx, gy, lz))
                };
                // **Take the surface, not the slice.**
                //
                // Where the eye's level is above the ground it is sky, so
                // look down until it is not; where it is below the ground
                // it is solid, so look up. Either way what is drawn is the
                // surface of the ground at that spot, which is the thing
                // somebody standing here can actually see.
                //
                // Indoors nothing moves — floors are level — so this costs
                // nothing and changes nothing inside a building.
                let mut t = look(z);
                let mut d = 0i64;
                if !standing {
                    // The caller asked for this level; give them it.
                } else if t == Tile::Sky {
                    while t == Tile::Sky && -d < SIGHT_LEVELS {
                        d -= 1;
                        t = look(z + d);
                    }
                } else if matches!(t, Tile::Earth | Tile::Rock) {
                    // Climbing out of the ground reaches the surface tile
                    // itself — grass, road, floor — because that is what
                    // the generator puts at a surface level. There is no
                    // stepping back down onto the soil under it.
                    while matches!(t, Tile::Earth | Tile::Rock) && d < SIGHT_LEVELS {
                        d += 1;
                        t = look(z + d);
                    }
                }
                if matches!(t, Tile::Sky | Tile::Earth | Tile::Rock) && d != 0 {
                    // Nothing found within sight: leave it as the slice
                    // said, and let it read as unknown ground.
                    t = look(z);
                    d = 0;
                }
                tiles.push(t);
                rel.push(d as i16);
                // **Sight is a question of metres, not of levels.** The 3 m
                // level is how the world is *drawn*; quantising the
                // viewshed to it turned a road climbing at five per cent
                // into a flight of three-metre walls, each of which hid
                // everything past it. The ground itself is continuous and
                // the line of sight has to be measured against that.
                surf.push((surface_m(seed, plan, gx, gy) - eye_m) as f32);
                let t = TILES_PER_PLOT as i64;
                let (px, py) = (gx.div_euclid(t), gy.div_euclid(t));
                let lot = if px < 0
                    || py < 0
                    || px >= plan.width as i64
                    || py >= plan.height as i64
                {
                    Lot::Open
                } else {
                    plan.at(px as usize, py as usize)
                };
                fabric.push(fabric_colour(lot, px, py, seed));
            }
        }
        Ground {
            z,
            over: vec![None; tiles.len()],
            w,
            h,
            origin,
            tiles,
            rel,
            surf,
            fabric,
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
        self.draw(person, false, true)
    }

    /// The same, in DF's sixteen colours.
    pub fn render_in_colour(&self, person: Option<(i64, i64)>) -> String {
        self.draw(person, true, true)
    }

    /// **Everything, whether or not anybody can see it.** Clearly what it
    /// is: an inspection view, not a player's view.
    pub fn render_omniscient(&self, person: Option<(i64, i64)>, colour: bool) -> String {
        self.draw(person, colour, false)
    }

    fn draw(&self, person: Option<(i64, i64)>, colour: bool, eyes: bool) -> String {
        // Where the viewer is standing decides what there is to draw.
        let seen = match (eyes, person) {
            (true, Some(at)) => self.visible_from(at),
            _ => vec![true; self.w * self.h],
        };
        let mut out = String::with_capacity((self.w + 1) * self.h);
        let mut last: Option<(Colour, bool)> = None;
        for y in 0..self.h {
            for x in 0..self.w {
                let here = (self.origin.0 + x as i64, self.origin.1 + y as i64);
                // Priority: the player, then whatever stands on the
                // ground, then the ground itself.
                let d = if person == Some(here) {
                    Display { glyph: '@', fg: Colour::White, dim: false }
                } else if !seen[y * self.w + x] {
                    // Out of sight. Not a void — simply not known, which
                    // is a different thing from empty and will become
                    // remembered ground once there is a memory to keep it
                    // in.
                    Display { glyph: ' ', fg: Colour::Black, dim: false }
                } else if let Some(part) = self.over[y * self.w + x] {
                    // **What an enclosure shows is its outside.** A part
                    // inside a closed hull is not on view to somebody
                    // standing next to it, any more than a bed is visible
                    // through a house wall.
                    //
                    // The inspection view is the exception, and it is the
                    // whole reason to have one: it shows the assembly.
                    let shown = if eyes { part.seen_from_outside() } else { part };
                    Display { glyph: shown.glyph(), fg: Colour::LightRed, dim: false }
                } else {
                    let mut d = self.at(x, y).display();
                    // Topology decides a wall's line and use decides its
                    // colour; the tile stays a wall either way.
                    if self.at(x, y) == Tile::Wall {
                        d.glyph = self.wall_glyph(x, y);
                        d.fg = self.fabric[y * self.w + x];
                    }
                    // **Ground on another level is drawn dimmer**, because
                    // it is not where you are standing. The glyph is
                    // unchanged — it is the same ground, seen from above
                    // or below.
                    if self.rel[y * self.w + x] != 0 {
                        d.dim = true;
                    }
                    d
                };
                if colour && last != Some((d.fg, d.dim)) {
                    // Faint has to be cleared explicitly, or every colour
                    // after the first dim tile stays dim.
                    out.push_str(if d.dim { "\x1b[2m" } else { "\x1b[22m" });
                    out.push_str(d.fg.ansi());
                    last = Some((d.fg, d.dim));
                }
                out.push(d.glyph);
            }
            out.push('\n');
        }
        if colour {
            out.push_str("\x1b[0m");
        }
        out
    }

    /// **What can actually be seen from a point**, by casting a ray to
    /// every cell in the window.
    ///
    /// The contract, in order: boundary opacity, then line of sight, then
    /// the visible set, then the renderer. Nothing is drawn that nothing
    /// can see.
    ///
    /// The first opaque cell along a ray is visible and everything beyond
    /// it is not — so a wall shows and the room behind it does not, and a
    /// window shows *and* lets the ray through, which is what a window is.
    pub fn visible_from(&self, eye: (i64, i64)) -> Vec<bool> {
        let mut seen = vec![false; self.w * self.h];
        let (ex, ey) = (eye.0 - self.origin.0, eye.1 - self.origin.1);
        if ex < 0 || ey < 0 || ex as usize >= self.w || ey as usize >= self.h {
            // Nobody is looking, so everything is drawn: this is the
            // omniscient case and it is used deliberately.
            return vec![true; self.w * self.h];
        }
        for ty in 0..self.h as i64 {
            for tx in 0..self.w as i64 {
                // **Cast it both ways.**
                //
                // One Bresenham line is not symmetric: stepping from the eye
                // and stepping from the target visit different cells, so a
                // great many places plainly in view are called hidden
                // because the single line the algorithm happened to pick
                // clipped the corner of something. Standing on an open
                // crossroads gave a narrow wedge of visible road with a
                // staircase edge to it, when what you see from a crossroads
                // is most of the way down all four streets.
                //
                // Accepting either direction is the cheap half of what a
                // shadowcaster does properly, and it removes the artefact.
                if self.ray_reaches((ex, ey), (tx, ty), (ex, ey))
                    || self.ray_reaches((tx, ty), (ex, ey), (ex, ey))
                {
                    seen[ty as usize * self.w + tx as usize] = true;
                }
            }
        }
        seen
    }

    /// **What stops a ray at this cell**: the ground, or whatever is
    /// standing on it.
    ///
    /// A vehicle's hull is a boundary in exactly the way a wall is, which
    /// is the whole point of having one contract rather than two — a box
    /// van parked across a window blocks the view through it, and fixing
    /// that once fixes containers, railcars and enclosed machinery with
    /// it.
    pub fn blocks_sight(&self, x: usize, y: usize) -> bool {
        if let Some(part) = self.over[y * self.w + x] {
            if part.opaque() {
                return true;
            }
        }
        // **A boundary only blocks you at your own level.** A wall down in
        // the cutting does not hide the far side from somebody standing on
        // the lip, and a wall up on the bank is not between you and the
        // road. What ground at another level does to a view is a question
        // of height, and it is answered by the profile check below.
        if self.rel[y * self.w + x] != 0 {
            return false;
        }
        self.at(x, y).opaque()
    }

    /// Bresenham from the eye to the target. Stops at the first opaque
    /// cell — which is itself reached, because you can see a wall.
    ///
    /// **And at the first ground that stands above the line of sight.**
    /// Tile opacity alone is a flat-world rule: it can say a wall is in the
    /// way and cannot say a hill is. So the ray carries a height as well as
    /// a position — starting at the eye, ending at whatever it is looking
    /// at — and any ground rising above that line stops it. Which is what
    /// a crest is, and it is also why a gentle slope hides nothing: the
    /// line climbs with the ground.
    fn ray_reaches(&self, from: (i64, i64), to: (i64, i64), eye: (i64, i64)) -> bool {
        let (mut x, mut y) = from;
        let (dx, dy) = ((to.0 - x).abs(), -(to.1 - y).abs());
        let (sx, sy) = (if x < to.0 { 1 } else { -1 }, if y < to.1 { 1 } else { -1 });
        let mut err = dx + dy;
        // The line of sight runs from the eye outward whichever way round
        // the cells happen to be walked, or a hill would be transparent
        // from one side and solid from the other.
        let aim = if eye == from { to } else { from };
        let height = |p: (i64, i64)| self.surf[p.1 as usize * self.w + p.0 as usize] as f64;
        let eye_h = height(eye) + EYE_M;
        let aim_h = height(aim);
        let span = (aim.0 - eye.0).abs().max((aim.1 - eye.1).abs()) as f64;
        loop {
            if (x, y) == to {
                return true;
            }
            // The eye's own cell never blocks it.
            if (x, y) != eye && self.blocks_sight(x as usize, y as usize) {
                return false;
            }
            if (x, y) != eye && span > 0.0 {
                let gone = (x - eye.0).abs().max((y - eye.1).abs()) as f64 / span;
                let line = eye_h + (aim_h - eye_h) * gone;
                if height((x, y)) > line {
                    return false;
                }
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
            if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
                return false;
            }
        }
    }

    fn wall_glyph(&self, x: usize, y: usize) -> char {
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
    //
    // **Not where somebody has built.** A built plot is levelled all the
    // way across, so the only step is at its boundary — which is exactly
    // where the building's flank wall stands, and the ramp was being
    // returned first and eating it. A shop came out with a line of ramps
    // down its east side and no wall at all, open to the air. What holds
    // back the ground beside a building is the building, or a retaining
    // wall; it is never a slope through the shop floor.
    let built = matches!(lot, Lot::House | Lot::Flats | Lot::Shop | Lot::Works);
    if gz == 0 && !built {
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
        if gz >= levels_of(lot, floorplate) {
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
        if gz != 0 {
            return Tile::Sky;
        }
        // **What is behind a shop is a yard, not a forest.**
        //
        // The remainder of a plot was always the raw biome, so the back of
        // every block in a city centre came out as woodland — and standing
        // at a central junction in a city of three and a half million you
        // looked past the buildings straight into taiga. The land use was
        // reaching the plot; it stopped at the building's own footprint.
        //
        // A commercial plot's spare ground is made ground: hardstanding,
        // bins, a service alley and somewhere to turn a van round. A
        // house's is a garden, which *is* grass and trees, and that
        // difference is the whole of it.
        return match lot {
            Lot::Shop | Lot::Works | Lot::Flats => {
                // A little planting survives even on a service yard.
                if hash(seed, gx, gy, 11) < 0.06 {
                    Tile::Grass
                } else {
                    Tile::Parking
                }
            }
            _ => open_ground(seed, plan.ground, gx, gy),
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
    // **A terrace's high flank is closed by next door's party wall — if
    // there is a next door.**
    //
    // At the end of a terrace, or anywhere the next plot along is a
    // street, there is no neighbour to supply it, and the building ran
    // open to the air: you could stand at a till and look straight out of
    // the side of the shop with no wall, no corner and no window. The
    // roof was held up by nothing.
    //
    // So the flank is only left open where something is actually going to
    // close it.
    //
    // **And a neighbour on another level supplies nothing.** A building is
    // levelled to the street it fronts, so the plot next door can sit a
    // whole level up or down — and its party wall is then on a different Z
    // and never appears at this one. The shop came out open to the air
    // again, with a line of ramps where its east wall should have been. A
    // party wall is only shared by two buildings standing on the same
    // ground.
    let closed_by_next_door = f.terraced && {
        let (nx, ny) = (px + 1, py);
        let t = TILES_PER_PLOT as i64;
        nx >= 0
            && ny >= 0
            && nx < plan.width as i64
            && ny < plan.height as i64
            && matches!(
                plan.at(nx as usize, ny as usize),
                Lot::House | Lot::Flats | Lot::Shop | Lot::Works
            )
            && surface_z(seed, plan, nx * t + t / 2, ny * t + t / 2)
                == surface_z(seed, plan, px * t + t / 2, py * t + t / 2)
            // **And it has to be built up to the boundary.** Only a
            // terraced neighbour puts its wall on the shared line; a
            // works stands two metres back off it and a detached house
            // ten, so relying on either leaves the flank open with a
            // strip of yard where the wall should be. That is what was
            // still happening after the level check went in.
            && footprint_of(plan, plan.at(nx as usize, ny as usize), nx, ny).terraced
    };
    let on_wall = party
        || ix == lo_x
        || iy == lo_y
        || iy == hi_y
        || (ix == hi_x && !closed_by_next_door);
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
        // **A dock is an opening in the back wall.** A bay the lorry
        // reverses onto is no use if the wall behind it is solid: the
        // goods have to come through. Real docks are roller shutters,
        // which is a door that happens to be three metres wide.
        if lot == Lot::Shop && gz == 0 && iy == hi_y {
            let across = ix - lo_x;
            if across % 8 < 2 {
                return Tile::Door;
            }
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
    // **A footway does not run across a carriageway.**
    //
    // The junction flag stopped the *paint* and not the pavement, so
    // every crossroads had a footway laid straight through it: the
    // east-west carriageway ran in, hit three metres of kerbed footway,
    // and resumed on the far side. Nothing could drive across it. At a
    // real crossroads the whole box is carriageway and the footway stops
    // at the kerb, which is exactly why there is a crossing painted on
    // the approach rather than a path through the middle.
    let foot = if junction { Tile::Road } else { Tile::Pavement };
    // **And neither does a central reservation.** A dual carriageway
    // whose reserve runs unbroken through a crossroads is a road you can
    // drive into and not across: three metres of grass in the middle of
    // the box. Real dual carriageways have a gap in the reservation at
    // every junction, which is the whole reason a right turn is possible
    // at some of them and not others.
    let reserve = if junction { Tile::Road } else { Tile::Grass };
    match class {
        // ~10 m corridor: 5.5 m shared, 2 m footway each side.
        StreetClass::Lane => match across {
            0..=2 => Some(Tile::Road),
            3..=4 => Some(foot),
            _ => None,
        },
        // ~13 m: 7.3 m of two marked lanes, 2.5 m footways.
        StreetClass::Road => match across {
            0 => Some(centre_line),
            1..=3 => Some(Tile::Road),
            4..=6 => Some(foot),
            _ => None,
        },
        // ~25 m: a reserve, then 7.3 m of two lanes each way, then footways.
        StreetClass::Dual => match across {
            0..=1 => Some(reserve), // central reserve
            2 | 9 => Some(edge_line),   // solid: the edge of the carriageway
            5 => Some(lane_line),       // dashed: between the two lanes
            3..=8 => Some(Tile::Road),
            10..=12 => Some(foot),
            _ => None,
        },
        // ~33 m, which is the whole plot: reserve, 11 m of three lanes
        // each way, and a 3.3 m hard shoulder. **No footway** — you cannot
        // walk on a motorway, and a town it runs through is cut in two.
        StreetClass::Motorway => match across {
            // A motorway's reserve is never broken — its junctions are
            // grade-separated, which is what makes it a motorway.
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
fn shop_interior(lo_x: i64, hi_x: i64, lo_y: i64, hi_y: i64, ix: i64, iy: i64) -> Tile {
    // **Front and depth are different axes.** They were the same while
    // every building was a square inset in the middle of its plot; once a
    // shop ran the full width of a terrace they came apart, and passing
    // the width where the depth was wanted put the back wall halfway up
    // the shop.
    let depth = hi_y - lo_y;
    let width = hi_x - lo_x;
    let from_front = iy - lo_y;
    let across = ix - lo_x;

    // **The back of house is a separate room, and customers never see
    // it.** The racking and the loading bays used to stand in the sales
    // floor with the shelving, in one undivided space — so a shopper
    // walked past the pallets and a lorry unloaded into the aisle.
    //
    // Real supermarkets put **20-30% of the floor area behind a wall**:
    // stockroom, chill room, staff area and the dock. The wall has staff
    // doors through it and nothing else.
    let back_of_house = (depth / 4).max(4);
    let partition = depth - back_of_house;
    if from_front == partition {
        // Staff doors through to the shop floor, at the ends of the
        // aisles rather than in the middle of the run.
        return if across % 9 == 4 {
            Tile::Door
        } else {
            Tile::Wall
        };
    }
    if from_front > partition {
        // **The dock is where the lorry backs up to**, so it sits against
        // the rear wall with the racking in front of it — you unload
        // across the bay and put it straight on a rack.
        return if from_front == depth - 1 {
            if across % 8 < 2 {
                Tile::Fitting(Fixture::LoadingBay)
            } else {
                Tile::Floor
            }
        } else if across % 2 == 0 {
            Tile::Fitting(Fixture::StockRack)
        } else {
            Tile::Floor
        };
    }

    // **A corner shop is not a small supermarket.** Under about 400 m²
    // there is a served counter and no checkout line at all, which is the
    // difference between the two trades rather than a matter of scale —
    // real corner shops run 250-1,000 m² against a superstore's
    // 2,800-4,650.
    if width * depth < 400 || depth < 14 {
        if from_front == 1 {
            return if across % 4 == 0 && across > 1 {
                Tile::Fitting(Fixture::Counter)
            } else {
                Tile::Floor
            };
        }
        if from_front < 3 {
            return Tile::Floor;
        }
        return if from_front % 3 != 0 && across % 5 != 0 {
            Tile::Fitting(Fixture::Shelving)
        } else {
            Tile::Floor
        };
    }

    // **A shop floor is mostly the space between the shelves.**
    //
    // The fittings were right and the circulation was not: aisles one tile
    // wide, shelving hard against the walls, and the checkouts standing
    // immediately inside the door with nowhere to queue. You cannot pass a
    // trolley in a metre and a line of six people had nowhere to stand.
    //
    // | | real |
    // |---|---|
    // | aisle, two trolleys passing | **1.8-2.4 m** |
    // | decompression zone inside the door | 1.5-4.5 m *(5-15 ft)* |
    // | queuing space at a checkout | 2-3 m |
    // | gondola run, double-sided | 1.0-1.25 m deep |
    // | perimeter racetrack | the main circulation, wider than an aisle |
    //
    // The decompression zone is a real retail term and a real measurement:
    // the first few metres inside a door are where people adjust and do
    // not buy, which is why nobody puts stock there.
    const LOBBY: i64 = 2;
    const QUEUE: i64 = 3;
    let tills_at = LOBBY + 1;
    let floor_start = tills_at + 1 + QUEUE;

    if from_front < tills_at {
        // Between the door and the checkouts: trolleys, and room to pack.
        return Tile::Floor;
    }
    if from_front == tills_at {
        // **A way in that is not through a checkout.** The entrance lane
        // runs past the end of the line — which is where the trolleys
        // stand, and is why you can walk into a supermarket without
        // squeezing between two tills.
        if across < 6 {
            return Tile::Floor;
        }
        return if (across - 6) % 3 == 0 {
            Tile::Fitting(Fixture::Till)
        } else {
            Tile::Floor
        };
    }
    if from_front < floor_start {
        // Where the queue stands.
        return Tile::Floor;
    }
    // **The racetrack**: the perimeter aisle a shop is circulated on, all
    // the way round the sales floor. Shelving used to run hard up against
    // the walls, so there was no way round the outside at all.
    if across < 3 || across > width - 3 || from_front >= partition - 2 {
        return Tile::Floor;
    }
    // A cross aisle every dozen metres, so a run is not forty metres long.
    if (across - 2) % 13 >= 11 {
        return Tile::Floor;
    }
    // Gondolas back to back, then an aisle two trolleys wide.
    if (from_front - floor_start) % 4 < 2 {
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
    let own = |ax: i64, ay: i64| terrain_m(seed, plan, ax * t + t / 2, ay * t + t / 2);
    // **A street is graded, not benched.** A road is cut and filled to a
    // steady gradient and then it *slopes*: it does not sit flat for
    // thirty-two metres and step down three at the kerb. Levelling every
    // street plot to its own centre built exactly that — a staircase of
    // retaining walls the length of every road, which at the bottom of one
    // put a wall across the view down the street.
    //
    // Real urban grades are 4-8% and hurt above about 10%; San Francisco's
    // worst is 31.5%. Five per cent over a plot is 1.6 m, which is a slope
    // you walk up without noticing and not a step you climb.
    if lot == Lot::Street {
        return terrain_m(seed, plan, gx, gy);
    }
    // **A building is levelled to the street it fronts, not to itself.**
    //
    // Levelling every plot to its own centre put a step between the
    // carriageway and the frontage beside it, and since plot edges are
    // straight the step ran dead straight the whole length of the road: a
    // forty-six tile wall of ramps between the pavement and the shop
    // doors. Nobody builds that. A street is graded as a corridor and the
    // frontages are cut and filled to *meet* it — that is what a building
    // line is, and it is why you step off a kerb and not off a cliff.
    //
    // Terraces still step down a hillside in runs, because neighbouring
    // stretches of street are at different heights. Bath and every hill
    // town in the world does exactly that; what they do not do is stand
    // three metres above their own pavement.
    for (dx, dy) in [(0i64, -1i64), (1, 0), (0, 1), (-1, 0)] {
        let (nx, ny) = (px + dx, py + dy);
        if nx < 0 || ny < 0 || nx >= plan.width as i64 || ny >= plan.height as i64 {
            continue;
        }
        if plan.at(nx as usize, ny as usize) == Lot::Street {
            return own(nx, ny);
        }
    }
    own(px, py)
}

/// **Tile Z is a local window, not a planetary range.**
///
/// A settlement on a 4,000 m plateau needs ordinary surface tiles, not a
/// coordinate 1,300 levels up. Local Z is the primary number — 0 is the
/// ground you are standing on — and absolute height is *derived*:
///
/// ```text
/// absolute elevation = local surface elevation + local Z x 3 m
/// ```
///
/// There is therefore no planetary Z range to bound, and none is defined.
/// What bounds a window is what is generated near somebody, which is the
/// same rule the horizontal ladder already runs on.
pub fn elevation_at_level(seed: u64, plan: &Plan, gx: i64, gy: i64, local_z: i64) -> f64 {
    surface_m(seed, plan, gx, gy) + local_z as f64 * METRES_PER_LEVEL
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

/// **What the ground is made of at a given depth.**
///
/// Terrain and material are separate things (a tile is `Rock`; *which*
/// rock is a different question), and a geological layer spans many Z
/// levels rather than being one of them. Real thicknesses:
///
/// | | depth below surface |
/// |---|---|
/// | topsoil | 0.1-0.3 m |
/// | subsoil | to 1-2 m |
/// | weathered rock | to ~10 m |
/// | sedimentary cover | 0 on a shield, typically 1-2 km on a continent |
/// | crystalline basement | below all of it |
///
/// **Whether there is any cover at all is what the surface rock tells
/// you.** Standing on sedimentary rock means a basin with cover under it;
/// standing on igneous or metamorphic means the basement *is* the
/// surface, which is what an exposed shield is.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Stratum {
    Topsoil,
    Subsoil,
    /// Broken, weathered rock: diggable with hand tools.
    Regolith,
    /// Bedded rock — sandstone, limestone, shale — where there is a basin.
    Cover(Rock),
    /// The crystalline floor everything else sits on.
    Basement(Rock),
}

impl Stratum {
    /// What it is made of. Soil is soil, whatever the rock beneath.
    pub fn rock(self) -> Option<Rock> {
        match self {
            Stratum::Cover(r) | Stratum::Basement(r) => Some(r),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Stratum::Topsoil => "topsoil",
            Stratum::Subsoil => "subsoil",
            Stratum::Regolith => "weathered rock",
            Stratum::Cover(_) => "bedded rock",
            Stratum::Basement(_) => "basement",
        }
    }

    /// Whether it is rock rather than something a spade goes through.
    pub fn is_rock(self) -> bool {
        matches!(self, Stratum::Cover(_) | Stratum::Basement(_))
    }
}

/// **Which way a level change faces.**
///
/// A ramp is not a glyph, it is a direction: the simulation must never
/// read the character to decide whether something is walkable. The
/// renderer picks a symbol from this; nothing else does.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Facing {
    North,
    South,
    East,
    West,
}

/// **A vertical connection, which both ends have to agree about.**
///
/// A stair down at (x, y, z) is only real if there is a stair up at
/// (x, y, z-1). Storing it as one tile's property lets the two drift; a
/// function that answers for both ends cannot.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Connections {
    pub up: bool,
    pub down: bool,
}

/// What connects vertically at this tile, computed for both ends at once.
pub fn connections_at(seed: u64, plan: &Plan, gx: i64, gy: i64, gz: i64) -> Connections {
    let is_stair = |z: i64| tile_at(seed, plan, gx, gy, z) == Tile::Stairs;
    Connections {
        up: is_stair(gz) && is_stair(gz + 1),
        down: is_stair(gz) && is_stair(gz - 1),
    }
}

/// Which way a ramp at this tile falls, if it is one.
pub fn ramp_facing(seed: u64, plan: &Plan, gx: i64, gy: i64) -> Option<Facing> {
    if tile_at(seed, plan, gx, gy, surface_z(seed, plan, gx, gy)) != Tile::Ramp {
        return None;
    }
    let here = surface_z(seed, plan, gx, gy);
    [
        (Facing::North, 0i64, -1i64),
        (Facing::South, 0, 1),
        (Facing::West, -1, 0),
        (Facing::East, 1, 0),
    ]
    .into_iter()
    .find(|&(_, dx, dy)| surface_z(seed, plan, gx + dx, gy + dy) < here)
    .map(|(f, _, _)| f)
}

/// The layer at a given depth below the surface, in metres.
pub fn stratum_at(plan: &Plan, depth_m: f64) -> Stratum {
    // A shield has no cover: the basement reaches the surface, which is
    // precisely what exposed igneous and metamorphic rock means.
    let cover_m = match plan.rock {
        Rock::Sedimentary => 1_500.0,
        _ => 0.0,
    };
    if depth_m < 0.3 {
        Stratum::Topsoil
    } else if depth_m < 1.6 {
        Stratum::Subsoil
    } else if depth_m < 10.0 {
        Stratum::Regolith
    } else if depth_m < 10.0 + cover_m {
        Stratum::Cover(Rock::Sedimentary)
    } else {
        // Under the cover, or straight away where there is none.
        Stratum::Basement(match plan.rock {
            Rock::Sedimentary => Rock::Metamorphic,
            other => other,
        })
    }
}

/// **Whether the buildings here have cellars.**
///
/// Two real, opposite constraints, and now both are measured rather than
/// guessed at from the biome.
///
/// A footing has to go below the frost line or it heaves, so where the
/// frost is deep you are digging that hole anyway and a basement is
/// nearly free — frost depths run 1.5 m in Minnesota, 1.2 in New York and
/// 0.13 in Georgia, and US basement prevalence follows almost exactly:
/// ~80% across the Midwest and Northeast, under 10% in the South.
///
/// The opposite constraint is water. **New Orleans has no basements
/// because the water table is a metre down**, and that was standing in as
/// a biome test until the world generated a water table to ask instead.
/// A cellar floor sits about 2.5 m down, so it wants the water at least
/// three metres below the surface.
fn has_cellars(plan: &Plan) -> bool {
    use Biome::*;
    let cold_enough = matches!(
        plan.ground,
        Taiga | Tundra | Snowcap | Mountain | Forest | Shrubland | Grassland
    );
    // A cellar floor is ~2.5 m down; leave half a metre under it.
    cold_enough && plan.water_m >= 3.0
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
    // Below the first level down, nothing is dug — the terrain is
    // whatever the strata put there. **Terrain, not material**: a tile is
    // `Earth` or `Rock`, and which rock is `stratum_at`'s business.
    if gz < -1 {
        let depth_m = -gz as f64 * METRES_PER_LEVEL;
        return if stratum_at(plan, depth_m).is_rock() {
            Tile::Rock
        } else {
            Tile::Earth
        };
    }

    match lot {
        Lot::House | Lot::Flats | Lot::Shop | Lot::Works if has_cellars(plan) => {
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

/// **A storey and a Z level are not the same thing.**
///
/// An ordinary storey takes one level. A warehouse bay, a workshop, a
/// theatre, an atrium or a double-height lobby takes two or more — real
/// industrial clear height is 6-12 m against a dwelling's 2.5-3, so a
/// shed is one storey and two or three levels. Assuming they are the same
/// is what makes a floor appear wherever the next Z coordinate happens to
/// exist.
pub fn levels_per_storey(lot: Lot) -> i64 {
    match lot {
        // A shed is one storey with the roof a long way up.
        Lot::Works => 3,
        _ => 1,
    }
}

/// How many Z levels a building occupies in total.
pub fn levels_of(lot: Lot, floorplate_m2: i64) -> i64 {
    storeys_of(lot, floorplate_m2) * levels_per_storey(lot)
}

/// **A floor is a boundary between two volumes, not a property of
/// either.**
///
/// It has no thickness in plan and it is what decides whether anything —
/// a person, water, a falling object — passes between one level and the
/// next. Mining through a floor is *modifying the boundary*, not deleting
/// a tile; that distinction is what makes shafts, bridges, grates,
/// collapses and double-height spaces the same mechanism rather than five.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Floor {
    pub material: FloorMaterial,
    /// A hole: a stairwell, a hoistway, an atrium, a breach.
    pub open: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum FloorMaterial {
    /// The ground itself, which is the boundary at the surface.
    Ground,
    Timber,
    Concrete,
}

impl Floor {
    /// Whether somebody standing on the level above is held up by it.
    pub fn supports_weight(self) -> bool {
        !self.open
    }

    /// Whether water gets through.
    pub fn liquid_permeable(self) -> bool {
        self.open
    }
}

/// **What separates this level from the one below it, if anything.**
///
/// `None` is an open volume — the two levels are one space. That is what
/// a double-height bay is, and a stairwell, and a lift shaft: not special
/// objects, just a missing boundary.
pub fn floor_below(seed: u64, plan: &Plan, gx: i64, gy: i64, gz: i64) -> Option<Floor> {
    let t = TILES_PER_PLOT as i64;
    let (px, py) = (gx.div_euclid(t), gy.div_euclid(t));
    let sz = surface_z(seed, plan, gx, gy);
    let rel = gz - sz;

    let lot = if px < 0 || py < 0 || px >= plan.width as i64 || py >= plan.height as i64 {
        Lot::Open
    } else {
        plan.at(px as usize, py as usize)
    };

    // At the surface, the ground is the boundary. Solid unless somebody
    // has dug through it.
    if rel == 0 {
        return Some(Floor {
            material: FloorMaterial::Ground,
            open: false,
        });
    }

    if rel < 0 {
        // Underground: rock and soil are their own boundary. A cellar has
        // a floor slab under it.
        return Some(Floor {
            material: if rel == -1 {
                FloorMaterial::Concrete
            } else {
                FloorMaterial::Ground
            },
            open: false,
        });
    }

    // Above ground there is a floor only where a building has one.
    if !matches!(lot, Lot::House | Lot::Flats | Lot::Shop | Lot::Works) {
        return None; // open air
    }
    let f = footprint_of(plan, lot, px, py);
    let across = if f.terraced { t } else { t - 2 * f.side };
    let floorplate = (t - f.front - f.back) * across;
    if rel >= levels_of(lot, floorplate) {
        return None; // above the roof
    }

    // **Inside a single storey there is no floor.** A shed's clear height
    // is one volume however many levels it spans.
    if rel % levels_per_storey(lot) != 0 {
        return None;
    }

    // A stairwell or a hoistway is a hole through every floor it passes:
    // that is what makes it a shaft rather than a stack of cupboards.
    let open = matches!(
        tile_at(seed, plan, gx, gy, gz),
        Tile::Stairs | Tile::Lift
    );
    Some(Floor {
        material: if lot == Lot::Works {
            FloorMaterial::Concrete
        } else {
            FloorMaterial::Timber
        },
        open,
    })
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
