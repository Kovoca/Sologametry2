//! **What somebody knows, how they came to know it, and what it does to
//! them later.**
//!
//! Slice 2 of `docs/mind-spec.md`. It begins with **provenance** rather
//! than with salience or forgetting, because lies, rumours, mistaken
//! identity, conflicting witnesses and reconstructed recall all depend on
//! that boundary being inviolable — and a boundary added afterwards is
//! not a boundary.
//!
//! ## Four objects, and they are not interchangeable
//!
//! | | |
//! |---|---|
//! | [`WorldEvent`] | what happened |
//! | [`Perceived`] | what *this* person witnessed, heard or inferred |
//! | [`Trace`] | what was encoded and kept |
//! | [`Recollection`] | a reconstruction, made now |
//!
//! A murder produces as many mental histories as there are minds near it.
//! One saw it clearly. One heard screaming and not who. A relative got an
//! official report a week later. A rumour blamed the wrong man. The
//! killer believes there were no witnesses. A citizen two streets away
//! never learns it happened at all.
//!
//! ## Recall cannot replay the original
//!
//! Enforced by the types rather than by discipline. A [`Trace`] keeps
//! what the event *meant at the time* as a [`RememberedFeeling`] — which
//! is a memory of having felt something, and is a different type from
//! [`Episode`]. There is no function anywhere that turns one into the
//! other. So recall must do what recall actually does:
//!
//! ```text
//! reconstruct the remembered content
//!   → appraise it with today's personality, values and concerns
//!   → produce new episodes
//!   → apply only their new effects
//! ```
//!
//! Which is why a memory can change meaning. The terror of a mine
//! collapse becomes, years on, grief for the dead, pride at having got
//! anybody out, and fresh anger on learning it was preventable — from the
//! same trace, because the person doing the remembering is not the same
//! person.

use crate::id::{Arena, Id};
use crate::mind::{Appraisal, Emotion, Episode, Facet, Happening, Mind, Value};
use crate::person::Person;
use crate::rng::Rng;

/// **What somebody looked like, when you could not say who they were.**
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Description(pub String);

/// **Who a perceiver thinks was involved — which is not the same as who
/// was.**
///
/// The world has a handle for every person in it. A mind must not acquire
/// that handle merely because the simulation has one, or there is no
/// mistaken identity, no rumour about a man who does not exist, and no
/// investigation to conduct.
///
/// A happening may objectively involve Alice while Bob sincerely
/// remembers that Carol did it — and his resentment then quite properly
/// aims at Carol. Correcting him later changes the attribution without
/// rewriting what he originally believed.
#[derive(Clone, Debug, PartialEq)]
pub enum PerceivedWho {
    /// Recognised. This is who it was.
    Known(Id<Person>),
    /// Named, with a doubt. **The dangerous one**, because it is
    /// actionable and can be wrong.
    Believed { person: Id<Person>, confidence: f32 },
    /// A figure in a dark coat. No handle at all, and there may never be
    /// one.
    Unknown(Description),
    /// "A guard." A role rather than a person.
    Role(u32),
    /// "Somebody from the north quarter."
    Group(u32),
}

impl PerceivedWho {
    /// The person this points at, where it points at one. **A `Believed`
    /// identification resolves too**, which is exactly what makes false
    /// blame able to damage the wrong relationship.
    pub fn person(&self) -> Option<Id<Person>> {
        match self {
            PerceivedWho::Known(p) => Some(*p),
            PerceivedWho::Believed { person, .. } => Some(*person),
            _ => None,
        }
    }

    /// How sure they are that this is who it was.
    pub fn certainty(&self) -> f32 {
        match self {
            PerceivedWho::Known(_) => 1.0,
            PerceivedWho::Believed { confidence, .. } => *confidence,
            _ => 0.0,
        }
    }

    /// Whether anybody real is named at all. **A fabricated rumour may
    /// name nobody**, and memory has to be able to hold that.
    pub fn is_anybody(&self) -> bool {
        self.person().is_some()
    }
}

/// Somewhere, identified. Becomes a real coordinate when the layers join.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Place(pub u32);

/// What sort of thing happened. Enough to cue a recollection, choose a
/// topic, and tell the ordinary from the exceptional.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventKind {
    Death,
    Injury,
    Collapse,
    Theft,
    Assault,
    Insult,
    Gift,
    Rescue,
    Promotion,
    Birth,
    Wedding,
    Discovery,
    // --- the ordinary run of a life ---
    Meal,
    Shift,
    Conversation,
}

impl EventKind {
    /// **The ordinary run of a life**, which is most of it.
    pub fn routine(self) -> bool {
        matches!(self, EventKind::Meal | EventKind::Shift | EventKind::Conversation)
    }
}

/// **What happened.** One record, whatever anybody made of it.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldEvent {
    pub kind: EventKind,
    pub who: Vec<Id<Person>>,
    /// Who did it, where there is such a person.
    pub actor: Option<Id<Person>>,
    pub place: Place,
    pub day: u64,
    /// How bad or good it was, before anybody read it.
    pub severity: f64,
    /// The facts an appraisal works from.
    pub facts: Happening,
}

/// **How somebody came to know.** Immutable once a trace exists: the
/// interpretation of a memory may drift for ever, but where it came from
/// may not, or a rumour can quietly become a thing you saw.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    /// Saw it.
    Witnessed,
    /// Heard part of it — a scream, a crash — without seeing who.
    Overheard,
    /// Somebody said so.
    /// Somebody said so. **The teller is a person you were talking to**,
    /// so they are known; what they *told* you may name anybody or
    /// nobody.
    Told { by: Id<Person> },
    /// It went through more than one mouth to get here.
    Rumour { hops: u8 },
    /// Read it in something official.
    Document,
    /// Worked it out.
    Inferred,
}

impl Source {
    /// **Did they see it themselves?** The question a court asks, and the
    /// one this whole module exists to keep answerable.
    pub fn firsthand(self) -> bool {
        matches!(self, Source::Witnessed | Source::Overheard)
    }

    /// How much a person starts out believing it, before anything they
    /// know about the teller.
    pub fn credence(self) -> f64 {
        match self {
            Source::Witnessed => 0.95,
            Source::Document => 0.85,
            Source::Overheard => 0.6,
            Source::Told { .. } => 0.6,
            Source::Inferred => 0.5,
            Source::Rumour { hops } => 0.45 * 0.7f64.powi(hops as i32),
        }
    }
}

/// **What one person took in.** May be wrong about the actor, the
/// severity, the place — and about whether it happened at all.
#[derive(Clone, Debug, PartialEq)]
pub struct Perceived {
    /// The event this is a perception *of*, where there is one. `None`
    /// means it never happened: somebody was lied to.
    pub of: Option<Id<WorldEvent>>,
    pub source: Source,
    pub kind: EventKind,
    /// Who they think did it. Not necessarily who did.
    pub believed_actor: Option<PerceivedWho>,
    pub place: Place,
    pub day: u64,
    /// How bad they took it to be.
    pub severity: f64,
    pub facts: Happening,
    /// How sure they are.
    pub confidence: f64,
}

/// **A memory of having felt something**, which is not a feeling.
///
/// Deliberately not an [`Episode`], and there is no conversion between
/// them anywhere in this module. That is what makes it impossible to
/// recall a bad day and be charged its stress a second time: there is
/// nothing to charge. Recall must appraise afresh.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RememberedFeeling {
    pub what: Emotion,
    pub how_strongly: f64,
}

/// **What was kept.** Half of it cannot change and half of it must.
#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    // --- immutable ---
    /// What was taken in at the time, kept as it was taken in.
    snapshot: Perceived,
    /// What it meant *then*, by the person they were then.
    encoding: Appraisal,
    /// That they were frightened — not the fright itself.
    felt: Vec<RememberedFeeling>,
    /// Where it came from. **Never edited**, or a rumour becomes an
    /// eyewitness account by being thought about often enough.
    provenance: Source,
    encoded_on: u64,

    // --- mutable ---
    /// How easily it comes to mind. Rises with recall, falls with time.
    pub accessibility: f64,
    /// How sure they are now, which is not how sure they were.
    pub confidence: f64,
    /// **Blame, reattributed.** Learning years later that the collapse
    /// was preventable does not change what was seen; it changes what it
    /// is taken to mean.
    pub blamed: Option<PerceivedWho>,
    pub recalls: u16,
    pub last_recalled: Option<u64>,
    /// Whether this is one of the few that made somebody who they are.
    pub core: bool,
    salience: f64,
}

impl Trace {
    pub fn source(&self) -> Source {
        self.provenance
    }
    pub fn what_was_perceived(&self) -> &Perceived {
        &self.snapshot
    }
    /// What it meant at the time. **Read-only**, and not an episode.
    pub fn as_encoded(&self) -> &Appraisal {
        &self.encoding
    }
    pub fn remembered_feelings(&self) -> &[RememberedFeeling] {
        &self.felt
    }
    pub fn salience(&self) -> f64 {
        self.salience
    }
    pub fn day(&self) -> u64 {
        self.encoded_on
    }

    /// **A memory of a claim, not of a fact.**
    ///
    /// "Bomrek told me the foreman knew" is what is stored, and it stays
    /// distinguishable from "I saw the foreman told" for ever. Without
    /// this there is no lying, no rumour, no investigation and no
    /// mistaken identity — only an omniscient population that happens to
    /// disagree.
    pub fn is_hearsay(&self) -> bool {
        !self.provenance.firsthand()
    }

    /// **Which happening this is a memory of**, where there is one.
    ///
    /// `None` is not a gap: it means there was no such event, which is
    /// what being lied to leaves behind.
    pub fn of_event(&self) -> Option<Id<WorldEvent>> {
        self.snapshot.of
    }

    /// How they came by it. **Never editable**, which is what stops a
    /// rumour becoming an eyewitness account by being gone over often
    /// enough.
    pub fn provenance(&self) -> Source {
        self.provenance
    }

    /// What kind of thing they take it to have been.
    pub fn kind(&self) -> EventKind {
        self.snapshot.kind
    }

    /// **What a core memory asks of a personality**, bounded, as a
    /// request rather than a write.
    ///
    /// It does not reach into a facet. It emits a signal, the caller
    /// decides whether to apply it, and `Personality::adapt` clamps what
    /// a whole life can do. A memory that could set a facet directly
    /// would make one bad afternoon able to replace somebody.
    pub fn plasticity(&self) -> Vec<(Facet, f32)> {
        if !self.core {
            return Vec::new();
        }
        let scale = (self.salience.min(1.0) * 0.15) as f32;
        let mut out = Vec::new();
        for f in &self.felt {
            match f.what {
                Emotion::Fear | Emotion::Anxiety => {
                    out.push((Facet::Anxiety, scale * f.how_strongly as f32))
                }
                Emotion::Pride | Emotion::Relief => {
                    out.push((Facet::Anxiety, -scale * f.how_strongly as f32))
                }
                Emotion::Grief => out.push((Facet::Gloom, scale * f.how_strongly as f32)),
                Emotion::Anger | Emotion::Resentment => {
                    out.push((Facet::Trust, -scale * f.how_strongly as f32))
                }
                Emotion::Gratitude | Emotion::Affection => {
                    out.push((Facet::Trust, scale * f.how_strongly as f32))
                }
                _ => {}
            }
        }
        out
    }
}

/// **Thirty ordinary meals are one fact about a life, not thirty
/// memories.**
///
/// The mechanism is **routine consolidation** — semanticisation — and not
/// suppression: human memory also does pattern separation precisely so
/// that similar experiences stay distinguishable. What is dropped is the
/// separate *episode*, while everything the repetition actually produced
/// is kept: familiarity, preference, habit, who you spent it with.
///
/// So a proposal over dinner, a bout of food poisoning, or the last meal
/// with a parent stays an episode. The other three hundred become "ate
/// with Bomrek most days for two years, and it was generally pleasant."
#[derive(Clone, Debug, PartialEq)]
pub struct Routine {
    pub kind: EventKind,
    pub place: Place,
    pub count: u32,
    pub first: u64,
    pub last: u64,
    /// Running mean of how good they were.
    pub quality: f64,
    /// Who was usually there.
    pub companions: Vec<(Id<Person>, u32)>,
    /// The range it ever covered — so "generally pleasant, once awful"
    /// survives even when the awful day did not stay an episode.
    pub best: f64,
    pub worst: f64,
}

/// What brought it back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cue {
    Place(Place),
    Person(Id<Person>),
    /// A year to the day, or several.
    Anniversary { day: u64 },
    /// Something of the same sort happening again.
    Similar(EventKind),
    /// Out of nowhere.
    Intrusive,
}

/// **A reconstruction, made now.** Not the memory: what the memory
/// becomes when it is brought up in a particular head on a particular
/// day.
#[derive(Clone, Debug, PartialEq)]
pub struct Recollection {
    pub of: Id<Trace>,
    /// Reconstructed content, which may be less than was encoded.
    pub content: Perceived,
    /// **Appraised now**, by the person they are now.
    pub appraisal: Appraisal,
    /// The new feelings, which are the only ones that count.
    pub episodes: Vec<Episode>,
    /// What they remember having felt, for comparison — never applied.
    pub then: Vec<RememberedFeeling>,
}

/// Everything one person has kept.
#[derive(Clone, Debug, Default)]
pub struct Memory {
    pub traces: Arena<Trace>,
    pub routines: Vec<Routine>,
    /// Perceptions that produced no trace at all, counted so a test can
    /// tell "nothing was encoded" from "nothing was perceived".
    pub discarded: u32,
}

/// How much of an event reaches somebody, 0 to 1. Slice 3 derives this
/// from the visibility system that already exists; for now the caller
/// says.
pub type Exposure = f64;

impl Memory {
    pub fn new() -> Self {
        Memory::default()
    }

    /// **Perceive an event, or fail to.**
    ///
    /// No exposure, no perception, and therefore no memory: somebody two
    /// streets away never learns it happened. What comes back is *this
    /// person's* version — which may have the actor wrong, the severity
    /// wrong, or in the case of a lie, no event behind it at all.
    pub fn perceive(
        &self,
        ev: &WorldEvent,
        id: Option<Id<WorldEvent>>,
        source: Source,
        exposure: Exposure,
        rng: &mut Rng,
    ) -> Option<Perceived> {
        if exposure <= 0.0 {
            return None;
        }
        // **How clearly it was taken in.** A witness at the far end of a
        // room and one standing over it do not encode the same event, and
        // that is where two honest accounts start to differ.
        let clarity = exposure.clamp(0.0, 1.0) * source.credence();
        let slip = (rng.next_f32() as f64 - 0.5) * 2.0 * (1.0 - clarity);
        // **How sure they are of the face is a third thing**, between
        // knowing and not. A clear look names somebody; a poor one names
        // somebody *and might be wrong*, which is where the wrong man
        // gets blamed.
        let believed_actor = ev.actor.and_then(|a| {
            if clarity > 0.75 {
                Some(PerceivedWho::Known(a))
            } else if clarity > 0.45 {
                Some(PerceivedWho::Believed { person: a, confidence: clarity as f32 })
            } else {
                None
            }
        });
        Some(Perceived {
            of: id,
            source,
            kind: ev.kind,
            believed_actor,
            place: ev.place,
            day: ev.day,
            severity: (ev.severity * (1.0 + 0.5 * slip)).clamp(-1.0, 1.0),
            facts: Happening {
                severity: (ev.severity * (1.0 + 0.5 * slip)).clamp(-1.0, 1.0),
                ..ev.facts
            },
            confidence: clarity,
        })
    }

    /// **Perceive it the way a particular sense delivered it.**
    ///
    /// `witness.rs` decides whether somebody saw it, heard it through a
    /// wall, or was told — and crucially whether they could say *who*.
    /// Seeing that something happened and seeing who did it are two
    /// different perceptions, and the gap between them is where mistaken
    /// identity lives.
    pub fn perceive_as(
        &self,
        ev: &WorldEvent,
        id: Option<Id<WorldEvent>>,
        w: &crate::witness::Witnessing,
        rng: &mut Rng,
    ) -> Option<Perceived> {
        let mut p = self.perceive(ev, id, w.source, w.exposure, rng)?;
        // The sense has the last word on identity: a clear line at thirty
        // metres gives you a figure, and a wall gives you a noise.
        if !w.could_identify {
            p.believed_actor = None;
        }
        Some(p)
    }

    /// **Somebody said so.** A claim, carried by whoever carried it, and
    /// it may be false.
    ///
    /// The event behind it is `None` when the teller made it up — which
    /// is what a lie is, and why a lie creates a belief about the world
    /// and does not modify the world.
    pub fn hear(
        kind: EventKind,
        blamed: Option<PerceivedWho>,
        place: Place,
        day: u64,
        from: Source,
    ) -> Perceived {
        Perceived {
            of: None,
            source: from,
            kind,
            believed_actor: blamed.clone(),
            place,
            day,
            severity: -0.5,
            facts: Happening {
                severity: -0.5,
                deliberate: blamed.is_some(),
                by_a_decision: false,
                ..Default::default()
            },
            confidence: from.credence(),
        }
    }

    /// **Encode it, or fold it into the run of ordinary days.**
    ///
    /// Salience decides which. What is kept as an episode is what was
    /// intense, unexpected, consequential or about somebody who matters;
    /// what is not becomes a line in a [`Routine`], and the fact of the
    /// repetition survives even though the separate days do not.
    pub fn encode(
        &mut self,
        p: Perceived,
        mind: &Mind,
        day: u64,
        felt: &[Episode],
    ) -> Option<Id<Trace>> {
        let appraisal = mind.read(&p.facts);
        let strongest = felt.iter().map(|e| e.strength).fold(0.0, f64::max);
        // emotional intensity + novelty + consequence + relevance
        let salience = (strongest * 1.2
            + p.facts.unexpected * 0.3
            + p.severity.abs() * 0.5
            + p.facts.to_mine * 0.3
            + if p.kind.routine() { 0.0 } else { 0.25 })
            * p.confidence.max(0.25);

        // **Routine consolidation**, not suppression. The episode is not
        // kept; everything the repetition produced is.
        if p.kind.routine() && salience < 0.55 {
            self.consolidate(&p, day);
            self.discarded += 1;
            return None;
        }

        let trace = Trace {
            encoding: appraisal,
            felt: felt
                .iter()
                .map(|e| RememberedFeeling { what: e.what, how_strongly: e.strength })
                .collect(),
            provenance: p.source,
            encoded_on: day,
            accessibility: (0.4 + 0.6 * salience).min(1.0),
            confidence: p.confidence,
            blamed: p.believed_actor.clone(),
            recalls: 0,
            last_recalled: None,
            core: salience > 1.1,
            salience,
            snapshot: p,
        };
        Some(self.traces.add(trace))
    }

    fn consolidate(&mut self, p: &Perceived, day: u64) {
        if let Some(r) = self
            .routines
            .iter_mut()
            .find(|r| r.kind == p.kind && r.place == p.place)
        {
            r.count += 1;
            r.last = day;
            let n = r.count as f64;
            r.quality += (p.severity - r.quality) / n;
            r.best = r.best.max(p.severity);
            r.worst = r.worst.min(p.severity);
            return;
        }
        self.routines.push(Routine {
            kind: p.kind,
            place: p.place,
            count: 1,
            first: day,
            last: day,
            quality: p.severity,
            companions: Vec::new(),
            best: p.severity,
            worst: p.severity,
        });
    }

    /// **What a cue brings back.** Ordered by how readily it comes.
    pub fn cued_by(&self, cue: Cue, today: u64) -> Vec<Id<Trace>> {
        let mut hits: Vec<(Id<Trace>, f64)> = self
            .traces
            .iter()
            .filter_map(|(id, t)| {
                let matches = match cue {
                    Cue::Place(p) => t.snapshot.place == p,
                    // **Whoever they think it was**, which may not be
                    // who it was — a cue reaches the memory a person
                    // actually holds.
                    Cue::Person(w) => {
                        t.snapshot.believed_actor.as_ref().and_then(|p| p.person()) == Some(w)
                            || t.blamed.as_ref().and_then(|p| p.person()) == Some(w)
                    }
                    Cue::Anniversary { day } => {
                        let since = today.saturating_sub(t.encoded_on);
                        since > 0 && since % 365 < 3 && t.encoded_on <= day
                    }
                    Cue::Similar(k) => t.snapshot.kind == k,
                    Cue::Intrusive => t.salience > 0.8,
                };
                matches.then_some((id, t.accessibility * t.salience))
            })
            .collect();
        hits.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        hits.into_iter().map(|(id, _)| id).collect()
    }

    /// **Bring it back, and appraise it as the person you are now.**
    ///
    /// The heart of the slice. Nothing stored is replayed: the remembered
    /// content is re-read by today's mind, against today's values and
    /// today's concerns, and what comes out are **new** episodes. The
    /// remembered feelings ride along for comparison and are never
    /// applied.
    ///
    /// Which is how a memory changes meaning without changing. The terror
    /// of a collapse becomes grief for the dead, pride at the rescue,
    /// acceptance after a hundred safe shifts — and fresh anger the day
    /// somebody learns it was preventable.
    pub fn recall(&mut self, which: Id<Trace>, mind: &Mind, today: u64) -> Option<Recollection> {
        let t = self.traces.get_mut(which)?;
        t.recalls += 1;
        t.last_recalled = Some(today);
        // **Rehearsal makes it easier to reach**, which is why the
        // memories somebody goes over are the ones that come back
        // uninvited.
        t.accessibility = (t.accessibility + 0.05).min(1.0);

        // Reconstructed, not retrieved: what comes back is degraded by
        // how reachable it was, and detail is what goes first.
        let mut content = t.snapshot.clone();
        if t.accessibility < 0.5 {
            content.believed_actor = t.blamed.clone().or(content.believed_actor);
            content.confidence *= t.accessibility * 2.0;
        }
        if let Some(b) = t.blamed.clone() {
            content.believed_actor = Some(b);
        }
        let then = t.felt.clone();
        let mut facts = content.facts;

        // **A memory is not a surprise.** Novelty is a large part of what
        // makes an event bite, and there is none at all in something you
        // already know happened — which is most of why remembering a
        // thing is survivable and living it was not.
        facts.unexpected = 0.05;

        // **Habituation.** Repeated exposure to the same memory, without
        // anything new in it, is exactly what makes exposure therapy
        // work: the hundredth recollection does not land like the first.
        //
        // Without this, a man who thinks about a bad day every day is
        // charged for it every day and breaks inside a year from one
        // event — which is not what rumination does, though it is
        // genuinely costly and remains so here.
        //
        // Learning something *new* about it is a different matter and
        // still bites, because that changes the facts rather than
        // rehearsing them.
        let dulled = 1.0 / (1.0 + 0.4 * t.recalls as f64);
        facts.severity *= dulled;
        facts.to_me *= dulled.sqrt();
        facts.to_mine *= dulled.sqrt();

        // Present-tense appraisal. **This is the only path to a feeling**
        // and it does not consult the encoding.
        let appraisal = mind.read(&facts);
        let episodes = mind.appraise(&appraisal);
        Some(Recollection { of: which, content, appraisal, episodes, then })
    }

    /// Time passes: what is not thought about gets harder to reach.
    /// **Never to nothing for a core memory** — those are what somebody
    /// is made of.
    pub fn a_day_passes(&mut self) {
        for (_, t) in self.traces.iter_mut() {
            let floor = if t.core { 0.35 } else { 0.02 };
            t.accessibility = (t.accessibility * 0.9985).max(floor);
        }
    }

    /// **Learning something later changes what it meant, not what was
    /// seen.**
    ///
    /// Reattributes blame while leaving the snapshot and the provenance
    /// exactly as they were — so a person can be wrong, be corrected, and
    /// still be able to say what they actually saw.
    pub fn reattribute(&mut self, which: Id<Trace>, to: PerceivedWho, confidence: f64) {
        if let Some(t) = self.traces.get_mut(which) {
            t.blamed = Some(to);
            t.confidence = confidence.clamp(0.0, 1.0);
        }
    }

    pub fn core_memories(&self) -> impl Iterator<Item = (Id<Trace>, &Trace)> {
        self.traces.iter().filter(|(_, t)| t.core)
    }
}

/// **What a witness would swear to.** Convenience for the thing this
/// module exists to make possible: asking somebody what they know and
/// getting an answer that carries where it came from.
pub fn testimony(t: &Trace) -> String {
    let who = match t.blamed.as_ref().or(t.snapshot.believed_actor.as_ref()) {
        Some(PerceivedWho::Known(p)) => format!("{p:?}"),
        Some(PerceivedWho::Believed { person, confidence }) => {
            format!("{person:?}, I think ({:.0}% sure)", confidence * 100.0)
        }
        Some(PerceivedWho::Unknown(Description(d))) => d.clone(),
        Some(PerceivedWho::Role(r)) => format!("one of the #{r}s"),
        Some(PerceivedWho::Group(g)) => format!("somebody from #{g}"),
        None => "somebody".into(),
    };
    match t.provenance {
        Source::Witnessed => format!("I saw {who} — {:?}", t.snapshot.kind),
        Source::Overheard => format!("I heard it — {:?} — I did not see who", t.snapshot.kind),
        Source::Told { by } => format!("{by:?} told me {who} did it"),
        Source::Rumour { hops } => format!("it is going round that {who} did it ({hops} removed)"),
        Source::Document => format!("the report says {who}"),
        Source::Inferred => format!("it must have been {who}"),
    }
}

/// Values a person may hold that change how a *remembered* event reads.
/// Used by the slice's own tests to vary one thing.
pub fn holds(mind: &mut Mind, topic: Value, to: i8) {
    for c in mind.values.iter_mut() {
        if c.topic == topic {
            c.held = to;
        }
    }
}
