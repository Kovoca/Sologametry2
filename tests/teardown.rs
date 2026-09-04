//! **Taking a thing apart reads the thing, not the recipe.**
//!
//! Four gates, and each of them is a way a fixed uncraft recipe lies:
//! by handing back oak that was never in it, by returning glue as though
//! glue came back, by paying no attention to the state the object is in,
//! and by making a sledgehammer as good as an afternoon with a screwdriver.

use scale_sim::craft::{hand_tools, standard_recipes, Halt, Maker, RecipeBook, WorkOrder, Workplace};
use scale_sim::item::{standard_catalogue, Catalogue, ItemInstance, JointMethod};
use scale_sim::material::Material;
use scale_sim::teardown::{heat_mj, possible, take_apart, Teardown};

fn world() -> (Catalogue, RecipeBook) {
    let cat = standard_catalogue();
    let book = standard_recipes(&cat);
    (cat, book)
}

fn a_good_hand() -> Maker {
    Maker { skill: 0.85, proficiency: 0.8, knows_recipe: true, tool_familiarity: 0.9,
            focus: 0.9, fatigue: 0.1 }
}

/// Make a chair, optionally out of something cheaper than the plan says.
fn a_chair(cat: &Catalogue, book: &RecipeBook, id: u64, cheap: bool) -> ItemInstance {
    let place = Workplace::a_workshop(hand_tools(cat));
    let mut o = WorkOrder::begin(id, book.must("chair, hand tools"), 1, 0, 1);
    if cheap {
        o.substituted(cat.must("oak board"), cat.must("particleboard sheet"));
    }
    for _ in 0..400 {
        if o.finished() {
            break;
        }
        o.advance(60.0, book, cat, &place, a_good_hand());
    }
    assert_eq!(o.state, Halt::Done);
    o.deliver(book, cat, 1).expect("no chair came out")
}

fn got(r: &scale_sim::teardown::Recovered, m: Material) -> f64 {
    r.material(m) + r.fuel_of(m)
}

// =====================================================================

/// **Gate: a substitute cannot come back as the better material.**
///
/// The classic transmutation exploit. A recipe accepts oak or
/// particleboard; a fixed uncraft recipe returns oak whichever went in.
/// Reading the assembly record instead means the chair gives back what it
/// was made of.
#[test]
fn a_chair_of_particleboard_does_not_yield_oak() {
    let (cat, book) = world();
    let proper = a_chair(&cat, &book, 301, false);
    let bodged = a_chair(&cat, &book, 301, true);

    let good = take_apart(&proper, Teardown::Disassemble, &cat, 0.8);
    let cheap = take_apart(&bodged, Teardown::Disassemble, &cat, 0.8);

    // The oak chair gives back oak, in whatever form oak can come back —
    // which for timber is firewood, because that is all timber ever is.
    assert!(got(&good, Material::Oak) > 1.0, "the oak chair gave back no oak");
    assert_eq!(got(&good, Material::Particleboard), 0.0);

    // **And the particleboard chair gives back no oak of any kind** — not
    // as a component, not as material, not as firewood. The parts are
    // called `chair parts` in both, and what tells them apart is the
    // composition the record kept.
    assert_eq!(got(&cheap, Material::Oak), 0.0, "particleboard came back as oak");
    assert!(
        got(&cheap, Material::Particleboard) > 0.0,
        "the particleboard went nowhere at all"
    );

    // Both give back their screws, because a screw unscrews out of a glued
    // frame: what recovers a component is what holds *it*.
    for r in [&good, &cheap] {
        assert!(
            r.components.iter().any(|c| c.definition == cat.must("wood screw")),
            "the screws stayed in"
        );
    }
}

/// **Gate: glue does not come back.**
///
/// Adhesive, paint and mortar are cured; what they were before is not
/// recoverable at any level of care. Solder and bolts are, in their
/// different proportions, and that difference is the joint doing its work.
#[test]
fn what_was_cured_stays_cured() {
    let (cat, book) = world();
    let chair = a_chair(&cat, &book, 302, false);
    let apart = take_apart(&chair, Teardown::Disassemble, &cat, 0.95);
    assert_eq!(got(&apart, Material::Adhesive), 0.0, "the glue came back out of the joint");

    // The joint table is where this lives, and it is not one number.
    let bolted = JointMethod::Bolted.recovery();
    let glued = JointMethod::Glued.recovery();
    let welded = JointMethod::Welded.recovery();
    let stitched = JointMethod::Stitched.recovery();

    assert!(bolted.fastener > 0.9, "a bolt did not survive being undone");
    assert_eq!(glued.fastener, 0.0);
    assert_eq!(welded.fastener, 0.0);
    assert!(stitched.fastener < 0.1, "the thread came off the seam reusable");

    // And the components differ as much as the fasteners do: a bolted
    // frame comes apart, a glued one tears, a welded one has to be cut.
    assert!(bolted.components > glued.components);
    assert!(glued.components > welded.components * 0.7);
    assert!(welded.needs_cutting && !bolted.needs_cutting);

    // Cast, forged and cooked are past the boundary entirely.
    for m in [JointMethod::Cast, JointMethod::Forged, JointMethod::Cooked, JointMethod::Reacted] {
        assert!(!m.reversible(), "{m:?} was treated as a joint that can be undone");
    }
    // A crimp is not a weld: the case is reusable, which is why
    // handloading exists at all.
    assert!(JointMethod::Crimped.reversible());
    assert!(JointMethod::Crimped.recovery().fastener > 0.5);
}

/// **Gate: a damaged thing yields less.**
///
/// Which is what makes salvaging a wreck worse than salvaging a working
/// machine, and why a scrapyard pays by condition rather than by weight.
#[test]
fn a_wreck_gives_back_less_than_a_working_machine() {
    let (cat, book) = world();
    let sound = a_chair(&cat, &book, 303, false);
    let mut wrecked = sound.clone();
    wrecked.condition.damage = 0.75;

    let a = take_apart(&sound, Teardown::Salvage, &cat, 0.7);
    let b = take_apart(&wrecked, Teardown::Salvage, &cat, 0.7);

    let usable = |r: &scale_sim::teardown::Recovered| {
        r.components.iter().map(|c| c.count as f64).sum::<f64>()
    };
    assert!(
        usable(&b) < usable(&a),
        "a smashed chair gave back as many parts as a sound one: {} against {}",
        usable(&b),
        usable(&a)
    );
    assert!(b.lost_kg > a.lost_kg, "breaking it first cost nothing");

    // And what does come out of the wreck is in worse condition than what
    // came out of the sound one.
    if let (Some(x), Some(y)) = (a.components.first(), b.components.first()) {
        assert!(y.condition.serviceability() <= x.condition.serviceability());
    }
}


/// **Gate: deconstruction is not demolition.**
///
/// Same object, same person, two intentions. A careful afternoon returns
/// pieces somebody can build with; a sledgehammer returns firewood. If
/// these come out alike the whole action list is decoration.
#[test]
fn a_careful_hour_and_a_sledgehammer_do_not_return_the_same_pile() {
    let (cat, book) = world();
    let chair = a_chair(&cat, &book, 304, false);

    let (careful, smashed) =
        scale_sim::teardown::compare(&chair, Teardown::Deconstruct, Teardown::Smash, &cat, 0.8);

    let parts = |r: &scale_sim::teardown::Recovered| {
        r.components.iter().map(|c| c.count as f64).sum::<f64>()
    };
    assert!(parts(&careful) > parts(&smashed), "the sledgehammer returned as many parts");
    assert_eq!(parts(&smashed), 0.0, "a smashed chair yielded reusable components");
    assert!(careful.lost_kg < smashed.lost_kg + chair.mass_kg * 0.5);
    assert!(careful.minutes > smashed.minutes * 5.0, "care took no longer than smashing");

    // The smashed chair is firewood, and that is a real destination with a
    // real figure on it: dry timber is about 16-17 MJ/kg.
    assert!(smashed.fuel_kg() > 0.0, "nothing at all was left of it");
    let oak = smashed.fuel_of(Material::Oak);
    assert!(oak > 0.0, "the timber did not even burn");
    let mj = heat_mj(Material::Oak, oak);
    assert!(mj / oak > 16.0 && mj / oak < 17.0, "oak burnt at {:.1} MJ/kg", mj / oak);
    // Rubber carries twice the heat of wood, which is why a tyre fire is
    // what it is.
    assert!(heat_mj(Material::Rubber, 1.0) > 2.0 * heat_mj(Material::Pine, 1.0) * 0.9);
}

/// **Every kilogram lands somewhere**, including the ones that land
/// nowhere useful. A recovery that quietly drops mass is a recovery
/// nobody can check.
#[test]
fn every_teardown_accounts_for_the_whole_object() {
    let (cat, book) = world();
    let chair = a_chair(&cat, &book, 305, false);
    for how in [Teardown::Disassemble, Teardown::Deconstruct, Teardown::Salvage,
                Teardown::Recycle, Teardown::CutUp, Teardown::Smash] {
        let r = take_apart(&chair, how, &cat, 0.7);
        assert!(
            (r.accounted_kg() - chair.mass_kg).abs() < 1e-6,
            "{}: {:.4} kg accounted for out of {:.4}",
            how.name(),
            r.accounted_kg(),
            chair.mass_kg
        );
    }
}

/// **Recycling is the route that gets material back**, and it is a
/// different question from whether the components survive. A chair
/// disassembled gives you a board; a chair recycled gives you feedstock —
/// and for timber, which only ever burns, that means fuel and not lumber.
#[test]
fn recycling_asks_the_material_and_disassembly_asks_the_joint() {
    let (cat, book) = world();
    let chair = a_chair(&cat, &book, 306, false);

    let apart = take_apart(&chair, Teardown::Disassemble, &cat, 0.9);
    let recycled = take_apart(&chair, Teardown::Recycle, &cat, 0.9);

    assert!(!apart.components.is_empty(), "careful work returned no components");
    assert!(recycled.components.is_empty(), "the shredder handed back a board");

    // Steel screws are feedstock and come back as steel; oak only burns,
    // whoever is holding it and however carefully.
    assert!(recycled.fuel_kg() > 0.0, "the timber went nowhere");
    assert!(recycled.material(Material::MildSteel) > 0.0, "the screws were not recovered");
    assert_eq!(recycled.material(Material::Oak), 0.0, "a shredder turned oak into lumber");
    assert!(recycled.fuel_of(Material::Oak) > 0.0);
}

/// A field strip opens the modules and stops. It is not a disassembly with
/// a different name — what separates them is which joints get undone.
#[test]
fn a_field_strip_stops_at_the_modules() {
    let (cat, book) = world();
    let place = Workplace::a_workshop(hand_tools(&cat));
    let mut o = WorkOrder::begin(307, book.must("rifle, assembled"), 1, 0, 1);
    for _ in 0..400 {
        if o.finished() {
            break;
        }
        o.advance(60.0, &book, &cat, &place, a_good_hand());
    }
    assert_eq!(o.state, Halt::Done);
    let rifle = o.deliver(&book, &cat, 1).unwrap();

    let strip = take_apart(&rifle, Teardown::FieldStrip, &cat, 0.9);
    let apart = take_apart(&rifle, Teardown::Disassemble, &cat, 0.9);

    let names = |r: &scale_sim::teardown::Recovered| {
        let mut v: Vec<&str> = r
            .components
            .iter()
            .filter_map(|c| cat.get(c.definition).map(|d| d.name))
            .collect();
        v.sort_unstable();
        v
    };
    let stripped = names(&strip);
    let stripped_down = names(&apart);

    // **What a field strip reaches is what unclips and unbolts.**
    assert!(stripped.contains(&"bolt assembly"), "the bolt carrier stayed in");
    assert!(stripped.contains(&"stock"), "the stock would not come off");
    assert!(stripped.contains(&"magazine"));

    // **And what it does not reach is what was pressed and riveted.** A
    // barrel is an armourer job and a riveted fire control group is not
    // coming out at a kitchen table.
    assert!(!stripped.contains(&"barrel"), "a field strip pulled the barrel");
    assert!(!stripped.contains(&"fire control group"), "it drilled out the rivets too");
    assert!(stripped_down.contains(&"barrel"), "a proper disassembly left the barrel in");

    assert!(strip.components.len() < apart.components.len());
    assert!(strip.minutes < apart.minutes, "stripping took as long as a strip-down");
}

/// Some actions are not available on some objects, and saying so is better
/// than handing back an empty pile and letting the caller guess why.
#[test]
fn an_action_that_cannot_be_attempted_says_so() {
    let cat = standard_catalogue();
    let loose = ItemInstance::one(&cat, cat.must("alternator"));
    assert!(!possible(&loose, Teardown::Uninstall), "an alternator on a bench was uninstalled");
    assert!(!possible(&loose, Teardown::Disassemble), "a thing with no record came apart into parts");
    // But anything at all can be shredded or smashed.
    assert!(possible(&loose, Teardown::Recycle));
    assert!(possible(&loose, Teardown::Smash));
}

/// **An item that arrived as a statistic has no history to read**, so the
/// best anybody can do with it is weigh it and shred it. That is honest
/// rather than a defect — and it is exactly why the aggregation rules
/// refuse to fold up anything whose particulars matter.
#[test]
fn a_thing_with_no_record_can_only_be_weighed() {
    let cat = standard_catalogue();
    let anonymous = ItemInstance::one(&cat, cat.must("wooden chair"));
    assert!(anonymous.assembly.is_none());

    let r = take_apart(&anonymous, Teardown::Disassemble, &cat, 0.9);
    assert!(r.components.is_empty(), "a chair with no history yielded named parts");
    assert!(r.fuel_kg() > 0.0 || !r.materials.is_empty(), "it yielded nothing whatever");
    assert!((r.accounted_kg() - anonymous.mass_kg).abs() < 1e-6);
}
