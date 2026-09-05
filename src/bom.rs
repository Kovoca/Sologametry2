//! **Every physical thing declares what it is made of, all the way down.**
//!
//! The contract, and it is not optional:
//!
//! > Grouping parts in the interface is allowed; omitting them from the
//! > underlying data is not.
//!
//! There is no definition that says only *toaster, 1.8 kg, steel*. It says
//! what the toaster contains — a stainless shell, a mild steel chassis,
//! nickel-chromium elements on mica insulators, copper wiring in polymer,
//! a control assembly, a lever and its springs, polymer feet and steel
//! fasteners — and each of those says what *it* contains, until the
//! recursion bottoms out in materials.
//!
//! **A grouped detail is still not massless.** Fourteen M4 screws may be
//! held as a count with a condition distribution rather than as fourteen
//! world objects, and they still weigh 70 grams and are still steel.
//!
//! What makes this a contract rather than an aspiration is
//! [`validate`]: a definition that has no mass, no bill, a component that
//! does not resolve, no way of coming into existence, or no way of
//! ceasing to, is a **content error** and fails a test.

use crate::item::{Catalogue, DefId, Family, JointMethod};
use crate::material::{Composition, Material, Recovers};

// =====================================================================
// the bill
// =====================================================================

/// One line of a bill of materials.
#[derive(Clone, Debug, PartialEq)]
pub struct BomEntry {
    pub def: DefId,
    pub count: u32,
    /// Declared rather than inferred, so a mismatch with the child's own
    /// mass is a content error somebody can be told about.
    pub kg: f64,
    /// Where in the thing it sits. Free text because it is for a person
    /// reading a teardown, not for the simulation.
    pub placement: &'static str,
    pub held_by: JointMethod,
}

impl BomEntry {
    pub fn new(def: DefId, count: u32, kg: f64, placement: &'static str, held_by: JointMethod)
        -> Self
    {
        BomEntry { def, count, kg, placement, held_by }
    }
}

/// **What a thing contains**, in the six kinds that behave differently
/// when it is taken apart.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bom {
    /// Sub-assemblies and parts that come out identifiable.
    pub components: Vec<BomEntry>,
    /// Material the thing itself was made *of* rather than assembled from:
    /// the shell of a toaster is pressed steel, not a steel component.
    pub bulk: Vec<(Material, f64)>,
    /// Adhesive, solder, weld metal, mortar, thread. Went into the joint.
    pub joints: Vec<(Material, f64)>,
    /// Paint, plating, lacquer.
    pub coatings: Vec<(Material, f64)>,
    /// Grease, oil, coolant, electrolyte.
    pub fluids: Vec<(Material, f64)>,
    /// Small, declared, and **not massless**. Solder on a board, the
    /// magnets in a speaker, the label on a tin.
    pub trace: Vec<(Material, f64)>,
}

impl Bom {
    /// A thing that is simply made of stuff: a board, a sheet, a brick.
    pub fn of_material(comp: &Composition, kg: f64) -> Self {
        Bom { bulk: comp.masses(kg), ..Default::default() }
    }

    pub fn assembled(components: Vec<BomEntry>) -> Self {
        Bom { components, ..Default::default() }
    }

    pub fn with_bulk(mut self, bulk: &[(Material, f64)]) -> Self {
        self.bulk = bulk.to_vec();
        self
    }

    pub fn with_joints(mut self, joints: &[(Material, f64)]) -> Self {
        self.joints = joints.to_vec();
        self
    }

    pub fn with_coatings(mut self, coatings: &[(Material, f64)]) -> Self {
        self.coatings = coatings.to_vec();
        self
    }

    pub fn with_fluids(mut self, fluids: &[(Material, f64)]) -> Self {
        self.fluids = fluids.to_vec();
        self
    }

    pub fn with_trace(mut self, trace: &[(Material, f64)]) -> Self {
        self.trace = trace.to_vec();
        self
    }

    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
            && self.bulk.is_empty()
            && self.joints.is_empty()
            && self.coatings.is_empty()
            && self.fluids.is_empty()
            && self.trace.is_empty()
    }

    /// Everything that is not a sub-assembly, by mass.
    pub fn loose_mass(&self) -> f64 {
        [&self.bulk, &self.joints, &self.coatings, &self.fluids, &self.trace]
            .iter()
            .flat_map(|v| v.iter())
            .map(|p| p.1)
            .sum()
    }

    /// **The total has to reconcile.** Components plus joints plus
    /// coatings plus fluids plus declared trace — and no
    /// "miscellaneous parts" line without a mass on it.
    pub fn declared_mass(&self) -> f64 {
        self.components.iter().map(|c| c.kg).sum::<f64>() + self.loose_mass()
    }

    /// Every material in it, one level down: sub-assemblies contribute
    /// their name and not their contents.
    pub fn shallow_materials(&self, cat: &Catalogue) -> Vec<(Material, f64)> {
        let mut out: Vec<(Material, f64)> = Vec::new();
        let mut add = |m: Material, kg: f64| match out.iter_mut().find(|p| p.0 == m) {
            Some(p) => p.1 += kg,
            None => out.push((m, kg)),
        };
        for c in &self.components {
            if let Some(d) = cat.get(c.def) {
                for (m, kg) in d.materials.masses(c.kg) {
                    add(m, kg);
                }
            }
        }
        for v in [&self.bulk, &self.joints, &self.coatings, &self.fluids, &self.trace] {
            for &(m, kg) in v {
                add(m, kg);
            }
        }
        out
    }
}

// =====================================================================
// where it came from and where it goes
// =====================================================================

/// How a thing that was not manufactured is got.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Acquisition {
    Mining,
    Logging,
    Harvesting,
    Butchery,
    Gathering,
    Extraction,
}

/// **How a thing can come into existence.**
///
/// There is no `craftable = false`. A microprocessor has a real production
/// route — semiconductor-grade silicon, photolithography, a cleanroom,
/// process chemicals, purified water, uninterrupted power and somebody who
/// knows how — and a survivor at a campfire cannot follow it. The
/// impossibility comes from the missing capabilities, not from a flag.
#[derive(Clone, Debug, PartialEq)]
pub enum Origin {
    /// A plan in the recipe book, which somebody can actually run.
    Made { plan: &'static str },
    /// A real industrial process that is not yet written as a plan. Named
    /// rather than denied, so the gap is visible.
    Industrial { needs: &'static [&'static str] },
    Gathered(Acquisition),
    /// It is not made here. Somewhere it is made by one of the above.
    Imported,
}

/// **How a thing stops being that thing.** Several are usually possible,
/// and none of them is "reverses into its ingredients".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndOfLife {
    /// Undo the joints and get components back.
    Disassembly,
    /// Break it up for whatever pieces survive.
    Salvage,
    /// Melt, shred or repulp into feedstock.
    Recycling,
    /// Composting, decay, digestion.
    Biological,
    /// Reaction, neutralisation, controlled destruction.
    Chemical,
    /// Burn it for the heat.
    Combustion,
    /// Landfill or hazardous disposal, which is what is left when nothing
    /// else applies.
    Disposal,
}

/// What the materials and the structure allow, derived rather than
/// asserted — so a definition cannot claim a route its contents rule out.
pub fn plausible_ends(bill: &Bom, materials: &Composition, family: Family) -> Vec<EndOfLife> {
    let mut out = Vec::new();
    if !bill.components.is_empty() {
        out.push(EndOfLife::Disassembly);
        out.push(EndOfLife::Salvage);
    }
    let mut recyclable = false;
    let mut burnable = false;
    let mut reactive = false;
    for &(m, _) in materials.parts() {
        match m.recovers() {
            Recovers::Feedstock | Recovers::Downcycled => recyclable = true,
            Recovers::Fuel => burnable = true,
            Recovers::Nothing => {}
        }
        if matches!(m, Material::Propellant) {
            reactive = true;
        }
    }
    if recyclable {
        out.push(EndOfLife::Recycling);
    }
    if burnable {
        out.push(EndOfLife::Combustion);
    }
    if matches!(family, Family::Foodstuff) {
        out.push(EndOfLife::Biological);
    }
    if reactive || matches!(family, Family::Medicine | Family::Ammunition) {
        out.push(EndOfLife::Chemical);
    }
    // **Always available, and always last.** Whatever else is true of a
    // thing, somebody can put it in a hole in the ground.
    out.push(EndOfLife::Disposal);
    out
}

// =====================================================================
// validation
// =====================================================================

/// A content error. Not a runtime failure — a thing wrong with the data
/// that a test should refuse to let through.
#[derive(Clone, Debug, PartialEq)]
pub enum Flaw {
    NoMass,
    NoDimensions,
    /// The bill is empty. Even a brick says what a brick is.
    NoBill,
    /// It does not add up.
    MassMismatch { declared: f64, bill: f64 },
    /// A line names a component that is not in the catalogue.
    UnknownComponent(DefId),
    /// A line names a component whose own mass disagrees with the line.
    ComponentMassMismatch { child: DefId, line: f64, own: f64 },
    /// Something contains itself, directly or through its children.
    Cycle,
    /// No way it can come into existence.
    NoOrigin,
    /// A plan that is not in the recipe book.
    UnknownPlan(&'static str),
    /// No way it can stop existing.
    NoEndOfLife,
    /// It claims a route its own materials rule out.
    ImplausibleEnd(EndOfLife),
    /// A grouped detail with no mass on it.
    MasslessLine(&'static str),
    /// Taking it apart returns more than it contains.
    RecoversTooMuch { contains: f64, returns: f64 },
    /// A cured, set or reacted joining material came back pristine.
    JoinReturnedPristine(Material),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub what: DefId,
    pub name: &'static str,
    pub flaw: Flaw,
}

/// **Every definition, checked against the contract.**
///
/// `plans` is the list of recipe names that exist, so a declared plan that
/// nobody wrote is caught rather than believed.
pub fn validate(cat: &Catalogue, plans: &[&str]) -> Vec<Finding> {
    let mut out = Vec::new();
    let tolerance = 0.02;

    for d in cat.iter() {
        let say = |flaw: Flaw| Finding { what: d.id, name: d.name, flaw };

        if d.nominal_mass_kg <= 0.0 {
            out.push(say(Flaw::NoMass));
        }
        if d.nominal.longest_m() <= 0.0 {
            out.push(say(Flaw::NoDimensions));
        }
        if d.bill.is_empty() {
            out.push(say(Flaw::NoBill));
            continue;
        }

        // ---- it adds up ------------------------------------------
        let bill = d.bill.declared_mass();
        if d.nominal_mass_kg > 0.0
            && (bill - d.nominal_mass_kg).abs() > d.nominal_mass_kg * tolerance
        {
            out.push(say(Flaw::MassMismatch { declared: d.nominal_mass_kg, bill }));
        }

        // ---- nothing is massless ---------------------------------
        for c in &d.bill.components {
            if c.kg <= 0.0 {
                out.push(say(Flaw::MasslessLine(c.placement)));
            }
            match cat.get(c.def) {
                None => out.push(say(Flaw::UnknownComponent(c.def))),
                Some(child) => {
                    let own = child.nominal_mass_kg * c.count.max(1) as f64;
                    if own > 0.0 && (own - c.kg).abs() > own.max(c.kg) * 0.05 {
                        out.push(say(Flaw::ComponentMassMismatch {
                            child: c.def,
                            line: c.kg,
                            own,
                        }));
                    }
                }
            }
        }
        for (label, v) in [
            ("bulk", &d.bill.bulk),
            ("joints", &d.bill.joints),
            ("coatings", &d.bill.coatings),
            ("fluids", &d.bill.fluids),
            ("trace", &d.bill.trace),
        ] {
            if v.iter().any(|p| p.1 <= 0.0) {
                out.push(say(Flaw::MasslessLine(label)));
            }
        }

        // ---- the tree terminates ---------------------------------
        if contains_itself(cat, d.id, d.id, 0) {
            out.push(say(Flaw::Cycle));
        }

        // ---- it can begin and it can end -------------------------
        if d.origin.is_empty() {
            out.push(say(Flaw::NoOrigin));
        }
        for o in &d.origin {
            if let Origin::Made { plan } = o {
                if !plans.contains(plan) {
                    out.push(say(Flaw::UnknownPlan(plan)));
                }
            }
        }
        if d.end_of_life.is_empty() {
            out.push(say(Flaw::NoEndOfLife));
        }
        let allowed = plausible_ends(&d.bill, &d.materials, d.family);
        for e in &d.end_of_life {
            if !allowed.contains(e) {
                out.push(say(Flaw::ImplausibleEnd(*e)));
            }
        }
    }
    out
}

fn contains_itself(cat: &Catalogue, root: DefId, here: DefId, depth: usize) -> bool {
    if depth > 24 {
        return true;
    }
    let Some(d) = cat.get(here) else { return false };
    d.bill.components.iter().any(|c| {
        c.def == root || contains_itself(cat, root, c.def, depth + 1)
    })
}

/// **The whole tree, flattened to materials.**
///
/// What the interface shows as "motor" is one line; what a deep teardown
/// reaches is copper, laminated steel, ferrite, bearings and insulation,
/// and the information was there the whole time.
pub fn explode(cat: &Catalogue, def: DefId, kg: f64) -> Vec<(Material, f64)> {
    let mut out: Vec<(Material, f64)> = Vec::new();
    walk(cat, def, kg, 0, &mut out);
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    out
}

fn walk(cat: &Catalogue, def: DefId, kg: f64, depth: usize, out: &mut Vec<(Material, f64)>) {
    let Some(d) = cat.get(def) else { return };
    if depth > 24 {
        return;
    }
    // Scale, so that exploding half a thing gives half of everything.
    let scale = if d.nominal_mass_kg > 0.0 { kg / d.nominal_mass_kg } else { 1.0 };
    let add = |m: Material, kg: f64, out: &mut Vec<(Material, f64)>| {
        match out.iter_mut().find(|p| p.0 == m) {
            Some(p) => p.1 += kg,
            None => out.push((m, kg)),
        }
    };
    for v in [&d.bill.bulk, &d.bill.joints, &d.bill.coatings, &d.bill.fluids, &d.bill.trace] {
        for &(m, mkg) in v {
            add(m, mkg * scale, out);
        }
    }
    for c in &d.bill.components {
        walk(cat, c.def, c.kg * scale, depth + 1, out);
    }
}

/// How deep the tree goes under something. One means it is simply made of
/// stuff; five means a drill.
pub fn depth_of(cat: &Catalogue, def: DefId) -> usize {
    fn go(cat: &Catalogue, def: DefId, d: usize) -> usize {
        if d > 24 {
            return d;
        }
        cat.get(def)
            .map(|x| {
                x.bill
                    .components
                    .iter()
                    .map(|c| go(cat, c.def, d + 1))
                    .max()
                    .unwrap_or(d + 1)
            })
            .unwrap_or(d)
    }
    go(cat, def, 0)
}

/// A readable tree, for a teardown screen or a diagnostic.
pub fn tree(cat: &Catalogue, def: DefId, kg: f64, indent: usize, into: &mut String) {
    let Some(d) = cat.get(def) else { return };
    let pad = "  ".repeat(indent);
    into.push_str(&format!("{pad}{} — {:.3} kg\n", d.name, kg));
    let scale = if d.nominal_mass_kg > 0.0 { kg / d.nominal_mass_kg } else { 1.0 };
    for c in &d.bill.components {
        let label = if c.count > 1 { format!(" x{}", c.count) } else { String::new() };
        let pad2 = "  ".repeat(indent + 1);
        if cat.get(c.def).map(|x| x.bill.components.is_empty()).unwrap_or(true) {
            // **A leaf still says what it is made of.** A line reading
            // "heating element" and nothing else is exactly the omission
            // the contract exists to prevent.
            let of = cat
                .get(c.def)
                .map(|x| {
                    x.materials.parts().iter().map(|p| p.0.name()).collect::<Vec<_>>().join(", ")
                })
                .unwrap_or_default();
            into.push_str(&format!(
                "{pad2}{}{label} — {:.3} kg  ({}; {of})\n",
                cat.get(c.def).map(|x| x.name).unwrap_or("?"),
                c.kg * scale,
                c.placement
            ));
        } else {
            tree(cat, c.def, c.kg * scale, indent + 1, into);
        }
    }
    for (label, v) in [
        ("", &d.bill.bulk),
        ("joint ", &d.bill.joints),
        ("coating ", &d.bill.coatings),
        ("fluid ", &d.bill.fluids),
        ("trace ", &d.bill.trace),
    ] {
        for &(m, mkg) in v {
            into.push_str(&format!(
                "{}{label}{} — {:.3} kg\n",
                "  ".repeat(indent + 1),
                m.name(),
                mkg * scale
            ));
        }
    }
}
