//! The vertical slice's acceptance test, as executable assertions.
//!
//! `docs/state-and-economy-spec.md` Part D ends with an eight-step chain
//! that must run with no special-case code. These are those steps.

use scale_sim::econ::{Commodity, Economy};
use scale_sim::slice::{self, Doctrine};

const FOOD: Commodity = Commodity::ProcessedFood;

fn run(econ: &mut Economy, days: u64) {
    for _ in 0..days {
        econ.step();
    }
}

#[test]
fn conservation_holds_over_a_long_run() {
    // Rule R1. Nothing may enter or leave except through the journal —
    // this is the property the whole ledger/journal structure exists to
    // guarantee, and the one that must never regress.
    let mut econ = slice::build(Doctrine::Negligent);
    for day in 0..400 {
        econ.step();
        econ.ledger.assert_conserved();
        assert!(
            econ.ledger.day == day + 1,
            "day counter drifted at {day}"
        );
    }
}

#[test]
fn stock_never_goes_negative() {
    let mut econ = slice::build(Doctrine::Negligent);
    for _ in 0..200 {
        econ.step();
        for site in 0..econ.ledger.sites.len() {
            for &c in Commodity::ALL.iter() {
                let q = econ.ledger.sites[site].stock[c as usize];
                assert!(
                    q > -1e-6,
                    "{} holds {q:.6} {c} — stock went negative",
                    econ.ledger.sites[site].name
                );
            }
        }
    }
}

#[test]
fn undisturbed_economy_settles_at_cost() {
    // With supply comfortably meeting demand, price should sit at the cost
    // of production and cover at its target. If the baseline drifts, every
    // disruption measurement downstream is meaningless.
    let mut econ = slice::build(Doctrine::Negligent);
    run(&mut econ, 60);

    for m in [slice::ASHFORD, slice::BEXLEY] {
        let price = econ.price(m, FOOD);
        let base = FOOD.base_cost();
        assert!(
            (price - base).abs() / base < 0.05,
            "{}: settled at {price:.0}, expected about {base:.0}",
            econ.markets[m].name
        );
        let cover = econ.markets[m].cover[FOOD as usize];
        assert!(
            (cover - FOOD.target_cover_days()).abs() < 0.5,
            "{}: cover settled at {cover:.1} days",
            econ.markets[m].name
        );
    }
}

#[test]
fn losing_the_only_line_starves_both_towns() {
    // Steps 1-4 of the acceptance test: no N-1 spare, so the line failure
    // stops the cannery, food stock runs down within days, and the price
    // rises steeply because food is inelastic.
    let mut econ = slice::build(Doctrine::Negligent);
    // Nobody available to fix it, so the outage persists and the full
    // cascade can be measured. The repair chain is tested separately.
    econ.response.crews = 0;
    run(&mut econ, 20);
    let before = econ.price(slice::ASHFORD, FOOD);

    assert!(econ.grid.fail_line("main line"));
    run(&mut econ, 3);
    assert!(
        econ.unserved_power > 0.0,
        "line is down but nothing is being shed"
    );

    // The 3-5 day retail buffer should carry them briefly, then fail.
    run(&mut econ, 12);
    let after = econ.price(slice::ASHFORD, FOOD);
    assert!(
        after > before * 2.0,
        "food went from {before:.0} to only {after:.0} after two weeks without power"
    );
    assert!(
        econ.unmet_demand[FOOD as usize] > 0.0,
        "stock is gone but nobody is recorded as going without"
    );
}

#[test]
fn a_redundant_grid_absorbs_the_same_failure() {
    // The doctrine trait made legible: identical shock, prudent grid, no
    // consequence at all. This is what a scouting player is reading.
    let mut econ = slice::build(Doctrine::Prudent);
    run(&mut econ, 20);
    let before = econ.price(slice::ASHFORD, FOOD);

    assert!(econ.grid.fail_line("main line"));
    run(&mut econ, 25);

    assert_eq!(
        econ.unserved_power, 0.0,
        "redundant grid shed load it should have carried"
    );
    let after = econ.price(slice::ASHFORD, FOOD);
    assert!(
        (after - before).abs() / before < 0.05,
        "price moved from {before:.0} to {after:.0} despite the spare line"
    );
}

#[test]
fn the_world_repairs_itself_without_being_told() {
    // Steps 6-8. Nothing here restores the line; the fault is noticed,
    // reported, assigned to a crew who travel to it, fixed, and the
    // economy recovers on its own.
    let mut econ = slice::build(Doctrine::Negligent);
    run(&mut econ, 20);
    let baseline = econ.price(slice::ASHFORD, FOOD);

    econ.grid.fail_line("main line");
    run(&mut econ, 40);

    let inc = econ.response.incidents.first().expect("no incident raised");
    assert!(inc.reported.is_some(), "the fault was never reported");
    assert!(inc.dispatched.is_some(), "no crew was ever sent");
    let resolved = inc.resolved.expect("the line was never repaired");
    let outage = resolved - inc.occurred;

    // A downed line is a matter of days even for a badly-run region —
    // crews carry what they need. Anything approaching a month means the
    // repair chain has broken.
    assert!(
        (3..=20).contains(&outage),
        "line was out for {outage} days, which is not a plausible line repair"
    );

    let recovered = econ.price(slice::ASHFORD, FOOD);
    assert!(
        (recovered - baseline).abs() / baseline < 0.10,
        "after repair price is {recovered:.0}, baseline was {baseline:.0}"
    );
}

#[test]
fn a_prudent_region_never_notices_the_outage() {
    // Fast response plus a spare line means the failure never reaches the
    // shops at all.
    let mut econ = slice::build(Doctrine::Prudent);
    run(&mut econ, 20);
    econ.grid.fail_line("main line");
    run(&mut econ, 20);

    let inc = econ.response.incidents.first().expect("no incident raised");
    let outage = inc.resolved.expect("never repaired") - inc.occurred;
    assert!(outage <= 5, "a well-run region took {outage} days");
    assert!(
        econ.unmet_demand[FOOD as usize] == 0.0,
        "nobody should have gone without"
    );
}

#[test]
fn a_spare_transformer_is_the_difference_between_days_and_never() {
    // The most consequential prudence decision on the grid. A transformer
    // is built to order with a lead time measured in months; holding one
    // in store turns a catastrophe into an inconvenience.
    let mut with_spare = slice::build(Doctrine::Prudent);
    run(&mut with_spare, 10);
    with_spare.grid.fail_transformer("main line");
    run(&mut with_spare, 40);
    let quick = with_spare.response.incidents[0]
        .resolved
        .expect("prudent region never replaced its transformer")
        - with_spare.response.incidents[0].occurred;
    assert!(
        (3..=30).contains(&quick),
        "swapping a spare transformer took {quick} days"
    );
    assert_eq!(with_spare.response.spare_transformers, 0, "spare not consumed");

    let mut without = slice::build(Doctrine::Negligent);
    run(&mut without, 10);
    without.grid.fail_transformer("main line");
    run(&mut without, 120);
    assert!(
        without.response.incidents[0].resolved.is_none(),
        "a transformer was replaced in four months with none in store"
    );
    assert!(
        without.unmet_demand[FOOD as usize] > 0.0,
        "four months without power and nobody went hungry"
    );
}

#[test]
fn nothing_is_fixed_when_nobody_can_report_it() {
    // Response is not automatic: it needs a witness with a working way to
    // tell someone. Cutting comms is an attack in its own right.
    let mut econ = slice::build(Doctrine::Negligent);
    econ.response.comms_up = false;
    run(&mut econ, 20);
    econ.grid.fail_line("main line");
    run(&mut econ, 60);

    assert_eq!(econ.response.unreported(), 1, "the fault got reported somehow");
    assert!(
        econ.response.incidents[0].dispatched.is_none(),
        "a crew was sent for a fault nobody had reported"
    );
    assert!(
        econ.unmet_demand[FOOD as usize] > 0.0,
        "two months of blackout and nobody went hungry"
    );
}

#[test]
fn a_haul_contract_appears_because_the_arithmetic_changed() {
    // Step 5, and the point of the whole exercise. No quest system
    // generates this; it exists when the price gap between two markets
    // exceeds the freight cost between them, and not before.
    let mut econ = slice::build(Doctrine::Negligent);
    run(&mut econ, 20);
    assert!(
        econ.arbitrage(0, FOOD) <= 0.0,
        "a profitable haul exists in an undisturbed economy — \
         the markets should be within freight cost of each other"
    );

    // A mere line fault will not do it: crews fix that inside the four-day
    // food buffer and the shops never notice. It takes a transformer with
    // no spare in store — a fault the region genuinely cannot answer.
    econ.grid.fail_transformer("main line");
    let mut appeared = false;
    for _ in 0..40 {
        econ.step();
        if econ.arbitrage(0, FOOD) > 0.0 {
            appeared = true;
            break;
        }
    }
    assert!(
        appeared,
        "the towns diverged but no profitable haul ever appeared"
    );
}

#[test]
fn a_routine_fault_never_reaches_the_shops() {
    // The counterpart, and the reason the one above needs a transformer:
    // a working region absorbs an ordinary failure entirely. If every
    // fault produced a famine the model would be worthless.
    let mut econ = slice::build(Doctrine::Negligent);
    run(&mut econ, 20);
    econ.grid.fail_line("main line");
    run(&mut econ, 25);

    assert!(
        econ.unmet_demand[FOOD as usize] == 0.0,
        "a four-day line repair left people hungry"
    );
    assert!(
        econ.arbitrage(0, FOOD) <= 0.0,
        "a routine fault created a trade opportunity"
    );
}

#[test]
fn cutting_the_road_decouples_the_markets() {
    // Spec A.7: the price gap between two markets cannot exceed the cost
    // of moving goods between them — unless nothing can move, in which
    // case they are no longer one market at all.
    let mut econ = slice::build(Doctrine::Prudent);
    run(&mut econ, 40);

    let gap_before = (econ.price(slice::ASHFORD, FOOD)
        - econ.price(slice::BEXLEY, FOOD))
    .abs();
    assert!(
        gap_before <= econ.routes[0].freight_cost + 1.0,
        "connected markets differ by {gap_before:.0}, more than the {:.0} freight cost",
        econ.routes[0].freight_cost
    );

    // Bexley has no works of its own; everything it eats comes down that
    // road.
    econ.routes[0].open = false;
    run(&mut econ, 25);

    let gap_after = (econ.price(slice::ASHFORD, FOOD)
        - econ.price(slice::BEXLEY, FOOD))
    .abs();
    assert!(
        gap_after > econ.routes[0].freight_cost * 5.0,
        "road is cut but the markets are still coupled (gap {gap_after:.0})"
    );
    assert!(
        econ.price(slice::BEXLEY, FOOD) > econ.price(slice::ASHFORD, FOOD),
        "the town with no industry should be the one that suffers"
    );
}

#[test]
fn the_journal_explains_every_change() {
    // Spec A2.3: an unattributed delta is a bug, not data. Every event
    // must carry a site and a quantity that actually moved.
    let mut econ = slice::build(Doctrine::Negligent);
    run(&mut econ, 30);

    assert!(!econ.journal.is_empty(), "nothing was journalled at all");
    for entry in econ.journal.entries() {
        assert!(entry.day < econ.ledger.day, "entry dated in the future");
        let qty = match &entry.event {
            scale_sim::econ::Event::Produced { qty, .. }
            | scale_sim::econ::Event::Consumed { qty, .. }
            | scale_sim::econ::Event::Shipped { qty, .. }
            | scale_sim::econ::Event::Spoiled { qty, .. } => *qty,
        };
        assert!(qty > 0.0, "journalled a non-positive quantity: {qty}");
    }
}

#[test]
fn the_slice_is_deterministic() {
    let mut a = slice::build(Doctrine::Negligent);
    let mut b = slice::build(Doctrine::Negligent);
    for _ in 0..50 {
        a.step();
        b.step();
    }
    a.grid.fail_line("main line");
    b.grid.fail_line("main line");
    run(&mut a, 20);
    run(&mut b, 20);

    for &c in Commodity::ALL.iter() {
        assert_eq!(
            a.ledger.total(c).to_bits(),
            b.ledger.total(c).to_bits(),
            "two identical runs hold different amounts of {c}"
        );
    }
    assert_eq!(a.journal.len(), b.journal.len());
}

#[test]
fn a_blackout_spoils_the_meat_and_only_delays_the_flour() {
    // **The difference between an inconvenience and a total loss.**
    //
    // A mill with no power loses production while it is off and catches up
    // afterwards. A butcher loses the stock: fresh meat is finished in a
    // day or two at ambient temperature and keeps four to six weeks
    // chilled, and that gap is the whole reason a cold chain exists.
    //
    // Before refrigerated shipping — the *Dunedin* carried frozen lamb
    // from New Zealand to London in 1882 — meat was eaten where it was
    // killed or it was salted.
    use scale_sim::econ::Commodity;

    // Ambient against chilled, in loss per day.
    let warm = Commodity::Meat.spoilage_per_day(false);
    let cold = Commodity::Meat.spoilage_per_day(true);
    assert!(warm > 0.4, "meat left out keeps better than a day");
    assert!(cold < 0.05, "a cold store loses {:.0}% of its stock a day", cold * 100.0);
    assert!(warm > cold * 10.0, "refrigeration barely helps");

    // **A shelf life is not a loss rate**, which was the first thing tried
    // and is wrong: grain in a decent silo loses 1-2% a *year* to insects,
    // rodents and damp. Reading one over the shelf life instead destroyed
    // 40% of a nation's grain annually and starved a country with a full
    // silo.
    let grain_year = Commodity::Grain.spoilage_per_day(true) * 365.0;
    assert!(
        (0.005..0.06).contains(&grain_year),
        "a silo loses {:.0}% of its grain a year",
        grain_year * 100.0
    );
    // A can loses nothing worth counting, which is what canning is for.
    assert!(Commodity::ProcessedFood.spoilage_per_day(true) * 365.0 < 0.02);
    // Stock on the hoof does not rot: it is alive, which is exactly why it
    // was walked to market for most of history.
    assert_eq!(Commodity::Livestock.spoilage_per_day(false), 0.0);

    // Over four days without power — the real time to fix a downed line —
    // an unrefrigerated store is essentially gone and a silo is untouched.
    let after = |c: Commodity, cold: bool, days: i32| {
        let mut held = 1.0f64;
        for _ in 0..days {
            held -= held * c.spoilage_per_day(cold);
        }
        held
    };
    assert!(
        after(Commodity::Meat, false, 4) < 0.10,
        "four days without power and the meat is still good"
    );
    assert!(after(Commodity::Grain, true, 4) > 0.99);
}

#[test]
fn a_line_to_a_house_does_not_take_out_the_country() {
    // **A grid is not one wire.** The old model pooled everything into one
    // or two transmission lines, so the only failure it could express was
    // "the country goes dark" — a service connection coming down took out
    // everybody.
    //
    // Real structure: generation, meshed transmission, primary substation,
    // feeder, distribution transformer, service connection. What decides
    // how many people a fault affects is *where in that it happens*, and
    // the numbers are not close — one customer for a service drop against
    // tens of thousands for a substation.
    use scale_sim::econ::{Doctrine, Level};
    use scale_sim::polity::Polities;
    use scale_sim::region::Region;
    use scale_sim::{network::Network, settlement::Settlements, world::World};

    let world = World::generate(256, 144, 20260828);
    let pol = Polities::partition(&world, 30);
    let set = Settlements::place(&world, &pol, 4000);
    let net = Network::build(&world, &set, 900);
    let id = pol.ranked()[2].0;
    let r = Region::extract(&world, &pol, &set, &net, id, 5, Doctrine::Prudent)
        .expect("a nation to model");
    let mut e = r.economy;
    let n_sites = e.ledger.sites.len();

    // Everything is on supply to start with.
    assert_eq!(e.grid.dark_sites(n_sites), 0, "the lights are out before anything broke");

    // **A service connection: one building.**
    let service = e
        .grid
        .lines
        .iter()
        .position(|l| l.level == Level::Service)
        .expect("no service connections in the grid");
    e.grid.lines[service].up = false;
    assert_eq!(
        e.grid.dark_sites(n_sites),
        1,
        "a service connection came down and took out more than one building"
    );
    e.grid.lines[service].up = true;

    // **A feeder: a share of a town, not a town and not a country.**
    let feeder = e
        .grid
        .lines
        .iter()
        .position(|l| l.level == Level::Feeder && !l.ring_fed && !l.feeds.is_empty())
        .or_else(|| {
            e.grid
                .lines
                .iter()
                .position(|l| l.level == Level::Feeder && !l.feeds.is_empty())
        })
        .expect("no feeders in the grid");
    let was_ring = e.grid.lines[feeder].ring_fed;
    e.grid.lines[feeder].ring_fed = false;
    e.grid.lines[feeder].up = false;
    let dark = e.grid.dark_sites(n_sites);
    assert!(
        dark > 0 && dark < n_sites,
        "a feeder fault put {dark} of {n_sites} works in the dark"
    );

    // **Ring-fed distribution is switched round in minutes.** Dense
    // networks are built as open rings so the operator restores supply
    // long before anybody repairs anything; the countryside gets one wire
    // and waits for a crew. That is why the same fault is an hour in a
    // city and most of a day in the country.
    e.grid.lines[feeder].ring_fed = true;
    assert_eq!(
        e.grid.dark_sites(n_sites),
        0,
        "a ring-fed feeder fault left people in the dark, with a second path standing"
    );
    e.grid.lines[feeder].ring_fed = was_ring;
    e.grid.lines[feeder].up = true;

    // **A substation takes its feeders with it**, which is what makes the
    // hierarchy a hierarchy rather than a list.
    let sub = e
        .grid
        .lines
        .iter()
        .position(|l| l.level == Level::Substation)
        .expect("no substations in the grid");
    e.grid.lines[sub].ring_fed = false;
    e.grid.lines[sub].up = false;
    let under_sub = e.grid.dark_sites(n_sites);
    assert!(
        under_sub > dark,
        "a substation took out {under_sub} works against a single feeder's {dark}"
    );
    e.grid.lines[sub].up = true;

    // **Transmission is meshed and built N-1**: lose one circuit and
    // nobody notices, which is why a pylon coming down is a news item and
    // not a blackout.
    let trans = e
        .grid
        .lines
        .iter()
        .position(|l| l.level == Level::Transmission)
        .expect("no transmission");
    e.grid.lines[trans].up = false;
    assert_eq!(
        e.grid.dark_sites(n_sites),
        0,
        "one transmission circuit out and the country is dark"
    );

    // A fault below transmission does not reduce what the grid can carry —
    // it disconnects what is behind it. Counting distribution into
    // capacity is what made a fault anywhere a shortage everywhere.
    let cap = e.grid.capacity();
    e.grid.lines[service].up = false;
    assert_eq!(e.grid.capacity(), cap, "a house's supply changed national capacity");
}

#[test]
fn a_utility_borrows_from_its_neighbour_rather_than_waiting_a_year() {
    // **A grid is licensed out in territories and each holder keeps its
    // own stores**, which is what decides whose shelf gets emptied — and
    // whether there is anybody to ask. Britain has fourteen distribution
    // licence areas; the United States has hundreds of investor-owned,
    // municipal and cooperative utilities.
    //
    // **And they lend to each other.** Mutual assistance is a real
    // formalised arrangement, and for the part that matters most there is
    // a named scheme: the Spare Transformer Equipment Program, under which
    // utilities pool large transformers and commit to releasing them. It
    // exists because a large power transformer is built to order and
    // cannot be bought in an emergency at any price.
    use scale_sim::econ::{Doctrine, Response, Sourced, Utility};

    let mut r = Response::for_doctrine(Doctrine::Prudent);
    r.utilities = vec![
        Utility { name: "North Power".into(), serves: vec![0, 2], spares: 1 },
        Utility { name: "South Power".into(), serves: vec![1, 3], spares: 1 },
    ];

    // Its own shelf: a swap, measured in days.
    let (own_days, how) = r.source_transformer(0);
    assert_eq!(how, Sourced::Own);
    assert!(own_days < 20, "fitting a spare took {own_days} days");

    // North is now out. The next fault in its area is covered by South —
    // and the delay is **haulage**, because a large power transformer is
    // 100-400 tonnes and 3.5-4.5 m wide: an abnormal load, an order from
    // the highway authority, a route surveyed for bridges, and a move at
    // walking pace. Real mutual-aid delivery runs two to six weeks.
    let (borrowed_days, how) = r.source_transformer(0);
    assert_eq!(how, Sourced::Borrowed { from: "South Power".into() });
    assert!(
        borrowed_days > own_days && borrowed_days < 60,
        "a borrowed transformer arrived in {borrowed_days} days"
    );

    // Now nobody has one, and it is a factory queue: **twelve to eighteen
    // months**, because these are built to order. That gap — days, weeks,
    // most of a year — is the single most consequential thing a utility's
    // stores decide.
    let (built_days, how) = r.source_transformer(0);
    assert_eq!(how, Sourced::Built);
    assert!(
        built_days > 300,
        "a transformer was built to order in {built_days} days"
    );
    assert!(
        built_days > borrowed_days * 5,
        "borrowing bought {borrowed_days} days against building's {built_days} — \
         not worth the standing agreement"
    );

    // A nation of any size has more than one company in it, and they can
    // reach each other.
    let world = scale_sim::world::World::generate(256, 144, 20260828);
    let pol = scale_sim::polity::Polities::partition(&world, 30);
    let set = scale_sim::settlement::Settlements::place(&world, &pol, 4000);
    let net = scale_sim::network::Network::build(&world, &set, 900);
    let id = pol.ranked()[2].0;
    let region = scale_sim::region::Region::extract(
        &world, &pol, &set, &net, id, 5, Doctrine::Prudent,
    )
    .expect("a nation to model");
    let us = &region.economy.response.utilities;
    assert!(!us.is_empty(), "a nation with no utility company in it");
    let served: usize = us.iter().map(|u| u.serves.len()).sum();
    assert_eq!(
        served,
        region.economy.markets.len(),
        "some towns are in nobody's licence area"
    );
}
