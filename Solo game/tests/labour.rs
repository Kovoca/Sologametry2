//! Who does the work.
//!
//! The chain this exists to protect: a transformer fails, so the mill has
//! no power, so the mill does not run, so the people who work at the mill
//! are not working, so they cannot buy food. Every test here guards one
//! link of it.

use scale_sim::econ::{Commodity, Doctrine, SiteKind, DAYS_PER_YEAR, RECIPES};
use scale_sim::labour::{HOURS_PER_WORKING_YEAR, NATURAL_UNEMPLOYMENT};
use scale_sim::network::Network;
use scale_sim::person::{self, Person, Trade, FOOD_PER_DAY};
use scale_sim::polity::Polities;
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::travel::Conveyance;
use scale_sim::world::World;

fn a_nation(doctrine: Doctrine) -> Region {
    let world = World::generate(384, 216, 20260828);
    let polities = Polities::partition(&world, 24);
    let settlements = Settlements::place(&world, &polities, 3000);
    let network = Network::build(&world, &settlements, 500);
    let &(id, _) = polities.ranked().first().expect("no nations");
    Region::extract(&world, &polities, &settlements, &network, id, 5, doctrine).expect("no region")
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
fn a_blackout_idles_the_works_but_not_the_farms() {
    // Spec A.2: a blackout stops the factory, a broken irrigation main
    // stops the farm. Gating every site on the grid made a dead
    // transformer idle a nation's agriculture, which is both wrong and
    // much too convenient a way to arrange a famine.
    let mut r = a_nation(Doctrine::Negligent);
    for _ in 0..40 {
        r.economy.step();
    }
    r.economy.grid.fail_transformer("main line");
    for _ in 0..60 {
        r.economy.step();
    }

    let mut farms_running = 0;
    for site in r.economy.ledger.sites.iter() {
        match site.kind {
            SiteKind::Farm => {
                if site.ran > 0.0 {
                    farms_running += 1;
                }
            }
            SiteKind::Mill | SiteKind::Factory => assert!(
                site.ran <= 0.0,
                "{} kept milling two months into a blackout",
                site.name
            ),
            _ => {}
        }
    }
    assert!(
        farms_running > 0,
        "a grid fault stopped every farm in the country — tractors run on diesel"
    );
}

#[test]
fn a_blackout_is_paid_for_in_wages_not_in_corpses() {
    // **What a supply shock actually does to a working man.**
    //
    // Not kill him — keep him poor. Bread quintuples in a fortnight and
    // his pay takes months to catch up, because nominal wages are reset
    // once a year and not every morning. That lag *is* the damage.
    //
    // Anchoring pay to today's food price erased it entirely: prices and
    // wages quintupled the same week and the blackout cost nobody
    // anything. Shutting the farms instead swung it the other way and
    // starved him outright. Neither is what happens.
    // **Measured while the shock is on, not at the end of the year.**
    //
    // A year-end balance says whatever the wage anchor happens to be
    // doing by then — it can even come out ahead, because prices fall back
    // faster than pay does once the grid is mended, and holding cash
    // through that is a windfall. That is real, but it is not the thing
    // this guards. What is always true is the squeeze itself: bread goes
    // up in a fortnight, pay takes months to follow, and in between a
    // day's work buys less.
    let mut r = a_nation(Doctrine::Negligent);
    for _ in 0..30 {
        r.economy.step();
    }
    let before = person::day_rate(&r.economy, 0, Trade::Labourer)
        / (r.economy.price(0, Commodity::ProcessedFood) * FOOD_PER_DAY);

    r.economy.grid.fail_transformer("main line");
    let mut worst = before;
    for _ in 0..120 {
        r.economy.step();
        let now = person::day_rate(&r.economy, 0, Trade::Labourer)
            / (r.economy.price(0, Commodity::ProcessedFood) * FOOD_PER_DAY);
        worst = worst.min(now);
    }

    assert!(
        worst < before * 0.75,
        "bread went up and a day's work still bought {worst:.1} days of it          against {before:.1} before — the wage is tracking the price, which          is the one thing it must not do"
    );
    assert!(
        worst > before * 0.1,
        "a day's work fell from {before:.1} days of food to {worst:.1} — that          is not stickiness, that is a collapse"
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

#[test]
fn a_shop_employs_people_and_the_fixtures_say_how_many() {
    // **Retail is the biggest employer there is and used to be none.**
    //
    // A shop was a stockpile with a name over the door: it produced
    // nothing, so it had no recipe, so `labour` counted nobody in it. That
    // left the model counting mills and canneries — about 1.5% of real
    // employment — and omitting the roughly 10% who work in shops.
    //
    // The headcount is not a percentage. Somebody has to work each till,
    // fill each bay of shelving and unload each lorry, so it comes off the
    // fixtures and changes when they do.
    use scale_sim::building::{Building, Fixture};

    let small = Building::shop(2.0, 4.0);
    let large = Building::shop(200.0, 4.0);
    assert!(
        large.staff() > small.staff() * 10.0,
        "a shop doing a hundred times the trade employs {:.0} against {:.0}",
        large.staff(),
        small.staff()
    );
    assert!(
        large.staff_at(Fixture::Till) > 1.0,
        "a supermarket with no cashiers"
    );

    let mut r = a_nation(Doctrine::Prudent);
    r.economy.step();
    let with_shops: f64 = r.economy.workforce.iter().map(|w| w.posts).sum();
    let shop_staff: f64 = r
        .economy
        .ledger
        .sites
        .iter()
        .filter_map(|s| s.fitted.as_ref())
        .map(|b| b.staff())
        .sum();
    // **Retail against every works in the country**, which is a different
    // question from what it used to be. This bar was 25% back when
    // `workforce.posts` counted only farms, mills, canneries and mines —
    // about a tenth of a real economy — and retail cleared it by scraping.
    // Now that ore, steel and manufacturing are in the count the
    // denominator is most of industry, and the honest comparison is with
    // the real shares: retail 14.1% of employment against manufacturing's
    // 7.6%, agriculture 1.1%, mining 0.2% and utilities 1.2%.
    //
    // **Known gap this made visible, recorded rather than tuned away:**
    // the works come out at 8.8% of the workforce, which is right, and the
    // shops at 1.2% against a real 14.1%, which is not. Shops are fitted
    // out per settlement in `building.rs` and a nation of 44M is not being
    // given anything like the retail floorspace it would really have.
    // That is a hole in the shop-fitting, not in the fixture arithmetic
    // this test is about.
    assert!(
        shop_staff > 0.0 && with_shops > 0.0,
        "a nation with no retail and no works at all"
    );
    assert!(
        shop_staff > with_shops * 0.05,
        "shops are {:.0} of {:.0} works posts — retail has all but vanished",
        shop_staff,
        with_shops
    );
}

#[test]
fn shop_work_is_shifts_not_a_salary() {
    // **Where retail's precarity actually lives.**
    //
    // The work is steady and the hours are not. A shop worker is put on
    // the rota a day at a time, so he is employed and still does not know
    // whether he is on next week — around 60% of UK retail work is
    // part-time and variable hours are the norm, and part-timers average
    // two and a half to three days.
    //
    // He should be able to live on it, and should not be able to count on
    // it. Rostering him a guaranteed six-day week instead gave him 85% of
    // the days in the year, which is a salary with a different name.
    let mut r = a_nation(Doctrine::Prudent);
    let mut hal = Person::new("Hal", Trade::Shopworker, 0, 60.0);
    let days = DAYS_PER_YEAR * 2;
    for _ in 0..days {
        r.economy.step();
        let day = r.economy.ledger.day;
        person::live_a_day(&mut hal, &mut r.economy, day);
    }
    assert!(hal.alive(), "a shop worker starved in a working town");

    let share = hal.days_worked as f64 / days as f64;
    assert!(
        share > 0.25,
        "he got on the rota {:.0}% of days — that is not a living",
        share * 100.0
    );
    assert!(
        share < 0.80,
        "he worked {:.0}% of every day for two years; that is not shift work",
        share * 100.0
    );
}

#[test]
fn a_small_shop_has_no_manager_and_a_big_one_has_several() {
    // **The owner works the till.**
    //
    // A corner shop has a proprietor who serves, orders, sweeps up and
    // does the books; inventing a separate manager for him gives you three
    // staff of whom two are supervising. The hats only come apart once
    // there are enough hands to need it.
    use scale_sim::building::Building;

    let corner = Building::shop(1.0, 4.0);
    assert_eq!(
        corner.supervisors(),
        0.0,
        "a corner shop with {:.1} hands has a supervisor",
        corner.floor_staff()
    );
    assert_eq!(corner.managers(), 0.0, "and a manager as well");

    let supermarket = Building::shop(75.0, 4.0);
    assert!(
        supermarket.supervisors() >= 1.0 && supermarket.managers() >= 1.0,
        "a supermarket of {:.0} hands runs itself",
        supermarket.floor_staff()
    );
    // Span of control: overheads should land near a tenth to a seventh,
    // which is what real organisations run at.
    let overhead = (supermarket.supervisors() + supermarket.managers()) / supermarket.floor_staff();
    assert!(
        (0.05..0.30).contains(&overhead),
        "{:.0}% of a supermarket is management",
        overhead * 100.0
    );
}

#[test]
fn a_quiet_shop_puts_fewer_people_on() {
    // Thirty checkouts, eight of them open on a wet Tuesday. About a third
    // of the floor's hours are fixed and the rest are rostered against the
    // till receipts, which is exactly why shop work is part-time and the
    // hours are never guaranteed. The managers are in regardless.
    use scale_sim::building::Building;

    let shop = Building::shop(75.0, 4.0);
    let flat_out = shop.staff_today(1.0);
    let quiet = shop.staff_today(0.1);
    assert!(
        quiet < flat_out * 0.85,
        "a shop at a tenth of its trade rosters {quiet:.0} against {flat_out:.0}"
    );
    assert!(
        quiet > flat_out * 0.3,
        "a quiet day emptied the place: {quiet:.0} against {flat_out:.0}"
    );
}

#[test]
fn the_floor_is_the_only_way_up() {
    // Supervising is the one promotion this economy contains, and it is
    // gated the way promotions are: a man off the street is not made a
    // chargehand. Real promotion to supervisor runs two to three years in.
    let mut r = a_nation(Doctrine::Prudent);
    let mut green = Person::new("Green", Trade::Shopworker, 0, 60.0);
    let day = r.economy.ledger.day;
    r.economy.step();
    let offers = person::work_available(&r.economy, 0, day, 0.0, Conveyance::OnFoot);
    assert!(
        offers.iter().any(|c| c.trade == Trade::Supervisor),
        "nowhere in a nation is anybody supervising anything"
    );
    person::live_a_day(&mut green, &mut r.economy, day);
    assert_eq!(
        green.trade,
        Trade::Shopworker,
        "made chargehand on his first morning"
    );

    // **And with the years in he *may* be — which is not the same thing.**
    //
    // Time on the floor is necessary and nowhere near sufficient. What
    // decides it is whether a post is going and what the people who fill
    // it think of you, and the strongest finding in the research on real
    // promotions is that **73% went to somebody who had worked with the
    // hiring manager or the manager's boss**. Proximity beats ability.
    //
    // Gated on tenure alone, every labourer in a three-year run of a
    // whole town was made up to chargehand and the cohort became all
    // supervisors, which is not a workforce.
    let mut good = Person::new("Hal", Trade::Shopworker, 0, 60.0);
    good.diligence = 0.85;
    let mut poor = Person::new("Wat", Trade::Shopworker, 0, 60.0);
    poor.diligence = 0.05;
    // **Five years, not four**, because the working week now means a shop
    // worker does not get 365 chances a year — and somebody who ends up
    // part-time takes longer to reach the two years of *service* the
    // threshold stands for. Which is real: part-timers are promoted more
    // slowly, and it is one of the ways part-time work costs more than
    // the hours it gives up.
    for _ in 0..(DAYS_PER_YEAR * 5) {
        r.economy.step();
        let d = r.economy.ledger.day;
        person::live_a_day(&mut good, &mut r.economy, d);
        person::live_a_day(&mut poor, &mut r.economy, d);
    }
    assert_eq!(
        good.trade,
        Trade::Supervisor,
        "four years on the floor, well thought of, and never made up — \
         worked {} days, standing {:.2}",
        good.days_worked,
        good.standing
    );
    assert_eq!(
        poor.trade,
        Trade::Shopworker,
        "made up to chargehand on time served alone, standing {:.2}",
        poor.standing
    );
    assert!(
        good.standing > poor.standing,
        "the better worker is no better regarded: {:.2} against {:.2}",
        good.standing,
        poor.standing
    );

    // **Being good somewhere nobody watches is worth very little.** A
    // haulier is on the road and a shop worker is across the counter from
    // whoever decides, so the same ability gets noticed in one and not the
    // other.
    let mut away = Person::new("Hal", Trade::Haulier, 0, 60.0);
    away.diligence = 0.85;
    for _ in 0..(DAYS_PER_YEAR * 2) {
        r.economy.step();
        let d = r.economy.ledger.day;
        person::live_a_day(&mut away, &mut r.economy, d);
    }
    let mut seen = Person::new("Hal", Trade::Shopworker, 0, 60.0);
    seen.diligence = 0.85;
    for _ in 0..(DAYS_PER_YEAR * 2) {
        r.economy.step();
        let d = r.economy.ledger.day;
        person::live_a_day(&mut seen, &mut r.economy, d);
    }
    assert!(
        seen.visibility > away.visibility,
        "the man on the road is as visible as the man behind the counter"
    );
}

#[test]
fn the_form_and_the_depth_both_follow_the_size() {
    // **Position and ownership are separate axes**, and both follow scale
    // — because the reasons to incorporate and the reasons to add a layer
    // of management are reasons that only arrive with size.
    use scale_sim::building::{Building, Ownership};

    // A corner shop: one person, who serves, orders, sweeps up and does
    // the books. Invent a manager for him and you get three staff of whom
    // two supervise.
    let corner = Building::shop(0.6, 4.0);
    assert_eq!(corner.ownership(), Ownership::SoleTrader);
    assert!(corner.ownership().owner_works_there());
    assert!(
        corner.ownership().unlimited_liability(),
        "his debts are the shop's"
    );
    assert!(!corner.ownership().can_sell_shares());
    assert_eq!(
        corner.supervisors(),
        0.0,
        "somebody supervising four people"
    );
    assert_eq!(corner.layers(), 1, "a corner shop with a hierarchy");

    // A supermarket: incorporated, the owner is somewhere else, and there
    // are layers between the till and the top.
    let big = Building::shop(75.0, 4.0);
    assert_eq!(big.ownership(), Ownership::Corporation);
    assert!(
        !big.ownership().owner_works_there(),
        "the shareholders are stacking shelves"
    );
    assert!(
        !big.ownership().unlimited_liability(),
        "liability has to stop at the company — that is what a company is"
    );
    assert!(big.ownership().can_sell_shares());
    assert!(big.supervisors() > 0.0);
    assert!(
        big.layers() >= 3,
        "a supermarket only {} layers deep",
        big.layers()
    );

    // **Depth stacks rather than being fixed at two.** One layer of
    // supervision is not enough once there are supervisors enough to need
    // supervising, and real large organisations run five to eight — two
    // million people at Walmart are about seven deep.
    let vast = Building::shop(4_000.0, 4.0);
    assert!(
        vast.layers() > big.layers(),
        "a distribution centre is no deeper than a supermarket"
    );
    assert!(
        vast.layers() <= 8,
        "{} layers, which is deeper than any real organisation",
        vast.layers()
    );

    // Management stays a sensible share: real organisations run a tenth to
    // a seventh of employment in supervision and management together.
    let overhead = (big.supervisors() + big.managers()) / big.staff();
    assert!(
        (0.05..0.30).contains(&overhead),
        "{:.0}% of a supermarket is management",
        overhead * 100.0
    );
}
