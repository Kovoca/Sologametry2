//! **Making something is a process, not a timer.**
//!
//! Every gate here is a way a crafting menu quietly lies: by consuming the
//! whole bill of materials up front, by charging a bakery for the hour the
//! bread spends rising, by giving a factory a magic multiplier instead of
//! amortised setup, or by rerolling a botched weld when the game is
//! reloaded.

use scale_sim::craft::{
    capability_of, hand_tools, machine_shop, rolled_throughput_yield, standard_recipes, Effort,
    Grade, Halt, Maker, ProcessCapability, RecipeBook, WorkOrder, Workplace,
};
use scale_sim::item::{standard_catalogue, Capability, Catalogue, JointMethod};
use scale_sim::material::Material;

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

/// Run until it finishes or stops for a reason. Returns the reason.
fn run(
    order: &mut WorkOrder,
    book: &RecipeBook,
    cat: &Catalogue,
    place: &Workplace,
    m: Maker,
) -> Halt {
    let mut last = Halt::Running;
    for _ in 0..4000 {
        let before = (order.step, order.step_done, order.elapsed_min);
        last = order.advance(30.0, book, cat, place, m);
        if order.finished() {
            break;
        }
        if (order.step, order.step_done, order.elapsed_min) == before {
            break; // it is stuck on something and more minutes will not help
        }
    }
    last
}

// =====================================================================
// the plan
// =====================================================================

/// **Gate: several ways to make the same thing.**
///
/// A chair by hand, in a cabinet shop and in a factory are three plans
/// with one result. They differ in setup, difficulty and the tolerance
/// they demand — not in what a chair is.
#[test]
fn one_result_has_more_than_one_plan() {
    let (cat, book) = world();
    let ways = book.ways_to_make(cat.must("wooden chair"));
    assert!(
        ways.len() >= 3,
        "only {} way(s) to make a chair",
        ways.len()
    );

    let setups: Vec<f64> = ways
        .iter()
        .map(|&i| book.get(i).unwrap().setup_minutes)
        .collect();
    assert!(
        setups.iter().fold(0.0f64, |a, b| a.max(*b))
            > 10.0 * setups.iter().fold(f64::MAX, |a, b| a.min(*b))
    );

    // And the same materials come out of every one of them: scale buys
    // speed and consistency, never cheaper timber.
    let mass: Vec<f64> = ways
        .iter()
        .map(|&i| book.get(i).unwrap().implied_product_mass(&cat))
        .collect();
    for m in &mass {
        assert!(
            (m - mass[0]).abs() < 1e-9,
            "one plan made a chair of a different weight"
        );
    }
}

/// **Gate: mass balances across product, waste and emissions.**
///
/// Checked on the recipes as written, so a plan whose declared wastes are
/// wrong shows up as a chair of the wrong weight rather than as timber
/// quietly disappearing.
#[test]
fn nothing_is_created_and_nothing_is_destroyed() {
    let (cat, book) = world();
    for r in &book.recipes {
        let implied = r.implied_product_mass(&cat);
        let nominal = cat.get(r.result).unwrap().nominal_mass_kg;
        assert!(
            (implied - nominal).abs() <= nominal * 0.06,
            "{}: the inputs less the waste come to {implied:.4} kg and a {} is {nominal:.4} kg",
            r.name,
            cat.get(r.result).unwrap().name
        );
    }
}

/// **Water leaves a loaf as steam**, and the balance says so rather than
/// hiding it. Real bread loses about 10% of the dough weight in the oven.
#[test]
fn the_oven_drives_off_water_and_it_is_counted() {
    let (_cat, book) = world();
    let loaf = book.get(book.must("loaf")).unwrap();
    let emitted: f64 = loaf
        .all_produced()
        .iter()
        .filter_map(|f| match f {
            scale_sim::craft::Flow::Emission { material, kg } if *material == Material::Water => {
                Some(*kg)
            }
            _ => None,
        })
        .sum();
    assert!(emitted > 0.0, "baking a loaf lost nothing at all");
    let dough = 0.85;
    assert!(emitted / dough > 0.04 && emitted / dough < 0.15);

    // And the water came from the tap, not from stock — recorded, so the
    // balance still closes.
    let from_env: f64 = loaf
        .all_consumed()
        .iter()
        .filter_map(|f| match f {
            scale_sim::craft::Flow::Environment { kg, .. } => Some(*kg),
            _ => None,
        })
        .sum();
    assert!(from_env > 0.3);
}

/// **Gate: a tool is used, not eaten.**
#[test]
fn no_recipe_swallows_its_own_tools() {
    let (cat, book) = world();
    for r in &book.recipes {
        assert!(
            r.consumes_a_tool(&cat).is_none(),
            "{} eats {:?}",
            r.name,
            r.consumes_a_tool(&cat)
                .and_then(|d| cat.get(d))
                .map(|d| d.name)
        );
    }
    // Whereas welding wire, glue and thread genuinely are consumed, and
    // the catalogue says so.
    for n in ["welding wire", "wood glue", "thread reel"] {
        assert!(
            cat.get(cat.must(n)).unwrap().consumable,
            "{n} was not consumable"
        );
    }
}

// =====================================================================
// time
// =====================================================================

/// **Gate: unattended time advances without consuming labour.**
///
/// Twelve hours of glue curing is twelve hours on the clock and none of
/// anybody's day. Charging it as employee-minutes is how a model comes to
/// think furniture-making employs three times the people it does.
#[test]
fn nobody_is_paid_to_watch_glue_dry() {
    let (cat, book) = world();
    let place = Workplace::a_workshop(hand_tools(&cat));
    let mut o = WorkOrder::begin(11, book.must("chair, hand tools"), 1, 0, 1);

    // Get as far as the curing step.
    let mut guard = 0;
    while o.state != Halt::Unattended && guard < 500 {
        o.advance(20.0, &book, &cat, &place, a_good_hand());
        guard += 1;
    }
    assert_eq!(
        o.state,
        Halt::Unattended,
        "the order never reached a step nobody attends"
    );

    let labour = o.active_labour_min;
    let elapsed = o.elapsed_min;
    o.advance(600.0, &book, &cat, &place, a_good_hand());
    assert!(
        (o.active_labour_min - labour).abs() < 1e-9,
        "somebody was paid to watch glue dry"
    );
    assert!(
        (o.elapsed_min - elapsed - 600.0).abs() < 1e-6,
        "the clock did not run"
    );
    assert!(o.unattended_min >= 600.0);
}

/// **A loaf is 35 minutes of work and two hours of morning.**
///
/// Real: mixing and shaping and loading are the labour, the oven is
/// occupied for 35 minutes and the dough proves for an hour on its own.
#[test]
fn work_time_and_elapsed_time_are_different_numbers() {
    let (_cat, book) = world();
    let loaf = book.get(book.must("loaf")).unwrap();
    assert!(
        (loaf.labour_minutes() - 35.0).abs() < 1e-9,
        "labour came to {}",
        loaf.labour_minutes()
    );
    assert!((loaf.machine_minutes() - 35.0).abs() < 1e-9);
    assert!(
        loaf.span_minutes() > 120.0,
        "elapsed came to {}",
        loaf.span_minutes()
    );
    assert!(
        loaf.span_minutes() > 3.0 * loaf.labour_minutes(),
        "the clock and the wage bill were nearly the same figure"
    );

    // Which is the whole point: staffing a bakery off elapsed time hires
    // three and a half times too many people.
    let overstated = loaf.span_minutes() / loaf.labour_minutes();
    assert!(
        overstated > 3.4 && overstated < 3.8,
        "overstatement was {overstated:.2}x"
    );
}

/// **Gate: a batch saves setup and nothing else.**
///
/// Per-unit labour and per-unit material are untouched, which is why a
/// factory is cheaper per chair and not free.
#[test]
fn a_batch_amortises_the_setup_and_not_the_work() {
    let (cat, book) = world();
    let r = book.get(book.must("chair, factory")).unwrap();

    let one = r.labour_for_batch(1);
    let forty = r.labour_for_batch(40);
    let per_one = one;
    let per_forty = forty / 40.0;

    assert!(
        per_forty < per_one,
        "a run of forty was no cheaper per chair"
    );
    // But never below the labour the chair itself takes.
    assert!(
        per_forty > r.labour_minutes() * 0.999,
        "the fortieth chair took less work than making a chair"
    );
    // And the saving is exactly the setup, spread.
    assert!((forty - (r.setup_minutes + 40.0 * r.labour_minutes())).abs() < 1e-6);

    // Material per unit does not move at all.
    let m1 = r.implied_product_mass(&cat);
    assert!(m1 > 0.0);
    let _ = cat;
}

/// **Gate: the same plan run by a man and by a works.**
///
/// Identical operations, identical materials. What differs is the
/// providers — and what that buys is speed, not cheaper timber.
#[test]
fn one_recipe_two_workshops() {
    let (cat, book) = world();
    let plan = book.must("chair, hand tools");

    let bench = Workplace::a_workshop(hand_tools(&cat));
    let works = Workplace::a_factory(machine_shop(&cat), 4.0, 4);

    let mut by_hand = WorkOrder::begin(21, plan, 1, 0, 1);
    let mut by_works = WorkOrder::begin(21, plan, 1, 0, 1);
    assert_eq!(
        run(&mut by_hand, &book, &cat, &bench, a_good_hand()),
        Halt::Done
    );
    assert_eq!(
        run(&mut by_works, &book, &cat, &works, a_good_hand()),
        Halt::Done
    );

    let a = by_hand.deliver(&book, &cat, 1).unwrap();
    let b = by_works.deliver(&book, &cat, 1).unwrap();

    // The same chair, out of the same timber. **Material spoiled over the
    // plan is scrap, not a heavier chair** — so the two come out at the
    // same weight even though the bench got through more board.
    assert!(
        (a.mass_kg - b.mass_kg).abs() < 1e-6,
        "the works made a chair of a different weight"
    );
    assert_eq!(a.materials.chiefly(), b.materials.chiefly());

    let board = cat.must("oak board");
    let used = |o: &WorkOrder| {
        o.consumed_items
            .iter()
            .find(|c| c.0 == board)
            .map(|c| c.1)
            .unwrap_or(0.0)
    };
    assert!(used(&by_hand) >= 1.0 && used(&by_works) >= 1.0);
    assert!(
        used(&by_works) <= used(&by_hand),
        "the jigged shop wasted more board than the bench: {:.3} against {:.3}",
        used(&by_works),
        used(&by_hand)
    );

    // The works is quicker in hands-on work, and both wait the same twelve
    // hours for the glue.
    assert!(
        by_works.active_labour_min < by_hand.active_labour_min,
        "machines saved no labour: {:.0} against {:.0}",
        by_works.active_labour_min,
        by_hand.active_labour_min
    );
    assert!(by_works.unattended_min >= 720.0 && by_hand.unattended_min >= 720.0);

    // And only the works drew any power.
    assert!(
        by_hand.power_kwh == 0.0,
        "a man with a handsaw used electricity"
    );
    assert!(by_works.power_kwh > 0.0, "a bandsaw ran on nothing");
}

// =====================================================================
// interruption
// =====================================================================

/// **Gate: power loss pauses the step that needs power.**
///
/// And it names the right reason: a plant waiting on the grid is a
/// different problem from a plant that never had the machine.
#[test]
fn the_grid_goes_down_and_the_oven_stops() {
    let (cat, book) = world();
    let plan = book.must("loaf");
    let mut dark = Workplace::a_factory(machine_shop(&cat), 2.0, 1);
    dark.power = false;

    let mut o = WorkOrder::begin(31, plan, 1, 0, 1);
    let why = run(&mut o, &book, &cat, &dark, a_good_hand());
    assert_eq!(why, Halt::NoPower, "the bakery carried on in the dark");
    assert_eq!(
        o.step, 3,
        "it stopped at step {} rather than at the oven",
        o.step
    );
    // Mixing, shaping and proving all happened. The dough is real and it
    // is sitting there.
    assert!(o
        .intermediates
        .iter()
        .any(|&(d, n)| d == cat.must("risen dough") && n > 0.0));

    // Put the power back and it finishes from where it stopped.
    let lit = Workplace::a_factory(machine_shop(&cat), 2.0, 1);
    assert_eq!(run(&mut o, &book, &cat, &lit, a_good_hand()), Halt::Done);
    assert!(o.deliver(&book, &cat, 2).is_some());
}

/// **And a blackout stops the factory, not the joiner.** The same plan,
/// the same moment, two workshops: one has nothing that runs without
/// power and carries on.
#[test]
fn a_man_with_a_handsaw_does_not_notice_a_blackout() {
    let (cat, book) = world();
    let plan = book.must("chair, hand tools");

    let mut dark_bench = Workplace::a_workshop(hand_tools(&cat));
    dark_bench.power = false;
    let mut dark_works = Workplace::a_factory(machine_shop(&cat), 4.0, 4);
    dark_works.power = false;

    let mut by_hand = WorkOrder::begin(41, plan, 1, 0, 1);
    assert_eq!(
        run(&mut by_hand, &book, &cat, &dark_bench, a_good_hand()),
        Halt::Done
    );

    let mut by_works = WorkOrder::begin(41, plan, 1, 0, 1);
    let why = run(&mut by_works, &book, &cat, &dark_works, a_good_hand());
    assert_eq!(
        why,
        Halt::NoPower,
        "the works ran its bandsaw off the mains it did not have"
    );
}

/// **Gate: an interruption leaves the intermediates where they are.**
///
/// The blanks and the offcuts exist. Taking the whole bill of materials up
/// front is what destroys them.
#[test]
fn a_stoppage_does_not_swallow_the_work_so_far() {
    let (cat, book) = world();
    let plan = book.must("chair, hand tools");

    // A bench with no clamps: the joiner can cut and drill and then has
    // nothing to hold a glued joint with.
    let no_clamps: Vec<_> = hand_tools(&cat)
        .into_iter()
        .filter(|p| p.capability != Capability::Glue)
        .collect();
    let place = Workplace::a_workshop(no_clamps);

    let mut o = WorkOrder::begin(51, plan, 1, 0, 1);
    let why = run(&mut o, &book, &cat, &place, a_good_hand());
    assert_eq!(why, Halt::NoTool(Capability::Glue));

    // The board was cut, so the parts and the sawdust are real.
    let parts = cat.must("chair parts");
    assert!(
        o.intermediates.iter().any(|&(d, n)| d == parts && n >= 1.0),
        "the cut parts vanished when the work stopped"
    );
    assert!(
        o.waste
            .iter()
            .any(|&(m, kg)| m == Material::Oak && kg > 1.0),
        "the offcuts vanished too"
    );
    // And the glue and the screws were never touched, because that step
    // never began.
    assert!(!o
        .consumed_items
        .iter()
        .any(|&(d, n)| d == cat.must("wood glue") && n > 0.0));
}

// =====================================================================
// what can go wrong
// =====================================================================

/// **Gate: a reload cannot reroll a failure.**
///
/// The outcome is keyed to the order, the step and the attempt, so the
/// same work goes the same way whether it is run once or saved half way
/// and picked up in the morning.
#[test]
fn a_botched_weld_stays_botched_across_a_reload() {
    let (cat, book) = world();
    let plan = book.must("chair, hand tools");
    let place = Workplace::a_workshop(hand_tools(&cat));
    // Somebody clumsy, so that something actually goes wrong.
    let clumsy = Maker {
        skill: 0.15,
        proficiency: 0.1,
        knows_recipe: true,
        tool_familiarity: 0.2,
        focus: 0.3,
        fatigue: 0.7,
    };

    let mut once = WorkOrder::begin(97, plan, 1, 0, 1);
    run(&mut once, &book, &cat, &place, clumsy);

    // The same order run again from scratch.
    let mut twice = WorkOrder::begin(97, plan, 1, 0, 1);
    run(&mut twice, &book, &cat, &place, clumsy);
    assert_eq!(
        once.completed, twice.completed,
        "the same work went two different ways"
    );

    // And the same order saved part way through and resumed.
    let mut part = WorkOrder::begin(97, plan, 1, 0, 1);
    part.advance(60.0, &book, &cat, &place, clumsy);
    let reloaded = part.clone(); // what a save and a load amount to
    let mut a = part;
    let mut b = reloaded;
    run(&mut a, &book, &cat, &place, clumsy);
    run(&mut b, &book, &cat, &place, clumsy);
    assert_eq!(a.completed, b.completed, "reloading changed what happened");
    assert_eq!(
        a.completed, once.completed,
        "resuming differed from running straight through"
    );
}

/// **A mishap happens to an operation, not to the object.** A clumsy
/// worker produces a worse chair, not the absence of a chair — which is
/// the difference between a simulation and a slot machine.
///
/// Measured over two hundred chairs, because **one chair is one draw** and
/// a single comparison says nothing: at a 96% first-pass yield the good
/// hand and the poor one will often produce the identical run of grades by
/// coincidence, and a test that reads a mechanism off one sample is the
/// mistake this project has already had to unlearn three times.
#[test]
fn a_poor_hand_makes_a_poor_chair_and_not_no_chair() {
    let (cat, book) = world();
    let plan = book.must("chair, hand tools");
    let place = Workplace::a_workshop(hand_tools(&cat));

    let good = a_good_hand();
    let poor = Maker {
        skill: 0.12,
        proficiency: 0.1,
        knows_recipe: true,
        tool_familiarity: 0.2,
        focus: 0.25,
        fatigue: 0.8,
    };

    let over_a_batch = |m: Maker| {
        let (mut quality, mut made, mut accepted, mut reworked, mut scrapped) =
            (0.0f64, 0.0f64, 0u32, 0u32, 0u32);
        for id in 0..200u64 {
            let mut o = WorkOrder::begin(id, plan, 1, 0, 1);
            run(&mut o, &book, &cat, &place, m);
            for r in &o.completed {
                match r.grade {
                    Grade::Accepted => accepted += 1,
                    Grade::Reworkable => reworked += 1,
                    Grade::Scrapped => scrapped += 1,
                }
            }
            if let Some(c) = o.deliver(&book, &cat, 1) {
                quality += c.quality.overall();
                made += 1.0;
            }
        }
        (quality / made.max(1.0), made, accepted, reworked, scrapped)
    };

    let (fine, made_fine, acc_g, rew_g, scr_g) = over_a_batch(good);
    let (rough, made_rough, acc_p, rew_p, scr_p) = over_a_batch(poor);

    assert!(
        made_fine > 190.0,
        "a skilled joiner finished only {made_fine} of 200 chairs"
    );
    assert!(
        made_rough > 100.0,
        "the novice finished almost nothing: {made_rough}"
    );
    assert!(
        fine > rough + 0.05,
        "workmanship came out the same: {fine:.3} against {rough:.3}"
    );

    // **Three separate rates**, and the novice is worse on all of them.
    let rate = |n: u32, a: u32, r: u32, s: u32| n as f64 / (a + r + s) as f64;
    assert!(rate(acc_g, acc_g, rew_g, scr_g) > rate(acc_p, acc_p, rew_p, scr_p));
    assert!(rate(scr_p, acc_p, rew_p, scr_p) > rate(scr_g, acc_g, rew_g, scr_g));
    // And the shop that makes more mistakes also scraps a larger share of
    // them, because catching a fault while it can still be put right is
    // itself a thing a good shop does.
    assert!(
        scr_p as f64 / (rew_p + scr_p) as f64 > scr_g as f64 / (rew_g + scr_g) as f64,
        "the bad shop reworked as large a share of its defects as the good one"
    );
}

/// **Gate: first-pass yield, rework and scrap are three different
/// numbers.**
///
/// ASQ defines first-pass yield as passing *without correction or rework*;
/// NIST lists yield, scrap ratio and rework ratio separately. A process at
/// FPY 92 / rework 7 / scrap 1 is perfectly ordinary — so a scrap rate of
/// 1-5% implies nothing at all about first-pass yield, and this file used
/// to say it did.
#[test]
fn a_scrap_rate_does_not_imply_a_first_pass_yield() {
    let ordinary = ProcessCapability {
        baseline: 0.0,
        worker: 0.0,
        tool: 0.0,
        workplace: 0.0,
        material: 0.0,
        difficulty: 0.0,
    };
    let y = ordinary.yields();

    // They are three fields, and they sum to one because every unit goes
    // somewhere.
    assert!((y.first_pass + y.rework + y.scrap - 1.0).abs() < 1e-9);
    assert!(
        y.scrap < y.rework,
        "an ordinary shop scrapped more than it put right"
    );

    // A scrap rate inside the real 1-5% band sits alongside a first-pass
    // yield well under 99%: the two are simply not the same measurement.
    assert!(
        (0.005..=0.05).contains(&y.scrap),
        "scrap came to {:.3}",
        y.scrap
    );
    assert!(y.first_pass < 0.99, "an ordinary shop was world class");
    assert!(y.first_pass > 0.90);
    assert!(
        y.defect_rate() > y.scrap * 1.5,
        "every defect was scrapped, so rework does not exist"
    );

    // A better process improves all three, and a worse one is worse on all
    // three — but never by the same factor.
    let good = ProcessCapability {
        worker: 0.4,
        tool: 0.2,
        workplace: 0.25,
        ..ordinary
    };
    let bad = ProcessCapability {
        worker: -0.4,
        tool: -0.2,
        difficulty: 0.5,
        ..ordinary
    };
    assert!(good.yields().first_pass > y.first_pass);
    assert!(bad.yields().first_pass < y.first_pass);
    assert!(bad.yields().scrap > good.yields().scrap * 10.0);
}

/// **Gate: yields compound down a sequence, and that is not the bug.**
///
/// `RTY = prod(FPY_i)`, so twenty operations at 99% each deliver 81.8% of
/// units clean through the line. The bug was multiplying four uncalibrated
/// modifiers against whole-craft success; this is the arithmetic that was
/// hiding behind it, and a long plan really is harder to get right first
/// time than a short one.
#[test]
fn a_long_plan_is_harder_to_get_right_than_a_short_one() {
    assert!((rolled_throughput_yield(0.99, 20) - 0.8179).abs() < 1e-3);
    assert!((rolled_throughput_yield(0.99, 1) - 0.99).abs() < 1e-9);

    let (cat, book) = world();
    let place = Workplace::a_workshop(hand_tools(&cat));
    let chair = book.get(book.must("chair, hand tools")).unwrap();
    let cap = capability_of(chair, &chair.steps[1], &[], &place, a_good_hand());
    let fpy = cap.yields().first_pass;

    let over_all_six = rolled_throughput_yield(fpy, chair.steps.len());
    assert!(over_all_six < fpy, "six operations were as easy as one");
    assert!(
        over_all_six > 0.5,
        "a skilled joiner botched half his chairs: {over_all_six:.2}"
    );
}

/// **Gate: the influences combine, then the outcome is calculated once.**
///
/// A poor tool in a poor hand is additively poor. Multiplying the two
/// against the chance of success is what had a skilled joiner spoiling
/// more than half his work.
#[test]
fn a_bad_tool_and_a_bad_hand_are_additively_bad() {
    let base = ProcessCapability {
        baseline: 0.0,
        worker: 0.0,
        tool: 0.0,
        workplace: 0.0,
        material: 0.0,
        difficulty: 0.0,
    };
    let bad_hand = ProcessCapability {
        worker: -0.3,
        ..base
    };
    let bad_tool = ProcessCapability { tool: -0.3, ..base };
    let both = ProcessCapability {
        worker: -0.3,
        tool: -0.3,
        ..base
    };

    assert!((both.effective() - (bad_hand.effective() + bad_tool.effective())).abs() < 1e-9);

    // And the defect rate that comes out of it is bounded rather than
    // compounding away: two poor contributions do not make the work
    // impossible.
    assert!(
        both.yields().first_pass > 0.6,
        "two setbacks made the job unperformable"
    );
    assert!(both.yields().first_pass < bad_hand.yields().first_pass);

    // A single mapping, so the same total gives the same rates however it
    // was arrived at.
    let elsewhere = ProcessCapability {
        workplace: -0.6,
        ..base
    };
    assert_eq!(elsewhere.yields(), both.yields());
}

/// **Taking longer is not a defect.** A job can run over and come out
/// perfect, so the schedule draw is independent of the quality one.
#[test]
fn a_job_can_be_slow_and_still_be_right() {
    let (cat, book) = world();
    let place = Workplace::a_workshop(hand_tools(&cat));
    let plan = book.must("chair, hand tools");

    let mut slow_but_accepted = 0;
    for id in 0..300u64 {
        let mut o = WorkOrder::begin(id, plan, 1, 0, 1);
        run(&mut o, &book, &cat, &place, a_good_hand());
        slow_but_accepted += o
            .completed
            .iter()
            .filter(|r| r.grade == Grade::Accepted && !r.mishap.is_none())
            .count();
    }
    assert!(
        slow_but_accepted > 0,
        "nothing that passed inspection had ever taken a minute longer than planned"
    );
}

/// **Handloading is hazardous work**, and a failure there is not a quietly
/// consumed component. Real primer and propellant work injures people.
#[test]
fn a_mistake_over_primers_is_not_a_wasted_component() {
    let (cat, book) = world();
    let plan = book.must("cartridge, handloaded");
    let place = Workplace::a_workshop(hand_tools(&cat));
    let careless = Maker {
        skill: 0.1,
        proficiency: 0.05,
        knows_recipe: true,
        tool_familiarity: 0.1,
        focus: 0.2,
        fatigue: 0.9,
    };

    let mut hurt = 0;
    for id in 0..400u64 {
        let mut o = WorkOrder::begin(id, plan, 1, 0, 1);
        run(&mut o, &book, &cat, &place, careless);
        if o.completed.iter().any(|r| {
            matches!(
                r.mishap,
                scale_sim::craft::Mishap::Injury | scale_sim::craft::Mishap::Fire
            )
        }) {
            hurt += 1;
        }
    }
    assert!(
        hurt > 0,
        "four hundred careless handloading sessions hurt nobody"
    );

    // And the same person doing something with no hazard in it is never
    // hurt by it, however badly it goes.
    let sewing = book.must("work trousers");
    let mut sewn_hurt = 0;
    for id in 1000..1400u64 {
        let mut o = WorkOrder::begin(id, sewing, 1, 0, 1);
        run(&mut o, &book, &cat, &place, careless);
        if o.completed.iter().any(|r| {
            matches!(
                r.mishap,
                scale_sim::craft::Mishap::Injury | scale_sim::craft::Mishap::Fire
            )
        }) {
            sewn_hurt += 1;
        }
    }
    assert_eq!(sewn_hurt, 0, "somebody set fire to a pair of trousers");
}

// =====================================================================
// what was actually used
// =====================================================================

/// **Gate: a substitution is recorded and is what was actually used.**
///
/// The classic transmutation exploit: a recipe accepts oak or
/// particleboard and the finished item says oak. Here the record says what
/// went in, measured out by mass rather than counted one for one.
#[test]
fn what_went_in_is_what_the_record_says() {
    let (cat, book) = world();
    let plan = book.must("chair, hand tools");
    let place = Workplace::a_workshop(hand_tools(&cat));
    let oak = cat.must("oak board");
    let cheap = cat.must("particleboard sheet");

    let mut proper = WorkOrder::begin(71, plan, 1, 0, 1);
    run(&mut proper, &book, &cat, &place, a_good_hand());
    let good_chair = proper.deliver(&book, &cat, 1).unwrap();

    let mut bodged = WorkOrder::begin(71, plan, 1, 0, 1);
    bodged
        .substituted(oak, cheap, &cat, &book)
        .expect("a sheet would not do");
    run(&mut bodged, &book, &cat, &place, a_good_hand());
    let cheap_chair = bodged.deliver(&book, &cat, 1).unwrap();

    assert_eq!(good_chair.materials.chiefly(), Some(Material::Oak));
    assert_eq!(
        cheap_chair.materials.chiefly(),
        Some(Material::Particleboard),
        "a chair made of particleboard came out as oak"
    );
    let rec = cheap_chair.assembly.as_ref().unwrap();
    assert_eq!(rec.substitutions.len(), 1);
    assert_eq!(rec.substitutions[0].wanted, oak);
    assert_eq!(rec.substitutions[0].used, cheap);

    // **An intermediate cannot launder a substitution.** Both chairs are
    // built out of a component the plan calls `chair parts`; what tells
    // them apart is what those parts are made of, which the record keeps
    // per component rather than taking from the definition.
    let parts = cat.must("chair parts");
    let made_of = |c: &scale_sim::item::ItemInstance| {
        c.assembly
            .as_ref()
            .unwrap()
            .components
            .iter()
            .find(|x| x.definition == parts)
            .expect("the chair had no parts in it")
            .materials
            .chiefly()
    };
    assert_eq!(made_of(&good_chair), Some(Material::Oak));
    assert_eq!(
        made_of(&cheap_chair),
        Some(Material::Particleboard),
        "the definition said oak and the record believed it"
    );

    // **And the board is not in the chair.** A record that listed the
    // stock rather than the parts would hand back the offcuts to anybody
    // who took the chair apart.
    for c in [&good_chair, &cheap_chair] {
        let r = c.assembly.as_ref().unwrap();
        assert!(
            !r.components
                .iter()
                .any(|x| x.definition == oak || x.definition == cheap),
            "the raw board was listed as part of the chair"
        );
        assert!(
            (r.total_component_mass() - c.mass_kg).abs() < 0.02,
            "the record adds up to {:.3} kg and the chair weighs {:.3}",
            r.total_component_mass(),
            c.mass_kg
        );
    }

    // **Measured out by mass**: a chair does not swallow a whole
    // two-and-a-half metre sheet, and the two chairs weigh the same.
    assert!(
        (good_chair.mass_kg - cheap_chair.mass_kg).abs() < 0.05,
        "the cheap chair weighed {:.2} against {:.2}",
        cheap_chair.mass_kg,
        good_chair.mass_kg
    );
}

/// The record keeps how things were held together, which is what a
/// disassembly reads instead of guessing from the recipe.
#[test]
fn the_record_says_how_it_was_held_together() {
    let (cat, book) = world();
    let place = Workplace::a_workshop(hand_tools(&cat));
    let mut o = WorkOrder::begin(81, book.must("chair, hand tools"), 1, 0, 1);
    run(&mut o, &book, &cat, &place, a_good_hand());
    let chair = o.deliver(&book, &cat, 1).unwrap();
    let rec = chair.assembly.as_ref().unwrap();

    assert!(
        rec.joints.iter().any(|j| j.method == JointMethod::Glued),
        "nothing was glued"
    );
    assert!(rec.work_order == Some(81));
    // Glue went in as a material, not as a bottle.
    assert!(
        rec.components
            .iter()
            .any(|c| c.definition == cat.must("wood glue"))
            || rec.consumed.iter().any(|c| c.0 == Material::Adhesive)
    );

    // A pair of trousers is stitched, and knows it.
    let mut t = WorkOrder::begin(82, book.must("work trousers"), 1, 0, 1);
    run(&mut t, &book, &cat, &place, a_good_hand());
    let trousers = t.deliver(&book, &cat, 1).unwrap();
    assert!(trousers
        .assembly
        .as_ref()
        .unwrap()
        .joints
        .iter()
        .any(|j| j.method == JointMethod::Stitched));
}

/// **A run balances**, whatever went wrong during it.
#[test]
fn every_run_accounts_for_every_kilogram() {
    let (cat, book) = world();
    let place = Workplace::a_workshop(hand_tools(&cat));
    for (id, name) in [
        (1u64, "chair, hand tools"),
        (2, "loaf"),
        (3, "work trousers"),
        (4, "cartridge, handloaded"),
    ] {
        let mut o = WorkOrder::begin(id, book.must(name), 1, 0, 1);
        run(&mut o, &book, &cat, &place, a_good_hand());
        let b = o.balance(&book, &cat);
        assert!(
            b.closes(1e-9),
            "{name}: {:.6} kg unaccounted for",
            b.residual()
        );
        assert!(b.product_kg > 0.0, "{name} produced nothing");
        assert!(b.inputs_kg >= b.product_kg, "{name} made more than it took");
    }
}

/// A step that produces an object leaves one; a step that only advances
/// the workpiece leaves nothing. Dough can go off and be stranded; a
/// drilled joint is not a thing you can put on a shelf.
#[test]
fn only_states_worth_having_become_objects() {
    let (cat, book) = world();
    let chair = book.get(book.must("chair, hand tools")).unwrap();
    let leaves: Vec<_> = chair.steps.iter().filter(|s| s.leaves.is_some()).collect();
    assert_eq!(
        leaves.len(),
        1,
        "every step in the chair left an object behind"
    );
    assert_eq!(leaves[0].name, "cut members");

    let loaf = book.get(book.must("loaf")).unwrap();
    let named: Vec<_> = loaf.steps.iter().filter_map(|s| s.leaves).collect();
    assert!(named.contains(&cat.must("dough")));
    assert!(named.contains(&cat.must("risen dough")));

    // And the resting step is the one nobody attends.
    let rest = loaf
        .steps
        .iter()
        .find(|s| matches!(s.effort, Effort::Unattended { .. }))
        .unwrap();
    assert_eq!(rest.name, "prove");
    assert_eq!(rest.effort.labour_minutes(), 0.0);
}

// =====================================================================
// where the finished thing goes
// =====================================================================

/// **Gate: completion cannot create an item without somewhere to put it.**
///
/// An order is not over when the last operation is; it is over when the
/// thing it made is somewhere. "The current ground" is a guess, and a
/// wardrobe nobody can carry has to be put somewhere in particular.
#[test]
fn a_finished_order_needs_a_destination() {
    use scale_sim::craft::Blocked;
    use scale_sim::item::{ItemInstance, Placement, Store};

    let (cat, book) = world();
    let place = Workplace::a_workshop(hand_tools(&cat));
    let plan = book.must("chair, hand tools");
    let mut store = Store::new();

    // Onto the floor: always fine.
    let mut onto_the_floor = WorkOrder::begin_for(1, plan, 1, 0, 1, Placement::anywhere());
    run(&mut onto_the_floor, &book, &cat, &place, a_good_hand());
    let id = onto_the_floor
        .deliver_into(&book, &cat, 1, &mut store)
        .expect("a chair would not go on the floor");
    assert!(matches!(
        store.placement(id),
        Some(Placement::Ground { .. })
    ));
    assert!(onto_the_floor.delivered);
    // And it is handed over exactly once.
    assert_eq!(
        onto_the_floor.deliver_into(&book, &cat, 1, &mut store),
        Err(Blocked::AlreadyDelivered)
    );

    // **Into somebody's hands: a five-kilogram chair is fine and a
    // seventieth is not.** Real carrying capacity is about 35 kg.
    let mut carried = WorkOrder::begin_for(2, plan, 1, 0, 1, Placement::Carried { person: 7 });
    run(&mut carried, &book, &cat, &place, a_good_hand());
    assert!(carried.deliver_into(&book, &cat, 1, &mut store).is_ok());

    let mut too_heavy = WorkOrder::begin_for(
        3,
        book.must("chair, hand tools"),
        1,
        0,
        1,
        Placement::Carried { person: 7 },
    );
    run(&mut too_heavy, &book, &cat, &place, a_good_hand());
    // Fill the poor man up first.
    for k in 0..7 {
        let mut heavy = ItemInstance::one(&cat, cat.must("washing machine"));
        heavy.mass_kg = 5.0;
        let _ = k;
        store.add(heavy, Placement::Carried { person: 7 });
    }
    // **The thing exists, and it is sitting on the bench.** "Completion
    // waits" is only honest if the finished object is real: the inputs
    // are consumed, the quality is settled, and the bench it is on is
    // still occupied by it.
    let parked = match too_heavy.deliver_into(&book, &cat, 1, &mut store) {
        Err(Blocked::NoRoom { parked }) => parked,
        other => panic!("a man already carrying his limit took a chair as well: {other:?}"),
    };
    assert!(
        store.get(parked).is_some(),
        "the chair that could not be handed over vanished"
    );
    assert_eq!(
        store.placement(parked).and_then(|p| p.occupying()),
        Some(0),
        "the finished chair is not holding up the bench it is sitting on"
    );
    assert!(
        too_heavy.delivered,
        "the work was not done, though the chair exists"
    );
    // And it cannot be rerolled by choosing somewhere else: the same
    // object moves.
    let was = store.get(parked).unwrap().quality.overall();
    too_heavy
        .unload(&mut store, &cat, parked, Placement::anywhere())
        .expect("it would not go on the floor either");
    assert!(matches!(
        store.placement(parked),
        Some(Placement::Ground { .. })
    ));
    assert_eq!(
        store.get(parked).unwrap().quality.overall(),
        was,
        "moving it rerolled it"
    );

    // **A mount is not a destination.** You do not finish a chair into a
    // bracket.
    let mut nonsense = WorkOrder::begin_for(
        4,
        plan,
        1,
        0,
        1,
        Placement::Installed {
            host: scale_sim::item::Host::Vehicle(1),
            mount: 0,
        },
    );
    run(&mut nonsense, &book, &cat, &place, a_good_hand());
    assert!(matches!(
        nonsense.deliver_into(&book, &cat, 1, &mut store),
        Err(Blocked::NoRoom { .. })
    ));

    // An order that has not finished has nothing to hand over.
    let mut unstarted = WorkOrder::begin(5, plan, 1, 0, 1);
    assert_eq!(
        unstarted.deliver_into(&book, &cat, 1, &mut store),
        Err(Blocked::NotFinished)
    );
}
