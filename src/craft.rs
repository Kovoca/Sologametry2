//! **A recipe is a plan, not the identity of a thing.**
//!
//! There is more than one way to make a wooden table — a factory, a
//! cabinet shop, a man with hand tools, an improvised bodge, or the
//! restoration of a broken one — and all five yield a wooden table while
//! differing in components, machinery, labour, energy, time, precision,
//! waste, expected quality and what you have to know. A recipe that *is*
//! the item cannot say any of that, and a crafting menu that turns
//! ingredients into an object after a timer cannot say any of it either.
//!
//! **Work time is not elapsed time**, and the economy will overstate
//! industrial employment badly until they are separate. Baking a loaf is
//! 35 minutes of labour, 35 minutes of oven occupancy and 125 minutes on
//! the clock — not 125 employee-minutes.
//!
//! **And an operation consumes what it uses, when it uses it.** Take the
//! whole bill of materials at the start and a power cut half way through
//! destroys steel that has already been cut into blanks. The blanks and
//! the offcuts are real, and they are still there in the morning.

use crate::id::Id;
use crate::item::{
    AssemblyRecord, Capability, Catalogue, Condition, DefId, DomainQuality, Family, Installed,
    ItemInstance, Joint, JointMethod, Placement, Provides, Quality, Store, Substitution,
};
use crate::material::{Amount, Composition, Fit, Material, Quantity};
use crate::rng::Rng;
use crate::save::channel;

// =====================================================================
// operations
// =====================================================================

/// **A shared vocabulary of operations**, so that every recipe does not
/// invent its own meaning for cutting or welding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Operation {
    Measure,
    Cut,
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
    Assemble,
    Test,
    Package,
    Mix,
    Bake,
    /// Sizing, swaging, crimping, stamping.
    Press,
    /// Rising, curing, drying, cooling. Time passes and nobody is needed.
    Rest,
}

/// Which part of the finished thing an operation decides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bears {
    Accuracy,
    Structure,
    Finish,
    Cleanliness,
    /// Checking rather than doing: it does not improve the work, it
    /// catches what is wrong with it.
    Inspection,
}

impl Operation {
    pub fn bears_on(self) -> Bears {
        use Operation::*;
        match self {
            Cut | Drill | Turn | Mill | Measure | Cast | Press => Bears::Accuracy,
            Weld | Solder | Sew | Fasten | Glue | Forge | Assemble | HeatTreat => Bears::Structure,
            Grind | Paint | Package => Bears::Finish,
            Sterilise | Mix | Bake => Bears::Cleanliness,
            Test => Bears::Inspection,
            Rest => Bears::Structure,
        }
    }

    pub fn name(self) -> &'static str {
        use Operation::*;
        match self {
            Measure => "measure",
            Cut => "cut",
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
            Assemble => "assemble",
            Test => "test",
            Package => "package",
            Mix => "mix",
            Bake => "bake",
            Press => "press",
            Rest => "rest",
        }
    }
}

// =====================================================================
// what a step wants and what it moves
// =====================================================================

/// **A capability with a figure on it**, not a named tool. Anything that
/// answers may be used, and what differs between the answers is speed,
/// waste, precision, power and how big a piece will fit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Need {
    pub capability: Capability,
    pub capacity: f64,
    pub precision_mm: f64,
}

impl Need {
    pub fn of(capability: Capability, capacity: f64, precision_mm: f64) -> Self {
        Need { capability, capacity, precision_mm }
    }
}

/// Something moving into or out of an operation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Flow {
    /// So many of a definition.
    Item { def: DefId, count: f64 },
    /// Loose material by mass: flour, sand, welding wire off a spool.
    Bulk { material: Material, kg: f64 },
    /// Offcuts, swarf, sawdust. Has a material, so recycling can ask.
    Waste { material: Material, kg: f64 },
    /// Steam, solvent, fume. Leaves the system entirely.
    Emission { material: Material, kg: f64 },
    /// Drawn from the surroundings rather than from stock — tap water,
    /// air. Counted, so the balance still closes.
    Environment { material: Material, kg: f64 },
}

impl Flow {
    pub fn mass_kg(self, cat: &Catalogue) -> f64 {
        match self {
            Flow::Item { def, count } => {
                cat.get(def).map(|d| d.nominal_mass_kg).unwrap_or(0.0) * count
            }
            Flow::Bulk { kg, .. }
            | Flow::Waste { kg, .. }
            | Flow::Emission { kg, .. }
            | Flow::Environment { kg, .. } => kg,
        }
    }

    pub fn material(self, cat: &Catalogue) -> Option<Material> {
        match self {
            Flow::Item { def, .. } => cat.get(def).and_then(|d| d.materials.chiefly()),
            Flow::Bulk { material, .. }
            | Flow::Waste { material, .. }
            | Flow::Emission { material, .. }
            | Flow::Environment { material, .. } => Some(material),
        }
    }
}

/// **What a step costs in the three different kinds of time.**
///
/// Keeping them apart is what stops a bakery employing three times the
/// people it needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effort {
    /// Somebody is doing it the whole time.
    Hands { minutes: f64 },
    /// A machine is occupied; somebody loads and unloads it and is
    /// otherwise free.
    Machine { minutes: f64, tending: f64 },
    /// Rising, curing, drying, cooling. Nobody is needed at all.
    Unattended { minutes: f64 },
}

impl Effort {
    pub fn labour_minutes(self) -> f64 {
        match self {
            Effort::Hands { minutes } => minutes,
            Effort::Machine { tending, .. } => tending,
            Effort::Unattended { .. } => 0.0,
        }
    }

    pub fn machine_minutes(self) -> f64 {
        match self {
            Effort::Machine { minutes, .. } => minutes,
            _ => 0.0,
        }
    }

    /// How long it takes at nominal rate, whoever is or is not watching.
    pub fn span_minutes(self) -> f64 {
        match self {
            Effort::Hands { minutes } => minutes,
            Effort::Machine { minutes, .. } => minutes,
            Effort::Unattended { minutes } => minutes,
        }
    }

    /// Whether the work stops when the last worker walks away.
    pub fn needs_somebody(self) -> bool {
        self.labour_minutes() > 0.0
    }
}

/// One named operation in a plan.
#[derive(Clone, Debug)]
pub struct Step {
    pub name: &'static str,
    pub operation: Operation,
    pub effort: Effort,
    pub needs: Vec<Need>,
    pub consumes: Vec<Flow>,
    pub produces: Vec<Flow>,
    /// The object this step leaves behind, when the state it reaches can
    /// be moved, traded, spoil, be reused or be stranded. A step that only
    /// gets the workpiece further along leaves nothing.
    pub leaves: Option<DefId>,
    /// A blackout stops it. False for anything done by hand — which is
    /// why a grid failure shuts a mill within the hour and the joiner
    /// carries on.
    pub needs_power: bool,
    /// If this step joins things, how — which is what disassembly reads.
    pub joins: Option<JointMethod>,
    /// Real process hazards: primer and propellant work, hot metal, dust.
    pub hazard: f64,
    /// **What shape of stock the operation actually needs.** Mass is a
    /// conservation check, not a fit: measured by weight alone a
    /// requirement for a board is met by a batten too narrow to cut a seat
    /// from and by an offcut too short to cut a leg from, both of which
    /// weigh exactly what a usable board weighs.
    pub shapes: Vec<(DefId, Amount)>,
}

impl Step {
    pub fn new(name: &'static str, operation: Operation, effort: Effort) -> Self {
        Step {
            name,
            operation,
            effort,
            needs: Vec::new(),
            consumes: Vec::new(),
            produces: Vec::new(),
            leaves: None,
            needs_power: false,
            joins: None,
            hazard: 0.0,
            shapes: Vec::new(),
        }
    }

    pub fn needing(mut self, needs: &[Need]) -> Self {
        self.needs = needs.to_vec();
        self.needs_power = self.needs_power || matches!(self.effort, Effort::Machine { .. });
        self
    }

    pub fn taking(mut self, consumes: &[Flow]) -> Self {
        self.consumes = consumes.to_vec();
        self
    }

    pub fn giving(mut self, produces: &[Flow]) -> Self {
        self.produces = produces.to_vec();
        self
    }

    pub fn leaving(mut self, def: DefId) -> Self {
        self.leaves = Some(def);
        self
    }

    pub fn powered(mut self) -> Self {
        self.needs_power = true;
        self
    }

    pub fn joined(mut self, method: JointMethod) -> Self {
        self.joins = Some(method);
        self
    }

    pub fn dangerous(mut self, hazard: f64) -> Self {
        self.hazard = hazard;
        self
    }

    /// What shape each of its inputs has to be in.
    pub fn shaped(mut self, shapes: &[(DefId, Amount)]) -> Self {
        self.shapes = shapes.to_vec();
        self
    }
}

// =====================================================================
// what somebody has to know
// =====================================================================

/// Broad ability, transferable between related tasks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Skill {
    Fabrication,
    Woodworking,
    Tailoring,
    Cooking,
    Electronics,
    Gunsmithing,
    Mechanics,
}

/// **Familiarity with one operation family**, which is a different thing.
/// A trained machinist who has never welded is not unskilled; he is slow
/// and his beads are poor until he has done a few.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Proficiency {
    None,
    Welding,
    Joinery,
    Sewing,
    Baking,
    Soldering,
    Handloading,
    Machining,
}

/// **Who is doing it, and what they can bring to it today.**
///
/// Five separate things, because a model with one "crafting level" cannot
/// state the ordinary cases: knowing the sequence without the control to
/// execute it, having the control but not this particular machine, or
/// being perfectly capable and too tired to be trusted with a press.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Maker {
    /// Broad ability at the trade, 0..1.
    pub skill: f64,
    /// Familiarity with this operation family, 0..1.
    pub proficiency: f64,
    /// Whether they know this particular plan at all.
    pub knows_recipe: bool,
    /// Experience with the actual machine, 0..1. A first day on an
    /// unfamiliar lathe is slower and no less skilled.
    pub tool_familiarity: f64,
    /// **The join to `mind.rs`.** Focus decides precision and mistakes;
    /// fatigue decides sustained output. Neither is a skill.
    pub focus: f64,
    pub fatigue: f64,
}

impl Default for Maker {
    fn default() -> Self {
        Maker {
            skill: 0.6,
            proficiency: 0.5,
            knows_recipe: true,
            tool_familiarity: 0.7,
            focus: 0.8,
            fatigue: 0.2,
        }
    }
}

impl Maker {
    /// How fast they work, against a competent hand.
    pub fn pace(&self) -> f64 {
        let base = 0.35 + 0.65 * self.skill;
        let known = 0.6 + 0.4 * self.tool_familiarity;
        let tired = 1.0 - 0.35 * self.fatigue.clamp(0.0, 1.0);
        (base * known * tired).max(0.05)
    }

    /// **How often an operation goes as intended.**
    ///
    /// Anchored on real first-pass yield rather than assembled out of
    /// multipliers: manufacturing scrap and rework run **1-5%** of
    /// operations, first-pass yield above 95% is ordinary and 99%+ is
    /// called world class. A competent person doing a familiar job gets
    /// it right nearly every time, and a model that has them botching a
    /// third of what they touch is not modelling work.
    pub fn care(&self) -> f64 {
        (0.70 + 0.20 * self.skill + 0.07 * self.proficiency + 0.03 * self.focus)
            .clamp(0.55, 0.995)
    }
}

// =====================================================================
// the plan
// =====================================================================

/// **One valid way of producing something.**
#[derive(Clone, Debug)]
pub struct RecipeDefinition {
    pub name: &'static str,
    pub result: DefId,
    /// How many the plan makes in one run.
    pub yields: u32,
    pub skill: Skill,
    pub proficiency: Proficiency,
    /// 0..1. What a competent person is up against.
    pub difficulty: f64,
    /// Whether it can be worked out or has to be taught or read.
    pub needs_knowledge: bool,
    pub steps: Vec<Step>,
    /// What may stand in for what. The substitution is *recorded*, which
    /// is what stops particleboard coming back out as oak.
    pub substitutions: Vec<(DefId, Vec<DefId>)>,
    /// Once per run whatever the batch size — jigs set, machine tooled,
    /// oven brought to temperature. This and only this is what a batch
    /// saves.
    pub setup_minutes: f64,
}

impl RecipeDefinition {
    pub fn labour_minutes(&self) -> f64 {
        self.steps.iter().map(|s| s.effort.labour_minutes()).sum()
    }

    pub fn machine_minutes(&self) -> f64 {
        self.steps.iter().map(|s| s.effort.machine_minutes()).sum()
    }

    /// Wall-clock time for one run at nominal rate, before anybody's pace
    /// or any parallel stations are considered.
    pub fn span_minutes(&self) -> f64 {
        self.setup_minutes + self.steps.iter().map(|s| s.effort.span_minutes()).sum::<f64>()
    }

    /// **What a batch actually saves**: setup, once, rather than per unit.
    /// Per-unit labour and per-unit material are untouched, which is why a
    /// factory is cheaper per item and not free.
    pub fn labour_for_batch(&self, units: u32) -> f64 {
        let runs = (units as f64 / self.yields.max(1) as f64).ceil();
        self.setup_minutes + runs * self.labour_minutes()
    }

    /// Everything the plan draws in, over all its steps.
    pub fn all_consumed(&self) -> Vec<Flow> {
        self.steps.iter().flat_map(|s| s.consumes.iter().copied()).collect()
    }

    pub fn all_produced(&self) -> Vec<Flow> {
        self.steps.iter().flat_map(|s| s.produces.iter().copied()).collect()
    }

    /// **A tool is used, not eaten.** Anything in `consumes` that is a
    /// tool or a machine is a mistake in the recipe, and it is worth
    /// asking the question rather than trusting the author.
    pub fn consumes_a_tool(&self, cat: &Catalogue) -> Option<DefId> {
        self.all_consumed().iter().find_map(|f| match f {
            Flow::Item { def, .. } => {
                let d = cat.get(*def)?;
                (matches!(d.family, Family::Tool | Family::Machine) && !d.consumable)
                    .then_some(*def)
            }
            _ => None,
        })
    }

    /// The mass the product must come out at if nothing is created or
    /// destroyed: everything in, less waste, emissions and byproducts.
    pub fn implied_product_mass(&self, cat: &Catalogue) -> f64 {
        let mut m = 0.0;
        for f in self.all_consumed() {
            // An intermediate this same plan made is not a fresh input —
            // it is the workpiece coming back, and counting it twice
            // would double the mass.
            if self.leaves_anywhere(f) {
                continue;
            }
            m += f.mass_kg(cat);
        }
        for f in self.all_produced() {
            m -= f.mass_kg(cat);
        }
        m / self.yields.max(1) as f64
    }

    fn leaves_anywhere(&self, f: Flow) -> bool {
        match f {
            Flow::Item { def, .. } => self.steps.iter().any(|s| s.leaves == Some(def)),
            _ => false,
        }
    }

    /// What shape of stock the plan wants where it calls for a given
    /// definition, if it has an opinion.
    pub fn shape_wanted(&self, def: DefId) -> Option<Amount> {
        self.steps.iter().find_map(|s| s.shapes.iter().find(|p| p.0 == def).map(|p| p.1))
    }

    /// The alternatives the plan's author had in mind. Advisory: what
    /// actually decides a substitution is whether the stock fits, not
    /// whether somebody wrote it down.
    pub fn suggested_substitutes(&self, def: DefId) -> &[DefId] {
        self.substitutions
            .iter()
            .find(|p| p.0 == def)
            .map(|p| p.1.as_slice())
            .unwrap_or(&[])
    }

    /// Whether every one of the plan's own needs can be met by these
    /// tools — which is what makes the same plan runnable by a man with a
    /// handsaw and by a factory.
    pub fn workable_with(&self, tools: &[Provides]) -> bool {
        self.steps.iter().all(|s| {
            s.needs
                .iter()
                .all(|n| tools.iter().any(|t| t.satisfies(n.capability, n.capacity, n.precision_mm)))
        })
    }
}

/// Every plan the world knows.
#[derive(Clone, Debug, Default)]
pub struct RecipeBook {
    pub recipes: Vec<RecipeDefinition>,
}

impl RecipeBook {
    pub fn add(&mut self, r: RecipeDefinition) -> usize {
        self.recipes.push(r);
        self.recipes.len() - 1
    }

    pub fn get(&self, i: usize) -> Option<&RecipeDefinition> {
        self.recipes.get(i)
    }

    pub fn named(&self, name: &str) -> Option<usize> {
        self.recipes.iter().position(|r| r.name == name)
    }

    pub fn must(&self, name: &str) -> usize {
        self.named(name).unwrap_or_else(|| panic!("no such recipe: {name}"))
    }

    /// **Several ways to make the same thing.**
    pub fn ways_to_make(&self, result: DefId) -> Vec<usize> {
        self.recipes
            .iter()
            .enumerate()
            .filter(|(_, r)| r.result == result)
            .map(|(i, _)| i)
            .collect()
    }
}

// =====================================================================
// what can go wrong
// =====================================================================

/// **Failure happens to an operation, not to the whole object.**
///
/// One final roll saying the thing either exists or does not is what makes
/// crafting a slot machine. A poor weld is a weak weld; a failed cake is
/// often still edible; a badly loaded cartridge is dangerous ammunition
/// rather than a pile of generic scrap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mishap {
    None,
    /// It took longer than it should have.
    Slow(f64),
    /// More material went into it than the plan allows for.
    Overrun(f64),
    /// Caught in time; the step has to be done again.
    Rework,
    DimensionalDefect,
    CosmeticFlaw,
    WeakJoint,
    DamagedComponent,
    RuinedComponent,
    ToolDamage,
    Injury,
    Contamination,
    Fire,
    /// The work is gone.
    CatastrophicLoss,
}

impl Mishap {
    pub fn is_none(self) -> bool {
        matches!(self, Mishap::None)
    }

    pub fn ends_the_order(self) -> bool {
        matches!(self, Mishap::CatastrophicLoss)
    }

    pub fn name(self) -> &'static str {
        use Mishap::*;
        match self {
            None => "went as intended",
            Slow(_) => "took longer",
            Overrun(_) => "used more material",
            Rework => "had to be done again",
            DimensionalDefect => "came out off-size",
            CosmeticFlaw => "came out untidy",
            WeakJoint => "left a weak joint",
            DamagedComponent => "damaged a component",
            RuinedComponent => "ruined a component",
            ToolDamage => "damaged the tool",
            Injury => "hurt somebody",
            Contamination => "contaminated the work",
            Fire => "caught fire",
            CatastrophicLoss => "destroyed the work",
        }
    }
}

/// **What an operation came to.**
///
/// Three destinations, not two, and they are the three the trade actually
/// counts. A unit that needed a touch-up is not a unit that passed, and it
/// is emphatically not a unit that was thrown away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grade {
    /// Passed with no correction and no rework. This and only this is what
    /// first-pass yield counts.
    Accepted,
    /// Wrong, and putting it right is cheaper than starting again.
    Reworkable,
    /// Gone.
    Scrapped,
}

/// **First-pass yield, rework and scrap are three different numbers.**
///
/// ASQ defines first-pass yield as the share of units passing *without
/// correction or rework*, and NIST lists first-pass yield, scrap ratio and
/// rework ratio as separate manufacturing indicators — because a process
/// can perfectly well run
///
/// ```text
/// FPY       92%
/// reworked   7%
/// scrapped   1%
/// ```
///
/// So **"1-5% scrap" implies nothing whatever about first-pass yield**,
/// and a note saying otherwise (this file used to) is conflating a
/// throughput measure with a loss measure. They are kept apart here so
/// that neither can quietly stand in for the other.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Yields {
    pub first_pass: f64,
    pub rework: f64,
    pub scrap: f64,
}

impl Yields {
    pub fn defect_rate(&self) -> f64 {
        self.rework + self.scrap
    }

    /// Where a drawn number in `[0, 1)` lands.
    pub fn grade(&self, x: f64) -> Grade {
        if x < self.first_pass {
            Grade::Accepted
        } else if x < self.first_pass + self.rework {
            Grade::Reworkable
        } else {
            Grade::Scrapped
        }
    }
}

/// **The influences combine, and then the outcome is calculated once.**
///
/// The bug this replaces was multiplying four uncalibrated modifiers
/// straight against the chance of success, which compounds to nonsense —
/// the same error this project already made over prices. The fix is not to
/// move the multiplication onto the defect rate (which is still four
/// multipliers); it is to add the contributions into one capability and
/// map that, once, into three rates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProcessCapability {
    /// What the process is worth before anybody touches it.
    pub baseline: f64,
    pub worker: f64,
    pub tool: f64,
    pub workplace: f64,
    /// Stock that is out of specification, warped, damp or substituted
    /// makes every operation on it harder. A hook with a real consumer
    /// once `Amount` refuses a bad fit outright.
    pub material: f64,
    pub difficulty: f64,
}

impl ProcessCapability {
    pub fn effective(&self) -> f64 {
        (self.baseline + self.worker + self.tool + self.workplace + self.material
            - self.difficulty)
            .clamp(-1.5, 1.5)
    }

    /// **Anchored on an ordinary shop.** At capability zero — a competent
    /// hand, an adequate tool, a job of middling difficulty — the defect
    /// rate is 5%, of which about a quarter is scrapped and the rest put
    /// right. Capability climbs and falls it eightfold per point.
    pub fn yields(&self) -> Yields {
        let e = self.effective();
        let defect = (0.05 * 8.0f64.powf(-e)).clamp(0.001, 0.60);
        // A better-run process does not merely make fewer mistakes; it
        // catches more of them at a stage where they can still be put
        // right. Bad shops scrap what good shops rework.
        let scrapped_share = (0.26 - 0.14 * e).clamp(0.08, 0.45);
        let scrap = defect * scrapped_share;
        Yields { first_pass: 1.0 - defect, rework: defect - scrap, scrap }
    }
}

/// **A rework is another go, not an unlimited one.** Past a few attempts
/// the piece is scrap and somebody has to admit it.
pub const MAX_ATTEMPTS: u32 = 3;

/// **Everything that bears on the work, added up.**
///
/// Each contribution is a shift in capability rather than a multiplier on
/// the outcome, so a poor tool and a poor hand are additively poor and not
/// catastrophically so, and the three yields are computed once from the
/// total.
///
/// The ranges are the model, and they are chosen so that an ordinary
/// competent hand at an ordinary bench on a job of middling difficulty
/// lands near zero — which is where the 5% defect anchor sits.
pub fn capability_of(
    recipe: &RecipeDefinition,
    step: &Step,
    tools: &[Provides],
    place: &Workplace,
    maker: Maker,
) -> ProcessCapability {
    // -0.5 .. +0.5. Skill dominates; proficiency with this operation
    // family and today's attention are real and smaller.
    let hand = 0.60 * maker.skill + 0.25 * maker.proficiency + 0.15 * maker.focus;
    let worker = (hand - 0.5) * 1.0 - 0.15 * maker.fatigue;

    // -0.3 .. +0.3. Room in hand on the tolerance, not raw capacity: a
    // tool working at its limit is where dimensional defects come from.
    let tool = (worst_precision(tools, &step.needs) - 0.7) * 1.0;

    // 0 .. +0.25. A jig is what holds a tolerance without a craftsman
    // holding it.
    let workplace = 0.25 * place.jigs;

    ProcessCapability {
        baseline: 0.0,
        worker: worker.clamp(-0.5, 0.5),
        tool: tool.clamp(-0.3, 0.3),
        workplace,
        material: 0.0,
        difficulty: 0.70 * recipe.difficulty,
    }
}

/// **Yields compound down a sequence of operations.**
///
/// `RTY = prod(FPY_i)`, which is why twenty operations at 99% each deliver
/// only 81.8% of units clean through the line. This is not the compounding
/// *bug* — it is the arithmetic the bug was hiding, and a plan with many
/// steps is genuinely harder to get right first time than a plan with two.
pub fn rolled_throughput_yield(first_pass: f64, operations: usize) -> f64 {
    first_pass.powi(operations as i32)
}

/// **Keyed to the order, the step and the attempt**, never to a running
/// stream — so reloading a save cannot reroll a failure, and adding a draw
/// somewhere else tomorrow cannot shift this one.
fn roll_outcome(
    order: u64,
    step: usize,
    attempt: u32,
    yields: Yields,
    hazard: f64,
) -> (Grade, Mishap) {
    let h = channel(order, step as u64, "operation outcome")
        ^ (attempt as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut r = Rng::new(h);
    let x = r.next_f32() as f64;
    let grade = yields.grade(x);
    let k = r.next_f32() as f64;

    // **Taking longer is not a defect.** A job can run over and come out
    // perfect, so the schedule draw is independent of the quality one and
    // an accepted unit can still have been slow or wasteful.
    let variance = |k: f64| -> Mishap {
        if k < 0.06 {
            Mishap::Slow(1.0 + k * 5.0)
        } else if k < 0.09 {
            Mishap::Overrun(0.04 + k * 0.3)
        } else {
            Mishap::None
        }
    };

    match grade {
        Grade::Accepted => (grade, variance(k)),
        Grade::Reworkable => {
            // Hazardous work turns a mistake into an injury far more
            // readily. Handloading is the reason this exists: a failure
            // that quietly consumes another unit and lets the worker carry
            // on is not what happens when primers go off.
            if k < hazard {
                return (grade, if k < hazard * 0.3 { Mishap::Fire } else { Mishap::Injury });
            }
            let m = match ((k - hazard) / (1.0 - hazard).max(1e-6)).clamp(0.0, 1.0) {
                s if s < 0.28 => Mishap::CosmeticFlaw,
                s if s < 0.52 => Mishap::Rework,
                s if s < 0.72 => Mishap::DimensionalDefect,
                s if s < 0.88 => Mishap::WeakJoint,
                s if s < 0.96 => Mishap::Contamination,
                _ => Mishap::ToolDamage,
            };
            (grade, m)
        }
        Grade::Scrapped => {
            if k < hazard {
                return (grade, if k < hazard * 0.5 { Mishap::Fire } else { Mishap::Injury });
            }
            let m = match ((k - hazard) / (1.0 - hazard).max(1e-6)).clamp(0.0, 1.0) {
                s if s < 0.45 => Mishap::DamagedComponent,
                s if s < 0.85 => Mishap::RuinedComponent,
                _ => Mishap::CatastrophicLoss,
            };
            (grade, m)
        }
    }
}

// =====================================================================
// the work order
// =====================================================================

/// What stopped the work, if anything has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Halt {
    Running,
    /// The clock is running and nobody is needed — curing, rising, drying.
    Unattended,
    NoPower,
    NoTool(Capability),
    NoWorker,
    /// Too cold or too hot for the step to proceed.
    OutOfRange,
    Done,
    Abandoned,
}

/// What the workplace can offer right now.
#[derive(Clone, Debug, Default)]
pub struct Workplace {
    pub power: bool,
    pub water: bool,
    pub celsius: f64,
    pub tools: Vec<Provides>,
    /// Full-time equivalents actually present.
    pub workers: f64,
    /// How many units can be worked on at once. A workshop has one bench;
    /// a factory has a line.
    pub stations: u32,
    /// Jigs and fixtures, 0..1. What holds a tolerance without a
    /// craftsman holding it.
    pub jigs: f64,
}

impl Workplace {
    /// A man with hand tools at a bench.
    pub fn a_workshop(tools: Vec<Provides>) -> Self {
        Workplace { power: true, water: true, celsius: 18.0, tools, workers: 1.0, stations: 1, jigs: 0.1 }
    }

    pub fn a_factory(tools: Vec<Provides>, workers: f64, stations: u32) -> Self {
        Workplace { power: true, water: true, celsius: 20.0, tools, workers, stations, jigs: 0.85 }
    }

    /// **In a blackout you reach for the handsaw.** A powered tool is not
    /// merely slower without power, it is not a tool at all — so what is
    /// available depends on what the grid is doing, and the same plan
    /// carries on in a workshop while it stops dead in a works.
    fn best_for(&self, n: Need) -> Option<Provides> {
        self.tools
            .iter()
            .filter(|t| t.satisfies(n.capability, n.capacity, n.precision_mm))
            .filter(|t| self.power || t.kw <= 0.0)
            .copied()
            .max_by(|a, b| a.speed.total_cmp(&b.speed))
    }
}

/// What one step actually did, kept so that the order can be reloaded and
/// asked what happened rather than being replayed.
#[derive(Clone, Debug, PartialEq)]
pub struct StepRecord {
    pub step: usize,
    pub attempt: u32,
    /// Whether it passed, needed putting right, or was thrown away.
    pub grade: Grade,
    /// What was wrong with it, or what merely cost time.
    pub mishap: Mishap,
    pub labour_minutes: f64,
    pub machine_minutes: f64,
    pub elapsed_minutes: f64,
}

/// **One actual attempt at a plan**, with everything it has used up so far
/// and everything it has left lying about.
#[derive(Clone, Debug)]
pub struct WorkOrder {
    pub id: u64,
    pub recipe: usize,
    pub intended: u32,
    pub workplace: u32,
    pub step: usize,
    /// How much of the current step is done, in step-minutes.
    pub step_done: f64,
    pub attempt: u32,
    pub setup_done: f64,
    pub completed: Vec<StepRecord>,
    /// Set aside for the order and not yet used. An interruption returns
    /// these.
    pub reserved: Vec<(DefId, f64)>,
    pub consumed_items: Vec<(DefId, f64)>,
    pub consumed_bulk: Vec<(Material, f64)>,
    pub from_environment: Vec<(Material, f64)>,
    /// **Real objects the order has produced part way through.** A power
    /// cut leaves these where they are.
    pub intermediates: Vec<(DefId, f64)>,
    pub waste: Vec<(Material, f64)>,
    pub emissions: Vec<(Material, f64)>,
    /// Departures from what the plan nominally asked for. **Recorded and
    /// applied**: a substitution that only decorated the paperwork would
    /// leave the recipe materials in the record, which is the
    /// transmutation exploit wearing a disguise.
    pub substitutions: Vec<Substitution>,
    pub active_labour_min: f64,
    pub machine_min: f64,
    pub unattended_min: f64,
    pub elapsed_min: f64,
    pub power_kwh: f64,
    pub state: Halt,
    /// **Where the finished thing goes.**
    ///
    /// Named when the order is raised, because "the current ground" is a
    /// guess and a wardrobe nobody can carry has to be put *somewhere*.
    /// If the destination cannot take it, completion waits.
    pub output: Placement,
    pub delivered: bool,
    pub started_day: u32,
    /// Running assessment of the work, one axis at a time.
    quality: Building,
}

/// One piece on the bench as the plan is walked. Not public: it is the
/// working state of `deliver`, and what comes out of it is the record.
#[derive(Clone, Debug)]
struct Part {
    def: Option<DefId>,
    count: f64,
    kg: f64,
    comp: Composition,
    held_by: Option<JointMethod>,
}

/// Sanding, trimming and cleaning take mass off the workpiece rather than
/// off any one component, so it comes off all of them in proportion.
fn shrink(parts: &mut [Part], lost: f64) {
    let total: f64 = parts.iter().map(|p| p.kg).sum();
    if total <= 0.0 || lost <= 0.0 {
        return;
    }
    let keep = ((total - lost) / total).clamp(0.0, 1.0);
    for p in parts.iter_mut() {
        p.kg *= keep;
    }
}

/// **Each axis is scored against the operations that bore on it.**
///
/// Dividing every axis by the total number of steps was wrong and looked
/// almost right: a plan with three cutting operations out of six came out
/// at 0.5 for dimensional accuracy however well every cut was made, and a
/// chair nobody was asked to polish scored half marks for finish. What is
/// wanted is the average over the operations that decide that axis, and no
/// average at all over an axis nothing touched.
#[derive(Clone, Copy, Debug, Default)]
struct Axis {
    total: f64,
    n: f64,
}

impl Axis {
    fn note(&mut self, x: f64) {
        self.total += x;
        self.n += 1.0;
    }

    fn penalise(&mut self, x: f64) {
        self.total -= x;
    }

    /// What it came to, or `None` if nothing in the plan bore on it.
    fn mean(self) -> Option<f64> {
        (self.n > 0.0).then(|| (self.total / self.n).clamp(0.0, 1.0))
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Building {
    accuracy: Axis,
    structure: Axis,
    finish: Axis,
    contamination: Axis,
    workmanship: Axis,
    /// How thoroughly the work was checked. Inspection does not improve
    /// anything; it catches what is wrong before it leaves.
    checked: f64,
}

impl WorkOrder {
    /// Raise an order with the output going onto the floor of the shop.
    pub fn begin(id: u64, recipe: usize, intended: u32, workplace: u32, day: u32) -> Self {
        WorkOrder::begin_for(id, recipe, intended, workplace, day, Placement::anywhere())
    }

    /// **Raise an order and say where the result goes.**
    pub fn begin_for(
        id: u64,
        recipe: usize,
        intended: u32,
        workplace: u32,
        day: u32,
        output: Placement,
    ) -> Self {
        WorkOrder {
            id,
            recipe,
            intended: intended.max(1),
            workplace,
            step: 0,
            step_done: 0.0,
            attempt: 0,
            setup_done: 0.0,
            completed: Vec::new(),
            reserved: Vec::new(),
            consumed_items: Vec::new(),
            consumed_bulk: Vec::new(),
            from_environment: Vec::new(),
            intermediates: Vec::new(),
            waste: Vec::new(),
            emissions: Vec::new(),
            substitutions: Vec::new(),
            active_labour_min: 0.0,
            machine_min: 0.0,
            unattended_min: 0.0,
            elapsed_min: 0.0,
            power_kwh: 0.0,
            state: Halt::Running,
            output,
            delivered: false,
            started_day: day,
            quality: Building::default(),
        }
    }

    /// **Use something other than what the plan named.** From here on the
    /// order draws the substitute, and the finished item record says so —
    /// which is what stops a chair of particleboard yielding oak.
    ///
    /// **Checked against the shape the operation needs, not against a
    /// list an author wrote.** A batten of the right timber at the right
    /// thickness weighs what a board weighs and will not yield a seat, so
    /// what refuses it is geometry.
    pub fn substituted(
        &mut self,
        wanted: DefId,
        used: DefId,
        cat: &Catalogue,
        book: &RecipeBook,
    ) -> Result<(), Unsuitable> {
        let recipe = book.get(self.recipe).ok_or(Unsuitable::NoSuchPlan)?;
        let d = cat.get(used).ok_or(Unsuitable::NoSuchPlan)?;
        if let Some(shape) = recipe.shape_wanted(wanted) {
            match shape.met_by(d.nominal, Quantity::Count(1), d.nominal_mass_kg) {
                Fit::Yes { .. } => {}
                Fit::NotEnough => return Err(Unsuitable::NotEnough),
                Fit::WrongShape(why) => return Err(Unsuitable::WrongShape(why)),
            }
        }
        self.substitutions.push(Substitution { wanted, used });
        Ok(())
    }

    /// **A substitute is measured out by mass, not counted one for one.**
    /// A chair calls for a 6.75 kg oak board; standing a 34.8 kg
    /// particleboard sheet in for it does not make the chair five times
    /// heavier — it means using a fifth of the sheet.
    fn actually(&self, def: DefId, count: f64, cat: &Catalogue) -> (DefId, f64) {
        let Some(sub) = self.substitutions.iter().find(|s| s.wanted == def) else {
            return (def, count);
        };
        let want = cat.get(def).map(|d| d.nominal_mass_kg).unwrap_or(0.0);
        let have = cat.get(sub.used).map(|d| d.nominal_mass_kg).unwrap_or(0.0);
        if have <= 0.0 {
            return (sub.used, count);
        }
        (sub.used, count * want / have)
    }

    pub fn finished(&self) -> bool {
        matches!(self.state, Halt::Done | Halt::Abandoned)
    }

    /// **Whether the output has anywhere to go.**
    ///
    /// A work order is not over when the last operation is; it is over
    /// when the thing it made is somewhere. A bench with no room, a
    /// carrier already at their limit, or a destination that is not a
    /// place at all all mean the same thing: it waits.
    pub fn somewhere_to_put_it(&self, store: &Store, cat: &Catalogue, kg: f64) -> bool {
        match self.output {
            Placement::Ground { .. } => true,
            // Real: 35 kg is what somebody carries any distance, which is
            // the figure `travel.rs` already uses.
            Placement::Carried { person } => {
                let already: f64 = store
                    .items
                    .iter()
                    .filter(|(id, _)| {
                        matches!(store.placement(*id), Some(Placement::Carried { person: p })
                            if p == person)
                    })
                    .map(|(_, i)| i.mass_kg)
                    .sum();
                already + kg <= 35.0
            }
            Placement::Contained { container } => store
                .get(container)
                .and_then(|c| cat.get(c.definition))
                .map(|d| {
                    let used: f64 = store
                        .get(container)
                        .map(|c| {
                            c.contents
                                .iter()
                                .filter_map(|&i| store.get(i))
                                .map(|i| i.mass_kg)
                                .sum()
                        })
                        .unwrap_or(0.0);
                    d.pockets.iter().any(|p| used + kg <= p.max_kg)
                })
                .unwrap_or(false),
            // You cannot finish a chair into a bracket or into another
            // order.
            Placement::Installed { .. } | Placement::InWorkOrder { .. } => false,
        }
    }

    /// **Complete the order into the world.**
    ///
    /// `Err(Blocked::NoRoom)` is not a failure of the work — the thing is
    /// made and there is nowhere to set it down, which is a real thing
    /// that happens in a small shop and is worth being able to say.
    pub fn deliver_into(
        &mut self,
        book: &RecipeBook,
        cat: &Catalogue,
        day: u32,
        store: &mut Store,
    ) -> Result<Id<ItemInstance>, Blocked> {
        if self.delivered {
            return Err(Blocked::AlreadyDelivered);
        }
        let made = self.deliver(book, cat, day).ok_or(Blocked::NotFinished)?;
        if !self.somewhere_to_put_it(store, cat, made.mass_kg) {
            return Err(Blocked::NoRoom);
        }
        let at = self.output;
        let id = store.add(made, at);
        self.delivered = true;
        Ok(id)
    }

    /// **Advance the work by so many minutes of wall clock.**
    ///
    /// Returns what it is doing at the end of them. Nothing is consumed by
    /// a minute that could not be worked, which is the difference between
    /// a stoppage and a loss.
    pub fn advance(
        &mut self,
        minutes: f64,
        book: &RecipeBook,
        cat: &Catalogue,
        place: &Workplace,
        maker: Maker,
    ) -> Halt {
        if self.finished() {
            return self.state;
        }
        let Some(recipe) = book.get(self.recipe) else {
            self.state = Halt::Abandoned;
            return self.state;
        };
        if !maker.knows_recipe && recipe.needs_knowledge {
            self.state = Halt::NoWorker;
            return self.state;
        }

        let mut left = minutes;
        // Setting up comes out of somebody's day before any unit is made.
        if self.setup_done < recipe.setup_minutes {
            if place.workers <= 0.0 {
                self.state = Halt::NoWorker;
                return self.state;
            }
            let want = recipe.setup_minutes - self.setup_done;
            let rate = place.workers.min(1.0) * maker.pace();
            let spend = left.min(want / rate.max(1e-6));
            self.setup_done += spend * rate;
            self.active_labour_min += spend * place.workers.min(1.0);
            self.elapsed_min += spend;
            left -= spend;
            if left <= 1e-9 {
                self.state = Halt::Running;
                return self.state;
            }
        }

        while left > 1e-9 {
            let Some(step) = recipe.steps.get(self.step) else {
                self.state = Halt::Done;
                return self.state;
            };

            // ---- can this step run at all? -------------------------
            if step.needs_power && !place.power {
                self.state = Halt::NoPower;
                return self.state;
            }
            if step.effort.needs_somebody() && place.workers <= 0.0 {
                self.state = Halt::NoWorker;
                return self.state;
            }
            let mut chosen: Vec<Provides> = Vec::new();
            for n in &step.needs {
                match place.best_for(*n) {
                    Some(t) => chosen.push(t),
                    None => {
                        // Nothing here can do it. If the grid is down and
                        // something here *could* have, say so — a plant
                        // waiting on power is a different problem from a
                        // plant that never had the machine.
                        self.state = if !place.power && self.could_have_with_power(place, *n) {
                            Halt::NoPower
                        } else {
                            Halt::NoTool(n.capability)
                        };
                        return self.state;
                    }
                }
            }

            // ---- take what it uses, when it uses it ----------------
            // Only on the first attempt: a joint done again is re-glued,
            // not re-sawn. What a second attempt costs is time and
            // consumables, which the rework itself accounts for.
            if self.step_done <= 0.0 && self.attempt == 0 {
                for f in &step.consumes {
                    match *f {
                        Flow::Item { def, count } => {
                            let (used, n) = self.actually(def, count, cat);
                            add(&mut self.consumed_items, used, n);
                            // An intermediate the order made itself is
                            // being taken back up, not bought in.
                            sub(&mut self.intermediates, used, n);
                        }
                        Flow::Bulk { material, kg } => add(&mut self.consumed_bulk, material, kg),
                        Flow::Environment { material, kg } => {
                            add(&mut self.from_environment, material, kg)
                        }
                        _ => {}
                    }
                }
            }

            // ---- how fast does it go? ------------------------------
            let tool_speed = if chosen.is_empty() {
                1.0
            } else {
                chosen.iter().map(|t| t.speed).sum::<f64>() / chosen.len() as f64
            };
            let hands = place.workers.max(0.0);
            let rate = match step.effort {
                Effort::Hands { .. } => {
                    // More hands help, with diminishing returns: two
                    // people on one chair are not twice as quick.
                    let people = if hands <= 1.0 { hands } else { 1.0 + (hands - 1.0).sqrt() };
                    tool_speed * maker.pace() * people
                }
                Effort::Machine { .. } => tool_speed.max(1.0) * place.stations.max(1) as f64,
                Effort::Unattended { .. } => 1.0,
            };
            let want = step.effort.span_minutes() - self.step_done;
            let spend = left.min(want / rate.max(1e-6));
            self.step_done += spend * rate;
            self.elapsed_min += spend;
            left -= spend;
            match step.effort {
                Effort::Hands { .. } => self.active_labour_min += spend * hands.min(1.0).max(0.0),
                Effort::Machine { minutes, tending } => {
                    self.machine_min += spend * rate;
                    let share = if minutes > 0.0 { tending / minutes } else { 0.0 };
                    self.active_labour_min += spend * share;
                }
                Effort::Unattended { .. } => self.unattended_min += spend,
            }
            let kw: f64 = chosen.iter().map(|t| t.kw).sum();
            if kw > 0.0 {
                self.power_kwh += kw * spend / 60.0;
            }

            if self.step_done + 1e-9 < step.effort.span_minutes() {
                self.state = if step.effort.needs_somebody() { Halt::Running } else { Halt::Unattended };
                return self.state;
            }

            // ---- the step is over. Did it go as intended? ----------
            // **Everything that bears on the work is added up first**, and
            // the outcome is calculated once from the total.
            let capability = capability_of(recipe, step, &chosen, place, maker);
            let yields = capability.yields();
            let (grade, mishap) =
                roll_outcome(self.id, self.step, self.attempt, yields, step.hazard);
            self.note(step, grade, mishap, place, cat, recipe);
            self.completed.push(StepRecord {
                step: self.step,
                attempt: self.attempt,
                grade,
                mishap,
                labour_minutes: step.effort.labour_minutes(),
                machine_minutes: step.effort.machine_minutes(),
                elapsed_minutes: step.effort.span_minutes(),
            });

            if mishap.ends_the_order() {
                self.state = Halt::Abandoned;
                return self.state;
            }
            // **Reworkable means it is done again.** Which is the whole
            // point of the grade: a unit that needed a touch-up did not
            // pass first time, and it did not go in the skip either.
            if grade == Grade::Reworkable && self.attempt < MAX_ATTEMPTS {
                self.attempt += 1;
                self.step_done = 0.0;
                continue;
            }

            for f in &step.produces {
                match *f {
                    Flow::Waste { material, kg } => add(&mut self.waste, material, kg),
                    Flow::Emission { material, kg } => add(&mut self.emissions, material, kg),
                    Flow::Item { def, count } => add(&mut self.intermediates, def, count),
                    _ => {}
                }
            }
            if let Some(d) = step.leaves {
                add(&mut self.intermediates, d, 1.0);
            }

            self.step += 1;
            self.attempt = 0;
            self.step_done = 0.0;
            if self.step >= recipe.steps.len() {
                self.state = Halt::Done;
                return self.state;
            }
        }
        self.state = Halt::Running;
        self.state
    }

    fn could_have_with_power(&self, place: &Workplace, n: Need) -> bool {
        place
            .tools
            .iter()
            .any(|t| t.satisfies(n.capability, n.capacity, n.precision_mm) && t.kw > 0.0)
    }

    /// **Material used over the plan is scrap, not product.** Cutting a
    /// second leg because the first was wrong does not make a heavier
    /// chair; it makes a chair and a ruined leg. The workpiece itself is
    /// exempt — there is only one of it.
    fn spoil(&mut self, step: &Step, recipe: &RecipeDefinition, cat: &Catalogue, factor: f64) {
        if factor <= 0.0 {
            return;
        }
        let extra: Vec<Flow> = step.consumes.clone();
        for f in extra {
            match f {
                Flow::Item { def, count } => {
                    if recipe.steps.iter().any(|s| s.leaves == Some(def)) {
                        continue; // the workpiece
                    }
                    let (used, n) = self.actually(def, count, cat);
                    let n = n * factor;
                    add(&mut self.consumed_items, used, n);
                    if let Some(d) = cat.get(used) {
                        if let Some(m) = d.materials.chiefly() {
                            add(&mut self.waste, m, d.nominal_mass_kg * n);
                        }
                    }
                }
                Flow::Bulk { material, kg } => {
                    add(&mut self.consumed_bulk, material, kg * factor);
                    add(&mut self.waste, material, kg * factor);
                }
                Flow::Environment { material, kg } => {
                    add(&mut self.from_environment, material, kg * factor);
                    add(&mut self.waste, material, kg * factor);
                }
                _ => {}
            }
        }
    }

    fn note(
        &mut self,
        step: &Step,
        grade: Grade,
        mishap: Mishap,
        place: &Workplace,
        cat: &Catalogue,
        recipe: &RecipeDefinition,
    ) {
        // Rework puts it right; it does not make it good. Scrap that was
        // carried on with is worse again.
        let good = match grade {
            Grade::Accepted => 1.0,
            Grade::Reworkable => 0.72,
            Grade::Scrapped => 0.4,
        };
        let jigged = 0.7 + 0.3 * place.jigs;
        let q = &mut self.quality;
        match step.operation.bears_on() {
            Bears::Accuracy => q.accuracy.note(good * jigged),
            Bears::Structure => q.structure.note(good),
            Bears::Finish => q.finish.note(good),
            Bears::Cleanliness => q.contamination.note(1.0 - good),
            // **A test does not make the work better.** It catches what is
            // wrong with it before it leaves the shop, which is a
            // different thing and belongs on a different line.
            Bears::Inspection => {
                q.checked += if mishap.is_none() { 1.0 } else { 0.4 };
                return;
            }
        }
        q.workmanship.note(good);
        match mishap {
            Mishap::DimensionalDefect => q.accuracy.penalise(0.8),
            Mishap::CosmeticFlaw => q.finish.penalise(0.8),
            Mishap::WeakJoint => q.structure.penalise(0.9),
            Mishap::Contamination => q.contamination.note(0.6),
            // Extra material for *this* operation, and it is spoiled.
            // Scaling everything consumed so far is how a fumbled sanding
            // pass came to eat another two kilograms of board.
            Mishap::Overrun(extra) => self.spoil(step, recipe, cat, extra),
            _ => {}
        }
        // Doing it again costs the consumables again, whatever it was that
        // sent it back.
        if grade == Grade::Reworkable {
            self.spoil(step, recipe, cat, 1.0);
        }
        match mishap {
            Mishap::Slow(f) => {
                self.elapsed_min += step.effort.span_minutes() * (f - 1.0);
                self.active_labour_min += step.effort.labour_minutes() * (f - 1.0);
            }
            _ => {}
        }
    }

    /// **What came out.**
    ///
    /// The product mass is what went in less what left, so the balance
    /// closes by construction — and a recipe whose declared wastes are
    /// wrong shows up as a chair of the wrong weight rather than as
    /// silently vanishing timber.
    ///
    /// The assembly record is built by **walking the plan** rather than by
    /// listing everything the order ever drew in. A board that was cut
    /// into parts is not in the chair; the parts are, and they weigh what
    /// is left after the offcuts went in the bin. Listing the board would
    /// hand those offcuts back to anybody who took the chair apart.
    pub fn deliver(&self, book: &RecipeBook, cat: &Catalogue, day: u32) -> Option<ItemInstance> {
        if self.state != Halt::Done {
            return None;
        }
        let recipe = book.get(self.recipe)?;
        let b = self.balance(book, cat);
        let per = (b.product_kg / recipe.yields.max(1) as f64).max(0.0);

        let mut item = ItemInstance::fresh(cat, recipe.result, Quantity::Count(1));
        item.mass_kg = per;
        item.quality = self.assessed(cat, recipe.result);
        item.condition = Condition::fresh();
        item.provenance.made_on_day = day;
        item.provenance.made_at = Some(self.workplace);

        let mut record = AssemblyRecord {
            work_order: Some(self.id),
            substitutions: self.substitutions.clone(),
            ..Default::default()
        };

        // ---- walk the plan -------------------------------------------
        let mut parts: Vec<Part> = Vec::new();
        for step in &recipe.steps {
            let mut taken: Vec<Part> = Vec::new();
            for f in &step.consumes {
                match *f {
                    Flow::Item { def, count } => {
                        let (used, n) = self.actually(def, count, cat);
                        if let Some(at) = parts.iter().position(|p| p.def == Some(used)) {
                            // Something this order made, coming back up
                            // onto the bench.
                            taken.push(parts.remove(at));
                        } else if let Some(d) = cat.get(used) {
                            taken.push(Part {
                                def: Some(used),
                                count: n,
                                kg: d.nominal_mass_kg * n,
                                comp: d.materials.clone(),
                                held_by: d.makes_joint(),
                            });
                        }
                    }
                    Flow::Bulk { material, kg } | Flow::Environment { material, kg } => {
                        taken.push(Part {
                            def: None,
                            count: 0.0,
                            kg,
                            comp: Composition::pure(material),
                            held_by: None,
                        });
                    }
                    _ => {}
                }
            }
            let lost: f64 = step
                .produces
                .iter()
                .filter(|f| matches!(f, Flow::Waste { .. } | Flow::Emission { .. }))
                .map(|f| f.mass_kg(cat))
                .sum();

            match step.leaves {
                // The step converted what it took into a named thing.
                Some(y) => {
                    let kg: f64 = taken.iter().map(|p| p.kg).sum::<f64>() - lost;
                    let mut comp = Composition::default();
                    let mut so_far = 0.0;
                    for p in &taken {
                        comp = Composition::blend((&comp, so_far), (&p.comp, p.kg));
                        so_far += p.kg;
                    }
                    parts.push(Part {
                        def: Some(y),
                        count: 1.0,
                        kg: kg.max(0.0),
                        comp,
                        held_by: None,
                    });
                }
                // Nothing new was named, so what it took is now part of
                // the workpiece — and what the step joined, it joined.
                None => {
                    for mut p in taken {
                        if p.held_by.is_none() {
                            p.held_by = step.joins;
                        }
                        parts.push(p);
                    }
                    if lost > 0.0 {
                        shrink(&mut parts, lost);
                    }
                }
            }
        }

        // ---- and what the plan says is what the record says -----------
        for (i, p) in parts.iter().enumerate() {
            let held_by = p.held_by.unwrap_or(JointMethod::Screwed);
            match p.def {
                Some(def) if p.count >= 1.0 => {
                    record.components.push(Installed {
                        definition: def,
                        quantity: Quantity::Count(p.count.round().max(1.0) as u32),
                        mass_kg: p.kg,
                        materials: p.comp.clone(),
                        condition_at_install: Condition::fresh(),
                        quality_at_install: Quality::default(),
                        held_by,
                    });
                }
                Some(def) => {
                    // Less than one of a thing is a quantity of stuff:
                    // part of a sheet, a fifth of a bottle of glue.
                    record.components.push(Installed {
                        definition: def,
                        quantity: Quantity::Mass { kg: p.kg },
                        mass_kg: p.kg,
                        materials: p.comp.clone(),
                        condition_at_install: Condition::fresh(),
                        quality_at_install: Quality::default(),
                        held_by,
                    });
                }
                None => {
                    for (m, kg) in p.comp.masses(p.kg) {
                        record.consumed.push((m, kg));
                    }
                }
            }
            let _ = i;
        }
        for (i, c) in record.components.iter().enumerate() {
            let first = record
                .components
                .iter()
                .position(|o| o.held_by == c.held_by)
                .unwrap_or(i);
            record.joints.push(Joint {
                method: c.held_by,
                joins: (first, i),
                fastener: None,
                accessible: true,
            });
        }

        // **What it is actually made of, not what the definition says a
        // normal one is made of.**
        let actual = record.actual_materials();
        item.materials = if actual.is_empty() { item.materials.clone() } else { actual };
        item.assembly = Some(record);
        Some(item)
    }

    fn assessed(&self, _cat: &Catalogue, _result: DefId) -> Quality {
        let q = self.quality;
        // **An axis nothing bore on takes the ordinary standard**, not
        // zero: a chair nobody was asked to polish is unfinished, not
        // badly finished, and the plan never called for paint.
        let dirt = q.contamination.mean().unwrap_or(0.0);
        // What inspection buys is that fewer defects leave the shop.
        let caught = (q.checked * 0.3).min(0.6);
        Quality {
            workmanship: q.workmanship.mean().unwrap_or(0.7),
            structural_integrity: q.structure.mean().unwrap_or(0.75),
            dimensional_accuracy: q.accuracy.mean().unwrap_or(0.75),
            finish: q.finish.mean().unwrap_or(0.6),
            contamination: (dirt * (1.0 - caught)).clamp(0.0, 1.0),
            domain: DomainQuality::None,
        }
    }

    /// **Nothing is created or destroyed.**
    pub fn balance(&self, book: &RecipeBook, cat: &Catalogue) -> Balance {
        let items: f64 = self
            .consumed_items
            .iter()
            .map(|&(d, n)| cat.get(d).map(|x| x.nominal_mass_kg).unwrap_or(0.0) * n)
            .sum();
        // An intermediate the order made and then took back up is the
        // workpiece, not a fresh input.
        let recycled: f64 = book
            .get(self.recipe)
            .map(|r| {
                self.consumed_items
                    .iter()
                    .filter(|(d, _)| r.steps.iter().any(|s| s.leaves == Some(*d)))
                    .map(|&(d, n)| cat.get(d).map(|x| x.nominal_mass_kg).unwrap_or(0.0) * n)
                    .sum()
            })
            .unwrap_or(0.0);
        let bulk: f64 = self.consumed_bulk.iter().map(|c| c.1).sum();
        let env: f64 = self.from_environment.iter().map(|c| c.1).sum();
        let waste: f64 = self.waste.iter().map(|c| c.1).sum();
        let emis: f64 = self.emissions.iter().map(|c| c.1).sum();
        let inputs = items - recycled + bulk + env;
        Balance {
            inputs_kg: inputs,
            from_environment_kg: env,
            waste_kg: waste,
            emissions_kg: emis,
            product_kg: (inputs - waste - emis).max(0.0),
        }
    }

    /// Total labour against total clock. The figure the economy needs and
    /// the one it has been getting wrong.
    pub fn labour_vs_elapsed(&self) -> (f64, f64) {
        (self.active_labour_min, self.elapsed_min)
    }
}

/// Why a proposed substitute will not do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unsuitable {
    NoSuchPlan,
    NotEnough,
    /// **The case mass alone cannot see.**
    WrongShape(&'static str),
}

/// Why a finished order has not been handed over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blocked {
    NotFinished,
    /// Made, and nowhere to set it down.
    NoRoom,
    AlreadyDelivered,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Balance {
    pub inputs_kg: f64,
    pub from_environment_kg: f64,
    pub product_kg: f64,
    pub waste_kg: f64,
    pub emissions_kg: f64,
}

impl Balance {
    pub fn residual(&self) -> f64 {
        self.inputs_kg - self.product_kg - self.waste_kg - self.emissions_kg
    }

    pub fn closes(&self, tolerance_kg: f64) -> bool {
        self.residual().abs() <= tolerance_kg
    }
}

/// **Room in hand on the tolerance.** A tool exactly at the limit of what
/// the operation asks holds it only just; one with a factor of four to
/// spare holds it without thinking. Returns roughly 0.55 to 1.0, with an
/// adequate tool near 0.7 — which is where `capability_of` centres it.
fn worst_precision(tools: &[Provides], needs: &[Need]) -> f64 {
    if needs.is_empty() {
        return 0.85;
    }
    let mut worst: f64 = 1.0;
    for (t, n) in tools.iter().zip(needs) {
        let headroom = (n.precision_mm / t.precision_mm.max(1e-6)).clamp(0.2, 4.0);
        worst = worst.min((0.55 + 0.12 * headroom).min(1.0));
    }
    worst
}

fn add<K: PartialEq + Copy>(v: &mut Vec<(K, f64)>, k: K, amount: f64) {
    match v.iter_mut().find(|p| p.0 == k) {
        Some(p) => p.1 += amount,
        None => v.push((k, amount)),
    }
}

fn sub<K: PartialEq + Copy>(v: &mut Vec<(K, f64)>, k: K, amount: f64) {
    if let Some(p) = v.iter_mut().find(|p| p.0 == k) {
        p.1 = (p.1 - amount).max(0.0);
    }
}

// =====================================================================
// the plans the world starts with
// =====================================================================

/// **The same physical recipe, worked three different ways.**
///
/// A factory, a cabinet shop and a man with hand tools all make a wooden
/// chair out of a board, some glue and a dozen screws. What differs is the
/// providers — tools, jigs, stations, hands — and what that buys is speed
/// and consistency, never a magical multiplier on the materials.
pub fn standard_recipes(cat: &Catalogue) -> RecipeBook {
    use Capability as C;
    let mut b = RecipeBook::default();

    let oak = cat.must("oak board");
    let particle = cat.must("particleboard sheet");
    let chair = cat.must("wooden chair");
    let chair_parts = cat.must("chair parts");
    let glue = cat.must("wood glue");
    let screw = cat.must("wood screw");

    // ---- a chair, by hand --------------------------------------------
    // 6.75 kg of board becomes 4.80 kg of parts and 1.95 kg of offcuts and
    // sawdust: a 71% yield off rough stock, which is what furniture
    // actually gets.
    let chair_steps = |powered: bool, speed_needs: bool| {
        // **A seat wants a panel and a leg wants a length.** Real chair
        // stock is boards of 18-32 mm at least 140 mm wide; a batten is
        // too narrow to get a seat out of and an offcut is too short to
        // get a leg out of, whatever either of them weighs.
        let cut = Step::new("cut members", Operation::Cut, Effort::Hands { minutes: 45.0 })
            .needing(&[Need::of(C::CutWood, 30.0, if speed_needs { 0.8 } else { 2.0 })])
            .taking(&[Flow::Item { def: oak, count: 1.0 }])
            .shaped(&[(
                oak,
                Amount::Sheet {
                    min_width_m: 0.14,
                    min_length_m: 1.2,
                    thickness_m: (0.016, 0.032),
                },
            )])
            .giving(&[Flow::Waste { material: Material::Oak, kg: 1.95 }])
            .leaving(chair_parts);
        let cut = if powered { cut.powered() } else { cut };
        vec![
            Step::new("measure and mark", Operation::Measure, Effort::Hands { minutes: 10.0 })
                .needing(&[Need::of(C::Measure, 1.0, 2.0)]),
            cut,
            Step::new("drill joints", Operation::Drill, Effort::Hands { minutes: 15.0 })
                .needing(&[Need::of(C::Drill, 8.0, 1.5)]),
            Step::new("glue and fasten", Operation::Glue, Effort::Hands { minutes: 20.0 })
                .needing(&[Need::of(C::Glue, 1.0, 3.0), Need::of(C::Fasten, 5.0, 3.0)])
                .taking(&[
                    Flow::Item { def: chair_parts, count: 1.0 },
                    Flow::Item { def: glue, count: 0.05 },
                    Flow::Item { def: screw, count: 12.0 },
                ])
                .joined(JointMethod::Glued),
            // **Curing takes twelve hours and nobody's day.** This is the
            // step that makes labour and elapsed time different numbers.
            Step::new("clamp and cure", Operation::Rest, Effort::Unattended { minutes: 720.0 }),
            Step::new("sand", Operation::Grind, Effort::Hands { minutes: 25.0 })
                .needing(&[Need::of(C::Grind, 2.0, 2.0)])
                .giving(&[Flow::Waste { material: Material::Oak, kg: 0.03 }]),
        ]
    };

    b.add(RecipeDefinition {
        name: "chair, hand tools",
        result: chair,
        yields: 1,
        skill: Skill::Woodworking,
        proficiency: Proficiency::Joinery,
        difficulty: 0.45,
        needs_knowledge: false,
        steps: chair_steps(false, false),
        substitutions: vec![(oak, vec![particle, cat.must("pine board")])],
        setup_minutes: 15.0,
    });

    b.add(RecipeDefinition {
        name: "chair, cabinet shop",
        result: chair,
        yields: 1,
        skill: Skill::Woodworking,
        proficiency: Proficiency::Joinery,
        difficulty: 0.35,
        needs_knowledge: false,
        steps: chair_steps(true, true),
        substitutions: vec![(oak, vec![particle, cat.must("pine board")])],
        setup_minutes: 30.0,
    });

    // The factory plan is the same physical operations with the setup
    // amortised over a run of forty, which is the only thing scale buys.
    let mut factory = chair_steps(true, true);
    for s in &mut factory {
        if let Effort::Hands { minutes } = s.effort {
            s.effort = Effort::Hands { minutes };
        }
    }
    b.add(RecipeDefinition {
        name: "chair, factory",
        result: chair,
        yields: 1,
        skill: Skill::Woodworking,
        proficiency: Proficiency::Machining,
        difficulty: 0.25,
        needs_knowledge: true,
        steps: factory,
        substitutions: vec![(oak, vec![particle, cat.must("pine board")])],
        setup_minutes: 240.0,
    });

    // ---- a loaf ------------------------------------------------------
    // 0.50 kg of flour and 0.35 of water make 0.85 of dough; the oven
    // drives off 0.05 as steam and 0.80 comes out. 35 minutes of labour,
    // 35 of oven, 125 on the clock.
    let dough = cat.must("dough");
    let risen = cat.must("risen dough");
    b.add(RecipeDefinition {
        name: "loaf",
        result: cat.must("loaf"),
        yields: 1,
        skill: Skill::Cooking,
        proficiency: Proficiency::Baking,
        difficulty: 0.2,
        needs_knowledge: false,
        steps: vec![
            Step::new("mix", Operation::Mix, Effort::Hands { minutes: 15.0 })
                .taking(&[
                    Flow::Bulk { material: Material::Flour, kg: 0.50 },
                    Flow::Environment { material: Material::Water, kg: 0.35 },
                ])
                .leaving(dough),
            Step::new("shape", Operation::Assemble, Effort::Hands { minutes: 10.0 })
                .taking(&[Flow::Item { def: dough, count: 1.0 }])
                .leaving(risen),
            Step::new("prove", Operation::Rest, Effort::Unattended { minutes: 60.0 }),
            Step::new("bake", Operation::Bake, Effort::Machine { minutes: 35.0, tending: 10.0 })
                .needing(&[Need::of(C::Bake, 220.0, 100.0)])
                .taking(&[Flow::Item { def: risen, count: 1.0 }])
                .giving(&[Flow::Emission { material: Material::Water, kg: 0.05 }])
                .powered(),
        ],
        substitutions: vec![],
        setup_minutes: 5.0,
    });

    // ---- trousers ----------------------------------------------------
    // 0.65 kg of cloth, 0.04 of thread; 0.09 of offcuts. Real garment
    // cutting wastes 10-20% of the roll.
    let cloth = cat.must("cotton cloth");
    let panels = cat.must("cut panels");
    b.add(RecipeDefinition {
        name: "work trousers",
        result: cat.must("work trousers"),
        yields: 1,
        skill: Skill::Tailoring,
        proficiency: Proficiency::Sewing,
        difficulty: 0.3,
        needs_knowledge: false,
        steps: vec![
            Step::new("cut panels", Operation::Cut, Effort::Hands { minutes: 25.0 })
                .needing(&[Need::of(C::CutFabric, 4.0, 3.0)])
                .taking(&[Flow::Bulk { material: Material::Cotton, kg: 0.67 }])
                .giving(&[Flow::Waste { material: Material::Cotton, kg: 0.09 }])
                .leaving(panels),
            Step::new("sew", Operation::Sew, Effort::Hands { minutes: 55.0 })
                .needing(&[Need::of(C::Sew, 4.0, 2.0)])
                .taking(&[
                    Flow::Item { def: panels, count: 1.0 },
                    Flow::Bulk { material: Material::Thread, kg: 0.018 },
                ])
                .joined(JointMethod::Stitched),
            Step::new("press and fold", Operation::Package, Effort::Hands { minutes: 6.0 }),
        ],
        substitutions: vec![(cloth, vec![cat.must("cotton cloth")])],
        setup_minutes: 10.0,
    });

    // ---- a rifle, at serviceable-assembly depth ----------------------
    // **Not every pin and spring.** CDDA declines to break a firearm into
    // dozens of interchangeable internals because the data burden would
    // outweigh the play, and it is right; what this needs is the level at
    // which parts are actually made, traded, replaced and serviced.
    //
    // The joints are the ones that decide what a field strip can reach: a
    // barrel is pressed and pinned and a fire control group is riveted, so
    // neither comes out at the bench, while the bolt carrier unclips and
    // the stock unbolts — which is exactly what stripping a rifle means.
    b.add(RecipeDefinition {
        name: "rifle, assembled",
        result: cat.must("rifle"),
        yields: 1,
        skill: Skill::Gunsmithing,
        proficiency: Proficiency::Machining,
        difficulty: 0.6,
        needs_knowledge: true,
        steps: vec![
            Step::new("press and pin the barrel", Operation::Press, Effort::Hands { minutes: 25.0 })
                .needing(&[Need::of(C::Press, 300.0, 0.05)])
                .taking(&[
                    Flow::Item { def: cat.must("receiver"), count: 1.0 },
                    Flow::Item { def: cat.must("barrel"), count: 1.0 },
                ])
                .joined(JointMethod::Crimped),
            Step::new("rivet in the fire control group", Operation::Fasten,
                      Effort::Hands { minutes: 30.0 })
                .needing(&[Need::of(C::Fasten, 5.0, 2.0)])
                .taking(&[
                    Flow::Item { def: cat.must("fire control group"), count: 1.0 },
                    Flow::Item { def: cat.must("rivet"), count: 4.0 },
                ])
                .joined(JointMethod::Riveted),
            Step::new("fit the bolt carrier", Operation::Assemble, Effort::Hands { minutes: 8.0 })
                .taking(&[Flow::Item { def: cat.must("bolt assembly"), count: 1.0 }])
                .joined(JointMethod::Clipped),
            Step::new("bolt on the stock", Operation::Fasten, Effort::Hands { minutes: 10.0 })
                .needing(&[Need::of(C::Fasten, 5.0, 2.0)])
                .taking(&[
                    Flow::Item { def: cat.must("stock"), count: 1.0 },
                    Flow::Item { def: cat.must("bolt"), count: 2.0 },
                ])
                .joined(JointMethod::Bolted),
            Step::new("fit the magazine", Operation::Assemble, Effort::Hands { minutes: 2.0 })
                .taking(&[Flow::Item { def: cat.must("magazine"), count: 1.0 }])
                .joined(JointMethod::Clipped),
            // **Headspace is measured, not assumed.** It is the one check
            // between a rifle and a hazard, and it wants a tolerance no
            // ordinary tool holds.
            Step::new("check headspace", Operation::Test, Effort::Hands { minutes: 12.0 })
                .needing(&[Need::of(C::Measure, 1.0, 0.05)]),
            Step::new("function test", Operation::Test, Effort::Hands { minutes: 6.0 }),
        ],
        substitutions: vec![],
        setup_minutes: 20.0,
    });

    // ---- a cartridge, which is its own chain and a hazardous one -----
    let case = cat.must("cartridge case");
    let sized = cat.must("sized case");
    let primed = cat.must("primed case");
    let charged = cat.must("charged case");
    b.add(RecipeDefinition {
        name: "cartridge, handloaded",
        result: cat.must("cartridge"),
        yields: 1,
        skill: Skill::Gunsmithing,
        proficiency: Proficiency::Handloading,
        difficulty: 0.5,
        needs_knowledge: true,
        steps: vec![
            Step::new("size and trim the case", Operation::Press, Effort::Hands { minutes: 0.6 })
                .needing(&[Need::of(C::Press, 400.0, 0.1)])
                .taking(&[Flow::Item { def: case, count: 1.0 }])
                .leaving(sized),
            Step::new("seat the primer", Operation::Press, Effort::Hands { minutes: 0.3 })
                .needing(&[Need::of(C::Press, 100.0, 0.1)])
                .taking(&[
                    Flow::Item { def: sized, count: 1.0 },
                    Flow::Item { def: cat.must("primer"), count: 1.0 },
                ])
                .leaving(primed)
                .dangerous(0.35),
            Step::new("charge", Operation::Measure, Effort::Hands { minutes: 0.5 })
                .needing(&[Need::of(C::Measure, 1.0, 0.05)])
                .taking(&[
                    Flow::Item { def: primed, count: 1.0 },
                    Flow::Bulk { material: Material::Propellant, kg: 0.0017 },
                ])
                .leaving(charged)
                .dangerous(0.25),
            Step::new("seat the bullet and crimp", Operation::Press, Effort::Hands { minutes: 0.4 })
                .needing(&[Need::of(C::Press, 300.0, 0.05)])
                .taking(&[
                    Flow::Item { def: charged, count: 1.0 },
                    Flow::Item { def: cat.must("bullet"), count: 1.0 },
                ])
                .joined(JointMethod::Crimped),
            Step::new("inspect", Operation::Test, Effort::Hands { minutes: 0.2 })
                .needing(&[Need::of(C::Measure, 1.0, 0.05)]),
        ],
        substitutions: vec![],
        setup_minutes: 20.0,
    });

    b
}

/// Tools a person owns rather than a factory: what one man at a bench can
/// bring to a plan.
pub fn hand_tools(cat: &Catalogue) -> Vec<Provides> {
    ["handsaw", "hand drill", "hammer", "screwdriver", "workbench", "scissors",
     "needle and thread", "oven", "reloading press", "clamps", "sanding block"]
        .iter()
        .filter_map(|n| cat.named(n))
        .filter_map(|d| cat.get(d))
        .flat_map(|d| d.provides.iter().copied())
        .collect()
}

/// What a works has: the same capabilities, faster and to a tighter
/// tolerance, and every one of them wanting power.
pub fn machine_shop(cat: &Catalogue) -> Vec<Provides> {
    ["bandsaw", "angle grinder", "plasma cutter", "cordless drill", "welder",
     "sewing machine", "workbench", "oven", "reloading press", "scissors", "hammer",
     "clamps", "sanding block"]
        .iter()
        .filter_map(|n| cat.named(n))
        .filter_map(|d| cat.get(d))
        .flat_map(|d| d.provides.iter().copied())
        .collect()
}
