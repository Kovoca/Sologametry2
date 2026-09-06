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
    /// **Pressed, drawn, cast or machined pieces that are not separately
    /// traded components and are not anonymous stuff either.** A door
    /// skin, an appliance panel, a bracket. Separated carefully they are
    /// still that shape; cut or crushed they become the material.
    pub formed: Vec<Formed>,
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

    pub fn with_formed(mut self, formed: &[Formed]) -> Self {
        self.formed = formed.to_vec();
        self
    }

    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
            && self.bulk.is_empty()
            && self.joints.is_empty()
            && self.coatings.is_empty()
            && self.fluids.is_empty()
            && self.trace.is_empty()
            && self.formed.is_empty()
    }

    pub fn formed_mass(&self) -> f64 {
        self.formed.iter().map(|f| f.kg).sum()
    }

    /// Everything that is not a sub-assembly and not a formed part, by
    /// mass. Genuine stuff: sealer, adhesive, paint, grease, trace.
    pub fn loose_mass(&self) -> f64 {
        [&self.bulk, &self.joints, &self.coatings, &self.fluids, &self.trace]
            .iter()
            .flat_map(|v| v.iter())
            .map(|p| p.1)
            .sum()
    }

    /// **The three ways mass gets into a thing**, which is what a printed
    /// tree has to show if it is to be audited rather than believed.
    pub fn reconcile(&self) -> Reconciliation {
        Reconciliation {
            components: self.components.iter().map(|c| c.kg).sum(),
            formed: self.formed_mass(),
            direct: self.loose_mass(),
        }
    }

    /// **The total has to reconcile.** Components plus joints plus
    /// coatings plus fluids plus declared trace — and no
    /// "miscellaneous parts" line without a mass on it.
    pub fn declared_mass(&self) -> f64 {
        self.components.iter().map(|c| c.kg).sum::<f64>()
            + self.formed_mass()
            + self.loose_mass()
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
        for f in &self.formed {
            add(f.material, f.kg);
        }
        for v in [&self.bulk, &self.joints, &self.coatings, &self.fluids, &self.trace] {
            for &(m, kg) in v {
                add(m, kg);
            }
        }
        out
    }
}

/// **What a node is made of, in the three kinds, so the arithmetic can be
/// checked by looking at it.**
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reconciliation {
    pub components: f64,
    pub formed: f64,
    pub direct: f64,
}

impl Reconciliation {
    pub fn total(&self) -> f64 {
        self.components + self.formed + self.direct
    }

    pub fn residual(&self, declared: f64) -> f64 {
        declared - self.total()
    }
}


// =====================================================================
// a formed part is neither bulk nor a component
// =====================================================================

/// **The rule, and it decides which of the two a thing is.**
///
/// > Bulk has no independently meaningful shape. If its shape matters
/// > after separation, it is a fabricated part.
///
/// Calling a pressed door skin "bulk steel" recreates the problem the
/// whole contract exists to fix: separated, it becomes anonymous sheet
/// rather than a door skin that is bent but still a door skin. It only
/// becomes scrap after somebody cuts, crushes or shreds it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Formed {
    pub name: &'static str,
    /// What it does in the assembly, for a person reading a teardown.
    pub role: &'static str,
    pub material: Material,
    pub kg: f64,
    pub geometry: Geometry,
    pub state: MaterialState,
    pub surface: Surface,
}

impl Formed {
    pub fn new(
        name: &'static str,
        role: &'static str,
        material: Material,
        kg: f64,
        geometry: Geometry,
    ) -> Self {
        Formed {
            name,
            role,
            material,
            kg,
            geometry,
            state: MaterialState::AsRolled,
            surface: Surface::Bare,
        }
    }

    pub fn treated(mut self, state: MaterialState) -> Self {
        self.state = state;
        self
    }

    pub fn finished(mut self, surface: Surface) -> Self {
        self.surface = surface;
        self
    }

    /// **What separating it gives you.** Undone carefully it is still the
    /// part; cut, crushed or shredded it is the material it was pressed
    /// from and nothing more.
    pub fn survives_separation(&self, destructive: bool) -> bool {
        !destructive && self.geometry.holds_its_shape()
    }
}

/// **The shape, because the shape is the point.** NIST's manufacturing
/// information work is explicit that an intermediate has to keep form
/// features and surface properties and not merely a material and a mass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Geometry {
    /// Flat stock of a thickness. Still generic: a sheet is a sheet.
    Sheet { mm: f64 },
    Bar { mm: f64 },
    /// Pressed to a shape. A door skin, an appliance panel.
    Stamping,
    /// Deep-drawn or folded into a body.
    Shell,
    Casting,
    Extrusion,
    /// Cut to a drawing on a machine.
    Machined,
    Woven,
    /// Wound, laid up, or otherwise built from a continuous run.
    Wound,
}

impl Geometry {
    /// Whether the thing has a shape worth keeping. Flat stock does not —
    /// a sheet separated from an assembly is just a sheet again.
    pub fn holds_its_shape(self) -> bool {
        !matches!(self, Geometry::Sheet { .. } | Geometry::Bar { .. })
    }

    pub fn name(self) -> &'static str {
        match self {
            Geometry::Sheet { .. } => "sheet",
            Geometry::Bar { .. } => "bar",
            Geometry::Stamping => "stamping",
            Geometry::Shell => "shell",
            Geometry::Casting => "casting",
            Geometry::Extrusion => "extrusion",
            Geometry::Machined => "machined",
            Geometry::Woven => "woven",
            Geometry::Wound => "wound",
        }
    }
}

/// What has been done to the metal, which decides what it will take next.
/// A quenched-and-tempered bolt cannot be bent cold; annealed sheet can.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialState {
    AsRolled,
    Annealed,
    Normalised,
    WorkHardened,
    QuenchedAndTempered,
    /// Set, and past the point of being reworked.
    Cured,
}

/// What is on the outside, which decides how it corrodes and whether it
/// can be welded or painted again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Bare,
    Galvanised,
    Primed,
    Painted,
    Anodised,
    Plated,
}

// =====================================================================
// where it came from and where it goes
// =====================================================================

/// **A declared mass is an expectation, not a measurement.**
///
/// A washing machine "weighs 70 kg" the way a car does 40 miles to the
/// gallon: real examples vary with configuration, moisture, how much water
/// is still in the pump and which parts have been changed. An instance's
/// mass comes from what is actually in it; this is what a normal one is
/// expected to come to, how far it may reasonably vary, and **where the
/// number came from** — because a figure somebody measured and a figure
/// somebody assumed are not the same kind of fact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NominalMass {
    pub expected: f64,
    pub provenance: MassProvenance,
    /// **How sure anybody is of the authored figure.** A measurement is
    /// firm; a placeholder is a guess. This says nothing about whether
    /// real examples differ from each other.
    pub source_uncertainty: f64,
    /// **How much real examples vary.** A machined part is held to a
    /// thousandth; a timber board varies with moisture and a washing
    /// machine varies with what is in the pump. This says nothing about
    /// whether the authored figure is any good.
    pub manufacturing_variation: f64,
}

impl NominalMass {
    /// The provenance supplies **defaults** for both, and does not
    /// determine either. A measured figure for a thing that genuinely
    /// varies is a firm number about a loose population.
    pub fn of(expected: f64, provenance: MassProvenance) -> Self {
        NominalMass {
            expected,
            provenance,
            source_uncertainty: provenance.usual_uncertainty(),
            manufacturing_variation: 0.03,
        }
    }

    pub fn varying_by(mut self, fraction: f64) -> Self {
        self.manufacturing_variation = fraction;
        self
    }

    pub fn known_to(mut self, fraction: f64) -> Self {
        self.source_uncertainty = fraction;
        self
    }

    /// Whether an actual example of this weight is an ordinary one. This
    /// is the **population** question, and it uses manufacturing
    /// variation.
    pub fn an_ordinary_example(&self, actual: f64) -> bool {
        (actual - self.expected).abs()
            <= self.expected * self.manufacturing_variation + 1e-9
    }

    /// Whether the authored figure could plausibly be this instead. This
    /// is the **authoring** question, and it uses source uncertainty.
    pub fn could_have_been(&self, other: f64) -> bool {
        (other - self.expected).abs() <= self.expected * self.source_uncertainty + 1e-9
    }
}

/// **Numerical rounding, and nothing else.**
///
/// A bill of materials either adds up or it does not. This is the width of
/// a floating-point sum over a few dozen terms, and it must never be used
/// to excuse a bill that is genuinely out — which is what a
/// provenance-scaled tolerance in the validator quietly did.
pub const BALANCE_EPSILON: f64 = 1e-6;

/// Where a number came from is part of the number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MassProvenance {
    /// Somebody put one on a scale.
    Measured,
    /// Off the maker's plate.
    ManufacturerSpecification,
    /// A published figure for things of this kind.
    LiteratureEstimate,
    /// Worked out from what it is made of.
    Inferred,
    /// Chosen so the model has something to work with. **Not a fact**, and
    /// the honest label for most of a young catalogue.
    DesignedPlaceholder,
}

impl MassProvenance {
    /// **How far the authored figure may be out** — a default, and only a
    /// default. It is not how much real examples vary, and it is
    /// emphatically not what the validator will accept in a bill.
    pub fn usual_uncertainty(self) -> f64 {
        match self {
            MassProvenance::Measured => 0.01,
            MassProvenance::ManufacturerSpecification => 0.03,
            MassProvenance::LiteratureEstimate => 0.10,
            MassProvenance::Inferred => 0.05,
            MassProvenance::DesignedPlaceholder => 0.15,
        }
    }
}

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
    /// It was in the world when the world was made, and nobody here can
    /// replace it.
    LegacyStock,
}

impl Origin {
    /// **Whether a local economy can actually produce it.**
    ///
    /// `Industrial` is explicit debt, not a portal: it names a real
    /// process that nobody has written a plan for, so a thing whose only
    /// route is industrial can be found, imported, salvaged or held as
    /// world-generation stock — and **cannot be manufactured here**. Once
    /// the existing stock is gone it stays gone until the chain exists.
    pub fn can_be_made_locally(&self) -> bool {
        matches!(self, Origin::Made { .. } | Origin::Gathered(_))
    }

    pub fn is_debt(&self) -> bool {
        matches!(self, Origin::Industrial { .. })
    }
}

/// **How a thing stops being that thing.** Several are usually possible,
/// and none of them is "reverses into its ingredients".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndOfLife {
    /// Used again as it is, by somebody else.
    Reuse,
    /// Put right and used again.
    Repair,
    /// Stripped, cleaned and put back to a working standard.
    Refurbish,
    /// Taken to pieces and rebuilt to as-new, which is a factory
    /// operation and not a repair.
    Remanufacture,
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
    if !bill.components.is_empty() || !bill.formed.is_empty() {
        out.push(EndOfLife::Disassembly);
        out.push(EndOfLife::Salvage);
        out.push(EndOfLife::Refurbish);
        out.push(EndOfLife::Remanufacture);
    }
    // Anything durable enough to have a second owner can have one, and
    // anything that can be taken apart can be put right.
    if !matches!(family, Family::Foodstuff | Family::Medicine | Family::Ammunition) {
        out.push(EndOfLife::Reuse);
        out.push(EndOfLife::Repair);
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

/// **What is technically possible is not what happens.**
///
/// Five different questions, and collapsing them is how a model comes to
/// believe every washing machine is recycled. Whether the thing *can* be
/// remanufactured is a fact about the object; whether anybody within reach
/// has a plant that does it, whether the law allows it, and whether it is
/// worth anybody's while are three more, and only the last of them
/// decides what actually happens.
#[derive(Clone, Debug, Default)]
pub struct Available {
    /// What plant is within reach: "a foundry", "a remanufacturer".
    pub facilities: Vec<&'static str>,
    /// What the law here permits. Empty means everything.
    pub forbidden: Vec<EndOfLife>,
    /// What each route pays, per kilogram, net of the work.
    pub worth: Vec<(EndOfLife, f64)>,
}

impl Available {
    pub fn can_perform(&self, route: EndOfLife) -> bool {
        match route {
            EndOfLife::Remanufacture => self.facilities.contains(&"a remanufacturer"),
            EndOfLife::Recycling => self.facilities.contains(&"a foundry"),
            EndOfLife::Refurbish => self.facilities.contains(&"a workshop"),
            // Anybody with hands can take a thing apart, burn it or bury
            // it, which is why those are always the fallbacks.
            _ => true,
        }
    }

    pub fn permitted(&self, route: EndOfLife) -> bool {
        !self.forbidden.contains(&route)
    }

    pub fn pays(&self, route: EndOfLife) -> f64 {
        self.worth.iter().find(|w| w.0 == route).map(|w| w.1).unwrap_or(0.0)
    }
}

/// **What actually becomes of it**, given what it is, what state it is in
/// and what is available. The routes are filtered in the order the
/// questions are asked, and the last surviving one is chosen on what it
/// pays — with disposal as the floor, because a hole in the ground is
/// always available.
pub fn what_happens_to_it(
    technically_possible: &[EndOfLife],
    have: &Available,
    condition: f64,
    contamination: f64,
) -> EndOfLife {
    // **Disposal is the floor and it is what happens by default.** A
    // route only displaces it by being worth more than it, which is why a
    // sound machine in a village with no scrap dealer still goes in the
    // ground.
    let mut best = EndOfLife::Disposal;
    let mut best_pay = have.pays(EndOfLife::Disposal);
    for &route in technically_possible {
        if !have.can_perform(route) || !have.permitted(route) {
            continue;
        }
        // A wreck is not reused and a contaminated thing is not refurbished.
        let condition_ok = match route {
            EndOfLife::Reuse => condition > 0.7 && contamination < 0.2,
            EndOfLife::Repair | EndOfLife::Refurbish => condition > 0.25,
            EndOfLife::Remanufacture => condition > 0.1,
            _ => true,
        };
        if !condition_ok {
            continue;
        }
        let pay = have.pays(route);
        if pay > best_pay {
            best_pay = pay;
            best = route;
        }
    }
    best
}

/// **How much of the catalogue anybody can actually make, by what it is
/// for.** A count of definitions says far less than which parts of life
/// are covered: 18 plans out of 134 is one number, and "medical
/// necessities 24%" is the one that tells you something.
pub fn plan_coverage(cat: &Catalogue) -> Vec<(Family, usize, usize)> {
    let families = [
        Family::Foodstuff,
        Family::Clothing,
        Family::Furniture,
        Family::Appliance,
        Family::Tool,
        Family::SparePart,
        Family::Ammunition,
        Family::Firearm,
        Family::Medicine,
        Family::Stock,
        Family::Fastening,
        Family::Machine,
        Family::Container,
    ];
    families
        .iter()
        .map(|&f| {
            let all: Vec<_> = cat.of_family(f).collect();
            let made = all
                .iter()
                .filter(|d| d.origin.iter().any(|o| o.can_be_made_locally()))
                .count();
            (f, made, all.len())
        })
        .filter(|(_, _, n)| *n > 0)
        .collect()
}

/// **When a leaf should stop being a leaf.**
///
/// Not to record how a bolt was forged — manufacturing history is not
/// physical composition, and a bolt body carrying its alloy, its heat
/// treatment and its coating is a perfectly good leaf. Open it only when
/// the inside has a consequence: it fails on its own, it is repaired or
/// replaced on its own, it is recovered on its own, it changes what the
/// thing does, or it makes a crafting decision somebody would think about.
pub fn should_be_opened(cat: &Catalogue, def: DefId) -> Option<&'static str> {
    let d = cat.get(def)?;
    if !d.bill.components.is_empty() {
        return None;
    }
    // A sealed thing whose insides fail, are replaced, are recovered
    // separately or are hazardous is not honestly a leaf.
    let materials: Vec<Material> = d.materials.parts().iter().map(|p| p.0).collect();
    if materials.contains(&Material::Lead) && d.family == Family::SparePart {
        return Some("its cells, casing and electrolyte fail and are recovered separately");
    }
    if materials.contains(&Material::Propellant) && d.nominal_mass_kg > 0.01 {
        return Some("what is inside it is hazardous and handled on its own");
    }
    None
}

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

    for d in cat.iter() {
        // **A bill either adds up or it does not.** How sure anybody is of
        // the declared figure, and how much real examples vary, are
        // different questions and neither of them excuses arithmetic.
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
            && (bill - d.nominal_mass_kg).abs() > BALANCE_EPSILON * d.nominal_mass_kg.max(1.0)
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
        for f in &d.bill.formed {
            if f.kg <= 0.0 {
                out.push(say(Flaw::MasslessLine(f.name)));
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

/// One line of an audited tree.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub depth: usize,
    pub name: &'static str,
    /// What the line says this much of it weighs.
    pub declared: f64,
    /// How that mass is made up. `None` for a leaf, which is simply the
    /// material it is made of.
    pub made_up_of: Option<Reconciliation>,
    pub detail: String,
}

impl Node {
    /// **The number that has to be zero.** A node whose parts do not add
    /// up to what it says it weighs is a content error the printed tree
    /// must show rather than hide.
    pub fn residual(&self) -> f64 {
        self.made_up_of.map(|r| r.residual(self.declared)).unwrap_or(0.0)
    }
}

/// **Walk a thing and account for every gram of it, node by node.**
///
/// The diagnostic is data before it is text, so a gate can check that
/// every printed node reconciles rather than trusting that an internal
/// validator would have caught it.
pub fn audit(cat: &Catalogue, def: DefId, kg: f64) -> Vec<Node> {
    let mut out = Vec::new();
    audit_into(cat, def, kg, 0, "", &mut out);
    out
}

fn audit_into(
    cat: &Catalogue,
    def: DefId,
    kg: f64,
    depth: usize,
    role: &str,
    out: &mut Vec<Node>,
) {
    let Some(d) = cat.get(def) else { return };
    if depth > 24 {
        return;
    }
    let scale = if d.nominal_mass_kg > 0.0 { kg / d.nominal_mass_kg } else { 1.0 };
    // Clamp the floating-point dust, or a tree prints "-0.000" and a
    // reader has to wonder what it means.
    let tidy = |x: f64| if x.abs() < 5e-7 { 0.0 } else { x };
    let r = Reconciliation {
        components: tidy(d.bill.components.iter().map(|c| c.kg * scale).sum()),
        formed: tidy(d.bill.formed_mass() * scale),
        direct: tidy(d.bill.loose_mass() * scale),
    };
    let leaf = d.bill.components.is_empty() && d.bill.formed.is_empty();
    out.push(Node {
        depth,
        name: d.name,
        declared: kg,
        made_up_of: if leaf { None } else { Some(r) },
        detail: if leaf {
            let of: Vec<&str> = d.materials.parts().iter().map(|p| p.0.name()).collect();
            if role.is_empty() {
                of.join(", ")
            } else {
                format!("{role}; {}", of.join(", "))
            }
        } else {
            role.to_string()
        },
    });
    if leaf {
        return;
    }
    for c in &d.bill.components {
        let each = if c.count > 0 { c.kg / c.count as f64 } else { c.kg };
        // **Unambiguous**: three jaws of fifteen grams, not three jaws of
        // forty-five. A count and a total that could be read either way
        // is a report nobody can audit.
        let role = format!("{} x {:.3} kg = {:.3} kg [{}]", c.count, each * scale,
                           c.kg * scale, c.placement);
        audit_into(cat, c.def, c.kg * scale, depth + 1, &role, out);
    }
    for f in &d.bill.formed {
        out.push(Node {
            depth: depth + 1,
            name: f.name,
            declared: f.kg * scale,
            made_up_of: None,
            detail: format!(
                "formed {} of {}, {:?}, {:?} [{}]",
                f.geometry.name(),
                f.material.name(),
                f.state,
                f.surface,
                f.role
            ),
        });
    }
    for (label, v) in [
        ("bulk", &d.bill.bulk),
        ("joint", &d.bill.joints),
        ("coating", &d.bill.coatings),
        ("fluid", &d.bill.fluids),
        ("trace", &d.bill.trace),
    ] {
        for &(m, mkg) in v {
            out.push(Node {
                depth: depth + 1,
                name: m.name(),
                declared: mkg * scale,
                made_up_of: None,
                detail: format!("direct {label}"),
            });
        }
    }
}

/// The same walk, rendered. **Every node shows its own arithmetic**, so a
/// reader can add the children up and get the parent.
pub fn tree(cat: &Catalogue, def: DefId, kg: f64, indent: usize, into: &mut String) {
    for n in audit(cat, def, kg) {
        let pad = "  ".repeat(indent + n.depth);
        match n.made_up_of {
            Some(r) => into.push_str(&format!(
                "{pad}{}: declared {:.3} kg = parts {:.3} + formed {:.3} + direct {:.3} \
                 (residual {:+.3}){}\n",
                n.name,
                n.declared,
                r.components,
                r.formed,
                r.direct,
                n.residual(),
                if n.detail.is_empty() { String::new() } else { format!("  {}", n.detail) },
            )),
            None => into.push_str(&format!(
                "{pad}{}: {:.3} kg  ({})\n",
                n.name, n.declared, n.detail
            )),
        }
    }
}
