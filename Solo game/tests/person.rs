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
use scale_sim::travel::Conveyance;
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
    // **Count what he owns, not just what is in his pocket.**
    //
    // A man who spends a year's savings on a handcart is better off than
    // when he started, not worse, and measuring only cash called that a
    // failed life. What he has is the money plus the vehicle — and the
    // vehicle is the point of the saving.
    let wage = person::day_rate(&r.economy, hal.market, hal.trade);
    let kit = hal.conveyance.price_in_wage_days() * wage;
    assert!(
        hal.money + kit > 60.0,
        "worked {} days and is worth {:.0} ({:.0} in hand, {:.0} of {}), \
         having started with 60 — earned {:.0}, spent {:.0}, wage {:.2} \
         against bread at {:.2}",
        hal.days_worked,
        hal.money + kit,
        hal.money,
        kit,
        hal.conveyance.name(),
        hal.earned,
        hal.spent,
        wage,
        r.economy.price(hal.market, FOOD) * FOOD_PER_DAY,
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

    // A genuinely workless place: every road shut *and* every works
    // stopped. Closing the roads alone is not enough any more, and should
    // not be — most haulage is carting about inside a town, and that goes
    // on whether or not the road to the next town is open.
    for route in r.economy.routes.iter_mut() {
        route.open = false;
    }
    for site in r.economy.ledger.sites.iter_mut() {
        site.throughput = 0.0;
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
    // Starvation takes weeks — about forty days on its own. **Sleeping
    // out shortens it**, because exposure costs condition on top of
    // hunger, and the two together are what actually kills people on the
    // street rather than either alone.
    assert!(
        hal.days_hungry >= 25,
        "he died after only {} hungry days — starvation takes weeks even          with nowhere to sleep",
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
    // With every route shut, nothing can be carried *between towns* — and
    // no long haul is offered however badly a haulier needs one. Carting
    // about inside the town carries on, because a closed road does not
    // stop the grain going from the farm to the mill.
    let mut r = a_nation(20260828);
    for _ in 0..60 {
        r.economy.step();
    }
    for route in r.economy.routes.iter_mut() {
        route.open = false;
    }
    r.economy.step();

    let day = r.economy.ledger.day;
    let offers = person::work_available(&r.economy, 0, day, 500.0, Conveyance::Artic);
    assert!(
        !offers.iter().any(|c| matches!(
            c.kind,
            scale_sim::person::Job::Haul { .. } | scale_sim::person::Job::Venture { .. }
        )),
        "a haul between towns was offered with every road closed"
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
    // **His books must close.** What he holds is what he started with,
    // plus what he made, less what he ate and what he bought to work with.
    // Nothing else may appear or vanish — and the failures that made this
    // worth asserting were exactly that: a venture settled on cargo that
    // was never loaded, and a journey whose fodder was charged against the
    // profit but never taken out of the purse.
    // A trader caught mid-journey has his money in the cargo rather than
    // in his hand, and that is not a leak.
    let staked = hal.job.as_ref().map(|c| c.stake()).unwrap_or(0.0);
    let closes = 60.0 + hal.earned - hal.spent - staked;
    assert!(
        (hal.money - closes).abs() < 0.01_f64.max(hal.money.abs() * 1e-9),
        "he holds {:.2} but the books say {closes:.2} \
         (earned {:.2}, spent {:.2}, {staked:.2} staked on the road)",
        hal.money,
        hal.earned,
        hal.spent
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
        let offers = person::work_available(&r.economy, m, day, 0.0, Conveyance::Artic);
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

#[test]
fn rent_is_the_biggest_thing_he_buys() {
    // **Housing is a quarter to a third of a low income** *(real: 25-35%,
    // and "housing stressed" is the term for anything above 30)*, against
    // a tenth to a seventh on food. It was not modelled at all, so
    // everybody in this world lived rent-free and the poorest man in it
    // could still save for a lorry.
    let r = a_nation(20260828);
    for m in 0..r.economy.markets.len() {
        let rent = person::rent_per_day(&r.economy, m);
        let food = r.economy.price(m, FOOD) * FOOD_PER_DAY;
        let wage = person::day_rate(&r.economy, m, Trade::Labourer);
        assert!(
            rent > food,
            "{}: rent {rent:.2} a day against {food:.2} for food — housing is \
             the larger of the two everywhere",
            r.economy.markets[m].name
        );
        let share = rent / wage;
        assert!(
            (0.15..0.45).contains(&share),
            "{}: rent is {:.0}% of a day's wage",
            r.economy.markets[m].name,
            share * 100.0
        );
    }
}

#[test]
fn a_man_in_work_keeps_his_roof() {
    // The baseline has to be that working keeps you housed, or the
    // failure state means nothing. A shop worker on three and a half days
    // a week should hold a rented room and put a little by.
    let mut r = a_nation(20260828);
    let mut hal = Person::new("Hal", Trade::Shopworker, 0, 60.0);
    for _ in 0..(DAYS_PER_YEAR * 2) {
        r.economy.step();
        let day = r.economy.ledger.day;
        person::live_a_day(&mut hal, &mut r.economy, day);
    }
    assert!(hal.alive());
    assert_ne!(
        hal.housing,
        person::Housing::Homeless,
        "two years of shop work and he is on the street, having worked {} days",
        hal.days_worked
    );
}

#[test]
fn a_price_shock_can_put_a_working_man_on_the_street() {
    // And once he is out he is half as employable, because the address
    // goes on the form — which is what makes homelessness self-sustaining
    // rather than a bad month. The order matters: hungry first, then
    // evicted, because rent is due whether or not he was on the rota and
    // food can be gone without for a day.
    // **A negligent nation, because a prudent one absorbs this.**
    //
    // This used to run on the default region and produce a quintupling of
    // bread from one transformer failure. It no longer does, and that is
    // the model getting better rather than the test getting stale: a
    // country with a spare transformer in store and a freight industry
    // that moves stock on days of cover simply rides the fault out. This
    // project's own notes say so — *with a spare transformer the run is
    // byte-identical to no fault at all* — so testing the shock requires
    // a country that has not bought one.
    let world = World::generate(384, 216, 20260828);
    let polities = Polities::partition(&world, 24);
    let settlements = Settlements::place(&world, &polities, 3000);
    let network = Network::build(&world, &settlements, 500);
    let &(id, _) = polities.ranked().first().expect("no nations");
    let mut r = Region::extract(
        &world,
        &polities,
        &settlements,
        &network,
        id,
        5,
        Doctrine::Negligent,
    )
    .expect("no region");
    let mut hal = Person::new("Hal", Trade::Shopworker, 0, 5.0);
    let mut evicted = false;
    for n in 0..800u64 {
        if n == 20 {
            r.economy.grid.fail_transformer("main line");
        }
        r.economy.step();
        let day = r.economy.ledger.day;
        person::live_a_day(&mut hal, &mut r.economy, day);
        if hal.housing == person::Housing::Homeless {
            evicted = true;
            break;
        }
    }
    assert!(
        evicted || hal.state == person::State::Dead,
        "a country with no spare transformer lost its power, and a man on shop \
         wages kept both his job and his tenancy throughout: worked {} days, \
         hungry {}, money {:.0}, bread {:.0}",
        hal.days_worked,
        hal.days_hungry,
        hal.money,
        r.economy
            .price(0, scale_sim::econ::Commodity::ProcessedFood)
    );
}

#[test]
fn a_child_under_school_age_is_a_reason_people_do_not_work() {
    // **The sharpest financial cliff most households ever cross.**
    //
    // Real English figures: a full-time nursery place for a child under
    // two takes **65% of a median take-home wage**; after-school care for
    // a five to eleven year old takes **17%**; from sixteen it is nothing
    // but food and a roof. The reason is staffing ratios — a nursery keeps
    // one adult to three under-twos, tighter than almost anywhere in
    // Europe — and you cannot make childcare cheap without making it
    // worse.
    //
    // Which is why maternal employment with under-fives is **just over
    // 60% against about 75% overall**.
    //
    // Held still and compared one thing at a time, because in a population
    // the effect is buried: a parent's work record is a lifetime and the
    // child was only small for part of it.
    use scale_sim::person::{childcare_share_of_wage, Person, Trade, SCHOOL_ENDS, SCHOOL_STARTS};

    // The cliff itself.
    assert_eq!(childcare_share_of_wage(1.0), 0.65, "an under-two");
    assert_eq!(childcare_share_of_wage(4.9), 0.65, "still not at school");
    assert_eq!(
        childcare_share_of_wage(SCHOOL_STARTS),
        0.17,
        "the fifth birthday is the cliff"
    );
    assert_eq!(childcare_share_of_wage(SCHOOL_ENDS), 0.0, "school is over");
    assert!(
        childcare_share_of_wage(1.0) > childcare_share_of_wage(6.0) * 3.0,
        "a nursery place is barely dearer than an after-school club"
    );

    // Two identical people, one with a toddler — **in a country with no
    // family policy**, which is the case the 65% figure describes.
    let mut r = a_nation(20260828);
    r.economy.government = None;
    let mut free = Person::new("Ann", Trade::Shopworker, 0, 200.0);
    let mut parent = Person::new("Ann", Trade::Shopworker, 0, 200.0);
    parent.children.push(1.0);

    for _ in 0..(DAYS_PER_YEAR * 2) {
        r.economy.step();
        let d = r.economy.ledger.day;
        person::live_a_day(&mut free, &mut r.economy, d);
        person::live_a_day(&mut parent, &mut r.economy, d);
    }

    assert!(
        parent.days_worked < free.days_worked,
        "a toddler cost nothing: {} days worked against {}",
        parent.days_worked,
        free.days_worked
    );
    // **A second child is what actually stops people**: at 65% each, two
    // under-fives cost more than the day pays, and the day stops being
    // worth working at all.
    let mut two = Person::new("Ann", Trade::Shopworker, 0, 200.0);
    two.children.push(1.0);
    two.children.push(3.0);
    for _ in 0..(DAYS_PER_YEAR * 2) {
        r.economy.step();
        let d = r.economy.ledger.day;
        person::live_a_day(&mut two, &mut r.economy, d);
    }
    assert!(
        two.days_worked < parent.days_worked,
        "a second under-five cost nothing on top of the first"
    );

    // **And that is only destiny if nobody carries any of it.** Real
    // family spending runs from 0.6% of GDP in the United States to about
    // 4% in France; Sweden caps what a parent pays at roughly 3% of income
    // against England's 65% of a wage. Put the same two children in a
    // country with a funded family policy and the constraint largely goes
    // away — which is the entire argument for having one.
    let mut supported = a_nation(20260828);
    supported.economy.government = Some(scale_sim::state::Government::govern(
        &supported.economy,
        scale_sim::state::Capacity::Developed,
    ));
    let mut helped = Person::new("Ann", Trade::Shopworker, 0, 200.0);
    helped.children.push(1.0);
    helped.children.push(3.0);
    for _ in 0..(DAYS_PER_YEAR * 2) {
        supported.economy.step();
        let d = supported.economy.ledger.day;
        person::live_a_day(&mut helped, &mut supported.economy, d);
    }
    assert!(
        helped.days_worked > two.days_worked,
        "a funded family policy bought nothing: {} days worked against {} without one",
        helped.days_worked,
        two.days_worked
    );
}
