//! A town's ground plan: streets, plots, and what stands on them.
//!
//! The rung between the world map and a building's interior. Until now a
//! market was a point with a population hung off it, and the shop the
//! economy fitted out with tills and shelving stood nowhere in particular.
//! This gives it an address.
//!
//! **Generated, never stored** (spec A1.5). A plan is a pure function of
//! the world seed and the settlement's cell, so walking away and coming
//! back rebuilds it identically and the save stays small. Only things that
//! actually happen — a fire, a wreck, a sale — would ever be written down,
//! and that is a later concern.
//!
//! ## Scale
//!
//! One plot is 25 m square, which is a house and its garden, or a shop
//! front, or a lane. That is the resolution CDDA's overmap works at and it
//! is the right one for the question this answers: what is on this street?
//!
//! Real densities, which is what makes a town the size it is:
//! - A European city runs 3,000-6,000 people per km²; London is ~5,700.
//! - A household is about 2.4 people.
//! - Blocks are 80-200 m between streets.
//! - Density falls off from the centre roughly exponentially — Clark's
//!   law, and it holds remarkably well across cities and centuries.

use crate::ground::Colour;
use crate::rng::Rng;
use crate::world::Biome;

/// **Plots a big shop runs across.** 96 m of frontage, which after the
/// service yard is about 2,600 m² — a real superstore is 2,800-4,650, a
/// corner shop 250-1,000. Beyond three it would be a shopping centre,
/// which is a different thing.
pub const SHOP_PLOTS: usize = 3;

/// **Plots deep a big store runs.** A 96 m frontage one plot deep is a
/// 910 m² strip; two deep is about 3,000 m² of sales floor, which is a
/// supermarket by the US bands *(20,000-50,000 sq ft, median ~40,000)*.
/// A store is a block, not a strip.
pub const SHOP_DEEP: usize = 2;

/// Metres across one plot — a house and its garden, a shop front, a lane.
///
/// **32, so the ladder multiplies out.** A region cell is 16 localities of
/// 1,024 m; a locality is 32 plots of 32 m; a plot is 32 tiles of a metre.
/// 16 x 32 x 32 = 16,384, which is why the region cell is 16.384 km. It
/// was 25 while nothing hung off it and the arithmetic did not close.
pub const METRES_PER_PLOT: f64 = 32.0;

/// Tiles across one plot. The bottom of the ladder: one tile is a metre,
/// which is where a person stands.
pub const TILES_PER_PLOT: usize = 32;

/// People in a household *(real: 2.3-2.6 across the developed world)*.
pub const HOUSEHOLD: f64 = 2.4;

/// Households on one plot of flats.
///
/// Four storeys with two dwellings a floor is the ordinary European
/// tenement, and it is what makes a centre hold 10,000 to a square
/// kilometre where a street of houses holds 2,500.
pub const HOUSEHOLDS_PER_BLOCK: f64 = 8.0;

/// **Households on one plot of houses, which depends on the form.**
///
/// A 32 m frontage is not one house. Real frontages: a terraced house is
/// 4.5-6 m, a semi 8-9, a detached 10-15 — so the same plot holds five
/// terraces, three semis or one detached house, and *that* is what makes
/// a terraced street four times denser than a suburb of the same plots.
/// One dwelling per plot regardless housed a village of nine hundred with
/// two hundred and seventy-four.
///
/// The resulting gross densities are real: a terrace at 5 households comes
/// to ~11,700/km² *(Islington runs 10-13,000)* and a detached plot to
/// ~2,300 *(detached suburbs 1,500-3,000)*.
pub const HOUSEHOLDS_PER_TERRACE: f64 = 5.0;

/// **How big a road is, which is decided by what uses it.**
///
/// Real cross-sections, and the reason a town does not look like a grid of
/// identical strips *(all figures real, UK/EU practice)*:
///
/// | | carriageway | corridor | carries |
/// |---|---|---|---|
/// | lane | 5.5 m (2 x 2.75) | ~10 m | under 300 vehicles a day |
/// | road | 7.3 m (2 x 3.65) | ~13 m | up to 13,000 |
/// | dual | 2 x 7.3 + 4 m reserve | ~22 m | beyond that |
/// | motorway | 2 x 11 (3 lanes) + shoulders | ~34 m | a trunk route |
///
/// A lane is two-way at 2.75 m a lane, which is the minimum anybody
/// builds; 3.65 is the standard lane and what a motorway uses. Note that
/// a motorway corridor is *wider than a 32 m plot* — which is right, and
/// is why a trunk road through a town takes a whole block and severs it.
///
/// The thresholds are the same ones `network.rs` uses to decide whether a
/// stretch of country gets paved at all: about 300 vehicles a day to
/// justify pavement, about 13,000 before a single carriageway needs
/// dualling.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum StreetClass {
    /// Residential access. Most of the length of any town.
    Lane,
    /// A distributor: the road you take to get out of your estate.
    Road,
    /// Dual carriageway, with a reserve down the middle.
    Dual,
    /// A trunk route, wider than the plots either side of it.
    Motorway,
}

impl StreetClass {
    /// Half-width of the running surface, in metres from the centre line.
    pub fn carriageway_half_m(self) -> i64 {
        match self {
            StreetClass::Lane => 2,      // 5.5 m, two lanes of 2.75
            StreetClass::Road => 3,      // 7.3 m, two lanes of 3.65
            StreetClass::Dual => 9,      // two of 7.3 either side of a reserve
            StreetClass::Motorway => 16, // two of 11, three lanes each way
        }
    }

    /// **How wide one lane is.** 3.65 m is the standard everywhere it
    /// matters; 2.75 m is the narrowest anybody lays and is what you get
    /// on a residential street where the two directions share the surface
    /// and give way to each other.
    pub fn lane_width_m(self) -> f64 {
        match self {
            StreetClass::Lane => 2.75,
            StreetClass::Road | StreetClass::Dual | StreetClass::Motorway => 3.65,
        }
    }

    /// Lanes in one direction.
    pub fn lanes_each_way(self) -> usize {
        match self {
            StreetClass::Lane | StreetClass::Road => 1,
            StreetClass::Dual => 2,
            StreetClass::Motorway => 3,
        }
    }

    /// **The surface something over-wide can actually use.**
    ///
    /// On a lane or a road you can take the whole width, because closing
    /// it to oncoming traffic is a thing that happens. On a dual or a
    /// motorway you cannot: the other side is behind a reserve and might
    /// as well be a different road.
    pub fn usable_m(self) -> f64 {
        match self {
            StreetClass::Lane => 5.5,
            StreetClass::Road => 7.3,
            StreetClass::Dual => 7.3,
            StreetClass::Motorway => 10.95,
        }
    }

    /// **What this road says about something that wide.**
    ///
    /// The bands are the real ones *(UK Special Types order)*, and they are
    /// the reason an army moves tanks on transporters rather than driving
    /// them, and why a grid operator cannot simply deliver a replacement
    /// transformer to a substation.
    ///
    /// For scale: a lorry is 2.55 m — the European legal maximum — and is
    /// ordinary traffic; a main battle tank is 3.5-3.9 m and lands
    /// squarely in the escorted band; a large power transformer is
    /// 3.5-4.5 m and often needs the order.
    pub fn clearance_for(self, width_m: f64) -> Clearance {
        let usable = self.usable_m();
        // **US bands** *(FHWA National Network and state permit practice)*:
        //
        // | over | what it takes |
        // |---|---|
        // | **8 ft 6 in** | an oversize permit from every state crossed |
        // | **12 ft** | pilot cars, daylight running only |
        // | **14 ft** | police escort and a surveyed route, in most states |
        // | 16 ft *(or 200,000 lb)* | superload: bridge-by-bridge review |
        //
        // For scale: a semi is 8 ft 6 in and legal everywhere; an M1
        // Abrams is 12 ft and escorted everywhere; a large power
        // transformer is 12-15 ft and is why a substation stays dark.
        //
        // **Known gap:** a superload is triggered by *weight* as much as
        // width — a 400-ton transformer trips it whatever its beam — and
        // this only knows about width.
        let permission = if width_m > 4.27 {
            Permission::SpecialOrder
        } else if width_m > 3.66 {
            Permission::Escorted
        } else if width_m > 2.59 {
            Permission::Notifiable
        } else {
            Permission::Ordinary
        };
        // **Can anything come the other way?** One rule for every class:
        // the load, a car beside it and a metre of air have to fit on the
        // surface. Below that somebody stops and waits, which is what a
        // lorry in a village actually is — and it is the only part of this
        // that depends on which road you are standing on.
        Clearance {
            permission,
            blocks_the_road: width_m + 1.8 + 1.0 > usable,
            will_not_fit: width_m > usable,
        }
    }

    /// Bigger is bigger. Used where two roads meet and one has to win.
    pub fn size(self) -> u8 {
        match self {
            StreetClass::Lane => 0,
            StreetClass::Road => 1,
            StreetClass::Dual => 2,
            StreetClass::Motorway => 3,
        }
    }

    /// How it shows on the town plan.
    pub fn glyph(self) -> char {
        match self {
            StreetClass::Lane => '.',
            StreetClass::Road => '-',
            StreetClass::Dual => '=',
            StreetClass::Motorway => '#',
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            StreetClass::Lane => "local street",
            StreetClass::Road => "collector",
            StreetClass::Dual => "divided arterial",
            StreetClass::Motorway => "freeway",
        }
    }
}

/// **What has to be arranged before something this wide moves**, which is
/// a question about the load and not about any particular road.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Permission {
    /// Ordinary traffic. Nothing to arrange.
    Ordinary,
    /// Over 2.9 m — the police want two clear days' notice.
    Notifiable,
    /// Over 3.5 m — escorted, at a walking pace, on a route surveyed in
    /// advance for bridges and pinch points.
    Escorted,
    /// Over 4.3 m — an order from the highway authority, which takes
    /// weeks. **Paperwork, not physics**, and it is a large part of why a
    /// replacement transformer is not simply driven to the substation.
    SpecialOrder,
}

/// **What a given road says about something of a given width.**
///
/// Two facts that do not collapse into one scale, which is the mistake
/// worth not making: what you must arrange before you set off is a
/// property of the load, and whether anything can get past you is a
/// property of the road. A tank needs an escort on a motorway and on a
/// village lane alike — but on the motorway the traffic still flows, and
/// on the lane everybody behind it waits.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Clearance {
    pub permission: Permission,
    /// Nothing can come the other way while it is here.
    pub blocks_the_road: bool,
    /// It is wider than the running surface. No notice helps.
    pub will_not_fit: bool,
}

impl Clearance {
    pub fn name(self) -> String {
        if self.will_not_fit {
            return "will not physically fit".into();
        }
        let p = match self.permission {
            Permission::Ordinary => "legal load",
            Permission::Notifiable => "oversize permit, per state",
            Permission::Escorted => "permit and pilot cars, daylight only",
            Permission::SpecialOrder => "superload: engineering review, weeks",
        };
        if self.blocks_the_road {
            format!("{p}; nothing gets past it")
        } else {
            p.into()
        }
    }
}

/// **How a settlement is laid out**, which turns on whether it was
/// surveyed at once or simply grew.
///
/// A grid is what you get when an authority lays out land before anybody
/// builds on it: Roman colonies, the Laws of the Indies, the US Land
/// Ordinance, Manhattan's Commissioners' Plan, Barcelona's Eixample. An
/// irregular town is what you get when the routes came first and the
/// buildings followed them.
///
/// Size correlates strongly, which is the useful part: a large city has
/// almost certainly been planned or replanned, because you cannot run
/// water, sewers, trams and freight through a medieval tangle. A village
/// never needed to be.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Pattern {
    /// **One street**, buildings either side, fields behind. The commonest
    /// village form there is, and it is not a grid in any sense.
    Linear,
    /// Routes meeting, and lanes wandering off them. Nothing was surveyed,
    /// so the lanes do not line up and most of them go nowhere.
    Organic,
    /// Laid out at once by somebody with a ruler, and **anisotropic**:
    /// Manhattan's blocks are 80 m by 274, so the avenues are three and a
    /// half times further apart than the streets.
    Grid,
}

impl Pattern {
    /// **Real thresholds** *(UK convention: a village is under ~2,500; a
    /// place is a town to about 100,000)*. Grid planning is what
    /// nineteenth-century city expansion did almost everywhere.
    pub fn for_population(population: f64) -> Pattern {
        if population < 2_500.0 {
            Pattern::Linear
        } else if population < 100_000.0 {
            Pattern::Organic
        } else {
            Pattern::Grid
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Pattern::Linear => "a single street",
            Pattern::Organic => "grown, not planned",
            Pattern::Grid => "laid out on a grid",
        }
    }
}

/// What stands on one plot./// What stands on one plot.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Lot {
    /// Nothing built: fields, scrub, whatever the region says is there.
    Open,
    /// Made road. The thing everything else is arranged along.
    Street,
    /// Housing. Most of a town is this and it is easy to forget.
    House,
    /// **Flats.** The same plot with four floors on it.
    ///
    /// A city centre is not dense because the plots are smaller; it is
    /// dense because it is built upward. Houses alone gave a capital of
    /// sixteen million the population density of an American suburb.
    Flats,
    /// A shop, of the sort `building.rs` fits out.
    Shop,
    /// A works: mill, cannery, depot.
    Works,
    /// Somewhere nobody built, on purpose.
    Park,
}

impl Lot {
    /// One character, the way DF and CDDA both do it.
    pub fn glyph(self) -> char {
        match self {
            Lot::Open => '.',
            Lot::Street => '#',
            Lot::House => 'h',
            Lot::Flats => 'H',
            Lot::Shop => 'S',
            Lot::Works => 'W',
            Lot::Park => ',',
        }
    }
}

/// A square of ground plan around a town centre.
pub struct Plan {
    pub width: usize,
    pub height: usize,
    pub lots: Vec<Lot>,
    /// How big each street is. Empty for anything that is not a street.
    pub classes: Vec<Option<StreetClass>>,
    /// The class of the through-route running north-south down each
    /// column, and east-west along each row. **Kept apart from `classes`
    /// because a junction needs to know which road is which**: where two
    /// streets meet, the bigger one runs through and the lesser one stops.
    pub col_class: Vec<Option<StreetClass>>,
    pub row_class: Vec<Option<StreetClass>>,
    /// How it was laid out, which follows from how big it is.
    pub pattern: Pattern,
    /// **What is under it.** Dig far enough and you are in the rock the
    /// planet put there, which `geology.rs` has known since it was written
    /// and nothing at ground level has ever asked about.
    pub rock: crate::geology::Rock,
    /// **How much the ground rises and falls here**, in metres over a
    /// kilometre. Real: a floodplain is 0-10, rolling country 30-100, and
    /// mountains 300-1,000. Everything the tile layer does vertically is
    /// scaled by this one figure.
    pub relief_m: f64,
    /// Height above the sea, in metres, at the middle of the plan.
    pub elevation_m: f64,
    /// **How far down the water is**, in metres. Decides whether you can
    /// dig a cellar and how deep a well has to be.
    pub water_m: f64,
    /// **The country the town is standing in.**
    ///
    /// A town is not built on a blank sheet: settle in a green zone and
    /// there are trees between the houses and fields beyond the last
    /// street, settle in badlands and there is scrub. The unbuilt plots
    /// are that ground showing through, which is the whole reason the
    /// locality above this exists.
    pub ground: Biome,
}

impl Plan {
    pub fn at(&self, x: usize, y: usize) -> Lot {
        self.lots[y * self.width + x]
    }

    /// Lay out `size` × `size` plots around the centre of a town of
    /// `population`, deterministically from `seed` and the town's `cell`.
    ///
    /// **The street grid comes first and everything else fills in around
    /// it**, which is how towns are actually surveyed and why a planned
    /// town looks planned. Blocks are not uniform: the spacing wanders,
    /// because a perfectly regular grid reads as a spreadsheet rather than
    /// a place.
    pub fn lay_out(seed: u64, cell: usize, population: f64, size: usize) -> Self {
        Plan::lay_out_on(seed, cell, population, size, Biome::Grassland)
    }

    /// The same, on ground of a known kind.
    pub fn lay_out_on(
        seed: u64,
        cell: usize,
        population: f64,
        size: usize,
        ground: Biome,
    ) -> Self {
        let mut rng = Rng::new(seed ^ (cell as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut lots = vec![Lot::Open; size * size];
        let mid = size as f64 / 2.0;

        // **How far the built-up area reaches.**
        //
        // From the population and a real density: a town of a hundred
        // thousand at 4,000 to the square kilometre covers 25 km², which
        // is a touch under 3 km across. Bigger towns spread further, and
        // the falloff means the edge is ragged rather than a wall.
        // **Bigger places are denser, not just wider.** A fixed figure
        // here meant a village and a megacity had the identical central
        // density and differed only in extent, which is the same error as
        // every nation growing 125% of what it ate. Real mean densities:
        // a village 500-2,000/km², a small town 2,000-4,000, Los Angeles
        // 3,200, London 5,700, New York 11,000, Paris 20,000.
        let people_per_km2 = (1_000.0 * (population / 1_000.0).powf(0.21)).clamp(600.0, 12_000.0);
        let area_km2 = population / people_per_km2;
        let radius_km = (area_km2 / std::f64::consts::PI).sqrt();
        let radius_plots = radius_km * 1000.0 / METRES_PER_PLOT;

        // --- Streets ---
        //
        // **What the place looks like depends on whether it was laid out
        // or grew.** All three patterns start the same way, because every
        // settlement that exists is on a route: two through-roads crossing
        // near the middle, which are the roads the town is there for.
        let pattern = Pattern::for_population(population);
        let spine_x = (size / 2).saturating_sub(1 + (rng.next_f32() * 3.0) as usize);
        let spine_y = (size / 2).saturating_sub(1 + (rng.next_f32() * 3.0) as usize);
        for y in 0..size {
            lots[y * size + spine_x] = Lot::Street;
        }
        if pattern != Pattern::Linear {
            for x in 0..size {
                lots[spine_y * size + x] = Lot::Street;
            }
        }

        match pattern {
            // A village is one street. Everything fronts it and the fields
            // start behind the back gardens.
            Pattern::Linear => {}

            // **Lanes that go nowhere.** What makes a place look grown
            // rather than planned is not irregular *spacing* — jittering a
            // grid still reads as a grid — it is that the lanes do not run
            // through. They come off the main road, serve a few houses and
            // stop, and they do not line up with the lane opposite.
            Pattern::Organic => {
                let mut y = 0usize;
                while y < size {
                    // Off one side or the other, never both, and only part
                    // of the way across.
                    let from_left = rng.next_f32() < 0.5;
                    let run = 3 + (rng.next_f32() * (size as f32 * 0.45)) as usize;
                    for k in 0..run {
                        let x = if from_left { k } else { size - 1 - k };
                        if x >= size {
                            break;
                        }
                        lots[y * size + x] = Lot::Street;
                    }
                    y += 3 + (rng.next_f32() * 4.0) as usize;
                }
                let mut x = 0usize;
                while x < size {
                    let from_top = rng.next_f32() < 0.5;
                    let run = 3 + (rng.next_f32() * (size as f32 * 0.45)) as usize;
                    for k in 0..run {
                        let y = if from_top { k } else { size - 1 - k };
                        if y >= size {
                            break;
                        }
                        lots[y * size + x] = Lot::Street;
                    }
                    x += 3 + (rng.next_f32() * 4.0) as usize;
                }
            }

            // **Regular, and anisotropic.** Manhattan's blocks are 80 m by
            // 274, Chicago's about 100 by 200: streets close together one
            // way, avenues far apart the other, because a block wants a
            // long side of frontage and a short walk across. Making them
            // square is the giveaway of a grid nobody measured.
            Pattern::Grid => {
                // **Measured between streets, the block is what is left.**
                // A street every 3 plots leaves *2* built plots — 64 m,
                // under the real 80 m minimum — and puts a third of the
                // town under carriageway against a real 20-25%. It also
                // meant no building in any town could span three plots, so
                // there was nowhere a superstore could physically go.
                //
                // Every 4 gives a 96 m block, which is Chicago's 100 m
                // short side; the long way, every 8 gives 224 m against
                // Manhattan's 274.
                let (close, far) = (4usize, 8usize);
                let (dx, dy) = if rng.next_f32() < 0.5 {
                    (far, close)
                } else {
                    (close, far)
                };
                let mut x = spine_x % dx;
                while x < size {
                    for y in 0..size {
                        lots[y * size + x] = Lot::Street;
                    }
                    x += dx;
                }
                let mut y = spine_y % dy;
                while y < size {
                    for x in 0..size {
                        lots[y * size + x] = Lot::Street;
                    }
                    y += dy;
                }
            }
        }

        // --- What fills the blocks ---
        let radius_m = (radius_plots * METRES_PER_PLOT).max(METRES_PER_PLOT);
        let dear_land = (population / 250_000.0).sqrt().min(1.0);
        for y in 0..size {
            for x in 0..size {
                let i = y * size + x;
                if lots[i] == Lot::Street {
                    continue;
                }
                let dx = x as f64 - mid;
                let dy = y as f64 - mid;
                let d = (dx * dx + dy * dy).sqrt();

                // Clark's law: density decays exponentially with distance
                // from the centre. Real cities follow this closely, which
                // is why the edge of a town is a gradient and not a line.
                let mut built = (-d / radius_plots.max(1.0)).exp();

                // **Everything fronts a road**, because a building needs
                // to be got at. Without this a village of nine hundred was
                // scattered evenly over a square mile with no relation to
                // its own street — Clark's law decays from the centre, and
                // a linear village decays from the road.
                // **A reach, not a decay.** As a decay it thinned every
                // block from the edge inward and left a city of 400,000 at
                // a fifth of the density it should have. What is true is
                // simpler: a plot more than about 90 m from a road cannot
                // be got at, and one within it is ordinary building land.
                if street_distance(&lots, size, x, y) > 3 {
                    continue;
                }
                if rng.next_f32() as f64 > built {
                    continue; // open country, or the gap between suburbs
                }

                // **Trade wants a frontage, and it wants the middle.**
                //
                // Shops face the street because a shop that nobody walks
                // past is not a shop, and they crowd the centre because
                // that is where everybody's walk crosses. Real retail
                // density falls off very fast: a high street is a few
                // hundred metres of almost nothing but shops, and half a
                // kilometre out it is houses with a corner shop every
                // block or two. Sprinkling them evenly gave one shop to
                // every five houses everywhere, which is a bazaar, not a
                // town.
                let d_m = d * METRES_PER_PLOT;
                let on_a_corner = touching_street(&lots, size, x, y);
                let shop_here = 0.80 * (-d_m / 250.0).exp() + 0.006;

                // Works want cheap land and room for lorries, so they are
                // out past the housing on a through road — which is why
                // you do not see a cannery from a town square.
                let works_here = if d_m > 800.0 { 0.12 } else { 0.0 };

                lots[i] = if on_a_corner && (rng.next_f32() as f64) < shop_here {
                    Lot::Shop
                } else if on_a_corner && (rng.next_f32() as f64) < works_here {
                    Lot::Works
                } else if rng.next_f32() < 0.06 {
                    Lot::Park
                } else if (rng.next_f32() as f64) < dear_land * (-d_m / (radius_m * 0.45)).exp() {
                    // **Built upward where land is dear**, which needs both
                    // a central site *and* a place big enough for land to
                    // be worth anything. Land value alone put twenty-one
                    // blocks of flats in a village of nine hundred: nobody
                    // raises four storeys where a field is cheap. Real
                    // apartment blocks are all but absent below about
                    // 20,000 people and dominant over half a million.
                    Lot::Flats
                } else {
                    Lot::House
                };
            }
        }

        // --- A high street is a run of shopfronts ---
        //
        // Each plot drew independently, so a town of four hundred thousand
        // came out with **148 lone shops, 17 pairs and not one run of
        // three** — which is a corner shop on every other block and no
        // building in the town big enough to be a supermarket. One plot is
        // 32 m, and after the frontage and the service yard that is about
        // 810 m²; a real superstore is **2,800-4,650 m²** against a corner
        // shop's 250-1,000.
        //
        // Retail is not sprinkled. A high street is a **continuous terrace
        // of shopfronts** for a few hundred metres, and a superstore is a
        // single box occupying a whole block. Both are runs. Out past the
        // centre a lone shop is exactly right, and stays one.
        //
        // Grown from the head of a run only, and capped at three plots, so
        // it cannot cascade along a whole street.
        for y in 0..size {
            for x in 0..size {
                if lots[y * size + x] != Lot::Shop {
                    continue;
                }
                if x > 0 && lots[y * size + x - 1] == Lot::Shop {
                    continue; // not the head of the run
                }
                let dx = x as f64 - size as f64 / 2.0;
                let dy = y as f64 - size as f64 / 2.0;
                if (dx * dx + dy * dy).sqrt() * METRES_PER_PLOT > 400.0 {
                    continue; // out here it is a corner shop, and stays one
                }
                let mut len = 1;
                while len < SHOP_PLOTS && x + len < size && lots[y * size + x + len] == Lot::Shop {
                    len += 1;
                }
                while len < SHOP_PLOTS && x + len < size {
                    let j = y * size + x + len;
                    // Only over housing, and only where the frontage
                    // continues — a shop still has to be walked past.
                    if !matches!(lots[j], Lot::House | Lot::Flats)
                        || !touching_street(&lots, size, x + len, y)
                    {
                        break;
                    }
                    lots[j] = Lot::Shop;
                    len += 1;
                }
            }
        }

        // **And back off the street, because a store is a block.** The
        // frontage is what sells, so retail grows along the street first;
        // the depth behind it is the sales floor, the backroom and the
        // dock, and none of that needs a window. A big American store is
        // set back in its lot with the parking in front of it.
        for y in 0..size {
            for x in 0..size {
                if lots[y * size + x] != Lot::Shop {
                    continue;
                }
                if y > 0 && lots[(y - 1) * size + x] == Lot::Shop {
                    continue; // not the front of the store
                }
                let mut deep = 1;
                while deep < SHOP_DEEP && y + deep < size && lots[(y + deep) * size + x] == Lot::Shop
                {
                    deep += 1;
                }
                while deep < SHOP_DEEP && y + deep < size {
                    let j = (y + deep) * size + x;
                    if !matches!(lots[j], Lot::House | Lot::Flats) {
                        break;
                    }
                    lots[j] = Lot::Shop;
                    deep += 1;
                }
            }
        }

        // --- How big is each street? ---
        //
        // **A hierarchy, because that is what road networks are.** By
        // length the UK is roughly 1% motorway, 12% A-road and 87% minor,
        // and every town has that shape: a trunk route or two, a handful
        // of distributors, and streets of houses hanging off them. Making
        // them all one width gave a grid of identical strips, which is a
        // housing estate drawn by somebody who has never seen one.
        //
        // Size goes by **rank among the through-routes**, most central
        // first, because the road a town grew along is the one that ends
        // up carrying everything. Ranking rather than measuring off the
        // middle matters: the street grid is deliberately irregular, so
        // "within a plot of centre" found nothing at all on most towns and
        // quietly gave every road the same width again.
        let mid = size as f64 / 2.0;
        let mut lines: Vec<(usize, bool, f64)> = Vec::new(); // (index, is_column, offset)
        for x in 0..size {
            if (0..size).filter(|&y| lots[y * size + x] == Lot::Street).count() > size / 2 {
                lines.push((x, true, (x as f64 - mid).abs()));
            }
        }
        for y in 0..size {
            if (0..size).filter(|&x| lots[y * size + x] == Lot::Street).count() > size / 2 {
                lines.push((y, false, (y as f64 - mid).abs()));
            }
        }
        lines.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)).then(b.1.cmp(&a.1)));

        // **Traffic decides, not rank.** Ranking alone gave an 18,000
        // town a dual carriageway, which is the identical mistake
        // `network.rs` had to unlearn: percentiles force the same
        // hierarchy on every settlement however big or small it is.
        //
        // The busiest street in a place scales with about the square root
        // of its population — a village on a B-road sees a couple of
        // thousand vehicles a day, a town of twenty thousand about eight,
        // a city of half a million forty *(all real)* — and it caps,
        // because no single road carries more than about 120,000.
        let main_vpd = (60.0 * population.sqrt()).min(120_000.0);
        // Traffic is very concentrated: in the UK motorways are 1% of road
        // length and 21% of traffic, A-roads 12% and 44%. Each rank down
        // carries about a third of the one above.
        let of_rank = |r: usize| {
            let vpd = main_vpd * 0.35_f64.powi(r as i32);
            // The same absolute thresholds the road network already uses:
            // a 7.3 m single carriageway wants dualling around 13,000, and
            // below about a thousand a street is residential access.
            if vpd >= 40_000.0 {
                StreetClass::Motorway
            } else if vpd >= 13_000.0 {
                StreetClass::Dual
            } else if vpd >= 1_000.0 {
                StreetClass::Road
            } else {
                StreetClass::Lane
            }
        };

        let mut col = vec![None; size];
        let mut row = vec![None; size];
        for (rank, &(i, is_col, _)) in lines.iter().enumerate() {
            let c = of_rank(rank);
            if is_col {
                col[i] = Some(c);
            } else {
                row[i] = Some(c);
            }
        }

        let mut classes = vec![None; size * size];
        for y in 0..size {
            for x in 0..size {
                if lots[y * size + x] != Lot::Street {
                    continue;
                }
                // At a crossroads the bigger road wins: you do not narrow
                // a trunk route because a lane joins it.
                classes[y * size + x] = match (col[x], row[y]) {
                    (Some(a), Some(b)) => Some(if a.size() >= b.size() { a } else { b }),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    // A short spur off the grid, serving a few houses.
                    (None, None) => Some(StreetClass::Lane),
                };
            }
        }

        Plan {
            width: size,
            height: size,
            lots,
            classes,
            col_class: col,
            row_class: row,
            pattern,
            rock: crate::geology::Rock::Sedimentary,
            relief_m: 20.0,
            elevation_m: 60.0,
            water_m: 12.0,
            ground,
        }
    }

    /// Tell the plan what it is standing on. Callers that have a
    /// generated world can read it off `world.geology.rock[cell]`.
    pub fn on_rock(mut self, rock: crate::geology::Rock) -> Self {
        self.rock = rock;
        self
    }

    /// Tell the plan what the ground does. Callers with a generated world
    /// read it off the elevation field.
    pub fn on_ground(mut self, elevation_m: f64, relief_m: f64) -> Self {
        self.elevation_m = elevation_m;
        self.relief_m = relief_m;
        self
    }

    /// How far down the water is here.
    pub fn with_water_at(mut self, water_m: f64) -> Self {
        self.water_m = water_m;
        self
    }

    /// The through-route running north-south down this column, if any.
    pub fn col_class(&self, x: usize) -> Option<StreetClass> {
        self.col_class.get(x).copied().flatten()
    }

    /// The through-route running east-west along this row, if any.
    pub fn row_class(&self, y: usize) -> Option<StreetClass> {
        self.row_class.get(y).copied().flatten()
    }

    /// How big the street on this plot is, if it is a street.
    pub fn street_class(&self, x: usize, y: usize) -> Option<StreetClass> {
        self.classes[y * self.width + x]
    }

    /// How many of each kind of plot there are.
    pub fn count(&self, lot: Lot) -> usize {
        self.lots.iter().filter(|&&l| l == lot).count()
    }

    /// People the housing here would hold.
    pub fn housed(&self) -> f64 {
        let mut households = self.count(Lot::Flats) as f64 * HOUSEHOLDS_PER_BLOCK;
        for y in 0..self.height {
            for x in 0..self.width {
                if self.at(x, y) != Lot::House {
                    continue;
                }
                households += if self.terraced(x, y) {
                    HOUSEHOLDS_PER_TERRACE
                } else {
                    1.0
                };
            }
        }
        households * HOUSEHOLD
    }

    /// **Whether this plot's building shares walls with its neighbours.**
    ///
    /// Not a type but a consequence: a building whose neighbours along the
    /// street are also built is joined to them, and one whose neighbours
    /// are fields stands alone. Used both for how many households fit and
    /// for how the building sits on the ground.
    pub fn terraced(&self, x: usize, y: usize) -> bool {
        let built = |dx: i64, dy: i64| -> bool {
            let (nx, ny) = (x as i64 + dx, y as i64 + dy);
            nx >= 0
                && ny >= 0
                && (nx as usize) < self.width
                && (ny as usize) < self.height
                && matches!(
                    self.at(nx as usize, ny as usize),
                    Lot::House | Lot::Flats | Lot::Shop
                )
        };
        let street = |dx: i64, dy: i64| -> bool {
            let (nx, ny) = (x as i64 + dx, y as i64 + dy);
            nx >= 0
                && ny >= 0
                && (nx as usize) < self.width
                && (ny as usize) < self.height
                && self.at(nx as usize, ny as usize) == Lot::Street
        };
        // The front faces the street; a terrace runs along it.
        if street(0, -1) || street(0, 1) {
            built(-1, 0) && built(1, 0)
        } else {
            built(0, -1) && built(0, 1)
        }
    }

    /// The plan as text, the way both games draw it.
    pub fn render(&self) -> String {
        self.draw(false)
    }

    /// **The same plan in DF's sixteen colours.**
    ///
    /// A town plan is a wall of letters — `h H S W , ~ b d ; " * f s t u ^
    /// A` — and at four hundred plots across, the shape of a place is not
    /// legible from the letters alone. Hue carries the class (housing,
    /// retail, industry, open country, water, made ground) and brightness
    /// carries the rank within it, which is the same division the ground
    /// view uses one metre at a time.
    pub fn render_in_colour(&self) -> String {
        self.draw(true)
    }

    fn draw(&self, colour: bool) -> String {
        let mut out = String::with_capacity((self.width + 1) * self.height);
        let mut last: Option<Colour> = None;
        for y in 0..self.height {
            for x in 0..self.width {
                let l = self.at(x, y);
                let (glyph, fg) = if l == Lot::Open {
                    (ground_glyph(self.ground), ground_colour(self.ground))
                } else if let Some(c) = self.street_class(x, y) {
                    // The hierarchy has to be visible from up here too, or
                    // the town reads as a uniform grid right up until you
                    // walk down onto it. **Brightness is the traffic**: a
                    // lane is dim and a motorway is white.
                    (c.glyph(), road_colour(c))
                } else {
                    (l.glyph(), lot_colour(l))
                };
                if colour && last != Some(fg) {
                    out.push_str(fg.ansi());
                    last = Some(fg);
                }
                out.push(glyph);
            }
            out.push('\n');
        }
        if colour {
            out.push_str("\x1b[0m");
        }
        out
    }
}

/// **Hue is the class, brightness the rank.** Housing is brown, retail
/// magenta and industry red — the same three the ground view uses for a
/// dwelling, a shop fitting and a works.
pub fn lot_colour(l: Lot) -> Colour {
    match l {
        Lot::House => Colour::Brown,
        Lot::Flats => Colour::Yellow,
        Lot::Shop => Colour::LightMagenta,
        Lot::Works => Colour::LightRed,
        Lot::Park => Colour::Green,
        Lot::Street => Colour::Grey,
        Lot::Open => Colour::DarkGrey,
    }
}

pub fn road_colour(c: StreetClass) -> Colour {
    match c {
        StreetClass::Lane => Colour::DarkGrey,
        StreetClass::Road => Colour::Grey,
        StreetClass::Dual => Colour::White,
        StreetClass::Motorway => Colour::White,
    }
}

/// What the unbuilt country is coloured, which is what it is made of.
pub fn ground_colour(b: Biome) -> Colour {
    use Biome::*;
    match b {
        Ocean | Shallows => Colour::LightBlue,
        Beach => Colour::Yellow,
        Desert => Colour::Yellow,
        Savanna => Colour::Brown,
        Grassland => Colour::Green,
        Shrubland => Colour::Brown,
        Forest | Rainforest => Colour::LightGreen,
        Swamp => Colour::Cyan,
        Taiga => Colour::Green,
        Tundra => Colour::Grey,
        Mountain => Colour::DarkGrey,
        Snowcap => Colour::White,
    }
}

/// How many plots to the nearest street. A building has to be got at, so
/// this is what decides whether a plot can be built on at all.
fn street_distance(lots: &[Lot], size: usize, x: usize, y: usize) -> usize {
    for r in 0..6 {
        for dy in -(r as i64)..=r as i64 {
            for dx in -(r as i64)..=r as i64 {
                if dx.abs().max(dy.abs()) != r as i64 {
                    continue;
                }
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx < 0 || ny < 0 || nx >= size as i64 || ny >= size as i64 {
                    continue;
                }
                if lots[ny as usize * size + nx as usize] == Lot::Street {
                    return r;
                }
            }
        }
    }
    6
}

/// What unbuilt ground looks like, which is whatever country the town was
/// put down in.
pub fn ground_glyph(b: Biome) -> char {
    use Biome::*;
    // **The country must not look like the town built on it.** Desert was
    // `.` and Tundra `-`, which are a lane and a road; Beach was `,`,
    // which is a park. A town in the desert had streets you could not see.
    match b {
        Ocean | Shallows => '~',
        Beach => 'b',
        Desert => 'd',
        Savanna => ';',
        Grassland => '"',
        Shrubland => '*',
        Forest | Rainforest => 'f',
        Swamp => 's',
        Taiga => 't',
        Tundra => 'u',
        Mountain => '^',
        Snowcap => 'A',
    }
}

/// **One legend for the plot view**, so every binary that draws a town
/// says the same thing about it.
pub fn plan_legend(ground: Biome) -> String {
    plan_legend_in(ground, false)
}

/// The same key, painted the way the plan is, so the two can be matched.
pub fn plan_legend_in(ground: Biome, colour: bool) -> String {
    let mut out = String::new();
    let item = |g: char, c: Colour, name: &str| -> String {
        if colour {
            format!("{}{g}{} {name}", c.ansi(), Colour::Grey.ansi())
        } else {
            format!("{g} {name}")
        }
    };
    if colour {
        out.push_str(Colour::Grey.ansi());
    }
    out.push_str("  streets   ");
    for (r, name) in [
        (StreetClass::Lane, "local street"),
        (StreetClass::Road, "collector"),
        (StreetClass::Dual, "divided arterial"),
        (StreetClass::Motorway, "freeway"),
    ] {
        out.push_str(&item(r.glyph(), road_colour(r), name));
        out.push_str("   ");
    }
    out.push_str("\n  built     ");
    for (l, name) in [
        (Lot::House, "houses"),
        (Lot::Flats, "apartments"),
        (Lot::Shop, "store"),
        (Lot::Works, "plant"),
        (Lot::Park, "park"),
    ] {
        out.push_str(&item(l.glyph(), lot_colour(l), name));
        out.push_str("   ");
    }
    out.push_str(&format!(
        "\n  country   {}the {:?} the town stands in",
        item(ground_glyph(ground), ground_colour(ground), ""),
        ground,
    ));
    if colour {
        out.push_str("\x1b[0m");
    }
    out
}

fn touching_street(lots: &[Lot], size: usize, x: usize, y: usize) -> bool {
    let n = [
        (x.wrapping_sub(1), y),
        (x + 1, y),
        (x, y.wrapping_sub(1)),
        (x, y + 1),
    ];
    n.iter().any(|&(nx, ny)| {
        nx < size && ny < size && lots[ny * size + nx] == Lot::Street
    })
}
