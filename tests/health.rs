//! **Who pays for medicine, and what follows from it.**
//!
//! Before this, every nation on every planet ran the British arrangement:
//! the state paid the hospitals in full and nobody else paid anything, so
//! an illness could not cost a household a penny and a country could not
//! contain an uninsured man. Real health financing is a three-way split
//! and the shares differ enormously — 82% government in Britain against
//! 55% in the United States, and 45% out of pocket in India.
//!
//! Each of these gates was checked by breaking the thing it names.

use scale_sim::econ::{Doctrine, Economy, SiteKind};
use scale_sim::money::Account;
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Region;
use scale_sim::state::HealthSystem;

fn a_nation() -> Region {
    let world = scale_sim::world::World::generate(384, 216, 20260828);
    let polities = Polities::partition(&world, 24);
    let settlements = scale_sim::settlement::Settlements::place(&world, &polities, 3000);
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

/// The same country under a different arrangement, run far enough in that
/// nothing is still settling.
fn a_country_that(system: HealthSystem, days: u64) -> Economy {
    let mut e = a_nation().economy;
    for gov in e.governments.values_mut() {
        gov.health = system;
    }
    for _ in 0..days {
        e.step();
    }
    // **A policy is a fact about the country, not about the day.** Asserted
    // rather than re-applied every morning: re-applying it would be a guard
    // against something that cannot happen today and would quietly hide the
    // day a `govern` re-founding started resetting it.
    for gov in e.governments.values() {
        assert_eq!(
            gov.health, system,
            "a country's health policy was reset while the days ran"
        );
    }
    e
}

/// What the hospitals of a country were paid today, by whom.
fn who_paid(e: &Economy) -> (f64, f64, f64) {
    let hospitals: std::collections::BTreeSet<usize> = e
        .ledger
        .sites
        .iter()
        .enumerate()
        .filter(|(_, s)| s.kind == SiteKind::Hospital)
        .map(|(i, _)| i)
        .collect();
    let (mut state, mut insurers, mut pocket) = (0.0, 0.0, 0.0);
    for t in e.treasury.today.iter() {
        let Account::Firm(site) = t.to else { continue };
        if !hospitals.contains(&site) {
            continue;
        }
        match t.from {
            Account::State(_) => state += t.amount,
            Account::ServiceSector(_) => insurers += t.amount,
            Account::Households(_) => pocket += t.amount,
            _ => {}
        }
    }
    (state, insurers, pocket)
}

/// **The country decides who settles the bill, and the money says so.**
///
/// Asserted on what actually moved between accounts rather than on the
/// constants — the same world four times over, one thing varying.
/// Sabotaged by giving every system the same shares, it names the
/// arrangement whose money did not follow it.
#[test]
fn who_pays_the_hospital_is_what_the_country_decided() {
    for system in HealthSystem::ALL {
        let e = a_country_that(system, 40);
        let (state, insurers, pocket) = who_paid(&e);
        let total = state + insurers + pocket;
        assert!(
            total > 0.0,
            "{}: nobody paid the hospitals at all",
            system.name()
        );
        let (want_public, want_insured, want_pocket) = system.shares();
        let got = (state / total, insurers / total, pocket / total);
        let off = (got.0 - want_public)
            .abs()
            .max((got.1 - want_insured).abs())
            .max((got.2 - want_pocket).abs());
        assert!(
            off < 0.06,
            "{}: the bill was settled {:.0}/{:.0}/{:.0} by the state, the insurers and the \
             patient against a policy of {:.0}/{:.0}/{:.0}",
            system.name(),
            got.0 * 100.0,
            got.1 * 100.0,
            got.2 * 100.0,
            want_public * 100.0,
            want_insured * 100.0,
            want_pocket * 100.0,
        );
    }
}

/// **A hospital is paid the same whoever pays for it.**
///
/// The split is about who finds the money, not about how much medicine
/// there is: a ward staffed the same and using the same drugs costs the
/// same in Manchester and in Memphis, and if it does not then the policy
/// has quietly become a discount. Every real difference in what a country
/// spends on health belongs to prices and to how much care is bought —
/// which is a separate change and is named in `docs/status.md` rather
/// than smuggled in here.
#[test]
fn a_hospital_is_paid_the_same_whoever_pays_for_it() {
    let mut takings = Vec::new();
    for system in HealthSystem::ALL {
        let e = a_country_that(system, 40);
        let (a, b, c) = who_paid(&e);
        takings.push((system, a + b + c));
    }
    let most = takings.iter().map(|&(_, v)| v).fold(0.0f64, f64::max);
    let least = takings.iter().map(|&(_, v)| v).fold(f64::MAX, f64::min);
    assert!(least > 0.0, "a country whose hospitals were paid nothing");
    assert!(
        most / least < 1.20,
        "the same hospitals took {most:.3e} under one arrangement and {least:.3e} under \
         another — the split has become a discount"
    );
}

/// **A state that does not pay for medicine does not tax for it.**
///
/// It raised the whole of the hospital bill and paid only its own share of
/// it, so the exchequer hoarded: over a year a state's balance went 2.36e9
/// to 4.09e9 — thirty-three days of its own wage bill piling up with
/// nothing to spend it on — and `tests/money.rs` caught it.
///
/// The real difference is the same one: total government revenue is about
/// **27% of GDP in the United States against 39% in the United Kingdom**
/// *(OECD)*, and a large part of that gap is who buys the medicine.
///
/// **The claim is the trend, not the level.** A state begins with an
/// issued balance like everybody else, so what says it is over-collecting
/// is that the pile grows — and sabotaged, by raising the whole bill
/// again, it does.
#[test]
fn a_state_that_does_not_pay_for_medicine_does_not_tax_for_it() {
    let mut takes = Vec::new();
    for system in [HealthSystem::TaxFunded, HealthSystem::PrivateInsurance] {
        let mut e = a_country_that(system, 40);
        let held = |e: &Economy| -> f64 {
            e.nations()
                .into_iter()
                .map(|n| e.treasury.balance(Account::State(n)))
                .sum()
        };
        let opening = held(&e);
        for _ in 0..200 {
            e.step();
        }
        let closing = held(&e);
        let outgoings: f64 = e
            .treasury
            .today
            .iter()
            .filter(|t| matches!(t.from, Account::State(_)))
            .map(|t| t.amount)
            .sum();
        let tax: f64 = e
            .treasury
            .today
            .iter()
            .filter(|t| matches!(t.why, scale_sim::money::Why::Tax))
            .map(|t| t.amount)
            .sum();
        assert!(tax > 0.0, "{}: a state raising nothing", system.name());
        assert!(
            (closing - opening) < outgoings * 20.0,
            "{}: the exchequer grew {:.3e} over two hundred days, {:.0} days of its own \
             outgoings — it is taxing for something it is not buying",
            system.name(),
            closing - opening,
            (closing - opening) / outgoings.max(1e-9)
        );
        takes.push(tax);
    }
    assert!(
        takes[1] < takes[0],
        "the state that buys 55% of the medicine raised {:.3e} against {:.3e} for the one \
         that buys 77% — it is taxing for a service it does not provide",
        takes[1],
        takes[0]
    );
}

/// **And a state does not spend what it did not raise.** The mirror of the
/// gate above, and the half this file recorded as missing: `Treasury::pay`
/// lets a state overdraw by design, so nothing stopped the exchequer paying
/// out more than it taxed for.
///
/// It happened. Hospitals came to bill their costs over a margin, the
/// payment read the new bill and the tax read its own copy of the old one,
/// and over five years three worlds' states ended **3.5-5.8e10 below
/// nought** — money that was never raised, reaching households as
/// hospital dividends. What discriminates is the direction: the exchequer
/// must not *fall* by more than a few weeks of its own outgoings, the same
/// bar the hoarding gate sets on it rising.
#[test]
fn a_state_does_not_spend_what_it_did_not_raise() {
    for system in [HealthSystem::TaxFunded, HealthSystem::PrivateInsurance] {
        let mut e = a_country_that(system, 40);
        let held = |e: &Economy| -> f64 {
            e.nations()
                .into_iter()
                .map(|n| e.treasury.balance(Account::State(n)))
                .sum()
        };
        let opening = held(&e);
        for _ in 0..200 {
            e.step();
        }
        let closing = held(&e);
        let outgoings: f64 = e
            .treasury
            .today
            .iter()
            .filter(|t| matches!(t.from, Account::State(_)))
            .map(|t| t.amount)
            .sum();
        assert!(outgoings > 0.0, "{}: a state paying nobody", system.name());
        assert!(
            opening - closing < outgoings * 20.0,
            "{}: the exchequer fell {:.3e} over two hundred days, {:.0} days of its own \
             outgoings — it is paying for something it did not tax for",
            system.name(),
            opening - closing,
            (opening - closing) / outgoings
        );
    }
}

/// **What a health service delivers is what somebody paid for.**
///
/// It used to read the budget line — what the *state* could afford —
/// which is the whole answer only where the state pays the whole bill.
/// And it barely moved: `funded[Health]` is a cover ratio worked out once
/// when the country is founded and does not depend on the arrangement at
/// all, so every country on every planet read the same figure.
///
/// The discriminating case is a **state that cannot collect**, which is
/// the commonest kind there is. Break the exchequer and a tax-funded
/// service loses 77% of its money while a privately insured one loses
/// 55%, so the wards must come out differently. That is also the real
/// feedback loop: it is exactly why poor countries end up paying at the
/// door.
///
/// Sabotaged back to the budget line, the two read identically.
#[test]
fn what_a_health_service_delivers_is_what_somebody_paid_for() {
    let mut delivered = Vec::new();
    for system in [HealthSystem::TaxFunded, HealthSystem::PrivateInsurance] {
        let mut e = a_nation().economy;
        for gov in e.governments.values_mut() {
            gov.health = system;
            // **A state that cannot reach its own economy.** Real
            // effective takes run 10-18% where control is thin against
            // 35-50% in a developed state.
            gov.capacity = scale_sim::state::Capacity::Weak;
        }
        for _ in 0..60 {
            e.step();
        }
        let n = e.nations()[0];
        delivered.push((system, e.governments[&n].health_delivered()));
    }
    let tax_funded = delivered[0].1;
    let privately = delivered[1].1;
    assert!(
        tax_funded < 0.95,
        "a state that cannot collect and buys every ward's medicine still delivers {:.0}%",
        tax_funded * 100.0
    );
    assert!(
        privately > tax_funded + 0.08,
        "with the same broke exchequer, the tax-funded service delivered {:.0}% and the \
         privately insured one {:.0}% — the wards are reading a budget line rather than \
         what anybody paid",
        tax_funded * 100.0,
        privately * 100.0
    );
}

/// **Every bill is paid by somebody.** The three shares are a partition,
/// not three independent dials, so a country whose shares do not sum to
/// one is one where medicine is free or is billed twice.
#[test]
fn the_three_payers_account_for_the_whole_bill() {
    for system in HealthSystem::ALL {
        let (a, b, c) = system.shares();
        assert!(
            (a + b + c - 1.0).abs() < 1e-9,
            "{}: {:.2} + {:.2} + {:.2} of a bill",
            system.name(),
            a,
            b,
            c
        );
        assert!(
            a > 0.0 && b >= 0.0 && c > 0.0,
            "{}: a country where one of the three payers does not exist",
            system.name()
        );
    }
    // And the arrangements are actually different from each other, or
    // there is one system wearing four names.
    let states: Vec<f64> = HealthSystem::ALL.iter().map(|s| s.shares().0).collect();
    let spread = states.iter().cloned().fold(0.0f64, f64::max)
        - states.iter().cloned().fold(f64::MAX, f64::min);
    assert!(
        spread > 0.15,
        "the state's share runs over a range of {spread:.2}, which is one arrangement \
         wearing four names"
    );
}
