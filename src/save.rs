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
//! | | derived from | written down |
//! |---|---|---|
//! | unresolved objective outcome | `world_seed` + a stable event id | no |
//! | **resolved** objective outcome | nothing — it is history | **yes** |
//! | one person's perception of it | that person's id + the event id | no |
//!
//! **Never from a person's own seed**, which is the trap: two witnesses
//! would generate two incompatible versions of one accident.
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
/// Bumped whenever the byte layout changes in a way an older reader
/// could misread.
pub const FORMAT: u32 = 1;

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
        let n = r.len()?;
        for _ in 0..n {
            g.episodics.push(Episodic::load(r)?);
        }
        let n = r.len()?;
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
        let n = r.len()?;
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
        let n = r.len()?;
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
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let who = crate::id::Id::from_bits(r.u64()?);
        let origin = PersonOrigin::load(r)?;
        let mut c = Coarse::new(who, origin.seed, 0);
        c.origin = origin;
        let n = r.len()?;
        c.baseline = (0..n).map(|_| r.f32()).collect::<Result<_, _>>()?;
        c.growth = Growth::load(r)?;
        c.strain = Strain::load(r)?;
        c.perceived_control = ControlAppraisal::load(r)?;
        c.habits = Habits::load(r)?;
        let n = r.len()?;
        c.standing = (0..n).map(|_| Standing::load(r)).collect::<Result<_, _>>()?;
        c.attempts_outstanding = r.u16()?;
        c.support_expected = r.f64()?;
        c.last_update = r.u64()?;
        Ok(c)
    }
}

// ---------------------------------------------------------------------
// the journal: what happened, and is never worked out twice
// ---------------------------------------------------------------------

pub type EventId = u64;

/// A mixing function of the project's own, so nothing here needs `rand`.
fn mix(a: u64, b: u64) -> u64 {
    let mut h = a ^ b.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^ (h >> 31)
}

/// **What has actually happened**, as distinct from what could be worked
/// out again.
///
/// The rule this type exists to enforce: an objective outcome may be
/// *derived* while it is still unresolved, and the moment it is resolved
/// it becomes history and is written down. Recomputing it later — after
/// a balance change, an RNG change, a new version — could alter an event
/// that a witness already remembers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Journal {
    resolved: BTreeMap<EventId, u64>,
}

impl Journal {
    pub fn new() -> Self {
        Journal::default()
    }

    /// **The objective outcome, resolved once.**
    ///
    /// Derived from the **world's** seed and a stable event id, never
    /// from any person's — two witnesses must not be able to generate
    /// incompatible versions of the same accident. Once taken it is
    /// recorded, and every later call returns what happened rather than
    /// what would happen.
    pub fn objective(&mut self, world_seed: u64, event: EventId) -> u64 {
        if let Some(v) = self.resolved.get(&event) {
            return *v;
        }
        let v = mix(world_seed, event);
        self.resolved.insert(event, v);
        v
    }

    /// Whether this has already happened.
    pub fn already(&self, event: EventId) -> Option<u64> {
        self.resolved.get(&event).copied()
    }

    /// **One person's reading of it**, which is a different question and
    /// is derived separately. Not recorded, because a perception is not a
    /// world fact — and not a function of the outcome, because two people
    /// must be able to be wrong about it differently.
    pub fn perception(person: u64, event: EventId) -> u64 {
        mix(person.wrapping_mul(0xD6E8_FEB8_6659_FD93), event)
    }

    pub fn len(&self) -> usize {
        self.resolved.len()
    }
    pub fn is_empty(&self) -> bool {
        self.resolved.is_empty()
    }
}

impl Store for Journal {
    fn store(&self, w: &mut Writer) {
        // A `BTreeMap`, so the same history writes the same bytes every
        // time — the reason it is not a `HashMap`.
        w.len(self.resolved.len());
        for (k, v) in &self.resolved {
            w.u64(*k);
            w.u64(*v);
        }
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let mut j = Journal::new();
        let n = r.len()?;
        for _ in 0..n {
            let (k, v) = (r.u64()?, r.u64()?);
            j.resolved.insert(k, v);
        }
        Ok(j)
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

        let mut out = Writer::new();
        out.bytes.extend_from_slice(MAGIC);
        out.u32(FORMAT);
        out.u32(crate::scaling::GENERATION_SCHEMA);
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
        let _schema = r.u32()?;
        let sum = r.u64()?;
        let n = r.len()?;
        let body = r.take(n)?;
        if checksum(body) != sum {
            return Err(SaveError::Checksum);
        }

        let mut b = Reader::new(body);
        let world_seed = b.u64()?;
        let day = b.u64()?;
        let n = b.len()?;
        let people = (0..n).map(|_| Coarse::load(&mut b)).collect::<Result<_, _>>()?;
        let journal = Journal::load(&mut b)?;
        Ok(Save { world_seed, day, people, journal })
    }
}
