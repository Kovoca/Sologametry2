//! One person living in the economy.
//!
//! The tests that matter here are not that a life goes well. They are that
//! it can go badly, that nothing is arranged for anybody, and that a
//! person's work moves real goods rather than conjuring wages out of the
//! air.

use scale_sim::econ::{Commodity, Doctrine, DAYS_PER_YEAR};
use scale_sim::network::Network;
use scale_sim::person::{self, Person, State, Trade, FOOD_PER_DAY};
use scale_sim::polity::Polities;
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

const FOOD: Commodity = Commodity::ProcessedFood;

fn a_nation(seed: u64) -> Region {
    let world = World::generate(384, 216, seed);
    let polities = Polities::partition(&world, 24);
    let settlements = Settlements::place(&world, &polities, 3000);
    let network = Network::build(&world, &settlements, 500);
    let &(id, _) = polities.ranked().first().expect("no nations");
    Region::extract(
        &world,
        &polities,
        &settlements,
        &network,
        id,
        5,
        Doctrine::Prudent,
    )
    .expect("no region")
}

#[test]
fn a_wage_covers_food_but_not_by_much() {
    // The number the whole thing rests on. A day's work must buy several
    // days' food or nobody could ever get ahead; it must not buy hundreds
    // or the economy is a fiction. Real low-wage work runs at something
    // like six to ten times a day's food.
    for seed in [1u64, 42, 20260828] {
        let r = a_nation(seed);
        for m in 0..r.economy.markets.len() {
            for trade in [Trade::Haulier, Trade::Labourer] {
                let wage = person::day_rate(&r.economy, m, trade);
                let food = r.economy.price(m, FOOD) * FOOD_PER_DAY;
                let ratio = wage / food.max(1e-9);
                assert!(
                    (1.5..12.0).contains(&ratio),
                    "seed {seed} {}: a day's work buys {ratio:.1} days of food",
                    r.economy.markets[m].name
                );
            }
        }
    }
}

#[test]
fn a_year_of_work_makes_a_living() {
    let mut r = a_nation(20260828);
    let start = 0;
    let mut hal = Person::new("Hal", Trade::Haulier, start, 60.0);

    for _ in 0..DAYS_PER_YEAR {
        r.economy.step();
        let day = r.economy.ledger.day;
        person::live_a_day(&mut hal, &mut r.economy, day);
    }

    assert!(hal.alive(), "a year of steady work and he starved anyway");
    assert!(
        hal.days_worked > 100,
        "he only found {} days of work in a year",
        hal.days_worked
    );
    assert!(
        hal.money > 60.0,
        "worked {} days and ended with {:.0}, having started with 60",
        hal.days_worked,
        hal.money
    );
}

#[test]
fn a_man_with_nothing_and_no_work_dies() {
    // The failure state has to exist. Without work and without money a
    // person starves, and that must actually happen rather than being a
    // number that never reaches zero.
    let mut r = a_nation(20260828);
    let mut hal = Person::new("Hal", Trade::Haulier, 0, 0.0);
    hal.larder = 0.0;

    // No work of his trade exists if there is nowhere to haul to.
    for route in r.economy.routes.iter_mut() {
        route.open = false;
    }

    let mut died_on = None;
    for _ in 0..120 {
        r.economy.step();
        let day = r.economy.ledger.day;
        person::live_a_day(&mut hal, &mut r.economy, day);
        if hal.state == State::Dead {
            died_on = Some(day);
            break;
        }
    }
    let died = died_on.expect("penniless with no work for four months and still alive");
    assert!(
        hal.days_hungry >= 40,
        "he died after only {} hungry days — starvation takes weeks",
        hal.days_hungry
    );
    let _ = died;
}

#[test]
fn work_moves_real_goods_and_conserves() {
    // A haul that pays but shifts nothing is a treadmill: the price gap
    // never closes and the same job is offered for ever. His work must go
    // through the journal like everything else.
    let mut r = a_nation(20260828);
    let mut hal = Person::new("Hal", Trade::Haulier, 0, 200.0);

    for _ in 0..(DAYS_PER_YEAR / 2) {
        r.economy.step();
        let day = r.economy.ledger.day;
        person::live_a_day(&mut hal, &mut r.economy, day);
        r.economy.ledger.assert_conserved();
    }
    assert!(hal.days_worked > 0, "he never worked at all");
}

#[test]
fn nothing_is_offered_that_the_economy_does_not_want() {
    // Spec A4.1: opportunities are side-effects of other lives, not gifts.
    // With every route shut there is no haulage to be done, and a haulier
    // is simply out of luck — no work appears because he needs some.
    let mut r = a_nation(20260828);
    for _ in 0..60 {
        r.economy.step();
    }
    for route in r.economy.routes.iter_mut() {
        route.open = false;
    }
    r.economy.step();

    let day = r.economy.ledger.day;
    let offers = person::work_available(&r.economy, 0, day, 500.0);
    assert!(
        !offers.iter().any(|c| c.trade == Trade::Haulier),
        "haulage was offered with every road closed"
    );
}

#[test]
fn a_hungry_man_cannot_take_heavy_work() {
    // Hunger is not only a countdown to death; it is a constraint on what
    // can be done long before that, which is what makes being poor a trap
    // rather than a timer.
    let mut r = a_nation(20260828);
    for _ in 0..30 {
        r.economy.step();
    }
    let mut hal = Person::new("Hal", Trade::Haulier, 0, 0.0);
    hal.larder = 0.0;
    hal.condition = 0.2;

    let day = r.economy.ledger.day;
    person::live_a_day(&mut hal, &mut r.economy, day);
    if let Some(job) = &hal.job {
        assert!(
            job.days <= 3.0,
            "a man at 0.2 condition took {} days of heavy work",
            job.days
        );
    }
}

#[test]
fn nobody_can_buy_a_towns_last_reserve() {
    // The rule that stopped ventures being a money printer. A market holds
    // back a working reserve, and that applies to a man with a lorry as
    // much as to a firm — otherwise the most profitable trade in the
    // country is always to strip whichever town is shortest of something
    // and carry it over the hill.
    let mut r = a_nation(20260828);
    for _ in 0..60 {
        r.economy.step();
    }
    for m in 0..r.economy.markets.len() {
        for &c in Commodity::ALL.iter() {
            if !c.storable() {
                continue;
            }
            let held: f64 = (0..r.economy.ledger.sites.len())
                .filter(|&s| r.economy.ledger.sites[s].market == m)
                .map(|s| r.economy.ledger.stock(s, c))
                .sum();
            let keep = r.economy.daily_draw(m, c) * c.target_cover_days();
            let surplus = r.economy.surplus(m, c);
            assert!(
                surplus <= (held - keep).max(0.0) + 1e-6,
                "{} would sell more {c} than it has to spare",
                r.economy.markets[m].name
            );
        }
    }
}

#[test]
fn a_trader_is_not_a_money_printer() {
    // Two years of the best trading he can find must look like a trade and
    // not like compound interest on nothing. The failures this guards are
    // real ones that both shipped: settling a venture on the tonnage he
    // *meant* to carry rather than what was loaded, and being charged for
    // a cargo the warehouse never handed over.
    let mut r = a_nation(20260828);
    let mut hal = Person::new("Hal", Trade::Haulier, 0, 60.0);
    for _ in 0..(DAYS_PER_YEAR * 2) {
        r.economy.step();
        let day = r.economy.ledger.day;
        person::live_a_day(&mut hal, &mut r.economy, day);
        r.economy.ledger.assert_conserved();
    }
    assert!(
        hal.money < 60.0 * 10_000.0,
        "two years of hauling turned 60 into {:.0} — that is a printer, not a trade",
        hal.money
    );
    // And what he holds has to be what he actually made.
    assert!(
        (hal.money - (60.0 + hal.earned)).abs() < hal.money.abs() * 0.5 + 100.0,
        "he holds {:.0} but claims to have earned {:.0} on a stake of 60",
        hal.money,
        hal.earned
    );
}

#[test]
fn routine_haulage_exists_when_nothing_is_mispriced() {
    // A driver in a city of millions is not idle for four months because
    // no arbitrage happens to be open. Most freight is a firm moving its
    // own stock, with no price gap involved at all — and offering only the
    // arbitrage hauls starved him.
    let mut r = a_nation(20260828);
    for _ in 0..120 {
        r.economy.step();
    }
    let day = r.economy.ledger.day;
    let mut found = 0;
    for m in 0..r.economy.markets.len() {
        let offers = person::work_available(&r.economy, m, day, 0.0);
        if offers.iter().any(|c| c.trade == Trade::Haulier) {
            found += 1;
        }
    }
    assert!(
        found >= r.economy.markets.len() - 1,
        "only {found} of {} markets had any haulage at all",
        r.economy.markets.len()
    );
}

#[test]
fn a_life_is_deterministic() {
    let run = || {
        let mut r = a_nation(555);
        let mut p = Person::new("Hal", Trade::Haulier, 0, 60.0);
        for _ in 0..200 {
            r.economy.step();
            let day = r.economy.ledger.day;
            person::live_a_day(&mut p, &mut r.economy, day);
        }
        (p.money.to_bits(), p.days_worked, p.log.len())
    };
    assert_eq!(run(), run(), "two identical lives diverged");
}
