//! **Taking a thing apart is not the recipe run backwards.**
//!
//! CDDA has three item-level routes and its own documentation names the
//! trouble with each: a fixed uncraft recipe transmutes (a chair of
//! particleboard yields oak because the recipe says oak), a reversible
//! recipe can hand back what was never put in, and generic salvage is
//! deliberately coarse. All three exist because *one* of them cannot cover
//! the ground.
//!
//! The resolution is to stop asking the definition and ask the object. An
//! `AssemblyRecord` says what actually went in and how it was held
//! together, so what comes out is what was there — as the material it
//! really was, in the condition it is really in, less whatever the joint
//! destroys on the way out.
//!
//! **Nine actions, one engine.** Uninstalling an alternator, field
//! stripping a rifle, deconstructing a wall and smashing a chair for
//! firewood are the same transformation with different goals, different
//! care and very different yields — and the whole point of separating them
//! is that a careful hour and a sledgehammer must not return the same
//! pile.

use crate::item::{
    AssemblyRecord, Catalogue, Condition, DefId, ItemInstance, JointMethod, Quality,
};
use crate::bom::Formed;
use crate::material::{Composition, Material, Recovers};
use crate::rng::Rng;
use crate::save::channel;

/// **The coupling, stated once.**
///
/// ```text
/// u = hash(world seed, teardown event, component, named draw)
/// survives = u < survival_probability(method, skill, joint, condition)
/// ```
///
/// **The intention is deliberately outside the key.** That is a monotone
/// coupling rather than a reroll: the same unit is tested against a higher
/// probability when the work is careful, so a method with a higher
/// authored recovery can never return fewer components merely because it
/// drew different numbers.
///
/// **And separate questions get separate draws.** Whether a part came off
/// in one piece, how badly it was knocked about, whether it came out
/// dirty, and whether it has something wrong with it that nobody can see
/// are four different facts, and one number cannot carry them. There is
/// also an **event-level** draw, so that a job that went badly went badly
/// for everything — common-mode damage is real and per-component draws
/// alone cannot produce it.
fn draw(event: u64, component: usize, unit: u32, what: &str) -> f64 {
    let h = channel(event, component as u64, what)
        ^ (unit as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    Rng::new(h).next_f32() as f64
}

/// How badly the whole job went, before any one component is considered.
/// A wall that came down in a heap damages everything in it.
fn common_mode(event: u64, how: Teardown) -> f64 {
    let x = Rng::new(channel(event, 0, "common mode")).next_f32() as f64;
    // Careful work has little common-mode risk; a sledgehammer is nearly
    // all common mode.
    (x * (1.0 - how.care())).clamp(0.0, 1.0)
}

/// **A unique component is recovered or it is destroyed.** It is never
/// 0.6 of a component, and it must not be decided by rounding: flooring
/// always destroys a lone part at any probability under one, ceiling
/// always saves it, and ordinary rounding turns every probability above
/// a half into a certainty. All three are wrong in the same way — they
/// replace a chance with a rule.
///
/// So the answer is a **deterministic Bernoulli**, keyed by the teardown
/// event and which component it is. Doing it again gives the same answer;
/// reloading a save gives the same answer; a different teardown of the
/// same object is a different event and may go differently.
///
/// **The method is deliberately not in the key.** Using one draw across
/// intentions is common random numbers: the same unit is tested against a
/// higher probability when the work is careful, so careful recovery can
/// never come out worse than smashing by an accident of sampling. That is
/// a property worth having and it is asserted in the tests.
fn survivors(event: u64, component: usize, count: u32, p: f64) -> u32 {
    let p = p.clamp(0.0, 1.0);
    if count == 0 {
        return 0;
    }
    // A handful of parts: settle each one on its own.
    if count <= 512 {
        let mut out = 0;
        for unit in 0..count {
            if draw(event, component, unit, "component separability") < p {
                out += 1;
            }
        }
        return out;
    }
    // **A lot is a number.** Above a few hundred the binomial spread is
    // negligible beside the count, so take the expectation and settle only
    // the fractional remainder — which keeps the aggregate unbiased
    // without drawing ten thousand times.
    let expected = count as f64 * p;
    let whole = expected.floor();
    let h = channel(event, component as u64, "lot recovery remainder");
    let extra = ((Rng::new(h).next_f32() as f64) < expected - whole) as u32;
    (whole as u32 + extra).min(count)
}

/// **What somebody is trying to achieve**, which is what decides the
/// yield. Not nine special cases — nine settings of the same three knobs:
/// how much care, whether the joints are undone or broken, and whether
/// components or material are what is wanted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Teardown {
    /// Remove one working component and leave the rest alone.
    Uninstall,
    /// Open it into its serviceable modules and no further. What a soldier
    /// does to a rifle to clean it.
    FieldStrip,
    /// Undo the joints and recover identifiable components.
    Disassemble,
    /// Get back whatever pieces are still usable, accepting damage.
    Salvage,
    /// Melt, shred or repulp it into feedstock.
    Recycle,
    /// Fast, destructive material recovery. A pair of shears and no
    /// interest in what any of it used to be.
    CutUp,
    /// A sledgehammer.
    Smash,
    /// Biological decomposition into parts.
    Butcher,
    /// Take a structure down carefully enough to reuse the pieces.
    Deconstruct,
}

impl Teardown {
    /// How much of what the joints would allow actually survives, given
    /// how it is being done.
    pub fn care(self) -> f64 {
        use Teardown::*;
        match self {
            Uninstall => 1.00,
            FieldStrip => 0.99,
            Disassemble => 0.92,
            Deconstruct => 0.80,
            Salvage => 0.65,
            Recycle => 0.50,
            CutUp => 0.30,
            Butcher => 0.75,
            Smash => 0.10,
        }
    }

    /// Whether the goal is to get components back at all, or only stuff.
    pub fn wants_components(self) -> bool {
        !matches!(self, Teardown::Recycle | Teardown::CutUp | Teardown::Smash)
    }

    /// How long it takes, against undoing the joints carefully.
    pub fn time_multiplier(self) -> f64 {
        use Teardown::*;
        match self {
            Uninstall => 0.15,
            FieldStrip => 0.10,
            Disassemble => 1.00,
            Deconstruct => 1.40,
            Salvage => 0.70,
            Recycle => 0.50,
            Butcher => 0.90,
            CutUp => 0.25,
            Smash => 0.05,
        }
    }

    pub fn name(self) -> &'static str {
        use Teardown::*;
        match self {
            Uninstall => "uninstall",
            FieldStrip => "field strip",
            Disassemble => "disassemble",
            Salvage => "salvage",
            Recycle => "recycle",
            CutUp => "cut up",
            Smash => "smash",
            Butcher => "butcher",
            Deconstruct => "deconstruct",
        }
    }
}

/// **What state a recovered piece is in**, which is what decides whether
/// anybody can use it and for what. A brick that comes off clean goes back
/// in a wall; one still covered in mortar needs a man with a bolster
/// first; a chipped one goes where nobody looks at it; a broken one is
/// hardcore.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecoveryGrade {
    /// Clean and sound: structural reuse.
    IntactClean,
    /// Sound, and still bonded to what it was set in. Reusable after a
    /// cleaning operation somebody has to pay for.
    IntactBonded,
    /// Sound enough for somewhere it will not be seen or loaded.
    Chipped,
    /// Aggregate.
    Broken,
    /// Waste, or a specialist problem.
    Contaminated,
}

/// One component that came back out.
#[derive(Clone, Debug, PartialEq)]
pub struct Returned {
    pub definition: DefId,
    pub count: u32,
    pub mass_kg: f64,
    /// Something wrong with it that will not show until it is used.
    pub hidden_defect: bool,
    /// What state it came out in, which is a different question from how
    /// many came out.
    pub grade: RecoveryGrade,
    /// What it is made of — read off the record, so a part cut from
    /// particleboard comes back as particleboard however the intermediate
    /// was named.
    pub materials: Composition,
    /// What state it is in *now* — which is the state it went in plus
    /// whatever getting it out again cost it.
    pub condition: Condition,
    /// How well it was made, carried through unchanged. Taking a chair
    /// apart does not improve the joinery of its legs.
    pub quality: Quality,
}

/// **What you actually got.**
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recovered {
    pub components: Vec<Returned>,
    /// **Pressed and machined pieces that came off in one piece.** A
    /// door skin recovered from a careful strip-down is a door skin, bent
    /// or not; the same skin out of a shredder is in `materials`.
    pub formed: Vec<(Formed, Condition)>,
    /// Loose material: swarf, offcuts, shredded feedstock, firewood.
    pub materials: Vec<(Material, f64)>,
    /// Mass that went nowhere useful — burnt off, ground away, dropped in
    /// the mud. Counted, so the balance still closes.
    pub lost_kg: f64,
    /// **Recoverable only as heat**, and only if somebody burns it — but
    /// it still knows what it is. A single figure would make oak and wool
    /// the same pile, and then no test could tell whether a chair of
    /// particleboard had quietly yielded oak.
    pub fuel: Vec<(Material, f64)>,
    pub minutes: f64,
}

impl Recovered {
    /// Everything that came back, by mass.
    pub fn mass_kg(&self) -> f64 {
        self.components.iter().map(|r| r.mass_kg).sum::<f64>()
            + self.formed.iter().map(|f| f.0.kg).sum::<f64>()
            + self.materials.iter().map(|m| m.1).sum::<f64>()
            + self.fuel_kg()
    }

    pub fn fuel_kg(&self) -> f64 {
        self.fuel.iter().map(|f| f.1).sum()
    }

    /// Original mass, all destinations counted.
    pub fn accounted_kg(&self) -> f64 {
        self.mass_kg() + self.lost_kg
    }

    pub fn material(&self, m: Material) -> f64 {
        self.materials.iter().find(|p| p.0 == m).map(|p| p.1).unwrap_or(0.0)
    }

    pub fn fuel_of(&self, m: Material) -> f64 {
        self.fuel.iter().find(|p| p.0 == m).map(|p| p.1).unwrap_or(0.0)
    }

    /// For a caller that has worked out a recovery itself — `wip` releases
    /// real component identities and reports the rest as material.
    pub fn add_material_public(&mut self, m: Material, kg: f64) {
        self.add_material(m, kg)
    }

    fn add_material(&mut self, m: Material, kg: f64) {
        if kg <= 0.0 {
            return;
        }
        match self.materials.iter_mut().find(|p| p.0 == m) {
            Some(p) => p.1 += kg,
            None => self.materials.push((m, kg)),
        }
    }

    fn add_fuel(&mut self, m: Material, kg: f64) {
        if kg <= 0.0 {
            return;
        }
        match self.fuel.iter_mut().find(|p| p.0 == m) {
            Some(p) => p.1 += kg,
            None => self.fuel.push((m, kg)),
        }
    }
}

/// **Take it apart.**
///
/// Reads the assembly record where there is one and falls back to the
/// composition where there is not — which is honest about what is known:
/// an item that arrived as a statistic has no history to read, so the best
/// anybody can do with it is weigh it and shred it.
pub fn take_apart(
    item: &ItemInstance,
    how: Teardown,
    cat: &Catalogue,
    // `skill` is the person's ability at it, 0..1. Care sets the ceiling
    // and skill says how near it they get.
    skill: f64,
    // `event` identifies this particular taking-apart, so that the same
    // one cannot be rerolled and a different one may go differently.
    event: u64,
) -> Recovered {
    let mut out = Recovered::default();
    let total = item.mass_kg;
    let skill = skill.clamp(0.0, 1.0);

    // **A damaged item yields less**, which is what makes salvaging a
    // wreck worse than salvaging a working machine.
    let sound = item.condition.serviceability().clamp(0.0, 1.0);

    match (&item.assembly, how.wants_components()) {
        // ---- there is a record, and somebody wants the parts ----------
        (Some(rec), true) => {
            let minutes = 4.0 * rec.components.len() as f64 * how.time_multiplier()
                / (0.4 + 0.6 * skill);
            out.minutes = minutes;
            recover_from_record(&mut out, rec, item, how, skill, sound, event);
        }
        // ---- destructive, or nothing is known about how it was made --
        _ => {
            out.minutes = 3.0 * how.time_multiplier() / (0.4 + 0.6 * skill);
            shred(&mut out, &item.materials, total, how, skill, sound);
        }
    }

    // Whatever did not land anywhere is lost, and it is counted rather
    // than quietly dropped.
    let landed = out.mass_kg();
    out.lost_kg = (total - landed).max(0.0);
    let _ = cat;
    out
}

fn recover_from_record(
    out: &mut Recovered,
    rec: &AssemblyRecord,
    item: &ItemInstance,
    how: Teardown,
    skill: f64,
    sound: f64,
    event: u64,
) {
    // A field strip goes only as far as the modules; it does not separate
    // what was glued, welded, cast or crimped.
    let modules_only = how == Teardown::FieldStrip;
    let shared = common_mode(event, how);

    // **The object weighs what it weighs.** If the record and the object
    // have drifted apart — moisture, wear, a repair — the object is the
    // authority, or a teardown would hand back mass that is not there.
    let recorded = rec.total_component_mass();
    let scale = if recorded > 0.0 { (item.mass_kg / recorded).min(1.0) } else { 0.0 };

    for (index, comp) in rec.components.iter().enumerate() {
        // **The fastener names the joint**, so a screw comes out of a
        // glued frame by unscrewing.
        let method = comp.held_by;
        if modules_only
            && !matches!(method, JointMethod::Bolted | JointMethod::Screwed | JointMethod::Clipped)
        {
            continue; // left assembled: it is part of a module
        }

        let r = method.recovery();
        let survives = (r.components * how.care() * (0.55 + 0.45 * skill) * (0.4 + 0.6 * sound))
            .clamp(0.0, 1.0);
        let kg = comp.mass_kg * scale;

        // **Some components are countable and some are stuff.** Four legs
        // come back as legs or not at all; the part-sheet a seat was cut
        // from comes back as so much board.
        let count = match comp.quantity {
            crate::material::Quantity::Count(n) => Some(n),
            crate::material::Quantity::Stock { count, .. } => Some(count),
            _ => None,
        };
        let Some(count) = count else {
            deposit(out, &comp.materials, kg * survives);
            continue;
        };

        // **A Bernoulli, not a rounding.** Flooring makes a lone part
        // unrecoverable at any probability under one; rounding makes it
        // certain at any probability over a half. Neither is a chance.
        let whole = survivors(event, index, count, survives);
        let each = if count > 0 { kg / count as f64 } else { 0.0 };

        if whole > 0 {
            // **The condition it went in at, plus what coming out cost
            // it.** A chair built from salvaged timber does not yield new
            // timber, and undoing a weld leaves the edges chewed.
            let mut cond = comp.condition_at_install;
            cond.wear = (cond.wear + 0.10 + if r.needs_cutting { 0.15 } else { 0.0 }).min(1.0);
            cond.damage = (cond.damage + item.condition.damage * 0.5).min(1.0);
            // Four separate questions, four separate draws, plus how
            // badly the job as a whole went.
            let knocked = draw(event, index, 0, "damage severity");
            cond.damage = (cond.damage + knocked * (1.0 - how.care()) + shared * 0.4).min(1.0);
            cond.contamination = (cond.contamination
                + draw(event, index, 0, "contamination") * (1.0 - how.care()) * 0.6)
                .min(1.0);
            let hidden = draw(event, index, 0, "hidden defect")
                < (0.05 + 0.25 * (1.0 - how.care()) + 0.3 * shared);
            // **How many came back and what state they are in are two
            // questions.** A mortared joint gives the brick back bonded;
            // a cut one leaves it chipped; contamination is its own axis.
            let grade = if cond.contamination > 0.5 {
                RecoveryGrade::Contaminated
            } else if cond.damage > 0.6 {
                RecoveryGrade::Broken
            } else if r.needs_cutting || cond.damage > 0.25 {
                RecoveryGrade::Chipped
            } else if matches!(
                method,
                JointMethod::LimeMortared | JointMethod::CementMortared | JointMethod::Glued
            ) {
                RecoveryGrade::IntactBonded
            } else {
                RecoveryGrade::IntactClean
            };
            out.components.push(Returned {
                definition: comp.definition,
                count: whole,
                mass_kg: each * whole as f64,
                materials: comp.materials.clone(),
                grade,
                condition: cond,
                // **Workmanship is not touched.** Pulling a leg off a
                // badly made chair gives you a badly made leg.
                quality: comp.quality_at_install,
                // **Something wrong with it that nobody can see yet.** A
                // hairline crack in a casting, a strained thread. Real,
                // and the reason salvaged parts are cheaper.
                hidden_defect: hidden,
            });
        }
        // The rest of that component is scrap of whatever it was made of.
        let broken = (count - whole) as f64 * each;
        if broken > 0.0 {
            scrap_out(out, &comp.materials, broken, how, skill);
        }
    }

    // **A formed part is neither.** Taken off carefully it is still that
    // shape — a bent door skin is a door skin — and only cutting,
    // crushing or shredding turns it back into the sheet it came from.
    let destructive = matches!(
        how,
        Teardown::Recycle | Teardown::CutUp | Teardown::Smash
    );
    for (k, f) in rec.formed.iter().enumerate() {
        let mut piece = *f;
        piece.kg *= scale;
        let survives = f.survives_separation(destructive)
            && draw(event, 1000 + k, 0, "formed separability")
                < how.care() * (0.55 + 0.45 * skill) * sound;
        if survives {
            let mut cond = Condition::fresh();
            cond.damage = (draw(event, 1000 + k, 0, "damage severity") * (1.0 - how.care())
                + shared * 0.4)
                .min(1.0);
            out.formed.push((piece, cond));
        } else {
            deposit_at(out, piece.material, piece.kg * how.care() * (0.6 + 0.4 * skill));
        }
    }

    // **The body of the thing.** Not a component and not a joint: the
    // pressed shell of a toaster is steel, and what it comes back as is
    // decided by the material and by how much care was taken.
    for &(m, kg) in &rec.bulk {
        deposit_at(out, m, kg * scale * how.care() * (0.55 + 0.45 * skill) * sound);
    }

    // **Glue, solder, welding wire and thread do not come back pristine.**
    // What the joint gives up is set by the method; what the material can
    // ever be is set by the material.
    for &(m, kg) in &rec.consumed {
        let via = rec.joints.iter().map(|j| j.method.recovery().fastener).fold(0.0f64, f64::max);
        deposit_at(out, m, kg * scale * via * how.care());
    }
}

/// No record, or nobody cares: weigh it and take what the materials allow.
fn shred(
    out: &mut Recovered,
    materials: &Composition,
    total_kg: f64,
    how: Teardown,
    skill: f64,
    sound: f64,
) {
    let yield_fraction = how.care() * (0.6 + 0.4 * skill) * (0.5 + 0.5 * sound);
    for (m, kg) in materials.masses(total_kg) {
        deposit_at(out, m, kg * yield_fraction);
    }
}

fn scrap_out(out: &mut Recovered, materials: &Composition, kg: f64, how: Teardown, skill: f64) {
    shred(out, materials, kg, how, skill, 1.0);
}

/// Put a known mass where its materials allow it to go, with no further
/// discount — for a caller that has already worked out how much survived.
fn deposit(out: &mut Recovered, materials: &Composition, kg: f64) {
    for (m, mkg) in materials.masses(kg) {
        deposit_at(out, m, mkg);
    }
}

/// **What a material can ever come back as** is a property of the
/// material, and the only place that question is asked.
fn deposit_at(out: &mut Recovered, m: Material, kg: f64) {
    match m.recovers() {
        // Recycling is the route that gets the full material back.
        Recovers::Feedstock => out.add_material(m, kg),
        Recovers::Downcycled => out.add_material(m, kg * 0.55),
        Recovers::Fuel => out.add_fuel(m, kg),
        Recovers::Nothing => {}
    }
}

/// **The heat in what is left.** Real gross calorific values: dry wood
/// ~16 MJ/kg, textiles ~18, rubber ~32. Burning it is a real destination
/// and not a second-best kind of recycling.
pub fn heat_mj(m: Material, kg: f64) -> f64 {
    let per_kg = match m {
        Material::Rubber => 32.0,
        Material::Leather => 19.0,
        Material::Cotton | Material::Wool | Material::Thread => 18.0,
        Material::Oak | Material::Pine | Material::Plywood | Material::Particleboard => 16.5,
        _ => 15.0,
    };
    kg * per_kg
}

/// **Whether this action can be attempted at all.**
///
/// A field strip needs an assembly to open; recycling wants something that
/// can be shredded; uninstalling needs something to be installed. Refusing
/// up front is better than returning an empty pile and letting the caller
/// guess why.
pub fn possible(item: &ItemInstance, how: Teardown) -> bool {
    match how {
        // **Whether it is fitted to something is the store's question**,
        // not the instance's. What an instance can answer on its own is
        // whether anything is fitted to *it*.
        Teardown::Uninstall => !item.attachments.is_empty(),
        // **Having a bill is not having parts.** Every definition now says
        // what it is made of, so a board has a record too — and a board
        // still cannot be disassembled, because there is nothing in it
        // that comes out as a component.
        Teardown::FieldStrip | Teardown::Disassemble => {
            item.assembly.as_ref().map(|a| !a.components.is_empty()).unwrap_or(false)
        }
        _ => true,
    }
}

/// **The difference between demolition and deconstruction**, expressed as
/// a comparison rather than asserted: same object, same person, two
/// intentions.
pub fn compare(
    item: &ItemInstance,
    a: Teardown,
    b: Teardown,
    cat: &Catalogue,
    skill: f64,
    event: u64,
) -> (Recovered, Recovered) {
    (take_apart(item, a, cat, skill, event), take_apart(item, b, cat, skill, event))
}
