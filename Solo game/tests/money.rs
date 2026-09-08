//! Money, and whether it circulates rather than appearing and vanishing.

use scale_sim::econ::{Doctrine, DAYS_PER_YEAR};
use scale_sim::money::{Account, Treasury, Why};
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

fn a_nation(seed: u64, rank: usize) -> Region {
    let world = World::generate(384, 216, seed);
    let pol = Polities::partition(&world, 30);
    let set = Settlements::place(&world, &pol, 5000);
    let net = Network::build(&world, &set, 1000);
    let id = pol.ranked()[rank].0;
    Region::extract(&world, &pol, &set, &net, id, 5, Doctrine::Prudent).expect("a nation")
}

/// **Nothing appears and nothing vanishes**, which is the same guarantee
/// the commodity ledger gives for tonnage and for the same reason.
#[test]
fn money_is_conserved() {
    let mut e = a_nation(20260828, 2).economy;
    let issued = e.treasury.total();
    assert!(issued > 0.0, "a country with no money in it");

    for _ in 0..(DAYS_PER_YEAR * 3) {
        e.step();
        e.treasury.assert_conserved();
    }
    let after = e.treasury.total();
    assert!(
        (after - issued).abs() < issued * 1e-9,
        "{issued:.0} was issued and {after:.0} is in existence"
    );
    // And the goods ledger is unaffected by any of it.
    e.ledger.assert_conserved();
}

/// **It circulates.** Money that all ends up in one place is not a
/// currency, it is a leak with extra steps.
#[test]
fn the_circuit_closes() {
    let mut e = a_nation(20260828, 2).economy;
    let issued = e.treasury.total();

    let held = |e: &scale_sim::econ::Economy| {
        let hh: f64 = (0..e.markets.len())
            .map(|m| e.treasury.balance(Account::Households(m)))
            .sum();
        let firms: f64 = (0..e.ledger.sites.len())
            .map(|i| e.treasury.balance(Account::Firm(i)))
            .sum();
        (hh, firms, e.treasury.balance(Account::State))
    };

    for _ in 0..DAYS_PER_YEAR {
        e.step();
    }
    let (hh1, firms1, state1) = held(&e);
    for _ in 0..(DAYS_PER_YEAR * 2) {
        e.step();
    }
    let (hh3, firms3, state3) = held(&e);

    // **Households must still have money after three years.** They were
    // drained inside a fortnight when wages were the only flow into them
    // and every purchase was a flow out — the residual a firm makes is
    // somebody's income too, and without it the circuit does not close.
    assert!(
        hh3 > issued * 0.01,
        "households hold {hh3:.0} of {issued:.0} after three years"
    );
    // Steady, not merely non-zero.
    assert!(
        (hh3 - hh1).abs() < hh1 * 0.5,
        "household balances went {hh1:.0} -> {hh3:.0}, which is not a circuit"
    );
    // **And nobody hoards.** A firm holds working capital, not a fortune,
    // and a state that collects more than it spends is a bug rather than a
    // policy — a tax rate is quoted against value added, and turnover
    // counts the same value at every step of a chain.
    assert!(
        firms3 < issued * 0.05,
        "firms are sitting on {firms3:.0} of {issued:.0}"
    );
    // **Steady as well as small**, the same test households and the
    // treasury get. Working capital that drifts over two years is money
    // accumulating or leaking somewhere in the circuit, and a level check
    // alone cannot see it.
    assert!(
        (firms3 - firms1).abs() < firms1.max(1.0) * 0.5,
        "firm balances went {firms1:.0} -> {firms3:.0} over two years"
    );
    assert!(
        (state3 - state1).abs() < state1.max(1.0) * 0.25,
        "the treasury went {state1:.0} -> {state3:.0}: it is hoarding or bleeding"
    );
}

/// **A wage is an expense somebody bears.** That is the whole point: a
/// firm can only fail to pay one once paying it is real.
#[test]
fn wages_come_out_of_a_firms_own_balance() {
    let mut e = a_nation(20260828, 2).economy;
    for _ in 0..30 {
        e.step();
    }

    let payroll: f64 = e
        .treasury
        .today
        .iter()
        .filter(|t| t.why == Why::Payroll)
        .map(|t| t.amount)
        .sum();
    assert!(payroll > 0.0, "nobody was paid a wage today");

    // **Every wage is paid by an employer and to a household**, never
    // conjured. An employer is a firm with premises or the pooled service
    // sector of a town — construction, hospitality, recreation and offices
    // are 37% of employment and have no premises in this model, because a
    // service is consumed where the people are and cannot be shipped.
    for t in e.treasury.today.iter().filter(|t| t.why == Why::Payroll) {
        assert!(
            matches!(t.from, Account::Firm(_) | Account::ServiceSector(_)),
            "a wage was paid by {}",
            t.from.name()
        );
        assert!(
            matches!(t.to, Account::Households(_)),
            "a wage was paid to {}",
            t.to.name()
        );
    }

    // **And wages are most of what households live on**, which they were
    // not until the service sector had a payroll: profit was doing four
    // fifths of the work of getting money to people. Real household income
    // is about 60% wages, 20% profit and rent, 20% transfers.
    let into_households = |why: Why| -> f64 {
        e.treasury
            .today
            .iter()
            .filter(|t| t.why == why && matches!(t.to, Account::Households(_)))
            .map(|t| t.amount)
            .sum::<f64>()
    };
    let earned = into_households(Why::Payroll) + into_households(Why::PublicSpending);
    let unearned = into_households(Why::Profit);
    assert!(
        earned > unearned * 0.7,
        "wages are only {:.0} against {unearned:.0} of profit, which is a \
         rentier economy rather than a working one",
        earned
    );

    // The state pays its own, out of what it collected.
    let public: f64 = e
        .treasury
        .today
        .iter()
        .filter(|t| t.why == Why::PublicSpending)
        .map(|t| t.amount)
        .sum();
    let tax: f64 = e
        .treasury
        .today
        .iter()
        .filter(|t| t.why == Why::Tax)
        .map(|t| t.amount)
        .sum();
    assert!(public > 0.0, "a state that employs nobody");
    assert!(
        (public - tax).abs() < tax * 0.02,
        "the state collected {tax:.0} and spent {public:.0}"
    );
}

/// A payer cannot pay what it does not have, and the shortfall is the
/// point rather than an inconvenience.
#[test]
fn a_firm_cannot_pay_what_it_has_not_got() {
    let mut t = Treasury::new();
    t.open(Account::Firm(0), 100.0);
    t.open(Account::Households(0), 0.0);

    let paid = t.pay(
        1,
        Account::Firm(0),
        Account::Households(0),
        250.0,
        Why::Payroll,
    );
    assert_eq!(paid, 100.0, "it should pay what it has");
    assert_eq!(t.balance(Account::Firm(0)), 0.0);
    assert_eq!(t.balance(Account::Households(0)), 100.0);
    assert!(
        (t.unpaid - 150.0).abs() < 1e-9,
        "the shortfall should be recorded, not waived: {}",
        t.unpaid
    );
    t.assert_conserved();

    // Money is never created by paying, however hard anybody tries.
    for _ in 0..100 {
        t.pay(
            2,
            Account::Firm(0),
            Account::Households(0),
            1e9,
            Why::Payroll,
        );
    }
    t.assert_conserved();
    assert_eq!(t.total(), 100.0);
}
