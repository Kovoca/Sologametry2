//! **What the hand-built fixtures are, and that they stay what they are.**
//!
//! A fixture is an instrument, and an instrument with a fault in it
//! measures the fault. Both of this project's fixtures have been caught
//! living on an endowment — a power station opening on twenty thousand
//! tonnes of coal with nothing to refill it — so every gate that ran them
//! past the day the pile ran out was reading a dark country.

use scale_sim::econ::{Commodity, Doctrine, Economy};
use scale_sim::money::Account;
use scale_sim::slice;

fn run(e: &mut Economy, days: u64) {
    for _ in 0..days {
        e.step();
    }
}

fn cannery_ran(e: &Economy) -> f64 {
    e.ledger.sites[slice::site(e, slice::Role::AshfordCannery)].ran
}

/// **The two-town fixture keeps its lights on.**
///
/// It went dark on day 253: the station burnt its opening pile and nothing
/// dug more, so electricity sat at the 12,000 cap from then on, the cannery
/// stopped, and Bexley's income and spending both went to nought — which
/// read as a distressed town when it was the whole country.
///
/// Four hundred days is chosen to be well past that day, and the reading is
/// the last hundred, so a fixture that is merely *going* dark more slowly
/// still has to be lit at the end.
#[test]
fn the_two_town_fixture_does_not_run_out_of_fuel() {
    let mut e = slice::build(Doctrine::Prudent);
    run(&mut e, 20);
    let ordinary = e.price(slice::ASHFORD, Commodity::Electricity);
    let canning = cannery_ran(&e);
    run(&mut e, 280);
    for _ in 0..100 {
        e.step();
        assert!(
            e.unserved_power == 0.0,
            "day {}: {:.1} of load unserved in an undisturbed fixture",
            e.ledger.day,
            e.unserved_power
        );
    }
    let late = e.price(slice::ASHFORD, Commodity::Electricity);
    assert!(
        late < ordinary * 2.0,
        "electricity at {late:.1} on day 400 against {ordinary:.1} on day 20"
    );
    assert!(
        cannery_ran(&e) > canning * 0.5,
        "the cannery runs {:.1} on day 400 against {canning:.1} on day 20",
        cannery_ran(&e)
    );
    e.ledger.assert_conserved();
}

/// **What declines is a town, not the country.**
///
/// Bexley is kept as the fixture's distressed town on purpose: twenty-six
/// thousand people, one shop, no works and no payroll, living on what that
/// shop makes. A world has to be able to contain one, and this is the claim
/// that it does — Bexley spends more than it takes in over its last hundred
/// days and runs its savings down, **while Ashford's cannery runs and the
/// grid carries the whole load**.
///
/// It is a claim about what the fixture *is*. Should a later change make
/// Bexley pay its way — a commuting link, a works of its own — this goes red
/// and the fixture has changed character, which is worth knowing rather than
/// discovering in some other gate's reading.
#[test]
fn bexley_declines_while_the_country_works() {
    let mut e = slice::build(Doctrine::Prudent);
    run(&mut e, 300);
    let before = e.treasury.balance(Account::Households(slice::BEXLEY));
    let mut income = 0.0;
    let mut paid = 0.0;
    for _ in 0..100 {
        e.step();
        assert_eq!(e.unserved_power, 0.0, "the country went dark, not the town");
        for t in &e.treasury.today {
            if t.to == Account::Households(slice::BEXLEY) {
                income += t.amount;
            }
            if t.from == Account::Households(slice::BEXLEY) {
                paid += t.amount;
            }
        }
    }
    let after = e.treasury.balance(Account::Households(slice::BEXLEY));
    assert!(
        paid > income && after < before,
        "Bexley took in {income:.0} and paid {paid:.0} over days 300-399, purse \
         {before:.0} -> {after:.0}: it is not declining"
    );
    assert!(
        cannery_ran(&e) > 0.0,
        "Ashford's cannery has stopped, so this is not a town declining"
    );
}
