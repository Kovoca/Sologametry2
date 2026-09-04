//! **What a thing is made of, and how much of it there is.**
//!
//! Two ideas the economy has been managing without, and which the item
//! layer cannot start without.
//!
//! **A material is not a name.** `econ.rs` moves tonnes of `Steel`, which
//! is the right abstraction for a blast furnace and useless the moment
//! somebody has to decide whether a blade will hold an edge or a bracket
//! will corrode. A material carries density, what it can be recovered as,
//! and how it behaves under the operations `craft.rs` performs.
//!
//! **And a quantity is not a number of charges.** CDDA counts nearly
//! everything in charges, which is a serviceable game abstraction and
//! becomes wrong as soon as production is real: nails are a count, rope is
//! a length, sheet steel has dimensions, flour is a mass, fuel is a volume
//! at a temperature, and electricity is energy. Ten metres of rope is not
//! ten of anything, and cutting it in half gives two ropes rather than
//! destroying five charges.

/// **What a material becomes when it is recovered.**
///
/// The thermodynamic boundary, and the reason a reversible-recipe flag
/// cannot express it: a bolt comes out of an assembly as a bolt, a weld
/// comes out as scrap, and a cake does not come out as flour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recovers {
    /// Melts or repulps back to the same feedstock — metals, glass.
    Feedstock,
    /// Recoverable but worse than it went in: chipboard from timber,
    /// regrind from clean thermoplastic, hardcore from concrete.
    Downcycled,
    /// Only its energy is left. Wood scrap, thread, most textiles.
    Fuel,
    /// Cured, cooked, reacted or set. Nothing comes back.
    Nothing,
}

/// A real material, with the figures that decide what can be done to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Material {
    MildSteel,
    ToolSteel,
    Stainless,
    Aluminium,
    Copper,
    Brass,
    Lead,
    Oak,
    Pine,
    Plywood,
    Particleboard,
    Glass,
    Cotton,
    Wool,
    Polyester,
    Polyethylene,
    Abs,
    Rubber,
    Leather,
    Concrete,
    Brick,
    Mortar,
    Ceramic,
    Paperboard,
    Silicon,
    /// Smokeless powder. Its own material because loading it is a process
    /// with real hazards, not a generic component.
    Propellant,
    /// PVA, epoxy, contact adhesive. Cures; does not come back.
    Adhesive,
    Solder,
    Thread,
    Paint,
    /// Wheat flour, standing in for the foodstuffs `econ.rs` already moves
    /// in bulk, so a loaf has something to be made of.
    Flour,
    Water,
}

impl Material {
    /// kg per cubic metre. Real figures, because volume and mass are both
    /// asked for and deriving one from the other by guesswork is how a
    /// wooden chair comes to weigh what a steel one does.
    pub fn density(self) -> f64 {
        use Material::*;
        match self {
            MildSteel | ToolSteel => 7850.0,
            Stainless => 8000.0,
            Aluminium => 2700.0,
            Copper => 8960.0,
            Brass => 8500.0,
            Lead => 11340.0,
            Oak => 750.0,
            Pine => 500.0,
            Plywood => 600.0,
            Particleboard => 650.0,
            Glass => 2500.0,
            Cotton => 1540.0,
            Wool => 1310.0,
            Polyester => 1380.0,
            Polyethylene => 950.0,
            Abs => 1050.0,
            Rubber => 1200.0,
            Leather => 860.0,
            Concrete => 2400.0,
            Brick => 1900.0,
            Mortar => 2000.0,
            Ceramic => 2300.0,
            Paperboard => 700.0,
            Silicon => 2330.0,
            Propellant => 1600.0,
            Adhesive => 1100.0,
            Solder => 8500.0,
            Thread => 1380.0,
            Paint => 1300.0,
            Flour => 590.0,
            Water => 1000.0,
        }
    }

    /// **What is left after taking something apart.**
    pub fn recovers(self) -> Recovers {
        use Material::*;
        match self {
            MildSteel | ToolSteel | Stainless | Aluminium | Copper | Brass | Lead | Solder
            | Glass => Recovers::Feedstock,
            Polyethylene | Abs | Polyester | Paperboard | Concrete | Brick | Silicon => {
                Recovers::Downcycled
            }
            Oak | Pine | Plywood | Particleboard | Cotton | Wool | Leather | Thread | Rubber => {
                Recovers::Fuel
            }
            // Cured, set, cooked or reacted.
            Adhesive | Paint | Mortar | Ceramic | Propellant | Flour | Water => Recovers::Nothing,
        }
    }

    /// What a cut takes out of it. A saw removes a slice of steel; scissors
    /// remove nothing from cloth, which is why fabric is nested and metal
    /// is not.
    pub fn kerf_mm(self) -> f64 {
        use Material::*;
        match self {
            MildSteel | ToolSteel | Stainless | Aluminium | Copper | Brass | Lead => 2.0,
            Oak | Pine | Plywood | Particleboard => 3.2,
            Glass | Ceramic | Concrete | Brick => 4.0,
            _ => 0.0,
        }
    }

    pub fn name(self) -> &'static str {
        use Material::*;
        match self {
            MildSteel => "mild steel",
            ToolSteel => "tool steel",
            Stainless => "stainless steel",
            Aluminium => "aluminium",
            Copper => "copper",
            Brass => "brass",
            Lead => "lead",
            Oak => "oak",
            Pine => "pine",
            Plywood => "plywood",
            Particleboard => "particleboard",
            Glass => "glass",
            Cotton => "cotton",
            Wool => "wool",
            Polyester => "polyester",
            Polyethylene => "polyethylene",
            Abs => "ABS",
            Rubber => "rubber",
            Leather => "leather",
            Concrete => "concrete",
            Brick => "brick",
            Mortar => "mortar",
            Ceramic => "ceramic",
            Paperboard => "paperboard",
            Silicon => "silicon",
            Propellant => "propellant",
            Adhesive => "adhesive",
            Solder => "solder",
            Thread => "thread",
            Paint => "paint",
            Flour => "flour",
            Water => "water",
        }
    }
}

/// **What a thing is made of, by mass.**
///
/// Proportions rather than a single material, because almost nothing real
/// is one substance: a drill is steel and ABS and copper, and which of
/// those it is mostly made of decides what salvaging it returns.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Composition {
    parts: Vec<(Material, f64)>,
}

impl Composition {
    pub fn of(parts: &[(Material, f64)]) -> Self {
        let mut c = Composition { parts: parts.to_vec() };
        c.normalise();
        c
    }

    pub fn pure(m: Material) -> Self {
        Composition { parts: vec![(m, 1.0)] }
    }

    fn normalise(&mut self) {
        let total: f64 = self.parts.iter().map(|p| p.1).sum();
        if total > 0.0 {
            for p in &mut self.parts {
                p.1 /= total;
            }
        }
        // Deterministic order, so two compositions built in different
        // orders are the same composition.
        self.parts.sort_by(|a, b| a.0.cmp(&b.0));
        self.parts.retain(|p| p.1 > 0.0);
    }

    pub fn parts(&self) -> &[(Material, f64)] {
        &self.parts
    }

    pub fn fraction_of(&self, m: Material) -> f64 {
        self.parts.iter().find(|p| p.0 == m).map(|p| p.1).unwrap_or(0.0)
    }

    /// The material it is mostly made of, which is what a coarse salvage
    /// keys off.
    pub fn chiefly(&self) -> Option<Material> {
        self.parts
            .iter()
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
            .map(|p| p.0)
    }

    /// Split a mass into the masses of each constituent.
    pub fn masses(&self, total_kg: f64) -> Vec<(Material, f64)> {
        self.parts.iter().map(|&(m, f)| (m, f * total_kg)).collect()
    }

    /// Mean density, which is what turns a volume into a mass.
    pub fn density(&self) -> f64 {
        // Volume-weighted: 1/rho = sum(mass fraction / rho_i).
        let inv: f64 = self.parts.iter().map(|&(m, f)| f / m.density()).sum();
        if inv > 0.0 {
            1.0 / inv
        } else {
            0.0
        }
    }

    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Combine two compositions weighted by their masses — what happens
    /// when an assembly's bill of materials is added up.
    pub fn blend(a: (&Composition, f64), b: (&Composition, f64)) -> Composition {
        let mut parts: Vec<(Material, f64)> = Vec::new();
        for (c, kg) in [a, b] {
            for &(m, f) in c.parts() {
                match parts.iter_mut().find(|p| p.0 == m) {
                    Some(p) => p.1 += f * kg,
                    None => parts.push((m, f * kg)),
                }
            }
        }
        Composition::of(&parts)
    }
}

/// Length, width and height in metres. The longest dimension decides
/// whether a thing goes through a door, onto a shelf or into a van, which
/// is why it is kept rather than derived from a volume.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Dims {
    pub length_m: f64,
    pub width_m: f64,
    pub height_m: f64,
}

impl Dims {
    pub fn new(length_m: f64, width_m: f64, height_m: f64) -> Self {
        Dims { length_m, width_m, height_m }
    }

    pub fn litres(self) -> f64 {
        self.length_m * self.width_m * self.height_m * 1000.0
    }

    pub fn longest_m(self) -> f64 {
        self.length_m.max(self.width_m).max(self.height_m)
    }
}

/// **How much there is, in the unit the thing is actually measured in.**
///
/// The alternative — one `charges: u32` on everything — cannot say that
/// half a rope is two ropes, that a sheet has a size as well as a mass, or
/// that fuel has a temperature. Every variant carries a mass, because the
/// balance in `craft.rs` has to close whatever is being counted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Quantity {
    /// Nails, cartridges, screws, whole objects.
    Count(u32),
    /// Rope, cable, wire, moulding.
    Length { metres: f64, kg: f64 },
    /// Fabric, sheet goods sold by area, leather.
    Area { m2: f64, kg: f64 },
    /// Flour, sand, scrap, cement.
    Mass { kg: f64 },
    /// Water, fuel, solvent. Temperature is carried because it decides
    /// whether the thing is usable and how much of it evaporates.
    Fluid { litres: f64, kg: f64, celsius: f64 },
    /// A charged battery, a receiver of compressed air.
    Energy { kwh: f64 },
    /// Sheet metal and lumber: so many pieces, each of a size.
    Stock { count: u32, each: Dims, kg: f64 },
}

impl Quantity {
    /// **Everything has a mass.** The conservation check in `craft.rs`
    /// rests on it, so there is no variant without one.
    pub fn mass_kg(self, unit_mass_kg: f64) -> f64 {
        match self {
            Quantity::Count(n) => n as f64 * unit_mass_kg,
            Quantity::Length { kg, .. }
            | Quantity::Area { kg, .. }
            | Quantity::Mass { kg }
            | Quantity::Fluid { kg, .. }
            | Quantity::Stock { kg, .. } => kg,
            // Energy has no rest mass worth counting. It is tracked
            // separately and deliberately stays out of the mass balance.
            Quantity::Energy { .. } => 0.0,
        }
    }

    /// Whether this is a thing you can have a fraction of.
    pub fn divisible(self) -> bool {
        !matches!(self, Quantity::Count(_) | Quantity::Stock { .. })
    }

    /// **Two stacks merge only if what is being thrown away does not
    /// matter.** Fluids at different temperatures, stock of different
    /// sizes and counts of different things are not one stack — and
    /// silently merging them is how ammunition of two loadings becomes an
    /// indistinguishable pile.
    pub fn mergeable_with(self, other: Quantity) -> bool {
        match (self, other) {
            (Quantity::Count(_), Quantity::Count(_)) => true,
            (Quantity::Length { .. }, Quantity::Length { .. }) => true,
            (Quantity::Area { .. }, Quantity::Area { .. }) => true,
            (Quantity::Mass { .. }, Quantity::Mass { .. }) => true,
            (Quantity::Energy { .. }, Quantity::Energy { .. }) => true,
            (Quantity::Fluid { celsius: a, .. }, Quantity::Fluid { celsius: b, .. }) => {
                (a - b).abs() < 5.0
            }
            (Quantity::Stock { each: a, .. }, Quantity::Stock { each: b, .. }) => a == b,
            _ => false,
        }
    }

    /// Add two like quantities. Fluids mix to a mass-weighted temperature,
    /// which is the whole reason temperature sits on the quantity.
    pub fn merged(self, other: Quantity) -> Option<Quantity> {
        if !self.mergeable_with(other) {
            return None;
        }
        Some(match (self, other) {
            (Quantity::Count(a), Quantity::Count(b)) => Quantity::Count(a + b),
            (Quantity::Length { metres: a, kg: ka }, Quantity::Length { metres: b, kg: kb }) => {
                Quantity::Length { metres: a + b, kg: ka + kb }
            }
            (Quantity::Area { m2: a, kg: ka }, Quantity::Area { m2: b, kg: kb }) => {
                Quantity::Area { m2: a + b, kg: ka + kb }
            }
            (Quantity::Mass { kg: a }, Quantity::Mass { kg: b }) => Quantity::Mass { kg: a + b },
            (Quantity::Energy { kwh: a }, Quantity::Energy { kwh: b }) => {
                Quantity::Energy { kwh: a + b }
            }
            (
                Quantity::Fluid { litres: la, kg: ka, celsius: ca },
                Quantity::Fluid { litres: lb, kg: kb, celsius: cb },
            ) => {
                let kg = ka + kb;
                let celsius = if kg > 0.0 { (ca * ka + cb * kb) / kg } else { ca };
                Quantity::Fluid { litres: la + lb, kg, celsius }
            }
            (
                Quantity::Stock { count: a, each, kg: ka },
                Quantity::Stock { count: b, kg: kb, .. },
            ) => Quantity::Stock { count: a + b, each, kg: ka + kb },
            _ => return None,
        })
    }
}
