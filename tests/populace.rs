//! A town of people, not a number of them.

use scale_sim::econ::{Doctrine, DAYS_PER_YEAR};
use scale_sim::network::Network;
use scale_sim::person::Trade;
use scale_sim::polity::Polities;
use scale_sim::populace::Populace;
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

fn a_nation() -> scale_sim::region::Region {
    let world = World::generate(384, 216, 20260828);
    let pol = Polities::partition(&world, 30);
    let set = Settlements::place(&world, &pol, 5000);
    let net = Network::build(&world, &set, 1000);
    let id = pol.ranked()[2].0;
    Region::extract(&world, &pol, &set, &net, id, 5, Doctrine::Prudent)
        .expect("a nation to model")
}

#[test]
fn a_cohort_of_people_lives_in_the_economy_without_breaking_it() {
    // **The point of running people rather than numbers.** A single man in
    // a single town cannot tell you whether the person model and the
    // workforce statistics are describing the same place; a sample in
    // every town can.
    let mut e = a_nation().economy;
    let mut folk = Populace::seed(&e, 30, 20260828);
    assert!(folk.people.len() >= 60, "not enough people to say anything");

    let days = DAYS_PER_YEAR * 2;
    for day in 0..days {
        e.step();
        folk.live_a_day(&mut e, day);
    }
    // Everything a person does goes through the journal like everything
    // else, so conservation covers their work too.
    e.ledger.assert_conserved();

    // **Nobody starves in a working nation.** People go hungry — that is
    // the point of modelling a larder — but a prudent nation with its
    // lights on should not be killing a tenth of its sample.
    let died = folk.gone.len() as f64 / folk.people.len() as f64;
    assert!(
        died < 0.20,
        "{:.0}% of the sample died over two years in a working nation",
        died * 100.0
    );

    // Somebody has to be doing well and somebody badly, or the model is
    // not producing lives, it is producing an average.
    let mut money: Vec<f64> = folk.people.iter().map(|p| p.money).collect();
    money.sort_by(f64::total_cmp);
    let (poorest, richest) = (money[0], money[money.len() - 1]);
    assert!(
        richest > poorest * 3.0 + 100.0,
        "everybody ended on much the same money: {poorest:.0} to {richest:.0}"
    );
}

#[test]
fn advancement_needs_a_vacancy_and_not_a_timer() {
    // **Gated on days worked alone, every labourer in a three-year run was
    // made up to chargehand** — the whole cohort became supervisors, which
    // is not a workforce but a promotion timer with nobody to supervise.
    //
    // Real span of control is eight to fifteen, so about one in ten of a
    // shop or a works is in charge of the rest and the rest stay on the
    // floor because there is nowhere to go.
    let mut e = a_nation().economy;
    let mut folk = Populace::seed(&e, 40, 20260828);
    for day in 0..(DAYS_PER_YEAR * 3) {
        e.step();
        folk.live_a_day(&mut e, day);
    }

    for m in 0..e.markets.len() {
        let mine: Vec<&scale_sim::person::Person> =
            folk.people.iter().filter(|p| p.market == m).collect();
        if mine.len() < 10 {
            continue;
        }
        let bosses = mine.iter().filter(|p| p.trade == Trade::Supervisor).count();
        let share = bosses as f64 / mine.len() as f64;
        // **The supply of promotions is a real figure**, not a ratio
        // picked to look right: the labour model counts supervisory posts
        // from the works and shops that actually exist, at a span of
        // control of about ten. The cohort should settle near it.
        let w = &e.workforce[m];
        let real_share = w.supervisory_posts / w.posts.max(1e-9);
        assert!(
            (0.03..0.15).contains(&real_share),
            "{} has {:.0}% of its posts supervising, against a real 6-10%",
            e.markets[m].name,
            real_share * 100.0
        );
        assert!(
            share < real_share * 2.5 + 0.02,
            "{} is {:.0}% supervisors after three years against {:.0}% of posts",
            e.markets[m].name,
            share * 100.0,
            real_share * 100.0
        );
        // And the floor is still there to supervise.
        assert!(
            mine.iter().any(|p| p.trade != Trade::Supervisor),
            "{} has nobody left on the floor",
            e.markets[m].name
        );
    }
}

#[test]
fn a_town_the_statistics_call_idle_is_one_where_people_find_less_work() {
    // **The check the whole exercise exists for.** If the aggregates and
    // the individuals disagree, one of them is wrong.
    //
    // Compared trade by trade, because they are not otherwise the same
    // question: `labour::Workforce` counts the trades the *works* employ,
    // and a shop worker is rostered by `building.rs` and never appears in
    // it. A town can carry idle industrial hands and busy shops at once.
    let mut e = a_nation().economy;
    let mut folk = Populace::seed(&e, 40, 20260828);
    let days = DAYS_PER_YEAR;
    for day in 0..days {
        e.step();
        folk.live_a_day(&mut e, day);
    }

    let mut seen: Vec<(f64, f64)> = Vec::new();
    for m in 0..e.markets.len() {
        let worked = folk.worked_by(m, Trade::Labourer, days);
        if worked.is_nan() {
            continue;
        }
        seen.push((e.workforce[m].unemployment, worked));
    }
    assert!(seen.len() >= 3, "not enough towns with labourers to compare");

    // The town the statistics call idlest should not be the town whose
    // labourers work most.
    let idlest = seen
        .iter()
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .copied()
        .unwrap();
    let busiest = seen
        .iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .copied()
        .unwrap();
    assert!(
        idlest.1 <= busiest.1,
        "the town with {:.0}% statistical unemployment has the hardest-working \
         labourers in the country at {:.0}% of days",
        idlest.0 * 100.0,
        idlest.1 * 100.0
    );
}

#[test]
fn most_people_have_a_contract_and_some_have_nothing() {
    // **Everything here was offered a shift at a time**, which is how
    // *casual* work is done and is not how most people work. Most people
    // have a contract: guaranteed hours, paid whether or not trade was
    // brisk, ended by notice rather than by nobody ringing.
    //
    // Real UK: **56% are permanent full-time and 44% are not**; part-time
    // is 24% against an EU average of 17%; zero-hours is 2.9% of
    // employment, about 900,000 people.
    use scale_sim::person::Employment;
    let mut e = a_nation().economy;
    let mut folk = Populace::seed(&e, 50, 20260828);
    for (i, p) in folk.people.iter_mut().enumerate() {
        if i % 6 == 0 {
            p.trade = Trade::Public;
        }
    }
    for day in 0..(DAYS_PER_YEAR * 2) {
        e.step();
        folk.live_a_day(&mut e, day);
    }

    let share = |t: Option<Trade>, kind: Employment| -> f64 {
        let m: Vec<_> = folk
            .people
            .iter()
            .filter(|p| t.is_none_or(|t| p.trade == t))
            .collect();
        m.iter().filter(|p| p.employment == kind).count() as f64 / m.len().max(1) as f64
    };

    let full = share(None, Employment::FullTime);
    assert!(
        (0.40..0.75).contains(&full),
        "{:.0}% of the workforce is permanent full-time, against a real 56%",
        full * 100.0
    );

    // **The unevenness is the point.** 28.8% of the accommodation and
    // food workforce are on zero-hours contracts against 2.1% in public
    // administration — a fourteenfold difference in whether you know you
    // have work next week.
    let public_casual = share(Some(Trade::Public), Employment::Casual);
    let shop_casual = share(Some(Trade::Shopworker), Employment::Casual)
        + share(Some(Trade::Shopworker), Employment::None);
    assert!(
        public_casual < 0.08,
        "public service is {:.0}% casual, against a real 2%",
        public_casual * 100.0
    );
    assert!(
        shop_casual > public_casual * 2.0,
        "shop work at {:.0}% insecure is no worse than public service at {:.0}%",
        shop_casual * 100.0,
        public_casual * 100.0
    );

    // **A contract is a hold on the work, not a daily audition.** Somebody
    // on guaranteed hours works more days than somebody hunting for them,
    // and that is what the security *is*.
    let worked = |kind: Employment| -> f64 {
        let m: Vec<_> = folk.people.iter().filter(|p| p.employment == kind).collect();
        if m.is_empty() {
            return f64::NAN;
        }
        let d: u64 = m.iter().map(|p| p.days_worked).sum();
        d as f64 / (DAYS_PER_YEAR * 2 * m.len() as u64) as f64
    };
    let ft = worked(Employment::FullTime);
    let casual = worked(Employment::Casual);
    if !casual.is_nan() {
        assert!(
            ft > casual,
            "a full-time contract found {:.0}% of days against casual work's {:.0}% — \
             the contract is buying nothing",
            ft * 100.0,
            casual * 100.0
        );
    }
    // And a full-timer works most of the week, because that is what five
    // contracted days in seven means.
    assert!(
        (0.55..0.85).contains(&ft),
        "a full-time contract works {:.0}% of days",
        ft * 100.0
    );
}

#[test]
fn a_seasonal_worker_has_a_year_with_a_shape() {
    // **Not everybody's work is there all year.** Farming and fishing
    // chiefly: real agricultural labour swings about twofold between
    // season and slack, Britain brings in some 45,000 people a year on a
    // seasonal visa purely to get the harvest in, and 30-50% of winter
    // days in the North Sea are lost to weather outright.
    //
    // The consequence is a year with a shape to it — earn hard for three
    // months and make it last nine, or move. A wage that is adequate in
    // August is nothing in February, and that is not unemployment, it is
    // the job.
    use scale_sim::person::Employment;
    let mut e = a_nation().economy;
    let mut folk = Populace::seed(&e, 60, 20260828);
    for p in folk.people.iter_mut() {
        p.trade = Trade::Labourer;
    }

    let mut by_quarter = [[0u64; 4]; 2]; // [seasonal, full-time]
    let mut prev: Vec<u64> = folk.people.iter().map(|p| p.days_worked).collect();
    for day in 0..(DAYS_PER_YEAR * 3) {
        e.step();
        folk.live_a_day(&mut e, day);
        let q = ((day % DAYS_PER_YEAR) * 4 / DAYS_PER_YEAR) as usize;
        for (i, p) in folk.people.iter().enumerate() {
            if p.days_worked > prev[i] {
                match p.employment {
                    Employment::Seasonal => by_quarter[0][q] += 1,
                    Employment::FullTime => by_quarter[1][q] += 1,
                    _ => {}
                }
            }
            prev[i] = p.days_worked;
        }
    }

    let swing = |q: [u64; 4]| -> f64 {
        *q.iter().max().unwrap() as f64 / (*q.iter().min().unwrap()).max(1) as f64
    };
    let seasonal = swing(by_quarter[0]);
    let permanent = swing(by_quarter[1]);
    assert!(
        by_quarter[0].iter().sum::<u64>() > 100,
        "nobody on the land is seasonal"
    );

    // **Real agricultural labour swings about twofold.** A permanent hand
    // on the same land sees some of that — the farm's own output is
    // seasonal — but far less of it, which is the whole difference
    // between the two ways of being employed.
    assert!(
        (1.4..3.5).contains(&seasonal),
        "seasonal work swings {seasonal:.1}x through the year, against a real ~2x"
    );
    assert!(
        seasonal > permanent,
        "seasonal work at {seasonal:.1}x is no more seasonal than permanent work at {permanent:.1}x"
    );

    // And the shape is the right way up: the lean quarter is not the busy
    // one. A harvest is a harvest.
    let s = by_quarter[0];
    assert!(
        s.iter().max().unwrap() > s.iter().min().unwrap(),
        "every quarter is the same on the land"
    );
}

#[test]
fn the_week_decides_who_works_when() {
    // **A year of 365 days was being lived as 365 identical ones.** Real
    // working life is shaped by the week far more sharply than by the
    // season: an office keeps Monday to Friday, a shop is open seven days
    // and is busiest at the weekend — real retail footfall peaks on
    // Saturday at about 1.5 times a weekday — and a works or a hospital
    // runs a rota that does not care what day it is.
    //
    // Which is why part-time and student work *is* weekend work. Not a
    // preference: it is where the shifts that are going actually are,
    // because the full-timers have Monday to Friday and somebody has to be
    // on the till on Saturday.
    use scale_sim::econ::Weekday;
    use scale_sim::person::Employment;
    use std::collections::HashMap;

    let mut e = a_nation().economy;
    let mut folk = Populace::seed(&e, 60, 20260828);
    for (i, p) in folk.people.iter_mut().enumerate() {
        p.trade = match i % 3 {
            0 => Trade::Office,
            1 => Trade::Shopworker,
            _ => Trade::Labourer,
        };
    }

    let mut tally: HashMap<(&str, bool, bool), u64> = HashMap::new();
    let mut prev: Vec<u64> = folk.people.iter().map(|p| p.days_worked).collect();
    let days = DAYS_PER_YEAR * 2;
    for day in 0..days {
        e.step();
        folk.live_a_day(&mut e, day);
        let weekend = Weekday::on(day).is_weekend();
        for (i, p) in folk.people.iter().enumerate() {
            if p.days_worked > prev[i] {
                let key = (p.trade.name(), p.employment == Employment::FullTime, weekend);
                *tally.entry(key).or_insert(0) += 1;
            }
            prev[i] = p.days_worked;
        }
    }
    let per_day = |t: &str, ft: bool, weekend: bool| -> f64 {
        let n = *tally.get(&(t, ft, weekend)).unwrap_or(&0) as f64;
        let of = if weekend { days * 2 / 7 } else { days * 5 / 7 };
        n / of as f64
    };

    // **An office keeps Monday to Friday**, and that is most of why people
    // want the job.
    assert_eq!(
        per_day("office work", true, true),
        0.0,
        "a full-time office worker came in at the weekend"
    );
    assert!(
        per_day("office work", true, false) > 1.0,
        "nobody is in the office on a Tuesday"
    );

    // **And the weekend shifts fall to whoever is not on a full-time
    // contract.** This is the shape of student and part-time work.
    let casual_weekend = per_day("shop worker", false, true);
    let casual_weekday = per_day("shop worker", false, false);
    assert!(
        casual_weekend > casual_weekday,
        "part-time shop work is no busier at the weekend: {casual_weekend:.1} against \
         {casual_weekday:.1} a day"
    );

    // While the full-timers in the same shop are on weekdays.
    let ft_weekend = per_day("shop worker", true, true);
    let ft_weekday = per_day("shop worker", true, false);
    assert!(
        ft_weekend < ft_weekday,
        "full-time shop staff work the weekend as hard as the week"
    );

    // A works runs a rota: quieter on a Sunday, not shut.
    assert!(
        per_day("labourer", true, true) > 0.0,
        "the mill closes at the weekend"
    );
}
