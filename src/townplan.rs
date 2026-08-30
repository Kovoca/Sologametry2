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

/// Metres across one plot.
pub const METRES_PER_PLOT: f64 = 25.0;

/// People in a household *(real: 2.3-2.6 across the developed world)*.
pub const HOUSEHOLD: f64 = 2.4;

/// Households on one plot of flats.
///
/// Four storeys with two dwellings a floor is the ordinary European
/// tenement, and it is what makes a centre hold 10,000 to a square
/// kilometre where a street of houses holds 2,500.
pub const HOUSEHOLDS_PER_BLOCK: f64 = 8.0;

/// What stands on one plot.
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

        Plan {
            width: size,
            height: size,
            lots,
            ground,
        }
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
