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
        assert!(
            share < 0.30,
            "{} is {:.0}% supervisors after three years",
            e.markets[m].name,
            share * 100.0
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
