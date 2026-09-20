//! The housing ladder, and what owning does to a life.

use scale_sim::econ::{Doctrine, DAYS_PER_YEAR};
use scale_sim::network::Network;
use scale_sim::person::{self, Housing, Person, Trade};
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

/// **A house costs what it takes to build plus what the ground is worth**,
/// and the multiple of income that falls out is the real one.
#[test]
fn a_house_costs_about_eight_years_of_wages() {
    let e = a_nation().economy;
    for m in 0..e.markets.len() {
        let price = e.house_price(m);
        let wage = person::day_rate(&e, m, Trade::ProductionWorker);
        // Real UK house prices run about eight times median annual
        // earnings, up from four in the 1990s. Nothing here was tuned to
        // produce it: a dwelling is 76 m², its bill of materials comes
        // from `building.rs`, a wage is six days of food, and the
        // multiple is what those give.
        let years = price / (wage * 250.0);
        assert!(
            (5.0..14.0).contains(&years),
            "{} houses cost {years:.1} years of wages",
            e.markets[m].name
        );
    }

    // **And land is why a city is dearer.** The building is the same
    // building; the ground under it is not.
    let biggest = (0..e.markets.len())
        .max_by(|&a, &b| e.markets[a].population.total_cmp(&e.markets[b].population))
        .unwrap();
    let smallest = (0..e.markets.len())
        .min_by(|&a, &b| e.markets[a].population.total_cmp(&e.markets[b].population))
        .unwrap();
    assert!(
        e.house_price(biggest) > e.house_price(smallest),
        "a house in the biggest town should not cost the same as in the smallest"
    );
}

/// **Owning stops the rent**, which is the whole reason people want to.
#[test]
fn owning_stops_the_rent() {
    assert_eq!(Housing::Owned.share_of_rent(), 0.0);
    assert_eq!(Housing::Rented.share_of_rent(), 1.0);
    // A room in somebody else's place is about half a flat.
    assert_eq!(Housing::Lodging.share_of_rent(), 0.5);
    assert!(Housing::Lodging.share_of_rent() < Housing::Rented.share_of_rent());
}

/// The rungs are reachable in order, and each one costs what it should.
#[test]
fn the_ladder_goes_up_one_rung_at_a_time() {
    let mut e = a_nation().economy;
    for _ in 0..60 {
        e.step();
    }

    // Somebody with a great deal of money climbs the whole way.
    let mut rich = Person::new(
        "Moneybags",
        Trade::BusinessSpecialist,
        0,
        e.house_price(0) * 4.0,
    );
    rich.housing = Housing::Lodging;
    let mut saw_rented = false;
    for day in 0..400 {
        e.step();
        let d = e.ledger.day;
        person::live_a_day(&mut rich, &mut e, d);
        if rich.housing == Housing::Rented {
            saw_rented = true;
        }
        if rich.housing == Housing::Owned {
            break;
        }
        let _ = day;
    }
    assert!(saw_rented, "he should have taken a tenancy on the way");
    assert_eq!(
        rich.housing,
        Housing::Owned,
        "a man with four times the price of a house never bought one"
    );

    // **And his books still close.** Buying is spending, not conjuring.
    assert!(
        rich.money >= 0.0,
        "he bought a house he could not afford: {:.0}",
        rich.money
    );
}

/// **Without credit, a house is out of reach of ordinary pay, and not of
/// good pay.**
///
/// At eight or nine times an ordinary year's income, saving the price
/// outright while paying rent and eating takes most of a working life —
/// which is why real buyers use a mortgage. This used to say nobody buys at
/// all, and that was only true while every wage sat within a few days of
/// food of every other. With pay placed by real medians a manager earns
/// sixteen days of food a day against a production worker's six, and saves
/// the price in about a decade: measured over 25 years, nobody paid under
/// five days of food bought except the few who inherited or had moved down,
/// a quarter of office clerks did, and 15 of 17 managers did. Real outright
/// ownership is a fact about the well-off and the old, and so it is here.
///
/// **What credit adds is reach**, so ownership without it stays below what
/// the United States reaches with it — about 65% of households.
#[test]
fn without_credit_ownership_stays_out_of_reach() {
    let mut e = a_nation().economy;
    let mut folk = Populace::seed(&e, 40, 20260828);
    for day in 0..(DAYS_PER_YEAR * 25) {
        e.step();
        folk.live_a_day(&mut e, day);
    }
    let n = folk.people.len() as f64;
    let owned = folk
        .people
        .values()
        .filter(|p| p.housing == Housing::Owned)
        .count() as f64;
    let housed = folk
        .people
        .values()
        .filter(|p| p.housing != Housing::Homeless)
        .count() as f64;

    // **The price is years of ordinary pay**, which is what puts it out of
    // reach of most people and within reach of a few.
    //
    // **The real figure is 7.1 years** — a median owner-occupied home is
    // $332,700 *(Census, 2020-24)* against a production worker's $46,987 a
    // year *(OEWS May 2025)* — and this model straddles it rather than
    // sitting on it: 5.6 years in this fixture after twenty-five years
    // against 10.6-13.7 in the three soak worlds, because a house is
    // priced off its bill of materials while pay moves with the town's cost
    // of living and its labour market. The band was 6-12, fitted to a
    // production worker paid six days of food a day, which was itself the
    // defect. It is a sanity bound until housing answers supply and demand
    // (`docs/status.md` item 9), which is what will decide the level.
    let years = e.house_price(0) / (person::day_rate(&e, 0, Trade::ProductionWorker) * 260.0);
    assert!(
        (4.0..16.0).contains(&years),
        "a house costs {years:.1} years of a production worker's pay, which would          mean the price or the wage is wrong"
    );
    // **Ownership follows pay.**
    let pay = |own: bool| -> f64 {
        let m: Vec<_> = folk
            .people
            .values()
            .filter(|p| (p.housing == Housing::Owned) == own)
            .collect();
        m.iter().map(|p| p.trade.days_of_food_a_day()).sum::<f64>() / m.len().max(1) as f64
    };
    assert!(
        pay(true) > pay(false) * 1.2,
        "owners are paid {:.1} days of food against {:.1} for everybody else —          ownership has come loose from what people earn",
        pay(true),
        pay(false)
    );
    // **And without a mortgage it stays below what a mortgage reaches.**
    assert!(
        owned / n < 0.65,
        "{:.0}% own outright without anybody ever borrowing, which is more than          own with a mortgage market",
        owned / n * 100.0
    );
    // But most people do get and keep a roof, and a good share of them
    // get past a rented room to a place of their own.
    assert!(
        housed / n > 0.5,
        "only {:.0}% of a cohort had a roof after twenty-five years",
        housed / n * 100.0
    );
    let own_door = folk
        .people
        .values()
        .filter(|p| p.housing == Housing::Rented)
        .count() as f64;
    assert!(
        own_door > 0.0,
        "nobody in the whole cohort ever got a tenancy of their own"
    );
}
