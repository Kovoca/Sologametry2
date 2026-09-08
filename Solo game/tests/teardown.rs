//! **Taking a thing apart reads the thing, not the recipe.**
//!
//! Four gates, and each of them is a way a fixed uncraft recipe lies:
//! by handing back oak that was never in it, by returning glue as though
//! glue came back, by paying no attention to the state the object is in,
//! and by making a sledgehammer as good as an afternoon with a screwdriver.

use scale_sim::craft::{
    hand_tools, standard_recipes, Halt, Maker, RecipeBook, WorkOrder, Workplace,
};
use scale_sim::item::{standard_catalogue, Catalogue, ItemInstance, JointMethod};
use scale_sim::material::Material;
use scale_sim::teardown::{heat_mj, possible, take_apart, Recovered, Teardown};

fn world() -> (Catalogue, RecipeBook) {
    let cat = standard_catalogue();
    let book = standard_recipes(&cat);
    (cat, book)
}

fn a_good_hand() -> Maker {
    Maker {
        skill: 0.85,
        proficiency: 0.8,
        knows_recipe: true,
        tool_familiarity: 0.9,
        focus: 0.9,
        fatigue: 0.1,
    }
}

/// Make a chair, optionally out of something cheaper than the plan says.
fn a_chair(cat: &Catalogue, book: &RecipeBook, id: u64, cheap: bool) -> ItemInstance {
    let place = Workplace::a_workshop(hand_tools(cat));
    let mut o = WorkOrder::begin(id, book.must("chair, hand tools"), 1, 0, 1);
    if cheap {
        o.substituted(
            cat.must("oak board"),
            cat.must("particleboard sheet"),
            cat,
            book,
        )
        .expect("a sheet would not do");
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

/// **Everything of one material that came back, in whatever form.** A
/// component that survived whole is still made of oak, and a test that
/// counted only the loose piles would be reading which way one Bernoulli
/// went rather than what the chair was made of.
fn got(r: &Recovered, m: Material) -> f64 {
    r.material(m)
        + r.fuel_of(m)
        + r.components
            .iter()
            .map(|c| c.materials.fraction_of(m) * c.mass_kg)
            .sum::<f64>()
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

    let good = take_apart(&proper, Teardown::Disassemble, &cat, 0.8, 1);
    let cheap = take_apart(&bodged, Teardown::Disassemble, &cat, 0.8, 1);

    // The oak chair gives back oak, in whatever form oak can come back —
    // which for timber is firewood, because that is all timber ever is.
    assert!(
        got(&good, Material::Oak) > 1.0,
        "the oak chair gave back no oak"
    );
    assert_eq!(got(&good, Material::Particleboard), 0.0);

    // **And the particleboard chair gives back no oak of any kind** — not
    // as a component, not as material, not as firewood. The parts are
    // called `chair parts` in both, and what tells them apart is the
    // composition the record kept.
    assert_eq!(
        got(&cheap, Material::Oak),
        0.0,
        "particleboard came back as oak"
    );
    assert!(
        got(&cheap, Material::Particleboard) > 0.0,
        "the particleboard went nowhere at all"
    );

    // Both give back their screws, because a screw unscrews out of a glued
    // frame: what recovers a component is what holds *it*.
    for r in [&good, &cheap] {
        assert!(
            r.components
                .iter()
                .any(|c| c.definition == cat.must("wood screw")),
            "the screws stayed in"
        );
    }
}

/// **Gate: a proposed substitute is judged on shape, not on weight.**
///
/// A batten is the right timber at the right thickness and weighs what a
/// board weighs; it will not yield a seat. An offcut is the right width
/// and too short to get a leg out of. Measured by mass both are accepted,
/// and that is the case a conservation check cannot see.
#[test]
fn the_right_mass_in_the_wrong_shape_will_not_do() {
    let (cat, book) = world();
    let plan = book.must("chair, hand tools");
    let oak = cat.must("oak board");

    let try_it = |name: &str| {
        let mut o = WorkOrder::begin(1, plan, 1, 0, 1);
        o.substituted(oak, cat.must(name), &cat, &book)
    };

    // Real board stock, in two thicknesses and two materials.
    assert!(try_it("particleboard sheet").is_ok());
    assert!(try_it("pine board").is_ok());

    // Same timber, same thickness, wrong geometry.
    assert!(
        matches!(
            try_it("oak batten"),
            Err(scale_sim::craft::Unsuitable::WrongShape(_))
        ),
        "a 40 mm batten was accepted as chair stock"
    );
    assert!(
        matches!(
            try_it("oak offcut"),
            Err(scale_sim::craft::Unsuitable::WrongShape(_))
        ),
        "a 600 mm offcut was accepted as chair stock"
    );
    // Nothing you could cut a chair from at all.
    assert!(
        try_it("steel sheet").is_err(),
        "2 mm plate was accepted as chair stock"
    );
    assert!(try_it("glass pane").is_err());
    assert!(try_it("rope").is_err());
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
    let apart = take_apart(&chair, Teardown::Disassemble, &cat, 0.95, 1);
    assert_eq!(
        got(&apart, Material::Adhesive),
        0.0,
        "the glue came back out of the joint"
    );

    // The joint table is where this lives, and it is not one number.
    let bolted = JointMethod::Bolted.recovery();
    let glued = JointMethod::Glued.recovery();
    let welded = JointMethod::Welded.recovery();
    let stitched = JointMethod::Stitched.recovery();

    assert!(bolted.fastener > 0.9, "a bolt did not survive being undone");
    assert_eq!(glued.fastener, 0.0);
    assert_eq!(welded.fastener, 0.0);
    assert!(
        stitched.fastener < 0.1,
        "the thread came off the seam reusable"
    );

    // And the components differ as much as the fasteners do: a bolted
    // frame comes apart, a glued one tears, a welded one has to be cut.
    assert!(bolted.components > glued.components);
    assert!(glued.components > welded.components * 0.7);
    assert!(welded.needs_cutting && !bolted.needs_cutting);

    // Cast, forged and cooked are past the boundary entirely.
    for m in [
        JointMethod::Cast,
        JointMethod::Forged,
        JointMethod::Cooked,
        JointMethod::Reacted,
    ] {
        assert!(
            !m.reversible(),
            "{m:?} was treated as a joint that can be undone"
        );
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

    let a = take_apart(&sound, Teardown::Salvage, &cat, 0.7, 1);
    let b = take_apart(&wrecked, Teardown::Salvage, &cat, 0.7, 1);

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

    let (careful, smashed) = scale_sim::teardown::compare(
        &chair,
        Teardown::Deconstruct,
        Teardown::Smash,
        &cat,
        0.8,
        77,
    );

    let parts = |r: &scale_sim::teardown::Recovered| {
        r.components.iter().map(|c| c.count as f64).sum::<f64>()
    };
    assert!(
        parts(&careful) > parts(&smashed),
        "the sledgehammer returned as many parts"
    );
    assert_eq!(
        parts(&smashed),
        0.0,
        "a smashed chair yielded reusable components"
    );
    assert!(careful.lost_kg < smashed.lost_kg + chair.mass_kg * 0.5);
    assert!(
        careful.minutes > smashed.minutes * 5.0,
        "care took no longer than smashing"
    );

    // The smashed chair is firewood, and that is a real destination with a
    // real figure on it: dry timber is about 16-17 MJ/kg.
    assert!(smashed.fuel_kg() > 0.0, "nothing at all was left of it");
    let oak = smashed.fuel_of(Material::Oak);
    assert!(oak > 0.0, "the timber did not even burn");
    let mj = heat_mj(Material::Oak, oak);
    assert!(
        mj / oak > 16.0 && mj / oak < 17.0,
        "oak burnt at {:.1} MJ/kg",
        mj / oak
    );
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
    for how in [
        Teardown::Disassemble,
        Teardown::Deconstruct,
        Teardown::Salvage,
        Teardown::Recycle,
        Teardown::CutUp,
        Teardown::Smash,
    ] {
        let r = take_apart(&chair, how, &cat, 0.7, 1);
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

    let apart = take_apart(&chair, Teardown::Disassemble, &cat, 0.9, 1);
    let recycled = take_apart(&chair, Teardown::Recycle, &cat, 0.9, 1);

    assert!(
        !apart.components.is_empty(),
        "careful work returned no components"
    );
    assert!(
        recycled.components.is_empty(),
        "the shredder handed back a board"
    );

    // Steel screws are feedstock and come back as steel; oak only burns,
    // whoever is holding it and however carefully.
    assert!(recycled.fuel_kg() > 0.0, "the timber went nowhere");
    assert!(
        recycled.material(Material::MildSteel) > 0.0,
        "the screws were not recovered"
    );
    assert_eq!(
        recycled.material(Material::Oak),
        0.0,
        "a shredder turned oak into lumber"
    );
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

    let names = |r: &Recovered| {
        let mut v: Vec<&str> = r
            .components
            .iter()
            .filter_map(|c| cat.get(c.definition).map(|d| d.name))
            .collect();
        v.sort_unstable();
        v
    };

    // **Reach is categorical; whether one draw succeeds is not.** Over
    // twenty separate strip-downs a field strip must *never* reach the
    // pinned barrel or the riveted fire control group, and a proper
    // disassembly must reach both sometimes. Asserting either off a single
    // teardown is reading a Bernoulli, not a rule.
    let (mut strip_barrel, mut apart_barrel, mut strip_bolt, mut apart_fcg) = (0, 0, 0, 0);
    let (mut strip_minutes, mut apart_minutes) = (0.0f64, 0.0f64);
    for event in 0..20u64 {
        let strip = take_apart(&rifle, Teardown::FieldStrip, &cat, 0.9, event);
        let apart = take_apart(&rifle, Teardown::Disassemble, &cat, 0.9, event);
        strip_minutes += strip.minutes;
        apart_minutes += apart.minutes;
        if names(&strip).contains(&"barrel") {
            strip_barrel += 1;
        }
        if names(&strip).contains(&"bolt assembly") {
            strip_bolt += 1;
        }
        if names(&apart).contains(&"barrel") {
            apart_barrel += 1;
        }
        if names(&apart).contains(&"fire control group") {
            apart_fcg += 1;
        }
    }

    // **What a field strip reaches is what unclips and unbolts.**
    assert!(
        strip_bolt > 15,
        "the bolt carrier would not come out: {strip_bolt} of 20"
    );
    // **And what it does not reach is what was pressed and riveted.** A
    // barrel is an armourer job, and it is never a matter of luck.
    assert_eq!(strip_barrel, 0, "a field strip pulled the barrel");
    assert!(
        apart_barrel > 0,
        "a proper disassembly never reached the barrel"
    );
    assert!(apart_fcg > 0, "the rivets were never drilled out");
    assert!(
        apart_minutes > strip_minutes,
        "stripping took as long as a strip-down"
    );
}

/// **Gate: a teardown cannot be rerolled by reloading.**
///
/// And a different teardown of the same object is a different event, so it
/// may go differently — which is what makes it a chance rather than a
/// property of the object.
#[test]
fn the_same_teardown_twice_gives_the_same_answer() {
    let (cat, book) = world();
    let chair = a_chair(&cat, &book, 401, false);

    let a = take_apart(&chair, Teardown::Salvage, &cat, 0.6, 900);
    let b = take_apart(&chair, Teardown::Salvage, &cat, 0.6, 900);
    assert_eq!(
        a.components, b.components,
        "reloading gave a different pile"
    );
    assert_eq!(a.materials, b.materials);

    let mut counts = std::collections::BTreeSet::new();
    for event in 0..40u64 {
        let r = take_apart(&chair, Teardown::Salvage, &cat, 0.6, event);
        counts.insert(r.components.iter().map(|c| c.count).sum::<u32>());
    }
    assert!(
        counts.len() > 1,
        "every teardown of every chair returned the same number of parts"
    );
}

/// **Gate: a unique component is recovered or destroyed, never 0.6 of
/// one** — and over enough of them the share recovered approaches the
/// probability the joint table authored.
#[test]
fn one_component_is_a_coin_and_many_are_a_rate() {
    let (cat, book) = world();
    let chair = a_chair(&cat, &book, 402, false);
    let parts = cat.must("chair parts");

    let mut recovered = 0;
    let trials = 3000u64;
    for event in 0..trials {
        let r = take_apart(&chair, Teardown::Deconstruct, &cat, 0.8, event);
        for c in &r.components {
            // Never a fraction: a count is a whole number of things.
            assert!(c.count >= 1);
            if c.definition == parts {
                assert_eq!(
                    c.count, 1,
                    "one set of chair parts came back as {}",
                    c.count
                );
                recovered += 1;
            }
        }
    }
    // The glued joint recovers 0.45, deconstruction takes 0.80 of that,
    // and a skilled hand gets 0.91 of what is left: about a third.
    let rate = recovered as f64 / trials as f64;
    assert!(
        (0.25..=0.42).contains(&rate),
        "the parts came back {:.1}% of the time, which is not the authored chance",
        rate * 100.0
    );
}

/// **Common random numbers**: the same unit is tested against a higher
/// probability when the work is careful, so **careful recovery can never
/// come out worse than smashing** by an accident of sampling. That is why
/// the intention is deliberately not part of the draw's key.
#[test]
fn care_never_returns_less_than_carelessness() {
    let (cat, book) = world();
    let chair = a_chair(&cat, &book, 403, false);
    let parts = |r: &Recovered| r.components.iter().map(|c| c.count).sum::<u32>();

    for event in 0..200u64 {
        let careful = take_apart(&chair, Teardown::Disassemble, &cat, 0.8, event);
        let rough = take_apart(&chair, Teardown::Salvage, &cat, 0.8, event);
        let smashed = take_apart(&chair, Teardown::Smash, &cat, 0.8, event);
        assert!(
            parts(&careful) >= parts(&rough) && parts(&rough) >= parts(&smashed),
            "on event {event}: careful {} rough {} smashed {}",
            parts(&careful),
            parts(&rough),
            parts(&smashed)
        );
    }
}

/// Some actions are not available on some objects, and saying so is better
/// than handing back an empty pile and letting the caller guess why.
#[test]
fn an_action_that_cannot_be_attempted_says_so() {
    let cat = standard_catalogue();
    let loose = ItemInstance::one(&cat, cat.must("alternator"));
    assert!(
        !possible(&loose, Teardown::Uninstall),
        "an alternator on a bench was uninstalled"
    );

    // **A bill is not a parts list.** A board says what it is made of and
    // still has nothing in it that comes out as a component.
    let board = ItemInstance::one(&cat, cat.must("oak board"));
    assert!(
        board.assembly.is_some(),
        "a board did not say what it is made of"
    );
    assert!(
        !possible(&board, Teardown::Disassemble),
        "a plank was taken apart into components"
    );
    // Whereas a drill is parts all the way down.
    let drill = ItemInstance::one(&cat, cat.must("cordless drill"));
    assert!(possible(&drill, Teardown::Disassemble));
    assert!(possible(&drill, Teardown::FieldStrip));
    // But anything at all can be shredded or smashed.
    assert!(possible(&loose, Teardown::Recycle));
    assert!(possible(&loose, Teardown::Smash));
}

/// **An instance whose history is genuinely unknown can only be
/// weighed.**
///
/// Every definition now carries a bill, so this is no longer the ordinary
/// case — it is what is left when a particular object's own record is
/// missing: something recovered from a save an older generator wrote, or a
/// distant thing materialised without provenance. The fallback is honest
/// rather than a defect, and it is exactly why the aggregation rules
/// refuse to fold up anything whose particulars matter.
#[test]
fn a_thing_with_no_record_can_only_be_weighed() {
    let cat = standard_catalogue();
    let mut anonymous = ItemInstance::one(&cat, cat.must("wooden chair"));
    // The ordinary case first: it knows.
    assert!(
        anonymous.assembly.is_some(),
        "a spawned chair did not carry its bill"
    );
    let known = take_apart(&anonymous, Teardown::Disassemble, &cat, 0.9, 1);
    assert!(!known.components.is_empty());

    // And now with the history gone.
    anonymous.assembly = None;
    let r = take_apart(&anonymous, Teardown::Disassemble, &cat, 0.9, 1);
    assert!(
        r.components.is_empty(),
        "a chair with no history yielded named parts"
    );
    assert!(
        r.fuel_kg() > 0.0 || !r.materials.is_empty(),
        "it yielded nothing whatever"
    );
    assert!((r.accounted_kg() - anonymous.mass_kg).abs() < 1e-6);
}

/// **Gate: how many came back and what state they are in are two
/// questions.**
///
/// A brick off a lime-mortared wall comes away bonded and wants cleaning;
/// one cut out of cement comes away chipped; a smashed one is aggregate.
/// A single "recovered" count cannot say which, and what a reclamation
/// yard will pay for turns entirely on it.
#[test]
fn a_recovered_brick_has_a_grade_as_well_as_a_count() {
    use scale_sim::fitted::WallAssembly;
    use scale_sim::teardown::RecoveryGrade;

    let cat = standard_catalogue();
    let store = scale_sim::item::Store::new();
    let lime = WallAssembly::brick(30, &cat, JointMethod::LimeMortared);
    let cement = WallAssembly::brick(31, &cat, JointMethod::CementMortared);

    let grades = |w: &WallAssembly, how: Teardown| {
        let mut seen: Vec<RecoveryGrade> = Vec::new();
        for event in 0..30u64 {
            for c in &w.take_down(how, &cat, &store, 0.85, event).components {
                if !seen.contains(&c.grade) {
                    seen.push(c.grade);
                }
            }
        }
        seen
    };

    // Lime gives bricks back, and they come away still bonded — which is
    // a cleaning job somebody has to pay for rather than a free brick.
    let careful = grades(&lime, Teardown::Deconstruct);
    assert!(
        careful.contains(&RecoveryGrade::IntactBonded),
        "lime-mortared brick came off with no mortar on it: {careful:?}"
    );
    assert!(
        !careful.contains(&RecoveryGrade::IntactClean),
        "a mortared joint gave back a clean brick"
    );

    // Cement has to be cut out, so what survives is chipped.
    let cut = grades(&cement, Teardown::Deconstruct);
    assert!(
        cut.contains(&RecoveryGrade::Chipped),
        "cement-mortared brick came away undamaged: {cut:?}"
    );

    // A screwed timber frame gives back clean studs, which is the
    // contrast: the grade follows the joint and not the material.
    let door = ItemInstance::one(&cat, cat.must("car door"));
    let apart = take_apart(&door, Teardown::Disassemble, &cat, 0.9, 1);
    assert!(
        apart
            .components
            .iter()
            .any(|c| c.grade == RecoveryGrade::IntactClean),
        "nothing bolted came off clean"
    );

    // And the grades are ordered from best to worst, so a buyer can sort
    // on them.
    assert!(RecoveryGrade::IntactClean < RecoveryGrade::IntactBonded);
    assert!(RecoveryGrade::Chipped < RecoveryGrade::Broken);
    assert!(RecoveryGrade::Broken < RecoveryGrade::Contaminated);
}
