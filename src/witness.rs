//! **Whether somebody was in a position to know.**
//!
//! Slice 3 of `docs/mind-spec.md`. `memory.rs` takes an exposure and
//! decides what is perceived; this is where an exposure comes from, and
//! the answer is different at each rung of the ladder.
//!
//! The visibility system in `ground.rs` has been able to say *could this
//! person see that* since it was built, and nothing has ever asked it.
//!
//! ## Three tiers, because a distant miner has no tile
//!
//! The specification's §26. A person standing on generated ground gets a
//! real answer; a sampled person in a town gets a shared-context answer;
//! somebody a thousand kilometres away gets told or does not.
//!
//! | tier | how exposure is decided |
//! |---|---|
//! | **loaded** | line of sight, distance, and what a wall does to sound |
//! | **settlement** | whether two people share a household, a workplace, a tavern |
//! | **distant** | word of mouth, and only for what is worth carrying |
//!
//! Exposure stays the interface so all three can supply it. Wiring memory
//! directly to tiles would make it depend on everybody having a position
//! finer than which market they are in, which is the reification problem
//! and is not true of a sampled person.
//!
//! ## Seeing and hearing are not the same sense
//!
//! Which matters more than it sounds, because it is the whole reason two
//! honest witnesses give different accounts:
//!
//! - **Sight needs a clear line and it needs to be close.** You can make
//!   out a figure across a field and you cannot say who it is. Real
//!   recognition distances: a person is *detectable* at a kilometre in
//!   open ground, **recognisable at about 25 m**, and their expression is
//!   readable at about 10.
//! - **Sound does not need a line, and it goes much further.** It gets
//!   round corners, through doorways and — attenuated — through walls.
//!   Which is exactly the case of the neighbour who heard the screaming
//!   and cannot say who was shouting.
//!
//! So a wall between two people does not make one of them ignorant. It
//! makes them a different sort of witness.

use crate::ground::Ground;
use crate::id::Id;
use crate::memory::{EventKind, Exposure, Source};
use crate::person::Person;

/// **What a sound is, at a metre.** Real figures, in decibels:
///
/// | | dB at 1 m |
/// |---|---|
/// | ordinary talk | 60 |
/// | raised voice | 75 |
/// | a shout or a scream | **90** |
/// | a roof coming in | **120** |
pub fn loudness_db(kind: EventKind) -> f64 {
    match kind {
        EventKind::Collapse => 120.0,
        EventKind::Assault | EventKind::Injury | EventKind::Death => 90.0,
        EventKind::Insult => 75.0,
        EventKind::Wedding | EventKind::Birth => 80.0,
        EventKind::Conversation | EventKind::Meal => 60.0,
        EventKind::Shift => 70.0,
        _ => 65.0,
    }
}

/// **Background noise**, and it is what decides whether anything is
/// audible at all. A quiet room is 30 dB, a house 40, a street 60-70, a
/// factory floor 85 — which is why nobody hears anything on a factory
/// floor, and why a conversation two rooms away carries at night.
pub const AMBIENT_INDOORS_DB: f64 = 40.0;
pub const AMBIENT_STREET_DB: f64 = 65.0;

/// **A wall takes far more out of a voice than out of a rumble**, and the
/// difference is the reason you hear the bass through a party wall and
/// not the singing.
///
/// Masonry gives 35-45 dB of transmission loss at speech frequencies and
/// only about 15-20 dB down in the low end, where mass laws and stud
/// resonances stop helping. So a scream next door is muffled to nothing
/// through two walls while a roof coming in is heard across the street.
const WALL_MID_DB: f64 = 35.0;
const WALL_LOW_DB: f64 = 18.0;

/// **How low the sound sits**, 0 for a voice and 1 for a collapse. What
/// it decides is how much of it survives a wall.
pub fn low_frequency(kind: EventKind) -> f64 {
    match kind {
        EventKind::Collapse => 0.9,
        // Machinery: the reason a mill is audible from the next street.
        EventKind::Shift => 0.7,
        EventKind::Assault | EventKind::Injury | EventKind::Death => 0.15,
        EventKind::Conversation | EventKind::Insult | EventKind::Meal => 0.1,
        _ => 0.3,
    }
}

/// **Some sounds are built to be heard**, and treating audibility as
/// "louder than the background" misses it badly.
///
/// A scream is not just a loud voice. Screams occupy a **roughness** band
/// — 30-150 Hz amplitude modulation — that speech does not use at all,
/// and which reaches the amygdala by a shorter route *(Arnal et al.,
/// 2015)*. That is what makes one cut through a room of noise that would
/// bury a shout of the same level. Impulsive sounds get a smaller version
/// of the same advantage, because a bang stands out of a steady
/// background in a way a continuous sound does not.
///
/// Without this, one masonry wall made a scream at twelve metres
/// inaudible in a quiet room, which is plainly wrong — and it was wrong
/// because the model only knew how loud things were, not what they were.
fn cuts_through_db(kind: EventKind) -> f64 {
    match kind {
        EventKind::Assault | EventKind::Injury | EventKind::Death => 12.0,
        EventKind::Collapse => 6.0,
        _ => 0.0,
    }
}

/// **Recognising a face.** Reliable identification of somebody known runs
/// to about 25 m; beyond that you have a figure and a coat.
const RECOGNITION_M: f64 = 25.0;

/// **Where somebody is when it happens.**
#[derive(Clone, Debug, PartialEq)]
pub enum Vantage {
    /// Standing on generated ground, with a position. The real answer.
    OnTheGround { at: (i64, i64) },
    /// Somewhere in a settlement, sharing whatever contexts they share.
    /// No tile, because a sampled person does not have one.
    InTheSettlement { shares: Vec<Context> },
    /// Far enough away that only word of mouth reaches them.
    Elsewhere,
}

/// **Where people are thrown together.** The specification's §13: people
/// do not socialise because compatible records exist, they socialise
/// because they are in the same room for a reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Context {
    Household(u32),
    Workplace(u32),
    Tavern(u32),
    Temple(u32),
    Street(u32),
    Unit(u32),
}

/// What somebody got, and how.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Witnessing {
    pub exposure: Exposure,
    pub source: Source,
    /// **Whether they could say who.** Seeing that something happened and
    /// seeing who did it are two different perceptions, and the gap
    /// between them is where mistaken identity lives.
    pub could_identify: bool,
}

/// Nothing reached them.
pub const OBLIVIOUS: Option<Witnessing> = None;

/// **What one person on the ground got of an event on the ground.**
///
/// Sight is line of sight and range; hearing is an inverse-square law and
/// whatever the walls take out. Both are asked, because they give
/// different answers and the difference is the point.
pub fn from_the_ground(
    g: &Ground,
    observer: (i64, i64),
    event_at: (i64, i64),
    kind: EventKind,
    ambient_db: f64,
) -> Option<Witnessing> {
    let dx = (event_at.0 - observer.0) as f64;
    let dy = (event_at.1 - observer.1) as f64;
    let distance = (dx * dx + dy * dy).sqrt().max(1.0);

    // --- sight ---
    let seen = clear_line(g, observer, event_at);
    // Detail falls off with distance whether or not the line is clear.
    let sharpness = if seen {
        (RECOGNITION_M / distance).min(1.0)
    } else {
        0.0
    };

    // --- hearing ---
    //
    // Inverse square: 6 dB per doubling, which is 20·log10(distance).
    let walls = walls_between(g, observer, event_at) as f64;
    let lf = low_frequency(kind);
    let per_wall = WALL_MID_DB + (WALL_LOW_DB - WALL_MID_DB) * lf;
    let level = loudness_db(kind) - 20.0 * distance.log10() - walls * per_wall;
    let over_ambient = level + cuts_through_db(kind) - ambient_db;
    let heard = over_ambient > 0.0;

    if !seen && !heard {
        return OBLIVIOUS;
    }

    if seen && sharpness > 0.35 {
        // Close enough, and a clear line: they saw it and can say who.
        Some(Witnessing {
            exposure: (0.5 + 0.5 * sharpness).min(1.0),
            source: Source::Witnessed,
            could_identify: true,
        })
    } else if seen {
        // A clear line and too far to be sure of a face. They watched it
        // happen and could not tell you who.
        Some(Witnessing {
            exposure: (0.25 + 0.5 * sharpness).min(0.7),
            source: Source::Witnessed,
            could_identify: false,
        })
    } else {
        // Heard it through a wall or round a corner.
        Some(Witnessing {
            exposure: (over_ambient / 40.0).clamp(0.05, 0.5),
            source: Source::Overheard,
            could_identify: false,
        })
    }
}

/// **What somebody in the same town got**, when neither of them has a
/// tile.
///
/// Sharing a household is not the same as sharing a street: you see
/// everything that happens in the first and a fraction of what happens in
/// the second. A workplace sits between, and how much of it you see
/// depends on the work — two guards on a quiet gate talk for hours, and
/// two men on a stamping press cannot hear each other at all.
pub fn in_the_settlement(
    observer: &[Context],
    event_in: Context,
    kind: EventKind,
) -> Option<Witnessing> {
    if !observer.contains(&event_in) {
        return OBLIVIOUS;
    }
    let share = match event_in {
        Context::Household(_) => 0.95,
        Context::Unit(_) => 0.8,
        Context::Workplace(_) => 0.6,
        Context::Tavern(_) => 0.5,
        Context::Temple(_) => 0.45,
        // **A street is not a room.** Something happening on the street
        // you live on mostly does not happen in front of you.
        Context::Street(_) => 0.12,
    };
    // A collapse in the next room is not missed; a conversation is.
    let carrying = if kind.routine() { 0.7 } else { 1.0 };
    let exposure = share * carrying;
    Some(Witnessing {
        exposure,
        source: Source::Witnessed,
        could_identify: exposure > 0.4,
    })
}

/// **What reaches somebody who was nowhere near.**
///
/// Only what is worth carrying, and only through somebody. Real word of
/// mouth in a small community is fast and thorough — a death travels
/// through a village in a day — and a stranger's ordinary Tuesday travels
/// nowhere at all. Beyond about **150 people** *(Dunbar's number: the
/// group in which everybody knows everybody)* it stops being everybody's
/// business and starts being news, which is a different mechanism.
pub fn told(kind: EventKind, hops: u8, teller: Id<Person>) -> Option<Witnessing> {
    let worth_repeating = match kind {
        EventKind::Death | EventKind::Collapse | EventKind::Assault => 1.0,
        EventKind::Wedding | EventKind::Birth | EventKind::Theft => 0.8,
        EventKind::Promotion | EventKind::Rescue | EventKind::Discovery => 0.7,
        EventKind::Injury | EventKind::Insult | EventKind::Gift => 0.5,
        // Nobody carries a stranger's dinner to the next town.
        _ => 0.0,
    };
    if worth_repeating <= 0.0 {
        return OBLIVIOUS;
    }
    let source = if hops <= 1 {
        Source::Told { by: teller }
    } else {
        Source::Rumour { hops }
    };
    Some(Witnessing {
        exposure: worth_repeating * source.credence(),
        source,
        // **Second-hand identity is the weakest link in the chain**, and
        // it is where a rumour blames the wrong man.
        could_identify: hops <= 1,
    })
}

/// Is there a clear line from one to the other?
fn clear_line(g: &Ground, from: (i64, i64), to: (i64, i64)) -> bool {
    walk(g, from, to, |g, x, y| !g.blocks_sight(x, y))
}

/// How many opaque boundaries stand between them. **Sound gets through
/// them, quieter each time.**
pub fn walls_between(g: &Ground, from: (i64, i64), to: (i64, i64)) -> u32 {
    let mut n = 0;
    walk(g, from, to, |g, x, y| {
        if g.blocks_sight(x, y) {
            n += 1;
        }
        true
    });
    n
}

/// Bresenham between two world points, in window coordinates, skipping
/// both ends — you are not behind yourself and the thing you are looking
/// at is not in its own way.
fn walk(
    g: &Ground,
    from: (i64, i64),
    to: (i64, i64),
    mut each: impl FnMut(&Ground, usize, usize) -> bool,
) -> bool {
    let (mut x, mut y) = (from.0 - g.origin.0, from.1 - g.origin.1);
    let end = (to.0 - g.origin.0, to.1 - g.origin.1);
    let (dx, dy) = ((end.0 - x).abs(), -(end.1 - y).abs());
    let (sx, sy) = (if x < end.0 { 1 } else { -1 }, if y < end.1 { 1 } else { -1 });
    let mut err = dx + dy;
    loop {
        if (x, y) == end {
            return true;
        }
        if (x, y) != (from.0 - g.origin.0, from.1 - g.origin.1) {
            if x < 0 || y < 0 || x as usize >= g.w || y as usize >= g.h {
                return false;
            }
            if !each(g, x as usize, y as usize) {
                return false;
            }
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}
