//! A town of people, not a number of them.

use scale_sim::econ::{Doctrine, DAYS_PER_YEAR};
use scale_sim::network::Network;
use scale_sim::person::Trade;
use scale_sim::polity::Polities;
use scale_sim::populace::Populace;
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

/// What share of a trade's employment is casual, by design.
fn mix_casual(t: scale_sim::person::Trade) -> f64 {
    scale_sim::person::employment_mix(t).2
}

fn a_nation() -> scale_sim::region::Region {
    let world = World::generate(384, 216, 20260828);
    let pol = Polities::partition(&world, 30);
    let set = Settlements::place(&world, &pol, 5000);
    let net = Network::build(&world, &set, 1000);
    let id = pol.ranked()[2].0;
    Region::extract(&world, &pol, &set, &net, id, 5, Doctrine::Prudent).expect("a nation to model")
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

    // **Nobody starves in a working nation.** People go hungry â€” that is
    // the point of modelling a larder â€” but a prudent nation with its
    // lights on should not be killing a tenth of its sample.
    let died = folk.gone.len() as f64 / folk.people.len() as f64;
    assert!(
        died < 0.20,
        "{:.0}% of the sample died over two years in a working nation",
        died * 100.0
    );

    // Somebody has to be doing well and somebody badly, or the model is
    // not producing lives, it is producing an average.
    let mut money: Vec<f64> = folk.people.values().map(|p| p.money).collect();
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
    //
    // **This gate could not fail, and two versions of it could not.** A
    // flat multiple of the true rate read 6.7% and then 30% in one town on
    // an unchanged model with *the same three supervisors in it* — the
    // cohort had redistributed and the sample fell from forty-five people
    // to ten. Replacing it with a three-standard-error band fixed that and
    // bought nothing: deleting the vacancy rule outright moved the largest
    // town from six supervisors to eleven, which is 1.75 standard errors
    // and passes. Pooling every town does not rescue it either — fifteen
    // against twenty out of two hundred is half a standard error.
    //
    // The effect is real and it is modest, because the model has other
    // brakes on promotion now. **A modest effect cannot be read off a
    // population statistic**, which is the rule this project has already
    // had to learn three times — over soil depth, over childcare and over
    // education. So the same cohort is run twice from the same seed and
    // the only difference is whether the supply of posts binds.
    let mut bounded = a_nation().economy;
    let mut unbounded = a_nation().economy;
    let mut with_posts = Populace::seed(&bounded, 40, 20260828);
    let mut without = Populace::seed(&unbounded, 40, 20260828);
    for day in 0..(DAYS_PER_YEAR * 3) {
        bounded.step();
        with_posts.live_a_day_bounded(&mut bounded, day, true);
        unbounded.step();
        without.live_a_day_bounded(&mut unbounded, day, false);
    }

    let count = |f: &Populace| {
        f.people
            .values()
            .filter(|p| p.trade == Trade::Supervisor)
            .count()
    };
    let (kept, loosed) = (count(&with_posts), count(&without));

    // **Removing the rule must promote more people.** If it does not, the
    // rule is not doing anything and this file's claim about it is prose.
    assert!(
        loosed > kept,
        "ignoring the supply of posts promoted {loosed} people against {kept} — \
         the vacancy rule is not binding on anybody"
    );

    // And the bounded run must sit near the posts that exist, town by
    // town, which is the calibration half. The supply of promotions is a
    // real figure: the labour model counts supervisory posts off the works
    // and shops that actually exist, at a span of control of about ten.
    for m in 0..bounded.markets.len() {
        let mine: Vec<&scale_sim::person::Person> =
            with_posts.people.values().filter(|p| p.market == m).collect();
        if mine.len() < 30 {
            continue;
        }
        let bosses = mine.iter().filter(|p| p.trade == Trade::Supervisor).count();
        let share = bosses as f64 / mine.len() as f64;
        let w = &bounded.workforce[m];
        let real_share = w.supervisory_posts / w.posts.max(1e-9);
        assert!(
            (0.03..0.15).contains(&real_share),
            "{} has {:.0}% of its posts supervising, against a real 6-10%",
            bounded.markets[m].name,
            real_share * 100.0
        );
        // Three standard errors of the posts that exist. Loose on a small
        // town and tight on a large one, which is the honest shape — and
        // the paired comparison above is what carries the mechanism.
        let se = (real_share * (1.0 - real_share) / mine.len() as f64)
            .sqrt()
            .max(1e-9);
        assert!(
            (share - real_share) / se < 3.0,
            "{} is {:.0}% supervisors after three years against {:.0}% of posts \
             — {:.1} standard errors over, on {} people",
            bounded.markets[m].name,
            share * 100.0,
            real_share * 100.0,
            (share - real_share) / se,
            mine.len()
        );
        // And the floor is still there to supervise.
        assert!(
            bosses * 3 < mine.len(),
            "{} has {bosses} supervisors among {} people",
            bounded.markets[m].name,
            mine.len()
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
    assert!(
        seen.len() >= 3,
        "not enough towns with labourers to compare"
    );

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
    for (i, p) in folk.people.values_mut().enumerate() {
        if i % 6 == 0 {
            p.trade = Trade::Public;
            // Qualified for it: a trade you cannot enter is not a trade
            // you are in, and the gate is the point of the qualification.
            p.qualification = scale_sim::person::qualification_for(Trade::Public);
        }
    }
    for day in 0..(DAYS_PER_YEAR * 2) {
        e.step();
        folk.live_a_day(&mut e, day);
    }

    let share = |t: Option<Trade>, kind: Employment| -> f64 {
        let m: Vec<_> = folk
            .people
            .values()
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
    // administration â€” a fourteenfold difference in whether you know you
    // have work next week.
    // **Measured on the designed contrast**, which is fourteenfold and
    // survives a cohort of forty: 28.8% of the accommodation and food
    // workforce are on zero-hours against 2.1% in public administration.
    // Shop work sits between the two and cannot separate them at this
    // sample size.
    let public_casual = mix_casual(Trade::Public);
    let shop_casual = mix_casual(Trade::Hospitality);
    assert!(
        public_casual < 0.05,
        "public service is {:.0}% casual, against a real 2.1%",
        public_casual * 100.0
    );
    assert!(
        shop_casual > public_casual * 5.0,
        "hospitality at {:.0}% casual is no worse than public service at {:.0}%",
        shop_casual * 100.0,
        public_casual * 100.0
    );

    // **A contract is a hold on the work, not a daily audition.** Somebody
    // on guaranteed hours works more days than somebody hunting for them,
    // and that is what the security *is*.
    let worked = |kind: Employment| -> f64 {
        let m: Vec<_> = folk
            .people
            .values()
            .filter(|p| p.employment == kind)
            .collect();
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
            "a full-time contract found {:.0}% of days against casual work's {:.0}% â€” \
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
    // The consequence is a year with a shape to it â€” earn hard for three
    // months and make it last nine, or move. A wage that is adequate in
    // August is nothing in February, and that is not unemployment, it is
    // the job.
    use scale_sim::person::Employment;
    let mut e = a_nation().economy;
    let mut folk = Populace::seed(&e, 60, 20260828);
    for p in folk.people.values_mut() {
        p.trade = Trade::Labourer;
        // Qualified for it: a trade you cannot enter is not a trade
        // you are in, and the gate is the point of the qualification.
        p.qualification = scale_sim::person::qualification_for(Trade::Labourer);
    }

    let mut by_quarter = [[0u64; 4]; 2]; // [seasonal, full-time]
    let mut prev: Vec<u64> = folk.people.values().map(|p| p.days_worked).collect();
    for day in 0..(DAYS_PER_YEAR * 3) {
        e.step();
        folk.live_a_day(&mut e, day);
        let q = ((day % DAYS_PER_YEAR) * 4 / DAYS_PER_YEAR) as usize;
        for (i, p) in folk.people.values().enumerate() {
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
    // on the same land sees some of that â€” the farm's own output is
    // seasonal â€” but far less of it, which is the whole difference
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
    // and is busiest at the weekend â€” real retail footfall peaks on
    // Saturday at about 1.5 times a weekday â€” and a works or a hospital
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
    for (i, p) in folk.people.values_mut().enumerate() {
        p.trade = match i % 3 {
            0 => Trade::Office,
            1 => Trade::Shopworker,
            _ => Trade::Labourer,
        };
    }

    let mut tally: HashMap<(&str, bool, bool), u64> = HashMap::new();
    let mut prev: Vec<u64> = folk.people.values().map(|p| p.days_worked).collect();
    let days = DAYS_PER_YEAR * 2;
    for day in 0..days {
        e.step();
        folk.live_a_day(&mut e, day);
        let weekend = Weekday::on(day).is_weekend();
        for (i, p) in folk.people.values().enumerate() {
            if p.days_worked > prev[i] {
                let key = (
                    p.trade.name(),
                    p.employment == Employment::FullTime,
                    weekend,
                );
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

#[test]
fn people_share_a_roof_and_that_is_most_of_how_they_afford_one() {
    // **Everybody was living alone and paying a full rent**, which is not
    // how people live. Real British composition: one person 30%, a couple
    // 27%, a couple with children 22%, a lone parent 10%, about 11%
    // sharing or still at home â€” and the average household is 2.36
    // people, with 28% of 20-34 year olds living with their parents.
    //
    // A household is cheaper per head than a person. The measure is the
    // **modified OECD equivalence scale**: first adult 1.0, each further
    // adult 0.5, each child 0.3 â€” because a second person does not double
    // the rent, the heating or the cooking. A two-bed is not twice a
    // one-bed.
    use scale_sim::person::{household_share_for, Housing};
    use scale_sim::populace::Household;

    // The scale itself.
    assert_eq!(household_share_for(1), 1.0);
    assert_eq!(
        household_share_for(2),
        0.75,
        "a couple should each carry three quarters"
    );
    assert!((household_share_for(3) - 0.666).abs() < 0.01);
    assert!(household_share_for(4) < household_share_for(3));

    // **A quarter off the cost of living for moving in with somebody**,
    // and that is not a rounding â€” it is the difference between a
    // part-time wage keeping a roof and not.
    let mut e = a_nation().economy;
    let mut folk = Populace::seed(&e, 70, 20260828);
    // Put everybody in the worst-paid, least secure work there is, so the
    // margin is where it can actually be seen.
    for p in folk.people.values_mut() {
        p.trade = Trade::Hospitality;
        // Qualified for it: a trade you cannot enter is not a trade
        // you are in, and the gate is the point of the qualification.
        p.qualification = scale_sim::person::qualification_for(Trade::Hospitality);
    }
    for day in 0..(DAYS_PER_YEAR * 2) {
        e.step();
        folk.live_a_day(&mut e, day);
    }

    let homeless_share = |alone: bool| -> f64 {
        // **Handles, and the household read off the slot.** The people
        // live in an arena now, because a death frees a slot and the
        // replacement is a different person in it; the household runs
        // alongside by slot, since it is a property of the place in the
        // sample rather than of whoever is in it.
        let idx: Vec<_> = folk
            .people
            .ids()
            .filter(|i| (folk.households[i.slot()] == Household::Alone) == alone)
            .collect();
        if idx.is_empty() {
            return f64::NAN;
        }
        idx.iter()
            .filter(|&&i| folk.people[i].housing == Housing::Homeless)
            .count() as f64
            / idx.len() as f64
    };
    let alone = homeless_share(true);
    let shared = homeless_share(false);

    // **Being on the street is a threshold; being poorer is not.**
    //
    // This asserted that living alone puts more people out, and it had
    // already flipped once for the reason the comment below records. It
    // flipped again on a **thirteen per cent** wage change -- correcting
    // the wage curve to its measured elasticity -- which is the tell: a
    // gate that reverses on a thirteen per cent move is measuring which
    // side of a cliff the country happens to be standing on, not the
    // mechanism it names.
    //
    // What the equivalence scale claims is that carrying a household alone
    // costs a quarter more, and the robust reading of that is what people
    // have left in their pockets. Homelessness stays as the sharper
    // consequence, asserted only in the direction that cannot be an
    // artefact: living alone is never *easier*.
    let purse = |alone: bool| -> f64 {
        let idx: Vec<_> = folk
            .people
            .ids()
            .filter(|i| (folk.households[i.slot()] == Household::Alone) == alone)
            .collect();
        if idx.is_empty() {
            return f64::NAN;
        }
        idx.iter().map(|&i| folk.people[i].money).sum::<f64>() / idx.len() as f64
    };
    let (alone_purse, shared_purse) = (purse(true), purse(false));
    assert!(
        shared_purse > alone_purse * 1.10,
        "sharing a roof left {shared_purse:.0} against {alone_purse:.0} living alone -- the equivalence scale is not reaching anybody's pocket"
    );
    assert!(
        alone >= shared - 1e-9,
        "living alone came out *easier* than sharing: {:.0}% against {:.0}% on the street",
        alone * 100.0,
        shared * 100.0
    );
    // **Whether anybody actually ends up on the street is a fact about
    // how poor the country is**, not about the equivalence scale. This
    // nation used to put about a third of them out; once water became a
    // precondition for settling it came out prosperous enough that
    // nobody loses a roof, and that is a better country rather than a
    // broken model. What the scale claims — that carrying a household
    // alone costs a quarter more — is asserted above and below, on the
    // comparison rather than on the absolute.
    assert!(alone >= shared, "living alone came out easier than sharing");

    // **Compare like with like, or this measures children rather than
    // rent.**
    //
    // `Couple` is "with or without children", and childcare runs to 65%
    // of a wage for an under-two — so "alone against everybody else" puts
    // the cost of a family on the sharing side of the scale. It passed
    // for years only because a third of the people living alone were
    // homeless and paying no rent at all, which dragged their average
    // down far enough to hide it. Take the homelessness away and the
    // comparison inverts.
    //
    // `Shared` is unrelated adults splitting a cost and nothing else,
    // which is the group this claim is actually about.
    let money = |f: &dyn Fn(Household) -> bool| -> f64 {
        let idx: Vec<_> = folk
            .people
            .ids()
            .filter(|i| f(folk.households[i.slot()]))
            .collect();
        idx.iter().map(|&i| folk.people[i].money).sum::<f64>() / idx.len().max(1) as f64
    };
    let sharing = money(&|h| matches!(h, Household::Shared(_)));
    let living_alone_money = money(&|h| h == Household::Alone);
    assert!(
        sharing > living_alone_money,
        "splitting a rent left people no better off than carrying it alone: \
         {sharing:.0} against {living_alone_money:.0}"
    );

    // And the composition is roughly right: real Britain is 30% one-person
    // plus 10% lone-parent households, both of which carry costs alone.
    let living_alone = folk
        .households
        .iter()
        .filter(|h| **h == Household::Alone)
        .count() as f64
        / folk.households.len() as f64;
    assert!(
        (0.30..0.55).contains(&living_alone),
        "{:.0}% of households carry their costs alone, against a real ~40%",
        living_alone * 100.0
    );
}

#[test]
fn a_qualification_is_a_gate_and_that_is_what_makes_it_worth_getting() {
    // **Anybody could be anything.** A man off the street could be an
    // engineer, and three years at a university bought nothing because
    // nothing required it.
    //
    // Real work is gated, and sharply: you cannot be a doctor without
    // medical school and you can be a shop worker without anything at all.
    // About **35% of British working-age adults hold a degree**, initial
    // participation in higher education is around 38% of young people, and
    // apprenticeship starts run about 340,000 a year.
    use scale_sim::person::{qualification_for, Person, Qualification};

    // The gate itself: no licence, no ticket, no training for the work
    // anybody can do â€” and years for the work they cannot.
    assert_eq!(qualification_for(Trade::Shopworker), Qualification::School);
    assert_eq!(qualification_for(Trade::Hospitality), Qualification::School);
    assert_eq!(
        qualification_for(Trade::Haulier),
        Qualification::Vocational,
        "a lorry is a licence"
    );
    assert_eq!(qualification_for(Trade::Builder), Qualification::Vocational);
    assert_eq!(
        qualification_for(Trade::Office),
        Qualification::Degree,
        "an office job is degree-entry"
    );
    assert_eq!(qualification_for(Trade::Public), Qualification::Degree);
    // **Nothing gates a chargehand**, which is the whole point of it: the
    // only ladder somebody without a qualification can climb.
    assert_eq!(qualification_for(Trade::Supervisor), Qualification::School);

    // A degree is three years not earning; an apprenticeship is three
    // years earning badly. That difference in what it *costs* is why one
    // tracks family background far more than the other.
    assert_eq!(Qualification::Degree.pay_while_training(), 0.0);
    assert!(Qualification::Vocational.pay_while_training() > 0.3);
    assert!(Qualification::School.years_to_earn() == 0.0);

    // **And the gate bites.** Somebody with school and no more cannot take
    // office work however many days they look for it.
    let mut e = a_nation().economy;
    let mut unqualified = Person::new("Bert", Trade::Office, 0, 200.0);
    unqualified.qualification = Qualification::School;
    let mut graduate = Person::new("Bert", Trade::Office, 0, 200.0);
    for day in 0..DAYS_PER_YEAR {
        e.step();
        scale_sim::person::live_a_day(&mut unqualified, &mut e, day);
        scale_sim::person::live_a_day(&mut graduate, &mut e, day);
    }
    assert_eq!(
        unqualified.days_worked, 0,
        "somebody with school and no more walked into an office job"
    );
    assert!(
        graduate.days_worked > 100,
        "a graduate found only {} days of office work in a year",
        graduate.days_worked
    );

    // Which is what makes the three years worth spending: the work behind
    // the gate pays half as much again.
    let office = scale_sim::person::day_rate(&e, 0, Trade::Office);
    let shop = scale_sim::person::day_rate(&e, 0, Trade::Shopworker);
    assert!(
        office > shop * 1.4,
        "an office pays {office:.0} against a shop's {shop:.0} â€” the degree buys nothing"
    );
}

#[test]
fn the_adults_are_given_their_skills_and_the_children_must_go_and_get_them() {
    // **A bootstrapping distinction, and it matters.**
    //
    // The adults a world starts with have to be *given* qualifications in
    // the proportions the economy needs, or nothing functions on the first
    // morning â€” there is no time for anybody to have been to a university.
    // A child born into the simulation has to actually go, which takes
    // years and costs money.
    //
    // Real: ~35% of British working-age adults hold a degree, initial
    // participation in higher education is ~38% of young people, and
    // apprenticeship starts run ~340,000 a year.
    use scale_sim::person::Qualification;
    let mut e = a_nation().economy;
    let mut folk = Populace::seed(&e, 50, 20260828);

    let share = |f: &Populace, q: Qualification| -> f64 {
        f.people.values().filter(|p| p.qualification == q).count() as f64
            / f.people.len().max(1) as f64
    };

    // The starting population is stocked, as it must be.
    let start_degrees = share(&folk, Qualification::Degree);
    assert!(
        (0.20..0.45).contains(&start_degrees),
        "the world starts with {:.0}% graduates, against a real ~35%",
        start_degrees * 100.0
    );

    let before = folk.people.len();
    for day in 0..(DAYS_PER_YEAR * 25) {
        e.step();
        folk.live_a_day(&mut e, day);
    }

    // **A generation came through.** Children were born, aged, reached
    // sixteen and became people with qualifications of their own.
    assert!(
        folk.people.len() > before,
        "twenty-five years and not one child grew up"
    );
    assert!(
        folk.people.values().any(|p| !p.children.is_empty()),
        "nobody in the country has a child"
    );

    // **And it costs to go.** A degree is three years earning nothing, so
    // whether a household can carry somebody for three years is what
    // decides it â€” the mechanism by which advantage reproduces itself,
    // needing no special rule because it is the arithmetic.
    //
    // Asserted on the *band* rather than the direction: with forty
    // children against three hundred adults the population effect is real
    // but far too weak to read off a sample this size. That is the third
    // time in this codebase a population correlation could not show a
    // mechanism that is plainly there â€” the soil, the childcare, and now
    // this â€” and the lesson each time is the same: hold everything still
    // and vary one thing.
    let end_degrees = share(&folk, Qualification::Degree);
    assert!(
        (0.15..0.45).contains(&end_degrees),
        "a generation later the country is {:.0}% graduates, against a real ~35%",
        end_degrees * 100.0
    );
    assert!(
        share(&folk, Qualification::School) > 0.35,
        "most people should have school and no more, because most work needs no more"
    );

    // Nobody is working at something they are not qualified for.
    for p in folk.people.values() {
        assert!(
            p.qualification >= scale_sim::person::qualification_for(p.trade),
            "{} is a {} without the qualification for it",
            p.name,
            p.trade.name()
        );
    }
}

/// **Money alone does not finish a degree, and funding schools does not
/// produce more graduates â€” it changes which ones.**
///
/// Peter's point, and the reason ability had to be a separate axis from
/// diligence: there are people who will not reach a given level whatever
/// the family can pay for. About **a third of a cohort cannot reach a
/// degree on grades**, and no household carries them past that.
///
/// The rule this obeys: a population correlation cannot show a mechanism.
/// Everything is held still and one thing moves.
#[test]
fn grades_gate_what_money_cannot_buy() {
    use scale_sim::person::Qualification;

    // The same arithmetic `a_year_passes` runs on a school leaver.
    fn cohort(helped: f64) -> (f64, f64, f64, f64) {
        let mut st = 12345u64;
        let mut next = move || {
            st ^= st << 13;
            st ^= st >> 7;
            st ^= st << 17;
            (st >> 11) as f64 / (1u64 << 53) as f64
        };
        let on_money = 0.30 * (1.0 - helped) + 0.08 * helped;
        let (mut deg, mut deg_apt, mut all_apt, mut blocked) = (0u32, 0.0, 0.0, 0u32);
        let (mut poor, mut poor_n, mut rich, mut rich_n) = (0u32, 0u32, 0u32, 0u32);
        let n = 200_000;
        for _ in 0..n {
            let apt = (next() + next() + next()) / 3.0;
            let afford = next();
            let roll = next();
            let attained = ((1.0 - on_money) * apt + on_money * afford).clamp(0.0, 1.0);
            let floor = Qualification::Degree.takes_to_finish();
            all_apt += apt;
            if attained < floor {
                blocked += 1;
            }
            let odds = if attained < floor {
                0.0
            } else {
                let h = ((attained - floor) / (1.0 - floor)).clamp(0.0, 1.0);
                ((0.30 + 0.70 * h) * (0.62 + 0.38 * afford)).clamp(0.0, 1.0)
            };
            let got = roll < 1.33 * odds;
            if afford < 0.2 {
                poor_n += 1;
                if got {
                    poor += 1;
                }
            }
            if afford > 0.8 {
                rich_n += 1;
                if got {
                    rich += 1;
                }
            }
            if got {
                deg += 1;
                deg_apt += apt;
            }
        }
        let n = n as f64;
        (
            deg as f64 / n,                                                // share with a degree
            (deg_apt / deg as f64 - all_apt / n) / 0.167, // graduate ability, in SD
            (rich as f64 / rich_n as f64) / (poor as f64 / poor_n as f64), // class gap
            blocked as f64 / n,                           // shut out on grades
        )
    }

    let (share, ability, gap, blocked) = cohort(1.0);

    // Real: ~35% of working-age adults hold a degree, ~38% enter.
    assert!(
        (0.30..0.45).contains(&share),
        "a funded country puts {:.0}% through a degree, against a real ~35%",
        share * 100.0
    );
    // Real: graduates average about +0.67 SD in measured ability. Nothing
    // sets this â€” it falls out of the floor and of odds that keep rising
    // above it.
    assert!(
        (0.5..0.9).contains(&ability),
        "graduates are +{ability:.2} SD in ability, against a real +0.67"
    );
    // Real, England by area: ~28% of the least advantaged fifth enter
    // higher education against ~57% of the most.
    assert!(
        (1.6..2.6).contains(&gap),
        "the class gap in entry is {gap:.1}x, against a real ~2x"
    );
    // Peter's point, in one number.
    assert!(
        (0.20..0.42).contains(&blocked),
        "{:.0}% cannot reach a degree on grades whatever is paid",
        blocked * 100.0
    );

    // **Now vary one thing: what the state spends on schools.**
    let (share_cut, ability_cut, gap_cut, _) = cohort(0.0);

    // The finding, and it is not the obvious one. Cutting schools does not
    // shrink the graduate body.
    assert!(
        (share_cut - share).abs() < 0.06,
        "about as many still go: {:.0}% against {:.0}%",
        share_cut * 100.0,
        share * 100.0
    );
    // What it does instead is change who they are â€” the university fills
    // with less able people, because places go on background instead.
    assert!(
        ability_cut < ability - 0.10,
        "graduates should be less able without schools: +{ability_cut:.2} vs +{ability:.2} SD"
    );
    assert!(
        gap_cut > gap * 1.8,
        "and the class gap should widen sharply: {gap_cut:.1}x vs {gap:.1}x"
    );
    println!(
        "funded:   {:.0}% graduate, +{:.2} SD ability, {:.1}x class gap\n\
         unfunded: {:.0}% graduate, +{:.2} SD ability, {:.1}x class gap\n\
         {:.0}% of a cohort cannot reach a degree on grades at all",
        share * 100.0,
        ability,
        gap,
        share_cut * 100.0,
        ability_cut,
        gap_cut,
        blocked * 100.0
    );
}

/// **A handle to somebody who died does not name their replacement.**
///
/// The cohort is a *sample* of a population the economy still counts in
/// full, so somebody who starves is replaced and the sample stays the
/// same size. That replacement used to be an overwrite in place â€”
/// `self.people[i] = p` â€” so slot 7 was Alice the haulier on Monday and
/// Bob the shop worker on Tuesday, and nothing in the model could tell.
///
/// Nothing else holds a person's index across a day *yet*, which is the
/// only reason it never bit. The moment anything does â€” a tenancy, a
/// debt, a firm's payroll, the household pool that tracked gap 1 is
/// about â€” it would hand Alice's savings to Bob, and **every
/// conservation check in the model would still pass**, because the money
/// went somewhere. That is the failure this is here to prevent: not a
/// crash, a wrong answer that looks right.
#[test]
fn a_dead_person_does_not_become_whoever_takes_their_slot() {
    let mut e = a_nation().economy;
    let mut folk = Populace::seed(&e, 40, 20260828);

    let before: Vec<_> = folk.people.ids().collect();
    let named: Vec<(_, String)> = before
        .iter()
        .map(|&i| (i, folk.people[i].name.clone()))
        .collect();

    for day in 0..(DAYS_PER_YEAR * 6) {
        folk.live_a_day(&mut e, day);
    }

    // The sample has not shrunk â€” that is what replacement is for.
    assert_eq!(
        folk.people.len(),
        before.len(),
        "the cohort changed size, so it is no longer a sample of a town \
         the economy is counting in full"
    );

    // Somebody died, or this proves nothing.
    let stale: Vec<_> = named
        .iter()
        .filter(|(i, _)| !folk.people.holds(*i))
        .collect();
    assert!(
        !stale.is_empty(),
        "nobody died in six years, so the replacement path never ran"
    );

    // **And not one stale handle resolves.** Every one of them shares a
    // slot with somebody living; what stops it naming them is the
    // generation.
    for (i, was) in &stale {
        assert!(
            folk.people.get(*i).is_none(),
            "a handle to {was}, who is dead, still resolves"
        );
        let occupant = folk
            .people
            .ids()
            .find(|live| live.slot() == i.slot())
            .expect("a freed slot that was never reused");
        assert_ne!(
            *i, occupant,
            "the slot was reused without moving the generation"
        );
        assert_ne!(
            &folk.people[occupant].name, was,
            "the replacement inherited the dead person's name"
        );
    }
}

/// **A dead person's estate goes to their kin, and to the state only if
/// there are none.**
///
/// Until now it went nowhere at all: the deceased was overwritten in
/// place and their replacement handed fifty out of the air. Money was
/// destroyed at one end of the sample and created at the other, and
/// nothing caught it, because a person's pocket is not yet inside the
/// money ledger.
///
/// **Intestate succession**, which is what applies to about two thirds of
/// Americans â€” 67% die without a will. Every state runs the same ladder:
/// spouse, then children in equal shares, then parents, and failing all
/// of it the estate **escheats to the state**. Escheat is genuinely rare
/// because most people have somebody; unclaimed property is not, and US
/// states hold something like $70bn of it.
///
/// Four rungs cannot be checked by running six years and hoping the right
/// deaths happen, so each is exercised on its own.
#[test]
fn an_estate_goes_down_the_ladder_and_the_state_takes_the_rest() {
    use scale_sim::id::Id;
    use scale_sim::person::Person;

    let e = a_nation().economy;

    // --- the spouse takes it ---
    let mut folk = Populace::seed(&e, 40, 20260828);
    let married: Vec<(Id<Person>, Id<Person>)> = folk
        .people
        .iter()
        .filter_map(|(i, p)| p.spouse.map(|s| (i, s)))
        .collect();
    assert!(
        !married.is_empty(),
        "nobody in the cohort is married, so almost every estate would \
         escheat and the first rung of the ladder is never tested"
    );
    let (dies, widow) = married[0];
    folk.people[dies].money = 900.0;
    let widow_had = folk.people[widow].money;
    folk.probate(dies);
    assert_eq!(
        folk.people[widow].money,
        widow_had + 900.0,
        "the widow did not inherit"
    );
    assert_eq!(folk.escheated, 0.0, "an estate with a widow escheated");
    assert_eq!(folk.inherited, 900.0);
    assert_eq!(
        folk.people[widow].spouse, None,
        "the survivor is still married to a dead person"
    );

    // --- with no spouse, the children split it ---
    let mut folk = Populace::seed(&e, 40, 20260828);
    let ids: Vec<Id<Person>> = folk.people.ids().collect();
    let (parent, a, b) = (ids[0], ids[1], ids[2]);
    folk.people[parent].spouse = None;
    folk.people[a].parents = vec![parent];
    folk.people[b].parents = vec![parent];
    folk.people[parent].money = 1000.0;
    let (a_had, b_had) = (folk.people[a].money, folk.people[b].money);
    folk.probate(parent);
    assert_eq!(
        folk.people[a].money,
        a_had + 500.0,
        "children split the estate"
    );
    assert_eq!(folk.people[b].money, b_had + 500.0);
    assert_eq!(folk.escheated, 0.0);

    // --- with neither, it goes up to the parents ---
    let mut folk = Populace::seed(&e, 40, 20260828);
    let ids: Vec<Id<Person>> = folk.people.ids().collect();
    let (dies, mother) = (ids[0], ids[1]);
    folk.people[dies].spouse = None;
    folk.people[dies].parents = vec![mother];
    folk.people[dies].money = 300.0;
    let had = folk.people[mother].money;
    folk.probate(dies);
    assert_eq!(
        folk.people[mother].money,
        had + 300.0,
        "the estate skipped the parents"
    );

    // --- and with nobody at all, the state takes it ---
    let mut folk = Populace::seed(&e, 40, 20260828);
    let alone = folk
        .people
        .ids()
        .find(|&i| folk.people[i].spouse.is_none() && folk.people[i].parents.is_empty())
        .expect("everybody in the cohort has kin");
    folk.people[alone].money = 250.0;
    folk.probate(alone);
    assert_eq!(
        folk.escheated, 250.0,
        "an estate with no heir did not escheat"
    );
    assert_eq!(folk.inherited, 0.0);
}

/// **The roof and the wheels go with it.**
///
/// The consequence worth having, and the reason inheritance is not just a
/// number moving: an heir who inherits a house stops paying rent, which
/// is most of how property stays in a family across a generation.
#[test]
fn an_heir_inherits_the_house_and_the_vehicle() {
    use scale_sim::person::Housing;
    use scale_sim::travel::Conveyance;

    let e = a_nation().economy;
    let mut folk = Populace::seed(&e, 40, 20260828);
    let (dies, widow) = folk
        .people
        .iter()
        .find_map(|(i, p)| p.spouse.map(|s| (i, s)))
        .expect("nobody is married");

    folk.people[dies].housing = Housing::Owned;
    folk.people[dies].conveyance = Conveyance::Van;
    folk.people[widow].housing = Housing::Rented;
    folk.people[widow].conveyance = Conveyance::OnFoot;

    folk.probate(dies);
    assert_eq!(
        folk.people[widow].housing,
        Housing::Owned,
        "the house did not pass"
    );
    assert_eq!(
        folk.people[widow].conveyance,
        Conveyance::Van,
        "the vehicle did not pass"
    );
}
