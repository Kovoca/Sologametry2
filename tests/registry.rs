//! **A name is not a place.**
//!
//! Seven rules, seven gates. `id::Id<T>` is a slot and a generation, which
//! makes it a *position* — the same entity gets a different handle if it is
//! stored in a different order, and a freed slot is handed to somebody else
//! with only a counter between them. This is the primitive that is an
//! identity rather than an address, and each gate below is one clause of
//! the contract it has to keep.

use scale_sim::registry::{DefKey, Key, Lookup, Registry, Tombstone};
use scale_sim::save::{Reader, SaveError, Store, Writer};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
struct Cargo {
    what: String,
    tonnes: f64,
}

fn cargo(what: &str, tonnes: f64) -> Cargo {
    Cargo { what: what.to_string(), tonnes }
}

impl Store for Cargo {
    fn store(&self, w: &mut Writer) {
        w.str(&self.what);
        w.f64(self.tonnes);
    }
    fn load(r: &mut Reader) -> Result<Self, SaveError> {
        Ok(Cargo { what: r.str()?, tonnes: r.finite_f64()? })
    }
}

/// Through actual bytes, which is the only round trip that means anything.
fn through_bytes<T: Store>(thing: &T) -> T {
    let mut w = Writer::new();
    thing.store(&mut w);
    let mut r = Reader::new(&w.bytes);
    let back = T::load(&mut r).expect("it would not read back");
    assert!(r.done(), "the codec left bytes behind");
    back
}

/// **Rule 1: a key is not derived from a position, a name, or a
/// coordinate.**
///
/// The rule `id::Id` cannot keep. Register the same three things in two
/// different orders and each one still has to be able to say which is
/// which — and removing something must not renumber anything else.
#[test]
fn a_key_says_which_thing_not_where_it_is() {
    let mut a: Registry<Cargo> = Registry::new();
    let first = a.add(cargo("grain", 20.0));
    let second = a.add(cargo("coal", 30.0));
    let third = a.add(cargo("steel", 5.0));

    // **Nothing moves when something in the middle goes.** A slot-based
    // handle survives this by bumping a generation; a key survives it by
    // having had nothing to do with the layout in the first place.
    a.end(second, 10, "sold");
    assert_eq!(a.get(first).map(|c| c.what.as_str()), Some("grain"));
    assert_eq!(a.get(third).map(|c| c.what.as_str()), Some("steel"));
    assert!(a.get(second).is_none());

    // And the keys are distinct from one another and from anything about
    // the values.
    assert_ne!(first, second);
    assert_ne!(second, third);
    assert_ne!(first.raw(), third.raw());

    // Two registries that were given the same things in the same order
    // agree; that is determinism, not position-dependence. What matters is
    // that the *value* had no say in it.
    let mut b: Registry<Cargo> = Registry::new();
    let b1 = b.add(cargo("something else entirely", 999.0));
    assert_eq!(b1.raw(), first.raw(), "the key depended on what was in it");
}

/// **Rule 2: a key survives being written down and read back.**
#[test]
fn a_key_means_the_same_thing_after_a_reload() {
    let mut r: Registry<Cargo> = Registry::new();
    let one = r.add(cargo("grain", 20.0));
    let two = r.add(cargo("coal", 30.0));
    r.end(two, 5, "burnt");
    let three = r.add(cargo("steel", 5.0));

    // **Through the real codec**, not a struct clone. A round trip that
    // never becomes bytes proves the struct copies, which was never in
    // doubt.
    let back: Registry<Cargo> = through_bytes(&r);

    assert_eq!(back.get(one).map(|c| c.what.as_str()), Some("grain"));
    assert_eq!(back.get(three).map(|c| c.what.as_str()), Some("steel"));

    // A key survives on its own too, and is still the same key.
    assert_eq!(through_bytes(&one), one);
    assert_eq!(through_bytes(&DefKey::<Cargo>::from_raw(42)).raw(), 42);
    assert!(matches!(back.look(two), Lookup::Gone(_)), "the dead came back alive");

    // **And the counter came with it.** A world reloaded after some deaths
    // must not start handing out keys the tombstones already claim.
    let mut back = back;
    let fresh = back.add(cargo("timber", 1.0));
    assert_ne!(fresh, one);
    assert_ne!(fresh, two, "a reloaded world reissued a dead key");
    assert_ne!(fresh, three);
    assert!(fresh.raw() > three.raw());
}

/// **Rule 3: a dead key is never handed out again.**
///
/// Not "rarely", and not "with a generation counter so you can tell". The
/// counter only goes up, so there is no pool to be handed out of.
#[test]
fn nothing_is_ever_called_by_a_dead_name() {
    let mut r: Registry<Cargo> = Registry::new();
    let mut dead: Vec<Key<Cargo>> = Vec::new();
    for k in 0..50 {
        let key = r.add(cargo("grain", k as f64));
        if k % 2 == 0 {
            r.end(key, k, "consumed");
            dead.push(key);
        }
    }
    assert_eq!(r.len(), 25);
    assert_eq!(r.buried(), 25);

    // Fifty more, and not one of them may collide with a grave.
    for k in 0..50 {
        let key = r.add(cargo("coal", k as f64));
        assert!(
            !dead.contains(&key),
            "{key:?} was handed out again after being buried"
        );
        assert!(matches!(r.look(key), Lookup::Live(_)));
    }
    // The number keys are drawn from counts everything that ever was.
    assert_eq!(r.ever(), 100);
}

/// **Rule 4: a definition and an instance are different types.**
///
/// The commonest way an identity system goes wrong is both being the same
/// integer, so that "a car door" and "this car door" can be handed to each
/// other's functions. The compiler settles it — this gate records the
/// intention, and the `compile_fail` below proves it.
#[test]
fn what_a_thing_is_and_which_thing_it_is_are_different_questions() {
    let def: DefKey<Cargo> = DefKey::from_raw(7);
    let one: Key<Cargo> = Key::from_raw(7);
    // Same number underneath, and they are not interchangeable.
    assert_eq!(def.raw(), one.raw());
    assert_eq!(format!("{def:?}"), "def#7");
    assert_eq!(format!("{one:?}"), "#7");
}

/// **The paired siblings for the three `compile_fail` proofs** in
/// `registry`'s own documentation, which is where cargo actually runs
/// them — a doctest in an integration test is never run at all, which is
/// how the first version of this gate came to prove nothing.
///
/// A `compile_fail` block passes if its snippet fails for *any* reason, a
/// typo included, so each one needs the thing beside it that is allowed to
/// work reaching the same place.
#[test]
fn what_is_refused_has_a_sibling_that_is_allowed() {
    // Refused: a DefKey<u32> where a Key<u32> belongs. Allowed:
    let mut r: Registry<u32> = Registry::new();
    let k = r.add(5);
    assert_eq!(r.get(k), Some(&5));

    // Refused: a Key<Person> where a Key<Cargo> belongs. Allowed:
    let mut hold: Registry<Cargo> = Registry::new();
    let mine = hold.add(cargo("grain", 1.0));
    assert!(hold.get(mine).is_some());

    // Refused: indexing a Vec with a key. Allowed, because a key is a
    // number when a codec asks and never when a container does:
    let v = [10, 20, 30];
    assert_eq!(v[(mine.raw() - 1) as usize], 10);
}

/// **Rule 5: what is destroyed leaves a tombstone.**
///
/// Because everything that referred to it still refers to it — a journal
/// entry, a debt, a grievance, a memory. "Never heard of it" and "it is
/// dead" are different answers and callers act on the difference.
#[test]
fn the_dead_are_still_named() {
    let mut r: Registry<Cargo> = Registry::new();
    let lost = r.add(cargo("grain", 40.0));
    let never = Key::<Cargo>::from_raw(9_999);

    assert!(matches!(r.look(lost), Lookup::Live(_)));
    assert!(matches!(r.look(never), Lookup::Unknown));

    r.end(lost, 300, "went down with the ship");
    match r.look(lost) {
        Lookup::Gone(t) => {
            assert_eq!(t.day, 300);
            assert!(t.how.contains("ship"), "the tombstone forgot what happened");
        }
        other => panic!("a sunk cargo reads as {other:?}"),
    }
    // **And it is still distinguishable from something that never was.**
    assert!(matches!(r.look(never), Lookup::Unknown));
    assert_eq!(r.graves().count(), 1);
}

/// **Rule 6: storage order cannot affect the simulation.**
///
/// Iteration is in key order, so a pass over everything gives the same
/// sequence however the entries were inserted, removed, or reinserted.
#[test]
fn walking_the_registry_is_the_same_walk_every_time() {
    let mut r: Registry<Cargo> = Registry::new();
    let mut keys = Vec::new();
    for k in 0..30 {
        keys.push(r.add(cargo("grain", k as f64)));
    }
    // Churn it: remove scattered entries and add more.
    for (i, &k) in keys.iter().enumerate() {
        if i % 3 == 0 {
            r.end(k, i as u64, "used");
        }
    }
    for k in 0..10 {
        r.add(cargo("coal", k as f64));
    }

    let walked: Vec<u64> = r.keys().map(|k| k.raw()).collect();
    let mut sorted = walked.clone();
    sorted.sort();
    assert_eq!(walked, sorted, "iteration is not in key order");

    // And it is stable: the same walk twice.
    let again: Vec<u64> = r.keys().map(|k| k.raw()).collect();
    assert_eq!(walked, again);

    // A saved copy walks identically, which is the same claim across a
    // reload — and the bytes themselves are identical, because a map keyed
    // by number writes the same file every time.
    let back: Registry<Cargo> = through_bytes(&r);
    let after: Vec<u64> = back.keys().map(|k| k.raw()).collect();
    assert_eq!(walked, after);

    let (mut a, mut b) = (Writer::new(), Writer::new());
    r.store(&mut a);
    back.store(&mut b);
    assert_eq!(a.bytes, b.bytes, "the same registry wrote two different files");
}

/// **Rule 7: one entity keeps one key however closely anybody is
/// looking.**
///
/// The reification rule. A thing that is a statistic today and a person
/// standing on a tile tomorrow is the same thing, and promoting it must not
/// mint a second name for it — which is exactly how a sampled person's
/// savings came to be created out of nothing when they were individuated.
#[test]
fn looking_closer_does_not_make_it_a_different_thing() {
    let mut r: Registry<Cargo> = Registry::new();
    let coarse = r.add(cargo("grain", 40.0));

    // Promote: the same entity, now carried in detail.
    let detailed = cargo("grain, 40 t, in sacks", 40.0);
    *r.get_mut(coarse).expect("it stopped existing on the way up") = detailed.clone();
    assert_eq!(r.get(coarse), Some(&detailed));
    assert_eq!(r.len(), 1, "promoting it created a second entity");
    assert_eq!(r.ever(), 1, "promoting it consumed a second name");

    // And back down again.
    *r.get_mut(coarse).unwrap() = cargo("grain", 40.0);
    assert_eq!(r.len(), 1);
    assert_eq!(r.ever(), 1);
}

/// **And the registry is honest about what it has never seen.**
#[test]
fn an_unknown_key_is_not_quietly_something_else() {
    let r: Registry<Cargo> = Registry::new();
    let made_up = Key::<Cargo>::from_raw(12_345);
    assert!(r.get(made_up).is_none());
    assert!(!r.contains(made_up));
    assert!(matches!(r.look(made_up), Lookup::Unknown));
    assert_eq!(r.len(), 0);
    assert_eq!(r.ever(), 0);

    // A tombstone can be built by a loader and is distinguishable.
    let mut gone = BTreeMap::new();
    gone.insert(12_345u64, Tombstone { day: 1, how: "before this world".into() });
    let r: Registry<Cargo> = Registry::restore(1, BTreeMap::new(), gone);
    assert!(matches!(r.look(made_up), Lookup::Gone(_)));
}

/// **A file that names one thing twice is broken, not newer.**
///
/// The rule `save.rs` already keeps for the journal, and here it decides
/// something worse than a duplicate: without it, which entity a key refers
/// to would depend on which copy the reader happened to see last.
#[test]
fn a_name_used_twice_is_a_broken_file() {
    let mut w = Writer::new();
    w.u64(9); // next
    w.len(2); // two live
    w.u64(3);
    cargo("grain", 1.0).store(&mut w);
    w.u64(3); // ...both called #3
    cargo("coal", 2.0).store(&mut w);
    w.len(0);
    let mut r = Reader::new(&w.bytes);
    assert!(matches!(Registry::<Cargo>::load(&mut r), Err(SaveError::Conflict(3))));

    // And alive-and-buried at once is the same contradiction.
    let mut w = Writer::new();
    w.u64(9);
    w.len(1);
    w.u64(4);
    cargo("grain", 1.0).store(&mut w);
    w.len(1);
    w.u64(4);
    Tombstone { day: 1, how: "sank".into() }.store(&mut w);
    let mut r = Reader::new(&w.bytes);
    assert!(matches!(Registry::<Cargo>::load(&mut r), Err(SaveError::Conflict(4))));
}

/// **The counter is written down, not worked out.**
///
/// Deriving it from the highest key present is one line shorter and is the
/// bug: prune the tombstones of a world whose last entities are dead — or
/// simply bury the newest thing there is — and it starts handing their
/// names out again.
#[test]
fn a_reloaded_world_does_not_reissue_the_names_of_the_recently_dead() {
    let mut r: Registry<Cargo> = Registry::new();
    let a = r.add(cargo("grain", 1.0));
    let b = r.add(cargo("coal", 2.0));
    r.end(b, 4, "burnt");

    // A loader that dropped the grave — a legitimate thing to do with an
    // old one — and derived the counter would hand #2 out again.
    let (next, live, _gone) = r.parts();
    let stripped: Registry<Cargo> = Registry::restore(next, live.clone(), BTreeMap::new());
    let mut stripped = stripped;
    let fresh = stripped.add(cargo("steel", 3.0));
    assert_ne!(fresh, b, "a pruned grave let a dead name be reissued");
    assert_ne!(fresh, a);
    assert_eq!(fresh.raw(), 3);
}
