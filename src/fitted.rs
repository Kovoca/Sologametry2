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
    AssemblyRecord, Catalogue, Condition, DefId, Fitting, Host, Installed, ItemInstance, Joint,
    JointMethod, Placement, Quality, Refusal, Store,
};
use crate::material::{Composition, Material, Quantity};
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
        Mount { name, at, takes, provides: None, joint, occupant: None }
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
        individual: &[(fn(Part) -> bool, &'static str, &'static str, Fitting, JointMethod)],
        cat: &Catalogue,
        store: &mut Store,
    ) -> (Self, Vec<Id<ItemInstance>>) {
        let mut kept: Vec<(Part, i32, i32)> = Vec::new();
        let mut mounts = Vec::new();
        let mut loose = Vec::new();
        for &(part, x, y) in &base.parts {
            match individual.iter().find(|(matches, ..)| matches(part)) {
                Some(&(_, name, def_name, takes, joint)) => {
                    mounts.push(
                        Mount::new(name, (x, y), takes, joint).providing(part),
                    );
                    if let Some(def) = cat.named(def_name) {
                        let mut item = ItemInstance::one(cat, def);
                        item.placement = Placement::Loose { locality: 0, x, y };
                        loose.push(store.add(item));
                    }
                }
                None => kept.push((part, x, y)),
            }
        }
        let mut stripped = base.clone();
        stripped.parts = kept;
        (FittedVehicle { id, base: stripped, mounts, installations: Vec::new() }, loose)
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
        {
            let i = store.get(item).ok_or(WontFit::NoSuchItem)?;
            if !i.available() {
                return Err(WontFit::NotAvailable(i.placement.why_not()));
            }
            let d = cat.get(i.definition).ok_or(WontFit::NoSuchItem)?;
            if d.fits != Some(takes) {
                return Err(WontFit::DoesNotFit);
            }
        }
        store.place(item, Placement::Installed { host: Host::Vehicle(self.id), mount });
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
        store.place(item, Placement::Loose { locality: 0, x: 0, y: 0 });
        self.installations.retain(|i| i.item != item);
        Ok(item)
    }

    /// **Destroying the mount destroys what was in it.** It does not fall
    /// out loose, and it certainly does not go on existing somewhere else.
    pub fn destroy_mount(&mut self, store: &mut Store, mount: usize) {
        if let Some(m) = self.mounts.get_mut(mount) {
            if let Some(item) = m.occupant.take() {
                store.destroy(item);
                self.installations.retain(|i| i.item != item);
            }
        }
    }

    /// Whatever is fitted, in the order the mounts are declared.
    pub fn fitted(&self) -> impl Iterator<Item = (usize, Id<ItemInstance>)> + '_ {
        self.mounts.iter().enumerate().filter_map(|(i, m)| m.occupant.map(|o| (i, o)))
    }
}

/// The parts of a road vehicle worth making individual: the ones that
/// fail, are replaced, are traded and are serviced.
pub fn serviceable_parts() -> Vec<(fn(Part) -> bool, &'static str, &'static str, Fitting, JointMethod)>
{
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
        (is_alternator as fn(Part) -> bool, "alternator bracket", "alternator", Fitting::Bracket, JointMethod::Bolted),
        (is_battery, "battery tray", "battery pack", Fitting::BatteryRail, JointMethod::Clipped),
        (is_wheel, "hub", "road wheel", Fitting::Hub, JointMethod::Bolted),
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
}

impl WallAssembly {
    /// A 2.4 m by 3.0 m timber-framed wall with a door and a socket in it.
    /// About 25 kg to the square metre, which is what such a wall weighs.
    pub fn timber_framed(id: u32, cat: &Catalogue) -> Self {
        WallAssembly {
            id,
            courses: vec![
                Course { def: cat.must("stud"), count: 7, held_by: JointMethod::Screwed },
                Course { def: cat.must("sheathing board"), count: 3, held_by: JointMethod::Screwed },
                Course { def: cat.must("insulation batt"), count: 12, held_by: JointMethod::Clipped },
                Course { def: cat.must("plasterboard sheet"), count: 3, held_by: JointMethod::Screwed },
            ],
            // Jointing compound and mastic. Set, and not coming back.
            joint_mass: vec![(Material::Gypsum, 2.0), (Material::Adhesive, 0.5)],
            fixtures: vec![
                Mount::new("doorway", (0, 0), Fitting::Opening, JointMethod::Screwed),
                Mount::new("back box", (0, 0), Fitting::BackBox, JointMethod::Screwed),
            ],
            installations: Vec::new(),
        }
    }

    /// A brick wall of the same size: bricks in mortar, and the mortar is
    /// most of why taking one down carefully is worth so little.
    pub fn brick(id: u32, cat: &Catalogue) -> Self {
        WallAssembly {
            id,
            courses: vec![Course {
                def: cat.must("brick"),
                count: 430,
                held_by: JointMethod::Mortared,
            }],
            joint_mass: vec![(Material::Mortar, 190.0)],
            fixtures: vec![Mount::new("opening", (0, 0), Fitting::Opening, JointMethod::Mortared)],
            installations: Vec::new(),
        }
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
        {
            let i = store.get(item).ok_or(WontFit::NoSuchItem)?;
            if !i.available() {
                return Err(WontFit::NotAvailable(i.placement.why_not()));
            }
            let d = cat.get(i.definition).ok_or(WontFit::NoSuchItem)?;
            if d.fits != Some(takes) {
                return Err(WontFit::DoesNotFit);
            }
        }
        store.place(item, Placement::Installed { host: Host::Building(self.id), mount });
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
        store.place(item, Placement::Loose { locality: 0, x: 0, y: 0 });
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
            let Some(item) = m.occupant.and_then(|i| store.get(i)) else { continue };
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
            placement: Placement::Nowhere,
            assembly: None,
            provenance: Default::default(),
            ownership: Default::default(),
            given_name: None,
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
