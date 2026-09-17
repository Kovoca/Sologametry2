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
    let (saved, missing) = g.parts();
    let (n, owned) = g.saveable_parts();
    assert_eq!(n, saved.len());
    assert_eq!(owned, saved.len() + missing.len());

    // **The count is measured, not claimed.** Every part in `saved` was
    // arrived at by actually serialising it, so this cannot drift the way
    // two typed-in numbers did — and the parts still missing are named, so
    // the gap says *what* rather than only how much.
    assert!(
        saved.contains(&"the economy"),
        "the economy has a codec and the root does not know it: {saved:?}"
    );
    assert!(
        saved.contains(&"the ground"),
        "the ground overlay has a codec and the root does not know it: {saved:?}"
    );

    // **Deliberately failing to be complete**, and recorded as such: when
    // this reaches parity the assertion below is what has to change, and
    // changing it means the migration actually happened.
    assert!(
        !missing.is_empty(),
        "the root now saves everything it owns — update this gate and say so"
    );

    // A world with no economy in it must say so rather than counting one.
    let bare = scale_sim::game::GameState::new(1);
    assert!(
        !bare.parts().0.contains(&"the economy"),
        "a root with no economy claimed to have saved one"
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

/// **Gate: a decision is taken on the morning position, and proving that
/// means changing the world underneath it.**
///
/// The first version cloned the opening snapshot, ran a day, and asserted
/// the clone still equalled the snapshot it was cloned from. That proves
/// Rust values do not mutate each other. It says nothing whatever about
/// whether any decision *reads* the snapshot, which is the entire claim —
/// and an external review was right to call it out. Sixth time a gate of
/// mine had passed without testing what it names.
///
/// ### The second version hung on hundredths of a day
///
/// It picked the worst- and best-covered town for **steel**, emptied the
/// flush one into a third, and required a consignment to the short one.
/// Its own comment recorded the fixture as "24.4 days in the worst town
/// against 42.9 in the best on a target of twenty-five"; by the time
/// anything touched it the country read 24.36, 24.83, 24.96, 24.98 and
/// 30.18 on a target of 26.
///
/// **Which of five towns was "worst" had come to be decided by hundredths
/// of a day.** Any change that perturbed forty days of history reshuffled
/// them, the morning-worst town stopped being one the dispatcher would
/// serve, and the gate went red on a world behaving perfectly — reading
/// the road surface under a haul, which makes a motorway ten per cent
/// quicker, was enough. That is this file's older rule arriving again: **a
/// gate that reverses on a small move is measuring which side of a cliff
/// the country is on, not the mechanism it names.**
///
/// ### And the obvious replacement was wrong in a more interesting way
///
/// "Build the country twice, contradict live state in one, require the
/// same decision" is not the claim, because **two different reads are both
/// correct and only one of them is the photograph**:
///
/// | | read from |
/// |---|---|
/// | who needs it | **the morning position** — all anybody knows when the lorries leave |
/// | who can supply it | **live state** — you cannot load steel out of a town that has none |
///
/// Moving a town's whole holding somewhere else changes the *supply*
/// geography, and the dispatcher rightly answered differently. The
/// disturbance has to touch demand and leave every supplier alone.
///
/// ### What it asserts now
///
/// Run the country once and see where the lorries actually go. Then run it
/// again — deterministic, so it is the same country — and after the day
/// has opened, pile stock into the **consuming yards** of exactly those
/// destinations, taken out of the consuming yards of towns the lorries did
/// not serve. Supply is untouched: not one tonne moves into or out of a
/// site anybody could collect from.
///
/// Live state now says the towns the dispatcher was about to serve are the
/// best-stocked in the country. A dispatcher reading the photograph sends
/// the lorries anyway. One reading live figures cannot.
///
/// No marginal town, no threshold, and the tonnage moves rather than
/// appearing — a gate that has to break conservation to make its point is
/// testing a world that cannot exist.
#[test]
fn a_decision_is_taken_on_the_morning_position() {
    use scale_sim::econ::{Commodity, RECIPES};

    // **The commodity the dispatcher actually hauls**, found rather than
    // named. This was steel, the most-hauled good while every town was
    // built short of it and bought the balance from its neighbours; built
    // to make what they need, towns stopped sending steel about and the gate
    // was left watching an empty road. What it watches has to be something
    // lorries carry this morning, to works that consume it.
    let c = {
        let mut e = a_nation().economy;
        let mut freight = scale_sim::logistics::Logistics::found(&e);
        for _ in 0..40 {
            e.step();
        }
        let before: std::collections::BTreeSet<_> = e.shipments.iter().map(|(id, _)| id).collect();
        let day = e.ledger.day;
        freight.haul(&mut e, day);
        let mut count = vec![0usize; Commodity::ALL.len()];
        for (_, s) in e.shipments.iter().filter(|(id, _)| !before.contains(id)) {
            count[s.commodity as usize] += 1;
        }
        let consumed = |c: Commodity| {
            RECIPES
                .iter()
                .any(|r| !r.from_abroad && r.inputs.iter().any(|&(ic, _)| ic == c))
        };
        let mut hauled: Vec<Commodity> = Commodity::ALL
            .iter()
            .copied()
            .filter(|&c| count[c as usize] > 0 && consumed(c))
            .collect();
        hauled.sort_by(|a, b| count[*b as usize].cmp(&count[*a as usize]));
        *hauled
            .first()
            .expect("the dispatcher hauled nothing that any works consumes")
    };

    // Sites that *consume* the commodity. Piling stock into these changes
    // what the country looks like and changes nothing about what it can
    // send, because `logistics` will not collect from a works' own hopper.
    let consumers = |e: &scale_sim::econ::Economy, m: usize| -> Vec<usize> {
        (0..e.ledger.sites.len())
            .filter(|&s| e.ledger.sites[s].market == m)
            .filter(|&s| {
                e.ledger.sites[s]
                    .recipe
                    .map(|r| RECIPES[r].inputs.iter().any(|&(ic, _)| ic == c))
                    .unwrap_or(false)
            })
            .collect()
    };

    let run = |flatter: &[usize], suppliers: &[usize]| -> (Vec<usize>, Vec<usize>) {
        let mut e = a_nation().economy;
        let mut freight = scale_sim::logistics::Logistics::found(&e);
        for _ in 0..40 {
            e.step();
        }

        if !flatter.is_empty() {
            // The towns the lorries were about to visit, and their consuming
            // yards. **Filled halfway to full and no further**: a yard pushed
            // past its capacity cannot take the delivery, and a dispatcher
            // declining to send goods to a full yard is reading live room,
            // which it should. A gate that has to break a physical limit to
            // make its point is testing a world that cannot exist.
            let mut targets: Vec<usize> = Vec::new();
            for &m in flatter {
                targets.extend(consumers(&e, m));
            }
            assert!(
                !targets.is_empty(),
                "the towns the dispatcher served have no works that consume {c},                  so there is nowhere to put the stock without touching a supplier"
            );
            let room: Vec<f64> = targets
                .iter()
                .map(|&s| {
                    let site = &e.ledger.sites[s];
                    ((site.capacity[c as usize] - site.stock[c as usize]) * 0.5).max(0.0)
                })
                .collect();
            let placeable: f64 = room.iter().sum();

            // Taken from the consuming yards of towns nobody is serving and
            // nobody is sending from — a supplier's own yards count in what
            // it has to spare, which the dispatcher rightly reads live — and
            // no more than can be placed.
            let mut donors: Vec<usize> = Vec::new();
            for m in 0..e.markets.len() {
                if flatter.contains(&m) || suppliers.contains(&m) {
                    continue;
                }
                donors.extend(consumers(&e, m));
            }
            let held: f64 = donors
                .iter()
                .map(|&s| e.ledger.sites[s].stock[c as usize])
                .sum();
            let share = if held > 0.0 { (placeable / held).min(1.0) } else { 0.0 };
            let mut pot = 0.0;
            for s in donors {
                let give = e.ledger.sites[s].stock[c as usize] * share;
                e.ledger.sites[s].stock[c as usize] -= give;
                pot += give;
            }
            assert!(
                pot > 0.0,
                "no town that neither sends nor receives {c} holds any to move"
            );
            for (k, s) in targets.into_iter().enumerate() {
                e.ledger.sites[s].stock[c as usize] += pot * room[k] / placeable.max(1e-9);
            }
            e.ledger.assert_conserved();
        }

        let before: std::collections::BTreeSet<_> = e.shipments.iter().map(|(id, _)| id).collect();
        let day = e.ledger.day;
        freight.haul(&mut e, day);
        let today: Vec<_> = e
            .shipments
            .iter()
            .filter(|(id, _)| !before.contains(id))
            .filter(|(_, s)| s.commodity == c)
            .map(|(_, s)| (s.to_market, s.from_market))
            .collect();
        let mut went: Vec<usize> = today.iter().map(|t| t.0).collect();
        let mut from: Vec<usize> = today.iter().map(|t| t.1).collect();
        went.sort_unstable();
        went.dedup();
        from.sort_unstable();
        from.dedup();
        (went, from)
    };

    // Where the lorries go when nothing has been interfered with.
    let (ordinarily, sent_from) = run(&[], &[]);
    assert!(
        !ordinarily.is_empty(),
        "the dispatcher carried no {c} at all, so this gate is watching an \
         empty road — pick a commodity the country actually hauls"
    );

    // And where they go when live state says those very towns are the
    // best-stocked in the country.
    let (contradicted, _) = run(&ordinarily, &sent_from);

    let names = |v: &[usize]| -> String {
        if v.is_empty() {
            "nowhere at all".to_string()
        } else {
            v.iter()
                .map(|m| m.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        }
    };
    assert_eq!(
        ordinarily,
        contradicted,
        "the morning position sent the lorries to [{}] and, with live state \
         saying those towns are now the best-stocked in the country, they \
         went to [{}] instead. The dispatcher is reading the live figures.",
        names(&ordinarily),
        names(&contradicted),
    );
}

/// **And the photograph is a photograph**: last day's opening is not a
/// window onto today. Kept because it is cheap and it is the other half of
/// the claim, but it is deliberately *not* the gate — on its own it proves
/// only that a value does not change itself.
#[test]
fn yesterdays_opening_is_not_todays() {
    let mut g = a_world();
    g.a_day();
    let day_of = g
        .economy
        .as_ref()
        .unwrap()
        .opening()
        .expect("no opening")
        .day;
    g.a_day();
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
