//! **Every physical thing declares what it is made of, all the way down.**
//!
//! Grouping parts in the interface is allowed; omitting them from the data
//! is not. These gates are the enforcement — a definition that fails one
//! of them is a content error, not a runtime surprise.

use scale_sim::bom::{
    depth_of, explode, plausible_ends, tree, validate, Acquisition, Bom, BomEntry, EndOfLife,
    Flaw, Origin,
};
use scale_sim::craft::standard_recipes;
use scale_sim::item::{
    standard_catalogue, Catalogue, DefId, Family, ItemInstance, JointMethod,
};
use scale_sim::material::Material;
use scale_sim::teardown::{take_apart, Teardown};

fn plans(book: &scale_sim::craft::RecipeBook) -> Vec<&str> {
    book.recipes.iter().map(|r| r.name).collect()
}

// =====================================================================
// the contract
// =====================================================================

/// **Gate: the whole catalogue satisfies the contract.**
///
/// Mass and dimensions declared, a complete bill, every component id
/// resolving, the tree terminating, an origin and an end.
#[test]
fn every_definition_says_what_it_is_made_of() {
    let cat = standard_catalogue();
    let book = standard_recipes(&cat);
    let findings = validate(&cat, &plans(&book));
    assert!(
        findings.is_empty(),
        "{} content errors, first ten:\n{}",
        findings.len(),
        findings
            .iter()
            .take(10)
            .map(|f| format!("  {} — {:?}", f.name, f.flaw))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// **Gate: the validator actually bites.**
///
/// A test that only runs a clean catalogue through a checker proves the
/// catalogue is clean, not that the checker works. Each flaw is provoked.
#[test]
fn a_broken_definition_is_caught() {
    let good = standard_catalogue();
    let steel = good.must("steel sheet");

    let found = |c: &Catalogue| -> Vec<Flaw> {
        validate(c, &["a plan that exists"]).into_iter().map(|f| f.flaw).collect()
    };

    // No mass.
    let mut c = Catalogue::new();
    let id = c.add(a_thing("weightless", 0.0));
    c.set_bill(id, Bom::default().with_bulk(&[(Material::MildSteel, 0.0)]));
    assert!(found(&c).contains(&Flaw::NoMass));
    assert!(found(&c).iter().any(|f| matches!(f, Flaw::MasslessLine(_))));

    // No bill at all.
    let mut c = Catalogue::new();
    let id = c.add(a_thing("a mystery", 1.0));
    c.set_bill(id, Bom::default());
    assert!(found(&c).contains(&Flaw::NoBill));

    // The bill does not add up.
    let mut c = Catalogue::new();
    let id = c.add(a_thing("lighter than its parts", 1.0));
    c.set_bill(id, Bom::default().with_bulk(&[(Material::MildSteel, 4.0)]));
    assert!(found(&c).iter().any(|f| matches!(f, Flaw::MassMismatch { .. })));

    // A component that is not there.
    let mut c = Catalogue::new();
    let id = c.add(a_thing("full of ghosts", 1.0));
    c.set_bill(
        id,
        Bom::assembled(vec![BomEntry::new(DefId(999), 1, 1.0, "somewhere", JointMethod::Bolted)]),
    );
    assert!(found(&c).contains(&Flaw::UnknownComponent(DefId(999))));

    // A component whose own mass disagrees with the line.
    let mut c = Catalogue::new();
    let bolt = c.add(a_thing("bolt", 0.02));
    c.set_bill(bolt, Bom::default().with_bulk(&[(Material::MildSteel, 0.02)]));
    let id = c.add(a_thing("a fib", 1.0));
    c.set_bill(id, Bom::assembled(vec![BomEntry::new(bolt, 1, 1.0, "x", JointMethod::Bolted)]));
    assert!(found(&c).iter().any(|f| matches!(f, Flaw::ComponentMassMismatch { .. })));

    // Something that contains itself.
    let mut c = Catalogue::new();
    let id = c.add(a_thing("ouroboros", 1.0));
    c.set_bill(id, Bom::assembled(vec![BomEntry::new(id, 1, 1.0, "in itself", JointMethod::Bolted)]));
    assert!(found(&c).contains(&Flaw::Cycle));

    // No way in and no way out.
    let mut c = Catalogue::new();
    let id = c.add(a_thing("uncaused", 1.0));
    c.set_bill(id, Bom::default().with_bulk(&[(Material::MildSteel, 1.0)]));
    c.set_origin(id, vec![]);
    if let Some(d) = c.def_mut(id) {
        d.end_of_life = vec![];
    }
    let f = found(&c);
    assert!(f.contains(&Flaw::NoOrigin));
    assert!(f.contains(&Flaw::NoEndOfLife));

    // A plan nobody wrote.
    let mut c = Catalogue::new();
    let id = c.add(a_thing("vapourware", 1.0));
    c.set_bill(id, Bom::default().with_bulk(&[(Material::MildSteel, 1.0)]));
    c.set_origin(id, vec![Origin::Made { plan: "no such plan" }]);
    assert!(found(&c).contains(&Flaw::UnknownPlan("no such plan")));

    // An end its own materials rule out: solid steel does not compost.
    let mut c = Catalogue::new();
    let id = c.add(a_thing("a steel apple", 1.0));
    c.set_bill(id, Bom::default().with_bulk(&[(Material::MildSteel, 1.0)]));
    if let Some(d) = c.def_mut(id) {
        d.end_of_life = vec![EndOfLife::Biological];
    }
    assert!(found(&c).contains(&Flaw::ImplausibleEnd(EndOfLife::Biological)));

    let _ = steel;
}

fn a_thing(name: &'static str, kg: f64) -> scale_sim::item::ItemDefinition {
    scale_sim::item::bare(name, kg)
}

/// **Gate: no `craftable = false`.**
///
/// A route that names nothing is a prohibition wearing a disguise. Every
/// industrial origin says what it would take.
#[test]
fn nothing_is_simply_impossible() {
    let cat = standard_catalogue();
    for d in cat.iter() {
        assert!(!d.origin.is_empty(), "{} comes from nowhere", d.name);
        for o in &d.origin {
            if let Origin::Industrial { needs } = o {
                assert!(
                    !needs.is_empty(),
                    "{} is industrial and names nothing it needs",
                    d.name
                );
            }
        }
    }

    // The example that makes the point: a circuit board is not
    // uncraftable, it needs a fab, and the list says so.
    let board = cat.get(cat.must("circuit board")).unwrap();
    match &board.origin[0] {
        Origin::Industrial { needs } => {
            assert!(needs.len() >= 4, "a fab was described in {} words", needs.len());
            assert!(needs.iter().any(|n| n.contains("cleanroom")));
            assert!(needs.iter().any(|n| n.contains("silicon")));
        }
        other => panic!("a circuit board came from {other:?}"),
    }
}

/// **Gate: every definition can stop existing, and only in ways its own
/// materials allow.**
#[test]
fn everything_has_an_end() {
    let cat = standard_catalogue();
    for d in cat.iter() {
        assert!(!d.end_of_life.is_empty(), "{} can never be got rid of", d.name);
        // Disposal is always available, which is the floor rather than
        // the answer.
        assert!(d.end_of_life.contains(&EndOfLife::Disposal));
        let allowed = plausible_ends(&d.bill, &d.materials, d.family);
        for e in &d.end_of_life {
            assert!(allowed.contains(e), "{} claims {e:?} and its contents rule it out", d.name);
        }
    }

    // A loaf composts and a steel bracket does not.
    let loaf = cat.get(cat.must("loaf")).unwrap();
    assert!(loaf.end_of_life.contains(&EndOfLife::Biological));
    let sheet = cat.get(cat.must("steel sheet")).unwrap();
    assert!(!sheet.end_of_life.contains(&EndOfLife::Biological));
    assert!(sheet.end_of_life.contains(&EndOfLife::Recycling));
    // And an assembly can be taken apart where a board cannot.
    assert!(cat
        .get(cat.must("cordless drill"))
        .unwrap()
        .end_of_life
        .contains(&EndOfLife::Disassembly));
    assert!(!sheet.end_of_life.contains(&EndOfLife::Disassembly));
}

// =====================================================================
// the recursion
// =====================================================================

/// **Gate: the tree goes down, and it terminates in materials.**
///
/// The interface may show a drill as having a motor. Open the motor and
/// there is copper, laminated steel, ferrite and bearings — because it was
/// in the data all along.
#[test]
fn a_drill_opens_all_the_way_to_copper() {
    let cat = standard_catalogue();
    let drill = cat.must("cordless drill");
    let d = cat.get(drill).unwrap();

    // One level: named assemblies, not materials.
    let names: Vec<&str> = d
        .bill
        .components
        .iter()
        .filter_map(|c| cat.get(c.def).map(|x| x.name))
        .collect();
    for want in ["chuck assembly", "gearbox", "electric motor, small", "control assembly"] {
        assert!(names.contains(&want), "a drill has no {want}: {names:?}");
    }

    // The motor is itself an assembly, and its parts are real.
    let motor = cat.get(cat.must("electric motor, small")).unwrap();
    let inner: Vec<&str> = motor
        .bill
        .components
        .iter()
        .filter_map(|c| cat.get(c.def).map(|x| x.name))
        .collect();
    for want in ["motor winding", "stator laminations", "magnet", "ball bearing"] {
        assert!(inner.contains(&want), "a motor has no {want}: {inner:?}");
    }

    // And exploding the whole thing reaches materials, with the mass
    // conserved to the gram.
    let flat = explode(&cat, drill, d.nominal_mass_kg);
    let total: f64 = flat.iter().map(|m| m.1).sum();
    assert!(
        (total - d.nominal_mass_kg).abs() < 0.001,
        "a 1.6 kg drill exploded to {total:.4} kg"
    );
    for want in [Material::Copper, Material::Ferrite, Material::ToolSteel, Material::Abs,
                 Material::Lubricant] {
        assert!(
            flat.iter().any(|m| m.0 == want && m.1 > 0.0),
            "a drill contains no {}: {:?}",
            want.name(),
            flat.iter().map(|m| m.0.name()).collect::<Vec<_>>()
        );
    }

    // Deep enough to be a real tree rather than a list.
    assert!(depth_of(&cat, drill) >= 3, "a drill is only {} deep", depth_of(&cat, drill));
}

/// **The toaster, which is the example the contract was written about.**
/// No definition says only "toaster, 1.8 kg, steel".
#[test]
fn a_toaster_is_not_one_and_a_half_kilograms_of_steel() {
    let cat = standard_catalogue();
    let id = cat.must("toaster");
    let t = cat.get(id).unwrap();
    let flat = explode(&cat, id, t.nominal_mass_kg);

    // Nichrome elements on mica, which is what a toaster actually is and
    // is invisible if the definition says "steel".
    assert!(flat.iter().any(|m| m.0 == Material::Nichrome));
    assert!(flat.iter().any(|m| m.0 == Material::Mica));
    assert!(flat.iter().any(|m| m.0 == Material::Stainless));
    assert!(flat.iter().any(|m| m.0 == Material::Copper));
    assert!(flat.iter().any(|m| m.0 == Material::Rubber));
    assert!(flat.len() >= 6, "a toaster came out as {} materials", flat.len());

    let total: f64 = flat.iter().map(|m| m.1).sum();
    assert!((total - 1.8).abs() < 0.01, "the toaster exploded to {total:.3} kg");

    // The interface may group; the data does not.
    let mut s = String::new();
    tree(&cat, id, t.nominal_mass_kg, 0, &mut s);
    assert!(s.contains("heating element"));
    assert!(s.contains("nichrome") || s.contains("mica"));
}

/// **Gate: a rifle opens past the field-strip modules.**
///
/// A soldier sees a bolt carrier; an armourer sees the extractor, the
/// ejector, the firing pin and the springs — and destructive salvage
/// reduces the lot to steel.
#[test]
fn an_armourer_sees_further_than_a_soldier() {
    let cat = standard_catalogue();
    let bolt = cat.get(cat.must("bolt assembly")).unwrap();
    let inner: Vec<&str> = bolt
        .bill
        .components
        .iter()
        .filter_map(|c| cat.get(c.def).map(|x| x.name))
        .collect();
    for want in ["bolt body", "extractor", "ejector", "firing pin", "spring"] {
        assert!(inner.contains(&want), "a bolt assembly has no {want}: {inner:?}");
    }

    let fcg = cat.get(cat.must("fire control group")).unwrap();
    let inner: Vec<&str> = fcg
        .bill
        .components
        .iter()
        .filter_map(|c| cat.get(c.def).map(|x| x.name))
        .collect();
    for want in ["trigger", "hammer", "sear", "pin", "spring"] {
        assert!(inner.contains(&want), "a fire control group has no {want}: {inner:?}");
    }

    let rifle = cat.must("rifle");
    assert!(depth_of(&cat, rifle) >= 3);
    let flat = explode(&cat, rifle, 3.0);
    let total: f64 = flat.iter().map(|m| m.1).sum();
    assert!((total - 3.0).abs() < 0.02, "a rifle exploded to {total:.3} kg");
}

/// **A washing machine is mostly concrete**, which is the fact a
/// "steel appliance" definition hides and which is why moving one is a
/// two-person job.
#[test]
fn a_washing_machine_is_twenty_one_kilograms_of_concrete() {
    let cat = standard_catalogue();
    let id = cat.must("washing machine");
    let flat = explode(&cat, id, 70.0);
    let concrete = flat.iter().find(|m| m.0 == Material::Concrete).map(|m| m.1).unwrap_or(0.0);
    assert!(
        (20.0..=22.0).contains(&concrete),
        "the counterweights came to {concrete:.1} kg"
    );
    let total: f64 = flat.iter().map(|m| m.1).sum();
    assert!((total - 70.0).abs() < 0.05);
    // And the copper is real, which is why they are worth stripping.
    let copper = flat.iter().find(|m| m.0 == Material::Copper).map(|m| m.1).unwrap_or(0.0);
    assert!(copper > 2.0, "a washing machine held {copper:.2} kg of copper");
}

/// **Gate: a grouped detail is not a massless detail.**
///
/// A hundred and sixty screws may be one line with a count on it. They
/// still weigh 800 grams and they are still steel.
#[test]
fn a_group_of_fasteners_still_weighs_something() {
    let cat = standard_catalogue();
    let washer = cat.get(cat.must("washing machine")).unwrap();
    let screws = cat.must("machine screw");
    let line = washer
        .bill
        .components
        .iter()
        .find(|c| c.def == screws)
        .expect("a washing machine held together by nothing");

    assert!(line.count > 100, "only {} screws in a washing machine", line.count);
    assert!(line.kg > 0.5, "a hundred and sixty screws weighed {:.3} kg", line.kg);
    assert!((line.kg - line.count as f64 * 0.005).abs() < 1e-9);

    // And they turn up in the explosion as steel rather than vanishing.
    let flat = explode(&cat, cat.must("washing machine"), 70.0);
    assert!(flat.iter().any(|m| m.0 == Material::MildSteel && m.1 > 10.0));
}

// =====================================================================
// default composition against actual composition
// =====================================================================

/// **Gate: a spawned instance carries the default bill; a made one
/// carries what was actually used.**
#[test]
fn a_spawned_thing_and_a_made_thing_both_know_what_they_contain() {
    let cat = standard_catalogue();
    let chair = ItemInstance::one(&cat, cat.must("wooden chair"));

    // Straight out of the world: it has the authored bill.
    let rec = chair.assembly.as_ref().expect("a spawned chair contained nothing");
    assert!(!rec.components.is_empty());
    assert!(
        (rec.total_component_mass() - chair.mass_kg).abs() < chair.mass_kg * 0.03,
        "the default bill came to {:.3} against a {:.3} kg chair",
        rec.total_component_mass(),
        chair.mass_kg
    );
    // And it can be taken apart on the strength of it.
    let apart = take_apart(&chair, Teardown::Deconstruct, &cat, 0.8, 1);
    assert!((apart.accounted_kg() - chair.mass_kg).abs() < 1e-6);
    assert!(apart.components.iter().any(|c| c.definition == cat.must("wood screw")));
}

/// **Gate: taking apart the default bill cannot return more than the
/// thing contains.** Checked across every assembly in the catalogue, by
/// actually doing it.
#[test]
fn nothing_gives_back_more_than_it_weighs() {
    let cat = standard_catalogue();
    for d in cat.iter().filter(|d| d.is_assembly()) {
        let item = ItemInstance::one(&cat, d.id);
        for how in [Teardown::Disassemble, Teardown::Salvage, Teardown::Recycle, Teardown::Smash] {
            let r = take_apart(&item, how, &cat, 0.9, 5);
            assert!(
                r.mass_kg() <= item.mass_kg + 1e-6,
                "{} gave back {:.4} kg out of {:.4} by being {}",
                d.name,
                r.mass_kg(),
                item.mass_kg,
                how.name()
            );
            assert!(
                (r.accounted_kg() - item.mass_kg).abs() < 1e-6,
                "{}: {:.4} accounted for out of {:.4}",
                d.name,
                r.accounted_kg(),
                item.mass_kg
            );
        }
    }
}

/// **Gate: what was cured, set or reacted never comes back pristine** —
/// checked over the whole catalogue rather than on one chair.
#[test]
fn no_assembly_returns_its_own_adhesive() {
    let cat = standard_catalogue();
    let cured = [Material::Adhesive, Material::Paint, Material::Mortar, Material::Lubricant];
    for d in cat.iter().filter(|d| d.is_assembly()) {
        let item = ItemInstance::one(&cat, d.id);
        let r = take_apart(&item, Teardown::Disassemble, &cat, 0.99, 2);
        for m in cured {
            assert_eq!(
                r.material(m),
                0.0,
                "{} handed back {} out of the joint",
                d.name,
                m.name()
            );
        }
    }
}

/// Origins that are gathered rather than made say how. Timber is felled,
/// ore is mined, cotton is harvested.
#[test]
fn what_is_not_manufactured_is_got_from_somewhere() {
    let cat = standard_catalogue();
    let logged = cat.get(cat.must("oak board")).unwrap();
    assert!(logged.origin.contains(&Origin::Gathered(Acquisition::Logging)));

    let steel = cat.get(cat.must("steel sheet")).unwrap();
    match &steel.origin[0] {
        Origin::Industrial { needs } => assert!(needs.iter().any(|n| n.contains("ore"))),
        other => panic!("steel sheet came from {other:?}"),
    }

    // Everything a household buys has a plan or a named process.
    for f in [Family::Furniture, Family::Clothing, Family::Appliance, Family::Foodstuff] {
        for d in cat.of_family(f) {
            assert!(!d.origin.is_empty(), "{} comes from nowhere", d.name);
        }
    }
}
