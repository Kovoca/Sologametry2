//! **The border answers the balance, and a deficit is not a
//! disequilibrium.**
//!
//! Every import and every export used to settle against a fixed world
//! price, so a world that bought three times what it sold went on doing it
//! for ever and the money left: over 700 days on one planet 5.06e10 paid
//! out against 1.80e10 taken in, and households drained from 4.53e10 to
//! 2.44e10.
//!
//! Two mechanisms answer it, and they answer different halves. A floating
//! rate makes imports dearer and exports better paid until the balance
//! stops widening; a capital account carries the part that remains, which
//! is what lets a deficit persist — as the United States' has every year
//! since 1976 and the United Kingdom's since 1984.

use scale_sim::econ::{Commodity, Doctrine, Economy};
use scale_sim::exchange::Exchange;
use scale_sim::money::{Account, Why};
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Nations;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

fn a_world(seed: u64, nations: usize) -> Economy {
    let world = World::generate(384, 216, seed);
    let polities = Polities::partition(&world, 24);
    let settlements = Settlements::place(&world, &polities, 3000);
    let network = Network::build(&world, &settlements, 500);
    Nations::build(
        &world,
        &polities,
        &settlements,
        &network,
        nations,
        4,
        Doctrine::Prudent,
    )
    .economy
}

/// **A deficit nobody will fund makes the money worth less.**
///
/// This world buys about three times what it sells, which on the measure
/// the rate reads is an imbalance around +0.5 — three times the most
/// persistent real one there is. Nobody funds that, so the currency has to
/// go, and the direction is the whole claim: foreign money gets dearer,
/// which is what makes imports dear and exports worth having.
#[test]
fn a_deficit_nobody_funds_makes_the_money_worth_less() {
    let mut e = a_world(7, 4);
    assert!(
        (e.exchange.foreign_money() - 1.0).abs() < 1e-12,
        "a world should start at par"
    );
    for _ in 0..400 {
        e.step();
    }
    assert!(
        e.exchange.imbalance() > 0.3,
        "this fixture is meant to be a heavy importer and its balance is {:+.3}",
        e.exchange.imbalance()
    );
    assert!(
        e.exchange.foreign_money() > 1.05,
        "four hundred days of buying three times what it sells and foreign money \
         still costs {:.4}",
        e.exchange.foreign_money()
    );
}

/// **An ordinary deficit is funded, and funded means nothing moves.**
///
/// The correction that mattered: the first version chased the balance to
/// zero, and real economies do not have a zero balance. On 2024 trade the
/// United States sits at +0.126 on this measure and China at −0.161, both
/// for decades, because somebody is willing to hold the other side. So an
/// imbalance inside the funded band must leave the rate exactly where it
/// was **and** hand the whole net back as a claim.
#[test]
fn an_ordinary_deficit_is_funded_and_moves_nothing() {
    // The United States on 2024 goods and services, in billions.
    let (imports, exports) = (4110.0, 3192.0);
    let mut x = Exchange::at_par();
    let mut lent = 0.0;
    for _ in 0..(365 * 5) {
        lent += x.settle(imports, exports);
    }
    let b = x.imbalance();
    assert!(
        (b - 0.126).abs() < 0.005,
        "the measure should read the real figure: {b:+.4} against +0.126"
    );
    assert!(
        (x.foreign_money() - 1.0).abs() < 1e-12,
        "five years of a real deficit moved the rate to {:.6}",
        x.foreign_money()
    );
    // Every penny of it came back as somebody's claim, which is the
    // identity: current + capital = 0.
    let net = (imports - exports) * 365.0 * 5.0;
    assert!(
        (lent - net).abs() < net * 1e-9 && (x.owed_abroad() - net).abs() < net * 1e-9,
        "the funded deficit has to arrive as a claim: lent {lent:.1} and owed {:.1} \
         against a net {net:.1}",
        x.owed_abroad()
    );
}

/// **And a deficit too big to fund is only partly carried.**
///
/// A sudden stop is this share falling — Thailand in 1997, Argentina in
/// 2001, Greece in 2010 — and what is not carried is money that genuinely
/// leaves.
#[test]
fn a_deficit_past_what_anybody_will_hold_is_only_partly_carried() {
    let mut x = Exchange::at_par();
    for _ in 0..400 {
        x.settle(3.0, 1.0);
    }
    let b = x.imbalance();
    assert!(
        (b - 0.5).abs() < 1e-6,
        "the measure reads {b:+.4}, not +0.5"
    );
    let share = x.funded_share();
    assert!(
        (share - 0.3).abs() < 0.01,
        "a third of an imbalance of a half is funded, not {share:.3}"
    );
    assert!(
        x.unfunded() > 0.3,
        "and the rest is what the rate answers: {:.3}",
        x.unfunded()
    );
}

/// **Dearer foreign money turns importers into exporters.**
///
/// Marshall-Lerner, which is the reason a depreciation closes anything at
/// all: a rate improves the balance only if the two sides respond by more
/// than one for one between them. Here they do, and by construction rather
/// than by an elasticity typed in — both decisions are a price against a
/// parity that moves with the rate, so a town crosses out of importing and
/// into exporting as it goes.
#[test]
fn dearer_foreign_money_turns_importers_into_exporters() {
    let mut e = a_world(7, 4);
    for _ in 0..120 {
        e.step();
    }
    let count = |e: &Economy| {
        let mut buy = 0;
        let mut sell = 0;
        for m in 0..e.markets.len() {
            for &c in Commodity::ALL.iter() {
                if e.worth_importing(m, c) {
                    buy += 1;
                }
                if e.worth_exporting(m, c) {
                    sell += 1;
                }
            }
        }
        (buy, sell)
    };
    e.exchange = Exchange::at(1.0);
    let (buy_at_par, sell_at_par) = count(&e);
    e.exchange = Exchange::at(2.0);
    let (buy_dear, sell_dear) = count(&e);

    assert!(
        buy_at_par > 0,
        "nothing was worth importing even at par, so the gate proves nothing"
    );
    assert!(
        buy_dear < buy_at_par,
        "doubling the price of foreign money left {buy_dear} town-and-commodity pairs \
         worth importing against {buy_at_par} at par"
    );
    assert!(
        sell_dear > sell_at_par,
        "and {sell_dear} worth exporting against {sell_at_par} at par — a depreciation \
         has to pay exporters better or it closes nothing"
    );
}

/// **The band cannot close, whatever the rate.**
///
/// One side of the border adds every cost of moving a tonne and the other
/// takes it away, so no price can make a town worth importing into and
/// worth exporting from at once — which is the money printer the first
/// border had to be patched against. Scaling the world price by a rate
/// multiplies both sides of that, so the property has to survive a collapse
/// and a boom alike.
#[test]
fn the_band_cannot_close_at_any_rate() {
    let mut e = a_world(7, 4);
    for _ in 0..60 {
        e.step();
    }
    let mut checked = 0;
    for rate in [0.2, 0.5, 1.0, 2.0, 5.0] {
        e.exchange = Exchange::at(rate);
        for m in 0..e.markets.len() {
            for &c in Commodity::ALL.iter() {
                if c.sea_freight().is_none() {
                    continue;
                }
                let (import, export) = (e.import_parity(m, c), e.export_parity(m, c));
                assert!(
                    export < import,
                    "at a rate of {rate} the band closed on {c} in {}: export parity \
                     {export:.3} against import parity {import:.3}",
                    e.markets[m].name
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 100, "only {checked} pairs were examined");
}

/// **What the world is owed is what it actually lent.**
///
/// The stock and the flow are two accounts of one thing, and a model that
/// keeps them apart can have them disagree — which is the whole class of
/// defect the ledger's conservation assertion exists to catch, arriving at
/// the capital account. Every claim recorded has to be a payment somebody
/// made.
#[test]
fn what_the_world_is_owed_is_what_it_actually_lent() {
    let mut e = a_world(7, 4);
    let mut moved = 0.0;
    for _ in 0..300 {
        e.step();
        for t in e.treasury.today.iter() {
            if t.why != Why::Capital {
                continue;
            }
            match (t.from, t.to) {
                (Account::Abroad, _) => moved += t.amount,
                (_, Account::Abroad) => moved -= t.amount,
                _ => panic!("a capital flow that does not touch the outside world"),
            }
        }
    }
    let owed = e.exchange.owed_abroad();
    assert!(
        moved > 0.0,
        "nothing was lent at all in three hundred days, so the gate proves nothing"
    );
    assert!(
        (owed - moved).abs() < owed.abs() * 1e-6,
        "the world is recorded as owed {owed:.6e} and was actually paid {moved:.6e}"
    );
}

/// **A merchant does not land what nobody wants.**
///
/// `worth_importing` answers yes or no, and that used to decide whether a
/// terminal landed its whole rated tonnage or nothing — so a quay that was
/// worth opening opened all the way, every day, until the price came back
/// down to parity. The country ended up carrying **three and a half times
/// what it aims at**: goods in this fixture sat at 49.8 days of cover
/// against a target of 15.
///
/// A supply curve fixes the level as well as the limit cycle it was
/// introduced for, because the tonnage offered now falls as the price
/// approaches parity instead of staying at full tilt until it crosses.
#[test]
fn a_merchant_does_not_land_what_nobody_wants() {
    let mut e = scale_sim::slice::symmetric(Doctrine::Prudent);
    e.logistics = Some(scale_sim::logistics::Logistics::found(&e));
    for _ in 0..400 {
        e.step();
    }
    // What this fixture actually lands from abroad, read off the recipes.
    let mut landed: Vec<Commodity> = Vec::new();
    for s in 0..e.ledger.sites.len() {
        let Some(r) = e.ledger.sites[s].recipe else {
            continue;
        };
        if !scale_sim::econ::RECIPES[r].from_abroad {
            continue;
        }
        for &(c, q) in scale_sim::econ::RECIPES[r].outputs {
            if q > 0.0 && !landed.contains(&c) {
                landed.push(c);
            }
        }
    }
    assert!(
        !landed.is_empty(),
        "this fixture lands nothing from abroad, so the gate proves nothing"
    );
    for c in landed {
        for m in 0..e.markets.len() {
            let held: f64 = (0..e.ledger.sites.len())
                .filter(|&s| e.ledger.sites[s].market == m)
                .map(|s| e.ledger.stock(s, c))
                .sum();
            let draw = e.daily_draw(m, c);
            if draw <= 1e-9 {
                continue;
            }
            let cover = held / draw;
            let target = e.target_cover(m, c);
            assert!(
                cover < target * 2.0,
                "{c} in {} sits on {cover:.1} days against a target of {target:.1} — \
                 a quay that is worth opening is not worth opening all the way",
                e.markets[m].name
            );
        }
    }
}
