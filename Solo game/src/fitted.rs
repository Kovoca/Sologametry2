//! **Installing something is a move, not a copy.**
//!
//! `craft.rs` makes objects and `teardown.rs` takes them apart; this is
//! the seam where one of them goes *into* something else and stays the
//! same object while it is there. The alternator fitted to a van is
//! alternator #418 with its own hours, its own bearing fault and its own
//! maker — not a fresh `Part::Alternator(1500)` conjured from a table, and
//! not a copy of a thing that is also still on a shelf.
//!
//! **The state that must never exist** is the same component in a
//! stockroom and in a lorry at once. `item::Placement` makes it
//! unrepresentable and `Store` is the only thing that changes it, so an
//! installation is atomic by construction rather than by discipline.
//!
//! Two adapters, and they differ in the right way:
//!
//! - **A vehicle** is a grid of parts, so a mount is a tile and fitting
//!   something to it changes mass, balance, what the machine can do and
//!   what the road can see.
//! - **A building** is layers, and only some of it stays an identifiable
//!   object. A door, a window, a socket and a radiator come out and go
//!   back in. Studs and plasterboard are stock. Mortar, adhesive and
//!   sealant are **joint mass** and are never on a shelf again.

use crate::id::{Arena, Id};
use crate::item::{
    AssemblyRecord, Catalogue, Condition, DefId, Fault, Fitting, Host, Installed, ItemEnd,
    ItemInstance, Joint, JointMethod, Placement, Quality, Refusal, Store,
};
use crate::material::{Composition, Material, Quantity};
use crate::rng::Rng;
use crate::save::channel;
use crate::teardown::{take_apart, Recovered, Teardown};
use crate::vehicle::{Part, Vehicle};

// =====================================================================
// the record of putting one thing into another
// =====================================================================

/// **What was done, when, by whom, and what it cost to do it.**
///
/// Kept apart from the component itself because the fasteners and the
/// sealant are *sacrificed* installing it, and a record that folded them
/// together could not say that taking the door off returns the door and
/// not the mastic.
#[derive(Clone, Debug, PartialEq)]
pub struct Installation {
    pub item: Id<ItemInstance>,
    pub host: Host,
    pub mount: usize,
    pub joint: JointMethod,
    /// Bolts, screws, mastic, weld metal. Gone into the joint.
    pub consumed: Vec<(Material, f64)>,
    pub installer: Option<u64>,
    pub installed_on_day: u32,
}

/// Somewhere one thing goes into another.
#[derive(Clone, Debug, PartialEq)]
pub struct Mount {
    pub name: &'static str,
    /// Where on the host, in tiles. Meaningless for a wall, which is why
    /// a building's mounts all sit at the origin.
    pub at: (i32, i32),
    pub takes: Fitting,
    /// **What the host's own physics sees when this mount is occupied.**
    /// The item carries identity and condition; the part carries what the
    /// machine can do with it.
    pub provides: Option<Part>,
    pub joint: JointMethod,
    pub occupant: Option<Id<ItemInstance>>,
}

impl Mount {
    pub fn new(name: &'static str, at: (i32, i32), takes: Fitting, joint: JointMethod) -> Self {
        Mount {
            name,
            at,
            takes,
            provides: None,
            joint,
            occupant: None,
        }
    }

    pub fn providing(mut self, part: Part) -> Self {
        self.provides = Some(part);
        self
    }

    pub fn occupied(&self) -> bool {
        self.occupant.is_some()
    }
}

/// Why something would not go in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WontFit {
    NoSuchMount,
    NoSuchItem,
    Occupied,
    Empty,
    /// The shape is wrong.
    DoesNotFit,
    /// It is somewhere else already — fitted, committed, or folded into a
    /// lot.
    NotAvailable(&'static str),
}

impl From<Refusal> for WontFit {
    fn from(r: Refusal) -> Self {
        match r {
            Refusal::NoSuchItem => WontFit::NoSuchItem,
            Refusal::PointTaken => WontFit::Occupied,
            Refusal::AlreadyInstalled => WontFit::NotAvailable("it is already fitted"),
            _ => WontFit::DoesNotFit,
        }
    }
}

// =====================================================================
// a vehicle whose parts are real things
// =====================================================================

/// **A vehicle, with some of its parts made individual.**
///
/// Which parts is a choice, and it is the same choice CDDA makes about
/// firearms: the ones that fail, are replaced, are traded and are
/// serviced. The rest stays nominal structure, because a model that
/// individuates every gusset costs more to keep than it can ever repay.
#[derive(Clone, Debug)]
pub struct FittedVehicle {
    pub id: u32,
    /// The parts that are not individually identified — chassis, bodywork,
    /// the things nobody takes off.
    pub base: Vehicle,
    pub mounts: Vec<Mount>,
    pub installations: Vec<Installation>,
}

impl FittedVehicle {
    /// **Pull the named kinds of part out of a vehicle and make them
    /// objects.** What comes back is the vehicle with holes in it and the
    /// loose components that used to fill them — which is exactly what an
    /// adapter is, stated once rather than written out per vehicle.
    pub fn adapt(
        id: u32,
        base: Vehicle,
        individual: &[(
            fn(Part) -> bool,
            &'static str,
            &'static str,
            Fitting,
            JointMethod,
        )],
        cat: &Catalogue,
        store: &mut Store,
    ) -> (Self, Vec<Id<ItemInstance>>) {
        let mut kept: Vec<(Part, i32, i32)> = Vec::new();
        let mut mounts = Vec::new();
        let mut loose = Vec::new();
        for &(part, x, y) in &base.parts {
            match individual.iter().find(|(matches, ..)| matches(part)) {
                Some(&(_, name, def_name, takes, joint)) => {
                    mounts.push(Mount::new(name, (x, y), takes, joint).providing(part));
                    if let Some(def) = cat.named(def_name) {
                        let item = ItemInstance::one(cat, def);
                        loose.push(store.add(item, Placement::Ground { locality: 0, x, y }));
                    }
                }
                None => kept.push((part, x, y)),
            }
        }
        let mut stripped = base.clone();
        stripped.parts = kept;
        (
            FittedVehicle {
                id,
                base: stripped,
                mounts,
                installations: Vec::new(),
            },
            loose,
        )
    }

    /// **The vehicle as it actually stands**, structure plus whatever is
    /// fitted. Everything the physics wants — speed, balance, generation,
    /// what the road can see — is computed from this and not from a
    /// specification sheet.
    pub fn assembled(&self) -> Vehicle {
        let mut v = self.base.clone();
        for m in &self.mounts {
            if let (Some(p), true) = (m.provides, m.occupied()) {
                v.parts.push((p, m.at.0, m.at.1));
            }
        }
        v
    }

    /// **Kerb mass, counting every fitted thing exactly once.**
    ///
    /// The structure is nominal and the fitted components weigh what they
    /// actually weigh — which is the point of making them objects, since a
    /// rebuilt alternator and a new one are not the same mass.
    pub fn kerb_kg(&self, store: &Store) -> f64 {
        let structure: f64 = self.base.parts.iter().map(|&(p, _, _)| p.mass_kg()).sum();
        let fitted: f64 = self
            .mounts
            .iter()
            .filter_map(|m| m.occupant)
            .filter_map(|i| store.get(i))
            .map(|i| i.mass_kg)
            .sum();
        structure + fitted
    }

    pub fn mount_named(&self, name: &str) -> Option<usize> {
        self.mounts.iter().position(|m| m.name == name)
    }

    /// **Fit it.** The item leaves wherever it was and arrives here, in
    /// one step, and the fasteners used doing it are gone.
    pub fn install(
        &mut self,
        store: &mut Store,
        mount: usize,
        item: Id<ItemInstance>,
        cat: &Catalogue,
        consumed: &[(Material, f64)],
        installer: Option<u64>,
        day: u32,
    ) -> Result<(), WontFit> {
        let m = self.mounts.get(mount).ok_or(WontFit::NoSuchMount)?;
        if m.occupied() {
            return Err(WontFit::Occupied);
        }
        let takes = m.takes;
        let joint = m.joint;
        if !store.available(item) {
            return Err(WontFit::NotAvailable(store.why_not(item)));
        }
        {
            let i = store.get(item).ok_or(WontFit::NoSuchItem)?;
            let d = cat.get(i.definition).ok_or(WontFit::NoSuchItem)?;
            if d.fits != Some(takes) {
                return Err(WontFit::DoesNotFit);
            }
        }
        store.place(
            item,
            Placement::Installed {
                host: Host::Vehicle(self.id),
                mount,
            },
        );
        self.mounts[mount].occupant = Some(item);
        self.installations.push(Installation {
            item,
            host: Host::Vehicle(self.id),
            mount,
            joint,
            consumed: consumed.to_vec(),
            installer,
            installed_on_day: day,
        });
        Ok(())
    }

    /// **Take it off.** The same object comes back, with everything that
    /// has happened to it since it went on.
    pub fn uninstall(
        &mut self,
        store: &mut Store,
        mount: usize,
    ) -> Result<Id<ItemInstance>, WontFit> {
        let m = self.mounts.get_mut(mount).ok_or(WontFit::NoSuchMount)?;
        let item = m.occupant.take().ok_or(WontFit::Empty)?;
        store.place(item, Placement::anywhere());
        self.installations.retain(|i| i.item != item);
        Ok(item)
    }

    /// **Wreck the mount, and settle what was in it.**
    ///
    /// The rule this replaces was "destroying the mount destroys the
    /// occupant", which prevented identity leaking and bought universal
    /// annihilation instead. A component in a crashed vehicle may stay
    /// bolted to the wreck, come off intact, come off bent, be jammed
    /// where nobody can reach it, split and spill what was in it, or be
    /// broken up — and which of those depends on the joint, the impact and
    /// how strong the thing is.
    ///
    /// **Exactly one outcome, and the mass reaches it.** Nothing is
    /// duplicated and nothing quietly disappears.
    pub fn wreck_mount(
        &mut self,
        store: &mut Store,
        mount: usize,
        severity: f64,
        cat: &Catalogue,
        event: u64,
        day: u32,
    ) -> Option<InstallationFailure> {
        let (item, joint) = {
            let m = self.mounts.get(mount)?;
            (m.occupant?, m.joint)
        };
        let robustness = store
            .get(item)
            .map(|i| i.quality.structural_integrity * i.condition.serviceability())
            .unwrap_or(0.5);
        let holds_something = store
            .get(item)
            .map(|i| !i.contents.is_empty())
            .unwrap_or(false);
        let outcome = settle_mount(joint, severity, robustness, holds_something, event, mount);

        match &outcome {
            InstallationFailure::RemainsAttached { damage } => {
                if let Some(i) = store.get_mut(item) {
                    i.condition.damage = (i.condition.damage + damage).min(1.0);
                }
            }
            InstallationFailure::Detached { damage } => {
                self.mounts[mount].occupant = None;
                self.installations.retain(|i| i.item != item);
                store.place(item, Placement::anywhere());
                if let Some(i) = store.get_mut(item) {
                    i.condition.damage = (i.condition.damage + damage).min(1.0);
                }
            }
            InstallationFailure::Inaccessible => {
                if let Some(i) = store.get_mut(item) {
                    i.condition.damage = (i.condition.damage + 0.2 * severity).min(1.0);
                    i.faults.push(Fault {
                        what: "jammed in the wreckage",
                        severity: 0.9,
                        disabling: false,
                        since_day: day,
                    });
                }
            }
            InstallationFailure::ContentsReleased => {
                let spilt: Vec<_> = store
                    .get(item)
                    .map(|i| i.contents.clone())
                    .unwrap_or_default();
                for c in spilt {
                    store.place(c, Placement::anywhere());
                }
                if let Some(i) = store.get_mut(item) {
                    i.condition.damage = (i.condition.damage + 0.5).min(1.0);
                }
            }
            InstallationFailure::Destroyed { .. } => {
                self.mounts[mount].occupant = None;
                self.installations.retain(|i| i.item != item);
                store.end(item, ItemEnd::Destroyed, day);
            }
        }

        let _ = cat;
        Some(outcome)
    }

    /// The wreckage of a component, worked out by smashing it — used by a
    /// caller that wants the scrap rather than only the verdict.
    pub fn wreckage_of(item: &ItemInstance, cat: &Catalogue, event: u64) -> Recovered {
        take_apart(item, Teardown::Smash, cat, 0.2, event)
    }

    /// **Structural breakup moves what survives, it does not kill it.**
    ///
    /// When a lorry comes in half, a component on a tile that is still
    /// part of a standing section is still bolted to that section. Only
    /// what was on the ground that is gone has to be settled.
    pub fn break_up(
        &mut self,
        store: &mut Store,
        lost_tiles: &[(i32, i32)],
        severity: f64,
        cat: &Catalogue,
        event: u64,
        day: u32,
    ) -> Vec<(usize, InstallationFailure)> {
        let hit: Vec<usize> = self
            .mounts
            .iter()
            .enumerate()
            .filter(|(_, m)| m.occupied() && lost_tiles.contains(&m.at))
            .map(|(i, _)| i)
            .collect();
        let mut out = Vec::new();
        for mount in hit {
            if let Some(f) = self.wreck_mount(store, mount, severity, cat, event, day) {
                out.push((mount, f));
            }
        }
        // Everything else is still attached to whatever is left standing,
        // which is the point: a hole in the floor is not a total loss.
        self.base
            .parts
            .retain(|&(_, x, y)| !lost_tiles.contains(&(x, y)));
        out
    }

    /// Whatever is fitted, in the order the mounts are declared.
    pub fn fitted(&self) -> impl Iterator<Item = (usize, Id<ItemInstance>)> + '_ {
        self.mounts
            .iter()
            .enumerate()
            .filter_map(|(i, m)| m.occupant.map(|o| (i, o)))
    }
}

/// **What became of a fitted component when its mount was wrecked.**
///
/// Exactly one of these, and every one of them accounts for the mass.
#[derive(Clone, Debug, PartialEq)]
pub enum InstallationFailure {
    /// Still bolted to the wreck, and worse for it.
    RemainsAttached { damage: f64 },
    /// Came off, and is on the ground.
    Detached { damage: f64 },
    /// Still there and nobody can get at it. Not destroyed — which is a
    /// different and much more annoying problem.
    Inaccessible,
    /// It split and what was inside it is out.
    ContentsReleased,
    /// Broken past being a component. What is left is scrap, and
    /// `wreckage_of` says how much of what.
    Destroyed { recoverable: bool },
}

/// **How well a joint holds under load.** A weld holds until the metal
/// round it tears; a clip lets go early and the part is usually fine.
pub fn grip_of(joint: JointMethod) -> f64 {
    match joint {
        JointMethod::Welded | JointMethod::Cast | JointMethod::Forged => 0.95,
        JointMethod::Riveted | JointMethod::CementMortared => 0.85,
        JointMethod::LimeMortared => 0.75,
        JointMethod::Bolted => 0.7,
        JointMethod::Screwed | JointMethod::Crimped => 0.55,
        JointMethod::Glued | JointMethod::Soldered | JointMethod::Stitched => 0.4,
        JointMethod::Clipped => 0.25,
        JointMethod::Cooked | JointMethod::Reacted => 0.6,
    }
}

/// **The chance the joint lets go**, which is the model rather than a
/// figure measured off three hundred samples. Gating this directly is
/// stronger than gating a sample of it: a proportion over 300 draws can
/// pass or fail by luck even when the model is right.
pub fn chance_detached(joint: JointMethod, severity: f64) -> f64 {
    (severity.clamp(0.0, 1.0) * (1.0 - grip_of(joint)) * 1.8).clamp(0.0, 1.0)
}

/// **The chance the component itself is broken**, which the joint has
/// nothing to do with: it is the impact reaching the thing.
pub fn chance_destroyed(severity: f64, robustness: f64) -> f64 {
    (severity.clamp(0.0, 1.0).powf(1.5) * (1.0 - 0.8 * robustness.clamp(0.0, 1.0))).clamp(0.0, 1.0)
}

/// **The chance the wreckage folds round something that is still
/// attached.** Conditional by construction: it is only ever asked about a
/// component that neither broke nor came off.
pub fn chance_jammed(severity: f64) -> f64 {
    ((severity.clamp(0.0, 1.0) - 0.6) * 1.5).clamp(0.0, 1.0)
}

fn settle_mount(
    joint: JointMethod,
    severity: f64,
    robustness: f64,
    holds_something: bool,
    event: u64,
    mount: usize,
) -> InstallationFailure {
    let severity = severity.clamp(0.0, 1.0);
    let robustness = robustness.clamp(0.0, 1.0);

    // **Separate draws for separate questions**, asked as a decision tree
    // rather than four independent verdicts: whether the thing broke,
    // then — only if it did not — whether the joint let go, and only if it
    // did not, whether the wreckage folded round it.
    let mut sep = Rng::new(channel(event, mount as u64, "mount separability"));
    let mut sur = Rng::new(channel(event, mount as u64, "component survival"));
    let mut jam = Rng::new(channel(event, mount as u64, "jamming"));

    let let_go = (sep.next_f32() as f64) < chance_detached(joint, severity);
    let broke = (sur.next_f32() as f64) < chance_destroyed(severity, robustness);

    if broke {
        return if holds_something {
            InstallationFailure::ContentsReleased
        } else {
            InstallationFailure::Destroyed {
                recoverable: severity < 0.85,
            }
        };
    }
    if let_go {
        return InstallationFailure::Detached {
            damage: 0.25 * severity,
        };
    }
    // Still attached — and in a bad enough wreck, folded in where nobody
    // is getting a spanner to it. **Only ever asked about something that
    // is still there**, which is what makes it a tree rather than four
    // verdicts that might contradict each other.
    if (jam.next_f32() as f64) < chance_jammed(severity) {
        return InstallationFailure::Inaccessible;
    }
    InstallationFailure::RemainsAttached {
        damage: 0.15 * severity,
    }
}

/// The parts of a road vehicle worth making individual: the ones that
/// fail, are replaced, are traded and are serviced.
pub fn serviceable_parts() -> Vec<(
    fn(Part) -> bool,
    &'static str,
    &'static str,
    Fitting,
    JointMethod,
)> {
    fn is_alternator(p: Part) -> bool {
        matches!(p, Part::Alternator(_))
    }
    fn is_battery(p: Part) -> bool {
        matches!(p, Part::Battery(_))
    }
    fn is_wheel(p: Part) -> bool {
        matches!(p, Part::Wheel { .. })
    }
    vec![
        (
            is_alternator as fn(Part) -> bool,
            "alternator bracket",
            "alternator",
            Fitting::Bracket,
            JointMethod::Bolted,
        ),
        (
            is_battery,
            "battery tray",
            "battery pack",
            Fitting::BatteryRail,
            JointMethod::Clipped,
        ),
        (
            is_wheel,
            "hub",
            "road wheel",
            Fitting::Hub,
            JointMethod::Bolted,
        ),
    ]
}

// =====================================================================
// a building, which is layers with some objects in it
// =====================================================================

/// One layer of a built assembly: so many of a thing, held in a way.
#[derive(Clone, Debug, PartialEq)]
pub struct Course {
    pub def: DefId,
    pub count: u32,
    pub held_by: JointMethod,
}

/// **A wall, and the distinction that makes it worth modelling.**
///
/// Studs, sheathing, insulation and plasterboard are stock, held by
/// fasteners of known kinds. A door and a socket are fitted items that
/// come out whole. **Mortar, adhesive and sealant are joint mass** — they
/// went into the joint and there is no shelf they can go back to, which is
/// exactly what separates deconstruction from demolition.
#[derive(Clone, Debug)]
pub struct WallAssembly {
    pub id: u32,
    pub courses: Vec<Course>,
    pub joint_mass: Vec<(Material, f64)>,
    pub fixtures: Vec<Mount>,
    pub installations: Vec<Installation>,
    /// Which stages of the strip-down have been done. A wall that has had
    /// its door taken out is a different object from one that has not,
    /// and a sledgehammer arriving later can only wreck what is left.
    pub stripped: Vec<Stage>,
}

/// **The stages of taking a wall down**, in the order a demolition
/// contractor actually does them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Kill the power and the water. Nothing else is safe until this is
    /// done, and it recovers nothing.
    IsolateUtilities,
    /// Sockets, switches, radiators.
    RemoveFittings,
    /// The door and the windows, which come out whole if anybody bothers.
    RemoveOpenings,
    /// Plaster and paint off, which is dirty and returns almost nothing.
    StripFinishes,
    /// Sheathing and barrier off the frame.
    ExposeStructure,
    /// Take the frame apart, or knock the brickwork down.
    SeparateStructure,
    /// Reusable one side, recyclable the other, waste in the skip.
    Sort,
}

impl WallAssembly {
    /// A 2.4 m by 3.0 m timber-framed wall with a door and a socket in it.
    /// About 25 kg to the square metre, which is what such a wall weighs.
    pub fn timber_framed(id: u32, cat: &Catalogue) -> Self {
        WallAssembly {
            id,
            courses: vec![
                Course {
                    def: cat.must("stud"),
                    count: 7,
                    held_by: JointMethod::Screwed,
                },
                Course {
                    def: cat.must("sheathing board"),
                    count: 3,
                    held_by: JointMethod::Screwed,
                },
                Course {
                    def: cat.must("insulation batt"),
                    count: 12,
                    held_by: JointMethod::Clipped,
                },
                Course {
                    def: cat.must("plasterboard sheet"),
                    count: 3,
                    held_by: JointMethod::Screwed,
                },
            ],
            // Jointing compound and mastic. Set, and not coming back.
            joint_mass: vec![(Material::Gypsum, 2.0), (Material::Adhesive, 0.5)],
            fixtures: vec![
                Mount::new("doorway", (0, 0), Fitting::Opening, JointMethod::Screwed),
                Mount::new("back box", (0, 0), Fitting::BackBox, JointMethod::Screwed),
            ],
            installations: Vec::new(),
            stripped: Vec::new(),
        }
    }

    /// **A brick wall, and which mortar it was built in decides almost
    /// everything about taking it down.**
    ///
    /// Lime is softer than the brick, so the joint gives way and the brick
    /// survives; cement is harder than the brick, so the brick gives way.
    /// That is the difference between a reclamation yard and a skip.
    pub fn brick(id: u32, cat: &Catalogue, bond: JointMethod) -> Self {
        WallAssembly {
            id,
            courses: vec![Course {
                def: cat.must("brick"),
                count: 430,
                held_by: bond,
            }],
            joint_mass: vec![(Material::Mortar, 190.0)],
            fixtures: vec![Mount::new("opening", (0, 0), Fitting::Opening, bond)],
            installations: Vec::new(),
            stripped: Vec::new(),
        }
    }

    /// **The order the work is done in.**
    ///
    /// Taking a wall down is a sequence, not a verb. Which is why "a
    /// sledgehammer never saves the door" is not a property of
    /// demolition — it is a property of demolishing a wall with the door
    /// still in it.
    pub fn deconstruction_plan() -> &'static [Stage] {
        &[
            Stage::IsolateUtilities,
            Stage::RemoveFittings,
            Stage::RemoveOpenings,
            Stage::StripFinishes,
            Stage::ExposeStructure,
            Stage::SeparateStructure,
            Stage::Sort,
        ]
    }

    pub fn done(&self, stage: Stage) -> bool {
        self.stripped.contains(&stage)
    }

    /// Carry out one stage. Each takes out of the wall whatever that stage
    /// is *for*, so what a later operation can wreck is only what is
    /// still in it.
    pub fn perform(
        &mut self,
        stage: Stage,
        store: &mut Store,
        cat: &Catalogue,
    ) -> Vec<Id<ItemInstance>> {
        let mut out = Vec::new();
        match stage {
            Stage::IsolateUtilities => {}
            Stage::RemoveFittings | Stage::RemoveOpenings => {
                let want = if stage == Stage::RemoveOpenings {
                    Fitting::Opening
                } else {
                    Fitting::BackBox
                };
                let at: Vec<usize> = self
                    .fixtures
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| m.occupied() && m.takes == want)
                    .map(|(i, _)| i)
                    .collect();
                for i in at {
                    if let Ok(item) = self.uninstall(store, i) {
                        out.push(item);
                    }
                }
            }
            Stage::StripFinishes => {
                // The interior finish comes off first and comes off whole
                // far less often than the frame behind it.
                let finish = cat.named("plasterboard sheet");
                self.courses.retain(|c| Some(c.def) != finish);
            }
            Stage::ExposeStructure => {
                let sheathing = cat.named("sheathing board");
                self.courses.retain(|c| Some(c.def) != sheathing);
            }
            Stage::SeparateStructure | Stage::Sort => {}
        }
        if !self.stripped.contains(&stage) {
            self.stripped.push(stage);
        }
        out
    }

    pub fn fixture_named(&self, name: &str) -> Option<usize> {
        self.fixtures.iter().position(|m| m.name == name)
    }

    pub fn install(
        &mut self,
        store: &mut Store,
        mount: usize,
        item: Id<ItemInstance>,
        cat: &Catalogue,
        consumed: &[(Material, f64)],
        day: u32,
    ) -> Result<(), WontFit> {
        let m = self.fixtures.get(mount).ok_or(WontFit::NoSuchMount)?;
        if m.occupied() {
            return Err(WontFit::Occupied);
        }
        let (takes, joint) = (m.takes, m.joint);
        if !store.available(item) {
            return Err(WontFit::NotAvailable(store.why_not(item)));
        }
        {
            let i = store.get(item).ok_or(WontFit::NoSuchItem)?;
            let d = cat.get(i.definition).ok_or(WontFit::NoSuchItem)?;
            if d.fits != Some(takes) {
                return Err(WontFit::DoesNotFit);
            }
        }
        store.place(
            item,
            Placement::Installed {
                host: Host::Building(self.id),
                mount,
            },
        );
        self.fixtures[mount].occupant = Some(item);
        self.installations.push(Installation {
            item,
            host: Host::Building(self.id),
            mount,
            joint,
            consumed: consumed.to_vec(),
            installer: None,
            installed_on_day: day,
        });
        Ok(())
    }

    pub fn uninstall(
        &mut self,
        store: &mut Store,
        mount: usize,
    ) -> Result<Id<ItemInstance>, WontFit> {
        let m = self.fixtures.get_mut(mount).ok_or(WontFit::NoSuchMount)?;
        let item = m.occupant.take().ok_or(WontFit::Empty)?;
        store.place(item, Placement::anywhere());
        self.installations.retain(|i| i.item != item);
        Ok(item)
    }

    pub fn mass_kg(&self, cat: &Catalogue, store: &Store) -> f64 {
        let courses: f64 = self
            .courses
            .iter()
            .map(|c| cat.get(c.def).map(|d| d.nominal_mass_kg).unwrap_or(0.0) * c.count as f64)
            .sum();
        let joints: f64 = self.joint_mass.iter().map(|j| j.1).sum();
        let fixtures: f64 = self
            .fixtures
            .iter()
            .filter_map(|m| m.occupant)
            .filter_map(|i| store.get(i))
            .map(|i| i.mass_kg)
            .sum();
        courses + joints + fixtures
    }

    /// **The wall as one object, so that the same engine takes it apart.**
    ///
    /// Deconstruction and demolition are not two pieces of code; they are
    /// two intentions handed to `teardown`, and what separates them is the
    /// joint table plus how much care is taken. A wall that is bricks in
    /// mortar gives back very little however careful you are, and a
    /// screwed timber frame gives back most of itself.
    pub fn as_one_object(&self, cat: &Catalogue, store: &Store) -> ItemInstance {
        let mut record = AssemblyRecord::default();
        for c in &self.courses {
            let Some(d) = cat.get(c.def) else { continue };
            record.components.push(Installed {
                definition: c.def,
                quantity: Quantity::Count(c.count),
                mass_kg: d.nominal_mass_kg * c.count as f64,
                materials: d.materials.clone(),
                condition_at_install: Condition::fresh(),
                quality_at_install: Quality::default(),
                held_by: c.held_by,
            });
        }
        for m in &self.fixtures {
            let Some(item) = m.occupant.and_then(|i| store.get(i)) else {
                continue;
            };
            record.components.push(Installed {
                definition: item.definition,
                quantity: Quantity::Count(1),
                mass_kg: item.mass_kg,
                materials: item.materials.clone(),
                condition_at_install: item.condition,
                quality_at_install: item.quality,
                held_by: m.joint,
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
        record.consumed = self.joint_mass.clone();

        let mass = self.mass_kg(cat, store);
        let mut wall = ItemInstance {
            definition: self.courses.first().map(|c| c.def).unwrap_or(DefId(0)),
            quantity: Quantity::Count(1),
            mass_kg: mass,
            materials: Composition::default(),
            quality: Quality::default(),
            condition: Condition::fresh(),
            faults: Vec::new(),
            contents: Vec::new(),
            attachments: Vec::new(),
            assembly: None,
            provenance: Default::default(),
            ownership: Default::default(),
            given_name: None,
            shape: None,
            heat: None,
        };
        wall.materials = record.actual_materials();
        wall.assembly = Some(record);
        wall
    }

    /// Take it down, carefully or otherwise.
    pub fn take_down(
        &self,
        how: Teardown,
        cat: &Catalogue,
        store: &Store,
        skill: f64,
        event: u64,
    ) -> Recovered {
        take_apart(&self.as_one_object(cat, store), how, cat, skill, event)
    }
}

/// Every fitted vehicle and wall in one place, so a host id means
/// something.
#[derive(Debug, Default)]
pub struct Built {
    pub vehicles: Arena<FittedVehicle>,
    pub walls: Arena<WallAssembly>,
}
