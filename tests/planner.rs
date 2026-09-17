//! **People decide what to do about their own lives**, and only when
//! something gives them a reason to.
//!
//! Spec A4, settled and until now unbuilt. Every gate here was checked by
//! deleting the mechanism it names and requiring it to go red.

use scale_sim::econ::{Doctrine, Economy};
use scale_sim::person::{live_a_day, Person, Qualification, Trade};
use scale_sim::planner::{
    take_turns, thinks_allowed, worth_to, Heard, Lead, Outcome, Planner, Step, Trigger,
    A_WEEK_OF_LOOKING, DRIFT_DAYS, LEAD_LIFE_DAYS,
};
use scale_sim::slice;

/// A country that has been running for a month, so wages and prices mean
/// something.
fn warmed() -> Economy {
    let mut e = slice::build(Doctrine::Prudent);
    for _ in 0..30 {
        e.step();
    }
    e
}

fn lead(trade: Trade, market: usize, chance: f64, day: u64) -> Lead {
    Lead {
        trade,
        market,
        chance,
        heard: day,
        expires: day + LEAD_LIFE_DAYS,
        how: Heard::WordOfMouth,
    }
}

/// **Nobody thinks again without a reason** — A4.5's four triggers and
/// nothing else.
///
/// A life going fine is reviewed on the drift timer and not before, and a
/// lead that is merely better does not interrupt it. Sabotage: raise a
/// trigger every day, and somebody in steady work is thinking again on day
/// one.
#[test]
fn a_life_going_fine_is_not_reconsidered_before_its_review() {
    let mut pl = Planner::new();
    // Offset so the first review falls on day 90.
    let offset = 89;
    for day in 0..90u64 {
        pl.observe(day, Trade::ProductionWorker, 0, Outcome::Worked, offset);
        if day == 40 {
            // Somewhat better news — not twice as good.
            pl.hear(lead(Trade::Driver, 0, 0.9, day), 1.5, 1.0);
        }
        assert_eq!(
            pl.pending, None,
            "somebody in steady work had a reason to think again on day {day}"
        );
    }
    pl.observe(90, Trade::ProductionWorker, 0, Outcome::Worked, offset);
    assert_eq!(
        pl.pending,
        Some(Trigger::Drift),
        "the quarterly review never came round"
    );

    // And once seen to, the next review is a quarter off, not tomorrow.
    pl.pending = None;
    for day in 91..(91 + DRIFT_DAYS - 1) {
        pl.observe(day, Trade::ProductionWorker, 0, Outcome::Worked, offset);
        assert_eq!(pl.pending, None, "reviewed again on day {day}");
    }
}

/// **A week of looking and finding nothing is a plan failing.**
#[test]
fn a_week_without_work_is_a_reason_to_think_again() {
    let mut pl = Planner::new();
    for day in 0..(A_WEEK_OF_LOOKING as u64 - 1) {
        pl.observe(day, Trade::Sales, 0, Outcome::Looked, 60);
        assert_eq!(pl.pending, None, "gave up on day {day}");
    }
    pl.observe(A_WEEK_OF_LOOKING as u64 - 1, Trade::Sales, 0, Outcome::Looked, 60);
    assert_eq!(pl.pending, Some(Trigger::PlanFailed));
    assert!(
        pl.expectation() < 0.5,
        "a week of no work taught them nothing: they still expect {:.2}",
        pl.expectation()
    );
}

/// **News twice as good as what they have does not wait for a review.**
#[test]
fn news_too_good_to_wait_is_acted_on() {
    let mut pl = Planner::new();
    pl.observe(0, Trade::Sales, 0, Outcome::Worked, 60);
    pl.hear(lead(Trade::ProductionWorker, 0, 0.9, 0), 2.5, 1.0);
    assert_eq!(pl.pending, Some(Trigger::Salient));
}

/// **Something new has to clearly beat what they have** — A4.3's
/// hysteresis, the thing that stops people thrashing between two options
/// that are nearly equal.
///
/// Sabotage: set `CLEARLY_BETTER` to one and the marginal lead is taken.
#[test]
fn a_new_trade_must_clearly_beat_the_old_one() {
    let e = warmed();
    let mut who = Person::new("Hal Judd", Trade::Sales, 0, 100.0);
    who.qualification = Qualification::School;

    let decide = |factor: f64| {
        let mut pl = Planner::new();
        // Three weeks of little work, so there is room above what they
        // expect for a lead to be worth half as much again.
        for day in 0..21 {
            pl.observe(day, who.trade, 0, Outcome::Looked, 60);
        }
        let now = worth_to(&who, &e, who.trade, pl.expectation());
        let per = worth_to(&who, &e, Trade::ProductionWorker, 1.0);
        let chance = factor * now / per;
        assert!(chance <= 1.0, "the fixture cannot express {factor}x");
        pl.hear(lead(Trade::ProductionWorker, 0, chance, 21), factor * now, now);
        pl.reconsider(&who, &e, 21, |_, _| true)
    };

    assert_eq!(
        decide(1.10),
        None,
        "gave up shop work for labouring that pays a tenth more — that is \
         thrashing, not deciding"
    );
    assert_eq!(
        decide(1.60),
        Some(Trade::ProductionWorker),
        "turned down labouring that is plainly worth half as much again"
    );
}

/// **Nobody goes after work they are not allowed to do**, however good.
#[test]
fn a_lead_for_work_you_cannot_do_is_not_an_option() {
    let e = warmed();
    let who = Person::new("Ida Kemp", Trade::ProductionWorker, 0, 100.0);
    // A labourer is School; office work wants a degree.
    let mut pl = Planner::new();
    pl.observe(0, who.trade, 0, Outcome::Looked, 60);
    let now = worth_to(&who, &e, who.trade, pl.expectation());
    pl.hear(lead(Trade::BusinessSpecialist, 0, 1.0, 1), 50.0 * now, now);
    assert_eq!(
        pl.reconsider(&who, &e, 1, |_, _| true),
        None,
        "a labourer set out after office work, which needs a degree"
    );
}

/// **Two people are not both promised the same post** — A4.7.
///
/// `reserve` is how the town says whether an opening is still there. One
/// opening, two people who want it: the second keeps doing what they were
/// doing.
#[test]
fn one_opening_is_not_promised_twice() {
    let e = warmed();
    let mut openings = 1;
    let mut reserve = |_: usize, _: Trade| {
        if openings > 0 {
            openings -= 1;
            true
        } else {
            false
        }
    };
    let mut set_out = 0;
    for name in ["Alma Ash", "Bert Brook"] {
        let who = Person::new(name, Trade::Sales, 0, 100.0);
        let mut pl = Planner::new();
        pl.observe(0, who.trade, 0, Outcome::Looked, 60);
        let now = worth_to(&who, &e, who.trade, pl.expectation());
        pl.hear(lead(Trade::ProductionWorker, 0, 1.0, 1), 10.0 * now, now);
        if pl.reconsider(&who, &e, 1, &mut reserve).is_some() {
            set_out += 1;
        }
    }
    assert_eq!(set_out, 1, "{set_out} people set out after one opening");
}

/// **The town holds an opening for whoever sets out after it, and only
/// while there is one** — A4.7, the rule `plan_ahead` actually uses.
///
/// Tested here rather than in a whole world because in the worlds built so
/// far openings are plentiful — half the sample are shop workers against a
/// seventh of the posts — so a hold rarely has to say no, and a whole-world
/// gate passed with the check deleted. Sabotage: let every hold succeed.
#[test]
fn an_opening_is_held_for_one_person_at_a_time() {
    use scale_sim::id::Arena;
    use scale_sim::populace::hold_an_opening;

    let mut folk: Arena<Person> = Arena::new();
    let a = folk.add(Person::new("Alma Ash", Trade::Sales, 0, 100.0));
    let b = folk.add(Person::new("Bert Brook", Trade::Sales, 0, 100.0));
    let c = folk.add(Person::new("Cora Dell", Trade::Sales, 0, 100.0));

    let mut holds = Vec::new();
    // A post and a half going: room for one.
    assert!(hold_an_opening(&mut holds, a, 0, Trade::ProductionWorker, 1.5, 10));
    assert!(
        !hold_an_opening(&mut holds, b, 0, Trade::ProductionWorker, 1.5, 10),
        "two people were sent after one post"
    );
    // A different trade, or a different town, is a different opening.
    assert!(hold_an_opening(&mut holds, b, 0, Trade::FoodService, 1.5, 10));
    assert!(hold_an_opening(&mut holds, c, 1, Trade::ProductionWorker, 1.5, 10));
    // Nothing going at all.
    assert!(!hold_an_opening(&mut holds, c, 0, Trade::BusinessSpecialist, 0.4, 10));
    // And a hold lapses with the lead it was taken on.
    assert!(holds.iter().all(|h| h.until == 10 + LEAD_LIFE_DAYS));
}

/// **A lead that comes to nothing is forgotten, and that is normal** —
/// A4.6.
///
/// Sabotage: keep the lead, and they set out after the same stale post
/// again the moment they next think.
#[test]
fn a_lead_that_came_to_nothing_is_forgotten() {
    let e = warmed();
    let who = Person::new("Cora Dell", Trade::Sales, 0, 100.0);
    let mut pl = Planner::new();
    pl.observe(0, who.trade, 0, Outcome::Looked, 60);
    let now = worth_to(&who, &e, who.trade, pl.expectation());
    pl.hear(lead(Trade::ProductionWorker, 0, 1.0, 1), 10.0 * now, now);
    assert_eq!(pl.reconsider(&who, &e, 1, |_, _| true), Some(Trade::ProductionWorker));

    // They ask, and nobody takes them on — while they go on hearing that
    // the works are hiring, so the lead itself would outlast the attempt.
    // What removes it has to be the attempt failing, not the calendar.
    let mut day = 1;
    while pl.pending.is_none() && day < 30 {
        day += 1;
        if day == 5 {
            // Heard again, and no longer news: they are already after it.
            pl.hear(lead(Trade::ProductionWorker, 0, 1.0, day), now, now);
        }
        pl.observe(day, Trade::Sales, 0, Outcome::Looked, 60);
    }
    assert!(day < 5 + LEAD_LIFE_DAYS, "the attempt outlasted the lead it was on");
    assert_eq!(pl.pending, Some(Trigger::PlanFailed), "never gave up on it");
    assert_eq!(pl.failures, 1);
    assert!(
        pl.leads().iter().all(|l| l.trade != Trade::ProductionWorker),
        "still believes in the post that was not there"
    );
    assert!(
        matches!(
            pl.plan.as_ref().and_then(|p| p.step()),
            Some(Step::Work {
                trade: Trade::Sales,
                ..
            })
        ),
        "a failed plan left them with no plan at all"
    );
    assert_eq!(pl.reconsider(&who, &e, day, |_, _| true), None);
}

/// **The think budget is a hard cap, and nobody waits for ever** — A4.8.
#[test]
fn the_think_budget_is_a_cap_and_everybody_gets_a_turn() {
    let allowed = thinks_allowed(40);
    let mut waiting: Vec<usize> = (0..20).collect();
    let mut day = 0u64;
    while !waiting.is_empty() {
        let turn = take_turns(&waiting, allowed, day);
        assert!(turn.len() <= allowed, "{} thought on one day", turn.len());
        waiting.retain(|w| !turn.contains(w));
        day += 1;
        assert!(day < 60, "somebody has been waiting to think for two months");
    }
}

/// **Somebody going after other work takes it when it is offered** — and
/// somebody who is not, does not.
///
/// The join between the plan and the day. A shop worker in a country with
/// no shops has no work at all; told the works are taking people on, they
/// go and ask. Sabotage: drop the plan from the day's choice of work and
/// they sit idle beside the one who never heard.
#[test]
fn a_plan_to_take_up_a_trade_is_carried_out() {
    let mut e = warmed();
    let mut told = Person::new("Dai Ewart", Trade::Sales, 0, 500.0);
    let mut not = Person::new("Elsie Finn", Trade::Sales, 0, 500.0);

    let mut pl = std::mem::take(&mut told.planner);
    pl.observe(30, told.trade, 0, Outcome::Looked, 60);
    let now = worth_to(&told, &e, told.trade, pl.expectation());
    pl.hear(lead(Trade::ProductionWorker, 0, 0.9, 30), 10.0 * now, now);
    assert_eq!(pl.reconsider(&told, &e, 30, |_, _| true), Some(Trade::ProductionWorker));
    told.planner = pl;

    for day in 31..(31 + LEAD_LIFE_DAYS) {
        e.step();
        live_a_day(&mut told, &mut e, day);
        live_a_day(&mut not, &mut e, day);
    }
    assert_eq!(
        told.trade,
        Trade::ProductionWorker,
        "went after labouring and never got a shift in a week"
    );
    assert_eq!(
        not.trade,
        Trade::Sales,
        "somebody who never heard of the work took it up anyway"
    );
}

/// **A town sends nobody after work it does not have.**
///
/// The world switched on and left alone for eight months, with the openings
/// and the holds on them doing the only thing that stops a planner flooding
/// one trade. The sample stands for the town, so however many people took up
/// a trade, no more of them can be in it than the town's posts give that
/// trade a share of the sample — to within the one post a count rounds to.
///
/// And it has to have happened at all: a gate where nobody ever changed
/// trade would pass on an absence. Sabotage: drop the plan from the day's
/// choice of work, and nobody takes anything up. **Letting every hold
/// succeed leaves this green** — openings in this world are plentiful, so
/// the hold rarely binds; `an_opening_is_held_for_one_person_at_a_time`
/// carries that claim instead.
#[test]
fn nobody_is_sent_after_work_the_town_does_not_have() {
    use scale_sim::game::GameState;
    use scale_sim::occupation::jobs_by_occupation;
    use scale_sim::network::Network;
    use scale_sim::polity::Polities;
    use scale_sim::populace::Populace;
    use scale_sim::region::Nations;
    use scale_sim::settlement::Settlements;
    use scale_sim::world::World;

    let world = World::generate(384, 216, 7);
    let polities = Polities::partition(&world, 24);
    let settlements = Settlements::place(&world, &polities, 3000);
    let network = Network::build(&world, &settlements, 500);
    let n = Nations::build(&world, &polities, &settlements, &network, 2, 4, Doctrine::Prudent);
    let folk = Populace::seed(&n.economy, 40, 7);
    let mut g = GameState::new(7).with_economy(n.economy).with_folk(folk);
    g.advance(240);

    let (Some(e), Some(folk)) = (g.economy.as_ref(), g.folk.as_ref()) else {
        panic!("the world lost its economy or its people");
    };
    let took_up: u64 = folk.people.values().map(|p| p.planner.taken_up).sum();
    assert!(
        took_up > 0,
        "in eight months nobody in eight towns took up a trade they were not \
         in — a gate on a flood that never had the chance to happen"
    );

    let posts = jobs_by_occupation(e);
    for m in 0..e.markets.len() {
        let floor: Vec<_> = folk
            .people
            .values()
            .filter(|p| p.alive() && p.market == m && p.trade != Trade::Supervisor)
            .collect();
        let total: f64 = Trade::ALL
            .iter()
            .filter(|&&t| t != Trade::Supervisor)
            .map(|t| posts[m][t.index()])
            .sum();
        if floor.is_empty() || total <= 0.0 {
            continue;
        }
        for t in Trade::ALL {
            if t == Trade::Supervisor {
                continue;
            }
            let share = posts[m][t.index()] / total * floor.len() as f64;
            let came = floor
                .iter()
                .filter(|p| p.trade == t && p.planner.taken_up > 0)
                .count();
            assert!(
                came as f64 <= share.ceil() + 1.0,
                "{} people took up {} in {}, whose posts give that trade {:.1} \
                 of a sample of {}",
                came,
                t.name(),
                e.markets[m].name,
                share,
                floor.len()
            );
        }
    }
}
