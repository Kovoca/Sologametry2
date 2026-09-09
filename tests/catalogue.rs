//! **What the item catalogue calls things.**
//!
//! Named for the catalogue rather than for `bin/make`, which is the
//! diagnostic that prints bills of materials — a test file called
//! `make.rs` would read as belonging to it.

/// **A definition's name on disk is not where it sits in the catalogue.**
///
/// `Catalogue::add` assigns `DefId(defs.len())` and the codec writes that
/// bare number, so inserting one definition earlier reinterprets every
/// saved item — a cordless drill becoming a brick, file intact. The same
/// defect the shipment's commodity had, in the one place where there are a
/// hundred and fifty-six of them.
///
/// This gate is the groundwork rather than the fix: it establishes that
/// every definition has a stable namespaced name, that no two share one,
/// and that the catalogue resolves it. Wiring it into the codec belongs
/// with the item store joining the save, because choosing what a save
/// should do with a key that no longer resolves is a decision that wants a
/// consumer to test it against.
#[test]
fn every_definition_has_a_stable_name_of_its_own() {
    use scale_sim::item::{authored_key, standard_catalogue};
    use std::collections::BTreeMap;

    let cat = standard_catalogue();
    assert!(
        cat.all().len() > 100,
        "only {} definitions — this is not the standard catalogue",
        cat.all().len()
    );

    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    for d in cat.all() {
        let key = d.key();
        assert!(
            key.starts_with("core:item/"),
            "{} has an unnamespaced key {key:?}",
            d.name
        );
        assert!(
            key.len() > "core:item/".len(),
            "{} has an empty key",
            d.name
        );
        assert!(
            key.chars()
                .skip("core:item/".len())
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '/'),
            "{} has a key with something unquotable in it: {key:?}",
            d.name
        );
        if let Some(other) = seen.insert(key.clone(), d.name) {
            panic!("{:?} and {:?} both answer to {key}", other, d.name);
        }
        // And the catalogue resolves it to this very definition.
        assert_eq!(
            cat.by_key(&key),
            Some(d.id),
            "{} does not resolve to itself through its own key",
            d.name
        );
    }

    // **And no two definitions answer to one English name either.** The
    // family in the key makes them distinguishable on disk; it does not
    // help `Catalogue::named`, which several callers hand a bare name —
    // `craft::hand_tools` asks for "hammer" to give a bench its striking
    // capability and got the carpenter's one only because it was added
    // before the gun part. Flip the order and a workshop silently loses
    // the ability to hit things.
    let mut by_name: BTreeMap<&str, usize> = BTreeMap::new();
    for d in cat.all() {
        *by_name.entry(d.name).or_insert(0) += 1;
    }
    let shared: Vec<&str> = by_name
        .iter()
        .filter(|(_, &n)| n > 1)
        .map(|(&name, _)| name)
        .collect();
    assert!(
        shared.is_empty(),
        "these names belong to more than one definition, so looking one up          returns whichever was added first: {shared:?}"
    );

    // A key nothing has is not a key something else has.
    assert_eq!(cat.by_key("core:item/tool/a_thing_nobody_ever_made"), None);

    // The shape is the review's, and it is derived from the authored name
    // rather than being a second name that can drift from the first.
    use scale_sim::item::Family;
    assert_eq!(
        authored_key(Family::Tool, "cordless drill"),
        "core:item/tool/cordless_drill"
    );
    assert_eq!(
        authored_key(Family::Stock, "2x4 board"),
        "core:item/stock/2x4_board"
    );
    assert_eq!(
        authored_key(Family::Tool, "  spaced  out  "),
        "core:item/tool/spaced_out"
    );

    // **And the collision this gate was built by finding.** A carpenter's
    // hammer and the hammer in a rifle's fire control group are different
    // objects sharing an English word, and they must not share a name on
    // disk.
    assert_ne!(
        authored_key(Family::Tool, "hammer"),
        authored_key(Family::SparePart, "hammer"),
        "the carpenter and the gunsmith are still fighting over one name"
    );
}
