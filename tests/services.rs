//! The rest of what people do for a living.

use scale_sim::econ::Doctrine;
use scale_sim::person::Trade;
use scale_sim::services::{Sector, Services};

fn a_nation() -> scale_sim::region::Region {
    let world = scale_sim::world::World::generate(384, 216, 20260828);
    let pol = scale_sim::polity::Polities::partition(&world, 30);
    let set = scale_sim::settlement::Settlements::place(&world, &pol, 5000);
    let net = scale_sim::network::Network::build(&world, &set, 1000);
    let id = pol.ranked()[2].0;
    scale_sim::region::Region::extract(&world, &pol, &set, &net, id, 5, Doctrine::Prudent)
        .expect("a nation to model")
}

#[test]
fn somebody_has_to_fix_things_and_somewhere_has_to_be_open() {
    // **The economy modelled the goods and about a quarter of the jobs.**
    // Real UK employment: professional and technical 8.9%, administrative
    // and support 8.7%, accommodation and food 6.8%, construction 6.4%,
    // information 4.5%, finance 3.4%, arts and recreation 2.5% — some 43%
    // of everybody, and none of it existed.
    //
    // Two of those decide what a place is like to *live* in rather than
    // merely to eat in: somebody has to fix things, and somewhere has to
    // be open in the evening.
    let e = a_nation().economy;
    let s = e
        .services
        .as_ref()
        .expect("a nation with no services in it");
    let g = e.government.as_ref().expect("a nation with no state in it");

    let private = s.share_of_workforce(&e);
    assert!(
        (0.25..0.50).contains(&private),
        "private services employ {:.0}% of the workforce, against a real ~43%",
        private * 100.0
    );

    // Together with the state this is most of a modern workforce — which
    // is the point, because farms, mills and shops are not.
    let together = private + g.share_of_workforce(&e);
    assert!(
        together > 0.40,
        "services and state together are only {:.0}% of the workforce",
        together * 100.0
    );

    // Every sector actually employs somebody.
    for sector in Sector::ALL {
        let n: f64 = (0..e.markets.len()).map(|m| s.posts_in(m, sector)).sum();
        assert!(
            n > 0.0,
            "{} employs nobody in the whole nation",
            sector.name()
        );
    }

    // Offices are the largest block and construction is a real one — real
    // shares are 21% and 6.4%.
    let total: f64 = (0..e.markets.len()).map(|m| s.total_in(m)).sum();
    let office: f64 = (0..e.markets.len())
        .map(|m| s.posts_in(m, Sector::Office))
        .sum();
    assert!(
        office / total > 0.4,
        "offices are only {:.0}% of service employment",
        office / total * 100.0
    );
}

#[test]
fn what_a_sector_pays_and_how_securely_it_employs() {
    // The trades differ in more than name. **Hospitality is the
    // worst-paid sector there is** — about £20k against a £33k median —
    // and the least secure: 28.8% of that workforce is on zero-hours
    // contracts, the highest of any industry and fourteen times public
    // administration's 2.1%. Offices are the best paid, which is why
    // people move to cities for them.
    use scale_sim::person::employment_mix;

    let casual = |t: Trade| employment_mix(t).2;
    assert!(
        casual(Trade::Hospitality) > casual(Trade::Office) * 5.0,
        "hospitality at {:.0}% casual is no less secure than office work at {:.0}%",
        casual(Trade::Hospitality) * 100.0,
        casual(Trade::Office) * 100.0
    );
    assert!(
        casual(Trade::Hospitality) > casual(Trade::Public) * 5.0,
        "hospitality is as secure as public administration"
    );

    // Construction carries high self-employment, because the job ends.
    assert!(
        casual(Trade::Builder) > casual(Trade::Office),
        "a builder's work is as continuous as an office's"
    );
}

#[test]
fn offices_concentrate_and_a_kitchen_does_not() {
    // **Not every sector spreads evenly.** Professional and technical work
    // concentrates in cities and barely exists in a village; construction
    // and hospitality follow people wherever they are. Which is a real
    // thing about where you have to move to in order to work at
    // something.
    let mut e = a_nation().economy;
    // A village and a city, held against each other.
    e.markets[0].population = 2_000.0;
    e.markets[1].population = 900_000.0;
    let s = Services::provide(&e);

    let per_head = |m: usize, sector: Sector| s.posts_in(m, sector) / e.markets[m].population;
    assert!(
        per_head(1, Sector::Office) > per_head(0, Sector::Office) * 1.3,
        "a village has as much office work per head as a city"
    );
    // A kitchen and a building site do not care.
    let hosp = (
        per_head(0, Sector::Hospitality),
        per_head(1, Sector::Hospitality),
    );
    assert!(
        (hosp.0 - hosp.1).abs() < hosp.1 * 0.05,
        "hospitality concentrates in cities, which it does not"
    );
}
