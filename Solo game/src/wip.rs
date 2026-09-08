//! **An operation is an irreversible physical transaction.**
//!
//! Between the stock on the rack and the finished thing there is a
//! sequence of real objects — a blank, a stamped shell, a drilled shell, a
//! painted shell — and each of them can be seen, moved, stolen, damaged or
//! left stranded when the power goes off. A model that goes straight from
//! consumed inputs to a completed output cannot say any of that, and it
//! encodes the material-only shortcut into every cutting, stamping and
//! forming operation it will ever have.
//!
//! ```text
//! sheet → blank → stamped shell → drilled shell → coated shell → door
//! ```
//!
//! **The balance is the whole contract:**
//!
//! ```text
//! inputs from stock and environment
//!   = work in progress retained + outputs + scrap + emissions + residue
//! ```
//!
//! Environmental terms are not decoration. Painting adds coating and loses
//! solvent; combustion takes oxygen and gives back exhaust; drying takes
//! water out of the wood and puts it in the air.
//!
//! **And planning failure is not execution failure.** A reservation that
//! cannot secure everything rolls back completely; an operation that has
//! begun never rolls physics back. A stamping that goes wrong leaves a
//! malformed panel, the electricity spent, the tooling worn and the press
//! still occupied — it does not put the pristine sheet back on the rack.

use crate::bom::{Formed, Geometry, MaterialState, Surface};
use crate::craft::Operation;
use crate::id::Id;
use crate::item::{
    Catalogue, Condition, DefId, ItemEnd, ItemInstance, JointMethod, Placement, Quality, Store,
    WorkStatus,
};
use crate::material::{Composition, Dims, Material, Quantity};

// =====================================================================
// what a workpiece is, right now
// =====================================================================

/// Something that has been done to a piece and left a mark on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Feature {
    Hole { count: u32, mm: f64 },
    Cut { length_mm: f64 },
    Bend { degrees: f64 },
    Weld { segments: u32 },
    Coating { layer: Surface, microns: f64 },
}

/// **What a piece of work in progress actually is.**
///
/// Dynamic, and deliberately not a catalogue entry: a blank, a shell and a
/// drilled shell are three states of one object, and inventing a
/// definition for every temporary shape would put hundreds of things in
/// the catalogue that nobody ever stores, trades or names. Only
/// standardised intermediates — dough, chair parts, a primed case — earn a
/// definition, because those are things a person can put on a shelf.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// The design it is on the way to being.
    pub becoming: DefId,
    /// What a person would call it at this moment.
    pub stage: &'static str,
    pub geometry: Geometry,
    pub state: MaterialState,
    pub surface: Surface,
    pub dims: Dims,
    pub features: Vec<Feature>,
    /// **What it came from.** Cutting a sheet ends one identity and starts
    /// two, and each of them remembers which sheet.
    pub lineage: Vec<Id<ItemInstance>>,
}

impl Shape {
    pub fn of(becoming: DefId, stage: &'static str, geometry: Geometry, dims: Dims) -> Self {
        Shape {
            becoming,
            stage,
            geometry,
            state: MaterialState::AsRolled,
            surface: Surface::Bare,
            dims,
            features: Vec::new(),
            lineage: Vec::new(),
        }
    }

    pub fn has(&self, f: Feature) -> bool {
        self.features.contains(&f)
    }

    pub fn holes(&self) -> u32 {
        self.features
            .iter()
            .filter_map(|f| match f {
                Feature::Hole { count, .. } => Some(*count),
                _ => None,
            })
            .sum()
    }

    /// What it will be when it is finished, as a formed part.
    pub fn as_formed(&self, name: &'static str, material: Material, kg: f64) -> Formed {
        Formed {
            name,
            role: self.stage,
            material,
            kg,
            geometry: self.geometry,
            state: self.state,
            surface: self.surface,
        }
    }
}

/// **What went into a heat.**
///
/// Melting ends the *objects*: a chair leg and a car panel stop existing
/// and a billet begins. It must not end the *material provenance*, because
/// that is what carries alloy composition, contamination, whether anything
/// radioactive or hazardous went in, how much of it is recycled content,
/// which lots it came from, and who is answerable for a defective heat.
///
/// Deep ancestry can be compacted later into composition, hazard and
/// source summaries; what may never happen is losing it at the furnace
/// door.
#[derive(Clone, Debug, PartialEq)]
pub struct Heat {
    /// The objects that were charged. Their identities have ended; this
    /// is the record that they were in it.
    pub merged_from: Vec<Id<ItemInstance>>,
    /// What the certificate would say.
    pub composition: Composition,
    /// What got in that should not have. A tramp element in scrap steel is
    /// the reason secondary metal is not the same as primary.
    pub contamination: Vec<(Material, f64)>,
    /// Whether anything in the charge was hazardous, which follows the
    /// metal for ever.
    pub hazardous: bool,
    /// How much of it was scrap rather than ore.
    pub recycled_fraction: f64,
    pub process: &'static str,
}

impl Heat {
    /// Everything that has ever been melted into this metal, however many
    /// times it has been round.
    pub fn ancestry(&self) -> usize {
        self.merged_from.len()
    }
}

/// **Every stage name a workpiece can be at.**
///
/// A stage is authored text, so a save writes the words and a load has to
/// find the same static back. A roster rather than a leaked allocation:
/// a name this build does not know comes back as `unrecorded` and says so,
/// which is honest, and a gate asserts that everything the plans actually
/// use is in the list.
pub const STAGE_NAMES: &[&str] = &[
    "unrecorded",
    "blank",
    "offcut",
    "door blank",
    "outer blank",
    "inner blank",
    "outer skin",
    "inner frame",
    "intrusion beam",
    "stamped shell",
    "drilled shell",
    "coated shell",
    "malformed",
    "reworked",
    "finished",
    "melted",
    "cut to size",
];

/// Find the authored static for a name read back off disk.
pub fn intern_stage(name: &str) -> &'static str {
    STAGE_NAMES
        .iter()
        .copied()
        .find(|s| *s == name)
        .unwrap_or("unrecorded")
}

// =====================================================================
// progress is physical
// =====================================================================

/// **There is no universal 63% complete.**
///
/// What "part way through" means is different for every kind of work, and
/// once it is stated properly the interruption behaviour stops being a
/// special rule and becomes a consequence: a cut that is 60% of the way
/// along its path resumes from there, and a kiln that has fallen to 300 °C
/// has to climb back whatever the schedule says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Progress {
    Cut {
        done_mm: f64,
        total_mm: f64,
    },
    Heat {
        celsius: f64,
        target_c: f64,
        ambient_c: f64,
    },
    Dry {
        moisture: f64,
        target: f64,
    },
    /// **A cure runs on chemistry, not on the mains.** How fast it goes
    /// depends on the temperature and the humidity it is being held at —
    /// which the power failing may change, and may not.
    Cure {
        reacted: f64,
        at_c: f64,
        wants_c: f64,
    },
    Weld {
        done: u32,
        segments: u32,
    },
    Coat {
        microns: f64,
        target_microns: f64,
        layers: u32,
    },
    Assemble {
        joints_done: u32,
        joints: u32,
    },
    Machine {
        features_done: u32,
        features: u32,
        allowance_mm: f64,
    },
    /// The fallback, for work whose state genuinely is only elapsed time.
    Elapsed {
        minutes: f64,
        total: f64,
    },
}

impl Progress {
    /// What a person would call it, once, for a report. **Derived** from
    /// the physical state rather than being the state.
    pub fn fraction(&self) -> f64 {
        let f = match *self {
            Progress::Cut { done_mm, total_mm } => done_mm / total_mm.max(1e-9),
            Progress::Heat {
                celsius,
                target_c,
                ambient_c,
            } => (celsius - ambient_c) / (target_c - ambient_c).max(1e-9),
            Progress::Dry { moisture, target } => {
                if moisture <= target {
                    1.0
                } else {
                    (1.0 - moisture) / (1.0 - target).max(1e-9)
                }
            }
            Progress::Cure { reacted, .. } => reacted,
            Progress::Weld { done, segments } => done as f64 / segments.max(1) as f64,
            Progress::Coat {
                microns,
                target_microns,
                ..
            } => microns / target_microns.max(1e-9),
            Progress::Assemble {
                joints_done,
                joints,
            } => joints_done as f64 / joints.max(1) as f64,
            Progress::Machine {
                features_done,
                features,
                ..
            } => features_done as f64 / features.max(1) as f64,
            Progress::Elapsed { minutes, total } => minutes / total.max(1e-9),
        };
        f.clamp(0.0, 1.0)
    }

    pub fn complete(&self) -> bool {
        self.fraction() >= 1.0 - 1e-9
    }

    /// Work at it for so many minutes, at so much of the nominal rate.
    pub fn advance(&mut self, minutes: f64, rate: f64, nominal_minutes: f64) {
        let share = (minutes * rate / nominal_minutes.max(1e-9)).max(0.0);
        match self {
            Progress::Cut { done_mm, total_mm } => {
                *done_mm = (*done_mm + *total_mm * share).min(*total_mm)
            }
            Progress::Heat {
                celsius,
                target_c,
                ambient_c,
            } => *celsius = (*celsius + (*target_c - *ambient_c) * share).min(*target_c),
            Progress::Dry { moisture, target } => {
                *moisture = (*moisture - (*moisture - *target) * share.min(1.0)).max(*target)
            }
            Progress::Cure {
                reacted,
                at_c,
                wants_c,
            } => *reacted = (*reacted + share * arrhenius(*at_c, *wants_c)).min(1.0),
            Progress::Weld { done, segments } => {
                *done = ((*done as f64) + (*segments as f64) * share)
                    .round()
                    .min(*segments as f64) as u32
            }
            Progress::Coat {
                microns,
                target_microns,
                ..
            } => *microns = (*microns + *target_microns * share).min(*target_microns),
            Progress::Assemble {
                joints_done,
                joints,
            } => {
                *joints_done = ((*joints_done as f64) + (*joints as f64) * share)
                    .round()
                    .min(*joints as f64) as u32
            }
            Progress::Machine {
                features_done,
                features,
                allowance_mm,
            } => {
                *features_done = ((*features_done as f64) + (*features as f64) * share)
                    .round()
                    .min(*features as f64) as u32;
                *allowance_mm = (*allowance_mm * (1.0 - share)).max(0.0);
            }
            Progress::Elapsed { minutes: m, total } => *m = (*m + minutes * rate).min(*total),
        }
    }

    /// **What an interruption leaves behind**, which falls out of the
    /// state rather than being asserted. A cut keeps its path; a kiln
    /// loses its heat; a cure carries on by itself.
    pub fn while_stopped(&mut self, minutes: f64) {
        match self {
            // Heat leaks away. Real: a domestic oven loses roughly 150
            // degrees an hour with the door shut.
            Progress::Heat {
                celsius, ambient_c, ..
            } => *celsius = (*celsius - 150.0 * minutes / 60.0).max(*ambient_c),
            // **A cure does not stop because the lights went out** — but
            // it does not carry on regardless either. It goes at the speed
            // the conditions allow, and if the power was what was holding
            // the room warm then the conditions have changed and the
            // caller has already said so.
            Progress::Cure {
                reacted,
                at_c,
                wants_c,
            } => *reacted = (*reacted + (minutes / 720.0) * arrhenius(*at_c, *wants_c)).min(1.0),
            // Everything else simply waits.
            _ => {}
        }
    }
}

/// **Ten degrees doubles it**, which is the chemist's rule of thumb for a
/// reaction rate near room temperature and close enough for a glue line.
/// Cold enough and it effectively stops; warm enough and it races.
pub fn arrhenius(at_c: f64, wants_c: f64) -> f64 {
    if at_c <= 0.0 {
        // Below freezing most adhesives simply do not go off.
        return 0.0;
    }
    2.0f64.powf((at_c - wants_c) / 10.0).clamp(0.0, 8.0)
}

// =====================================================================
// the transaction
// =====================================================================

/// What was done, in enough detail to be audited afterwards.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessRecord {
    pub operation: Operation,
    pub by: Option<u64>,
    pub at_minute: f64,
    pub note: &'static str,
}

/// **One operation, as a transaction on the world.**
///
/// Every gram that goes in comes out somewhere: still in the workpiece,
/// in a new object, in the scrap bin, up the flue, or as residue nobody
/// can use.
#[derive(Clone, Debug, Default)]
pub struct Transformation {
    /// Whole objects consumed out of stock.
    pub from_stock: Vec<(Id<ItemInstance>, f64)>,
    /// **Taken from the surroundings**: tap water, air, oxygen.
    pub from_environment: Vec<(Material, f64)>,
    /// Went in and is still there, changed.
    pub retained: Vec<Id<ItemInstance>>,
    /// Came into existence.
    pub created: Vec<Id<ItemInstance>>,
    /// Stopped being an object, and how.
    pub ended: Vec<(Id<ItemInstance>, ItemEnd)>,
    pub scrap: Vec<(Material, f64)>,
    pub byproducts: Vec<(Material, f64)>,
    /// **Left the system.** Solvent vapour, steam, exhaust.
    pub emissions: Vec<(Material, f64)>,
    pub energy_kwh: f64,
    pub tool_wear: f64,
    pub record: Option<ProcessRecord>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Balance {
    pub in_kg: f64,
    pub retained_kg: f64,
    pub created_kg: f64,
    pub scrap_kg: f64,
    pub emissions_kg: f64,
}

impl Balance {
    pub fn residual(&self) -> f64 {
        self.in_kg - self.retained_kg - self.created_kg - self.scrap_kg - self.emissions_kg
    }

    pub fn closes(&self) -> bool {
        self.residual().abs() < 1e-6
    }
}

impl Transformation {
    /// Weigh both sides. `before` is what the retained pieces weighed
    /// going in, because a retained piece is on both sides of the sum.
    pub fn balance(&self, store: &Store, before: f64) -> Balance {
        let taken: f64 = self.from_stock.iter().map(|s| s.1).sum();
        let env: f64 = self.from_environment.iter().map(|m| m.1).sum();
        let retained: f64 = self
            .retained
            .iter()
            .filter_map(|&i| store.get(i))
            .map(|i| i.mass_kg)
            .sum();
        let created: f64 = self
            .created
            .iter()
            .filter_map(|&i| store.get(i))
            .map(|i| i.mass_kg)
            .sum();
        Balance {
            in_kg: before + taken + env,
            retained_kg: retained,
            created_kg: created,
            scrap_kg: self.scrap.iter().map(|s| s.1).sum::<f64>()
                + self.byproducts.iter().map(|s| s.1).sum::<f64>(),
            emissions_kg: self.emissions.iter().map(|s| s.1).sum(),
        }
    }

    fn noted(mut self, operation: Operation, by: Option<u64>, at: f64, note: &'static str) -> Self {
        self.record = Some(ProcessRecord {
            operation,
            by,
            at_minute: at,
            note,
        });
        self
    }
}

// =====================================================================
// the identity rules
// =====================================================================

/// **Cutting ends one identity and starts two.**
///
/// A sheet cut into a blank and an offcut is not a sheet any more. Both
/// pieces remember which sheet they came off, which is what makes a
/// material lot traceable and what stops an offcut being conjured from
/// nowhere at the end of a run.
pub fn cut(
    store: &mut Store,
    cat: &Catalogue,
    source: Id<ItemInstance>,
    take_kg: f64,
    becoming: DefId,
    stage: &'static str,
    dims: Dims,
    kerf_kg: f64,
    at: (f64, u32),
) -> Option<(Id<ItemInstance>, Option<Id<ItemInstance>>, Transformation)> {
    // **Both pieces land where the work was done.** An offcut originates
    // at the saw, not on the rack it came off, and putting it back is a
    // movement somebody has to make — see `carry_back`.
    let (whole, comp, def, where_) = {
        let i = store.get(source)?;
        (
            i.mass_kg,
            i.materials.clone(),
            i.definition,
            store.placement(source)?,
        )
    };
    if take_kg + kerf_kg > whole + 1e-9 {
        return None;
    }
    let left = whole - take_kg - kerf_kg;
    let material = comp.chiefly().unwrap_or(Material::MildSteel);

    let mut blank = ItemInstance::fresh(cat, def, Quantity::Mass { kg: take_kg });
    blank.mass_kg = take_kg;
    blank.materials = comp.clone();
    blank.bare_bill(take_kg);
    let mut shape = Shape::of(
        becoming,
        stage,
        Geometry::Sheet {
            mm: dims.height_m * 1000.0,
        },
        dims,
    );
    shape.lineage.push(source);
    shape.features.push(Feature::Cut {
        length_mm: dims.length_m * 2000.0,
    });
    blank.shape = Some(shape);
    let blank_id = store.add(blank, where_);

    let offcut_id = if left > 1e-6 {
        let mut off = ItemInstance::fresh(cat, def, Quantity::Mass { kg: left });
        off.mass_kg = left;
        off.materials = comp.clone();
        off.bare_bill(left);
        let mut s = Shape::of(
            becoming,
            "offcut",
            Geometry::Sheet {
                mm: dims.height_m * 1000.0,
            },
            dims,
        );
        s.lineage.push(source);
        off.shape = Some(s);
        Some(store.add(off, where_))
    } else {
        None
    };

    store.end(source, ItemEnd::Consumed, 0);
    let mut t = Transformation {
        from_stock: vec![(source, whole)],
        created: [Some(blank_id), offcut_id].into_iter().flatten().collect(),
        ended: vec![(source, ItemEnd::Consumed)],
        scrap: if kerf_kg > 0.0 {
            vec![(material, kerf_kg)]
        } else {
            vec![]
        },
        tool_wear: 0.002,
        ..Default::default()
    };
    t = t.noted(Operation::Cut, None, at.0, "cut to size");
    let _ = at.1;
    Some((blank_id, offcut_id, t))
}

/// **Forming preserves identity.** A bent panel is the same panel: same
/// object, same lineage, same mass, different geometry and a different
/// material state, because cold work hardens it.
pub fn form(
    store: &mut Store,
    piece: Id<ItemInstance>,
    geometry: Geometry,
    stage: &'static str,
    at: f64,
) -> Option<Transformation> {
    let before = store.get(piece)?.mass_kg;
    {
        let i = store.get_mut(piece)?;
        let s = i.shape.as_mut()?;
        s.geometry = geometry;
        s.stage = stage;
        s.state = MaterialState::WorkHardened;
        s.features.push(Feature::Bend { degrees: 90.0 });
    }
    let mut t = Transformation {
        retained: vec![piece],
        energy_kwh: 0.8,
        tool_wear: 0.01,
        ..Default::default()
    };
    t = t.noted(Operation::Press, None, at, "pressed to shape");
    let _ = before;
    Some(t)
}

/// **Drilling preserves identity and makes swarf.** The holes are on the
/// piece and the metal that used to be in them is on the floor.
pub fn drill(
    store: &mut Store,
    piece: Id<ItemInstance>,
    holes: u32,
    mm: f64,
    swarf_kg: f64,
    at: f64,
) -> Option<Transformation> {
    let material = {
        let i = store.get(piece)?;
        i.materials.chiefly().unwrap_or(Material::MildSteel)
    };
    {
        let i = store.get_mut(piece)?;
        i.mass_kg -= swarf_kg;
        let s = i.shape.as_mut()?;
        s.features.push(Feature::Hole { count: holes, mm });
    }
    Some(
        Transformation {
            retained: vec![piece],
            scrap: vec![(material, swarf_kg)],
            energy_kwh: 0.05 * holes as f64,
            tool_wear: 0.004 * holes as f64,
            ..Default::default()
        }
        .noted(Operation::Drill, None, at, "punched and drilled"),
    )
}

/// **Coating adds mass and loses solvent.**
///
/// Which is the environmental term made concrete: some of the tin ends up
/// on the panel, and the rest of it ends up in the air. A model that only
/// tracked what went on the panel would quietly destroy the difference.
pub fn coat(
    store: &mut Store,
    piece: Id<ItemInstance>,
    surface: Surface,
    applied_kg: f64,
    solids: f64,
    material: Material,
    at: f64,
) -> Option<Transformation> {
    let stays = applied_kg * solids.clamp(0.0, 1.0);
    let evaporates = applied_kg - stays;
    {
        let i = store.get_mut(piece)?;
        i.mass_kg += stays;
        let s = i.shape.as_mut()?;
        s.surface = surface;
        s.features.push(Feature::Coating {
            layer: surface,
            microns: 40.0,
        });
    }
    Some(
        Transformation {
            from_stock: vec![],
            from_environment: vec![(material, applied_kg)],
            retained: vec![piece],
            emissions: vec![(material, evaporates)],
            energy_kwh: 0.2,
            ..Default::default()
        }
        .noted(Operation::Paint, None, at, "coated"),
    )
}

/// **Joining makes an assembly and the components keep their
/// identities inside it.**
///
/// Which is what lets a door be taken apart later and give back *the*
/// latch rather than *a* latch.
pub fn join(
    store: &mut Store,
    cat: &Catalogue,
    parts: &[Id<ItemInstance>],
    into: DefId,
    method: JointMethod,
    consumed: &[(Material, f64)],
    at: f64,
) -> Option<(Id<ItemInstance>, Transformation)> {
    if parts.is_empty() {
        return None;
    }
    let where_ = store.placement(parts[0])?;
    let mut mass = consumed.iter().map(|c| c.1).sum::<f64>();
    let mut comp_parts: Vec<(Material, f64)> = Vec::new();
    for &p in parts {
        let Some(i) = store.get(p) else { continue };
        mass += i.mass_kg;
        for (m, kg) in i.materials.masses(i.mass_kg) {
            match comp_parts.iter_mut().find(|x| x.0 == m) {
                Some(x) => x.1 += kg,
                None => comp_parts.push((m, kg)),
            }
        }
    }
    for &(m, kg) in consumed {
        match comp_parts.iter_mut().find(|x| x.0 == m) {
            Some(x) => x.1 += kg,
            None => comp_parts.push((m, kg)),
        }
    }

    let mut assembly = ItemInstance::fresh(cat, into, Quantity::Count(1));
    assembly.mass_kg = mass;
    assembly.materials = Composition::of(&comp_parts);
    // **The assembly own bill is the joining material and nothing else.**
    // Its components are real objects sitting inside it, and listing them
    // here as well would count them twice when it comes apart.
    let mut record = crate::item::AssemblyRecord {
        consumed: consumed.to_vec(),
        ..Default::default()
    };
    // **As built**: the actual objects that went in, by handle. The
    // design still says what a door expects; this says what is in this
    // one, and their mass is theirs rather than being counted twice.
    record.as_built = parts.to_vec();
    record.joints = vec![crate::item::Joint {
        method,
        joins: (0, 0),
        fastener: None,
        accessible: true,
    }];
    assembly.assembly = Some(record);
    assembly.shape = None;
    let id = store.add(assembly, where_);

    // **The parts go inside it and stay themselves — and a component of
    // an assembly is not the contents of a box.** A latch in a toolbox can
    // be picked up; a latch welded into a door cannot, and has to be got
    // out by taking the door apart. `Contained` said the first of those
    // about both, so anybody could help themselves to the regulator out of
    // a finished door without dismantling anything.
    for (k, &p) in parts.iter().enumerate() {
        store.place(
            p,
            Placement::Installed {
                host: crate::item::Host::Item(id),
                mount: k,
            },
        );
        store.set_status(p, WorkStatus::Available);
    }
    Some((
        id,
        Transformation {
            retained: parts.to_vec(),
            created: vec![id],
            energy_kwh: 0.4,
            tool_wear: 0.006,
            ..Default::default()
        }
        .noted(Operation::Assemble, None, at, "joined"),
    ))
}

/// **Melting ends the input identities and makes a lot.** A billet is not
/// the swarf it was made from, and nothing can trace back through it.
pub fn melt(
    store: &mut Store,
    cat: &Catalogue,
    inputs: &[Id<ItemInstance>],
    into: DefId,
    loss: f64,
    at: f64,
    day: u32,
) -> Option<(Id<ItemInstance>, Transformation)> {
    let where_ = store.placement(*inputs.first()?)?;
    let mut mass = 0.0;
    let mut comp: Vec<(Material, f64)> = Vec::new();
    for &i in inputs {
        let Some(x) = store.get(i) else { continue };
        mass += x.mass_kg;
        for (m, kg) in x.materials.masses(x.mass_kg) {
            match comp.iter_mut().find(|p| p.0 == m) {
                Some(p) => p.1 += kg,
                None => comp.push((m, kg)),
            }
        }
    }
    let burnt = mass * loss.clamp(0.0, 1.0);
    let out = mass - burnt;
    let material = Composition::of(&comp)
        .chiefly()
        .unwrap_or(Material::MildSteel);

    // **The objects end; the metal remembers.** Which lots were charged,
    // what the composition came out at, whether anything hazardous went
    // in, and how much of it is scrap rather than ore — all of that
    // follows the heat, and losing it at the furnace door is how recycled
    // content, tramp elements and a defective cast stop being anybody's
    // responsibility.
    let recycled: f64 = inputs
        .iter()
        .filter_map(|&i| store.get(i))
        .map(|x| {
            let was_scrap = x.heat.as_ref().map(|h| h.recycled_fraction).unwrap_or(1.0);
            x.mass_kg * was_scrap
        })
        .sum::<f64>()
        / mass.max(1e-9);
    let hazardous = inputs.iter().filter_map(|&i| store.get(i)).any(|x| {
        x.heat.as_ref().map(|h| h.hazardous).unwrap_or(false)
            || x.materials
                .parts()
                .iter()
                .any(|&(m, f)| f > 0.0 && m.hazardous())
    });
    let mut ancestry: Vec<Id<ItemInstance>> = Vec::new();
    for &i in inputs {
        if let Some(h) = store.get(i).and_then(|x| x.heat.as_ref()) {
            ancestry.extend(h.merged_from.iter().copied());
        }
        ancestry.push(i);
    }

    let mut lot = ItemInstance::fresh(cat, into, Quantity::Mass { kg: out });
    lot.mass_kg = out;
    lot.materials = Composition::of(&comp);
    lot.bare_bill(out);
    // **No shape and no lineage of form.** A billet does not remember
    // being a panel, which is what melting *does* end.
    lot.shape = None;
    lot.heat = Some(Heat {
        merged_from: ancestry,
        composition: Composition::of(&comp),
        contamination: comp
            .iter()
            .copied()
            .filter(|(m, _)| *m != material)
            .collect(),
        hazardous,
        recycled_fraction: recycled.clamp(0.0, 1.0),
        process: "melted",
    });
    let id = store.add(lot, where_);
    for &i in inputs {
        store.end(i, ItemEnd::Consumed, day);
    }
    Some((
        id,
        Transformation {
            from_stock: inputs.iter().map(|&i| (i, 0.0)).collect(),
            created: vec![id],
            ended: inputs.iter().map(|&i| (i, ItemEnd::Consumed)).collect(),
            emissions: vec![(material, burnt)],
            energy_kwh: 1.6 * mass,
            ..Default::default()
        }
        .noted(Operation::Cast, None, at, "melted down"),
    ))
}

/// **Stock does not walk back to the rack by itself.**
///
/// An offcut is created at the machine that made it. Getting it back into
/// the racking is a real movement by a real person, and a model in which
/// it simply reappears where the sheet used to live is teleporting stock
/// past its own resource calendar.
///
/// Real: putting a part-sheet back on a rack is a minute or two of
/// somebody's time, more if it wants two people.
pub fn carry_back(
    store: &mut Store,
    piece: Id<ItemInstance>,
    to: Placement,
    hands: u32,
) -> Option<f64> {
    let kg = store.get(piece)?.mass_kg;
    store.place(piece, to);
    // A light piece is a minute; anything over about 25 kg is a two-man
    // lift and takes longer whoever is doing it.
    let minutes = 1.0 + kg / 20.0 + if kg > 25.0 && hands < 2 { 3.0 } else { 0.0 };
    Some(minutes)
}

/// **Completion promotes the workpiece; it does not make a second
/// object.**
///
/// The thing that has been cut, pressed, drilled and painted *is* the
/// door. Creating a fresh door and quietly discarding the workpiece would
/// throw away its lineage, its as-built record and every substitution
/// anybody made along the way.
pub fn complete(
    store: &mut Store,
    piece: Id<ItemInstance>,
    into: DefId,
    at: f64,
) -> Option<Transformation> {
    {
        let i = store.get_mut(piece)?;
        i.definition = into;
        if let Some(s) = i.shape.as_mut() {
            s.becoming = into;
            s.stage = "finished";
        }
    }
    Some(
        Transformation {
            retained: vec![piece],
            ..Default::default()
        }
        .noted(Operation::Test, None, at, "passed off"),
    )
}

/// **An execution failure never rolls physics back.**
///
/// A stamping that goes wrong leaves a malformed panel, the electricity
/// spent, the tooling worn, whatever offcut there was, and a press still
/// occupied. It does not put the pristine sheet back on the rack, and a
/// model that lets it has invented a way to unmake things.
pub fn botched(
    store: &mut Store,
    piece: Id<ItemInstance>,
    wasted_kg: f64,
    energy_kwh: f64,
    tool_wear: f64,
    at: f64,
) -> Option<Transformation> {
    let material = {
        let i = store.get(piece)?;
        i.materials.chiefly().unwrap_or(Material::MildSteel)
    };
    {
        let i = store.get_mut(piece)?;
        i.mass_kg -= wasted_kg.min(i.mass_kg);
        i.condition.damage = (i.condition.damage + 0.45).min(1.0);
        i.quality.dimensional_accuracy *= 0.4;
        if let Some(s) = i.shape.as_mut() {
            s.stage = "malformed";
        }
    }
    Some(
        Transformation {
            retained: vec![piece],
            scrap: vec![(material, wasted_kg)],
            energy_kwh,
            tool_wear,
            ..Default::default()
        }
        .noted(Operation::Press, None, at, "went wrong"),
    )
}

/// **Rework goes at the feature that is wrong.** Doing the whole thing
/// again is a different and much more expensive decision, and a model
/// that cannot tell them apart has to pick one and be wrong half the
/// time.
pub fn rework(
    store: &mut Store,
    piece: Id<ItemInstance>,
    feature: Feature,
    at: f64,
) -> Option<Transformation> {
    {
        let i = store.get_mut(piece)?;
        i.condition.damage = (i.condition.damage - 0.35).max(0.0);
        i.quality.dimensional_accuracy = (i.quality.dimensional_accuracy * 1.6).min(0.9);
        let s = i.shape.as_mut()?;
        if !s.features.contains(&feature) {
            s.features.push(feature);
        }
        s.stage = "reworked";
    }
    Some(
        Transformation {
            retained: vec![piece],
            energy_kwh: 0.1,
            tool_wear: 0.003,
            ..Default::default()
        }
        .noted(Operation::Grind, None, at, "put right"),
    )
}

/// **Taking an assembly apart returns the components that are actually in
/// it**, not the ones the design says it should have.
///
/// Which is the whole reason the parts stay themselves inside it: a door
/// whose latch was replaced with a second-hand one gives back the
/// second-hand latch, and a door built with two brass screws instead of
/// six steel ones gives back two brass screws.
pub fn dismantle(
    store: &mut Store,
    cat: &Catalogue,
    assembly: Id<ItemInstance>,
    how: crate::teardown::Teardown,
    skill: f64,
    event: u64,
    day: u32,
) -> (crate::teardown::Recovered, Vec<Id<ItemInstance>>) {
    use crate::teardown::Recovered;
    let mut out = Recovered::default();
    let mut released = Vec::new();
    let Some(host) = store.get(assembly).cloned() else {
        return (out, released);
    };
    let total = host.mass_kg;
    let where_ = store
        .placement(assembly)
        .unwrap_or_else(Placement::anywhere);

    let joint = host
        .assembly
        .as_ref()
        .and_then(|a| a.joints.first().map(|j| j.method))
        .unwrap_or(JointMethod::Bolted);
    let r = joint.recovery();
    let survives = r.components * how.care() * (0.55 + 0.45 * skill.clamp(0.0, 1.0));

    // **What is actually in it**, which the record knows by handle. An
    // assembly built by `join` holds its children as fitted components
    // rather than as loose contents; anything spawned or filled some other
    // way still keeps them in `contents`.
    let children: Vec<Id<ItemInstance>> = match host.assembly.as_ref() {
        Some(a) if !a.as_built.is_empty() => a.as_built.clone(),
        _ => host.contents.clone(),
    };
    for (k, &part) in children.iter().enumerate() {
        let Some(p) = store.get(part).cloned() else {
            continue;
        };
        let u = crate::rng::Rng::new(crate::save::channel(event, k as u64, "part separability"))
            .next_f32() as f64;
        if u < survives {
            // **The same object, back on the floor.**
            store.place(part, where_);
            store.set_status(part, WorkStatus::Available);
            if let Some(x) = store.get_mut(part) {
                x.condition.damage = (x.condition.damage + 0.05).min(1.0);
            }
            released.push(part);
        } else {
            store.end(part, ItemEnd::Destroyed, day);
            for (m, kg) in p.materials.masses(p.mass_kg) {
                out.add_material_public(m, kg * 0.6);
            }
        }
    }

    // The joining material behaves as it always does.
    if let Some(rec) = &host.assembly {
        for &(m, kg) in &rec.consumed {
            out.add_material_public(m, kg * r.fastener * how.care());
        }
        for &(m, kg) in &rec.bulk {
            out.add_material_public(m, kg * how.care());
        }
    }

    let recovered_mass: f64 = released
        .iter()
        .filter_map(|&i| store.get(i))
        .map(|i| i.mass_kg)
        .sum::<f64>()
        + out.materials.iter().map(|m| m.1).sum::<f64>()
        + out.fuel_kg();
    out.lost_kg = (total - recovered_mass).max(0.0);
    store.end(assembly, ItemEnd::Destroyed, day);
    let _ = cat;
    (out, released)
}

// =====================================================================
// what becomes of a thing that is not finished with
// =====================================================================

/// **Not everything that stops being used is thrown away.**
///
/// A sound washing machine with no scrap dealer nearby is not buried; it
/// sits in a yard, or goes up for sale, or waits for a lorry, or has its
/// motor taken for something else. The household basket needs all of
/// these for repair, replacement and the second-hand trade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposition {
    InUse,
    /// Put away, still whole, still somebody's.
    Stored,
    OfferedForSale,
    /// Waiting for somebody with a lorry.
    AwaitingTransport,
    /// Being kept for the parts that come off it.
    Cannibalized,
    /// Nobody's, and nobody is coming back for it.
    Abandoned,
}

impl Disposition {
    /// Whether the thing is still a thing, as opposed to on its way to
    /// stopping being one.
    pub fn still_whole(self) -> bool {
        !matches!(self, Disposition::Cannibalized)
    }

    /// Whether anybody still claims it.
    pub fn owned(self) -> bool {
        !matches!(self, Disposition::Abandoned)
    }
}

/// **Where a thing goes when its owner is finished with it**, before any
/// question of destroying it arises. Only when none of these applies is an
/// `EndOfLife` route the answer.
pub fn what_becomes_of_it(
    condition: f64,
    somebody_wants_it: bool,
    a_buyer_is_reachable: bool,
    parts_are_wanted: bool,
    still_owned: bool,
) -> Disposition {
    if !still_owned {
        return Disposition::Abandoned;
    }
    if condition > 0.6 && somebody_wants_it {
        return Disposition::InUse;
    }
    if condition > 0.4 && a_buyer_is_reachable {
        return Disposition::OfferedForSale;
    }
    if parts_are_wanted && condition < 0.5 {
        return Disposition::Cannibalized;
    }
    if condition > 0.2 {
        return Disposition::Stored;
    }
    Disposition::AwaitingTransport
}

// =====================================================================
// reservations
// =====================================================================

/// Why a set of reservations could not be taken.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CannotReserve {
    NotAvailable(Id<ItemInstance>),
    NoSuchItem(Id<ItemInstance>),
}

/// **All of it or none of it.**
///
/// A reservation that cannot secure everything rolls back completely —
/// which is the half of the planning/execution distinction that *is*
/// allowed to undo itself, and the reason it must be kept clearly apart
/// from the half that is not.
pub fn reserve_all(
    store: &mut Store,
    order: u64,
    items: &[Id<ItemInstance>],
) -> Result<(), CannotReserve> {
    let mut taken = Vec::new();
    for &i in items {
        if store.get(i).is_none() {
            release_all(store, &taken);
            return Err(CannotReserve::NoSuchItem(i));
        }
        if !store.available(i) {
            release_all(store, &taken);
            return Err(CannotReserve::NotAvailable(i));
        }
        store.set_status(i, WorkStatus::Reserved { order });
        taken.push(i);
    }
    Ok(())
}

pub fn release_all(store: &mut Store, items: &[Id<ItemInstance>]) {
    for &i in items {
        store.set_status(i, WorkStatus::Available);
    }
}

/// **Moving or damaging work in progress invalidates the reservation on
/// it.** A board booked for tomorrow and carried off tonight is not
/// booked for tomorrow any more, whatever the paperwork says.
pub fn still_good(store: &Store, order: u64, items: &[Id<ItemInstance>]) -> Vec<Id<ItemInstance>> {
    items
        .iter()
        .copied()
        .filter(|&i| {
            store.get(i).is_none()
                || store.status(i) != WorkStatus::Reserved { order }
                || !store.placement(i).map(|p| p.reachable()).unwrap_or(false)
        })
        .collect()
}

/// A workpiece begins as a fresh object made out of stock.
pub fn start(
    store: &mut Store,
    cat: &Catalogue,
    from: Id<ItemInstance>,
    becoming: DefId,
    order: u64,
    at_resource: u32,
) -> Option<Id<ItemInstance>> {
    let dims = store
        .get(from)
        .and_then(|i| cat.get(i.definition))
        .map(|d| d.nominal)?;
    let kg = store.get(from)?.mass_kg;
    let (blank, _, _) = cut(store, cat, from, kg, becoming, "blank", dims, 0.0, (0.0, 0))?;
    store.place(
        blank,
        Placement::Fixtured {
            resource: at_resource,
            slot: 0,
            clamped: true,
        },
    );
    store.set_status(
        blank,
        WorkStatus::Wip {
            order,
            operation: 0,
        },
    );
    Some(blank)
}

/// Convenience for a fresh piece with nothing but material in it.
impl ItemInstance {
    pub fn bare_bill(&mut self, kg: f64) {
        let record = crate::item::AssemblyRecord {
            bulk: self.materials.masses(kg),
            ..Default::default()
        };
        self.assembly = Some(record);
        self.quality = Quality::default();
        self.condition = Condition::fresh();
    }
}
