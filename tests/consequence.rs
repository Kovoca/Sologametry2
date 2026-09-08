//! **One world fact, travelling the whole way.**
//!
//! A transformer fails; the power goes; the mill stops; the firm decides;
//! a man is told; he appraises it, copes with it, and is changed by it;
//! it survives a save; and when somebody walks up to him afterwards, what
//! he says comes out of that.
//!
//! The labour market is read from a **real generated nation**, not
//! invented here — which is the point of the exercise.

use scale_sim::befall::befell;
use scale_sim::consequence::{
    as_world_event, learns, runway_days, runway_of, Consequences, Employer, Fact, Happened,
    Reemployment, Standing,
};
use scale_sim::converse::{ask, Approach, Asked};
use scale_sim::coping::FunctionalState;
use scale_sim::growth::ShapesWellbeing;
use scale_sim::id::{Arena, Id};
use scale_sim::labour::Workforce;
use scale_sim::memory::{Memory, PerceivedWho, Place, Source};
use scale_sim::mind::Value;
use scale_sim::person::{Person, Trade};
use scale_sim::save::{Journal, Save};
use scale_sim::scaling::{demote, promote, Coarse};

fn who(n: u32) -> Id<Person> {
    let mut a: Arena<Person> = Arena::new();
    let mut last = a.add(Person::new("x", Trade::Labourer, 0, 0.0));
    for _ in 0..n {
        last = a.add(Person::new("x", Trade::Labourer, 0, 0.0));
    }
    last
}

fn culture() -> Vec<(Value, i8)> {
    vec![(Value::Family, 25), (Value::Law, 20)]
}

/// A town where most hands are working, and one where few are.
fn a_busy_town() -> Workforce {
    Workforce {
        hands: 1000.0,
        working: 880.0,
        posts: 900.0,
        ..Default::default()
    }
}
fn a_dying_town() -> Workforce {
    Workforce {
        hands: 1000.0,
        working: 180.0,
        posts: 200.0,
        ..Default::default()
    }
}

/// A firm with a spare in store loses a few days; one without loses
/// months. **Only the outage differs** — the firm is the same firm.
fn a_mill() -> Employer {
    Employer {
        inventory_days: 12.0,
        liquidity_days: 30.0,
        retains_labour: 0.7,
        expected_repair_days: 4.0,
        alternative_power: 0.0,
        demand: 0.8,
    }
}

/// With no spare a transformer is built to order: twelve to eighteen
/// months, which is what the economy already models.
fn a_mill_with_no_spare() -> Employer {
    Employer {
        expected_repair_days: 400.0,
        ..a_mill()
    }
}

// =====================================================================
// gate 1: no false adversity
// =====================================================================

/// **With the spare installed, nothing happens to anybody.**
///
/// A four-day outage against twelve days of stock is not an event in
/// anybody's life, and a model that manufactures one has invented
/// suffering.
#[test]
fn a_spare_in_store_costs_nobody_their_work() {
    let mut seq = 0u64;
    let facts = a_mill().decides(4.0, 100, &mut seq);

    assert!(
        !facts.iter().any(|f| f.what == Fact::EmploymentEnded),
        "a four-day outage with a fortnight of stock put somebody out of work"
    );
    assert!(
        !facts
            .iter()
            .any(|f| matches!(f.what, Fact::PayReduced { .. })),
        "a short outage docked somebody's pay"
    );
    // The mill did stop, which is true and is a different claim.
    assert!(facts.iter().any(|f| f.what == Fact::WorkStopped));

    // And nothing durable reaches the man.
    let mut c = Coarse::new(who(1), 7, 0);
    let market = a_busy_town();
    let got: Vec<_> = facts.iter().map(|f| (*f, Source::Witnessed)).collect();
    let circ = Consequences::circumstance(&got, &market, 60.0, 1.0);
    assert!(!circ.lost_work);
    let mut event = 500u64;
    let changes = befell(&circ, 100, &mut event);
    assert!(
        !changes.iter().any(|ch| matches!(
            ch.what,
            scale_sim::scaling::What::Wellbeing {
                by: ShapesWellbeing::LostWork,
                ..
            }
        )),
        "a short outage left a durable well-being injury"
    );
    let me = promote(&c, &culture(), 0);
    c.advance_through(&me.mind, 0.3, &changes);
    assert!(
        c.standing.is_empty(),
        "he acquired a standing trouble out of nothing"
    );
}

// =====================================================================
// gate: the transformer does not fire anybody
// =====================================================================

/// **Infrastructure decides power; the firm decides employment.**
///
/// The same outage, at two firms, with different stock and different
/// expectations of the repair.
#[test]
fn the_firm_decides_and_not_the_grid() {
    let long = 200.0;
    let mut seq = 0u64;
    let hand_to_mouth = Employer {
        inventory_days: 1.0,
        liquidity_days: 3.0,
        expected_repair_days: 400.0,
        ..a_mill()
    };
    let well_found = Employer {
        inventory_days: 250.0,
        liquidity_days: 200.0,
        expected_repair_days: 30.0,
        ..a_mill()
    };

    let bad = hand_to_mouth.decides(long, 0, &mut seq);
    let fine = well_found.decides(long, 0, &mut seq);

    assert!(bad.iter().any(|f| f.what == Fact::EmploymentEnded));
    assert!(
        !fine.iter().any(|f| f.what == Fact::EmploymentEnded),
        "a firm with a year of stock laid people off in a stoppage"
    );

    // And a mill with its own power notices nothing much at all.
    let with_a_generator = Employer {
        alternative_power: 1.0,
        ..hand_to_mouth
    };
    assert!(with_a_generator.decides(long, 0, &mut seq).is_empty());
}

/// **The facts arrive separately, in order, on their own days.** Nobody
/// is handed the causal chain.
#[test]
fn the_news_arrives_a_piece_at_a_time() {
    let mut seq = 0u64;
    let facts = a_mill_with_no_spare().decides(200.0, 10, &mut seq);
    let kinds: Vec<Fact> = facts.iter().map(|f| f.what).collect();

    assert_eq!(kinds[0], Fact::WorkStopped);
    assert_eq!(kinds[1], Fact::ShiftCancelled);
    assert!(matches!(kinds[2], Fact::PayReduced { .. }));
    assert_eq!(*kinds.last().unwrap(), Fact::EmploymentEnded);

    // Different days, in order, and none of them on the day the
    // transformer failed except the stoppage itself.
    for pair in facts.windows(2) {
        assert!(pair[0].day <= pair[1].day, "the news arrived out of order");
    }
    assert!(
        facts.last().unwrap().day > facts[0].day + 20,
        "he was told he was finished the week it broke"
    );
    // Every fact has its own identity.
    let mut ids: Vec<u64> = facts.iter().map(|f| f.event).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), facts.len());
}

// =====================================================================
// gates 2 and 3: history first, and exactly once
// =====================================================================

/// **The objective fact is history before any mind consumes it**, and it
/// is applied exactly once however the world is stepped.
#[test]
fn one_fact_committed_before_anybody_appraises_it() {
    let mut seq = 0u64;
    let facts = a_mill_with_no_spare().decides(200.0, 10, &mut seq);
    let mut journal = Journal::new();

    let fresh = Consequences::commit(&mut journal, 4242, &facts);
    assert_eq!(fresh.len(), facts.len());
    // It is history now, before anybody has heard it.
    for f in &facts {
        assert!(
            journal.already(f.event).is_some(),
            "a fact reached a mind before the journal"
        );
    }

    // Committing the same day again commits nothing.
    let again = Consequences::commit(&mut journal, 4242, &facts);
    assert!(again.is_empty(), "replaying a day wrote history twice");
    assert_eq!(journal.len(), facts.len());

    // Delivery is exactly once.
    let first = Consequences::delivered_to(&mut journal, Standing::TheWorker, who(2), &facts);
    assert!(!first.is_empty());
    for f in &facts {
        assert!(
            journal.apply_once(f.event).is_none(),
            "a fact was applied twice"
        );
    }
}

/// **And it survives a save.** Reloading does not re-deliver the news.
#[test]
fn the_news_is_not_delivered_twice_after_a_reload() {
    let mut seq = 0u64;
    let facts = a_mill_with_no_spare().decides(200.0, 10, &mut seq);
    let mut journal = Journal::new();
    Consequences::commit(&mut journal, 1, &facts);
    Consequences::delivered_to(&mut journal, Standing::TheWorker, who(2), &facts);

    let bytes = Save {
        world_seed: 1,
        day: 300,
        people: vec![],
        journal: journal.clone(),
        ..Default::default()
    }
    .to_bytes();
    let back = Save::from_bytes(&bytes).unwrap().journal;

    assert!(
        back.unapplied().is_empty(),
        "a reload had news still to deliver that had already been delivered"
    );
    for f in &facts {
        assert_eq!(back.already(f.event), journal.already(f.event));
    }
}

// =====================================================================
// gate 4: exposure matters
// =====================================================================

/// **A stranger does not acquire the fact merely because it is true.**
#[test]
fn who_finds_out_depends_on_where_they_stand() {
    let ended = Fact::EmploymentEnded;
    let shift = Fact::ShiftCancelled;
    let t = who(3);

    assert!(learns(Standing::TheWorker, ended, t).is_some());
    assert!(learns(Standing::Household, ended, t).is_some());
    assert!(learns(Standing::Workmate, ended, t).is_some());
    assert_eq!(
        learns(Standing::Otherwise, ended, t),
        None,
        "a stranger simply knew"
    );

    // The household lives on the money, not on the roster.
    assert_eq!(learns(Standing::Household, shift, t), None);
    assert!(learns(Standing::Workmate, shift, t).is_some());

    // And how each of them came by it differs, which memory keeps.
    assert_eq!(
        learns(Standing::TheWorker, ended, t),
        Some(Source::Told { by: t })
    );
    assert_eq!(
        learns(Standing::Workmate, ended, t),
        Some(Source::Overheard)
    );
}

// =====================================================================
// gate 6: opportunity is not control
// =====================================================================

/// **A busy town supplies opportunity; the person may still not be able
/// to reach it.**
#[test]
fn opportunity_is_only_one_part_of_being_able_to_act() {
    let busy = a_busy_town();
    let mobile = Reemployment::in_this_town(&busy, Reemployment::default());
    let stuck = Reemployment::in_this_town(
        &busy,
        Reemployment {
            accessibility: 0.05,
            qualification_fit: 0.1,
            ability_to_search: 0.1,
            ..Default::default()
        },
    );
    assert!(
        (mobile.opportunity - stuck.opportunity).abs() < 1e-12,
        "the market differed"
    );
    assert!(
        mobile.actual(60.0).source > stuck.actual(60.0).source * 3.0,
        "being unable to get to the work made no difference"
    );
}

/// **The market really does differ between the two towns**, and it comes
/// from the labour model rather than from here.
#[test]
fn the_town_supplies_the_opportunity() {
    let busy = Reemployment::in_this_town(&a_busy_town(), Reemployment::default());
    let dying = Reemployment::in_this_town(&a_dying_town(), Reemployment::default());
    assert!(busy.opportunity > dying.opportunity * 3.0);
    assert!(busy.actual(60.0).source > dying.actual(60.0).source);
}

/// **Runway, not a balance.** What matters is how many days the money
/// lasts, which is why dependants and the price of bread matter without
/// anything here knowing about them.
#[test]
fn what_counts_is_how_long_the_money_lasts() {
    // The same purse, two households.
    assert!(runway_days(300.0, 2.0) > runway_days(300.0, 10.0));
    assert_eq!(runway_days(300.0, 10.0), 30.0);
    // And consequences are bearable in proportion.
    let r = Reemployment::default();
    assert!(r.actual(120.0).consequences > r.actual(5.0).consequences);
    assert!(
        r.actual(5.0).consequences < 0.15,
        "a week's money made it all survivable"
    );
}

// =====================================================================
// the vertical slice: three branches
// =====================================================================

struct Outcome {
    lost_work: bool,
    actual_reach: f64,
    consequences: f64,
    wellbeing: f64,
    state: FunctionalState,
}

fn run(branch: &str, employer: Employer, market: Workforce, liquid: f64) -> Outcome {
    let cult = culture();
    let mut seq = 0u64;
    let mut journal = Journal::new();
    let facts = employer.decides(200.0, 10, &mut seq);
    Consequences::commit(&mut journal, 4242, &facts);
    let got = Consequences::delivered_to(&mut journal, Standing::TheWorker, who(2), &facts);

    let essential_per_day = 2.0;
    let runway = runway_days(liquid, essential_per_day);
    let circ = Consequences::circumstance(&got, &market, runway, essential_per_day);

    let mut c = Coarse::new(who(2), 4242, 0);
    // What he believes he can do — deliberately the same in every
    // branch, so that any difference between them is the world's.
    c.perceived_control = scale_sim::coping::ControlAppraisal {
        source: 0.6,
        consequences: 0.6,
        own_response: 0.5,
    };
    let me = promote(&c, &cult, 0);

    let mut event = 9_000u64;
    let mut changes = befell(&circ, 40, &mut event);
    // The market and the man's own circumstances decide what is actually
    // possible, which is what `resolve` settles against.
    let reach = Reemployment::in_this_town(&market, Reemployment::default());
    let actual = reach.actual(runway);
    for ch in changes.iter_mut() {
        if let scale_sim::scaling::What::StressorBegins(ref mut s) = ch.what {
            s.actual = actual;
        }
    }
    c.advance_through(&me.mind, me.mind.stress.tolerance, &changes);
    c.advance_to(365 * 3, &me.mind, me.mind.stress.tolerance);

    let after = promote(&c, &cult, 365 * 3);
    let _ = branch;
    Outcome {
        lost_work: circ.lost_work,
        actual_reach: actual.source,
        consequences: actual.consequences,
        wellbeing: after.mind.wellbeing_baseline,
        state: c.strain.state,
    }
}

/// **The three branches, and what must differ between them.**
///
/// Deliberately *not* asserting that the third man is impaired or the
/// second is fine. What is asserted is the causal difference: control is
/// higher where there is work, consequences are weaker with more runway,
/// and a short outage produces nothing at all.
#[test]
fn one_fault_three_lives() {
    let kept = run("spare", a_mill(), a_busy_town(), 120.0);
    let busy = run(
        "no spare, busy town",
        a_mill_with_no_spare(),
        a_busy_town(),
        120.0,
    );
    let dying = run(
        "no spare, dying town",
        a_mill_with_no_spare(),
        a_dying_town(),
        20.0,
    );

    // 1. The spare is the difference between an event and no event.
    assert!(!kept.lost_work, "a spare in store still cost him his job");
    assert!(busy.lost_work && dying.lost_work);
    assert!(
        kept.wellbeing > -0.01,
        "a short outage injured his life satisfaction"
    );
    assert_eq!(kept.state, FunctionalState::Regulated);

    // 2. Where there is work, he can actually do something about it.
    assert!(
        busy.actual_reach > dying.actual_reach * 2.0,
        "the labour market made no difference to what he could do: {:.2} vs {:.2}",
        busy.actual_reach,
        dying.actual_reach
    );

    // 3. Runway makes the consequences bearable.
    assert!(busy.consequences > dying.consequences);

    // 4. Both men who lost work carry the well-being injury, and it is
    //    the same injury — what differs is what it did to their
    //    functioning, which is a separate layer.
    assert!(busy.wellbeing < -0.05 && dying.wellbeing < -0.05);
    assert!(
        dying.state >= busy.state,
        "the dying town was easier to live in"
    );
}

// =====================================================================
// gate 10: it becomes a person you can meet
// =====================================================================

/// **Walking up to him afterwards.**
///
/// No unemployment dialogue: what he says comes out of the state that
/// year left him in, the relationship he holds, and how he was
/// approached.
#[test]
fn afterwards_you_can_go_and_talk_to_him() {
    let cult = culture();
    let mut seq = 0u64;
    let mut journal = Journal::new();
    let facts = a_mill_with_no_spare().decides(300.0, 10, &mut seq);
    Consequences::commit(&mut journal, 1, &facts);
    let got = Consequences::delivered_to(&mut journal, Standing::TheWorker, who(2), &facts);
    let circ = Consequences::circumstance(&got, &a_dying_town(), 10.0, 2.0);

    let mut c = Coarse::new(who(2), 99, 0);
    let me = promote(&c, &cult, 0);
    let mut event = 20_000u64;
    let changes = befell(&circ, 40, &mut event);
    c.advance_through(&me.mind, 0.25, &changes);
    c.advance_to(365 * 2, &me.mind, 0.25);
    let him = promote(&c, &cult, 365 * 2);

    let mem = Memory::new();
    let friend = ask(
        &him.mind,
        &mem,
        None,
        c.strain.state,
        0.5,
        &Approach::a_friend(),
        Asked::HowTheyAre,
        who(9),
        who(2),
    );
    let stranger = ask(
        &him.mind,
        &mem,
        None,
        c.strain.state,
        0.5,
        &Approach::default(),
        Asked::HowTheyAre,
        who(9),
        who(2),
    );

    // The state that year left him in is what he answers out of.
    assert_eq!(friend.because.state, c.strain.state);
    // And being stopped in the street by somebody he does not know is
    // colder than being asked by a friend — the same man, both times.
    assert!(stranger.act.delivery.warmth < friend.act.delivery.warmth);
    assert!(stranger.because.awkwardness > friend.because.awkwardness);
}

// =====================================================================
// gate 9: it scales
// =====================================================================

/// **The same year, lived in detail and advanced coarsely**, comes out
/// compatible — so a man nobody was watching is the same man when you
/// finally meet him.
#[test]
fn a_worker_nobody_watched_is_the_same_worker() {
    let cult = culture();
    let mut seq = 0u64;
    let mut journal = Journal::new();
    let facts = a_mill_with_no_spare().decides(300.0, 10, &mut seq);
    Consequences::commit(&mut journal, 1, &facts);
    let got = Consequences::delivered_to(&mut journal, Standing::TheWorker, who(2), &facts);
    let circ = Consequences::circumstance(&got, &a_dying_town(), 15.0, 2.0);

    let base = Coarse::new(who(2), 31, 0);
    let me = promote(&base, &cult, 0);
    let mut event = 30_000u64;
    let changes = befell(&circ, 40, &mut event);

    let mut watched = base.clone();
    watched.advance_through(&me.mind, 0.3, &changes);
    for day in 41..=730u64 {
        watched.advance_to(day, &me.mind, 0.3);
    }

    let mut ignored = base.clone();
    ignored.advance_through(&me.mind, 0.3, &changes);
    ignored.advance_to(730, &me.mind, 0.3);

    assert_eq!(watched.strain.state, ignored.strain.state);
    assert!((watched.strain.debt - ignored.strain.debt).abs() < 1e-9);
    assert_eq!(watched.growth, ignored.growth);
    // And putting him down and picking him up changes nothing.
    assert_eq!(
        demote(&promote(&ignored, &cult, 730)).strain,
        ignored.strain
    );
}

// =====================================================================
// gate 7: the household ledger is the real one
// =====================================================================

/// **Runway comes off the same account that buys food.**
///
/// Not a psychological savings figure passed along by hand: what a man
/// can do about being out of work is read from the household pool the
/// economy already debits at every counter. Which is what stops the mind
/// inventing an abstract poverty of its own.
#[test]
fn what_he_can_fall_back_on_is_read_off_the_ledger() {
    use scale_sim::econ::{Doctrine, DAYS_PER_YEAR};
    use scale_sim::money::{Account, Why};
    use scale_sim::network::Network;
    use scale_sim::polity::Polities;
    use scale_sim::region::Region;
    use scale_sim::settlement::Settlements;
    use scale_sim::world::World;

    let world = World::generate(384, 216, 20260828);
    let pol = Polities::partition(&world, 30);
    let set = Settlements::place(&world, &pol, 5000);
    let net = Network::build(&world, &set, 1000);
    let id = pol.ranked()[2].0;
    let mut e = Region::extract(&world, &pol, &set, &net, id, 5, Doctrine::Prudent)
        .expect("a nation")
        .economy;

    for _ in 0..(DAYS_PER_YEAR / 2) {
        e.step();
    }

    let households = 2.36;
    let town = 0usize;
    let pool = e.treasury.balance(Account::Households(town));
    assert!(pool > 0.0, "a town whose households hold nothing");

    let runway = runway_of(&e, town, households);
    assert!(
        runway.is_finite() && runway > 0.0,
        "runway came out at {runway}"
    );

    // **The same account, and it is the one the counters draw on.**
    e.step();
    let bought = e.treasury.today.iter().any(|t| t.why == Why::Purchase);
    assert!(bought, "a day in which nobody bought anything");
    let after = e.treasury.balance(Account::Households(town));
    assert_ne!(
        after, pool,
        "a day of trade left the household pool exactly where it was"
    );

    // And what the mind consumes moves with it, rather than being told.
    let now = runway_of(&e, town, households);
    assert!(now.is_finite());
    assert!(
        (now - runway).abs() > f64::EPSILON || (after - pool).abs() < f64::EPSILON,
        "the pool moved and the runway did not"
    );

    // A household with more mouths has less runway on the same pool,
    // which is why dependants matter without this knowing about them.
    assert!(runway_of(&e, town, households * 2.0) < runway_of(&e, town, households));
}

// =====================================================================
// gate 5: he can be wrong about why, and be corrected
// =====================================================================

/// **He blames the man who told him.**
///
/// A transformer in a substation he has never seen is not available to
/// him. What is available is the manager who said the words — so that is
/// who it was, until somebody tells him otherwise.
#[test]
fn he_blames_whoever_told_him_and_can_be_put_right() {
    use scale_sim::mind::Mind;
    use scale_sim::rng::Rng;

    let mut seq = 0u64;
    let facts = a_mill_with_no_spare().decides(200.0, 10, &mut seq);
    let sacking: Happened = *facts
        .iter()
        .find(|f| f.what == Fact::EmploymentEnded)
        .expect("nobody was let go");

    let worker = who(2);
    let manager = who(5);
    let ev = as_world_event(&sacking, Place(3), worker, Some(manager));
    assert_eq!(ev.actor, Some(manager));
    assert!(
        ev.facts.deliberate,
        "being sacked did not read as somebody's doing"
    );

    let mind = Mind::draw(&mut Rng::new(11), &culture());
    let mut mem = Memory::new();
    let mut rng = Rng::new(11);
    let mut happened = Arena::new();
    let of = happened.add(ev.clone());
    let p = mem
        .perceive(&ev, Some(of), Source::Told { by: manager }, 0.95, &mut rng)
        .expect("he did not take it in");
    let felt = mind.appraise(&mind.read(&p.facts));
    let trace = mem
        .encode(p, &mind, sacking.day, &felt)
        .expect("nothing was encoded");

    // He blames the man who told him.
    assert_eq!(
        mem.traces[trace].blamed.as_ref().and_then(|w| w.person()),
        Some(manager)
    );
    let said_before = scale_sim::memory::testimony(&mem.traces[trace]);

    // **Months later he learns what actually happened.** The blame moves;
    // what he was told does not.
    mem.reattribute(
        trace,
        PerceivedWho::Unknown(scale_sim::memory::Description(
            "a transformer nobody had a spare for".into(),
        )),
        0.7,
    );

    let said_after = scale_sim::memory::testimony(&mem.traces[trace]);
    assert_ne!(said_after, said_before, "being put right changed nothing");
    assert_eq!(
        mem.traces[trace].provenance(),
        Source::Told { by: manager },
        "learning the real cause turned hearsay into something he witnessed"
    );
    assert!(
        mem.traces[trace]
            .blamed
            .as_ref()
            .and_then(|w| w.person())
            .is_none(),
        "he still blames the manager"
    );
}

/// **And a workmate who only overheard it is a different witness**, with
/// the same event and a different record of it.
#[test]
fn two_men_at_the_same_mill_remember_it_differently() {
    let mut seq = 0u64;
    let facts = a_mill_with_no_spare().decides(200.0, 10, &mut seq);
    let sacking = *facts
        .iter()
        .find(|f| f.what == Fact::EmploymentEnded)
        .unwrap();
    let teller = who(5);

    let his = learns(Standing::TheWorker, sacking.what, teller);
    let theirs = learns(Standing::Workmate, sacking.what, teller);
    assert_eq!(his, Some(Source::Told { by: teller }));
    assert_eq!(theirs, Some(Source::Overheard));
    assert_ne!(
        his, theirs,
        "the man it happened to and a bystander recall it alike"
    );
}
