//! **One world, one clock, one update.**
//!
//! Every system in this project can demonstrate itself and none of them
//! agreed with the others about what time it was. These gates are on the
//! root that fixes that, and deliberately on the *property* rather than on
//! any one subsystem — because the failure mode is precisely that each
//! subsystem is individually fine.

use scale_sim::econ::{Doctrine, DAYS_PER_YEAR};
use scale_sim::game::{GameState, Phase, PHASES};
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::populace::Populace;
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::slice;
use scale_sim::world::World;

fn a_nation() -> Region {
    let world = World::generate(384, 216, 20260828);
    let pol = Polities::partition(&world, 30);
    let set = Settlements::place(&world, &pol, 5000);
    let net = Network::build(&world, &set, 1000);
    let id = pol.ranked()[2].0;
    Region::extract(&world, &pol, &set, &net, id, 5, Doctrine::Prudent).expect("a nation")
}

fn a_world() -> GameState {
    GameState::new(20260828).with_economy(a_nation().economy)
}

// =====================================================================
// the clock
// =====================================================================

/// **Gate: there is one clock, and the subsystems do not keep their own.**
///
/// The invariant the whole root exists to make checkable. Before it, the
/// economy advanced `ledger.day` inside its own `step`, `scaling` advanced
/// another, a household kept a day of the year, and `person::live_a_day`
/// took the day as an *argument* — so whoever called it was responsible for
/// passing the same number the economy happened to be on, and nothing
/// checked that they matched. Being careful is not a contract.
///
/// **And this gate on its own is weak, which is worth saying.** Two counters
/// that both increment by one every day agree whatever either believes, so
/// most of what follows would pass even with the root not owning the clock
/// at all — I checked, by deleting the mechanism and watching it stay green.
/// What discriminates is the assertion at the end: being told the day *sets*
/// it, which a lagging or skipped subsystem cannot satisfy by accident.
#[test]
fn everybody_agrees_what_day_it_is() {
    let mut g = a_world();
    assert_eq!(g.day(), 0);
    g.assert_clocks_agree();

    for expected in 1..=200u64 {
        g.a_day();
        assert_eq!(g.day(), expected, "the world lost count");
        assert!(
            g.clocks_agree(),
            "on day {expected} the economy thinks it is day {}",
            g.economy.as_ref().unwrap().ledger.day
        );
    }

    // And in bulk, which is the same thing said a different way — a world
    // advanced in one call and one advanced a day at a time are the same
    // world.
    let mut slow = a_world();
    let mut quick = a_world();
    for _ in 0..90 {
        slow.a_day();
    }
    quick.advance(90);
    assert_eq!(slow.day(), quick.day());
    assert_eq!(
        slow.economy.as_ref().unwrap().ledger.day,
        quick.economy.as_ref().unwrap().ledger.day
    );

    // **The part that actually discriminates.**
    //
    // An economy counting for itself can only go forward by one, so it
    // agrees with a world that also goes forward by one whatever either of
    // them thinks — which is why everything above passes even with the
    // mechanism deleted. Told to be on day 500, it is on day 500: a
    // subsystem that has been skipped, paused, or is catching up after
    // being unloaded arrives where the world is rather than where it left
    // off.
    let mut e = a_nation().economy;
    let born = e.ledger.day;
    e.step_at(500);
    assert_eq!(
        e.ledger.day, 500,
        "an economy told it is day 500 counted on from its own {born} instead"
    );
    e.step_at(900);
    assert_eq!(e.ledger.day, 900, "it did not follow a jump forward");
    // And left to itself it still counts, because most of this project's
    // gates drive an economy directly and must go on working.
    e.step();
    assert_eq!(e.ledger.day, 901);
}

/// **Gate: attaching a subsystem does not import its clock.**
///
/// An economy that has been running on its own arrives with a day of its
/// own. It gives that up: the world's date is the date, and a system joining
/// a world in progress joins it where it is rather than dragging everybody
/// back to its own reckoning.
#[test]
fn a_subsystem_joins_the_world_where_the_world_is() {
    let mut e = a_nation().economy;
    // It does not start at nought — an economy is seeded with a day of the
    // year so its seasons are in the right place — which is itself a small
    // example of the problem: the number means something different to it
    // than a world's date does.
    let born = e.ledger.day;
    for _ in 0..50 {
        e.step();
    }
    assert_eq!(
        e.ledger.day,
        born + 50,
        "the economy did not count its own days"
    );

    let mut g = GameState::new(1);
    g.advance(300);
    assert_eq!(g.day(), 300);

    let g = g.with_economy(e);
    assert_eq!(
        g.economy.as_ref().unwrap().ledger.day,
        300,
        "an economy joined a world three hundred days old and brought its own date with it"
    );
    g.assert_clocks_agree();
}

/// **Gate: the clock cannot be set from outside.**
///
/// Not a stylistic preference. A day that anything can assign is a day
/// several things will assign, which is how five of them came to disagree.
/// The field is private and `advance` is the only way it moves — this gate
/// records that as an intention, and the compiler enforces it.
#[test]
fn nothing_but_advancing_moves_the_clock() {
    let mut g = a_world();
    let before = g.day();
    // Every public thing that is not `a_day`/`advance`.
    g.note(Phase::World, "something happened");
    let _ = g.clocks_agree();
    let _ = g.saveable_parts();
    assert_eq!(
        g.day(),
        before,
        "reading or noting something moved the date"
    );

    g.a_day();
    assert_eq!(g.day(), before + 1);
}

// =====================================================================
// the order of a day
// =====================================================================

/// **Gate: a day has a written order, and everything inside it sees one
/// date.**
///
/// The order is not arbitrary — two of the rules in it were learned the
/// hard way and are recorded in this project's notes: the shops trade
/// before anybody counts who worked, and a service is paid before wages
/// fall due.
#[test]
fn a_day_happens_in_a_stated_order() {
    // Five phases, in a fixed order, and no duplicates.
    assert_eq!(PHASES.len(), 5);
    let mut sorted = PHASES;
    sorted.sort();
    assert_eq!(sorted, PHASES, "the phases are not in their declared order");
    for p in PHASES {
        assert!(!p.name().is_empty());
    }

    // **Everything within a day sees the same date.** The clock moves once,
    // at the end — so a system running in the last phase and one running in
    // the first do not disagree about the day they are both in.
    let mut g = a_world();
    let start = g.day();
    g.note(Phase::World, "first");
    g.note(Phase::Consequences, "last");
    assert!(
        g.today.iter().all(|h| h.day == start),
        "the date moved mid-day"
    );
    g.a_day();
    // And the record is a day's record, not a history.
    assert!(
        g.today.len() < 100,
        "the day's notes are accumulating as a log"
    );
}

// =====================================================================
// and what it is honestly worth so far
// =====================================================================

/// **Gate: the root reports how much of itself it can actually save.**
///
/// This project has been told, fairly, that its save "demonstrates a
/// subsystem codec, not restoration of the game". The answer to that is not
/// to claim otherwise; it is to make the shortfall a number that has to go
/// up. A root that owns four things and saves three says so.
#[test]
fn the_save_gap_is_a_number_rather_than_a_claim() {
    let g = a_world();
    let (saved, owned) = g.saveable_parts();
    assert!(saved <= owned, "it claims to save more than it owns");
    assert!(owned > 0);
    // **Deliberately failing to be complete**, and recorded as such: when
    // this reaches parity the assertion below is what has to change, and
    // changing it means the migration actually happened.
    assert!(
        saved < owned,
        "the root now saves everything it owns — update this gate and say so"
    );
}

/// **Gate: a world runs a year without the pieces coming apart.**
///
/// Not a demonstration that any one model is right — each of them has its
/// own gates — but that they survive being driven together by one thing for
/// long enough to matter.
#[test]
fn a_year_passes_with_everything_attached() {
    let e = a_nation().economy;
    let folk = Populace::seed(&e, 20, 20260828);
    let mut g = GameState::new(20260828).with_economy(e).with_folk(folk);

    for _ in 0..DAYS_PER_YEAR {
        g.a_day();
        g.assert_clocks_agree();
    }
    assert_eq!(g.day(), DAYS_PER_YEAR);

    // The economy is still conserving, which is the oldest invariant here
    // and the one most likely to be broken by driving it differently.
    let e = g.economy.as_ref().unwrap();
    e.ledger.assert_conserved();
    e.treasury.assert_conserved();

    // And the people are still there and still individuated.
    let folk = g.folk.as_ref().unwrap();
    assert!(
        !folk.people.is_empty(),
        "a year of being driven by the root emptied the sample"
    );
}

// =====================================================================
// the contracts a migration has to be able to rely on
// =====================================================================

/// **Gate: a mismatched subsystem date is corrected by the root.**
///
/// The version of the clock gate that actually discriminates. Two counters
/// both stepping by one agree whatever either believes; what proves the
/// root owns the clock is putting a subsystem's date deliberately wrong and
/// watching the next day put it back.
#[test]
fn a_subsystem_that_loses_track_is_put_right() {
    let mut g = a_world();
    g.advance(20);
    g.assert_clocks_agree();

    // Somebody has reached past the root — a stale load, a bad migration, a
    // system that counted for itself.
    g.economy.as_mut().unwrap().ledger.day = 9_999;
    assert!(!g.clocks_agree(), "the check cannot even see a wrong date");

    g.a_day();
    g.assert_clocks_agree();
    assert_eq!(
        g.economy.as_ref().unwrap().ledger.day,
        g.day(),
        "a day passed and the economy kept its own wrong date"
    );

    // And backwards, which is the more dangerous direction: a subsystem
    // that has fallen behind must catch up rather than drag the world back.
    let now = g.day();
    g.economy.as_mut().unwrap().ledger.day = 3;
    g.a_day();
    assert_eq!(
        g.day(),
        now + 1,
        "a lagging subsystem pulled the world backwards"
    );
    g.assert_clocks_agree();
}

/// **Gate: one advance of thirty days is thirty advances of one.**
///
/// Trivially true while `advance` is a loop, and asserted anyway — because
/// the moment anything in a phase starts working in bulk for speed, this is
/// the property that will quietly stop holding.
#[test]
fn thirty_days_is_thirty_days_however_it_is_asked_for() {
    let mut slow = a_world();
    let mut quick = a_world();
    for _ in 0..30 {
        slow.a_day();
    }
    quick.advance(30);

    assert_eq!(slow.day(), quick.day());
    let (a, b) = (
        slow.economy.as_ref().unwrap(),
        quick.economy.as_ref().unwrap(),
    );
    assert_eq!(a.ledger.day, b.ledger.day);
    // And the same world, not merely the same date.
    for m in 0..a.markets.len() {
        for &c in scale_sim::econ::Commodity::ALL.iter() {
            assert!(
                (a.price(m, c) - b.price(m, c)).abs() < 1e-9,
                "{} in market {m} came out differently depending on how the days were asked for",
                c.name()
            );
        }
    }
}

/// **Gate: mass and money both balance across buyer, seller and carrier.**
///
/// Freight is the newest way money moves and the one most likely to leak:
/// the consignee pays, the carrier is paid, and neither the tonnage nor the
/// currency may change in the process.
#[test]
fn nothing_leaks_when_a_cargo_moves() {
    let e = a_nation().economy;
    let folk = Populace::seed(&e, 15, 20260828);
    let mut g = GameState::new(20260828).with_economy(e).with_folk(folk);

    for _ in 0..200 {
        g.a_day();
        let e = g.economy.as_ref().unwrap();
        e.ledger.assert_conserved();
        e.treasury.assert_conserved();
    }

    // Somebody really was paid for carrying things, or this gate is
    // watching an economy where nothing moved.
    let paid = g
        .economy
        .as_ref()
        .unwrap()
        .treasury
        .flows
        .get("freight")
        .copied()
        .unwrap_or(0.0);
    assert!(
        paid > 0.0,
        "two hundred days and no freight was ever charged"
    );
}

/// **Gate: a cargo cannot change the figures that authorised it.**
///
/// The day opens with a photograph of the world. Anybody deciding what to
/// do today reads that; what they do changes the live state and becomes
/// tomorrow's photograph. A decision that can read state its own
/// consequences have altered turns a day into an argument about ordering.
#[test]
fn the_opening_position_does_not_move_during_the_day() {
    let mut g = a_world();
    g.a_day();

    let opened = g
        .economy
        .as_ref()
        .unwrap()
        .opening()
        .cloned()
        .expect("no opening");
    let day_of = opened.day;

    // Run the whole of the next day and the previous opening is untouched —
    // it is a photograph, not a view.
    let before = opened.clone();
    g.a_day();
    assert_eq!(
        before, opened,
        "the snapshot was a window rather than a photograph"
    );

    // And the new day has its own, taken after the last one closed.
    let next = g
        .economy
        .as_ref()
        .unwrap()
        .opening()
        .cloned()
        .expect("no opening");
    assert!(next.day > day_of, "the day opened on yesterday's figures");
    assert_eq!(next.stock.len(), g.economy.as_ref().unwrap().markets.len());
}

/// **Told means told, and it has to be told before the day happens.**
///
/// `step_at` recorded the day and then ran the entire day against
/// `ledger.day` — the economy's *own*, previous or corrupted, value —
/// overwriting it only at the very end. So a subsystem whose clock had
/// gone wrong performed a full day's seasons, journal entries, treasury
/// movements, shipment departures and due-date checks as day 9,999, and
/// then relabelled itself with the root's date. `freight.haul(self,
/// self.ledger.day)` passed the stale figure explicitly.
///
/// **The existing gate could not see it**, because it compared the final
/// label. Both counters ended where they were told and the day in
/// between was somebody else's. What discriminates is the date on the
/// first thing the day actually did.
#[test]
fn a_corrupted_clock_does_not_get_to_date_the_day() {
    let mut e = slice::build(Doctrine::Prudent);
    for _ in 0..5 {
        e.step();
    }
    let entries_before = e.journal.entries().len();

    // Reach past the root and put the subsystem far into the future,
    // which is what a stale load or a bad migration does.
    e.ledger.day = 9_999;
    e.step_at(500);

    assert_eq!(
        e.ledger.day, 500,
        "the economy did not end up where it was told"
    );

    // The half with teeth: everything the day *did* must be dated 500.
    let wrong: Vec<u64> = e
        .journal
        .entries()
        .iter()
        .skip(entries_before)
        .map(|x| x.day)
        .filter(|&d| d != 500)
        .collect();
    assert!(
        wrong.is_empty(),
        "{} of the day's own journal entries are dated elsewhere, first {:?} \
         — the work was done before the clock was corrected",
        wrong.len(),
        wrong.first()
    );

    // And the snapshot the day's decisions were taken against.
    assert_eq!(
        e.opening.as_ref().map(|o| o.day),
        Some(500),
        "the opening snapshot was taken on the wrong day"
    );
}
