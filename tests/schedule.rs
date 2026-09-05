//! **A calendar, not a second crafting model.**
//!
//! `craft.rs` already knows that a loaf is 35 minutes of labour, 35 of
//! oven and 125 on the clock. These gates are about the thing it cannot
//! say: whether the baker can shape the next batch while the first one
//! proves, whether the oven is free, and what happens to a tray that is in
//! it when the power goes off.

use scale_sim::craft::{
    hand_tools, standard_recipes, Grade, Maker, ProcessCapability, RecipeBook, WorkOrder,
    Workplace,
};
use scale_sim::item::{standard_catalogue, Capability, Catalogue, ItemInstance, Placement, Store};
use scale_sim::schedule::{
    advance_to, batch_cost, batch_outcome, book, interrupt, labour_spent_by, machine_failed,
    station, worker_leaves, Booked, OnInterruption, Shop, Station, Unschedulable, Worker,
};

fn world() -> (Catalogue, RecipeBook) {
    let cat = standard_catalogue();
    let book = standard_recipes(&cat);
    (cat, book)
}

fn a_good_hand() -> Maker {
    Maker { skill: 0.85, proficiency: 0.8, knows_recipe: true, tool_familiarity: 0.9,
            focus: 0.9, fatigue: 0.1 }
}

/// A one-man joinery: every hand tool the chair plan asks for, one of each.
fn a_bench(cat: &Catalogue) -> Shop {
    let stations = ["handsaw", "hand drill", "hammer", "screwdriver", "workbench",
                    "clamps", "sanding block", "oven", "scissors", "needle and thread"]
        .iter()
        .filter_map(|n| station(cat, n))
        .collect();
    Shop::new(1, vec![Worker::new(1, a_good_hand())], stations)
}

fn an_order(book: &RecipeBook, id: u64, name: &str) -> WorkOrder {
    WorkOrder::begin(id, book.must(name), 1, 0, 1)
}

// =====================================================================
// 1-5: what a calendar is for
// =====================================================================

/// **Gate 1: labour, machine occupancy and elapsed time stay separate**
/// once they are on a calendar, which is where they are easiest to
/// conflate.
#[test]
fn the_three_kinds_of_time_survive_being_scheduled() {
    let (cat, recipes) = world();
    let mut shop = a_bench(&cat);
    let o = an_order(&recipes, 1, "loaf");
    let plan = book(&mut shop, &o, &recipes, 0.0, 1).expect("a loaf would not schedule");

    assert!(plan.labour_min() > 0.0);
    assert!(plan.machine_min() > 0.0, "the oven was not occupied at all");
    assert!(
        plan.elapsed_min() > plan.labour_min() * 2.0,
        "labour {:.0} against {:.0} on the clock",
        plan.labour_min(),
        plan.elapsed_min()
    );
    // The proving step costs nobody anything.
    let idle = plan.slots.iter().find(|s| s.labour_min == 0.0 && s.machine_min == 0.0);
    assert!(idle.is_some(), "no step in a loaf was unattended");
    assert!(idle.unwrap().elapsed() >= 59.0, "the prove was not an hour");
}

/// **Gate 2: a worker cannot be in two places at once.**
#[test]
fn one_pair_of_hands_does_one_thing_at_a_time() {
    let (cat, recipes) = world();
    let mut shop = a_bench(&cat);
    let a = an_order(&recipes, 1, "loaf");
    let b = an_order(&recipes, 2, "loaf");
    let first = book(&mut shop, &a, &recipes, 0.0, 1).unwrap();
    let second = book(&mut shop, &b, &recipes, 0.0, 1).unwrap();

    for s in first.slots.iter().chain(&second.slots) {
        let Some(w) = s.worker else { continue };
        let others: Vec<_> = first
            .slots
            .iter()
            .chain(&second.slots)
            .filter(|o| o.worker == Some(w) && !std::ptr::eq(*o, s))
            .filter(|o| o.start < s.end - 1e-9 && s.start < o.end - 1e-9)
            .collect();
        assert!(
            others.is_empty(),
            "worker {w} is on {} things at {:.1}",
            others.len() + 1,
            s.start
        );
    }
    assert_eq!(shop.calendar.load(Booked::Worker(0), 0.0, 1e9) as usize,
               first.slots.iter().filter(|s| s.worker.is_some()).count()
                   + second.slots.iter().filter(|s| s.worker.is_some()).count());
}

/// **Gate 3: the baker is free while the dough proves**, which is the
/// whole reason to keep the three kinds of time apart.
#[test]
fn nobody_is_booked_to_watch_dough_rise() {
    let (cat, recipes) = world();
    let mut shop = a_bench(&cat);
    let a = an_order(&recipes, 1, "loaf");
    let first = book(&mut shop, &a, &recipes, 0.0, 1).unwrap();

    let prove = first
        .slots
        .iter()
        .find(|s| s.labour_min == 0.0 && s.machine_min == 0.0 && s.elapsed() > 30.0)
        .expect("no proving step");
    assert_eq!(prove.worker, None, "somebody was rostered to watch it rise");

    // And a second loaf can therefore be started inside that hour.
    let b = an_order(&recipes, 2, "loaf");
    let second = book(&mut shop, &b, &recipes, 0.0, 1).unwrap();
    let overlaps = second
        .slots
        .iter()
        .any(|s| s.start < prove.end - 1e-9 && prove.start < s.end - 1e-9);
    assert!(overlaps, "the second loaf waited for the first to finish proving");
}

/// **Gate 4: two jobs cannot have the same bench.**
#[test]
fn a_bench_is_not_double_booked() {
    let (cat, recipes) = world();
    let mut shop = a_bench(&cat);
    let mut all = Vec::new();
    for id in 1..=4u64 {
        let o = an_order(&recipes, id, "chair, hand tools");
        all.extend(book(&mut shop, &o, &recipes, 0.0, 1).unwrap().slots);
    }
    for s in &all {
        let Some(k) = s.station else { continue };
        let at_once = all
            .iter()
            .filter(|o| o.station == Some(k))
            .filter(|o| o.start < s.end - 1e-9 && s.start < o.end - 1e-9)
            .count();
        assert!(
            at_once as u32 <= shop.stations[k].capacity,
            "{} jobs on {} (capacity {}) at {:.1}",
            at_once,
            shop.stations[k].name,
            shop.stations[k].capacity,
            s.start
        );
    }
}

/// **Gate 5: an oven that takes four trays takes four trays.** Capacity is
/// what separates a bandsaw from a proving rack, and a model with only
/// "in use" cannot say it.
#[test]
fn four_trays_go_in_the_same_oven() {
    let cat = standard_catalogue();
    let recipes = standard_recipes(&cat);
    let mut stations: Vec<Station> = ["workbench", "clamps"]
        .iter()
        .filter_map(|n| station(&cat, n))
        .collect();
    stations.push(station(&cat, "oven").unwrap().holding(4));
    // Four bakers, so hands are not the constraint.
    let workers = (1..=4).map(|p| Worker::new(p, a_good_hand())).collect();
    let mut shop = Shop::new(1, workers, stations);

    let mut bakes = Vec::new();
    for id in 1..=4u64 {
        let o = an_order(&recipes, id, "loaf");
        let plan = book(&mut shop, &o, &recipes, 0.0, 1).unwrap();
        bakes.push(plan.slots.iter().find(|s| s.machine_min > 0.0).copied().unwrap());
    }
    // All four are in the oven together.
    let earliest = bakes.iter().map(|b| b.start).fold(f64::MAX, f64::min);
    let latest_start = bakes.iter().map(|b| b.start).fold(0.0f64, f64::max);
    let first_out = bakes.iter().map(|b| b.end).fold(f64::MAX, f64::min);
    assert!(
        latest_start < first_out - 1e-9,
        "the fourth tray went in at {latest_start:.0} and the first came out at {first_out:.0}"
    );
    let _ = earliest;

    // A fifth has to wait for a shelf.
    let o = an_order(&recipes, 5, "loaf");
    let fifth = book(&mut shop, &o, &recipes, 0.0, 1).unwrap();
    let fifth_bake = fifth.slots.iter().find(|s| s.machine_min > 0.0).unwrap();
    assert!(
        fifth_bake.start >= first_out - 1e-9,
        "a fifth tray went into a four-shelf oven"
    );
}

/// **Gate 6: nothing teleports between sites.** A shop that has no
/// bandsaw cannot borrow the one in the next town, and the two calendars
/// are separate.
#[test]
fn a_shop_can_only_use_what_is_in_it() {
    let (cat, recipes) = world();
    // A shop with a bench and no saw.
    let mut poor = Shop::new(
        1,
        vec![Worker::new(1, a_good_hand())],
        ["workbench", "clamps", "sanding block", "hand drill", "screwdriver"]
            .iter()
            .filter_map(|n| station(&cat, n))
            .collect(),
    );
    let mut rich = a_bench(&cat);

    let o = an_order(&recipes, 1, "chair, hand tools");
    assert!(
        matches!(
            book(&mut poor, &o, &recipes, 0.0, 1),
            Err(Unschedulable::NoStation(Capability::CutWood))
        ),
        "a shop with no saw sawed a board anyway"
    );
    assert!(book(&mut rich, &o, &recipes, 0.0, 1).is_ok());

    // And booking in one leaves the other's diary empty.
    assert!(poor.calendar.bookings().is_empty());
    assert!(!rich.calendar.bookings().is_empty());
}

// =====================================================================
// 7-10: interruption, resumption and determinism
// =====================================================================

/// **Gate 7: an interruption splits execution at its exact time**, not at
/// the end of the step and not at the end of the day.
#[test]
fn the_power_goes_off_at_the_minute_it_goes_off() {
    let (cat, recipes) = world();
    let mut shop = a_bench(&cat);
    let o = an_order(&recipes, 1, "chair, hand tools");
    let mut plan = book(&mut shop, &o, &recipes, 0.0, 1).unwrap();

    // Pick a moment inside the longest step.
    let target = plan
        .slots
        .iter()
        .max_by(|a, b| a.elapsed().total_cmp(&b.elapsed()))
        .copied()
        .unwrap();
    let at = target.start + target.elapsed() * 0.4;
    let finish_before = plan.finish();

    let hit = interrupt(&mut plan, at, at + 30.0);
    assert!(!hit.is_empty(), "an outage in the middle of a step hit nothing");
    let h = hit.iter().find(|h| h.step == target.step).expect("it missed the step it was in");
    assert!(
        (h.progress_min - target.elapsed() * 0.4).abs() < 1e-6,
        "it split at {:.2} rather than at {:.2}",
        h.progress_min,
        target.elapsed() * 0.4
    );
    assert!(plan.finish() > finish_before, "a half-hour outage cost nothing");
}

/// **A blackout has no one universal result.** Curing is unaffected, a
/// saw resumes, a weld has to be redone, a kiln has to be brought back,
/// and a tray in a cooling oven goes on changing until it is spoiled.
#[test]
fn what_an_outage_costs_depends_on_the_operation() {
    let (cat, recipes) = world();
    let mut shop = a_bench(&cat);

    // The chair has a twelve-hour cure in it, which does not care.
    let o = an_order(&recipes, 1, "chair, hand tools");
    let mut plan = book(&mut shop, &o, &recipes, 0.0, 1).unwrap();
    let cure = plan
        .slots
        .iter()
        .find(|s| s.policy == OnInterruption::Unaffected)
        .copied()
        .expect("nothing in a chair is unaffected by an outage");
    let hit = interrupt(&mut plan, cure.start + 60.0, cure.start + 120.0);
    let h = hit.iter().find(|h| h.step == cure.step).unwrap();
    assert_eq!(h.lost_min, 0.0, "curing glue lost time to a power cut");
    let after = plan.slots.iter().find(|s| s.step == cure.step).unwrap();
    assert!((after.end - cure.end).abs() < 1e-9, "the cure was extended by the outage");

    // A loaf in the oven goes on changing, and long enough spoils it.
    let o = an_order(&recipes, 2, "loaf");
    let mut plan = book(&mut shop, &o, &recipes, 0.0, 1).unwrap();
    let bake = plan.slots.iter().find(|s| s.machine_min > 0.0).copied().unwrap();
    assert!(matches!(bake.policy, OnInterruption::ContinuesChanging { .. }));
    let brief = interrupt(&mut plan, bake.start + 5.0, bake.start + 15.0);
    assert!(!brief[0].spoiled, "ten minutes off ruined the bread");

    let mut plan = book(&mut shop, &an_order(&recipes, 3, "loaf"), &recipes, 0.0, 1).unwrap();
    let bake = plan.slots.iter().find(|s| s.machine_min > 0.0).copied().unwrap();
    let long = interrupt(&mut plan, bake.start + 5.0, bake.start + 300.0);
    assert!(long[0].spoiled, "five hours in a cold oven left the bread edible");
}

/// **Gate 8: resuming does not repeat completed work.**
#[test]
fn coming_back_after_a_stoppage_does_not_start_again() {
    let (cat, recipes) = world();
    let mut shop = a_bench(&cat);
    let o = an_order(&recipes, 1, "chair, hand tools");
    let mut plan = book(&mut shop, &o, &recipes, 0.0, 1).unwrap();

    // Work through the first two operations, then stop.
    let after_two = plan.slots[2].start;
    advance_to(&mut plan, after_two);
    let done_before: Vec<usize> =
        plan.slots.iter().filter(|s| s.done).map(|s| s.step).collect();
    let spent = labour_spent_by(&plan, after_two);
    assert!(!done_before.is_empty());

    interrupt(&mut plan, after_two + 1.0, after_two + 600.0);

    // What was done is still done, and it is not charged twice.
    let done_after: Vec<usize> = plan.slots.iter().filter(|s| s.done).map(|s| s.step).collect();
    assert_eq!(done_before, done_after, "finished work was un-finished by an outage");
    assert!(
        (labour_spent_by(&plan, after_two) - spent).abs() < 1e-6,
        "the same hours were charged twice"
    );
}

/// **Gate 9: a day at a time equals one jump.**
#[test]
fn advancing_by_the_day_and_advancing_in_one_step_agree() {
    let (cat, recipes) = world();
    let mut a = a_bench(&cat);
    let mut b = a_bench(&cat);
    let o = an_order(&recipes, 7, "chair, hand tools");
    let mut daily = book(&mut a, &o, &recipes, 0.0, 1).unwrap();
    let mut leap = book(&mut b, &o, &recipes, 0.0, 1).unwrap();

    let end = daily.finish() + 10.0;
    let mut t = 0.0;
    while t < end {
        t += 1440.0;
        advance_to(&mut daily, t.min(end));
    }
    advance_to(&mut leap, end);

    assert_eq!(daily.slots, leap.slots, "a day at a time gave a different answer");
    assert!((labour_spent_by(&daily, end) - labour_spent_by(&leap, end)).abs() < 1e-9);
}

/// **Gate 10: a save half way through changes nothing.**
#[test]
fn saving_in_the_middle_of_an_operation_changes_nothing() {
    let (cat, recipes) = world();
    let mut shop = a_bench(&cat);
    let o = an_order(&recipes, 11, "chair, hand tools");
    let mut plan = book(&mut shop, &o, &recipes, 0.0, 1).unwrap();

    let midway = plan.slots[1].start + plan.slots[1].elapsed() * 0.5;
    advance_to(&mut plan, midway);

    // A save and a load are a clone, as far as this can tell.
    let reloaded = plan.clone();
    let mut a = plan;
    let mut b = reloaded;
    let (ea, eb) = (a.finish(), b.finish());
    advance_to(&mut a, ea);
    advance_to(&mut b, eb);
    assert_eq!(a.slots, b.slots, "reloading changed the schedule");

    // And the craft outcomes for the same order are the same too.
    let place = Workplace::a_workshop(hand_tools(&cat));
    let mut one = WorkOrder::begin(11, recipes.must("chair, hand tools"), 1, 0, 1);
    let mut two = WorkOrder::begin(11, recipes.must("chair, hand tools"), 1, 0, 1);
    for _ in 0..400 {
        if !one.finished() {
            one.advance(37.0, &recipes, &cat, &place, a_good_hand());
        }
        if !two.finished() {
            two.advance(600.0, &recipes, &cat, &place, a_good_hand());
        }
    }
    assert_eq!(one.completed, two.completed, "the size of the step changed what happened");
}

// =====================================================================
// 11-14: whoever is holding the tool
// =====================================================================

/// **Gate 11: the player is not a special case.**
///
/// The same skills and the same equipment give the same work, whoever is
/// doing it. Ownership decides permission, accounting and access — never
/// the physics.
#[test]
fn a_player_and_an_npc_with_the_same_hands_do_the_same_work() {
    let (cat, recipes) = world();
    let hands = a_good_hand();

    let mut theirs = Shop::new(
        1,
        vec![Worker::new(500, hands)],
        a_bench(&cat).stations,
    );
    let mut mine = Shop::new(
        1,
        // The owner, who is not paid a wage. Same hands.
        vec![Worker::owner(999, hands)],
        a_bench(&cat).stations,
    );

    let o = an_order(&recipes, 1, "chair, hand tools");
    let a = book(&mut theirs, &o, &recipes, 0.0, 1).unwrap();
    let b = book(&mut mine, &o, &recipes, 0.0, 1).unwrap();

    assert_eq!(a.slots, b.slots, "the owner did the job differently from the employee");
    assert!((a.labour_min() - b.labour_min()).abs() < 1e-9);
}

/// **Gate 12: working costs the worker.** Time, tiredness and the edge on
/// the tool.
#[test]
fn an_hour_at_the_bench_costs_an_hour_of_somebody() {
    let (cat, recipes) = world();
    let mut shop = a_bench(&cat);
    let o = an_order(&recipes, 1, "chair, hand tools");
    let plan = book(&mut shop, &o, &recipes, 0.0, 1).unwrap();

    let fresh = shop.workers[0].maker.fatigue;
    let sharp = shop.stations[0].provides[0].precision_mm;

    for s in &plan.slots {
        if let Some(w) = s.worker {
            shop.worked(w, s.labour_min);
        }
        if let Some(k) = s.station {
            shop.tool_wear(k, s.elapsed());
        }
    }

    assert!(shop.workers[0].maker.fatigue > fresh, "a day at the bench tired nobody");
    assert!(
        shop.stations[0].provides[0].precision_mm >= sharp,
        "the tools came out of a day's work sharper"
    );
    // And a night off puts some of it back.
    let spent = shop.workers[0].maker.fatigue;
    shop.rested(0, 480.0);
    assert!(shop.workers[0].maker.fatigue < spent, "sleep did nothing");
}

/// **Gate 13: walking out stops the work you were doing and not the work
/// doing itself.**
#[test]
fn leaving_the_shop_stops_the_sawing_and_not_the_curing() {
    let (cat, recipes) = world();
    let mut shop = a_bench(&cat);
    let o = an_order(&recipes, 1, "chair, hand tools");
    let mut plan = book(&mut shop, &o, &recipes, 0.0, 1).unwrap();

    // In the middle of the cure, which nobody is attending.
    let cure = plan
        .slots
        .iter()
        .find(|s| s.worker.is_none() && s.elapsed() > 100.0)
        .copied()
        .unwrap();
    let (stopped, carried_on) = worker_leaves(&mut plan, 0, cure.start + 10.0);
    assert_eq!(stopped, 0, "the glue needed watching");
    assert!(carried_on >= 1, "the cure stopped because somebody went home");

    // And in the middle of a hands-on step it does stop.
    let hands_on = plan.slots.iter().find(|s| s.worker == Some(0)).copied().unwrap();
    advance_to(&mut plan, 0.0);
    let (stopped, _) = worker_leaves(&mut plan, 0, hands_on.start + 1.0);
    assert_eq!(stopped, 1, "the sawing carried on with nobody holding the saw");
}

/// **Gate 14: owner labour is hours even when it is not wages.**
#[test]
fn working_for_yourself_still_takes_the_day() {
    let (cat, recipes) = world();
    let mut shop = Shop::new(
        1,
        vec![Worker::owner(1, a_good_hand())],
        a_bench(&cat).stations,
    );
    let o = an_order(&recipes, 1, "chair, hand tools");
    let plan = book(&mut shop, &o, &recipes, 0.0, 1).unwrap();

    assert!(!shop.workers[0].paid, "the owner put himself on the payroll");
    assert!(
        plan.labour_min() > 100.0,
        "working for nothing took no time: {:.0} min",
        plan.labour_min()
    );
    // The calendar does not care who is paid.
    assert!(shop.calendar.load(Booked::Worker(0), 0.0, 1e9) > 0);
}

// =====================================================================
// 15-20: batches, rework, failure and identity
// =====================================================================

/// **Gate 15: setup once, unit work per unit** — and reported in units
/// that cannot be misread.
#[test]
fn a_batch_says_total_and_per_unit_and_never_confuses_them() {
    let (_cat, recipes) = world();
    let plan = recipes.must("chair, factory");
    let one = batch_cost(&recipes, plan, 1).unwrap();
    let five_hundred = batch_cost(&recipes, plan, 500).unwrap();

    assert_eq!(one.units, 1);
    assert!((one.total_labour_min - one.per_unit_labour_min).abs() < 1e-9);

    let four_hundred = batch_cost(&recipes, plan, 400).unwrap();
    assert!(
        five_hundred.total_labour_min > four_hundred.total_labour_min,
        "five hundred chairs took less than four hundred chairs' work"
    );
    assert!(
        five_hundred.per_unit_labour_min < one.per_unit_labour_min,
        "a run of five hundred was no cheaper per chair"
    );
    // **The saving is exactly the setup, spread** — and nothing else.
    let saved = one.per_unit_labour_min - five_hundred.per_unit_labour_min;
    assert!(
        (saved - five_hundred.setup_min * 499.0 / 500.0).abs() < 1e-6,
        "the run saved {saved:.1} minutes a chair and the setup is {:.1}",
        five_hundred.setup_min
    );
    // And per-unit never falls below what making a chair takes.
    let floor = recipes.get(plan).unwrap().labour_minutes();
    for n in [1u32, 5, 500, 50_000] {
        let c = batch_cost(&recipes, plan, n).unwrap();
        assert!(
            c.per_unit_labour_min >= floor - 1e-9,
            "{n} chairs came to {:.2} min each against a floor of {floor:.2}",
            c.per_unit_labour_min
        );
    }

    // Adding a unit never makes the total go down.
    let mut last = 0.0;
    for n in [1u32, 2, 5, 40, 200, 500] {
        let c = batch_cost(&recipes, plan, n).unwrap();
        assert!(c.total_labour_min >= last, "the total fell going to {n} units");
        last = c.total_labour_min;
    }
}

/// **Gate 16: a unit fault spoils a unit and a setup fault spoils the
/// run.** A model with one roll per batch cannot have the first; a model
/// with only per-unit rolls cannot have the second.
#[test]
fn a_jig_set_up_wrong_spoils_the_whole_run() {
    let ordinary = ProcessCapability {
        baseline: 0.0, worker: 0.0, tool: 0.0, workplace: 0.0, material: 0.0, difficulty: 0.0,
    };
    let y = ordinary.yields();

    // Over many runs, most are fine and the units within them vary.
    let (mut good_runs, mut spoiled_runs, mut mixed) = (0, 0, 0);
    for order in 0..400u64 {
        let b = batch_outcome(order, 50, y, 0.0);
        if b.setup == Grade::Accepted {
            good_runs += 1;
            if b.units.iter().any(|g| *g != Grade::Accepted)
                && b.units.iter().any(|g| *g == Grade::Accepted)
            {
                mixed += 1;
            }
        } else {
            spoiled_runs += 1;
            assert_eq!(b.accepted(), 0, "a run with a bad setup passed units anyway");
        }
    }
    assert!(good_runs > 300, "only {good_runs} of 400 setups went right");
    assert!(spoiled_runs > 0, "no setup ever went wrong in 400 runs");
    assert!(
        mixed > 200,
        "units within a run did not vary: only {mixed} runs had both good and bad"
    );
}

/// **Gate 17: rework gets its own reservations and its own time.** Doing
/// it again is not free and it is not instantaneous.
#[test]
fn a_rework_is_booked_like_any_other_operation() {
    let (cat, recipes) = world();
    let mut shop = a_bench(&cat);
    let o = an_order(&recipes, 1, "chair, hand tools");
    let plan = book(&mut shop, &o, &recipes, 0.0, 1).unwrap();
    let bookings_before = shop.calendar.bookings().len();
    let finish_before = plan.finish();

    // The drilling has to be done again: same order, same step, attempt 1.
    let redo = an_order(&recipes, 1, "chair, hand tools");
    let again = book(&mut shop, &redo, &recipes, finish_before, 1).unwrap();

    assert!(shop.calendar.bookings().len() > bookings_before, "the rework booked nothing");
    assert!(
        again.slots[0].start >= finish_before - 1e-9,
        "the rework was scheduled before the work it repeats"
    );
    assert!(again.labour_min() > 0.0, "doing it again cost nobody any time");
}

/// **Gate 19: a customer's repaired thing comes back as the same thing.**
///
/// Anything else quietly swaps somebody's property for a copy of it.
#[test]
fn a_repaired_chair_is_the_customer_s_chair() {
    let (cat, recipes) = world();
    let place = Workplace::a_workshop(hand_tools(&cat));
    let mut store = Store::new();

    let mut theirs = ItemInstance::one(&cat, cat.must("wooden chair"));
    theirs.given_name = Some("my grandmother's".into());
    theirs.quality.workmanship = 0.94;
    theirs.condition.damage = 0.55;
    let id = store.add(theirs, Placement::Carried { person: 42 });

    let mut job = WorkOrder::begin_for(
        3,
        recipes.must("chair, hand tools"),
        1,
        0,
        1,
        Placement::Carried { person: 42 },
    )
    .on(id);
    for _ in 0..400 {
        if job.finished() {
            break;
        }
        job.advance(60.0, &recipes, &cat, &place, a_good_hand());
    }

    let back = job.deliver_into(&recipes, &cat, 2, &mut store).expect("no chair came back");
    assert_eq!(back, id, "the customer was handed a different chair");
    let c = store.get(back).unwrap();
    assert_eq!(c.given_name.as_deref(), Some("my grandmother's"));
    assert_eq!(c.quality.workmanship, 0.94, "mending it improved the joinery");
    assert!(c.condition.damage < 0.55, "it came back as broken as it went in");
    assert_eq!(store.live(), 1, "a second chair appeared from somewhere");
}

/// **Gate 20: a machine failing leaves work in progress**, not a lost
/// order and certainly not two outputs.
#[test]
fn a_broken_machine_strands_work_rather_than_duplicating_it() {
    let (cat, recipes) = world();
    let mut shop = a_bench(&cat);
    let o = an_order(&recipes, 1, "chair, hand tools");
    let mut plan = book(&mut shop, &o, &recipes, 0.0, 1).unwrap();
    let steps_before = plan.slots.len();

    // The saw goes at the moment the second operation is under way.
    let saw = plan.slots[1].station.unwrap();
    let at = plan.slots[1].start + 1.0;
    let stranded = machine_failed(&mut shop, &mut plan, saw, at);

    assert!(!stranded.is_empty(), "breaking the saw stranded nothing");
    // Everything finished is still finished, and nothing is duplicated.
    let total = plan.slots.len() + stranded.len();
    assert_eq!(total, steps_before, "the failure created or lost operations");
    assert!(plan.slots.iter().all(|s| s.done || s.station != Some(saw)));
    // And its bookings are given up so somebody else can have the shop.
    assert!(
        shop.calendar.bookings().iter().all(|b| b.from < at + 1e-9),
        "a broken machine kept its diary"
    );
}

/// One scheduler for a lone survivor, an owner-operator, a workshop and a
/// works: what changes is the providers, not the physics.
#[test]
fn the_same_plan_runs_in_a_shed_and_in_a_works() {
    let (cat, recipes) = world();
    let mut shed = a_bench(&cat);
    let mut works = Shop::new(
        2,
        (1..=6).map(|p| Worker::new(p, a_good_hand())).collect(),
        a_bench(&cat).stations,
    );
    works.jigs = 0.85;

    let o = an_order(&recipes, 1, "chair, hand tools");
    let a = book(&mut shed, &o, &recipes, 0.0, 4).unwrap();
    let b = book(&mut works, &o, &recipes, 0.0, 4).unwrap();

    // Same operations, same labour per chair.
    assert_eq!(a.slots.len(), b.slots.len(), "the works ran a different plan");
    assert!((a.labour_min() - b.labour_min()).abs() < 1e-6, "the works saved labour by magic");
    // What scale buys is that the hands are not the bottleneck.
    assert!(
        b.finish() <= a.finish() + 1e-9,
        "six hands finished four chairs later than one pair"
    );
}
