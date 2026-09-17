//! Specific trades: a ticket, a rota and a rate.
//!
//! A construction site is not one trade and a hospital is not one job. An
//! electrician is not a labourer who happens to be near a wire, and a care
//! assistant cannot do a nurse's work — which is the same rule this
//! economy already applies to medicines, where you do not anaesthetise
//! anybody with aspirin.

use scale_sim::econ::{Doctrine, DAYS_PER_YEAR};
use scale_sim::network::Network;
use scale_sim::person::{self, qualification_for, Qualification, Trade};
use scale_sim::polity::Polities;
use scale_sim::populace::Populace;
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

fn a_nation() -> Region {
    let world = World::generate(384, 216, 20260828);
    let pol = Polities::partition(&world, 30);
    let set = Settlements::place(&world, &pol, 5000);
    let net = Network::build(&world, &set, 1000);
    let id = pol.ranked()[2].0;
    Region::extract(&world, &pol, &set, &net, id, 5, Doctrine::Prudent).expect("a nation")
}

/// **The ticket is what makes a trade a trade.** You may not wire a house
/// or fit a gas appliance without one, and you certainly may not practise
/// medicine.
#[test]
fn a_trade_is_gated_and_that_is_the_point() {
    // Years of training, in order.
    assert_eq!(qualification_for(Trade::Doctor), Qualification::Degree);
    assert_eq!(qualification_for(Trade::Nurse), Qualification::Degree);
    assert_eq!(
        qualification_for(Trade::Electrician),
        Qualification::Vocational
    );
    assert_eq!(
        qualification_for(Trade::Pipefitter),
        Qualification::Vocational
    );
    // **The route into healthcare that a degree is not needed for**, which
    // is most of why it exists: a year of practical training against three
    // of university.
    assert_eq!(
        qualification_for(Trade::CareAssistant),
        Qualification::Vocational
    );
    assert!(qualification_for(Trade::CareAssistant) < qualification_for(Trade::Nurse));

    // And the trades anybody can walk into are still there, which is what
    // makes them pay what they pay.
    assert_eq!(qualification_for(Trade::Sales), Qualification::School);
    assert_eq!(
        qualification_for(Trade::ProductionWorker),
        Qualification::School
    );
}

/// **The ladder is the training.** Real UK medians against a ~£33k
/// economy-wide figure: a doctor ~£80k, an electrician ~£40k, a registered
/// nurse ~£37k, a healthcare assistant ~£24k.
#[test]
fn what_a_trade_pays_follows_what_it_took_to_enter() {
    let e = a_nation().economy;
    let rate = |t| person::day_rate(&e, 0, t);

    assert!(rate(Trade::Doctor) > rate(Trade::Nurse) * 1.5);
    assert!(rate(Trade::Nurse) > rate(Trade::CareAssistant));
    assert!(rate(Trade::Electrician) > rate(Trade::ProductionWorker));
    assert!(rate(Trade::Pipefitter) > rate(Trade::ProductionWorker));
    // A care assistant is close to the bottom of the wage scale, which is
    // a real and much-remarked fact about social care.
    assert!(rate(Trade::CareAssistant) < rate(Trade::Electrician));

    // Nothing is below subsistence: this project's own band is that a
    // day's work buys six to ten days of food.
    for t in [
        Trade::Doctor,
        Trade::Nurse,
        Trade::CareAssistant,
        Trade::Electrician,
        Trade::Pipefitter,
    ] {
        assert!(rate(t) > 0.0, "{} is paid nothing", t.name());
    }
}

/// **A hospital does not close on Sunday**, which is most of what makes
/// clinical work different from every other qualified job.
#[test]
fn the_week_is_not_the_same_for_every_trade() {
    use scale_sim::person::Employment;
    // Find a Saturday.
    let weekend = (0..14u64)
        .find(|&d| scale_sim::econ::Weekday::on(d).is_weekend())
        .expect("a weekend in a fortnight");

    // An office keeps Monday to Friday, and that is most of why people
    // want the job.
    assert_eq!(
        person::works_on(Trade::BusinessSpecialist, weekend, Employment::FullTime),
        0.0
    );
    // Clinical work does not stop.
    for t in [Trade::Doctor, Trade::Nurse, Trade::CareAssistant] {
        assert_eq!(
            person::works_on(t, weekend, Employment::FullTime),
            1.0,
            "{} got the weekend off",
            t.name()
        );
    }
    // A site keeps weekday hours; a call-out does not.
    let site = person::works_on(Trade::Electrician, weekend, Employment::FullTime);
    assert!(
        site > 0.0 && site < 1.0,
        "an electrician's weekend came out at {site}"
    );
}

/// **A qualified trade is more its own boss**, because the ticket is what
/// lets you work for yourself.
#[test]
fn a_ticket_is_what_lets_you_work_for_yourself() {
    let casual = |t| person::employment_mix(t).2;
    assert!(casual(Trade::Electrician) > casual(Trade::ProductionWorker));
    assert!(casual(Trade::Pipefitter) > casual(Trade::ProductionWorker));
    // Clinical work is salaried and permanent, and that security is a
    // real part of why people take the training.
    assert!(casual(Trade::Doctor) < casual(Trade::FoodService));
    assert!(person::employment_mix(Trade::Doctor).0 > 0.85);
}

/// **The population contains the trades the economy has posts for.**
///
/// `draw_trade` produced four trades and there are thirteen: `services.rs`
/// and `state.rs` between them create posts for over half the workforce —
/// offices, teaching, nursing, construction — and not one sampled person
/// could ever hold one.
#[test]
fn a_country_contains_the_people_it_needs() {
    let e = a_nation().economy;
    let folk = Populace::seed(&e, 200, 20260828);
    let n = folk.people.len() as f64;
    let share = |t: Trade| folk.people.values().filter(|p| p.trade == t).count() as f64 / n;

    // Every trade that has posts should have people, and the health
    // service is a third of a percent of nobody otherwise.
    for t in [
        Trade::Doctor,
        Trade::Nurse,
        Trade::CareAssistant,
        Trade::Electrician,
        Trade::Pipefitter,
        Trade::BusinessSpecialist,
        Trade::Teacher,
        Trade::Builder,
    ] {
        assert!(share(t) > 0.0, "a country with no {}", t.name());
    }

    // **Nurses outnumber doctors about three to one**, which is the real
    // shape of a health service: ~3 doctors per 1,000 people against ~9
    // nurses. Read off the country's jobs rather than a sample: a thousand
    // people hold five nurses and three doctors, and a ratio of two small
    // counts is a coin toss, which is the small-sample mistake this project
    // records over supervisors and over crash outcomes.
    let jobs = scale_sim::occupation::jobs_by_occupation(&e);
    let posts = |t: Trade| jobs.iter().map(|town| town[t.index()]).sum::<f64>();
    assert!(
        posts(Trade::Nurse) > posts(Trade::Doctor) * 1.8,
        "{:.0} nursing posts against {:.0} doctors'",
        posts(Trade::Nurse),
        posts(Trade::Doctor)
    );
    // **The professions want a degree, and cannot hold more people than
    // there are graduates.** Managers, accountants and the rest of the
    // business professions, computing, engineering, science and law —
    // every one degree-entry — together hold a real 26% of American jobs;
    // only about a third of adults hold a degree, so they come out below
    // their share of the posts. A real constraint rather than an artefact:
    // the skills shortage, and why a country can have vacancies it cannot
    // fill and people it cannot employ.
    let office: f64 = [
        Trade::Manager,
        Trade::Accountant,
        Trade::BusinessSpecialist,
        Trade::ComputingSpecialist,
        Trade::Engineer,
        Trade::Scientist,
        Trade::Legal,
    ]
    .iter()
    .map(|&t| share(t))
    .sum();
    assert!(
        (0.04..0.20).contains(&office),
        "the professions came out at {:.1}% of the workforce",
        office * 100.0
    );
    let graduates = folk
        .people
        .values()
        .filter(|p| p.qualification == Qualification::Degree)
        .count() as f64
        / n;
    assert!(
        office < graduates,
        "more office workers ({:.1}%) than graduates ({:.1}%), which cannot be",
        office * 100.0,
        graduates * 100.0
    );

    // **A world starts with its crews already run, and no more of them.**
    // Handing supervising out at the draw once put a town at 27%
    // supervisors against 9% of its posts. What a world starts with is its
    // crews' posts, held by somebody with a couple of years at the work.
    let jobs = scale_sim::occupation::jobs_by_occupation(&e);
    for m in 0..e.markets.len() {
        let mine: Vec<_> = folk.people.values().filter(|p| p.market == m).collect();
        let total: f64 = jobs[m].iter().sum();
        if mine.is_empty() || total <= 0.0 {
            continue;
        }
        for t in Trade::ALL {
            let running: Vec<_> = mine
                .iter()
                .filter(|p| p.trade == t && p.rank == scale_sim::person::Rank::Supervisor)
                .collect();
            let posts = (mine.len() as f64 * jobs[m][t.index()] / total * t.supervisor_share())
                .floor()
                + 1.0;
            assert!(
                running.is_empty() || t.crew().is_some(),
                "a world starts with somebody supervising {}, which has no such rung",
                t.name()
            );
            assert!(
                running.len() as f64 <= posts,
                "{} starts with {} supervisors of {} against room for {posts}",
                e.markets[m].name,
                running.len(),
                t.name()
            );
            for p in running {
                assert!(
                    p.age_years - 18.0 - p.qualification.years_to_earn() >= 2.0,
                    "{} starts out running a crew with no years at the work",
                    p.name
                );
            }
        }
    }
}

/// A vacancy is a number of posts, not a flag.
#[test]
fn promotion_is_capped_by_the_posts_that_exist() {
    // **A vacancy is a number of posts, not a flag**, and the number is the
    // crew: a town's employers' jobs in each kind of work, over that work's
    // published crew size.
    use scale_sim::person::Rank;
    let mut e = a_nation().economy;
    let mut folk = Populace::seed(&e, 60, 20260828);
    for day in 0..(DAYS_PER_YEAR * 4) {
        e.step();
        folk.live_a_day(&mut e, day);
    }
    let jobs = scale_sim::occupation::jobs_by_occupation(&e);
    let mut anybody = 0usize;
    for m in 0..e.markets.len() {
        let mine: Vec<_> = folk.people.values().filter(|p| p.market == m).collect();
        let total: f64 = jobs[m].iter().sum();
        if mine.len() < 10 || total <= 0.0 {
            continue;
        }
        let n = mine.len() as f64;
        // What each kind of work can carry, rounded up the one post a
        // fraction can come to, and not a head more — whatever the years
        // served.
        for t in Trade::ALL {
            let posts = (n * jobs[m][t.index()] / total * t.supervisor_share()).floor() + 1.0;
            let running = mine
                .iter()
                .filter(|p| p.trade == t && p.rank == Rank::Supervisor)
                .count();
            anybody += running;
            assert!(
                t.crew().is_some() || running == 0,
                "{} has somebody supervising {}, which has no such rung",
                e.markets[m].name,
                t.name()
            );
            assert!(
                running as f64 <= posts,
                "{} has {running} supervisors of {} among {n} people, whose jobs \
                 give that work room for {posts}",
                e.markets[m].name,
                t.name()
            );
        }
    }
    assert!(
        anybody > 0,
        "four years and no crew anywhere is run by anybody"
    );
}
