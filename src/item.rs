//! **A definition is what a thing generally is; an instance is this one.**
//!
//! The rule the whole layer turns on, and the reason it comes before the
//! household basket rather than after it:
//!
//! > A recipe describes one way to make something. The resulting item
//! > records what was actually made, from what, by whom, and in what
//! > condition.
//!
//! `econ.rs` moves tonnes of `RetailGoods`, which says nothing about what
//! a country can make and what it must buy. A household does not buy a
//! tonne of goods; it buys a coat, a kettle, a chair and a box of screws,
//! each of which has a maker, a bill of materials and a service life. Wire
//! the basket to the vague commodity and every one of those becomes a
//! rewrite later.
//!
//! **Quality and condition are separate, and that is not a nicety.** A
//! beautifully made knife can be badly worn; a badly made knife can be
//! brand new; sharpening the worn one does not make it well made, and
//! replacing a chair's broken leg does not straighten a warped seat. One
//! `quality: 0.73` cannot say any of it.

use crate::bom::{
    plausible_ends, Acquisition, Bom, BomEntry, EndOfLife, Formed, Geometry, MassProvenance,
    MaterialState, NominalMass, Origin, Surface,
};
use crate::id::{Arena, Id};
use std::collections::BTreeMap;
use crate::material::{Composition, Dims, Material, Quantity};

// =====================================================================
// what kind of thing it is
// =====================================================================

/// **What a household or a works would call it.** The families the basket
/// needs, so that "retail goods" can stop being one number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Family {
    /// Bar, sheet, board, cloth, pellets. What production starts from.
    Stock,
    /// Screws, nails, rivets, glue, thread, welding wire.
    Fastening,
    Tool,
    Machine,
    Furniture,
    Clothing,
    Appliance,
    Ammunition,
    Firearm,
    /// A component that exists to be fitted to something else.
    SparePart,
    Foodstuff,
    Container,
    Medicine,
}

impl Family {
    /// **A durable is not repurchased every tick**, and that distinction
    /// is what the demand model will stand on.
    pub fn lifecycle(self) -> Lifecycle {
        use Family::*;
        match self {
            Foodstuff => Lifecycle::Perishable,
            Medicine => Lifecycle::Consumable,
            Ammunition | Fastening => Lifecycle::Consumable,
            Clothing => Lifecycle::SemiDurable,
            Stock => Lifecycle::Consumable,
            Container => Lifecycle::SemiDurable,
            Tool | Machine | Furniture | Appliance | Firearm | SparePart => Lifecycle::Durable,
        }
    }
}

/// How long an example is expected to last in use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    /// Days to weeks, and it goes off whether or not it is used.
    Perishable,
    /// Used up by using it.
    Consumable,
    /// A few years. Clothes, bedding, cheap containers.
    SemiDurable,
    /// Years to decades, repaired rather than replaced.
    Durable,
}

impl Lifecycle {
    /// Typical service life in days. Real: clothing 2-4 years, a washing
    /// machine 11, furniture decades.
    pub fn typical_life_days(self) -> f64 {
        match self {
            Lifecycle::Perishable => 5.0,
            Lifecycle::Consumable => 1.0,
            Lifecycle::SemiDurable => 1_100.0,
            Lifecycle::Durable => 4_000.0,
        }
    }
}

/// The physical form, which decides how it is handled and stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    Rigid,
    Sheet,
    Bar,
    Fabric,
    Granular,
    Liquid,
    Paste,
    /// Several parts fastened together — the thing an assembly record
    /// belongs to.
    Assembly,
}

// =====================================================================
// what a tool can do
// =====================================================================

/// **An abstract capability, not a named tool.** CDDA's best structural
/// idea: a recipe asks to cut 3 mm of mild steel to a millimetre, and a
/// hacksaw, a bandsaw, an angle grinder, a plasma cutter and a laser all
/// answer — differing in speed, waste, precision, power and noise rather
/// than in whether the job can be done at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Capability {
    CutMetal,
    CutWood,
    CutFabric,
    Drill,
    Grind,
    Turn,
    Mill,
    Forge,
    Cast,
    Weld,
    Solder,
    Sew,
    Fasten,
    Glue,
    HeatTreat,
    Paint,
    Sterilise,
    Measure,
    Strike,
    Bake,
    Chill,
    Mix,
    Press,
}

impl Capability {
    pub fn name(self) -> &'static str {
        use Capability::*;
        match self {
            CutMetal => "cut metal",
            CutWood => "cut wood",
            CutFabric => "cut fabric",
            Drill => "drill",
            Grind => "grind",
            Turn => "turn",
            Mill => "mill",
            Forge => "forge",
            Cast => "cast",
            Weld => "weld",
            Solder => "solder",
            Sew => "sew",
            Fasten => "fasten",
            Glue => "glue",
            HeatTreat => "heat treat",
            Paint => "paint",
            Sterilise => "sterilise",
            Measure => "measure",
            Strike => "strike",
            Bake => "bake",
            Chill => "chill",
            Mix => "mix",
            Press => "press",
        }
    }
}

/// What one tool actually offers of a capability.
///
/// `HAMMER 2` is a useful game abstraction and it cannot say whether the
/// thing delivers enough impact with enough control on a suitable face.
/// These are the terms an operation asks about.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Provides {
    pub capability: Capability,
    /// How much of it: mm of mild steel for a cutter, mm of drill
    /// diameter, degrees for an oven, kg for a press.
    pub capacity: f64,
    /// The tolerance it can hold, in millimetres. Smaller is better.
    pub precision_mm: f64,
    /// Multiplier on an operation's nominal time. A plasma cutter is 8x a
    /// hacksaw and takes far more out of the plate doing it.
    pub speed: f64,
    /// Draw while working. Zero for a hand tool, which is exactly why a
    /// blackout stops a factory and not a joiner.
    pub kw: f64,
    /// The largest workpiece it will take, in metres.
    pub max_workpiece_m: f64,
}

impl Provides {
    /// Whether this tool satisfies what an operation asked for.
    pub fn satisfies(&self, want: Capability, capacity: f64, precision_mm: f64) -> bool {
        self.capability == want && self.capacity >= capacity && self.precision_mm <= precision_mm
    }
}

/// **Where something attaches.** A fitting is a shape, not a permission
/// list: anything with an M8 thread goes into an M8 hole.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Fitting {
    ThreadM8,
    ThreadM12,
    /// A drill's or grinder's tool holder.
    Chuck,
    /// Battery pack rails on a cordless tool.
    BatteryRail,
    /// Where a barrel screws into a receiver.
    BarrelThread,
    Magazine,
    /// Bolted engine mount, alternator bracket.
    Bracket,
    /// A pocket sewn on, a patch.
    Stitched,
    /// A wheel onto a hub.
    Hub,
    /// A door or window into a structural opening.
    Opening,
    /// An electrical accessory into a back box.
    BackBox,
}

/// Somewhere a thing can hold other things.
#[derive(Clone, Debug, PartialEq)]
pub struct Pocket {
    pub name: &'static str,
    pub litres: f64,
    pub max_kg: f64,
    /// Which forms it will hold. A toolbox will not hold petrol.
    pub accepts: &'static [Form],
    /// Watertight, so what is inside does not leak or spoil at the outside
    /// rate.
    pub sealed: bool,
}

impl Pocket {
    pub fn accepts_form(&self, f: Form) -> bool {
        self.accepts.contains(&f)
    }
}

// =====================================================================
// how well it was made, and what has happened to it since
// =====================================================================

/// **How well it was made.** Fixed at manufacture and not improved by
/// repair — which is what makes a cheap chair a cheap chair for ever.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quality {
    pub workmanship: f64,
    pub structural_integrity: f64,
    pub dimensional_accuracy: f64,
    pub finish: f64,
    /// How much of what should not be in it, is. Zero is clean.
    pub contamination: f64,
    pub domain: DomainQuality,
}

impl Default for Quality {
    fn default() -> Self {
        Quality {
            workmanship: 0.6,
            structural_integrity: 0.8,
            dimensional_accuracy: 0.8,
            finish: 0.6,
            contamination: 0.0,
            domain: DomainQuality::None,
        }
    }
}

impl Quality {
    /// A single figure for a shelf label. The simulation keeps the
    /// dimensions; only the presentation collapses them.
    pub fn overall(&self) -> f64 {
        let core = (self.workmanship
            + self.structural_integrity
            + self.dimensional_accuracy
            + self.finish)
            / 4.0;
        (core * (1.0 - self.contamination)).clamp(0.0, 1.0)
    }

    pub fn with(mut self, domain: DomainQuality) -> Self {
        self.domain = domain;
        self
    }
}

/// **The dimensions only some things have.** A rifle has headspace and a
/// loaf does not; collapsing them into the common core loses exactly the
/// properties somebody would buy the thing for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DomainQuality {
    None,
    Blade { edge: f64, hardness: f64, toughness: f64 },
    Firearm { headspace: f64, alignment: f64, bore: f64 },
    Clothing { fit: f64, seams: f64, waterproof: f64 },
    Food { doneness: f64, taste: f64, safety: f64 },
    Electronics { connections: f64, calibration: f64 },
    Engine { clearances: f64, compression: f64, balance: f64 },
    Medicine { purity: f64, dose_accuracy: f64, sterility: f64 },
}

/// **What has happened to it since.** Every one of these can be changed by
/// use, weather or repair; none of them is how well it was made.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Condition {
    /// Ordinary use. Sharpening, servicing and replacement parts fix it.
    pub wear: f64,
    /// Something broke it. Repair fixes it, imperfectly.
    pub damage: f64,
    pub corrosion: f64,
    /// Residue from working: swarf, powder fouling, grease.
    pub fouling: f64,
    /// Dirt, and in food or medicine the thing that makes it unsafe.
    pub contamination: f64,
    pub temperature_c: f64,
    pub moisture: f64,
}

impl Condition {
    pub fn fresh() -> Self {
        Condition { temperature_c: 15.0, ..Default::default() }
    }

    /// How much of its function is left, all causes together.
    pub fn serviceability(&self) -> f64 {
        (1.0 - self.wear)
            .min(1.0 - self.damage)
            .min(1.0 - self.corrosion * 0.7)
            .min(1.0 - self.fouling * 0.4)
            .clamp(0.0, 1.0)
    }

    pub fn is_ruined(&self) -> bool {
        self.damage >= 0.999
    }
}

/// A named thing wrong with it. Separate from `damage` because a fault has
/// a cause and a fix, and one item may have several.
#[derive(Clone, Debug, PartialEq)]
pub struct Fault {
    pub what: &'static str,
    pub severity: f64,
    /// Whether the item still does its job at all.
    pub disabling: bool,
    pub since_day: u32,
}

// =====================================================================
// what it is actually made of, joint by joint
// =====================================================================

/// **How two components are held together**, which decides what taking
/// them apart returns. The table is the thermodynamic boundary CDDA's
/// single reversible flag cannot express.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JointMethod {
    Bolted,
    Screwed,
    Clipped,
    Riveted,
    Stitched,
    /// A mechanical interference fit: a crimped case mouth, a swaged
    /// ferrule. It comes apart with the right tool and the parts survive,
    /// which is exactly why handloading exists.
    Crimped,
    Glued,
    Soldered,
    Welded,
    Cast,
    Forged,
    Cooked,
    Reacted,
    /// **Bricks in lime mortar.** Soft, weaker than the brick, and it
    /// comes off with a bolster — which is why reclamation yards exist
    /// and why old brickwork is worth taking down carefully.
    LimeMortared,
    /// **Bricks in cement mortar.** Harder than the brick it holds, so
    /// what gives way is the brick. Cement-based mortar is named in the
    /// reclamation literature as *the* barrier to recovering brick, and
    /// it is also what damages softer historic fabric when somebody
    /// repoints with it.
    CementMortared,
}

impl JointMethod {
    /// **What comes back**: the fraction of each joined component
    /// recovered intact by careful disassembly, and whether the joining
    /// material itself survives.
    ///
    /// **These are reference-case figures, not "the recovery rate".** The
    /// case is: a sound example in ordinary condition, taken apart by hand
    /// by somebody competent with the right tools, counting pieces that
    /// come off in one piece. What a real job returns depends on age,
    /// condition, method, tooling and how much anybody is being paid to
    /// care — and `RecoveryGrade` is where "came off intact" and "came off
    /// clean enough to build with" stop being the same number.
    ///
    /// The masonry rows are the ones to read carefully: the reclamation
    /// literature reports separation around 85% for lime-mortared brick
    /// under favourable conditions, and cement-mortared recovery varying
    /// enormously with method. 0.85 and 0.30 here are the *joint*
    /// contribution before care, skill and condition are applied, which is
    /// why a careful hand recovers about 63% and 22% of a wall rather than
    /// those numbers.
    pub fn recovery(self) -> Recovery {
        use JointMethod::*;
        match self {
            Bolted => Recovery { components: 0.98, fastener: 0.95, needs_cutting: false },
            Screwed => Recovery { components: 0.95, fastener: 0.80, needs_cutting: false },
            Clipped => Recovery { components: 0.95, fastener: 0.60, needs_cutting: false },
            Riveted => Recovery { components: 0.90, fastener: 0.0, needs_cutting: true },
            Stitched => Recovery { components: 0.92, fastener: 0.05, needs_cutting: true },
            Crimped => Recovery { components: 0.90, fastener: 0.85, needs_cutting: false },
            Glued => Recovery { components: 0.45, fastener: 0.0, needs_cutting: false },
            Soldered => Recovery { components: 0.85, fastener: 0.30, needs_cutting: false },
            Welded => Recovery { components: 0.55, fastener: 0.0, needs_cutting: true },
            // Real reclamation: lime-mortared brick comes back at a high
            // rate, cement-mortared brick mostly does not — and "mostly"
            // rather than "never", because the techniques exist and are
            // slow rather than impossible.
            LimeMortared => Recovery { components: 0.85, fastener: 0.10, needs_cutting: false },
            CementMortared => Recovery { components: 0.30, fastener: 0.0, needs_cutting: true },
            // Past these there is no assembly to undo. The shape was made,
            // not joined, so what you get is scrap or nothing.
            Cast => Recovery { components: 0.0, fastener: 0.0, needs_cutting: true },
            Forged => Recovery { components: 0.0, fastener: 0.0, needs_cutting: true },
            Cooked => Recovery { components: 0.0, fastener: 0.0, needs_cutting: false },
            Reacted => Recovery { components: 0.0, fastener: 0.0, needs_cutting: false },
        }
    }

    /// Whether the join can be undone at all, or only broken.
    pub fn reversible(self) -> bool {
        self.recovery().components > 0.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recovery {
    /// Fraction of each joined component that comes back usable.
    pub components: f64,
    /// Fraction of the fastener or joining material that comes back.
    pub fastener: f64,
    /// Whether undoing it destroys material at the joint.
    pub needs_cutting: bool,
}

/// One component as it was actually installed.
#[derive(Clone, Debug, PartialEq)]
pub struct Installed {
    pub definition: DefId,
    pub quantity: Quantity,
    /// **What is really in the thing**, which is not what the definition
    /// says a normal example weighs. A chair holds four fifths of a board,
    /// not a board, and taking it apart must not hand back the offcuts
    /// that went in the bin at the sawbench.
    pub mass_kg: f64,
    /// **And what it is really made of.** The intermediate is named
    /// `chair parts` whether it was cut from oak or from particleboard;
    /// the composition is what tells them apart, and it is the whole
    /// reason a substitution cannot be laundered by an intermediate.
    pub materials: Composition,
    /// What state it was in when it went in — so a chair built from
    /// salvaged timber does not yield new timber.
    pub condition_at_install: Condition,
    pub quality_at_install: Quality,
    /// How this one is held in. **The fastener names the joint**: a screw
    /// unscrews out of a glued frame, because what recovers it is what it
    /// is and not what else was done at the same bench.
    pub held_by: JointMethod,
}

/// A join between two of the installed components.
#[derive(Clone, Debug, PartialEq)]
pub struct Joint {
    pub method: JointMethod,
    pub joins: (usize, usize),
    /// Which fastener, if it is a separate item.
    pub fastener: Option<DefId>,
    /// Whether it can be got at without taking other things off first.
    pub accessible: bool,
}

/// A recorded departure from what the recipe nominally called for.
#[derive(Clone, Debug, PartialEq)]
pub struct Substitution {
    pub wanted: DefId,
    pub used: DefId,
}

/// **What was actually made, from what, by whom.**
///
/// The exploit this closes is the standard one: a recipe accepts oak or
/// particleboard and a fixed uncraft recipe always returns oak. Reading
/// the record instead of the definition means what went in is what comes
/// out — as particleboard, and damaged at that.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AssemblyRecord {
    /// **What the design expects.** A door expects a regulator, a latch
    /// and glass, and that is true of the definition whether or not any
    /// particular door has them in it.
    pub components: Vec<Installed>,
    /// **What is actually in this one.** Real objects, installed inside
    /// it, each still itself — so their mass is theirs and is not counted
    /// again here. A spawned instance has the design bill and no
    /// as-built list; one that was assembled has the as-built list and a
    /// direct account of only the joining material.
    pub as_built: Vec<Id<ItemInstance>>,
    /// **Pressed, drawn and machined pieces.** Not components and not
    /// stuff: a door skin separated carefully is a bent door skin, and it
    /// only becomes sheet after somebody cuts or crushes it.
    pub formed: Vec<crate::bom::Formed>,
    /// **The body of the thing**, as opposed to the parts bolted to it.
    /// A toaster shell is pressed steel and not a steel component, and a
    /// model with nowhere to put that either invents a component or loses
    /// the mass.
    pub bulk: Vec<(Material, f64)>,
    pub joints: Vec<Joint>,
    /// Adhesive, solder, welding wire, thread, coating. Consumed into the
    /// assembly and generally not recoverable.
    pub consumed: Vec<(Material, f64)>,
    pub substitutions: Vec<Substitution>,
    pub maker: Option<u64>,
    pub work_order: Option<u64>,
}

impl AssemblyRecord {
    pub fn total_component_mass(&self) -> f64 {
        self.components.iter().map(|c| c.mass_kg).sum::<f64>()
            + self.formed.iter().map(|f| f.kg).sum::<f64>()
            + self.bulk.iter().map(|c| c.1).sum::<f64>()
            + self.consumed.iter().map(|c| c.1).sum::<f64>()
    }

    /// The materials actually present, weighted by mass — not the ones the
    /// definition says a normal example has.
    pub fn actual_materials(&self) -> Composition {
        let mut parts: Vec<(Material, f64)> = Vec::new();
        let mut add = |m: Material, kg: f64| match parts.iter_mut().find(|p| p.0 == m) {
            Some(p) => p.1 += kg,
            None => parts.push((m, kg)),
        };
        for c in &self.components {
            for (m, mkg) in c.materials.masses(c.mass_kg) {
                add(m, mkg);
            }
        }
        for f in &self.formed {
            add(f.material, f.kg);
        }
        for &(m, kg) in self.bulk.iter().chain(self.consumed.iter()) {
            add(m, kg);
        }
        Composition::of(&parts)
    }
}

// =====================================================================
// where it came from, and whose it is
// =====================================================================

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Provenance {
    /// Which market or works it was made at.
    pub made_at: Option<u32>,
    pub made_on_day: u32,
    /// Named makers matter: a marked piece is not aggregatable.
    pub marked: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ownership {
    /// A person, if anybody in particular owns it.
    pub owner: Option<u64>,
    /// Somebody would notice it gone. Stops it being folded into a lot.
    pub accounted_for: bool,
}

// =====================================================================
// definitions
// =====================================================================

/// A handle into the catalogue. Not a `usize` into whatever vector this
/// happened to be, for the reasons `id.rs` already records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DefId(pub u32);

/// **What a cordless drill, a brick or a rifle generally is.** It does not
/// say that every example is factory-new — that is the instance's job.
#[derive(Clone, Debug)]
pub struct ItemDefinition {
    pub id: DefId,
    pub name: &'static str,
    pub family: Family,
    pub form: Form,
    pub nominal: Dims,
    /// What a normal example is expected to come to. Kept as a bare
    /// number because everything reads it; `mass` says how firm it is.
    pub nominal_mass_kg: f64,
    /// **How firm that figure is, and where it came from.** An instance's
    /// mass comes from what is actually in it; this is only the
    /// expectation, and most of a young catalogue is honestly a designed
    /// placeholder rather than a measurement.
    pub mass: NominalMass,
    pub materials: Composition,
    /// What it offers as a tool. Several can coexist: an angle grinder
    /// cuts and grinds, and a rifle is a firearm and a club.
    pub provides: Vec<Provides>,
    pub pockets: Vec<Pocket>,
    /// Where other things fit onto it.
    pub attachment_points: Vec<Fitting>,
    /// What it fits into, if it is a component.
    pub fits: Option<Fitting>,
    pub lifecycle: Lifecycle,
    /// What it takes to put right.
    pub repair_with: Vec<Material>,
    /// Whether using it uses it up. A drill is not consumed by drilling;
    /// a drill bit wears and welding wire is gone.
    pub consumable: bool,
    /// **What it contains, and it is never optional.** A definition that
    /// says only "toaster, 1.8 kg, steel" is a content error; this says
    /// what the toaster is made of, and each component says what *it* is
    /// made of, until the recursion bottoms out in materials.
    pub bill: Bom,
    /// **How it can come into existence.** There is no `craftable =
    /// false`: a thing nobody here can make still has a real route, and
    /// what stops them is the missing capability rather than a flag.
    pub origin: Vec<Origin>,
    /// **And how it stops.** Usually several, and never "reverses into its
    /// ingredients".
    pub end_of_life: Vec<EndOfLife>,
}

impl ItemDefinition {
    pub fn volume_litres(&self) -> f64 {
        self.nominal.litres()
    }

    /// Whether it is an assembly of named parts or simply made of stuff.
    pub fn is_assembly(&self) -> bool {
        !self.bill.components.is_empty()
    }

    /// **A fastener names the joint it makes.** Which is what lets a screw
    /// come back out of a frame that was also glued: what recovers a
    /// component is what holds *it*, not what else happened at the bench.
    pub fn makes_joint(&self) -> Option<JointMethod> {
        if self.family != Family::Fastening {
            return None;
        }
        Some(match self.name {
            "wood screw" => JointMethod::Screwed,
            "bolt" => JointMethod::Bolted,
            "rivet" => JointMethod::Riveted,
            "wood glue" => JointMethod::Glued,
            "thread reel" => JointMethod::Stitched,
            "welding wire" => JointMethod::Welded,
            "solder" => JointMethod::Soldered,
            _ => return None,
        })
    }

    pub fn capability(&self, c: Capability) -> Option<&Provides> {
        self.provides.iter().find(|p| p.capability == c)
    }
}

/// Every definition the world knows about.
#[derive(Clone, Debug, Default)]
pub struct Catalogue {
    defs: Vec<ItemDefinition>,
    by_name: Vec<(&'static str, DefId)>,
}

impl Catalogue {
    pub fn new() -> Self {
        Catalogue::default()
    }

    pub fn add(&mut self, mut d: ItemDefinition) -> DefId {
        let id = DefId(self.defs.len() as u32);
        d.id = id;
        self.by_name.push((d.name, id));
        self.defs.push(d);
        id
    }

    /// **Add something that is assembled from named parts**, deriving what
    /// it is made of from its bill rather than being told twice. The
    /// children have to be in the catalogue already, which is what makes
    /// the tree finite: it is built from the leaves up.
    pub fn add_built(&mut self, d: ItemDefinition, bill: Bom) -> DefId {
        let id = self.add(d);
        self.set_bill(id, bill);
        id
    }

    /// Give something that is already in the catalogue a real bill —
    /// for the assemblies that were declared before their parts existed.
    pub fn set_bill(&mut self, id: DefId, bill: Bom) {
        let shallow = bill.shallow_materials(self);
        let Some(d) = self.defs.get_mut(id.0 as usize) else { return };
        if !shallow.is_empty() {
            d.materials = Composition::of(&shallow);
        }
        d.end_of_life = plausible_ends(&bill, &d.materials, d.family);
        d.bill = bill;
    }

    /// Replace how it comes into existence.
    pub fn set_origin(&mut self, id: DefId, origin: Vec<Origin>) {
        if let Some(d) = self.defs.get_mut(id.0 as usize) {
            d.origin = origin;
        }
    }

    pub fn get(&self, id: DefId) -> Option<&ItemDefinition> {
        self.defs.get(id.0 as usize)
    }

    /// Say where a mass figure came from, which tightens or loosens what
    /// the validator will accept.
    pub fn set_mass(&mut self, id: DefId, mass: NominalMass) {
        if let Some(d) = self.defs.get_mut(id.0 as usize) {
            d.nominal_mass_kg = mass.expected;
            d.mass = mass;
        }
    }

    /// For content tooling and tests. The catalogue is authored data, so
    /// editing it at run time is a build step rather than a game action.
    pub fn def_mut(&mut self, id: DefId) -> Option<&mut ItemDefinition> {
        self.defs.get_mut(id.0 as usize)
    }

    pub fn named(&self, name: &str) -> Option<DefId> {
        self.by_name.iter().find(|p| p.0 == name).map(|p| p.1)
    }

    /// Panics if the name is unknown, which is what you want in a
    /// catalogue built at start-up: a typo should not be a silent absence.
    pub fn must(&self, name: &str) -> DefId {
        self.named(name).unwrap_or_else(|| panic!("no such item definition: {name}"))
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ItemDefinition> {
        self.defs.iter()
    }

    pub fn of_family(&self, f: Family) -> impl Iterator<Item = &ItemDefinition> {
        self.defs.iter().filter(move |d| d.family == f)
    }
}

fn def(
    name: &'static str,
    family: Family,
    form: Form,
    nominal: Dims,
    kg: f64,
    materials: &[(Material, f64)],
) -> ItemDefinition {
    let comp = Composition::of(materials);
    // **Simply made of stuff, until something says otherwise.** A board, a
    // sheet, a brick: the bill is the material and the mass, which is a
    // complete and honest answer for a thing with no parts in it.
    let bill = Bom::of_material(&comp, kg);
    let origin = vec![default_origin(family, &comp)];
    let end_of_life = plausible_ends(&bill, &comp, family);
    ItemDefinition {
        id: DefId(0),
        name,
        family,
        form,
        nominal,
        nominal_mass_kg: kg,
        mass: NominalMass::of(kg, MassProvenance::DesignedPlaceholder),
        materials: comp,
        provides: Vec::new(),
        pockets: Vec::new(),
        attachment_points: Vec::new(),
        fits: None,
        lifecycle: family.lifecycle(),
        repair_with: Vec::new(),
        consumable: matches!(family, Family::Stock | Family::Fastening | Family::Ammunition),
        bill,
        origin,
        end_of_life,
    }
}

/// **Where a thing of this kind comes from, before anybody says
/// otherwise.** Timber is felled, ore is mined, oil is extracted, food is
/// harvested; everything else is made in a works, and the works is named
/// rather than the making being denied.
fn default_origin(family: Family, comp: &Composition) -> Origin {
    use Material::*;
    if family == Family::Stock {
        if let Some(m) = comp.chiefly() {
            return match m {
                Oak | Pine => Origin::Gathered(Acquisition::Logging),
                Flour | Water => Origin::Gathered(Acquisition::Harvesting),
                Brick | Concrete | Mortar => {
                    Origin::Industrial { needs: &["a quarry", "a kiln"] }
                }
                MildSteel | ToolSteel | Stainless | Aluminium | Copper | Brass | Lead
                | Nichrome | Ferrite => {
                    Origin::Industrial { needs: &["ore", "a smelter", "a rolling mill"] }
                }
                Mica => Origin::Gathered(Acquisition::Mining),
                Polyethylene | Abs | Polyester | Lubricant => {
                    Origin::Industrial { needs: &["petroleum", "a cracker"] }
                }
                Cotton | Wool | Leather => Origin::Gathered(Acquisition::Harvesting),
                _ => Origin::Industrial { needs: &["a works"] },
            };
        }
    }
    if family == Family::Foodstuff {
        return Origin::Industrial { needs: &["ingredients", "a kitchen"] };
    }
    Origin::Industrial { needs: &["a works", "tooling"] }
}

fn tool(mut d: ItemDefinition, provides: &[Provides]) -> ItemDefinition {
    d.provides = provides.to_vec();
    d
}

/// **The catalogue the model starts with.** Deliberately small and real
/// rather than exhaustive: one example from each family the household
/// basket needs, plus the stock and fasteners they are made of, so a chain
/// can be followed end to end before it is widened.
pub fn standard_catalogue() -> Catalogue {
    use Capability as C;
    use Material::*;
    let mut c = Catalogue::new();
    let d = Dims::new;

    // ---- stock -------------------------------------------------------
    // A 2 mm sheet, 2 m by 1 m: 0.004 m3 of steel is 31.4 kg.
    c.add(def("steel sheet", Family::Stock, Form::Sheet, d(2.0, 1.0, 0.002), 31.4,
              &[(MildSteel, 1.0)]));
    c.add(def("steel bar", Family::Stock, Form::Bar, d(3.0, 0.025, 0.025), 14.7,
              &[(MildSteel, 1.0)]));
    // 2.4 m by 150 by 25 of oak: 0.009 m3 at 750 is 6.75 kg.
    c.add(def("oak board", Family::Stock, Form::Bar, d(2.4, 0.15, 0.025), 6.75,
              &[(Oak, 1.0)]));
    c.add(def("pine board", Family::Stock, Form::Bar, d(2.4, 0.15, 0.025), 4.5,
              &[(Pine, 1.0)]));
    c.add(def("particleboard sheet", Family::Stock, Form::Sheet, d(2.44, 1.22, 0.018), 34.8,
              &[(Particleboard, 1.0)]));
    // **Right material, right thickness, right mass, wrong shape.** A
    // batten will not yield a seat and an offcut will not yield a leg, and
    // a substitution weighed rather than measured accepts both.
    c.add(def("oak batten", Family::Stock, Form::Bar, d(2.4, 0.04, 0.025), 1.8,
              &[(Oak, 1.0)]));
    c.add(def("oak offcut", Family::Stock, Form::Bar, d(0.6, 0.15, 0.025), 1.69,
              &[(Oak, 1.0)]));
    c.add(def("rope", Family::Stock, Form::Bar, d(4.8, 0.012, 0.012), 1.44,
              &[(Cotton, 1.0)]));
    c.add(def("cotton cloth", Family::Stock, Form::Fabric, d(10.0, 1.5, 0.0004), 4.5,
              &[(Cotton, 1.0)]));
    c.add(def("copper wire", Family::Stock, Form::Bar, d(100.0, 0.002, 0.002), 2.8,
              &[(Copper, 0.85), (Polyethylene, 0.15)]));
    c.add(def("polymer pellets", Family::Stock, Form::Granular, d(0.5, 0.4, 0.3), 25.0,
              &[(Polyethylene, 1.0)]));
    c.add(def("glass pane", Family::Stock, Form::Sheet, d(1.2, 0.9, 0.004), 10.8,
              &[(Glass, 1.0)]));
    c.add(def("brass case stock", Family::Stock, Form::Granular, d(0.3, 0.2, 0.2), 10.0,
              &[(Brass, 1.0)]));

    // ---- fastenings --------------------------------------------------
    c.add(def("wood screw", Family::Fastening, Form::Rigid, d(0.05, 0.004, 0.004), 0.005,
              &[(MildSteel, 1.0)]));
    c.add(def("bolt", Family::Fastening, Form::Rigid, d(0.06, 0.008, 0.008), 0.02,
              &[(MildSteel, 1.0)]));
    c.add(def("rivet", Family::Fastening, Form::Rigid, d(0.012, 0.004, 0.004), 0.001,
              &[(Aluminium, 1.0)]));
    c.add(def("wood glue", Family::Fastening, Form::Liquid, d(0.08, 0.08, 0.2), 1.0,
              &[(Adhesive, 1.0)]));
    c.add(def("thread reel", Family::Fastening, Form::Bar, d(0.05, 0.03, 0.03), 0.05,
              &[(Thread, 1.0)]));
    c.add(def("welding wire", Family::Fastening, Form::Bar, d(0.2, 0.2, 0.1), 5.0,
              &[(MildSteel, 1.0)]));
    c.add(def("solder", Family::Fastening, Form::Bar, d(0.06, 0.06, 0.03), 0.25,
              &[(Solder, 1.0)]));
    c.add(def("paint", Family::Fastening, Form::Liquid, d(0.16, 0.16, 0.2), 3.0,
              &[(Paint, 1.0)]));

    // ---- tools -------------------------------------------------------
    // Capacity for a cutter is mm of mild steel; precision is the
    // tolerance it holds; speed is against a nominal hand operation.
    c.add(tool(
        def("hacksaw", Family::Tool, Form::Rigid, d(0.4, 0.15, 0.03), 0.5,
            &[(MildSteel, 0.7), (Abs, 0.3)]),
        &[Provides { capability: C::CutMetal, capacity: 6.0, precision_mm: 1.0, speed: 1.0,
                     kw: 0.0, max_workpiece_m: 0.3 }],
    ));
    c.add(tool(
        def("angle grinder", Family::Tool, Form::Rigid, d(0.35, 0.12, 0.12), 2.2,
            &[(MildSteel, 0.55), (Abs, 0.3), (Copper, 0.15)]),
        &[Provides { capability: C::CutMetal, capacity: 12.0, precision_mm: 2.0, speed: 4.0,
                     kw: 1.2, max_workpiece_m: 0.5 },
          Provides { capability: C::Grind, capacity: 12.0, precision_mm: 1.0, speed: 3.0,
                     kw: 1.2, max_workpiece_m: 0.5 }],
    ));
    c.add(tool(
        def("bandsaw", Family::Machine, Form::Rigid, d(0.9, 0.7, 1.7), 180.0,
            &[(MildSteel, 0.85), (Abs, 0.1), (Copper, 0.05)]),
        &[Provides { capability: C::CutMetal, capacity: 25.0, precision_mm: 0.5, speed: 6.0,
                     kw: 1.5, max_workpiece_m: 0.3 },
          Provides { capability: C::CutWood, capacity: 200.0, precision_mm: 0.5, speed: 6.0,
                     kw: 1.5, max_workpiece_m: 3.0 }],
    ));
    c.add(tool(
        def("plasma cutter", Family::Machine, Form::Rigid, d(0.5, 0.25, 0.35), 22.0,
            &[(MildSteel, 0.6), (Copper, 0.2), (Abs, 0.2)]),
        &[Provides { capability: C::CutMetal, capacity: 40.0, precision_mm: 2.5, speed: 12.0,
                     kw: 7.0, max_workpiece_m: 3.0 }],
    ));
    c.add(tool(
        def("handsaw", Family::Tool, Form::Rigid, d(0.65, 0.15, 0.02), 0.6,
            &[(MildSteel, 0.7), (Pine, 0.3)]),
        &[Provides { capability: C::CutWood, capacity: 80.0, precision_mm: 1.5, speed: 1.0,
                     kw: 0.0, max_workpiece_m: 2.0 }],
    ));
    c.add(tool(
        def("cordless drill", Family::Tool, Form::Rigid, d(0.22, 0.07, 0.22), 1.6,
            &[(Abs, 0.45), (MildSteel, 0.35), (Copper, 0.15), (Silicon, 0.05)]),
        &[Provides { capability: C::Drill, capacity: 13.0, precision_mm: 1.0, speed: 5.0,
                     kw: 0.4, max_workpiece_m: 1.0 },
          Provides { capability: C::Fasten, capacity: 8.0, precision_mm: 2.0, speed: 4.0,
                     kw: 0.4, max_workpiece_m: 2.0 }],
    ));
    c.add(tool(
        def("hand drill", Family::Tool, Form::Rigid, d(0.3, 0.08, 0.2), 0.9,
            &[(MildSteel, 0.8), (Pine, 0.2)]),
        &[Provides { capability: C::Drill, capacity: 8.0, precision_mm: 1.5, speed: 1.0,
                     kw: 0.0, max_workpiece_m: 1.0 }],
    ));
    c.add(tool(
        def("hammer", Family::Tool, Form::Rigid, d(0.33, 0.12, 0.03), 0.6,
            &[(MildSteel, 0.75), (Pine, 0.25)]),
        &[Provides { capability: C::Strike, capacity: 20.0, precision_mm: 3.0, speed: 1.0,
                     kw: 0.0, max_workpiece_m: 2.0 },
          Provides { capability: C::Fasten, capacity: 5.0, precision_mm: 3.0, speed: 1.0,
                     kw: 0.0, max_workpiece_m: 2.0 }],
    ));
    c.add(tool(
        def("screwdriver", Family::Tool, Form::Rigid, d(0.25, 0.03, 0.03), 0.12,
            &[(MildSteel, 0.6), (Abs, 0.4)]),
        &[Provides { capability: C::Fasten, capacity: 8.0, precision_mm: 1.0, speed: 1.0,
                     kw: 0.0, max_workpiece_m: 2.0 }],
    ));
    c.add(tool(
        def("welder", Family::Machine, Form::Rigid, d(0.5, 0.3, 0.4), 30.0,
            &[(MildSteel, 0.55), (Copper, 0.35), (Abs, 0.1)]),
        &[Provides { capability: C::Weld, capacity: 12.0, precision_mm: 2.0, speed: 1.0,
                     kw: 5.0, max_workpiece_m: 4.0 }],
    ));
    c.add(tool(
        def("soldering iron", Family::Tool, Form::Rigid, d(0.25, 0.03, 0.03), 0.15,
            &[(MildSteel, 0.4), (Copper, 0.3), (Abs, 0.3)]),
        &[Provides { capability: C::Solder, capacity: 1.0, precision_mm: 0.5, speed: 1.0,
                     kw: 0.06, max_workpiece_m: 0.5 }],
    ));
    c.add(tool(
        def("sewing machine", Family::Machine, Form::Rigid, d(0.45, 0.2, 0.35), 8.0,
            &[(MildSteel, 0.5), (Abs, 0.35), (Copper, 0.15)]),
        &[Provides { capability: C::Sew, capacity: 6.0, precision_mm: 1.0, speed: 8.0,
                     kw: 0.1, max_workpiece_m: 3.0 }],
    ));
    c.add(tool(
        def("needle and thread", Family::Tool, Form::Rigid, d(0.05, 0.01, 0.01), 0.01,
            &[(MildSteel, 0.5), (Thread, 0.5)]),
        &[Provides { capability: C::Sew, capacity: 4.0, precision_mm: 2.0, speed: 1.0,
                     kw: 0.0, max_workpiece_m: 3.0 }],
    ));
    c.add(tool(
        def("scissors", Family::Tool, Form::Rigid, d(0.2, 0.07, 0.01), 0.1,
            &[(Stainless, 0.8), (Abs, 0.2)]),
        &[Provides { capability: C::CutFabric, capacity: 8.0, precision_mm: 2.0, speed: 1.0,
                     kw: 0.0, max_workpiece_m: 3.0 }],
    ));
    c.add(tool(
        def("oven", Family::Appliance, Form::Rigid, d(0.6, 0.6, 0.6), 45.0,
            &[(MildSteel, 0.8), (Glass, 0.12), (Abs, 0.05), (Copper, 0.03)]),
        &[Provides { capability: C::Bake, capacity: 250.0, precision_mm: 0.0, speed: 1.0,
                     kw: 2.5, max_workpiece_m: 0.5 }],
    ));
    c.add(tool(
        def("clamps", Family::Tool, Form::Rigid, d(0.3, 0.1, 0.05), 1.2,
            &[(MildSteel, 0.9), (Abs, 0.1)]),
        &[Provides { capability: C::Glue, capacity: 2.0, precision_mm: 3.0, speed: 1.0,
                     kw: 0.0, max_workpiece_m: 1.5 }],
    ));
    c.add(tool(
        def("sanding block", Family::Tool, Form::Rigid, d(0.12, 0.07, 0.04), 0.15,
            &[(Pine, 0.6), (Paperboard, 0.4)]),
        &[Provides { capability: C::Grind, capacity: 3.0, precision_mm: 2.0, speed: 1.0,
                     kw: 0.0, max_workpiece_m: 2.0 }],
    ));
    c.add(tool(
        def("workbench", Family::Furniture, Form::Rigid, d(1.8, 0.7, 0.9), 60.0,
            &[(Pine, 0.85), (MildSteel, 0.15)]),
        &[Provides { capability: C::Measure, capacity: 2.0, precision_mm: 1.0, speed: 1.0,
                     kw: 0.0, max_workpiece_m: 2.0 },
          Provides { capability: C::Press, capacity: 200.0, precision_mm: 2.0, speed: 1.0,
                     kw: 0.0, max_workpiece_m: 2.0 }],
    ));
    c.add(tool(
        def("reloading press", Family::Machine, Form::Rigid, d(0.3, 0.2, 0.4), 9.0,
            &[(MildSteel, 0.95), (Abs, 0.05)]),
        &[Provides { capability: C::Press, capacity: 500.0, precision_mm: 0.05, speed: 1.0,
                     kw: 0.0, max_workpiece_m: 0.1 },
          Provides { capability: C::Measure, capacity: 1.0, precision_mm: 0.02, speed: 1.0,
                     kw: 0.0, max_workpiece_m: 0.1 }],
    ));

    // ---- what a household buys ---------------------------------------
    let mut chair = def("wooden chair", Family::Furniture, Form::Assembly,
                        d(0.45, 0.45, 0.9), 5.01, &[(Oak, 0.96), (MildSteel, 0.03), (Adhesive, 0.01)]);
    chair.repair_with = vec![Oak, Adhesive, MildSteel];
    c.add(chair);

    let mut table = def("wooden table", Family::Furniture, Form::Assembly,
                        d(1.5, 0.9, 0.75), 30.0, &[(Oak, 0.97), (MildSteel, 0.02), (Adhesive, 0.01)]);
    table.repair_with = vec![Oak, Adhesive];
    c.add(table);

    // A washing machine really does carry 20-25 kg of concrete
    // counterweight, which is most of why it is so heavy and why moving
    // one is a two-person job.
    let mut washer = def("washing machine", Family::Appliance, Form::Assembly,
                         d(0.6, 0.6, 0.85), 70.0,
                         &[(MildSteel, 0.45), (Concrete, 0.30), (Abs, 0.12), (Copper, 0.05),
                           (Glass, 0.03), (Rubber, 0.03), (Silicon, 0.02)]);
    washer.repair_with = vec![MildSteel, Rubber, Copper];
    washer.attachment_points = vec![Fitting::Bracket];
    c.add(washer);

    c.add(def("electric kettle", Family::Appliance, Form::Assembly, d(0.22, 0.16, 0.25), 1.0,
              &[(Abs, 0.55), (Stainless, 0.35), (Copper, 0.08), (Silicon, 0.02)]));

    let mut trousers = def("work trousers", Family::Clothing, Form::Fabric,
                           d(1.0, 0.5, 0.01), 0.6, &[(Cotton, 0.96), (Thread, 0.03), (MildSteel, 0.01)]);
    trousers.repair_with = vec![Cotton, Thread];
    c.add(trousers);

    let mut coat = def("coat", Family::Clothing, Form::Fabric, d(1.2, 0.6, 0.03), 1.4,
                       &[(Wool, 0.8), (Polyester, 0.15), (Thread, 0.04), (MildSteel, 0.01)]);
    coat.repair_with = vec![Wool, Thread];
    c.add(coat);

    c.add(def("loaf", Family::Foodstuff, Form::Rigid, d(0.28, 0.12, 0.12), 0.8,
              &[(Flour, 0.62), (Water, 0.38)]));

    // ---- components and spares ---------------------------------------
    let mut alt = def("alternator", Family::SparePart, Form::Assembly, d(0.2, 0.16, 0.16), 5.5,
                      &[(MildSteel, 0.5), (Copper, 0.32), (Aluminium, 0.13), (Abs, 0.05)]);
    alt.fits = Some(Fitting::Bracket);
    c.add(alt);

    let mut batt = def("battery pack", Family::SparePart, Form::Rigid, d(0.1, 0.08, 0.06), 0.6,
                       &[(Abs, 0.4), (Lead, 0.35), (Copper, 0.2), (Silicon, 0.05)]);
    batt.fits = Some(Fitting::BatteryRail);
    c.add(batt);

    // ---- a firearm, at serviceable-assembly depth --------------------
    // Not every pin and spring: those stay inside a subassembly unless
    // they fail, are replaced, are manufactured or change how it shoots.
    let mut receiver = def("receiver", Family::SparePart, Form::Assembly, d(0.25, 0.05, 0.09), 0.9,
                           &[(Aluminium, 0.85), (MildSteel, 0.15)]);
    receiver.attachment_points = vec![Fitting::BarrelThread, Fitting::Magazine];
    c.add(receiver);
    let mut barrel = def("barrel", Family::SparePart, Form::Bar, d(0.5, 0.03, 0.03), 0.8,
                         &[(ToolSteel, 1.0)]);
    barrel.fits = Some(Fitting::BarrelThread);
    c.add(barrel);
    c.add(def("bolt assembly", Family::SparePart, Form::Assembly, d(0.18, 0.03, 0.03), 0.4,
              &[(ToolSteel, 1.0)]));
    c.add(def("fire control group", Family::SparePart, Form::Assembly, d(0.09, 0.04, 0.05), 0.25,
              &[(ToolSteel, 0.9), (Abs, 0.1)]));
    c.add(def("stock", Family::SparePart, Form::Rigid, d(0.3, 0.06, 0.12), 0.5,
              &[(Abs, 0.8), (MildSteel, 0.2)]));
    let mut magazine = def("magazine", Family::SparePart, Form::Assembly, d(0.19, 0.03, 0.09), 0.12,
                           &[(Abs, 0.7), (MildSteel, 0.3)]);
    magazine.fits = Some(Fitting::Magazine);
    c.add(magazine);
    let mut rifle = def("rifle", Family::Firearm, Form::Assembly, d(0.9, 0.06, 0.22), 3.01,
                        &[(ToolSteel, 0.45), (Aluminium, 0.3), (Abs, 0.25)]);
    rifle.attachment_points = vec![Fitting::Magazine, Fitting::BarrelThread];
    rifle.repair_with = vec![ToolSteel, Abs];
    c.add(rifle);

    // ---- ammunition, which is its own chain --------------------------
    // A 5.56 cartridge is about 12 g: 6.4 g of brass case, 4 g bullet,
    // 1.7 g of propellant, 0.3 g primer.
    c.add(def("cartridge case", Family::Ammunition, Form::Rigid, d(0.045, 0.01, 0.01), 0.0064,
              &[(Brass, 1.0)]));
    c.add(def("primer", Family::Ammunition, Form::Rigid, d(0.005, 0.005, 0.003), 0.0003,
              &[(Brass, 0.7), (Propellant, 0.3)]));
    c.add(def("bullet", Family::Ammunition, Form::Rigid, d(0.02, 0.006, 0.006), 0.004,
              &[(Lead, 0.7), (Copper, 0.3)]));
    c.add(def("propellant charge", Family::Ammunition, Form::Granular, d(0.01, 0.01, 0.01), 0.0017,
              &[(Propellant, 1.0)]));
    c.add(def("cartridge", Family::Ammunition, Form::Assembly, d(0.057, 0.01, 0.01), 0.0124,
              &[(Brass, 0.55), (Lead, 0.23), (Copper, 0.1), (Propellant, 0.12)]));

    // ---- what a building is made of ----------------------------------
    // **Some of a building stays an identifiable item and some of it does
    // not.** A door, a window, a socket and a radiator come out and go
    // back in; mortar, adhesive and sealant become joint mass and are
    // never on a shelf again.
    //
    // Real: a 2.4 m stud is 89 x 38 mm, OSB sheathing is 11 mm, a batt is
    // 100 mm, plasterboard is 12.5 mm and about 8.5 kg/m2.
    c.add(def("stud", Family::Stock, Form::Bar, d(2.4, 0.089, 0.038), 4.06,
              &[(Pine, 1.0)]));
    c.add(def("sheathing board", Family::Stock, Form::Sheet, d(2.4, 1.2, 0.011), 19.0,
              &[(Plywood, 1.0)]));
    c.add(def("insulation batt", Family::Stock, Form::Sheet, d(1.2, 0.6, 0.1), 0.9,
              &[(Polyester, 1.0)]));
    c.add(def("plasterboard sheet", Family::Stock, Form::Sheet, d(2.4, 1.2, 0.0125), 25.2,
              &[(Gypsum, 1.0)]));
    c.add(def("brick", Family::Stock, Form::Rigid, d(0.215, 0.1025, 0.065), 2.7,
              &[(Brick, 1.0)]));

    let mut door = def("door", Family::SparePart, Form::Assembly, d(1.98, 0.76, 0.04), 25.0,
                       &[(Pine, 0.86), (MildSteel, 0.1), (Glass, 0.04)]);
    door.fits = Some(Fitting::Opening);
    door.repair_with = vec![Pine, MildSteel];
    c.add(door);

    let mut window = def("window", Family::SparePart, Form::Assembly, d(1.2, 1.0, 0.06), 30.0,
                         &[(Glass, 0.7), (Abs, 0.25), (MildSteel, 0.05)]);
    window.fits = Some(Fitting::Opening);
    c.add(window);

    let mut socket = def("socket outlet", Family::SparePart, Form::Rigid, d(0.086, 0.086, 0.03),
                         0.1, &[(Abs, 0.7), (Copper, 0.25), (MildSteel, 0.05)]);
    socket.fits = Some(Fitting::BackBox);
    c.add(socket);

    let mut radiator = def("radiator", Family::SparePart, Form::Assembly, d(1.0, 0.6, 0.08), 22.0,
                           &[(MildSteel, 0.96), (Paint, 0.04)]);
    radiator.fits = Some(Fitting::ThreadM12);
    c.add(radiator);

    let mut wheel = def("road wheel", Family::SparePart, Form::Rigid, d(1.05, 0.3, 1.05), 70.0,
                        &[(MildSteel, 0.6), (Rubber, 0.4)]);
    wheel.fits = Some(Fitting::Hub);
    c.add(wheel);

    // ---- things that exist only part way through making something ----
    // **An intermediate is instantiated when it can be moved, traded,
    // spoil, be reused elsewhere, need its own storage, or be left
    // stranded by an interrupted process.** Dough can go off and cut
    // panels can be used for something else, so both are real. A step that
    // merely leaves a workpiece a bit further along gets no object.
    c.add(def("chair parts", Family::Stock, Form::Rigid, d(1.0, 0.5, 0.2), 4.82,
              &[(Oak, 1.0)]));
    c.add(def("cut panels", Family::Stock, Form::Fabric, d(1.0, 0.5, 0.01), 0.58,
              &[(Cotton, 1.0)]));
    let mut dough = def("dough", Family::Foodstuff, Form::Paste, d(0.25, 0.15, 0.1), 0.95,
                        &[(Flour, 0.6), (Water, 0.4)]);
    dough.lifecycle = Lifecycle::Perishable;
    c.add(dough);
    let mut risen = def("risen dough", Family::Foodstuff, Form::Paste, d(0.3, 0.18, 0.15), 0.95,
                        &[(Flour, 0.6), (Water, 0.4)]);
    risen.lifecycle = Lifecycle::Perishable;
    c.add(risen);
    c.add(def("sized case", Family::Ammunition, Form::Rigid, d(0.045, 0.01, 0.01), 0.0064,
              &[(Brass, 1.0)]));
    c.add(def("primed case", Family::Ammunition, Form::Rigid, d(0.045, 0.01, 0.01), 0.0067,
              &[(Brass, 0.96), (Propellant, 0.04)]));
    c.add(def("charged case", Family::Ammunition, Form::Rigid, d(0.045, 0.01, 0.01), 0.0084,
              &[(Brass, 0.77), (Propellant, 0.23)]));
    c.add(def("steel blank", Family::Stock, Form::Sheet, d(0.4, 0.3, 0.002), 1.9,
              &[(MildSteel, 1.0)]));
    c.add(def("washer shell", Family::Stock, Form::Assembly, d(0.6, 0.6, 0.85), 32.0,
              &[(MildSteel, 1.0)]));

    // **Everything above is a leaf or a stub; this is where the trees
    // are.** It runs last because a bill can only name parts that exist.
    deepen(&mut c);

    c
}

// =====================================================================
// all the way down
// =====================================================================

/// **What the interface groups, the data still holds.**
///
/// A crafting screen may show a drill as having a motor. A deep teardown
/// opens the motor and finds copper windings, laminated steel, ferrite
/// magnets, two bearings and polymer insulation — and every one of those
/// was in the data the whole time. This is where the trees are written.
///
/// The masses are real and they reconcile: a cordless drill is 1.6 kg and
/// its parts come to 1.6 kg, which the validator checks rather than
/// trusting.
fn deepen(c: &mut Catalogue) {
    use Material::*;
    let d = Dims::new;
    let e = |def: DefId, count: u32, kg: f64, place: &'static str, j: JointMethod| {
        BomEntry::new(def, count, kg, place, j)
    };

    // ---- fasteners that are not woodscrews ---------------------------
    let mscrew = c.add(def("machine screw", Family::Fastening, Form::Rigid,
                           d(0.016, 0.004, 0.004), 0.005, &[(MildSteel, 1.0)]));
    let mbolt = c.add(def("machine bolt", Family::Fastening, Form::Rigid,
                          d(0.04, 0.008, 0.008), 0.02, &[(MildSteel, 1.0)]));
    let spring = c.add(def("spring", Family::Fastening, Form::Rigid,
                           d(0.03, 0.008, 0.008), 0.009, &[(ToolSteel, 1.0)]));
    let pin = c.add(def("pin", Family::Fastening, Form::Rigid,
                        d(0.03, 0.004, 0.004), 0.006, &[(ToolSteel, 1.0)]));

    // ---- parts that turn up in more than one machine -----------------
    let bearing = c.add_built(
        def("ball bearing", Family::SparePart, Form::Rigid, d(0.022, 0.022, 0.007), 0.01,
            &[(MildSteel, 1.0)]),
        Bom::default().with_bulk(&[(MildSteel, 0.0098)]).with_fluids(&[(Lubricant, 0.0002)]),
    );
    let board = c.add_built(
        def("circuit board", Family::SparePart, Form::Sheet, d(0.08, 0.05, 0.002), 0.05,
            &[(Glass, 1.0)]),
        // Glass-epoxy laminate with copper on it, and the solder is trace
        // — small, declared, and emphatically not massless.
        Bom::default()
            .with_bulk(&[(Glass, 0.02), (Copper, 0.015), (Abs, 0.01)])
            .with_trace(&[(Solder, 0.005)]),
    );
    let loom_s = c.add_built(
        def("wiring loom, small", Family::SparePart, Form::Bar, d(0.4, 0.01, 0.01), 0.03,
            &[(Copper, 1.0)]),
        Bom::default().with_bulk(&[(Copper, 0.021), (Polyethylene, 0.009)]),
    );
    let loom_a = c.add_built(
        def("wiring loom, appliance", Family::SparePart, Form::Bar, d(2.5, 0.02, 0.02), 1.2,
            &[(Copper, 1.0)]),
        Bom::default().with_bulk(&[(Copper, 0.84), (Polyethylene, 0.36)]),
    );
    let magnet = c.add(def("magnet", Family::SparePart, Form::Rigid, d(0.03, 0.02, 0.006),
                           0.02, &[(Ferrite, 1.0)]));
    let winding_s = c.add_built(
        def("motor winding", Family::SparePart, Form::Rigid, d(0.05, 0.05, 0.03), 0.13,
            &[(Copper, 1.0)]),
        Bom::default().with_bulk(&[(Copper, 0.12), (Polyester, 0.01)]),
    );
    let lams_s = c.add(def("stator laminations", Family::SparePart, Form::Rigid,
                           d(0.05, 0.05, 0.04), 0.15, &[(MildSteel, 1.0)]));
    let motor_s = c.add_built(
        def("electric motor, small", Family::SparePart, Form::Assembly, d(0.07, 0.05, 0.05),
            0.36, &[(Copper, 1.0)]),
        Bom::assembled(vec![
            e(winding_s, 1, 0.13, "rotor", JointMethod::Glued),
            e(lams_s, 1, 0.15, "stator", JointMethod::Riveted),
            e(magnet, 2, 0.04, "field", JointMethod::Glued),
            e(bearing, 2, 0.02, "shaft ends", JointMethod::Crimped),
        ])
        .with_bulk(&[(Polyester, 0.02)]),
    );

    // ---- a cordless drill, opened up ---------------------------------
    let chuck_body = c.add(def("chuck body", Family::SparePart, Form::Rigid,
                               d(0.05, 0.04, 0.04), 0.18, &[(ToolSteel, 1.0)]));
    let jaw = c.add(def("chuck jaw", Family::SparePart, Form::Rigid, d(0.03, 0.006, 0.006),
                        0.015, &[(ToolSteel, 1.0)]));
    let chuck = c.add_built(
        def("chuck assembly", Family::SparePart, Form::Assembly, d(0.06, 0.05, 0.05), 0.30,
            &[(ToolSteel, 1.0)]),
        Bom::assembled(vec![
            e(chuck_body, 1, 0.18, "body", JointMethod::Crimped),
            e(jaw, 3, 0.045, "jaws", JointMethod::Clipped),
        ])
        // The adjustment ring and the retaining parts, grouped — and
        // still weighing 75 grams of steel.
        .with_bulk(&[(MildSteel, 0.075)]),
    );
    let gear = c.add(def("spur gear", Family::SparePart, Form::Rigid, d(0.03, 0.03, 0.01),
                         0.03, &[(ToolSteel, 1.0)]));
    let shaft = c.add(def("shaft", Family::SparePart, Form::Bar, d(0.08, 0.008, 0.008),
                          0.03, &[(ToolSteel, 1.0)]));
    let gearbox = c.add_built(
        def("gearbox", Family::SparePart, Form::Assembly, d(0.07, 0.05, 0.05), 0.28,
            &[(ToolSteel, 1.0)]),
        Bom::assembled(vec![
            e(gear, 4, 0.12, "reduction train", JointMethod::Crimped),
            e(shaft, 2, 0.06, "shafts", JointMethod::Crimped),
            e(bearing, 4, 0.04, "journals", JointMethod::Crimped),
        ])
        .with_bulk(&[(Abs, 0.05)])
        .with_fluids(&[(Lubricant, 0.01)]),
    );
    let trigger_sw = c.add_built(
        def("trigger switch", Family::SparePart, Form::Rigid, d(0.03, 0.02, 0.02), 0.02,
            &[(Abs, 1.0)]),
        Bom::default().with_bulk(&[(Abs, 0.014), (Copper, 0.006)]),
    );
    let heatsink = c.add(def("heat sink", Family::SparePart, Form::Rigid, d(0.04, 0.03, 0.01),
                             0.02, &[(Aluminium, 1.0)]));
    let control = c.add_built(
        def("control assembly", Family::SparePart, Form::Assembly, d(0.06, 0.04, 0.04), 0.12,
            &[(Abs, 1.0)]),
        Bom::assembled(vec![
            e(trigger_sw, 1, 0.02, "trigger", JointMethod::Clipped),
            e(board, 1, 0.05, "controller", JointMethod::Screwed),
            e(loom_s, 1, 0.03, "harness", JointMethod::Crimped),
            e(heatsink, 1, 0.02, "on the switching device", JointMethod::Screwed),
        ]),
    );
    let casing = c.add(def("drill casing", Family::SparePart, Form::Rigid, d(0.22, 0.07, 0.22),
                           0.42, &[(Abs, 1.0)]));
    let batt_if = c.add_built(
        def("battery interface", Family::SparePart, Form::Rigid, d(0.08, 0.06, 0.02), 0.06,
            &[(Abs, 1.0)]),
        Bom::default().with_bulk(&[(Abs, 0.04), (Copper, 0.02)]),
    );
    let drill = c.must("cordless drill");
    c.set_bill(
        drill,
        Bom::assembled(vec![
            e(chuck, 1, 0.30, "output spindle", JointMethod::Crimped),
            e(gearbox, 1, 0.28, "behind the chuck", JointMethod::Screwed),
            e(motor_s, 1, 0.36, "midships", JointMethod::Screwed),
            e(control, 1, 0.12, "in the grip", JointMethod::Screwed),
            e(casing, 1, 0.42, "clamshell", JointMethod::Screwed),
            e(batt_if, 1, 0.06, "foot of the grip", JointMethod::Screwed),
            e(mscrew, 12, 0.06, "throughout", JointMethod::Screwed),
        ]),
    );

    // ---- a toaster, which is the example that started this -----------
    let shell = c.add(def("toaster shell", Family::SparePart, Form::Sheet, d(0.3, 0.18, 0.19),
                          0.55, &[(Stainless, 1.0)]));
    let chassis = c.add(def("toaster chassis", Family::SparePart, Form::Sheet,
                            d(0.28, 0.16, 0.17), 0.42, &[(MildSteel, 1.0)]));
    let element = c.add_built(
        def("heating element", Family::SparePart, Form::Sheet, d(0.14, 0.11, 0.004), 0.06,
            &[(Nichrome, 1.0)]),
        Bom::default().with_bulk(&[(Nichrome, 0.05), (Mica, 0.01)]),
    );
    let insulator = c.add(def("element insulator", Family::SparePart, Form::Sheet,
                              d(0.15, 0.12, 0.003), 0.09, &[(Mica, 1.0)]));
    let t_control = c.add_built(
        def("toaster control", Family::SparePart, Form::Assembly, d(0.08, 0.05, 0.04), 0.14,
            &[(MildSteel, 1.0)]),
        Bom::assembled(vec![
            e(board, 1, 0.05, "timer", JointMethod::Screwed),
            e(trigger_sw, 1, 0.02, "browning control", JointMethod::Clipped),
        ])
        // The thermostat and its bimetal strip, grouped.
        .with_bulk(&[(MildSteel, 0.05), (Copper, 0.02)]),
    );
    let lever = c.add_built(
        def("carriage lever", Family::SparePart, Form::Assembly, d(0.16, 0.06, 0.03), 0.11,
            &[(MildSteel, 1.0)]),
        Bom::assembled(vec![e(spring, 2, 0.018, "return", JointMethod::Clipped)])
            .with_bulk(&[(MildSteel, 0.072), (Abs, 0.02)]),
    );
    let feet = c.add_built(
        def("toaster feet and trim", Family::SparePart, Form::Rigid, d(0.28, 0.16, 0.01),
            0.10, &[(Rubber, 1.0)]),
        Bom::default().with_bulk(&[(Rubber, 0.06), (Abs, 0.04)]),
    );
    let toaster = c.add(def("toaster", Family::Appliance, Form::Assembly, d(0.3, 0.18, 0.19),
                            1.80, &[(Stainless, 1.0)]));
    c.set_bill(
        toaster,
        Bom::assembled(vec![
            e(shell, 1, 0.55, "outside", JointMethod::Screwed),
            e(chassis, 1, 0.42, "inside", JointMethod::Riveted),
            e(element, 2, 0.12, "each side of the slot", JointMethod::Clipped),
            e(insulator, 2, 0.18, "behind the elements", JointMethod::Clipped),
            e(loom_s, 3, 0.09, "throughout", JointMethod::Crimped),
            e(t_control, 1, 0.14, "front", JointMethod::Screwed),
            e(lever, 1, 0.11, "side", JointMethod::Clipped),
            e(feet, 1, 0.10, "underneath", JointMethod::Clipped),
            e(mscrew, 18, 0.09, "throughout", JointMethod::Screwed),
        ]),
    );

    // ---- the rifle, past field-strip depth ---------------------------
    let bolt_body = c.add(def("bolt body", Family::SparePart, Form::Rigid,
                              d(0.09, 0.025, 0.025), 0.28, &[(ToolSteel, 1.0)]));
    let extractor = c.add(def("extractor", Family::SparePart, Form::Rigid,
                              d(0.03, 0.008, 0.008), 0.03, &[(ToolSteel, 1.0)]));
    let ejector = c.add(def("ejector", Family::SparePart, Form::Rigid, d(0.02, 0.006, 0.006),
                            0.02, &[(ToolSteel, 1.0)]));
    let firing_pin = c.add(def("firing pin", Family::SparePart, Form::Bar,
                               d(0.07, 0.005, 0.005), 0.025, &[(ToolSteel, 1.0)]));
    let bolt_asm = c.must("bolt assembly");
    c.set_bill(
        bolt_asm,
        Bom::assembled(vec![
            e(bolt_body, 1, 0.28, "carrier", JointMethod::Crimped),
            e(extractor, 1, 0.03, "bolt face", JointMethod::Clipped),
            e(ejector, 1, 0.02, "bolt face", JointMethod::Clipped),
            e(firing_pin, 1, 0.025, "through the carrier", JointMethod::Clipped),
            e(spring, 5, 0.045, "retainers", JointMethod::Clipped),
        ]),
    );
    let trigger = c.add(def("trigger", Family::SparePart, Form::Rigid, d(0.04, 0.008, 0.03),
                            0.05, &[(ToolSteel, 1.0)]));
    let hammer = c.add(def("hammer", Family::SparePart, Form::Rigid, d(0.04, 0.01, 0.04),
                           0.07, &[(ToolSteel, 1.0)]));
    let sear = c.add(def("sear", Family::SparePart, Form::Rigid, d(0.02, 0.006, 0.015),
                         0.025, &[(ToolSteel, 1.0)]));
    let fcg = c.must("fire control group");
    c.set_bill(
        fcg,
        Bom::assembled(vec![
            e(trigger, 1, 0.05, "front", JointMethod::Riveted),
            e(hammer, 1, 0.07, "middle", JointMethod::Riveted),
            e(sear, 1, 0.025, "on the hammer", JointMethod::Riveted),
            e(pin, 5, 0.03, "pivots", JointMethod::Riveted),
            e(spring, 3, 0.027, "trigger and hammer", JointMethod::Clipped),
        ])
        .with_bulk(&[(MildSteel, 0.048)]),
    );
    let rifle = c.must("rifle");
    c.set_bill(
        rifle,
        Bom::assembled(vec![
            e(c.must("receiver"), 1, 0.9, "the serialised part", JointMethod::Riveted),
            e(c.must("barrel"), 1, 0.8, "forward", JointMethod::Crimped),
            e(bolt_asm, 1, 0.4, "in the receiver", JointMethod::Clipped),
            e(fcg, 1, 0.25, "under the receiver", JointMethod::Riveted),
            e(c.must("stock"), 1, 0.5, "rear", JointMethod::Bolted),
            e(c.must("magazine"), 1, 0.12, "magazine well", JointMethod::Clipped),
            e(mbolt, 2, 0.04, "stock bolts", JointMethod::Bolted),
        ]),
    );

    // ---- a washing machine, which is mostly concrete -----------------
    let w_casing = c.add(def("washer casing", Family::SparePart, Form::Sheet,
                             d(0.6, 0.6, 0.85), 12.0, &[(MildSteel, 1.0)]));
    let w_drum = c.add(def("washer drum", Family::SparePart, Form::Assembly,
                           d(0.5, 0.5, 0.4), 8.5, &[(Stainless, 1.0)]));
    let w_tub = c.add(def("washer tub", Family::SparePart, Form::Assembly, d(0.55, 0.55, 0.45),
                          5.0, &[(Polyethylene, 1.0)]));
    // **Real, and the reason a washing machine is a two-person lift.**
    let weight = c.add(def("counterweight", Family::SparePart, Form::Rigid,
                           d(0.4, 0.15, 0.1), 10.5, &[(Concrete, 1.0)]));
    let winding_l = c.add_built(
        def("motor winding, large", Family::SparePart, Form::Rigid, d(0.15, 0.15, 0.08), 2.2,
            &[(Copper, 1.0)]),
        Bom::default().with_bulk(&[(Copper, 2.0), (Polyester, 0.2)]),
    );
    let lams_l = c.add(def("stator laminations, large", Family::SparePart, Form::Rigid,
                           d(0.15, 0.15, 0.1), 2.8, &[(MildSteel, 1.0)]));
    let motor_a = c.add_built(
        def("electric motor, appliance", Family::SparePart, Form::Assembly, d(0.2, 0.16, 0.16),
            6.0, &[(Copper, 1.0)]),
        Bom::assembled(vec![
            e(winding_l, 1, 2.2, "rotor", JointMethod::Glued),
            e(lams_l, 1, 2.8, "stator", JointMethod::Riveted),
            e(bearing, 2, 0.02, "shaft ends", JointMethod::Crimped),
        ])
        .with_bulk(&[(MildSteel, 0.9)])
        .with_fluids(&[(Lubricant, 0.08)]),
    );
    let pump = c.add_built(
        def("water pump", Family::SparePart, Form::Assembly, d(0.14, 0.1, 0.1), 1.5,
            &[(Abs, 1.0)]),
        Bom::default()
            .with_bulk(&[(Abs, 0.6), (MildSteel, 0.5), (Copper, 0.35)])
            .with_fluids(&[(Lubricant, 0.05)]),
    );
    let a_board = c.add_built(
        def("appliance control board", Family::SparePart, Form::Assembly, d(0.25, 0.1, 0.04),
            0.6, &[(Abs, 1.0)]),
        Bom::assembled(vec![e(board, 4, 0.20, "stacked", JointMethod::Screwed)])
            .with_bulk(&[(Abs, 0.30), (Copper, 0.09)])
            .with_trace(&[(Solder, 0.01)]),
    );
    let w_door = c.add_built(
        def("washer door", Family::SparePart, Form::Assembly, d(0.35, 0.35, 0.08), 3.2,
            &[(Glass, 1.0)]),
        Bom::default().with_bulk(&[(Glass, 2.2), (Abs, 0.9), (Rubber, 0.1)]),
    );
    let damper = c.add_built(
        def("suspension damper", Family::SparePart, Form::Bar, d(0.3, 0.04, 0.04), 1.0,
            &[(MildSteel, 1.0)]),
        Bom::default()
            .with_bulk(&[(MildSteel, 0.85), (Polyethylene, 0.1)])
            .with_fluids(&[(Lubricant, 0.05)]),
    );
    let w_frame = c.add(def("washer frame", Family::SparePart, Form::Assembly,
                            d(0.58, 0.58, 0.8), 3.0, &[(MildSteel, 1.0)]));
    let washer = c.must("washing machine");
    c.set_bill(
        washer,
        Bom::assembled(vec![
            e(w_casing, 1, 12.0, "outside", JointMethod::Screwed),
            e(w_frame, 1, 3.0, "chassis", JointMethod::Bolted),
            e(w_drum, 1, 8.5, "inside the tub", JointMethod::Bolted),
            e(w_tub, 1, 5.0, "suspended", JointMethod::Bolted),
            e(weight, 2, 21.0, "front and top of the tub", JointMethod::Bolted),
            e(motor_a, 1, 6.0, "under the tub", JointMethod::Bolted),
            e(pump, 1, 1.5, "sump", JointMethod::Clipped),
            e(a_board, 1, 0.6, "behind the fascia", JointMethod::Screwed),
            e(w_door, 1, 3.2, "front", JointMethod::Screwed),
            e(damper, 4, 4.0, "tub suspension", JointMethod::Bolted),
            e(loom_a, 1, 1.2, "throughout", JointMethod::Crimped),
            e(mscrew, 160, 0.8, "throughout", JointMethod::Screwed),
            e(mbolt, 50, 1.0, "structure", JointMethod::Bolted),
        ])
        .with_bulk(&[(Rubber, 2.2)]),
    );

    // ---- and the chair, which the plan already half knew -------------
    let leg = c.add(def("chair leg", Family::Stock, Form::Bar, d(0.45, 0.04, 0.04), 0.4875,
                        &[(Oak, 1.0)]));
    let rail = c.add(def("chair rail", Family::Stock, Form::Bar, d(0.4, 0.05, 0.02), 0.3925,
                         &[(Oak, 1.0)]));
    let seat = c.add(def("chair seat", Family::Stock, Form::Sheet, d(0.4, 0.4, 0.02), 0.74,
                         &[(Oak, 1.0)]));
    let backrest = c.add(def("chair back", Family::Stock, Form::Bar, d(0.4, 0.3, 0.02), 0.56,
                             &[(Oak, 1.0)]));
    // **The intermediate is a group, not a mystery.** The plan cuts a
    // board into "chair parts"; what those parts are is written down.
    let parts = c.must("chair parts");
    c.set_bill(
        parts,
        Bom::assembled(vec![
            e(leg, 4, 1.95, "legs", JointMethod::Clipped),
            e(rail, 4, 1.57, "rails", JointMethod::Clipped),
            e(seat, 1, 0.74, "seat", JointMethod::Clipped),
            e(backrest, 1, 0.56, "back", JointMethod::Clipped),
        ]),
    );
    let chair = c.must("wooden chair");
    c.set_bill(
        chair,
        Bom::assembled(vec![
            e(parts, 1, 4.82, "the frame", JointMethod::Glued),
            e(c.must("wood screw"), 12, 0.06, "seat and rails", JointMethod::Screwed),
        ])
        .with_joints(&[(Adhesive, 0.09)])
        .with_coatings(&[(Paint, 0.04)]),
    );

    // ---- a car door, which is the case that named the problem --------
    //
    // **Calling a pressed skin "bulk steel" recreates the problem.** Taken
    // off carefully it is a door skin, bent or not; it only becomes sheet
    // after somebody cuts or crushes it. So the skin, the inner frame, the
    // intrusion beam and the brackets are *formed parts* — neither
    // separately traded components nor anonymous stuff — while the seam
    // sealer and the damping compound genuinely are stuff.
    //
    // Real: a front door is 25-30 kg with the glass and the trim in it.
    let latch = c.add(def("door latch", Family::SparePart, Form::Assembly,
                          d(0.12, 0.08, 0.06), 1.1, &[(MildSteel, 0.8), (Abs, 0.2)]));
    let hinge = c.add(def("door hinge", Family::SparePart, Form::Rigid,
                          d(0.1, 0.06, 0.05), 0.8, &[(MildSteel, 1.0)]));
    let regulator = c.add_built(
        def("window regulator", Family::SparePart, Form::Assembly, d(0.5, 0.4, 0.05), 2.4,
            &[(MildSteel, 1.0)]),
        Bom::assembled(vec![
            e(motor_s, 1, 0.36, "drive", JointMethod::Bolted),
            e(mscrew, 6, 0.03, "rails", JointMethod::Screwed),
        ])
        .with_formed(&[
            Formed::new("regulator rail", "guides the glass", MildSteel, 1.2, Geometry::Stamping)
                .finished(Surface::Galvanised),
            Formed::new("lift arm", "carries the glass", MildSteel, 0.81, Geometry::Stamping),
        ]),
    );
    let door_loom = c.add_built(
        def("wiring loom, door", Family::SparePart, Form::Bar, d(1.2, 0.02, 0.02), 0.9,
            &[(Copper, 1.0)]),
        Bom::default().with_bulk(&[(Copper, 0.63), (Polyethylene, 0.27)]),
    );
    let seal = c.add(def("weather seal", Family::SparePart, Form::Bar, d(3.5, 0.02, 0.02),
                         1.2, &[(Rubber, 1.0)]));
    let glass = c.add(def("door glass", Family::SparePart, Form::Sheet, d(0.9, 0.5, 0.004),
                          4.0, &[(Glass, 1.0)]));
    let trim = c.add(def("door trim panel", Family::SparePart, Form::Sheet, d(0.9, 0.6, 0.03),
                         2.0, &[(Abs, 0.75), (Polyester, 0.25)]));

    let mut car_door = def("car door", Family::SparePart, Form::Assembly, d(1.0, 1.0, 0.15),
                           28.1, &[(MildSteel, 1.0)]);
    car_door.fits = Some(Fitting::Opening);
    car_door.repair_with = vec![MildSteel, Paint, Glass];
    let door = c.add(car_door);
    c.set_bill(
        door,
        Bom::assembled(vec![
            e(latch, 1, 1.1, "rear edge", JointMethod::Bolted),
            e(hinge, 2, 1.6, "front edge", JointMethod::Bolted),
            e(regulator, 1, 2.4, "inside the cavity", JointMethod::Bolted),
            e(door_loom, 1, 0.9, "through the cavity", JointMethod::Clipped),
            e(seal, 1, 1.2, "round the aperture", JointMethod::Clipped),
            e(glass, 1, 4.0, "in the regulator", JointMethod::Clipped),
            e(trim, 1, 2.0, "inner face", JointMethod::Clipped),
        ])
        .with_formed(&[
            Formed::new("outer skin", "the visible panel", MildSteel, 4.5, Geometry::Stamping)
                .finished(Surface::Painted),
            Formed::new("inner frame", "the structure", MildSteel, 6.0, Geometry::Stamping)
                .finished(Surface::Galvanised),
            Formed::new("intrusion beam", "side-impact protection", MildSteel, 2.2,
                        Geometry::Extrusion)
                .treated(MaterialState::QuenchedAndTempered),
            Formed::new("mounting brackets", "hinge and latch mountings", MildSteel, 0.8,
                        Geometry::Stamping),
        ])
        // **These really are stuff.** Neither has a shape that means
        // anything once it is off the panel.
        .with_bulk(&[(Adhesive, 0.25), (Rubber, 0.6)])
        .with_coatings(&[(Zinc, 0.15), (Paint, 0.3)])
        .with_joints(&[(MildSteel, 0.05), (Adhesive, 0.05)]),
    );

    // ---- a battery is not honestly a leaf ----------------------------
    // A bolt body can stay a leaf: how it was forged is manufacturing
    // history, not composition. A sealed pack cannot, because its cells,
    // its casing and its electrolyte fail, are replaced and are recovered
    // on their own — and one of them is hazardous.
    let cell = c.add_built(
        def("battery cell", Family::SparePart, Form::Rigid, d(0.065, 0.018, 0.018), 0.046,
            &[(Lithium, 1.0)]),
        Bom::default()
            .with_bulk(&[(Lithium, 0.006), (Copper, 0.018), (Aluminium, 0.012), (Abs, 0.004)])
            .with_fluids(&[(Electrolyte, 0.006)]),
    );
    let pack = c.must("battery pack");
    c.set_bill(
        pack,
        Bom::assembled(vec![
            e(cell, 5, 0.23, "in series", JointMethod::Soldered),
            e(board, 1, 0.05, "protection circuit", JointMethod::Soldered),
        ])
        .with_formed(&[Formed::new("pack casing", "the shell", Abs, 0.26, Geometry::Shell)])
        .with_bulk(&[(Copper, 0.06)]),
    );

    // ---- which plan makes which thing --------------------------------
    for (name, plan) in [
        ("wooden chair", "chair, hand tools"),
        ("loaf", "loaf"),
        ("work trousers", "work trousers"),
        ("cartridge", "cartridge, handloaded"),
        ("rifle", "rifle, assembled"),
    ] {
        let id = c.must(name);
        c.set_origin(id, vec![Origin::Made { plan }]);
    }
    // **A microprocessor is not uncraftable; it needs a fab.** Naming the
    // requirement is the difference between a gap and a prohibition.
    let id = c.must("circuit board");
    c.set_origin(
        id,
        vec![Origin::Industrial {
            needs: &[
                "semiconductor-grade silicon",
                "photolithography",
                "a cleanroom",
                "process chemicals",
                "purified water",
                "uninterrupted power",
            ],
        }],
    );
    let _ = (toaster, drill, washer, mbolt);
}


// =====================================================================
// instances
// =====================================================================

/// **What is holding it up.**
///
/// A vehicle, a building, or another item. Kept as a bare id rather than a
/// handle so that `item.rs` does not have to know what a lorry is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Host {
    Item(Id<ItemInstance>),
    Vehicle(u32),
    Building(u32),
}

/// **A live item is in exactly one place**, and every one of these is a
/// real place.
///
/// There is deliberately no `Nowhere`. That variant was two different
/// conditions wearing one name — *not made yet* and *destroyed* — and
/// neither of them is a location. Whether a thing exists is
/// [`ItemEnd`]'s question; this answers only where it is, and it exists
/// only for things that are in a [`Store`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Ground { locality: u32, x: i32, y: i32 },
    Carried { person: u64 },
    Contained { container: Id<ItemInstance> },
    Installed { host: Host, mount: usize },
    /// **In a machine**: clamped in the press, on the bed, or sitting in
    /// its output tray. A real place, which is why it can hold the machine
    /// up and why somebody can pick a cool panel out of a tray and walk
    /// off with it.
    Fixtured { resource: u32, slot: u32, clamped: bool },
}

impl Placement {
    /// Somewhere on the floor at the origin. For tests and for anything
    /// that genuinely does not care where.
    pub fn anywhere() -> Self {
        Placement::Ground { locality: 0, x: 0, y: 0 }
    }

    /// **Whether it can be got at.** A panel in an output tray can be
    /// picked up and walked off with; the same panel clamped in the press
    /// cannot, and that is a fact about where it is rather than about who
    /// has booked it.
    pub fn reachable(self) -> bool {
        match self {
            Placement::Installed { .. } => false,
            Placement::Fixtured { clamped, .. } => !clamped,
            _ => true,
        }
    }

    pub fn is_installed(self) -> bool {
        matches!(self, Placement::Installed { .. })
    }

    pub fn why_not(self) -> &'static str {
        match self {
            Placement::Installed { .. } => "it is fitted to something",
            Placement::Fixtured { clamped: true, .. } => "it is clamped in a machine",
            _ => "it is where it can be got at",
        }
    }

    /// Which machine it is holding up, if any.
    pub fn occupying(self) -> Option<u32> {
        match self {
            Placement::Fixtured { resource, .. } => Some(resource),
            _ => None,
        }
    }
}

/// **What a thing is spoken for, which is not where it is.**
///
/// A work order is not a place. A reserved board is physically on its
/// rack; a workpiece part way through a stamping is physically in the
/// press; a finished panel awaiting collection is physically in the output
/// tray. Folding any of that into the placement puts `Nowhere` back under
/// a more informative name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WorkStatus {
    #[default]
    Available,
    /// Somebody has claimed it for a job that has not started. It has not
    /// moved.
    Reserved { order: u64 },
    /// It is being worked on right now, and this is which operation.
    Wip { order: u64, operation: usize },
    /// Finished, and still where it was made. Whoever wants it has to come
    /// and fetch it, and until they do the machine is not free.
    AwaitingUnload { order: u64 },
}

impl WorkStatus {
    pub fn free(self) -> bool {
        matches!(self, WorkStatus::Available)
    }

    pub fn order(self) -> Option<u64> {
        match self {
            WorkStatus::Available => None,
            WorkStatus::Reserved { order }
            | WorkStatus::Wip { order, .. }
            | WorkStatus::AwaitingUnload { order } => Some(order),
        }
    }

    pub fn why_not(self) -> &'static str {
        match self {
            WorkStatus::Available => "it is free",
            WorkStatus::Reserved { .. } => "it is committed to a work order",
            WorkStatus::Wip { .. } => "it is being worked on",
            WorkStatus::AwaitingUnload { .. } => "it is finished and not yet collected",
        }
    }
}

/// **How a thing stopped being a live item.**
///
/// Not a placement. An item that reaches one of these leaves the store
/// altogether and survives as a tombstone — which is what keeps the live
/// world bounded and still lets the books be checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemEnd {
    /// Used up: eaten, burnt, welded into something, fired.
    Consumed,
    /// Broken past recovery. Whatever came off it is separate objects.
    Destroyed,
    /// Folded into a lot. It has no particulars any more, and it cannot
    /// be addressed individually until the lot is expanded.
    Aggregated { lot: u64 },
}

/// What is left in the record of something that has gone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tombstone {
    pub was: DefId,
    pub end: ItemEnd,
    pub mass_kg: f64,
    pub on_day: u32,
}

/// **This particular one**, including everything that has happened to it.
#[derive(Clone, Debug)]
pub struct ItemInstance {
    pub definition: DefId,
    pub quantity: Quantity,
    /// What it actually weighs, which drifts from nominal with wear,
    /// moisture, contents and substituted materials.
    pub mass_kg: f64,
    /// What it is actually made of. Read off the assembly record where
    /// there is one, so a chair built of particleboard is particleboard.
    pub materials: Composition,
    pub quality: Quality,
    pub condition: Condition,
    pub faults: Vec<Fault>,
    pub contents: Vec<Id<ItemInstance>>,
    pub attachments: Vec<(Fitting, Id<ItemInstance>)>,
    pub assembly: Option<AssemblyRecord>,
    pub provenance: Provenance,
    pub ownership: Ownership,
    /// A name somebody gave it. Named things are never aggregated.
    pub given_name: Option<String>,
    /// **What it is on the way to being**, for something part way through
    /// a plan. `None` for an ordinary finished object; `Some` for a blank,
    /// a pressed shell or a coated panel, which are real things with a
    /// geometry and a history and no catalogue entry of their own.
    pub shape: Option<crate::wip::Shape>,
    /// **Where the metal came from**, for anything that has been through a
    /// furnace. The objects that were charged have ended; their
    /// composition, contamination, hazard and recycled content have not.
    pub heat: Option<crate::wip::Heat>,
}

impl ItemInstance {
    /// **A factory-fresh example**, which knows what it contains.
    ///
    /// A spawned instance receives the definition bill: what a normal one
    /// of these is made of. A crafted instance receives the *actual* bill
    /// — what really went into that one — and the two are allowed to
    /// differ, which is the whole point of keeping both.
    pub fn fresh(cat: &Catalogue, id: DefId, quantity: Quantity) -> Self {
        let d = cat.get(id).expect("unknown definition");
        let mass = quantity.mass_kg(d.nominal_mass_kg);
        let assembly = default_record(cat, d, mass);
        ItemInstance {
            definition: id,
            quantity,
            mass_kg: mass,
            materials: d.materials.clone(),
            quality: Quality::default(),
            condition: Condition::fresh(),
            faults: Vec::new(),
            contents: Vec::new(),
            attachments: Vec::new(),
            assembly,
            provenance: Provenance::default(),
            ownership: Ownership::default(),
            given_name: None,
            shape: None,
            heat: None,
        }
    }

    pub fn one(cat: &Catalogue, id: DefId) -> Self {
        ItemInstance::fresh(cat, id, Quantity::Count(1))
    }

    /// How much of its job it can still do — quality and condition
    /// together, which is the only place they are allowed to combine.
    pub fn effectiveness(&self) -> f64 {
        let disabled = self.faults.iter().any(|f| f.disabling);
        if disabled || self.condition.is_ruined() {
            return 0.0;
        }
        (self.quality.overall() * self.condition.serviceability()).clamp(0.0, 1.0)
    }

    /// **Repair moves condition and leaves quality alone.** A mended chair
    /// is a mended cheap chair.
    pub fn repair(&mut self, effort: f64) {
        let take = effort.clamp(0.0, 1.0);
        // Repair is never quite as good as new, and the ceiling is what
        // the thing was made to be.
        self.condition.damage = (self.condition.damage * (1.0 - take * 0.9)).max(0.0);
        self.condition.wear = (self.condition.wear * (1.0 - take * 0.5)).max(0.0);
        self.faults.retain(|f| f.severity > take);
    }


    /// Whether this is an example whose particulars would survive being
    /// folded into a lot. **Where it is, is the `Store`'s question** — a
    /// fitted part is not aggregatable either, and `Store::aggregatable`
    /// asks both.
    pub fn aggregatable(&self) -> bool {
        self.given_name.is_none()
            && !self.provenance.marked
            && !self.ownership.accounted_for
            && self.contents.is_empty()
            && self.attachments.is_empty()
            && self.faults.is_empty()
            && self.assembly.as_ref().map(|a| a.substitutions.is_empty()).unwrap_or(true)
    }
}

/// Turn a definition bill into the record a fresh example carries.
fn default_record(cat: &Catalogue, d: &ItemDefinition, mass_kg: f64) -> Option<AssemblyRecord> {
    if d.bill.is_empty() {
        return None;
    }
    let scale = if d.nominal_mass_kg > 0.0 { mass_kg / d.nominal_mass_kg } else { 1.0 };
    let mut record = AssemblyRecord {
        formed: d
            .bill
            .formed
            .iter()
            .map(|f| {
                let mut f = *f;
                f.kg *= scale;
                f
            })
            .collect(),
        bulk: d.bill.bulk.iter().map(|&(m, kg)| (m, kg * scale)).collect(),
        consumed: d
            .bill
            .joints
            .iter()
            .chain(&d.bill.coatings)
            .chain(&d.bill.fluids)
            .chain(&d.bill.trace)
            .map(|&(m, kg)| (m, kg * scale))
            .collect(),
        ..Default::default()
    };
    for c in &d.bill.components {
        let Some(child) = cat.get(c.def) else { continue };
        record.components.push(Installed {
            definition: c.def,
            quantity: Quantity::Count(c.count.max(1)),
            mass_kg: c.kg * scale,
            materials: child.materials.clone(),
            condition_at_install: Condition::fresh(),
            quality_at_install: Quality::default(),
            held_by: c.held_by,
        });
    }
    for (i, c) in record.components.iter().enumerate() {
        record.joints.push(Joint {
            method: c.held_by,
            joins: (i, i),
            fastener: None,
            accessible: true,
        });
    }
    Some(record)
}

/// A definition with nothing but a name and a mass — for tests that need
/// to prove the validator rejects one.
pub fn bare(name: &'static str, kg: f64) -> ItemDefinition {
    def(name, Family::Stock, Form::Rigid, Dims::new(0.1, 0.1, 0.1), kg, &[(Material::MildSteel, 1.0)])
}

/// **Every instance in one place, held by durable handle.**
///
/// The reason it is an `Arena` and not a `Vec`: an alternator taken out of
/// a lorry must be the same alternator that went in, with its own hours
/// and its own history, and an index into whatever vector this happened to
/// be cannot promise that.
#[derive(Debug, Default)]
pub struct Store {
    pub items: Arena<ItemInstance>,
    /// **Where each live item is.** Held here rather than on the instance
    /// because an `ItemInstance` that is not in a store is not anywhere —
    /// it is a value, not an object in the world — and a field that had to
    /// hold *something* is exactly how `Nowhere` came to exist.
    ///
    /// A `BTreeMap`, because a save must write in the same order every
    /// time.
    where_: BTreeMap<u64, Placement>,
    /// **What it is spoken for.** Kept apart from where it is, because a
    /// reserved board has not moved off its rack.
    status_: BTreeMap<u64, WorkStatus>,
    graves: Vec<Tombstone>,
}

impl Store {
    pub fn new() -> Self {
        Store::default()
    }

    /// **Putting something into the world requires saying where.**
    ///
    /// There is no default destination: "the current ground" is a guess,
    /// and a work order that finishes a wardrobe somebody cannot carry
    /// has to be told what to do with it rather than quietly dropping it
    /// at the origin.
    pub fn add(&mut self, item: ItemInstance, at: Placement) -> Id<ItemInstance> {
        let id = self.items.add(item);
        self.where_.insert(id.bits(), at);
        if let Placement::Contained { container } = at {
            if let Some(c) = self.items.get_mut(container) {
                c.contents.push(id);
            }
        }
        id
    }

    /// On the floor, for a caller that genuinely does not care where.
    pub fn add_loose(&mut self, item: ItemInstance) -> Id<ItemInstance> {
        self.add(item, Placement::anywhere())
    }

    pub fn get(&self, id: Id<ItemInstance>) -> Option<&ItemInstance> {
        self.items.get(id)
    }

    pub fn get_mut(&mut self, id: Id<ItemInstance>) -> Option<&mut ItemInstance> {
        self.items.get_mut(id)
    }

    /// Put one thing inside another. The instance is not destroyed and
    /// recreated as a statistic — that is the whole point.
    pub fn put_in(
        &mut self,
        container: Id<ItemInstance>,
        thing: Id<ItemInstance>,
        cat: &Catalogue,
    ) -> Result<(), Refusal> {
        if container == thing {
            return Err(Refusal::WouldContainItself);
        }
        // **A move, not a copy.** Something fitted to a lorry or
        // committed to a work order is not also on a shelf.
        if self.is_installed(thing) {
            return Err(Refusal::AlreadyInstalled);
        }
        let (cdef, tdef, tmass) = {
            let c = self.items.get(container).ok_or(Refusal::NoSuchItem)?;
            let t = self.items.get(thing).ok_or(Refusal::NoSuchItem)?;
            (c.definition, t.definition, t.mass_kg)
        };
        let cd = cat.get(cdef).ok_or(Refusal::NoSuchItem)?;
        let td = cat.get(tdef).ok_or(Refusal::NoSuchItem)?;
        let used: f64 = {
            let c = self.items.get(container).unwrap();
            c.contents
                .iter()
                .filter_map(|&i| self.items.get(i))
                .map(|i| cat.get(i.definition).map(|d| d.volume_litres()).unwrap_or(0.0))
                .sum()
        };
        let pocket = cd
            .pockets
            .iter()
            .find(|p| {
                p.accepts_form(td.form)
                    && used + td.volume_litres() <= p.litres
                    && tmass <= p.max_kg
            })
            .ok_or(Refusal::NoRoom)?;
        let _ = pocket;
        self.place(thing, Placement::Contained { container });
        Ok(())
    }

    /// **Fitting a part does not consume it.** It is the same instance,
    /// with its wear, its faults and its maker, and taking it out again
    /// returns exactly that.
    pub fn install(
        &mut self,
        host: Id<ItemInstance>,
        part: Id<ItemInstance>,
        cat: &Catalogue,
    ) -> Result<Fitting, Refusal> {
        let (hdef, pdef) = {
            let h = self.items.get(host).ok_or(Refusal::NoSuchItem)?;
            let p = self.items.get(part).ok_or(Refusal::NoSuchItem)?;
            (h.definition, p.definition)
        };
        if !self.available(part) {
            return Err(Refusal::AlreadyInstalled);
        }
        let hd = cat.get(hdef).ok_or(Refusal::NoSuchItem)?;
        let pd = cat.get(pdef).ok_or(Refusal::NoSuchItem)?;
        let fitting = pd.fits.ok_or(Refusal::DoesNotFit)?;
        if !hd.attachment_points.contains(&fitting) {
            return Err(Refusal::DoesNotFit);
        }
        if self.items.get(host).unwrap().attachments.iter().any(|a| a.0 == fitting) {
            return Err(Refusal::PointTaken);
        }
        // **Atomic.** It leaves wherever it was and arrives here, and
        // there is no instant in between when it is in both.
        self.detach(part);
        let at = self.items.get(host).unwrap().attachments.len();
        self.items.get_mut(host).unwrap().attachments.push((fitting, part));
        self.where_
            .insert(part.bits(), Placement::Installed { host: Host::Item(host), mount: at });
        Ok(fitting)
    }

    /// Take it off again, onto the ground. Returns the same handle it was
    /// installed with.
    pub fn uninstall(
        &mut self,
        host: Id<ItemInstance>,
        fitting: Fitting,
    ) -> Result<Id<ItemInstance>, Refusal> {
        let h = self.items.get_mut(host).ok_or(Refusal::NoSuchItem)?;
        let at = h.attachments.iter().position(|a| a.0 == fitting).ok_or(Refusal::NothingThere)?;
        let (_, part) = h.attachments.remove(at);
        self.where_.insert(part.bits(), Placement::anywhere());
        Ok(part)
    }

    /// **Take it out of wherever it currently is.** The private half of
    /// every move: without it a part fitted to a lorry would still be
    /// listed in the crate it came out of.
    fn detach(&mut self, thing: Id<ItemInstance>) {
        let was = self.where_.remove(&thing.bits());
        match was {
            Some(Placement::Contained { container }) => {
                if let Some(c) = self.items.get_mut(container) {
                    c.contents.retain(|&x| x != thing);
                }
            }
            Some(Placement::Installed { host: Host::Item(host), .. }) => {
                if let Some(h) = self.items.get_mut(host) {
                    h.attachments.retain(|a| a.1 != thing);
                }
            }
            _ => {}
        }
    }

    /// Put it down somewhere, taking it out of wherever it was.
    pub fn place(&mut self, thing: Id<ItemInstance>, where_: Placement) {
        self.detach(thing);
        if !self.items.holds(thing) {
            return;
        }
        if let Placement::Contained { container } = where_ {
            if let Some(c) = self.items.get_mut(container) {
                c.contents.push(thing);
            }
        }
        self.where_.insert(thing.bits(), where_);
    }

    /// Where it is, if it is anywhere. `None` means it is not a live item
    /// — which is a different answer from "at the origin".
    pub fn placement(&self, thing: Id<ItemInstance>) -> Option<Placement> {
        self.where_.get(&thing.bits()).copied()
    }

    /// **Whether it can be picked up, fitted or consumed**, which needs
    /// both answers: it has to be somewhere it can be got at *and* not
    /// spoken for by somebody else.
    pub fn available(&self, thing: Id<ItemInstance>) -> bool {
        self.placement(thing).map(|p| p.reachable()).unwrap_or(false)
            && self.status(thing).free()
    }

    /// **What is standing on a machine.**
    ///
    /// A finished thing nobody has come for is still occupying the bay it
    /// was made in, which is exactly what makes a blocked delivery cost
    /// the shop its next job rather than costing it nothing. Asked of the
    /// store rather than of the calendar, because the object being there
    /// is a physical fact and the booking is only a plan about it.
    pub fn occupants_of(&self, resource: u32) -> Vec<Id<ItemInstance>> {
        self.where_
            .iter()
            .filter(|(_, p)| p.occupying() == Some(resource))
            .map(|(&bits, _)| Id::from_bits(bits))
            .collect()
    }

    /// Whether a particular position on a machine is clear.
    pub fn slot_free(&self, resource: u32, slot: u32) -> bool {
        !self.where_.values().any(|p| {
            matches!(p, Placement::Fixtured { resource: r, slot: s, .. }
                     if *r == resource && *s == slot)
        })
    }

    /// What it is spoken for. `Available` for anything nobody has claimed.
    pub fn status(&self, thing: Id<ItemInstance>) -> WorkStatus {
        self.status_.get(&thing.bits()).copied().unwrap_or_default()
    }

    /// **Claim it, or let it go.** Reserving does not move it.
    pub fn set_status(&mut self, thing: Id<ItemInstance>, status: WorkStatus) {
        if status.free() {
            self.status_.remove(&thing.bits());
        } else {
            self.status_.insert(thing.bits(), status);
        }
    }

    /// Why it cannot be had — where it is, or who has it.
    pub fn why_not(&self, thing: Id<ItemInstance>) -> &'static str {
        match self.placement(thing) {
            None => "it is not a live item",
            Some(p) if !p.reachable() => p.why_not(),
            _ => self.status(thing).why_not(),
        }
    }

    pub fn is_installed(&self, thing: Id<ItemInstance>) -> bool {
        self.placement(thing).map(|p| p.is_installed()).unwrap_or(false)
    }

    /// Whether it could be folded into a lot: its particulars must survive
    /// it, and it must be loose stock rather than fitted to something.
    pub fn aggregatable(&self, thing: Id<ItemInstance>) -> bool {
        self.items.get(thing).map(|i| i.aggregatable()).unwrap_or(false)
            && self.available(thing)
    }

    /// **It stopped being a live item.**
    ///
    /// Whatever came off it is separate objects placed separately; this
    /// only says that *this* one has gone, and leaves a tombstone so the
    /// books can still be checked.
    pub fn end(
        &mut self,
        thing: Id<ItemInstance>,
        end: ItemEnd,
        day: u32,
    ) -> Option<ItemInstance> {
        self.detach(thing);
        self.status_.remove(&thing.bits());
        let gone = self.items.remove(thing)?;
        self.graves.push(Tombstone {
            was: gone.definition,
            end,
            mass_kg: gone.mass_kg,
            on_day: day,
        });
        Some(gone)
    }

    /// Everything that has gone, and how.
    pub fn graves(&self) -> &[Tombstone] {
        &self.graves
    }

    /// How many live items there are.
    pub fn live(&self) -> usize {
        self.items.len()
    }

    /// Mass of a thing and everything in or on it.
    pub fn laden_mass(&self, id: Id<ItemInstance>) -> f64 {
        let Some(i) = self.items.get(id) else { return 0.0 };
        i.mass_kg
            + i.contents.iter().map(|&c| self.laden_mass(c)).sum::<f64>()
            + i.attachments.iter().map(|a| self.laden_mass(a.1)).sum::<f64>()
    }

    /// What capability a thing offers, its own or one of its attachments'.
    pub fn capability(
        &self,
        id: Id<ItemInstance>,
        want: Capability,
        cat: &Catalogue,
    ) -> Option<Provides> {
        let i = self.items.get(id)?;
        if i.effectiveness() <= 0.0 {
            return None;
        }
        let d = cat.get(i.definition)?;
        // A worn tool is slower and less precise, which is what makes
        // maintenance worth doing.
        let e = i.effectiveness();
        d.capability(want).map(|p| Provides {
            speed: p.speed * (0.4 + 0.6 * e),
            precision_mm: p.precision_mm / (0.4 + 0.6 * e),
            ..*p
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NoSuchItem,
    NoRoom,
    DoesNotFit,
    PointTaken,
    NothingThere,
    AlreadyInstalled,
    WouldContainItself,
}

// =====================================================================
// aggregate stock, for the parts of the world nobody is standing in
// =====================================================================

/// **A million toothbrushes are a number, not a million objects.**
///
/// A distant shop's ordinary stock is a lot; the drill somebody carries is
/// an instance. What must never be aggregated is anything whose
/// particulars would be destroyed by it — a named thing, a modified one, a
/// loaded gun, somebody's property, evidence, an active device, or work in
/// progress — because those are exactly the objects a story is made of.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemLot {
    pub definition: DefId,
    pub count: u32,
    pub total_mass_kg: f64,
    /// Mean and spread, so expanding a lot does not make every example
    /// identical.
    pub quality_mean: f64,
    pub quality_spread: f64,
    pub condition_mean: f64,
    pub condition_spread: f64,
    pub mean_age_days: f64,
    pub provenance: Provenance,
}

impl ItemLot {
    /// Fold instances into a lot. Refuses any that must stay particular,
    /// and hands them back rather than silently dropping them.
    pub fn aggregate(
        definition: DefId,
        instances: Vec<ItemInstance>,
        today: u32,
    ) -> (Option<ItemLot>, Vec<ItemInstance>) {
        let mut kept = Vec::new();
        let mut folded: Vec<ItemInstance> = Vec::new();
        for i in instances {
            if i.definition == definition && i.aggregatable() {
                folded.push(i);
            } else {
                kept.push(i);
            }
        }
        if folded.is_empty() {
            return (None, kept);
        }
        let n = folded.len() as f64;
        let mass: f64 = folded.iter().map(|i| i.mass_kg).sum();
        let q: Vec<f64> = folded.iter().map(|i| i.quality.overall()).collect();
        let c: Vec<f64> = folded.iter().map(|i| i.condition.serviceability()).collect();
        let age: f64 = folded
            .iter()
            .map(|i| (today.saturating_sub(i.provenance.made_on_day)) as f64)
            .sum::<f64>()
            / n;
        let mean = |v: &[f64]| v.iter().sum::<f64>() / n;
        let spread = |v: &[f64], m: f64| (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / n).sqrt();
        let qm = mean(&q);
        let cm = mean(&c);
        let count: u32 = folded
            .iter()
            .map(|i| match i.quantity {
                Quantity::Count(n) => n,
                Quantity::Stock { count, .. } => count,
                _ => 1,
            })
            .sum();
        (
            Some(ItemLot {
                definition,
                count,
                total_mass_kg: mass,
                quality_mean: qm,
                quality_spread: spread(&q, qm),
                condition_mean: cm,
                condition_spread: spread(&c, cm),
                mean_age_days: age,
                provenance: folded[0].provenance,
            }),
            kept,
        )
    }

    /// **Expansion is deterministic**, keyed by the lot rather than drawn
    /// from a running stream, so looking at a warehouse twice does not
    /// produce two different warehouses. Count and mass are conserved
    /// exactly; the last example takes the rounding.
    pub fn expand(&self, cat: &Catalogue, seed: u64) -> Vec<ItemInstance> {
        let n = self.count.max(1);
        let mut out = Vec::with_capacity(n as usize);
        let mut assigned = 0.0;
        for k in 0..n {
            let h = crate::save::channel(seed, self.definition.0 as u64, "lot expansion")
                ^ (k as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let mut r = crate::rng::Rng::new(h);
            // Two draws about the mean, clamped, so a lot of middling
            // stock expands to middling stock and not to a uniform.
            let jitter = |r: &mut crate::rng::Rng, mean: f64, sd: f64| {
                let a = r.next_f32() as f64;
                let b = r.next_f32() as f64;
                (mean + (a + b - 1.0) * sd * 1.732).clamp(0.0, 1.0)
            };
            let q = jitter(&mut r, self.quality_mean, self.quality_spread);
            let c = jitter(&mut r, self.condition_mean, self.condition_spread);
            let each_mass = if k + 1 == n {
                self.total_mass_kg - assigned
            } else {
                self.total_mass_kg / n as f64
            };
            assigned += each_mass;
            let mut item = ItemInstance::fresh(cat, self.definition, Quantity::Count(1));
            item.mass_kg = each_mass;
            item.quality = Quality {
                workmanship: q,
                structural_integrity: q,
                dimensional_accuracy: q,
                finish: q,
                contamination: 0.0,
                domain: DomainQuality::None,
            };
            item.condition = Condition { wear: 1.0 - c, ..Condition::fresh() };
            item.provenance = self.provenance;
            out.push(item);
        }
        out
    }
}
