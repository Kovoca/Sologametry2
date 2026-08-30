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

use crate::rng::Rng;
use crate::world::Biome;

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
        let permission = if width_m > 4.3 {
            Permission::SpecialOrder
        } else if width_m > 3.5 {
            Permission::Escorted
        } else if width_m > 2.9 {
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
            StreetClass::Lane => "lane",
            StreetClass::Road => "road",
            StreetClass::Dual => "dual carriageway",
            StreetClass::Motorway => "motorway",
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
            Permission::Ordinary => "ordinary traffic",
            Permission::Notifiable => "notifiable: two days' notice to the police",
            Permission::Escorted => "escorted, at walking pace, on a surveyed route",
            Permission::SpecialOrder => "an order from the highway authority: weeks",
        };
        if self.blocks_the_road {
            format!("{p}; nothing gets past it")
        } else {
            p.into()
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
        const PEOPLE_PER_KM2: f64 = 4_000.0;
        let area_km2 = population / PEOPLE_PER_KM2;
        let radius_km = (area_km2 / std::f64::consts::PI).sqrt();
        let radius_plots = radius_km * 1000.0 / METRES_PER_PLOT;

        // --- Streets ---
        //
        // A grid of through-roads with wandering spacing, plus the two
        // main streets that cross at the centre and are what the town grew
        // along in the first place.
        let mut x = 0usize;
        while x < size {
            for y in 0..size {
                lots[y * size + x] = Lot::Street;
            }
            // Blocks of 4-8 plots: 100-200 m, which is what a real street
            // grid runs at.
            x += 4 + (rng.next_f32() * 5.0) as usize;
        }
        let mut y = 0usize;
        while y < size {
            for x in 0..size {
                lots[y * size + x] = Lot::Street;
            }
            y += 4 + (rng.next_f32() * 5.0) as usize;
        }

        // --- What fills the blocks ---
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
                let built = (-d / radius_plots.max(1.0)).exp();
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
                } else if (rng.next_f32() as f64) < (-d_m / 600.0).exp() {
                    // Built upward where land is dear, which is the middle.
                    Lot::Flats
                } else {
                    Lot::House
                };
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

        let of_rank = |r: usize| match r {
            // A trunk route only goes through somewhere big enough to be
            // worth going through.
            0 if population > 500_000.0 => StreetClass::Motorway,
            0 | 1 => StreetClass::Dual,
            2 | 3 => StreetClass::Road,
            _ => StreetClass::Lane,
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
            ground,
        }
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
        (self.count(Lot::House) as f64
            + self.count(Lot::Flats) as f64 * HOUSEHOLDS_PER_BLOCK)
            * HOUSEHOLD
    }

    /// The plan as text, the way both games draw it.
    pub fn render(&self) -> String {
        let mut out = String::with_capacity((self.width + 1) * self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                let l = self.at(x, y);
                out.push(if l == Lot::Open {
                    ground_glyph(self.ground)
                } else if let Some(c) = self.street_class(x, y) {
                    // The hierarchy has to be visible from up here too, or
                    // the town reads as a uniform grid right up until you
                    // walk down onto it.
                    c.glyph()
                } else {
                    l.glyph()
                });
            }
            out.push('\n');
        }
        out
    }
}

/// What unbuilt ground looks like, which is whatever country the town was
/// put down in.
pub fn ground_glyph(b: Biome) -> char {
    use Biome::*;
    match b {
        Ocean | Shallows => '~',
        Beach => ',',
        Desert => '.',
        Savanna => ';',
        Grassland => '"',
        Shrubland => '*',
        Forest | Rainforest => 'f',
        Swamp => 's',
        Taiga => 't',
        Tundra => '-',
        Mountain => '^',
        Snowcap => 'A',
    }
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
