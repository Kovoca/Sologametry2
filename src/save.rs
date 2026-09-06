//! **Writing a world down, and getting the same world back.**
//!
//! Phase 1, slice 3. Hand-rolled: no `serde`, in keeping with the
//! project's minimal-dependency rule, and because the format is a thing
//! worth being deliberate about rather than derived.
//!
//! # A snapshot and a journal are different things
//!
//! - The **snapshot** is state that cannot be derived: what a person's
//!   life has done to them, where their strain stands, what they have
//!   come to reach for.
//! - The **journal** is the append-only record of things that *happened*
//!   — and its whole purpose is that they are never worked out a second
//!   time.
//!
//! That distinction is what closes the contract slice 9 named and could
//! not implement. An event resolved while somebody was unloaded is a
//! **world fact**: recomputing it after a balance change, an RNG change
//! or a new model version could alter something that has already
//! happened, and a witness who was there would remember it differently.
//! So:
//!
//! | state | source | persisted |
//! |---|---|---|
//! | objective outcome, unresolved | world seed + a stable event id | no |
//! | objective outcome, **resolved** | nothing — it is history | **yes** |
//! | perceptual *noise* | person + **exposure** + a named draw | no |
//! | a perceived event, in flight | historical exposure and the person as they were | only while it is being consumed |
//! | **an encoded memory** | the immutable half of a `memory::Trace` | **yes** |
//! | what somebody makes of it now | reconstructed from the trace and the mind of today | yes, where durable |
//!
//! **Never from a person's own seed**, which is the trap: two witnesses
//! would generate two incompatible versions of one accident.
//!
//! **And a perception is not derivable from two identifiers.** The table
//! said so and it was wrong. Two integers can produce *noise*; they
//! cannot produce a perception, which depends on whether somebody was
//! there at all, how far off, what stood between, what they were
//! attending to, how tired or frightened they were, how they came to
//! hear of it, and what they already believed. Every one of those is
//! **historical**, so recomputing later either uses today's state or
//! drops them — and either way rewrites what somebody originally saw.
//! The durable part therefore lives in that person's own record, in the
//! immutable half of a trace, and is not re-derived at all.
//!
//! Exposure is addressed separately from the event because **hearing
//! about an accident tomorrow is not witnessing it today**.
//!
//! # Two rules the format follows
//!
//! - **A variant's position is not its encoding.** Every enum written
//!   here has an explicit code or an explicit name, so inserting a
//!   variant tomorrow cannot silently reinterpret every save made today.
//! - **Floats are stored as bits.** Exact round-trip, and byte-identical
//!   output for identical state, which is what makes a save comparable.

use std::collections::BTreeMap;

use crate::coping::{
    Acute, ControlAppraisal, Coping, CrisisEpisode, FunctionalState, ImpairmentHistory, Strain,
};
use crate::growth::{
    Cause, DurableTarget, Episodic, Growth, Persistence, Role, ShapesPersonality, ShapesWellbeing,
};
use crate::mind::Facet;
use crate::scaling::{Coarse, Habits, PersonOrigin, Standing};

pub const MAGIC: &[u8; 8] = b"SCALESIM";
/// **Three versions, because three different things can change.**
///
/// The byte layout, the generator that draws a world, and the rules the
/// simulation runs by are independent: a rebalance changes none of the
/// bytes, and a new facet changes no rule. One number for all three
/// either rejects saves it could read or accepts saves it cannot.
///
/// Bumped whenever the byte layout changes in a way an older reader
/// could misread.
pub const FORMAT: u32 = 1;

/// The simulation's own rules — constants, calibrations, thresholds.
/// Recorded so a save can say what it was played under; **not**
/// enforced, because a resolved outcome is history and does not care
/// what the rules are now.
pub const RULES: u32 = 1;

/// Nothing sane needs more than this many of anything in one list, and
/// a corrupt length must not be allowed to ask for a terabyte before it
/// fails.
pub const MAX_COUNT: usize = 8_000_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaveError {
    Truncated,
    BadMagic,
    UnknownFormat(u32),
    /// A code this build has no meaning for — a save from a newer
    /// version, or a corrupted one. Named, so the message says which
    /// table failed rather than "invalid input".
    UnknownCode(&'static str, u32),
    Checksum,
    /// A length no sane file contains, caught before anything is
    /// allocated for it.
    AbsurdLength(usize),
    /// A float that cannot mean anything: a NaN or an infinity where a
    /// quantity belongs.
    NotANumber,
    /// The same event recorded twice, or twice differently. A file
    /// claiming one thing happened two ways is broken, not newer.
    Conflict(u64),
    /// Bytes after the end of what the format says is there.
    TrailingBytes(usize),
}

// ---------------------------------------------------------------------
// the codec
// ---------------------------------------------------------------------

#[derive(Default)]
pub struct Writer {
    pub bytes: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Writer::default()
    }
    pub fn u8(&mut self, v: u8) {
        self.bytes.push(v);
    }
    pub fn bool(&mut self, v: bool) {
        self.u8(v as u8);
    }
    pub fn u16(&mut self, v: u16) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i64(&mut self, v: i64) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }
    /// **Bits, not a decimal rendering.** Exact, and identical state
    /// gives identical bytes.
    pub fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
    pub fn f64(&mut self, v: f64) {
        self.u64(v.to_bits());
    }
    pub fn str(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.bytes.extend_from_slice(s.as_bytes());
    }
    pub fn len(&mut self, n: usize) {
        self.u32(n as u32);
    }
}

pub struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Reader { bytes, at: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], SaveError> {
        if self.at + n > self.bytes.len() {
            return Err(SaveError::Truncated);
        }
        let s = &self.bytes[self.at..self.at + n];
        self.at += n;
        Ok(s)
    }
    pub fn u8(&mut self) -> Result<u8, SaveError> {
        Ok(self.take(1)?[0])
    }
    pub fn bool(&mut self) -> Result<bool, SaveError> {
        Ok(self.u8()? != 0)
    }
    pub fn u16(&mut self) -> Result<u16, SaveError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> Result<u32, SaveError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn u64(&mut self) -> Result<u64, SaveError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn i64(&mut self) -> Result<i64, SaveError> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn f32(&mut self) -> Result<f32, SaveError> {
        Ok(f32::from_bits(self.u32()?))
    }
    pub fn f64(&mut self) -> Result<f64, SaveError> {
        Ok(f64::from_bits(self.u64()?))
    }
    pub fn str(&mut self) -> Result<String, SaveError> {
        let n = self.u32()? as usize;
        let s = self.take(n)?;
        String::from_utf8(s.to_vec()).map_err(|_| SaveError::Truncated)
    }
    pub fn len(&mut self) -> Result<usize, SaveError> {
        Ok(self.u32()? as usize)
    }

    /// **A length that is going to be allocated against.**
    ///
    /// Checked twice: against a sane maximum, and against the bytes
    /// actually left — a count of four billion in a two-hundred-byte
    /// file is a corrupt file and must say so rather than trying.
    pub fn count(&mut self) -> Result<usize, SaveError> {
        let n = self.u32()? as usize;
        if n > MAX_COUNT || n > self.bytes.len().saturating_sub(self.at).saturating_add(1) {
            return Err(SaveError::AbsurdLength(n));
        }
        Ok(n)
    }

    /// A float that has to be a quantity. **Rejects NaN and infinities**,
    /// which are how a corrupt file turns into a world that behaves
    /// strangely rather than one that fails to load.
    pub fn finite_f64(&mut self) -> Result<f64, SaveError> {
        let v = self.f64()?;
        if v.is_finite() {
            Ok(v)
        } else {
            Err(SaveError::NotANumber)
        }
    }

    pub fn finite_f32(&mut self) -> Result<f32, SaveError> {
        let v = self.f32()?;
        if v.is_finite() {
            Ok(v)
        } else {
            Err(SaveError::NotANumber)
        }
    }

    pub fn left(&self) -> usize {
        self.bytes.len().saturating_sub(self.at)
    }
    pub fn done(&self) -> bool {
        self.at >= self.bytes.len()
    }
}

/// Something that can be written down and read back exactly.
pub trait Store: Sized {
    fn store(&self, w: &mut Writer);
    fn load(r: &mut Reader) -> Result<Self, SaveError>;
}

// ---------------------------------------------------------------------
// stable codes
// ---------------------------------------------------------------------

macro_rules! coded {
    ($t:ty, $name:literal, $($v:path => $c:literal),+ $(,)?) => {
        impl Store for $t {
            fn store(&self, w: &mut Writer) {
                w.u16(match self { $($v => $c),+ });
            }
            fn load(r: &mut Reader) -> Result<Self, SaveError> {
                let c = r.u16()?;
                Ok(match c {
                    $($c => $v,)+
                    other => return Err(SaveError::UnknownCode($name, other as u32)),
                })
            }
        }
    };
}

coded!(FunctionalState, "FunctionalState",
    FunctionalState::Regulated => 1,
    FunctionalState::Strained => 2,
    FunctionalState::Depleted => 3,
    FunctionalState::Impaired => 4,
);

coded!(Acute, "Acute",
    Acute::Panic => 1,
    Acute::Dissociation => 2,
    Acute::Flight => 3,
    Acute::Aggression => 4,
    Acute::Freeze => 5,
);

coded!(ShapesPersonality, "ShapesPersonality",
    ShapesPersonality::RoleDemand => 1,
    ShapesPersonality::CoreMemory => 2,
    ShapesPersonality::SustainedTreatment => 3,
    ShapesPersonality::Trauma => 4,
);

coded!(ShapesWellbeing, "ShapesWellbeing",
    ShapesWellbeing::Bereavement => 1,
    ShapesWellbeing::LostWork => 2,
    ShapesWellbeing::Impairment => 3,
    ShapesWellbeing::Attachment => 4,
);

coded!(Coping, "Coping",
    Coping::Active => 1,
    Coping::Planning => 2,
    Coping::InstrumentalSupport => 3,
    Coping::EmotionalSupport => 4,
    Coping::Reframing => 5,
    Coping::Acceptance => 6,
    Coping::Faith => 7,
    Coping::Humour => 8,
    Coping::Distraction => 9,
    Coping::Denial => 10,
    Coping::SubstanceUse => 11,
    Coping::Venting => 12,
    Coping::Disengagement => 13,
    Coping::SelfBlame => 14,
);

/// **A facet is written by name.**
///
/// Twenty-five explicit codes would work and a name is better here: it
/// is self-describing in a hex dump, and it survives a facet being
/// *inserted* rather than appended, which a position never does.
/// Renaming a variant breaks it, and renaming a variant is a deliberate
/// act that should break saves.
impl Store for Facet {
    fn store(&self, w: &mut Writer) {
        w.str(&format!("{self:?}"));
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let name = r.str()?;
        Facet::ALL
            .into_iter()
            .find(|f| format!("{f:?}") == name)
            .ok_or(SaveError::UnknownCode("Facet", 0))
    }
}

impl Store for DurableTarget {
    fn store(&self, w: &mut Writer) {
        match self {
            DurableTarget::Facet(f) => {
                w.u16(1);
                f.store(w);
            }
            DurableTarget::WellbeingBaseline => w.u16(2),
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        match r.u16()? {
            1 => Ok(DurableTarget::Facet(Facet::load(r)?)),
            2 => Ok(DurableTarget::WellbeingBaseline),
            other => Err(SaveError::UnknownCode("DurableTarget", other as u32)),
        }
    }
}

impl Store for Cause {
    fn store(&self, w: &mut Writer) {
        match self {
            Cause::Personality(m) => {
                w.u16(1);
                m.store(w);
            }
            Cause::Wellbeing(m) => {
                w.u16(2);
                m.store(w);
            }
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        match r.u16()? {
            1 => Ok(Cause::Personality(ShapesPersonality::load(r)?)),
            2 => Ok(Cause::Wellbeing(ShapesWellbeing::load(r)?)),
            other => Err(SaveError::UnknownCode("Cause", other as u32)),
        }
    }
}

// ---------------------------------------------------------------------
// the records themselves
// ---------------------------------------------------------------------

impl Store for Persistence {
    fn store(&self, w: &mut Writer) {
        w.f32(self.residual_fraction);
        w.f32(self.calibration_horizon_days);
        w.f32(self.recovery_half_life_days);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(Persistence {
            residual_fraction: r.f32()?,
            calibration_horizon_days: r.f32()?,
            recovery_half_life_days: r.f32()?,
        })
    }
}

impl Store for Episodic {
    fn store(&self, w: &mut Writer) {
        self.target.store(w);
        self.cause.store(w);
        self.persistence.store(w);
        w.f32(self.initial);
        w.u64(self.day);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(Episodic {
            target: DurableTarget::load(r)?,
            cause: Cause::load(r)?,
            persistence: Persistence::load(r)?,
            initial: r.f32()?,
            day: r.u64()?,
        })
    }
}

impl Store for Role {
    fn store(&self, w: &mut Writer) {
        self.facet.store(w);
        w.f32(self.target);
        w.u64(self.started);
        match self.ended {
            None => w.bool(false),
            Some(d) => {
                w.bool(true);
                w.u64(d);
            }
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(Role {
            facet: Facet::load(r)?,
            target: r.f32()?,
            started: r.u64()?,
            ended: if r.bool()? { Some(r.u64()?) } else { None },
        })
    }
}

impl Store for Growth {
    fn store(&self, w: &mut Writer) {
        w.len(self.episodics.len());
        for e in &self.episodics {
            e.store(w);
        }
        w.len(self.roles.len());
        for x in &self.roles {
            x.store(w);
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let mut g = Growth::new();
        let n = r.count()?;
        for _ in 0..n {
            g.episodics.push(Episodic::load(r)?);
        }
        let n = r.count()?;
        for _ in 0..n {
            g.roles.push(Role::load(r)?);
        }
        Ok(g)
    }
}

impl Store for ImpairmentHistory {
    fn store(&self, w: &mut Writer) {
        w.u32(self.current_episode_days);
        w.u32(self.lifetime_days);
        w.u32(self.episode_count);
        match self.days_since_last {
            None => w.bool(false),
            Some(d) => {
                w.bool(true);
                w.u32(d);
            }
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(ImpairmentHistory {
            current_episode_days: r.u32()?,
            lifetime_days: r.u32()?,
            episode_count: r.u32()?,
            days_since_last: if r.bool()? { Some(r.u32()?) } else { None },
        })
    }
}

impl Store for CrisisEpisode {
    fn store(&self, w: &mut Writer) {
        self.kind.store(w);
        w.f64(self.activation);
        w.u64(self.started);
        w.u64(self.because_of);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(CrisisEpisode {
            kind: Acute::load(r)?,
            activation: r.f64()?,
            started: r.u64()?,
            because_of: r.u64()?,
        })
    }
}

impl Store for Strain {
    fn store(&self, w: &mut Writer) {
        w.f64(self.debt);
        self.state.store(w);
        w.u32(self.days_in_state);
        self.history.store(w);
        match &self.crisis {
            None => w.bool(false),
            Some(c) => {
                w.bool(true);
                c.store(w);
            }
        }
        // The appraisal ring, so relapse sensitivity is not re-applied to
        // a trouble somebody is already living with.
        let seen: Vec<(u64, f64)> = self.appraised.iter().flatten().copied().collect();
        w.len(seen.len());
        for (e, v) in seen {
            w.u64(e);
            w.f64(v);
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let mut s = Strain::default();
        s.debt = r.f64()?;
        s.state = FunctionalState::load(r)?;
        s.days_in_state = r.u32()?;
        s.history = ImpairmentHistory::load(r)?;
        s.crisis = if r.bool()? { Some(CrisisEpisode::load(r)?) } else { None };
        let n = r.count()?;
        for _ in 0..n {
            let (e, v) = (r.u64()?, r.f64()?);
            s.remember_appraisal(e, v);
        }
        Ok(s)
    }
}

impl Store for ControlAppraisal {
    fn store(&self, w: &mut Writer) {
        w.f64(self.source);
        w.f64(self.consequences);
        w.f64(self.own_response);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(ControlAppraisal {
            source: r.f64()?,
            consequences: r.f64()?,
            own_response: r.f64()?,
        })
    }
}

impl Store for Habits {
    /// **Written against the strategy's code, not its position.** The
    /// weights live in an array, and an array index is exactly the kind
    /// of thing that shifts when somebody adds a strategy.
    fn store(&self, w: &mut Writer) {
        w.len(Coping::ALL.len());
        for c in Coping::ALL {
            c.store(w);
            w.f32(self.of(c));
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let mut h = Habits::default();
        let n = r.count()?;
        for _ in 0..n {
            let c = Coping::load(r)?;
            h.set(c, r.f32()?);
        }
        Ok(h)
    }
}

impl Store for Standing {
    fn store(&self, w: &mut Writer) {
        w.f64(self.severity);
        w.u64(self.since);
        w.f64(self.worsens_if_ignored);
        w.f64(self.actual.source);
        w.f64(self.actual.consequences);
        w.f64(self.actual.exit);
        w.f64(self.actual.means);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(Standing {
            severity: r.f64()?,
            since: r.u64()?,
            worsens_if_ignored: r.f64()?,
            actual: crate::coping::ActualControl {
                source: r.f64()?,
                consequences: r.f64()?,
                exit: r.f64()?,
                means: r.f64()?,
            },
        })
    }
}

impl Store for PersonOrigin {
    fn store(&self, w: &mut Writer) {
        w.u64(self.seed);
        w.u32(self.schema);
        w.u64(self.born);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(PersonOrigin { seed: r.u64()?, schema: r.u32()?, born: r.u64()? })
    }
}

impl Store for Coarse {
    fn store(&self, w: &mut Writer) {
        w.u64(self.who.bits());
        self.origin.store(w);
        w.len(self.baseline.len());
        for &b in &self.baseline {
            w.f32(b);
        }
        self.growth.store(w);
        self.strain.store(w);
        self.perceived_control.store(w);
        self.habits.store(w);
        w.len(self.standing.len());
        for s in &self.standing {
            s.store(w);
        }
        w.u16(self.attempts_outstanding);
        w.f64(self.support_expected);
        w.u64(self.last_update);
        w.len(self.active.len());
        for a in &self.active {
            w.u64(a.event);
            w.u32(a.revision);
            w.u64(a.opened_at);
            w.u64(a.last_material_change);
            w.f64(a.felt);
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let who = crate::id::Id::from_bits(r.u64()?);
        let origin = PersonOrigin::load(r)?;
        let mut c = Coarse::new(who, origin.seed, 0);
        c.origin = origin;
        let n = r.count()?;
        c.baseline = (0..n).map(|_| r.f32()).collect::<Result<_, _>>()?;
        c.growth = Growth::load(r)?;
        c.strain = Strain::load(r)?;
        c.perceived_control = ControlAppraisal::load(r)?;
        c.habits = Habits::load(r)?;
        let n = r.count()?;
        c.standing = (0..n).map(|_| Standing::load(r)).collect::<Result<_, _>>()?;
        c.attempts_outstanding = r.u16()?;
        c.support_expected = r.finite_f64()?;
        c.last_update = r.u64()?;
        let n = r.count()?;
        for _ in 0..n {
            c.active.push(crate::scaling::ActiveAppraisal {
                event: r.u64()?,
                revision: r.u32()?,
                opened_at: r.u64()?,
                last_material_change: r.u64()?,
                felt: r.finite_f64()?,
            });
        }
        Ok(c)
    }
}

// ---------------------------------------------------------------------
// the four stores, which are four different jobs
// ---------------------------------------------------------------------

/// An event in the world.
pub type EventId = u64;

/// **One person's exposure to one event**, which is not the event.
///
/// Hearing about an accident tomorrow is a different exposure from
/// watching it today, and the two must be addressable apart or a
/// perception cannot be tied to how it was come by.
pub type ExposureId = u64;

/// A mixing function of the project's own, so nothing here needs `rand`.
fn mix(a: u64, b: u64) -> u64 {
    let mut h = a ^ b.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^ (h >> 31)
}

/// **A named draw, never a sequential stream.**
///
/// `(world seed, event, "injury severity")` and
/// `(world seed, event, "which way the cart went")` are independent, so
/// **adding a draw to a resolver tomorrow cannot shift every later
/// outcome**. A shared cursor through one stream would, which is the
/// commonest way a saved world quietly stops matching itself.
pub fn channel(seed: u64, event: u64, name: &str) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in name.as_bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    mix(mix(seed, event), h)
}


// =====================================================================
// work in progress
// =====================================================================
//
// **"Pure data mutation" proves repeatability given the same data.** It
// says nothing about whether a hand-written codec preserved all of it,
// and an enum variant nothing happens to write is exactly the one that
// silently does not come back. Every variant of every type below is
// round-tripped by a table-driven gate.

impl Store for crate::material::Material {
    fn store(&self, w: &mut Writer) {
        w.str(self.name());
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let n = r.str()?;
        crate::material::Material::from_name(&n)
            .ok_or(SaveError::UnknownCode("Material", n.len() as u32))
    }
}

impl Store for crate::material::Dims {
    fn store(&self, w: &mut Writer) {
        w.f64(self.length_m);
        w.f64(self.width_m);
        w.f64(self.height_m);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(crate::material::Dims {
            length_m: r.f64()?,
            width_m: r.f64()?,
            height_m: r.f64()?,
        })
    }
}

impl Store for crate::item::DefId {
    fn store(&self, w: &mut Writer) {
        w.u32(self.0);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(crate::item::DefId(r.u32()?))
    }
}

impl Store for crate::id::Id<crate::item::ItemInstance> {
    fn store(&self, w: &mut Writer) {
        w.u64(self.bits());
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(crate::id::Id::from_bits(r.u64()?))
    }
}

coded!(crate::bom::MaterialState, "MaterialState",
    crate::bom::MaterialState::AsRolled => 1,
    crate::bom::MaterialState::Annealed => 2,
    crate::bom::MaterialState::Normalised => 3,
    crate::bom::MaterialState::WorkHardened => 4,
    crate::bom::MaterialState::QuenchedAndTempered => 5,
    crate::bom::MaterialState::Cured => 6,
);

coded!(crate::bom::Surface, "Surface",
    crate::bom::Surface::Bare => 1,
    crate::bom::Surface::Galvanised => 2,
    crate::bom::Surface::Primed => 3,
    crate::bom::Surface::Painted => 4,
    crate::bom::Surface::Anodised => 5,
    crate::bom::Surface::Plated => 6,
);

impl Store for crate::bom::Geometry {
    fn store(&self, w: &mut Writer) {
        use crate::bom::Geometry::*;
        match self {
            Sheet { mm } => {
                w.u16(1);
                w.f64(*mm);
            }
            Bar { mm } => {
                w.u16(2);
                w.f64(*mm);
            }
            Stamping => w.u16(3),
            Shell => w.u16(4),
            Casting => w.u16(5),
            Extrusion => w.u16(6),
            Machined => w.u16(7),
            Woven => w.u16(8),
            Wound => w.u16(9),
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        use crate::bom::Geometry::*;
        Ok(match r.u16()? {
            1 => Sheet { mm: r.f64()? },
            2 => Bar { mm: r.f64()? },
            3 => Stamping,
            4 => Shell,
            5 => Casting,
            6 => Extrusion,
            7 => Machined,
            8 => Woven,
            9 => Wound,
            other => return Err(SaveError::UnknownCode("Geometry", other as u32)),
        })
    }
}

impl Store for crate::wip::Feature {
    fn store(&self, w: &mut Writer) {
        use crate::wip::Feature::*;
        match self {
            Hole { count, mm } => {
                w.u16(1);
                w.u32(*count);
                w.f64(*mm);
            }
            Cut { length_mm } => {
                w.u16(2);
                w.f64(*length_mm);
            }
            Bend { degrees } => {
                w.u16(3);
                w.f64(*degrees);
            }
            Weld { segments } => {
                w.u16(4);
                w.u32(*segments);
            }
            Coating { layer, microns } => {
                w.u16(5);
                layer.store(w);
                w.f64(*microns);
            }
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        use crate::wip::Feature::*;
        Ok(match r.u16()? {
            1 => Hole { count: r.u32()?, mm: r.f64()? },
            2 => Cut { length_mm: r.f64()? },
            3 => Bend { degrees: r.f64()? },
            4 => Weld { segments: r.u32()? },
            5 => Coating {
                layer: crate::bom::Surface::load(r)?,
                microns: r.f64()?,
            },
            other => return Err(SaveError::UnknownCode("Feature", other as u32)),
        })
    }
}

impl Store for crate::wip::Progress {
    fn store(&self, w: &mut Writer) {
        use crate::wip::Progress::*;
        match self {
            Cut { done_mm, total_mm } => {
                w.u16(1);
                w.f64(*done_mm);
                w.f64(*total_mm);
            }
            Heat { celsius, target_c, ambient_c } => {
                w.u16(2);
                w.f64(*celsius);
                w.f64(*target_c);
                w.f64(*ambient_c);
            }
            Dry { moisture, target } => {
                w.u16(3);
                w.f64(*moisture);
                w.f64(*target);
            }
            Cure { reacted, at_c, wants_c } => {
                w.u16(4);
                w.f64(*reacted);
                w.f64(*at_c);
                w.f64(*wants_c);
            }
            Weld { done, segments } => {
                w.u16(5);
                w.u32(*done);
                w.u32(*segments);
            }
            Coat { microns, target_microns, layers } => {
                w.u16(6);
                w.f64(*microns);
                w.f64(*target_microns);
                w.u32(*layers);
            }
            Assemble { joints_done, joints } => {
                w.u16(7);
                w.u32(*joints_done);
                w.u32(*joints);
            }
            Machine { features_done, features, allowance_mm } => {
                w.u16(8);
                w.u32(*features_done);
                w.u32(*features);
                w.f64(*allowance_mm);
            }
            Elapsed { minutes, total } => {
                w.u16(9);
                w.f64(*minutes);
                w.f64(*total);
            }
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        use crate::wip::Progress::*;
        Ok(match r.u16()? {
            1 => Cut { done_mm: r.f64()?, total_mm: r.f64()? },
            2 => Heat { celsius: r.f64()?, target_c: r.f64()?, ambient_c: r.f64()? },
            3 => Dry { moisture: r.f64()?, target: r.f64()? },
            4 => Cure { reacted: r.f64()?, at_c: r.f64()?, wants_c: r.f64()? },
            5 => Weld { done: r.u32()?, segments: r.u32()? },
            6 => Coat { microns: r.f64()?, target_microns: r.f64()?, layers: r.u32()? },
            7 => Assemble { joints_done: r.u32()?, joints: r.u32()? },
            8 => Machine {
                features_done: r.u32()?,
                features: r.u32()?,
                allowance_mm: r.f64()?,
            },
            9 => Elapsed { minutes: r.f64()?, total: r.f64()? },
            other => return Err(SaveError::UnknownCode("Progress", other as u32)),
        })
    }
}

impl Store for crate::item::Host {
    fn store(&self, w: &mut Writer) {
        use crate::item::Host::*;
        match self {
            Item(id) => {
                w.u16(1);
                id.store(w);
            }
            Vehicle(n) => {
                w.u16(2);
                w.u32(*n);
            }
            Building(n) => {
                w.u16(3);
                w.u32(*n);
            }
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        use crate::item::Host::*;
        Ok(match r.u16()? {
            1 => Item(crate::id::Id::load(r)?),
            2 => Vehicle(r.u32()?),
            3 => Building(r.u32()?),
            other => return Err(SaveError::UnknownCode("Host", other as u32)),
        })
    }
}

impl Store for crate::item::Placement {
    fn store(&self, w: &mut Writer) {
        use crate::item::Placement::*;
        match self {
            Ground { locality, x, y } => {
                w.u16(1);
                w.u32(*locality);
                w.i64(*x as i64);
                w.i64(*y as i64);
            }
            Carried { person } => {
                w.u16(2);
                w.u64(*person);
            }
            Contained { container } => {
                w.u16(3);
                container.store(w);
            }
            Installed { host, mount } => {
                w.u16(4);
                host.store(w);
                w.u32(*mount as u32);
            }
            Fixtured { resource, slot, clamped } => {
                w.u16(5);
                w.u32(*resource);
                w.u32(*slot);
                w.bool(*clamped);
            }
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        use crate::item::Placement::*;
        Ok(match r.u16()? {
            1 => Ground { locality: r.u32()?, x: r.i64()? as i32, y: r.i64()? as i32 },
            2 => Carried { person: r.u64()? },
            3 => Contained { container: crate::id::Id::load(r)? },
            4 => Installed {
                host: crate::item::Host::load(r)?,
                mount: r.u32()? as usize,
            },
            5 => Fixtured { resource: r.u32()?, slot: r.u32()?, clamped: r.bool()? },
            other => return Err(SaveError::UnknownCode("Placement", other as u32)),
        })
    }
}

impl Store for crate::item::WorkStatus {
    fn store(&self, w: &mut Writer) {
        use crate::item::WorkStatus::*;
        match self {
            Available => w.u16(1),
            Reserved { order } => {
                w.u16(2);
                w.u64(*order);
            }
            Wip { order, operation } => {
                w.u16(3);
                w.u64(*order);
                w.u32(*operation as u32);
            }
            AwaitingUnload { order } => {
                w.u16(4);
                w.u64(*order);
            }
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        use crate::item::WorkStatus::*;
        Ok(match r.u16()? {
            1 => Available,
            2 => Reserved { order: r.u64()? },
            3 => Wip { order: r.u64()?, operation: r.u32()? as usize },
            4 => AwaitingUnload { order: r.u64()? },
            other => return Err(SaveError::UnknownCode("WorkStatus", other as u32)),
        })
    }
}

impl Store for crate::wip::Shape {
    fn store(&self, w: &mut Writer) {
        self.becoming.store(w);
        w.str(self.stage);
        self.geometry.store(w);
        self.state.store(w);
        self.surface.store(w);
        self.dims.store(w);
        w.len(self.features.len());
        for f in &self.features {
            f.store(w);
        }
        w.len(self.lineage.len());
        for l in &self.lineage {
            l.store(w);
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let becoming = crate::item::DefId::load(r)?;
        // **A stage name is authored text**, so it is interned rather than
        // held as an owned string on every workpiece in the world.
        let stage = crate::wip::intern_stage(&r.str()?);
        let geometry = crate::bom::Geometry::load(r)?;
        let state = crate::bom::MaterialState::load(r)?;
        let surface = crate::bom::Surface::load(r)?;
        let dims = crate::material::Dims::load(r)?;
        let n = r.count()?;
        let mut features = Vec::with_capacity(n);
        for _ in 0..n {
            features.push(crate::wip::Feature::load(r)?);
        }
        let n = r.count()?;
        let mut lineage = Vec::with_capacity(n);
        for _ in 0..n {
            lineage.push(crate::id::Id::load(r)?);
        }
        Ok(crate::wip::Shape {
            becoming,
            stage,
            geometry,
            state,
            surface,
            dims,
            features,
            lineage,
        })
    }
}

impl Store for crate::wip::Heat {
    fn store(&self, w: &mut Writer) {
        w.len(self.merged_from.len());
        for i in &self.merged_from {
            i.store(w);
        }
        w.len(self.composition.parts().len());
        for &(m, f) in self.composition.parts() {
            m.store(w);
            w.f64(f);
        }
        w.len(self.contamination.len());
        for &(m, kg) in &self.contamination {
            m.store(w);
            w.f64(kg);
        }
        w.bool(self.hazardous);
        w.f64(self.recycled_fraction);
        w.str(self.process);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let n = r.count()?;
        let mut merged_from = Vec::with_capacity(n);
        for _ in 0..n {
            merged_from.push(crate::id::Id::load(r)?);
        }
        let n = r.count()?;
        let mut parts = Vec::with_capacity(n);
        for _ in 0..n {
            parts.push((crate::material::Material::load(r)?, r.f64()?));
        }
        let n = r.count()?;
        let mut contamination = Vec::with_capacity(n);
        for _ in 0..n {
            contamination.push((crate::material::Material::load(r)?, r.f64()?));
        }
        Ok(crate::wip::Heat {
            merged_from,
            composition: crate::material::Composition::of(&parts),
            contamination,
            hazardous: r.bool()?,
            recycled_fraction: r.f64()?,
            process: crate::wip::intern_stage(&r.str()?),
        })
    }
}

/// **Where a journal entry sits in the order of things.**
///
/// Canonical bytes are not causal order. A map keyed by event id writes
/// the same file every time and says nothing about what happened before
/// what, which is exactly what a replay needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct JournalKey {
    pub time: u64,
    /// Which pass of the day — so two things at the same instant in
    /// different phases have a defined order without inventing one.
    pub phase: u16,
    /// Breaks ties between dependent events at one instant. Independent
    /// ones may be given the same sequence and must then commute.
    pub sequence: u64,
    pub event: EventId,
}

/// What was decided, and whether its effects have been put into the
/// world yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: JournalKey,
    pub outcome: u64,
    /// **Committed is not applied.** A crash between the two must replay;
    /// a crash after it must not apply twice.
    pub applied: bool,
}

/// **Something scheduled that has not happened yet.**
///
/// Carries the inputs its resolver will need, because those must be the
/// inputs as they were when it was scheduled — not as they are when it
/// finally fires.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    pub event: EventId,
    pub scheduled_at: u64,
    pub resolver: u32,
    pub resolver_version: u32,
    /// Named, for the same reason draws are named.
    pub inputs: Vec<(String, u64)>,
}

/// **What happened, in order, and what is still to happen.**
///
/// Four jobs kept apart, because collapsing them is how a journal
/// becomes the unbounded state this project has already had to fix twice
/// elsewhere:
///
/// | store | what for | how long it is kept |
/// |---|---|---|
/// | checkpoint | authoritative state at a known sequence | until the next one |
/// | this journal | committed changes since that checkpoint | folded into the next checkpoint |
/// | archive | events later systems may still refer to | selectively |
/// | pending | scheduled and unresolved | until resolved or cancelled |
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Journal {
    entries: BTreeMap<JournalKey, Entry>,
    by_event: BTreeMap<EventId, JournalKey>,
    pending: Vec<Pending>,
    next_sequence: u64,
}

/// Where a checkpoint stands, so a journal can be replayed onto exactly
/// the state it was written against.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Checkpoint {
    pub world_id: u64,
    pub checkpoint_id: u64,
    pub last_applied_sequence: u64,
}

impl Journal {
    pub fn new() -> Self {
        Journal::default()
    }

    /// **Resolve once, and commit it in the same breath.**
    ///
    /// A deterministic draw stops a crash producing a *different* answer;
    /// it does nothing about the same answer being *applied* twice. So
    /// resolving records the outcome as unapplied, and `apply_once` is
    /// the only thing that marks it done — which is what makes replay
    /// exactly-once rather than at-least-once.
    pub fn resolve(&mut self, world_seed: u64, key: JournalKey, draw: &str) -> u64 {
        if let Some(k) = self.by_event.get(&key.event) {
            return self.entries[k].outcome;
        }
        let outcome = channel(world_seed, key.event, draw);
        self.commit(Entry { key, outcome, applied: false })
            .expect("a fresh event cannot conflict with itself");
        outcome
    }

    /// **Duplicate rules, stated rather than assumed.**
    ///
    /// The same event with the same contents is an idempotent no-op — a
    /// replay must be able to re-offer what it already has. The same
    /// event with *different* contents is a conflict, not a later
    /// version: it means two runs disagreed about history.
    pub fn commit(&mut self, e: Entry) -> Result<(), SaveError> {
        if let Some(k) = self.by_event.get(&e.event()) {
            let existing = self.entries[k];
            if existing.outcome == e.outcome {
                return Ok(());
            }
            return Err(SaveError::Conflict(e.event()));
        }
        self.next_sequence = self.next_sequence.max(e.key.sequence + 1);
        self.by_event.insert(e.event(), e.key);
        self.entries.insert(e.key, e);
        Ok(())
    }

    /// **Put the effects in, and only the first time.** Returns the
    /// outcome on the pass that should actually apply it, and nothing on
    /// every pass after — which is the whole of exactly-once.
    pub fn apply_once(&mut self, event: EventId) -> Option<u64> {
        let k = *self.by_event.get(&event)?;
        let e = self.entries.get_mut(&k)?;
        if e.applied {
            return None;
        }
        e.applied = true;
        Some(e.outcome)
    }

    /// What a replay after a crash still has to do, in causal order.
    pub fn unapplied(&self) -> Vec<Entry> {
        self.entries.values().filter(|e| !e.applied).copied().collect()
    }

    /// Everything since a checkpoint, in causal order.
    pub fn since(&self, c: &Checkpoint) -> Vec<Entry> {
        self.entries
            .values()
            .filter(|e| e.key.sequence > c.last_applied_sequence)
            .copied()
            .collect()
    }

    /// **Fold into a checkpoint.** What has been applied is now part of
    /// the state and stops being a change to it — which is what keeps a
    /// journal from growing with the length of a game rather than with
    /// its history.
    pub fn fold_into(&mut self, c: &mut Checkpoint) {
        let mut high = c.last_applied_sequence;
        self.entries.retain(|_, e| {
            if e.applied {
                high = high.max(e.key.sequence);
                false
            } else {
                true
            }
        });
        let live: BTreeMap<EventId, JournalKey> =
            self.entries.values().map(|e| (e.key.event, e.key)).collect();
        self.by_event = live;
        c.last_applied_sequence = high;
        c.checkpoint_id += 1;
    }

    pub fn already(&self, event: EventId) -> Option<u64> {
        self.by_event.get(&event).map(|k| self.entries[k].outcome)
    }
    pub fn entry(&self, event: EventId) -> Option<Entry> {
        self.by_event.get(&event).map(|k| self.entries[k])
    }
    pub fn in_order(&self) -> Vec<Entry> {
        self.entries.values().copied().collect()
    }
    pub fn next_sequence(&mut self) -> u64 {
        let n = self.next_sequence;
        self.next_sequence += 1;
        n
    }
    pub fn schedule(&mut self, p: Pending) {
        self.pending.push(p);
        self.pending.sort_by(|a, b| {
            a.scheduled_at.cmp(&b.scheduled_at).then(a.event.cmp(&b.event))
        });
    }
    pub fn pending(&self) -> &[Pending] {
        &self.pending
    }
    pub fn take_due(&mut self, day: u64) -> Vec<Pending> {
        let due: Vec<Pending> =
            self.pending.iter().filter(|p| p.scheduled_at <= day).cloned().collect();
        self.pending.retain(|p| p.scheduled_at > day);
        due
    }

    /// **Perceptual noise only**, and the name matters.
    ///
    /// This is *not* a perception. What somebody perceived depends on
    /// whether they were there at all, how far off, what was between
    /// them, what they were attending to, how tired or frightened they
    /// were, and what they already believed — none of which two integers
    /// can carry, and all of which are *historical*. Recomputing from ids
    /// alone would either use today's state or drop those inputs, and
    /// either way rewrites what somebody originally saw.
    ///
    /// The boundary is: objective outcome → historical exposure →
    /// perceived event → appraisal → durable consequence. This supplies
    /// the *jitter* at the third step, keyed by the exposure rather than
    /// the event so that hearing about it tomorrow is a different draw
    /// from watching it today. What is durable is kept in the person's
    /// own record, in the immutable half of a `memory::Trace`.
    pub fn perceptual_noise(person: u64, exposure: ExposureId, draw: &str) -> u64 {
        channel(person.wrapping_mul(0xD6E8_FEB8_6659_FD93), exposure, draw)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Entry {
    pub fn event(&self) -> EventId {
        self.key.event
    }
}

impl Store for JournalKey {
    fn store(&self, w: &mut Writer) {
        w.u64(self.time);
        w.u16(self.phase);
        w.u64(self.sequence);
        w.u64(self.event);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(JournalKey {
            time: r.u64()?,
            phase: r.u16()?,
            sequence: r.u64()?,
            event: r.u64()?,
        })
    }
}

impl Store for Pending {
    fn store(&self, w: &mut Writer) {
        w.u64(self.event);
        w.u64(self.scheduled_at);
        w.u32(self.resolver);
        w.u32(self.resolver_version);
        w.len(self.inputs.len());
        for (k, v) in &self.inputs {
            w.str(k);
            w.u64(*v);
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let event = r.u64()?;
        let scheduled_at = r.u64()?;
        let resolver = r.u32()?;
        let resolver_version = r.u32()?;
        let n = r.count()?;
        let mut inputs = Vec::with_capacity(n);
        for _ in 0..n {
            inputs.push((r.str()?, r.u64()?));
        }
        Ok(Pending { event, scheduled_at, resolver, resolver_version, inputs })
    }
}

impl Store for Journal {
    fn store(&self, w: &mut Writer) {
        // A `BTreeMap` keyed causally, so the same history writes the
        // same bytes *and* replays in the same order.
        w.len(self.entries.len());
        for e in self.entries.values() {
            e.key.store(w);
            w.u64(e.outcome);
            w.bool(e.applied);
        }
        w.len(self.pending.len());
        for p in &self.pending {
            p.store(w);
        }
        w.u64(self.next_sequence);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let mut j = Journal::new();
        let n = r.count()?;
        for _ in 0..n {
            let key = JournalKey::load(r)?;
            let outcome = r.u64()?;
            let applied = r.bool()?;
            // **Duplicate keys are rejected**, not last-one-wins: a file
            // claiming one event happened twice is a broken file.
            if j.by_event.contains_key(&key.event) {
                return Err(SaveError::Conflict(key.event));
            }
            j.by_event.insert(key.event, key);
            j.entries.insert(key, Entry { key, outcome, applied });
        }
        let n = r.count()?;
        for _ in 0..n {
            j.pending.push(Pending::load(r)?);
        }
        j.next_sequence = r.u64()?;
        Ok(j)
    }
}

impl Store for Checkpoint {
    fn store(&self, w: &mut Writer) {
        w.u64(self.world_id);
        w.u64(self.checkpoint_id);
        w.u64(self.last_applied_sequence);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(Checkpoint {
            world_id: r.u64()?,
            checkpoint_id: r.u64()?,
            last_applied_sequence: r.u64()?,
        })
    }
}

// ---------------------------------------------------------------------
// the ground, and the base it was cut against
// ---------------------------------------------------------------------

use crate::patch::{
    BaseChunk, Boundary, ChunkAt, Construction, Field, Fluid, Material, Materialised, ObjectId,
    Overlay, Terrain, TileChange, Vegetation,
};

coded!(Terrain, "Terrain",
    Terrain::Solid => 1,
    Terrain::Floor => 2,
    Terrain::Open => 3,
    Terrain::Ramp => 4,
);

coded!(Material, "Material",
    Material::Soil => 1,
    Material::Sand => 2,
    Material::Sedimentary => 3,
    Material::Igneous => 4,
    Material::Metamorphic => 5,
    Material::Brick => 6,
    Material::Concrete => 7,
    Material::Timber => 8,
    Material::Steel => 9,
    Material::Glass => 10,
    Material::Tarmac => 11,
);

coded!(Construction, "Construction",
    Construction::Wall => 1,
    Construction::Door => 2,
    Construction::Window => 3,
    Construction::Partition => 4,
    Construction::Fitting => 5,
);

coded!(Boundary, "Boundary",
    Boundary::Solid => 1,
    Boundary::Open => 2,
    Boundary::Grate => 3,
    Boundary::Hatch => 4,
);

coded!(Fluid, "Fluid",
    Fluid::Water => 1,
    Fluid::Sewage => 2,
);

coded!(Vegetation, "Vegetation",
    Vegetation::Grass => 1,
    Vegetation::Scrub => 2,
    Vegetation::Crop => 3,
    Vegetation::Tree => 4,
);

impl Store for ObjectId {
    fn store(&self, w: &mut Writer) {
        w.u64(self.0);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(ObjectId(r.u64()?))
    }
}

/// **Three states, and the third is the point.** `Remove` is not
/// `Set(nothing)`: a door the generator put there has to be removable.
impl<T: Store + Copy> Store for Field<T> {
    fn store(&self, w: &mut Writer) {
        match self {
            Field::Unchanged => w.u8(0),
            Field::Set(v) => {
                w.u8(1);
                v.store(w);
            }
            Field::Remove => w.u8(2),
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        match r.u8()? {
            0 => Ok(Field::Unchanged),
            1 => Ok(Field::Set(T::load(r)?)),
            2 => Ok(Field::Remove),
            other => Err(SaveError::UnknownCode("Field", other as u32)),
        }
    }
}

impl Store for TileChange {
    fn store(&self, w: &mut Writer) {
        self.terrain.store(w);
        self.material.store(w);
        self.construction.store(w);
        self.ceiling.store(w);
        self.fluid.store(w);
        self.vegetation.store(w);
        self.object.store(w);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(TileChange {
            terrain: Field::load(r)?,
            material: Field::load(r)?,
            construction: Field::load(r)?,
            ceiling: Field::load(r)?,
            fluid: Field::load(r)?,
            vegetation: Field::load(r)?,
            object: Field::load(r)?,
        })
    }
}

impl Store for ChunkAt {
    fn store(&self, w: &mut Writer) {
        w.i64(self.cx);
        w.i64(self.cy);
        w.i64(self.cz);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(ChunkAt { cx: r.i64()?, cy: r.i64()?, cz: r.i64()? })
    }
}

impl Store for BaseChunk {
    fn store(&self, w: &mut Writer) {
        self.at.store(w);
        w.u32(self.worldgen_version);
        w.u64(self.base_hash);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(BaseChunk {
            at: ChunkAt::load(r)?,
            worldgen_version: r.u32()?,
            base_hash: r.u64()?,
        })
    }
}

impl Store for Overlay {
    fn store(&self, w: &mut Writer) {
        w.len(self.base.len());
        for b in self.base.values() {
            b.store(w);
        }
        w.len(self.tiles.len());
        for (at, c) in &self.tiles {
            w.i64(at.0);
            w.i64(at.1);
            w.i64(at.2);
            c.store(w);
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let mut o = Overlay::new();
        let n = r.count()?;
        for _ in 0..n {
            let b = BaseChunk::load(r)?;
            if o.base.insert(b.at, b).is_some() {
                return Err(SaveError::Conflict(b.at.cx as u64));
            }
        }
        let n = r.count()?;
        for _ in 0..n {
            let at = (r.i64()?, r.i64()?, r.i64()?);
            let c = TileChange::load(r)?;
            if o.tiles.insert(at, c).is_some() {
                return Err(SaveError::Conflict(at.2 as u64));
            }
        }
        Ok(o)
    }
}

impl Store for Materialised {
    fn store(&self, w: &mut Writer) {
        fn opt<T: Store>(w: &mut Writer, v: &Option<T>) {
            match v {
                None => w.u8(0),
                Some(x) => {
                    w.u8(1);
                    x.store(w);
                }
            }
        }
        opt(w, &self.terrain);
        opt(w, &self.material);
        opt(w, &self.construction);
        opt(w, &self.ceiling);
        opt(w, &self.fluid);
        opt(w, &self.vegetation);
        opt(w, &self.object);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        fn opt<T: Store>(r: &mut Reader) -> Result<Option<T>, SaveError> {
            match r.u8()? {
                0 => Ok(None),
                1 => Ok(Some(T::load(r)?)),
                other => Err(SaveError::UnknownCode("Option", other as u32)),
            }
        }
        Ok(Materialised {
            terrain: opt(r)?,
            material: opt(r)?,
            construction: opt(r)?,
            ceiling: opt(r)?,
            fluid: opt(r)?,
            vegetation: opt(r)?,
            object: opt(r)?,
        })
    }
}

// ---------------------------------------------------------------------
// a whole save
// ---------------------------------------------------------------------

/// **A snapshot of what cannot be derived, and a journal of what
/// happened.**
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Save {
    pub world_seed: u64,
    pub day: u64,
    pub people: Vec<Coarse>,
    pub journal: Journal,
    /// Where the journal is to be replayed from.
    pub checkpoint: Checkpoint,
    /// What made this world, and what rules it was played under. Read
    /// and kept; neither is enforced.
    pub schema: u32,
    pub rules: u32,
    /// **What was done to the ground**, and the base each change was cut
    /// against.
    pub overlay: Overlay,
}

/// A cheap checksum over the body, so a truncated or corrupted file says
/// so rather than being read as a plausible world.
fn checksum(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

impl Save {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut body = Writer::new();
        body.u64(self.world_seed);
        body.u64(self.day);
        body.len(self.people.len());
        for p in &self.people {
            p.store(&mut body);
        }
        self.journal.store(&mut body);
        self.checkpoint.store(&mut body);
        self.overlay.store(&mut body);

        let mut out = Writer::new();
        out.bytes.extend_from_slice(MAGIC);
        out.u32(FORMAT);
        out.u32(crate::scaling::GENERATION_SCHEMA);
        out.u32(RULES);
        out.u64(checksum(&body.bytes));
        out.len(body.bytes.len());
        out.bytes.extend_from_slice(&body.bytes);
        out.bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SaveError> {
        let mut r = Reader::new(bytes);
        if r.take(8)? != MAGIC {
            return Err(SaveError::BadMagic);
        }
        let format = r.u32()?;
        if format != FORMAT {
            return Err(SaveError::UnknownFormat(format));
        }
        // **The generator's version is read and kept, not enforced.** A
        // save made by an older generator is still readable precisely
        // because the baseline is written down rather than re-derived.
        // The rules version likewise: a resolved outcome is history and
        // does not care what the rules are today.
        let schema = r.u32()?;
        let rules = r.u32()?;
        let sum = r.u64()?;
        let n = r.count()?;
        let body = r.take(n)?;
        if checksum(body) != sum {
            return Err(SaveError::Checksum);
        }
        // **Nothing after the end.** Trailing bytes mean the file is not
        // what it says it is, and reading the prefix anyway is how a
        // truncated-and-appended file loads as a plausible world.
        if !r.done() {
            return Err(SaveError::TrailingBytes(r.left()));
        }

        let mut b = Reader::new(body);
        let world_seed = b.u64()?;
        let day = b.u64()?;
        let n = b.len()?;
        let people = (0..n).map(|_| Coarse::load(&mut b)).collect::<Result<_, _>>()?;
        let journal = Journal::load(&mut b)?;
        let checkpoint = Checkpoint::load(&mut b)?;
        let overlay = Overlay::load(&mut b)?;
        if !b.done() {
            return Err(SaveError::TrailingBytes(b.left()));
        }
        Ok(Save { world_seed, day, people, journal, checkpoint, schema, rules, overlay })
    }
}
