//! **Over a counter you pay; on a bill you can fall behind.**
//!
//! `consume_households` took the goods off the shelf whatever the balance
//! and put the shortfall on a counter, so a poor town ate like a rich one
//! on credit nobody extended (`docs/status.md` item 3). A till does not
//! extend credit.
//!
//! What these gates ask is not whether a number fell. It is whether a
//! delivery over a counter has a commercial outcome at all, whether the
//! *shape* of what is given up is the published one, and whether going
//! without for want of money is still distinguishable from a famine.

use scale_sim::econ::{Commodity, Economy, SiteKind};
use scale_sim::money::{Account, Why};
use scale_sim::slice::{self, Doctrine};

const FOOD: Commodity = Commodity::ProcessedFood;
const GOODS: Commodity = Commodity::RetailGoods;

fn settled() -> Economy {
    let mut e = slice::symmetric(Doctrine::Prudent);
    for _ in 0..40 {
        e.step();
    }
    e
}

/// Take every household's money out of the country, so the shelves are
/// full and nobody can pay. Money is moved rather than destroyed, so the
/// treasury's own conservation assertion still holds.
fn empty_the_pockets(e: &mut Economy) {
    let day = e.ledger.day;
    for m in 0..e.markets.len() {
        let had = e.treasury.balance(Account::Households(m)).max(0.0);
        e.treasury.pay(
            day,
            Account::Households(m),
            Account::Abroad,
            had,
            Why::Trade,
        );
    }
}

/// What the shops hold of one commodity.
fn on_the_shelves(e: &Economy, c: Commodity) -> f64 {
    (0..e.ledger.sites.len())
        .filter(|&s| e.ledger.sites[s].kind == SiteKind::Shop)
        .map(|s| e.ledger.stock(s, c))
        .sum()
}

fn wanted_today(e: &Economy, c: Commodity) -> f64 {
    (0..e.markets.len())
        .map(|m| e.markets[m].daily_household_demand(c))
        .sum()
}

/// **Nothing leaves a shelf that nobody paid for.**
///
/// The defect stated as a rule, and the discriminating construction is a
/// country whose shops are full and whose people are broke — because that
/// separates the two reasons somebody goes without.
#[test]
fn a_till_does_not_extend_credit() {
    let mut e = settled();
    assert!(
        on_the_shelves(&e, FOOD) > 0.0,
        "the fixture has no food to sell"
    );

    empty_the_pockets(&mut e);
    let before = on_the_shelves(&e, FOOD);
    e.step();

    let sold = before - on_the_shelves(&e, FOOD);
    let wanted = wanted_today(&e, FOOD);
    assert!(
        e.went_without[FOOD as usize] > 0.0,
        "a country with no money bought its whole basket"
    );
    assert!(
        sold < wanted,
        "goods left the shelves though nobody could pay: {sold:.1} t sold of {wanted:.1} t wanted"
    );
    e.treasury.assert_conserved();
}

/// **Going without for want of money is not a famine**, and the two
/// counters have to be able to say so.
///
/// `unmet_demand` is wanted and *not there*; `went_without` is wanted, on
/// the shelf, and unaffordable. Collapsing them would make a country whose
/// shops are full read as short of food, and every famine bound in this
/// project would stop meaning anything.
#[test]
fn a_full_shop_and_an_empty_pocket_is_not_a_shortage() {
    let mut e = settled();
    empty_the_pockets(&mut e);
    e.step();

    assert!(
        e.went_without[FOOD as usize] > 0.0,
        "nobody went without in a country with no money"
    );
    assert_eq!(
        e.unmet_demand[FOOD as usize], 0.0,
        "a town with full shelves was recorded as short of food"
    );
    assert!(
        on_the_shelves(&e, FOOD) > 0.0,
        "the shelves emptied, so this measures a shortage after all"
    );
}

/// **Engel's law is an output.**
///
/// Nothing writes down a share of spending. What is written down is the
/// published income elasticity of each thing — food 0.53, household
/// durables 1.29 *(BLS Consumer Expenditure Survey by income quintile)* —
/// and a household short of money therefore gives up proportionally more
/// of the furniture than of the dinner, which is what Engel's law says.
#[test]
fn the_furniture_goes_before_the_dinner() {
    let mut e = settled();
    // **Put the furniture on the shelf.** Neither fixture stocks retail
    // goods any more — both lost the import merchant they could not pay
    // for — and this gate is about the *order* things are given up in, so
    // it needs both on sale. Stocking it here says so rather than leaning
    // on a fixture that happens to have a depot.
    for site in 0..e.ledger.sites.len() {
        if e.ledger.sites[site].kind != SiteKind::Shop {
            continue;
        }
        let want = e.markets[e.ledger.sites[site].market].daily_household_demand(GOODS);
        e.ledger.sites[site].capacity[GOODS as usize] = want * 20.0;
        e.ledger.apply(
            &mut e.journal,
            scale_sim::econ::Event::Produced {
                site,
                commodity: GOODS,
                qty: want * 10.0,
            },
        );
    }
    empty_the_pockets(&mut e);
    // Hand back half a basket, so the money binds without being hopeless.
    let day = e.ledger.day;
    for m in 0..e.markets.len() {
        let basket: f64 = [FOOD, GOODS]
            .iter()
            .map(|&c| e.markets[m].daily_household_demand(c) * e.markets[m].price[c as usize])
            .sum();
        e.treasury.pay(
            day,
            Account::Abroad,
            Account::Households(m),
            basket * 0.5,
            Why::Trade,
        );
    }
    e.step();

    let share = |c: Commodity| e.went_without[c as usize] / wanted_today(&e, c);
    let (food, goods) = (share(FOOD), share(GOODS));
    assert!(
        food > 0.0 && goods > 0.0,
        "nothing was given up, so there is no ordering to read: food {food:.3} goods {goods:.3}"
    );
    // **A strict inequality here passes on float noise**, which is this
    // project's own oldest allocation lesson arriving at a gate. Flatten
    // every elasticity to one and the two shares come out
    // 0.66827339183828371 and 0.66827339183828383 — identical but for the
    // last bit, and `goods > food` was *true*. So the claim is a material
    // gap: comfortably above the hundredth of a per cent the allocation
    // already treats as a tie, and far below the 1.52 measured here, so it
    // discriminates without sitting on the reading.
    assert!(
        goods > food * 1.05,
        "a household short of money gave up about as much of its dinner as of \
         its furniture: food {food:.6}, goods {goods:.6}"
    );
}

/// **The order is read off a table, not chosen.**
///
/// `e = ln(top quintile / bottom quintile) / ln(3.79)`, and the
/// consequence that matters is the one nobody expects: **medicine is the
/// least income-elastic thing a household buys** — the top fifth spend 42%
/// more on drugs than the bottom fifth while spending nearly four times as
/// much altogether.
#[test]
fn medicine_is_the_last_thing_anybody_gives_up() {
    let e = |c: Commodity| c.till_elasticity().expect("not bought at a till");
    assert!(
        e(Commodity::Remedies) < e(Commodity::Meat),
        "medicine was given up before meat"
    );
    assert!(
        e(Commodity::Meat) < e(FOOD),
        "meat was given up before the staple"
    );
    assert!(
        e(FOOD) < 1.0,
        "food's income elasticity is not under one, which is Engel's law itself"
    );
    assert!(
        e(GOODS) > 1.0,
        "household durables are not a luxury, which the CE table says they are"
    );
    // The gap that carries the model is the one to durables, not the hairs
    // between the three protected categories.
    assert!(
        e(GOODS) > 2.0 * e(FOOD),
        "durables are not sharply more elastic than food"
    );
}

/// **A bill is not a till.**
///
/// Electricity has a household demand and no till elasticity, and that is
/// a claim rather than an omission: `utility.rs` bills a month in arrears
/// and establishes that nobody is cut off the day they cannot pay. A
/// hospital sends an invoice for the same reason. Groceries are neither.
#[test]
fn the_power_stays_on_in_a_town_that_cannot_pay() {
    // How much the households of the whole country actually drew, read off
    // the journal rather than off a flag — a gate asserting only that the
    // counter stayed at nought would pass on the power never flowing.
    let drawn_by_households = |e: &Economy| -> f64 {
        e.journal
            .recent(e.ledger.day, 1)
            .filter_map(|entry| match entry.event {
                scale_sim::econ::Event::Consumed {
                    commodity: Commodity::Electricity,
                    qty,
                    reason: scale_sim::econ::Use::Household,
                    ..
                } => Some(qty),
                _ => None,
            })
            .sum()
    };

    let mut flush = settled();
    flush.step();
    let with_money = drawn_by_households(&flush);
    assert!(
        with_money > 0.0,
        "the fixture draws no household power, so this proves nothing"
    );

    let mut broke = settled();
    empty_the_pockets(&mut broke);
    broke.step();

    assert!(
        drawn_by_households(&broke) >= with_money * 0.999,
        "a town with no money drew less power, which is not how a utility \
         bills or collects: {} against {with_money}",
        drawn_by_households(&broke)
    );
    assert_eq!(
        broke.went_without[Commodity::Electricity as usize],
        0.0,
        "power was recorded as gone without at a counter it is not sold over"
    );
}

/// **Every commodity a household buys is either at a till or is a bill**,
/// and which one is stated rather than left to whether somebody remembered.
///
/// The rule this project already keeps for storage — a site with no
/// `capacity` entry for one of its own inputs can never receive a tonne of
/// it, and fails silently — arriving at the counter. A new household
/// commodity with no elasticity would be bought whatever the balance, and
/// nothing downstream could see it.
#[test]
fn nothing_a_household_buys_is_left_unaccounted_for() {
    // Household demands that are billed rather than handed over a counter:
    // a month of usage, three weeks to pay, a notice, a winter rule.
    const BILLED: [Commodity; 1] = [Commodity::Electricity];
    for &c in Commodity::ALL.iter() {
        if c.per_capita_annual() <= 0.0 {
            assert!(
                c.till_elasticity().is_none(),
                "{c:?} is not bought by households and carries a till elasticity"
            );
            continue;
        }
        assert!(
            c.till_elasticity().is_some() || BILLED.contains(&c),
            "{c:?} is bought by households and is neither at a till nor named \
             as billed, so it would be taken whatever the balance"
        );
        // **And not both.** They are contradictory claims about one
        // commodity: a thing handed over a counter is not also a thing you
        // are billed a month in arrears for, and a commodity carrying both
        // would be throttled at the till *and* excused at the meter.
        assert!(
            !(c.till_elasticity().is_some() && BILLED.contains(&c)),
            "{c:?} is both sold over a counter and billed in arrears"
        );
    }
}
