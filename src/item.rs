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

use crate::id::{Arena, Id};
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
    /// Bricks in mortar, a wall.
    Mortared,
}

impl JointMethod {
    /// **What comes back**: the fraction of each joined component
    /// recovered intact by careful disassembly, and whether the joining
    /// material itself survives.
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
            Mortared => Recovery { components: 0.60, fastener: 0.0, needs_cutting: true },
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
    pub components: Vec<Installed>,
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
        for &(m, kg) in &self.consumed {
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
    pub nominal_mass_kg: f64,
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
}

impl ItemDefinition {
    pub fn volume_litres(&self) -> f64 {
        self.nominal.litres()
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

    pub fn get(&self, id: DefId) -> Option<&ItemDefinition> {
        self.defs.get(id.0 as usize)
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
    ItemDefinition {
        id: DefId(0),
        name,
        family,
        form,
        nominal,
        nominal_mass_kg: kg,
        materials: Composition::of(materials),
        provides: Vec::new(),
        pockets: Vec::new(),
        attachment_points: Vec::new(),
        fits: None,
        lifecycle: family.lifecycle(),
        repair_with: Vec::new(),
        consumable: matches!(family, Family::Stock | Family::Fastening | Family::Ammunition),
    }
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
                        d(0.45, 0.45, 0.9), 5.0, &[(Oak, 0.96), (MildSteel, 0.03), (Adhesive, 0.01)]);
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
    let mut rifle = def("rifle", Family::Firearm, Form::Assembly, d(0.9, 0.06, 0.22), 3.0,
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

    // ---- things that exist only part way through making something ----
    // **An intermediate is instantiated when it can be moved, traded,
    // spoil, be reused elsewhere, need its own storage, or be left
    // stranded by an interrupted process.** Dough can go off and cut
    // panels can be used for something else, so both are real. A step that
    // merely leaves a workpiece a bit further along gets no object.
    c.add(def("chair parts", Family::Stock, Form::Rigid, d(1.0, 0.5, 0.2), 4.8,
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

    c
}

// =====================================================================
// instances
// =====================================================================

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
    /// Where it is installed, if it is fitted to something.
    pub installed_in: Option<Id<ItemInstance>>,
    pub assembly: Option<AssemblyRecord>,
    pub provenance: Provenance,
    pub ownership: Ownership,
    /// A name somebody gave it. Named things are never aggregated.
    pub given_name: Option<String>,
}

impl ItemInstance {
    /// A factory-fresh example of a definition.
    pub fn fresh(cat: &Catalogue, id: DefId, quantity: Quantity) -> Self {
        let d = cat.get(id).expect("unknown definition");
        let mass = quantity.mass_kg(d.nominal_mass_kg);
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
            installed_in: None,
            assembly: None,
            provenance: Provenance::default(),
            ownership: Ownership::default(),
            given_name: None,
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

    pub fn is_installed(&self) -> bool {
        self.installed_in.is_some()
    }

    /// Whether this is an example that may be folded into a lot, or one
    /// whose particulars would be destroyed by it.
    pub fn aggregatable(&self) -> bool {
        self.given_name.is_none()
            && !self.provenance.marked
            && !self.ownership.accounted_for
            && self.contents.is_empty()
            && self.attachments.is_empty()
            && self.installed_in.is_none()
            && self.faults.is_empty()
            && self.assembly.as_ref().map(|a| a.substitutions.is_empty()).unwrap_or(true)
    }
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
}

impl Store {
    pub fn new() -> Self {
        Store::default()
    }

    pub fn add(&mut self, item: ItemInstance) -> Id<ItemInstance> {
        self.items.add(item)
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
        self.items.get_mut(container).unwrap().contents.push(thing);
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
            if p.installed_in.is_some() {
                return Err(Refusal::AlreadyInstalled);
            }
            (h.definition, p.definition)
        };
        let hd = cat.get(hdef).ok_or(Refusal::NoSuchItem)?;
        let pd = cat.get(pdef).ok_or(Refusal::NoSuchItem)?;
        let fitting = pd.fits.ok_or(Refusal::DoesNotFit)?;
        if !hd.attachment_points.contains(&fitting) {
            return Err(Refusal::DoesNotFit);
        }
        if self.items.get(host).unwrap().attachments.iter().any(|a| a.0 == fitting) {
            return Err(Refusal::PointTaken);
        }
        self.items.get_mut(host).unwrap().attachments.push((fitting, part));
        self.items.get_mut(part).unwrap().installed_in = Some(host);
        Ok(fitting)
    }

    /// Take it off again. Returns the same handle it was installed with.
    pub fn uninstall(
        &mut self,
        host: Id<ItemInstance>,
        fitting: Fitting,
    ) -> Result<Id<ItemInstance>, Refusal> {
        let h = self.items.get_mut(host).ok_or(Refusal::NoSuchItem)?;
        let at = h.attachments.iter().position(|a| a.0 == fitting).ok_or(Refusal::NothingThere)?;
        let (_, part) = h.attachments.remove(at);
        if let Some(p) = self.items.get_mut(part) {
            p.installed_in = None;
        }
        Ok(part)
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
