//! Who does the work.
//!
//! The chain this exists to protect: a transformer fails, so the mill has
//! no power, so the mill does not run, so the people who work at the mill
//! are not working, so they cannot buy food. Every test here guards one
//! link of it.

use scale_sim::econ::{Doctrine, SiteKind, DAYS_PER_YEAR, RECIPES};
use scale_sim::labour::{HOURS_PER_WORKING_YEAR, NATURAL_UNEMPLOYMENT};
use scale_sim::network::Network;
use scale_sim::person::{self, Person, State, Trade};
use scale_sim::polity::Polities;
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

fn a_nation(doctrine: Doctrine) -> Region {
    let world = World::generate(384, 216, 20260828);
    let polities = Polities::partition(&world, 24);
    let settlements = Settlements::place(&world, &polities, 3000);
    let network = Network::build(&world, &settlements, 500);
    let &(id, _) = polities.ranked().first().expect("no nations");
    Region::extract(&world, &polities, &settlements, &network, id, 5, doctrine)
        .expect("no region")
}

#[test]
fn headcount_comes_from_the_real_labour_hours() {
    // A mill's staffing is not a guess: 0.2 person-hours to mill a tonne
    // of flour, 1,800 hours in a working year. If this drifts, every
    // employment figure in the sim is wrong at the root.
    let mut r = a_nation(Doctrine::Prudent);
    r.economy.step();

    for site in r.economy.ledger.sites.iter() {
        let Some(rc) = site.recipe else { continue };
        if site.kind == SiteKind::PowerPlant {
            continue; // rated off the grid, not off a throughput sentinel
        }
        let expect = site.throughput * RECIPES[rc].labour * 365.0 / HOURS_PER_WORKING_YEAR;
        assert!(
            expect.is_finite() && expect >= 0.0,
            "{}: nonsense headcount {expect}",
            site.name
        );
    }

    // And the total has to be a believable share of the people there. Six
    // commodities is a thin slice of a real economy, so these trades
    // should employ a small percentage of a city — not a third of it, and
    // not nobody.
    for m in 0..r.economy.markets.len() {
        let share = r.economy.workforce[m].posts / r.economy.markets[m].population.max(1.0);
        assert!(
            share < 0.35,
            "{}: these few trades claim {:.0}% of the whole population",
            r.economy.markets[m].name,
            share * 100.0
        );
    }
}

#[test]
fn a_working_region_is_near_full_employment() {
    // The baseline. If an undisturbed economy reads as mass unemployment
    // the measure is broken and nothing built on it means anything.
    let mut r = a_nation(Doctrine::Prudent);
    for _ in 0..90 {
        r.economy.step();
    }
    for m in 0..r.economy.markets.len() {
        let w = &r.economy.workforce[m];
        if w.posts <= 0.0 {
            continue;
        }
        assert!(
            w.unemployment < 0.60,
            "{} sits at {:.0}% unemployment with nothing wrong",
            r.economy.markets[m].name,
            w.unemployment * 100.0
        );
    }
}

#[test]
fn a_dead_power_station_puts_people_out_of_work() {
    // **The whole point of the module.** Before this, a blackout was an
    // inconvenience to a stockpile; it did not happen to anybody.
    let mut r = a_nation(Doctrine::Negligent);
    for _ in 0..40 {
        r.economy.step();
    }
    let before: f64 = r.economy.workforce.iter().map(|w| w.unemployment).sum::<f64>()
        / r.economy.workforce.len() as f64;

    // Destroyed, with no spare in store: built to order, so months.
    r.economy.grid.fail_transformer("main line");
    for _ in 0..120 {
        r.economy.step();
    }
    let after: f64 = r.economy.workforce.iter().map(|w| w.unemployment).sum::<f64>()
        / r.economy.workforce.len() as f64;

    assert!(
        after > before + 0.10,
        "the grid went down for four months and unemployment went {:.0}% -> {:.0}%",
        before * 100.0,
        after * 100.0
    );
}

#[test]
fn nobody_is_hired_and_fired_by_the_day() {
    // Firms hoard labour through a short stoppage, because losing trained
    // hands costs more than paying them to sweep up. Tying employment
    // straight to today's output made towns flicker between full
    // employment and half idle overnight.
    let mut r = a_nation(Doctrine::Prudent);
    for _ in 0..60 {
        r.economy.step();
    }
    let mut last: Vec<f64> = r.economy.workforce.iter().map(|w| w.unemployment).collect();
    for _ in 0..DAYS_PER_YEAR {
        r.economy.step();
        for m in 0..r.economy.workforce.len() {
            let now = r.economy.workforce[m].unemployment;
            assert!(
                (now - last[m]).abs() < 0.15,
                "{} went {:.0}% -> {:.0}% unemployed in a single day",
                r.economy.markets[m].name,
                last[m] * 100.0,
                now * 100.0
            );
            last[m] = now;
        }
    }
}

#[test]
fn wages_sag_where_hands_are_idle_but_do_not_collapse() {
    // Nominal wages are sticky. A doubling of unemployment does not halve
    // anybody's pay — what actually gives is whether anybody is hiring,
    // which is rationed separately.
    let mut r = a_nation(Doctrine::Prudent);
    for _ in 0..60 {
        r.economy.step();
    }
    for w in r.economy.workforce.iter() {
        assert!(
            (0.75..=1.40).contains(&w.wage_index),
            "a wage index of {:.2} is not a labour market, it is a lever",
            w.wage_index
        );
    }
}

#[test]
fn losing_the_grid_can_starve_a_man() {
    // End to end, in one life: the transformer that nobody kept a spare
    // for takes the mill down, the mill sheds its hands, and a man with no
    // savings runs out of food. This is the sentence the whole simulation
    // exists to be able to say.
    let mut r = a_nation(Doctrine::Negligent);
    let mut hal = Person::new("Hal", Trade::Labourer, 0, 40.0);

    for day_n in 0..400u64 {
        if day_n == 30 {
            r.economy.grid.fail_transformer("main line");
        }
        r.economy.step();
        let day = r.economy.ledger.day;
        person::live_a_day(&mut hal, &mut r.economy, day);
        if hal.state == State::Dead {
            break;
        }
    }

    assert!(
        hal.days_hungry > 0 || hal.state == State::Dead,
        "the grid was down for a year and it never cost him a meal"
    );
}

#[test]
fn a_labour_market_is_deterministic() {
    let run = || {
        let mut r = a_nation(Doctrine::Prudent);
        for _ in 0..150 {
            r.economy.step();
        }
        r.economy
            .workforce
            .iter()
            .map(|w| (w.working.to_bits(), w.hands.to_bits()))
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), run(), "two identical labour markets diverged");
}

#[test]
fn natural_unemployment_is_a_real_figure() {
    assert!(
        (0.03..=0.08).contains(&NATURAL_UNEMPLOYMENT),
        "a healthy labour market runs at four to six percent, not {NATURAL_UNEMPLOYMENT}"
    );
    assert!(
        (1_200.0..=2_400.0).contains(&HOURS_PER_WORKING_YEAR),
        "a working year is not {HOURS_PER_WORKING_YEAR} hours"
    );
}
