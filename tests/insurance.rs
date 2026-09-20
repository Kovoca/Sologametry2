//! **An insurer is a firm**: premiums in, claims out, and people whose job
//! is to settle them.
//!
//! Before this, a premium left a pocket and reached nobody and a claim was
//! a bill that shrank rather than a payment from somebody's reserves — the
//! same shape as a haulier's revenue that was accumulated and paid to no
//! one. What these gates hold is that the money has both ends, and that
//! the employment follows the work rather than a share somebody typed in.

use scale_sim::econ::Doctrine;
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Region;
use scale_sim::services::{self, Sector};
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

fn a_nation() -> Region {
    let world = World::generate(384, 216, 20260828);
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

/// **What an insurer employs follows what there is to settle.**
///
/// The chain is all real figures and nothing is typed in at the end of it:
/// 0.87 vehicles a head *(FHWA 2024, 297,525,836 registered)*, 11.41
/// claims per hundred vehicles a year *(ISO 2024)*, one adjuster per 110
/// claims and two clerks to every three adjusters *(OEWS May 2025:
/// 324,230 and 214,260)*, plus an agent or underwriter per 735 policies
/// *(479,100 and 105,420)*.
///
/// It is asserted against **the real employment share**, not against the
/// model's own constant: 1.12 million people in the four insurance
/// occupations against 155.5 million in work is 0.72%.
#[test]
fn an_insurer_employs_the_people_its_claims_need() {
    let e = a_nation().economy;
    let svc = e
        .services
        .as_ref()
        .expect("a nation with no service sector");
    let people: f64 = e.markets.iter().map(|m| m.population).sum();
    let posts: f64 = (0..e.markets.len())
        .map(|m| svc.posts_in(m, Sector::Insurance))
        .sum();
    let share = posts / (people * 0.5);
    assert!(
        (0.005..0.009).contains(&share),
        "insurance employs {:.2}% of everybody in work against a real 0.72%",
        share * 100.0
    );

    // **And it is the claims that put them there.** Recomputed from the
    // chain: an industry whose staffing did not follow its claims would
    // not move when a claim took twice as long to settle.
    let claims_a_head = services::VEHICLES_A_HEAD * services::CLAIMS_A_VEHICLE_A_YEAR;
    let settling = claims_a_head / services::CLAIMS_AN_ADJUSTER_A_YEAR
        * (1.0 + services::CLERKS_TO_AN_ADJUSTER);
    assert!(
        settling / (services::INSURANCE_SHARE_OF_EMPLOYMENT * 0.5) > 0.4,
        "settling claims is {:.0}% of the industry, which is not an insurer",
        settling / (services::INSURANCE_SHARE_OF_EMPLOYMENT * 0.5) * 100.0
    );
}

/// **A book that pays out more than it takes in cannot employ anybody.**
///
/// Claims are 56% of premiums here, derived from the frequencies and
/// severities rather than chosen — real auto loss ratios run 65-70%, and
/// the difference is what this model pays its claims staff out of. So what
/// is left after claims has to cover the wage bill, or the sector is being
/// run at a loss that somebody else is quietly funding.
#[test]
fn what_is_left_after_the_claims_pays_the_people_who_settle_them() {
    let e = a_nation().economy;
    let svc = e
        .services
        .as_ref()
        .expect("a nation with no service sector");
    let mut premiums = 0.0;
    let mut wages = 0.0;
    for m in 0..e.markets.len() {
        premiums += e.premiums_a_day(m);
        wages += svc.posts_in(m, Sector::Insurance) * e.day_rate_here(m);
    }
    let kept = premiums * (1.0 - scale_sim::person::CLAIMS_SHARE_OF_PREMIUM);
    assert!(premiums > 0.0, "a country paying nothing for cover");
    assert!(
        kept > wages,
        "premiums leave {kept:.3e} a day after claims against a wage bill of {wages:.3e} — \
         the insurer cannot pay the people settling the claims"
    );
    // And not absurdly the other way either: an insurer that keeps five
    // times its wage bill is not an insurer, it is a toll.
    assert!(
        kept < wages * 6.0,
        "premiums leave {kept:.3e} against wages of {wages:.3e}, which is a margin no \
         insurance market survives"
    );
}

/// **One premium, two scales.** What a town pays per vehicle has to be
/// what a person with a van pays, or the aggregate and the sampled people
/// are insuring different worlds.
#[test]
fn a_town_pays_per_vehicle_what_a_person_pays_for_one() {
    use scale_sim::person::{self, Person, Trade};
    use scale_sim::travel::Conveyance;

    let e = a_nation().economy;
    let vehicles = e.markets[0].population * services::VEHICLES_A_HEAD;
    let town = e.premiums_a_day(0) * scale_sim::econ::DAYS_PER_YEAR as f64 / vehicles;

    let mut driver = Person::new("Reg", Trade::Driver, 0, 0.0);
    driver.conveyance = Conveyance::Van;
    let his = person::premium_a_year(&e, &driver);

    let gap = (town - his).abs() / his.max(1e-9);
    assert!(
        gap < 0.35,
        "a town pays {town:.0} a vehicle a year and a man with a van pays {his:.0} — \
         two scales insuring different worlds"
    );
}
