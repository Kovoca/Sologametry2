//! **A name is not a place.**
//!
//! `id::Id<T>` is a slot and a generation, which makes it fast and makes it
//! a *position*: it says where something is kept. That is fine for reaching
//! into an arena within one session and wrong for identity, because the
//! same entity gets a different handle if it is stored in a different
//! order, and a freed slot is handed to somebody else with only a
//! generation counter standing between the two.
//!
//! This is the other primitive. A `Key<T>` is a number from a counter that
//! only ever goes up. It has nothing to do with where the thing is kept, it
//! is never given to anything else, and it means the same thing after a
//! save, after a reload, and after the entity has been promoted from a
//! statistic to a person standing on a tile.
//!
//! The rules, and every one of them is a gate:
//!
//! 1. **A key is not derived from a position, a name, or a coordinate.**
//! 2. **A key survives save and load**, and survives being loaded into
//!    detail and put back.
//! 3. **A dead key is never handed out again**, silently or otherwise.
//! 4. **A definition and an instance are different types.** A key to "what
//!    a car door is" cannot be passed where a key to "this car door" is
//!    wanted.
//! 5. **What is destroyed leaves a tombstone**, because a journal entry and
//!    a relationship both go on referring to the dead.
//! 6. **Storage order cannot affect the simulation.**
//! 7. **One entity keeps one key** however closely anybody is looking at it.
//!
//! # What the compiler settles
//!
//! Rule 4 is the one a test cannot prove. Sixteen behavioural assertions
//! can show that a definition is never passed where an instance belongs;
//! they cannot show that it *could not be*. These can. Each is paired with
//! a normal test in `tests/registry.rs` reaching the sibling beside it, so
//! a block passing because of a typo would be caught.
//!
//! **A definition is not a thing.** You cannot look up "what a car door
//! is" in a shed full of car doors:
//!
//! ```compile_fail
//! use scale_sim::registry::{DefKey, Registry};
//! let r: Registry<u32> = Registry::new();
//! let what_it_is: DefKey<u32> = DefKey::from_raw(1);
//! let _ = r.get(what_it_is);
//! ```
//!
//! **And a name from one world is not a name in another.** The bug this
//! exists to make impossible is already in the codebase in `usize` form —
//! `region.rs` carries a translation table between two index spaces
//! precisely because nothing stops one being passed as the other:
//!
//! ```compile_fail
//! use scale_sim::registry::{Key, Registry};
//! struct Cargo;
//! struct Person;
//! let r: Registry<Cargo> = Registry::new();
//! let somebody: Key<Person> = Key::from_raw(1);
//! let _ = r.get(somebody);
//! ```
//!
//! **A key is not an address**, so it cannot be used as one. There is no
//! `Index` impl and no conversion to `usize`:
//!
//! ```compile_fail
//! use scale_sim::registry::Key;
//! struct Cargo;
//! let v = vec![1, 2, 3];
//! let k: Key<Cargo> = Key::from_raw(1);
//! let _ = v[k];
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;

// =====================================================================
// the key
// =====================================================================

/// **What a particular thing is called**, for as long as anybody remembers
/// it and afterwards.
pub struct Key<T> {
    n: u64,
    of: PhantomData<fn() -> T>,
}

/// **What a *kind* of thing is called.** A separate type on purpose: the
/// commonest way an identity system goes wrong is a definition handle and
/// an instance handle being the same integer, so that "a car door" and
/// "this car door" can be passed to each other's functions.
pub struct DefKey<T> {
    n: u64,
    of: PhantomData<fn() -> T>,
}

macro_rules! key_impls {
    ($name:ident, $what:literal) => {
        impl<T> $name<T> {
            /// The number behind it, for writing to a save. **Not for
            /// indexing anything**: a key says which thing, never where it
            /// is.
            pub fn raw(self) -> u64 {
                self.n
            }

            /// Rebuild one that was written down. Only a loader should call
            /// this; everything else gets keys from a registry.
            pub fn from_raw(n: u64) -> Self {
                $name { n, of: PhantomData }
            }
        }

        impl<T> Clone for $name<T> {
            fn clone(&self) -> Self {
                *self
            }
        }
        impl<T> Copy for $name<T> {}
        impl<T> PartialEq for $name<T> {
            fn eq(&self, other: &Self) -> bool {
                self.n == other.n
            }
        }
        impl<T> Eq for $name<T> {}
        impl<T> PartialOrd for $name<T> {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
        impl<T> Ord for $name<T> {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.n.cmp(&other.n)
            }
        }
        impl<T> std::hash::Hash for $name<T> {
            fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
                self.n.hash(h);
            }
        }
        impl<T> fmt::Debug for $name<T> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}{}", $what, self.n)
            }
        }
    };
}

key_impls!(Key, "#");
key_impls!(DefKey, "def#");

// =====================================================================
// what became of something
// =====================================================================

/// **Why a key has no entity behind it any more.**
///
/// Kept rather than forgotten, because everything that referred to the
/// thing still refers to it: a journal entry, a debt, a grievance, a
/// memory, a photograph. You go on loving, fearing and owing the dead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tombstone {
    /// When it stopped existing.
    pub day: u64,
    /// What happened to it, in the caller's own words.
    pub how: String,
}

/// What a lookup found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lookup<'a, T> {
    /// Here it is.
    Live(&'a T),
    /// It existed and does not now — which is a different answer from
    /// never having existed, and callers act on the difference.
    Gone(&'a Tombstone),
    /// No such thing was ever registered. Almost always a bug.
    Unknown,
}

// =====================================================================
// the registry
// =====================================================================

/// **Everything of one kind, and what became of the ones that are gone.**
///
/// Iteration is in key order rather than storage order, so nothing about
/// how entries happen to be laid out can reach the simulation.
#[derive(Clone, Debug)]
pub struct Registry<T> {
    /// Only ever goes up. **This is what makes a key never reused**, and it
    /// is deliberately not derived from how many things are alive.
    next: u64,
    live: BTreeMap<u64, T>,
    gone: BTreeMap<u64, Tombstone>,
}

impl<T> Default for Registry<T> {
    fn default() -> Self {
        Registry::new()
    }
}

impl<T> Registry<T> {
    pub fn new() -> Self {
        Registry { next: 1, live: BTreeMap::new(), gone: BTreeMap::new() }
    }

    /// **Take a key and keep the thing.**
    pub fn add(&mut self, thing: T) -> Key<T> {
        let n = self.next;
        self.next += 1;
        self.live.insert(n, thing);
        Key { n, of: PhantomData }
    }

    pub fn get(&self, key: Key<T>) -> Option<&T> {
        self.live.get(&key.n)
    }

    pub fn get_mut(&mut self, key: Key<T>) -> Option<&mut T> {
        self.live.get_mut(&key.n)
    }

    /// **What became of it**, which distinguishes the three answers a
    /// caller actually needs: here it is, it is dead, and I have never
    /// heard of it.
    pub fn look(&self, key: Key<T>) -> Lookup<'_, T> {
        if let Some(t) = self.live.get(&key.n) {
            return Lookup::Live(t);
        }
        if let Some(t) = self.gone.get(&key.n) {
            return Lookup::Gone(t);
        }
        Lookup::Unknown
    }

    /// **End it, and remember that it ended.** The key is not returned to
    /// the pool; there is no pool.
    pub fn end(&mut self, key: Key<T>, day: u64, how: impl Into<String>) -> Option<T> {
        let was = self.live.remove(&key.n)?;
        self.gone.insert(key.n, Tombstone { day, how: how.into() });
        Some(was)
    }

    pub fn contains(&self, key: Key<T>) -> bool {
        self.live.contains_key(&key.n)
    }

    pub fn len(&self) -> usize {
        self.live.len()
    }

    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }

    /// How many have ever existed, alive or not. **The number a key is
    /// drawn from**, and it never goes down.
    pub fn ever(&self) -> u64 {
        self.next - 1
    }

    pub fn buried(&self) -> usize {
        self.gone.len()
    }

    /// **In key order, always.** Not insertion order, not hash order, not
    /// whatever a `Vec` happens to hold — so two worlds that registered the
    /// same things can never diverge because of how they were stored.
    pub fn iter(&self) -> impl Iterator<Item = (Key<T>, &T)> {
        self.live.iter().map(|(&n, t)| (Key { n, of: PhantomData }, t))
    }

    pub fn keys(&self) -> impl Iterator<Item = Key<T>> + '_ {
        self.live.keys().map(|&n| Key { n, of: PhantomData })
    }

    pub fn graves(&self) -> impl Iterator<Item = (Key<T>, &Tombstone)> {
        self.gone.iter().map(|(&n, t)| (Key { n, of: PhantomData }, t))
    }

    /// **Forget the graves of things nobody will ask about again.**
    ///
    /// A registry that kept a tombstone for every consignment ever
    /// delivered would grow with history rather than with the world, which
    /// is the unbounded state this project has had to remove three times.
    ///
    /// **It is only safe because the counter is written down.** A loader
    /// that derived it from the highest key present would start handing
    /// out the names of the recently pruned; because it does not, a grave
    /// can go without its name coming back. What the caller has to know is
    /// that a key whose grave has been pruned reads as `Unknown` rather
    /// than as `Gone` — so prune only when nothing can still be holding
    /// one.
    pub fn forget_graves_before(&mut self, day: u64) {
        self.gone.retain(|_, t| t.day >= day);
    }

    /// **Put back what a save wrote down**, keys and all.
    ///
    /// The counter is carried across rather than recomputed, or a world
    /// reloaded after some deaths would start handing out keys that the
    /// tombstones already claim.
    pub fn restore(next: u64, live: BTreeMap<u64, T>, gone: BTreeMap<u64, Tombstone>) -> Self {
        let highest = live.keys().chain(gone.keys()).copied().max().unwrap_or(0);
        Registry { next: next.max(highest + 1), live, gone }
    }

    /// For a codec: the raw contents, in key order.
    pub fn parts(&self) -> (u64, &BTreeMap<u64, T>, &BTreeMap<u64, Tombstone>) {
        (self.next, &self.live, &self.gone)
    }
}

// =====================================================================
// writing it down
// =====================================================================

use crate::save::{Reader, SaveError, Store, Writer};

/// A key is a bare number on the wire. **Nothing about it is derived**, so
/// there is nothing to recompute on the way back in — which is the whole
/// of rule 2.
impl<T> Store for Key<T> {
    fn store(&self, w: &mut Writer) {
        w.u64(self.n);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(Key { n: r.u64()?, of: PhantomData })
    }
}

impl<T> Store for DefKey<T> {
    fn store(&self, w: &mut Writer) {
        w.u64(self.n);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(DefKey { n: r.u64()?, of: PhantomData })
    }
}

impl Store for Tombstone {
    fn store(&self, w: &mut Writer) {
        w.u64(self.day);
        w.str(&self.how);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(Tombstone { day: r.u64()?, how: r.str()? })
    }
}

impl<T: Store> Store for Registry<T> {
    /// **The counter goes first and is written outright.** Deriving it on
    /// load from the highest key present would be one line shorter and
    /// wrong: a world whose last few entities have been buried and whose
    /// tombstones were pruned would start handing their names out again.
    fn store(&self, w: &mut Writer) {
        w.u64(self.next);
        w.len(self.live.len());
        for (&n, thing) in &self.live {
            w.u64(n);
            thing.store(w);
        }
        w.len(self.gone.len());
        for (&n, stone) in &self.gone {
            w.u64(n);
            stone.store(w);
        }
    }

    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        let next = r.u64()?;
        let mut live = BTreeMap::new();
        for _ in 0..r.count()? {
            let n = r.u64()?;
            let thing = T::load(r)?;
            // **A file naming one thing twice is broken, not newer.** The
            // same rule the journal keeps, and here it matters more: a
            // silent overwrite would make the loaded world depend on which
            // duplicate came last in the file.
            if live.insert(n, thing).is_some() {
                return Err(SaveError::Conflict(n));
            }
        }
        let mut gone = BTreeMap::new();
        for _ in 0..r.count()? {
            let n = r.u64()?;
            let stone = Tombstone::load(r)?;
            if gone.insert(n, stone).is_some() {
                return Err(SaveError::Conflict(n));
            }
        }
        // **Alive and buried at once is a contradiction**, not a state to
        // be resolved by whichever map was read second.
        if let Some(&n) = live.keys().find(|n| gone.contains_key(n)) {
            return Err(SaveError::Conflict(n));
        }
        Ok(Registry::restore(next, live, gone))
    }
}
