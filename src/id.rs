//! **Durable identity.** Spec Phase 1, the first of four pieces.
//!
//! Everything in this simulation is identified by a bare `usize` index
//! into a `Vec`. That works exactly as long as nothing ever moves, and it
//! fails in three ways that matter:
//!
//! - **An index means nothing on its own.** `Site.market: usize` and a
//!   site's own position in `Ledger.sites` are the same type, so the
//!   compiler will happily let you pass one where the other belongs. That
//!   is not a hypothetical: this codebase already carries a
//!   `settlement_of_market` translation table precisely because two
//!   different `usize` spaces had to be kept apart by hand.
//! - **A removal silently re-points every index past it.** Nothing here
//!   removes a site yet, which is the only reason it has not bitten.
//! - **An index cannot be saved.** "Slot 7 of whatever vector this was"
//!   does not survive being written to disk and read back into a world
//!   built in a different order, which is what makes this the *first*
//!   piece of persistence rather than a tidying exercise.
//!
//! ## What this gives instead
//!
//! An `Id<T>` is a slot and a **generation**. The arena bumps the
//! generation when a slot is reused, so an identifier kept across a
//! removal is *detected* rather than silently pointing at whatever moved
//! in. That is the difference between a bug you find in a test and one
//! that quietly transfers a mill's stock to a hospital.
//!
//! `Id<T>` is `Copy`, is one `u64`, and carries its type in a
//! `PhantomData` that costs nothing at run time — so `Id<Site>` and
//! `Id<Market>` are different types to the compiler and the same bits to
//! the machine.
//!
//! **Deterministic, like everything else here.** Slots are handed out in
//! order and reused oldest-first, so a seed rebuilds the same world with
//! the same identifiers.

use std::fmt;
use std::marker::PhantomData;

/// A handle to something living in an [`Arena`].
///
/// Two `Id`s are equal when they name the same thing *and* the same
/// generation of it — an identifier for a site that has since been
/// removed and its slot reused is not equal to the new occupant.
pub struct Id<T> {
    slot: u32,
    generation: u32,
    of: PhantomData<fn() -> T>,
}

// Derived implementations would demand `T: Copy` and so on, which is
// wrong: an `Id` is a number and does not care what it points at.
impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Id<T> {}
impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.slot == other.slot && self.generation == other.generation
    }
}
impl<T> Eq for Id<T> {}
impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for Id<T> {
    /// **Ordered, because a save has to be written the same way twice.**
    /// The same rule the tile overlay follows for the same reason.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.slot, self.generation).cmp(&(other.slot, other.generation))
    }
}
impl<T> std::hash::Hash for Id<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.slot.hash(state);
        self.generation.hash(state);
    }
}
impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}v{}", self.slot, self.generation)
    }
}

impl<T> Id<T> {
    /// The slot, for indexing a parallel array. **Not** an identity: two
    /// different things can have held the same slot.
    pub fn slot(self) -> usize {
        self.slot as usize
    }

    /// Pack into a `u64`, for writing to a save.
    pub fn bits(self) -> u64 {
        ((self.slot as u64) << 32) | self.generation as u64
    }

    /// Unpack. The type has to be supplied by the caller, which is the
    /// one place the type safety has to be re-asserted by hand — so it
    /// happens in exactly one function, in the loader.
    pub fn from_bits(bits: u64) -> Self {
        Id {
            slot: (bits >> 32) as u32,
            generation: bits as u32,
            of: PhantomData,
        }
    }
}

/// **A collection whose handles stay valid.**
///
/// Backed by a `Vec`, so iteration is still a linear walk over contiguous
/// memory and the arithmetic is the same as it was. What changes is that
/// a handle carries a generation, and a removed slot is reused only after
/// its generation has been bumped.
#[derive(Clone, Debug, Default)]
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    /// Slots whose occupant was removed, oldest first — so reuse is
    /// deterministic and a seed rebuilds the same world.
    free: Vec<u32>,
}

#[derive(Clone, Debug)]
struct Slot<T> {
    generation: u32,
    value: Option<T>,
}

impl<T> Arena<T> {
    pub fn new() -> Self {
        Arena { slots: Vec::new(), free: Vec::new() }
    }

    pub fn with_capacity(n: usize) -> Self {
        Arena { slots: Vec::with_capacity(n), free: Vec::new() }
    }

    /// How many things are actually in it. Not the same as the number of
    /// slots, once anything has been removed.
    pub fn len(&self) -> usize {
        self.slots.len() - self.free.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The number of slots, which is what a parallel array must be sized
    /// to. Only grows.
    pub fn slots(&self) -> usize {
        self.slots.len()
    }

    pub fn add(&mut self, value: T) -> Id<T> {
        if let Some(slot) = self.free.first().copied() {
            self.free.remove(0);
            let s = &mut self.slots[slot as usize];
            s.value = Some(value);
            return Id { slot, generation: s.generation, of: PhantomData };
        }
        let slot = self.slots.len() as u32;
        self.slots.push(Slot { generation: 0, value: Some(value) });
        Id { slot, generation: 0, of: PhantomData }
    }

    /// Take something out. Its handle stops resolving from this moment,
    /// and the slot is not handed out again until its generation moves.
    pub fn remove(&mut self, id: Id<T>) -> Option<T> {
        let s = self.slots.get_mut(id.slot as usize)?;
        if s.generation != id.generation {
            return None;
        }
        let taken = s.value.take();
        if taken.is_some() {
            // **Bump on removal, not on reuse.** Bumping when the slot is
            // handed out again would leave a window in which a stale
            // handle still resolved — to nothing, but resolving at all is
            // the bug.
            s.generation = s.generation.wrapping_add(1);
            self.free.push(id.slot);
        }
        taken
    }

    pub fn get(&self, id: Id<T>) -> Option<&T> {
        let s = self.slots.get(id.slot as usize)?;
        if s.generation != id.generation {
            return None;
        }
        s.value.as_ref()
    }

    pub fn get_mut(&mut self, id: Id<T>) -> Option<&mut T> {
        let s = self.slots.get_mut(id.slot as usize)?;
        if s.generation != id.generation {
            return None;
        }
        s.value.as_mut()
    }

    /// Whether a handle still names something.
    pub fn holds(&self, id: Id<T>) -> bool {
        self.get(id).is_some()
    }

    /// Every live handle, in slot order.
    pub fn ids(&self) -> impl Iterator<Item = Id<T>> + '_ {
        self.slots.iter().enumerate().filter_map(|(i, s)| {
            s.value.as_ref().map(|_| Id {
                slot: i as u32,
                generation: s.generation,
                of: PhantomData,
            })
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = (Id<T>, &T)> + '_ {
        self.slots.iter().enumerate().filter_map(|(i, s)| {
            s.value.as_ref().map(|v| {
                (
                    Id { slot: i as u32, generation: s.generation, of: PhantomData },
                    v,
                )
            })
        })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Id<T>, &mut T)> + '_ {
        self.slots.iter_mut().enumerate().filter_map(|(i, s)| {
            let generation = s.generation;
            s.value.as_mut().map(move |v| {
                (Id { slot: i as u32, generation, of: PhantomData }, v)
            })
        })
    }

    pub fn values(&self) -> impl Iterator<Item = &T> + '_ {
        self.slots.iter().filter_map(|s| s.value.as_ref())
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut T> + '_ {
        self.slots.iter_mut().filter_map(|s| s.value.as_mut())
    }
}

impl<T> std::ops::Index<Id<T>> for Arena<T> {
    type Output = T;

    /// **Panics on a stale handle**, deliberately. Somewhere in this
    /// simulation a mill will hold an identifier for a market that no
    /// longer exists, and the choice is between a panic in a test and a
    /// tonne of flour delivered to whatever took its slot. The panic is
    /// cheaper.
    fn index(&self, id: Id<T>) -> &T {
        self.get(id)
            .unwrap_or_else(|| panic!("{id:?} no longer names anything"))
    }
}

impl<T> std::ops::IndexMut<Id<T>> for Arena<T> {
    fn index_mut(&mut self, id: Id<T>) -> &mut T {
        self.get_mut(id)
            .unwrap_or_else(|| panic!("{id:?} no longer names anything"))
    }
}

impl<T> FromIterator<T> for Arena<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut a = Arena::new();
        for v in iter {
            a.add(v);
        }
        a
    }
}
