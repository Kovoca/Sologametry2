//! What the state raises, spends, and employs.

use scale_sim::econ::{Doctrine, DAYS_PER_YEAR};
use scale_sim::person::Trade;
use scale_sim::state::{Capacity, Government, Service};

fn a_nation(d: Doctrine) -> scale_sim::region::Region {
    let world = scale_sim::world::World::generate(384, 216, 20260828);
    let pol = scale_sim::polity::Polities::partition(&world, 30);
    let set = scale_sim::settlement::Settlements::place(&world, &pol, 5000);
    let net = scale_sim::network::Network::build(&world, &set, 1000);
    let id = pol.ranked()[2].0;
    scale_sim::region::Region::extract(&world, &pol, &set, &net, id, 5, d)
        .expect("a nation to model")
}

#[test]
fn the_state_is_the_largest_employer_there_is() {
    // **The labour market had a hole in it the size of the public sector.**
    // Real government employment is 14-21% of the workforce — UK 17%, US
    // 14%, France 21% — and none of it existed. A nation's people could
    // work a farm, a mill, a cannery, a mine or a shop, which is about a
    // tenth of what people actually do, and there was nowhere at all for
    // the other nine tenths to go.
    let e = a_nation(Doctrine::Prudent).economy;
    let g = e.government.as_ref().expect("a nation with no state in it");

    let share = g.share_of_workforce(&e);
    assert!(
        (0.10..0.25).contains(&share),
        "the state employs {:.0}% of the workforce, against a real 14-21%",
        share * 100.0
    );

    // Health, education and administration are each about a fortieth of
    // the population and dwarf the uniformed services — which is the real
    // shape of a modern state and not what most people picture.
    let nationally = |s: Service| -> f64 {
        (0..e.markets.len()).map(|m| g.posts_for(&e, m, s)).sum()
    };
    assert!(
        nationally(Service::Health) > nationally(Service::Defence) * 5.0,
        "more soldiers than nurses"
    );
    assert!(
        nationally(Service::Education) > nationally(Service::Safety) * 3.0,
        "more police than teachers"
    );
}

#[test]
fn a_state_that_cannot_collect_cannot_provide() {
    // **A weak state cannot tax what it cannot reach**, which is a real
    // feedback loop and the reason weak states stay weak. Real effective
    // takes: 35-50% for a high-capacity developed state, 20-30%
    // middle-income, 10-18% where control is thin.
    let e = a_nation(Doctrine::Prudent).economy;

    let strong = Government::govern(&e, Capacity::Developed);
    let weak = Government::govern(&e, Capacity::Weak);

    assert!(
        weak.revenue < strong.revenue * 0.5,
        "a weak state raises {:.0} against a developed one's {:.0}",
        weak.revenue,
        strong.revenue
    );
    assert!(
        weak.funded[0] < 0.8,
        "a state collecting a seventh of its economy funds its services in full"
    );
    assert!(
        weak.share_of_workforce(&e) < strong.share_of_workforce(&e) * 0.6,
        "the weak state employs nearly as many people as the strong one"
    );
    // Under-funding shows up as fewer people, not as a worse multiplier
    // somewhere. A half-funded school has half the teachers.
    assert!(weak.posts.iter().sum::<f64>() < strong.posts.iter().sum::<f64>());
}

#[test]
fn public_work_is_steady_and_shop_work_is_not() {
    // The character of the job, not just its existence. **A school does
    // not send half its staff home because trade was slow**, so public
    // work is posted in weeks where shop work is a day at a time — and
    // that difference is most of what it means to have a steady job.
    //
    // Real: about 60% of UK retail is part-time and part-timers average
    // two and a half to three days; the public sector is overwhelmingly
    // permanent and full-time.
    let mut e = a_nation(Doctrine::Prudent).economy;
    let mut folk = scale_sim::populace::Populace::seed(&e, 50, 20260828);
    for (i, p) in folk.people.iter_mut().enumerate() {
        if i % 6 == 0 {
            p.trade = Trade::Public;
            // Qualified for it: a trade you cannot enter is not a trade
            // you are in, and the gate is the point of the qualification.
            p.qualification = scale_sim::person::qualification_for(Trade::Public);
        }
    }
    for day in 0..DAYS_PER_YEAR {
        e.step();
        folk.live_a_day(&mut e, day);
    }

    let worked_share = |t: Trade| -> f64 {
        let mine: Vec<_> = folk.people.iter().filter(|p| p.trade == t).collect();
        if mine.is_empty() {
            return f64::NAN;
        }
        let d: u64 = mine.iter().map(|p| p.days_worked).sum();
        d as f64 / (DAYS_PER_YEAR * mine.len() as u64) as f64
    };
    let public = worked_share(Trade::Public);
    let shop = worked_share(Trade::Shopworker);
    assert!(
        public > 0.6,
        "public service found work on only {:.0}% of days",
        public * 100.0
    );
    assert!(
        public > shop * 1.4,
        "public work at {:.0}% of days is no steadier than shop work at {:.0}%",
        public * 100.0,
        shop * 100.0
    );

    // And everything still balances: the state pays out of tax, and a week
    // teaching moves no tonnage — which is exactly what a service is.
    e.ledger.assert_conserved();
}

#[test]
fn a_below_replacement_birth_rate_is_only_destiny_if_nobody_pays() {
    // **The line that decides whether demographic decline is a fact or a
    // choice.** Real family spending runs from **0.6% of GDP in the United
    // States to about 4% in France**, OECD average 2%; Denmark, France,
    // Hungary, Sweden and the UK are all above 3.5% while Japan, Korea,
    // Spain and the US are under 1.5%.
    //
    // A state that leaves a nursery place at 65% of a wage gets the
    // maternal employment and the birth rate that implies. Sweden caps
    // what a parent pays at about 3% of income and gets both back.
    let e = a_nation(Doctrine::Prudent).economy;

    let strong = Government::govern(&e, Capacity::Developed);
    let weak = Government::govern(&e, Capacity::Weak);

    let borne_strong = strong.childcare_borne_by_parents();
    let borne_weak = weak.childcare_borne_by_parents();
    assert!(
        borne_strong < borne_weak * 0.6,
        "a funded family policy leaves parents bearing {:.0}% against an unfunded {:.0}%",
        borne_strong * 100.0,
        borne_weak * 100.0
    );

    // In real terms: 65% of a wage unsupported, and the Nordic figure is
    // a few percent.
    let real_cost = |g: &Government| 0.65 * g.childcare_borne_by_parents();
    assert!(
        real_cost(&strong) < 0.20,
        "a well-funded state still leaves childcare at {:.0}% of a wage",
        real_cost(&strong) * 100.0
    );
    assert!(
        real_cost(&weak) > 0.35,
        "a state funding a third of its family policy still makes childcare cheap"
    );

    // **And it employs people.** Nursery ratios of one adult to three
    // under-twos are why this costs what it does, and why it is a real
    // block of jobs rather than a cash transfer.
    let staff: f64 = (0..e.markets.len())
        .map(|m| strong.posts_for(&e, m, Service::Family))
        .sum();
    assert!(staff > 0.0, "family support that employs nobody");

    // The honest caveat, recorded rather than modelled away: **spending
    // does not simply buy births.** OECD fertility fell from 1.8 to 1.7
    // between 2009 and 2017 across countries spending heavily, and Korea
    // has cheap childcare and the lowest fertility on earth. What family
    // spending reliably buys is that a parent can *work*.
    assert!(
        Service::Family.peacetime_share() > 0.015
            && Service::Family.peacetime_share() < 0.045,
        "family spending is outside the real 0.6-4% of GDP range"
    );
}
