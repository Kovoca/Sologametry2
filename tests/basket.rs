//! **A household does not want a washing machine. It wants clean clothes.**
//!
//! Every gate here is a way a demand model lies: by buying a second cooker
//! when the first one works, by replacing what should have been mended, by
//! filling an order nobody could supply, by summing food and furniture into
//! one number, or by making a household in Miami spend a Minneapolis
//! winter's fuel bill.

use scale_sim::basket::*;
use scale_sim::basket::Route;
use scale_sim::item::{standard_catalogue, Catalogue, ItemInstance, Placement, Store};

fn home_of(who: Roster) -> Household {
    Household::new(who, Climate::temperate(), Placement::Ground { locality: 1, x: 0, y: 0 })
}

/// A town with a shop that stocks the ordinary things, and a repairer.
fn a_town(cat: &Catalogue) -> Market {
    let shopkeeper = 1u64;
    let mut m = Market { repairer: Some(45.0), power: Some(0.41), ..Default::default() };
    for (name, price) in [
        ("washing machine", 700.0),
        ("cooking stove", 900.0),
        ("refrigerator", 800.0),
        ("space heater", 90.0),
        ("electric light", 25.0),
        ("bed", 500.0),
        ("bicycle", 400.0),
        ("motor car", 9_000.0),
        ("cooking pot", 40.0),
        ("wash tub", 30.0),
        ("water butt", 70.0),
        ("coat", 120.0),
        ("work clothes", 60.0),
        ("telephone", 600.0),
        ("radio set", 80.0),
    ] {
        m.stock(Stall::new(cat.must(name), price, 5, shopkeeper));
    }
    m.services = vec![
        (Need::Nutrition, 8.5),
        (Need::Health, 2.0),
        (Need::Water, 0.4),
        (Need::CleanClothes, 6.0),
        (Need::Mobility, 4.0),
        (Need::CookedFood, 9.0),
    ];
    m
}

fn comfortable() -> Means {
    Means { income: 900.0, savings: 6_000.0, credit: 2_000.0 }
}

fn poor() -> Means {
    Means { income: 210.0, savings: 15.0, credit: 0.0 }
}

fn give(home: &mut Household, store: &mut Store, cat: &Catalogue, name: &str) -> scale_sim::id::Id<ItemInstance> {
    let id = store.add(ItemInstance::one(cat, cat.must(name)), home.home);
    home.owns.push(id);
    id
}

// =====================================================================
// 1-2: what you already have, and what is wrong with it
// =====================================================================

/// **Gate 1: a working durable prevents a purchase.**
///
/// The single most important thing a stock model does and the one an
/// annual-consumption model cannot do at all. A household with a sound
/// machine is not in the market for one, however much money it has.
#[test]
fn a_household_with_a_working_machine_does_not_buy_another() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let market = a_town(&cat);
    let mut home = home_of(Roster::of(2));
    give(&mut home, &mut store, &cat, "washing machine");

    let intent = what_to_do(
        Need::CleanClothes,
        1.0,
        &home,
        &store,
        &cat,
        &market,
        comfortable(),
    );
    assert_eq!(intent, Intent::Nothing, "it went shopping with a working machine at home");

    // And over a whole week of ordering, nothing is bought for it.
    let mut market = market;
    let out = a_period(&mut home, &mut store, &cat, &mut market, comfortable(), 7, 1);
    assert!(
        !out.bought.iter().any(|(o, _)| o.need == Need::CleanClothes),
        "a second washing machine turned up in the week"
    );

    // **Money is not what stops it.** The same household with no machine
    // and the same money does buy one.
    let mut bare = home_of(Roster::of(2));
    let bought = a_period(&mut bare, &mut store, &cat, &mut market, comfortable(), 7, 2);
    assert!(
        bought.bought.iter().any(|(o, _)| o.need == Need::CleanClothes),
        "a household with money and no machine did not buy one"
    );
}

/// **Gate 2: a failure creates repair demand before it creates a sale.**
///
/// Nobody replaces a machine the moment it stops. The repair trade is real
/// and it is the first thing a broken appliance produces — which is also
/// why a town with no repairer throws away what a town with one mends.
#[test]
fn a_broken_machine_is_mended_before_it_is_replaced() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let market = a_town(&cat);
    let mut home = home_of(Roster::of(2));
    let machine = give(&mut home, &mut store, &cat, "washing machine");
    store.get_mut(machine).unwrap().condition.damage = 0.55;

    let intent = what_to_do(Need::CleanClothes, 1.0, &home, &store, &cat, &market, comfortable());
    assert_eq!(intent, Intent::Repair(machine), "it scrapped a mendable machine");

    // **A town with nobody to mend things buys a new one instead** — the
    // same household, the same fault, a different place.
    let mut no_repairer = a_town(&cat);
    no_repairer.repairer = None;
    let instead =
        what_to_do(Need::CleanClothes, 1.0, &home, &store, &cat, &no_repairer, comfortable());
    assert_eq!(instead, Intent::BuyNew(cat.must("washing machine")));

    // **And a worn-out machine is not worth mending however cheap the
    // repairer is.** Real repair shops turn this work away.
    store.get_mut(machine).unwrap().condition.wear = 0.95;
    let scrapped = what_to_do(Need::CleanClothes, 1.0, &home, &store, &cat, &market, comfortable());
    assert!(
        !matches!(scrapped, Intent::Repair(_)),
        "it paid to mend a machine at the end of its life"
    );
}

// =====================================================================
// 3-4: the second-hand trade, and nobody selling
// =====================================================================

/// **Gate 3: a second-hand one satisfies the same need.**
///
/// Which is the whole reason a second-hand market exists — the service is
/// identical and the price is not.
#[test]
fn a_used_machine_washes_the_same_clothes() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let washer = cat.must("washing machine");
    let mut market = a_town(&cat);
    market.stock(Stall::second_hand(washer, 180.0, 1, 2));

    let mut home = home_of(Roster::of(2));
    // Enough to reach a used machine after the essentials, and nowhere
    // near enough for a new one. **The essentials come first**, which is
    // why this needs stating: with a smaller purse it correctly buys a tub
    // instead, having spent the money on food, warmth and a bed.
    let purse = Means { income: 400.0, savings: 2_400.0, credit: 0.0 };
    let intent = what_to_do(Need::CleanClothes, 1.0, &home, &store, &cat, &market, purse);
    assert_eq!(intent, Intent::BuyUsed(washer), "it paid full price with a used one on offer");

    let out = a_period(&mut home, &mut store, &cat, &mut market, purse, 7, 3);
    let spent: f64 = out.bought.iter().filter(|(o, _)| o.need == Need::CleanClothes).map(|(_, p)| *p).sum();
    assert!((spent - 180.0).abs() < 1e-9, "it did not pay the second-hand price: {spent}");

    // **And it does the job**, which is the point: the need is met and
    // nothing is outstanding.
    assert_eq!(
        provision_for(Need::CleanClothes, 1.0, &home.owns, &store, &cat).met(),
        true,
        "a second-hand machine washed nothing"
    );
    // It is not as good, though. Less life left in it.
    let bought = home
        .owns
        .iter()
        .find(|&&id| store.get(id).map(|i| i.definition) == Some(washer))
        .copied()
        .expect("it did not end up with a washing machine at all");
    assert!(store.get(bought).unwrap().condition.wear > 0.3, "a used machine came out box-fresh");
}

/// **Gate 4: nobody selling means unmet demand, not a phantom purchase.**
///
/// The failure this whole module exists to make impossible. A country with
/// no cookers has households cooking on fires, and a model that quietly
/// completes the sale cannot say so.
#[test]
fn a_want_nobody_can_supply_is_recorded_as_a_want() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let mut market = a_town(&cat);
    // The stove is on the price list and there are none in the country.
    for s in market.stalls.iter_mut() {
        if s.def == cat.must("cooking stove") {
            s.stock = 0;
        }
    }
    // And nobody sells cooked food either, or the household would just buy
    // its dinner.
    market.services.retain(|(n, _)| *n != Need::CookedFood);
    let mut home = home_of(Roster::of(2));

    let out = a_period(&mut home, &mut store, &cat, &mut market, comfortable(), 7, 4);

    assert!(
        !out.bought.iter().any(|(o, _)| matches!(
            o.intent,
            Intent::BuyNew(d) | Intent::BuyUsed(d) if d == cat.must("cooking stove")
        )),
        "it bought a stove out of an empty shop"
    );
    // It falls back to a pot, which is what a household without a stove
    // does — and the pot is really for sale, so that is a real purchase.
    let met = provision_for(Need::CookedFood, 1.0, &home.owns, &store, &cat).met()
        || out.by_hand.iter().any(|(n, _)| *n == Need::CookedFood);
    assert!(met, "the household simply stopped eating cooked food");

    // Now take away every way of doing it and the demand has to surface.
    let mut nothing = Market { repairer: None, ..Default::default() };
    nothing.services = vec![(Need::Nutrition, 12.0)];
    let mut bare = home_of(Roster::of(2));
    bare.free_hours = 0.5;
    let out = a_period(&mut bare, &mut store, &cat, &mut nothing, comfortable(), 7, 5);
    assert!(out.bought.iter().all(|(o, _)| o.need == Need::Nutrition), "it bought from an empty market");
    assert!(
        out.without.contains(&Need::CookedFood) || out.unmet.iter().any(|o| o.need == Need::CookedFood),
        "an unsupplied need vanished instead of being recorded"
    );
    assert!(bare.without_for(Need::CookedFood) > 0, "going without was forgotten by the next week");
}

// =====================================================================
// 5-6: who does the work
// =====================================================================

/// **Gate 5: household production goes through the same work orders.**
///
/// Not a parallel mechanism with its own arithmetic. The same recipe, the
/// same three kinds of time, the same failure model — which is exactly what
/// makes a thing made at the kitchen table worse and slower rather than a
/// different sort of object.
#[test]
fn making_it_at_home_is_the_same_work_order_a_works_raises() {
    use scale_sim::craft::{hand_tools, standard_recipes, Maker, Workplace};

    let cat = standard_catalogue();
    let book = standard_recipes(&cat);
    let mut store = Store::new();
    let mut home = home_of(Roster::of(2));
    let kitchen = Workplace::a_workshop(hand_tools(&cat));
    let hands = Maker { skill: 0.85, proficiency: 0.8, knows_recipe: true,
                        tool_familiarity: 0.9, focus: 0.9, fatigue: 0.1 };

    let plan = book.must("chair, hand tools");
    let before = store.live();
    let made = make_it_yourself(
        &mut home, &mut store, &cat, &book, &kitchen, plan, hands, 900, 1,
    );
    let id = made.expect("the chair would not be made at home");

    // It is a real object, in the house, owned by them.
    assert!(store.live() > before);
    assert!(home.owns.contains(&id));
    assert_eq!(store.placement(id), Some(home.home));
    // **And it has a real as-built record**, because it went through the
    // ordinary machinery rather than being conjured.
    assert!(store.get(id).unwrap().assembly.is_some(), "a home-made chair has no record of being made");
    assert!(store.get(id).unwrap().quality.overall() > 0.0);
}

/// **Gate 6: a household order can be filled by anybody, the player
/// included.**
///
/// If the money does not reach a named seller then a shop is scenery, and
/// there is no business for a player to run.
#[test]
fn the_money_reaches_whoever_sold_it() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let player = 4_242u64;
    let mut market = Market { repairer: Some(45.0), power: Some(0.41), ..Default::default() };
    market.stock(Stall::new(cat.must("cooking stove"), 900.0, 2, player));
    market.services = vec![(Need::Nutrition, 12.0)];

    let mut home = home_of(Roster::of(2));
    let out = a_period(&mut home, &mut store, &cat, &mut market, comfortable(), 7, 6);

    let took: f64 = out.paid_to.iter().filter(|(w, _)| *w == player).map(|(_, p)| *p).sum();
    assert!((took - 900.0).abs() < 1e-9, "the player's shop was not paid: {took}");
    // **And the shelf went down by one.** A sale is stock leaving, not a
    // number going up.
    assert_eq!(market.stalls.iter().find(|s| s.seller == player).unwrap().stock, 1);

    // Sell the last one and the next household finds an empty shop.
    let mut second = home_of(Roster::of(2));
    let _ = a_period(&mut second, &mut store, &cat, &mut market, comfortable(), 7, 7);
    let mut third = home_of(Roster::of(2));
    let out = a_period(&mut third, &mut store, &cat, &mut market, comfortable(), 7, 8);
    assert!(
        !out.paid_to.iter().any(|(w, _)| *w == player),
        "the shop sold a third stove out of two"
    );
}

// =====================================================================
// 7-8: money, and wanting what you cannot have
// =====================================================================

/// **Gate 7: income, savings and credit each constrain, and differently.**
///
/// A lump sum is not a week's wages. Real: about a third of US households
/// could not meet a $400 emergency out of savings, which is precisely the
/// household that cannot buy the cheap-in-the-long-run answer.
#[test]
fn a_lump_sum_is_not_a_weeks_wages() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let market = a_town(&cat);
    let home = home_of(Roster::of(2));

    // Same total means, distributed differently.
    let saved = Means { income: 200.0, savings: 3_000.0, credit: 0.0 };
    let earned = Means { income: 3_200.0, savings: 0.0, credit: 0.0 };
    assert!((saved.total() - earned.total()).abs() < 1e-9);
    assert!(
        saved.for_a_lump_sum() > earned.for_a_lump_sum(),
        "money in the bank was no better than money coming in for a big purchase"
    );

    // **The poor household cannot reach the machine.** What it can reach is
    // a thirty-pound tub, which is a real answer and not the same answer:
    // it makes Monday possible rather than unnecessary.
    let broke = what_to_do(Need::CleanClothes, 1.0, &home, &store, &cat, &market, poor());
    assert_ne!(
        broke,
        Intent::BuyNew(cat.must("washing machine")),
        "a household with 15 in the bank bought a 700 washing machine"
    );
    assert_eq!(broke, Intent::BuyNew(cat.must("wash tub")), "{broke:?}");

    // And having bought it, the washing is still somebody's evening —
    // which is the real price of being poor, and it is paid in hours.
    let mut with_tub = home_of(Roster::of(2));
    give(&mut with_tub, &mut store, &cat, "wash tub");
    let level = requirement_for(Need::CleanClothes, with_tub.who, with_tub.where_);
    match provision_for(Need::CleanClothes, level, &with_tub.owns, &store, &cat) {
        Route::ByHand { hours_a_week } => {
            // Real hand laundry for two, with a tub: the better part of a
            // day a week, which is why a machine sold.
            assert!(
                (4.0..14.0).contains(&hours_a_week),
                "hand laundry for two came to {hours_a_week:.1} h a week"
            )
        }
        other => panic!("owning a tub was treated as having clean clothes: {other:?}"),
    }
    // With nothing at all it is worse again — a bucket and a stream.
    let bare = home_of(Roster::of(2));
    let nothing = Market::default();
    match what_to_do(Need::CleanClothes, level, &bare, &store, &cat, &nothing, poor()) {
        Intent::ByHand { hours_a_week } => assert!(
            hours_a_week > 12.0,
            "washing in a stream took {hours_a_week:.1} h, no worse than with a tub"
        ),
        other => panic!("{other:?}"),
    }

    let rich = what_to_do(Need::CleanClothes, 1.0, &home, &store, &cat, &market, comfortable());
    assert_eq!(rich, Intent::BuyNew(cat.must("washing machine")));
}

/// **Gate 8: what was wanted and what was got are different numbers.**
///
/// A model that reports only purchases reports a country with nothing in
/// the shops as a country that wanted nothing.
#[test]
fn desired_is_not_the_same_number_as_fulfilled() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let mut market = a_town(&cat);
    for s in market.stalls.iter_mut() {
        s.stock = 0;
    }
    market.services.clear();
    let mut home = home_of(Roster::of(3));
    home.free_hours = 2.0;

    let wanted = home.wants();
    assert_eq!(wanted.len(), ALL_NEEDS.len(), "a household stopped wanting things");
    let out = a_period(&mut home, &mut store, &cat, &mut market, comfortable(), 7, 9);

    assert!(out.spent < 1e-9, "it spent money in a market with nothing in it");
    let missed = out.without.len() + out.unmet.len();
    assert!(missed > 3, "a household in an empty country came up short on {missed} needs only");
    assert!(out.satisfaction(&wanted) < 0.75, "going without most things read as satisfied");

    // **And a supplied country satisfies far more of the same list.**
    let mut supplied = a_town(&cat);
    let mut lucky = home_of(Roster::of(3));
    let good = a_period(&mut lucky, &mut store, &cat, &mut supplied, comfortable(), 7, 10);
    assert!(
        good.satisfaction(&wanted) > out.satisfaction(&wanted) + 0.2,
        "having shops made no difference"
    );
}

// =====================================================================
// 9-11: flows, people and weather
// =====================================================================

/// **Gate 9: food is a flow and an appliance is a stock.**
///
/// Summing them is how a model comes to think a household buys a cooker
/// every week, or that it can go a month without eating because it bought
/// one last year.
#[test]
fn a_household_buys_food_every_week_and_a_stove_once() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let mut market = a_town(&cat);
    let mut home = home_of(Roster::of(2));

    let mut stoves = 0;
    let mut food_weeks = 0;
    for week in 0..8 {
        let out = a_period(&mut home, &mut store, &cat, &mut market, comfortable(), 7, 20 + week);
        stoves += out
            .bought
            .iter()
            .filter(|(o, _)| matches!(o.intent, Intent::BuyNew(d) | Intent::BuyUsed(d) if d == cat.must("cooking stove")))
            .count();
        if out.bought.iter().any(|(o, _)| o.need == Need::Nutrition) {
            food_weeks += 1;
        }
    }
    assert_eq!(stoves, 1, "it bought {stoves} stoves in eight weeks");
    assert_eq!(food_weeks, 8, "it ate in only {food_weeks} weeks out of eight");

    // The classification is the item layer's, not a second opinion here.
    assert!(is_flow(&cat, cat.must("loaf")));
    assert!(is_durable(&cat, cat.must("cooking stove")));
    assert!(!is_flow(&cat, cat.must("washing machine")));
}

/// **Gate 10: who is in the household changes what it needs, and not
/// uniformly.**
///
/// Feeding four costs four times feeding one; heating the room for four
/// costs barely more than heating it for one. That asymmetry is most of why
/// sharing a roof is cheaper per head.
#[test]
fn four_people_eat_four_dinners_and_heat_one_house() {
    let one = requirements(Roster::of(1), Climate::temperate());
    let four = requirements(Roster::of(4), Climate::temperate());
    let get = |rs: &[Requirement], n: Need| rs.iter().find(|r| r.need == n).unwrap().level;

    let food = get(&four, Need::Nutrition) / get(&one, Need::Nutrition);
    let warmth = get(&four, Need::Warmth) / get(&one, Need::Warmth);
    assert!((food - 4.0).abs() < 0.05, "four adults did not eat four dinners: {food:.2}");
    assert!(warmth < 1.5, "heating a house for four cost {warmth:.2} times heating it for one");
    assert!(warmth > 1.0, "the fourth person made the house no more expensive to heat at all");

    // **A child is not an adult**, and an infant is not a child.
    let adults = Roster::of(2);
    let with_kids = Roster::of(2).with(1, 1);
    assert!(with_kids.eaters() > adults.eaters());
    assert!(
        with_kids.eaters() < adults.eaters() + 2.0,
        "a toddler ate as much as a grown man"
    );
    let kids_food = requirement_for(Need::Nutrition, with_kids, Climate::temperate());
    let adults_food = requirement_for(Need::Nutrition, adults, Climate::temperate());
    assert!(kids_food > adults_food && kids_food < adults_food * 2.0);
}

/// **Gate 11: the climate decides the heating bill and part of the
/// wardrobe.**
///
/// Real US heating degree-days: Minneapolis about 7,800, Miami about 150.
/// Two identical households in those two places are not the same household.
#[test]
fn miami_and_minneapolis_are_not_the_same_household() {
    let who = Roster::of(2);
    let cold = requirement_for(Need::Warmth, who, Climate::cold());
    let hot = requirement_for(Need::Warmth, who, Climate::hot());
    let mid = requirement_for(Need::Warmth, who, Climate::temperate());

    assert!(cold > mid && mid > hot, "cold {cold:.2} temperate {mid:.2} hot {hot:.2}");
    assert!(cold / hot > 2.0, "a Minnesota winter cost {:.1}x a Florida one", cold / hot);

    // **But a hot country is not free.** Cooling is a real bill and in
    // Miami it is the larger one.
    assert!(hot > 0.4, "a hot country needed almost nothing spent on comfort: {hot:.2}");

    // Clothing follows the cold too, and much more weakly — a coat lasts
    // years, so apparel spending varies far less than the weather does.
    let coats_cold = requirement_for(Need::Clothed, who, Climate::cold());
    let coats_hot = requirement_for(Need::Clothed, who, Climate::hot());
    assert!(coats_cold > coats_hot);
    assert!(
        coats_cold / coats_hot < cold / hot,
        "clothing varied with climate as much as heating did"
    );
}

// =====================================================================
// 12-13: getting rid of things, and doing it in bulk
// =====================================================================

/// **Gate 12: getting rid of something is a decision with five answers.**
///
/// A sound machine nobody here wants is not destroyed. It is stored, sold,
/// given away, robbed for parts, or left — and only the last of those means
/// nobody owns it any more.
#[test]
fn a_household_does_not_destroy_what_it_has_finished_with() {
    use scale_sim::wip::Disposition;
    let cat = standard_catalogue();
    let mut store = Store::new();
    let mut market = a_town(&cat);
    let mut home = home_of(Roster::of(1));
    let _ = &mut home;

    // Sound, still wanted: it stays in use.
    let good = give(&mut home, &mut store, &cat, "washing machine");
    assert_eq!(dispose_of(&home, &store, good, &market, true), Disposition::InUse);

    // Sound, not wanted here, and there is a second-hand trade: it is sold.
    let spare = store.add(ItemInstance::one(&cat, cat.must("radio set")), home.home);
    market.stock(Stall::second_hand(cat.must("radio set"), 20.0, 1, 9));
    assert_eq!(dispose_of(&home, &store, spare, &market, false), Disposition::OfferedForSale);

    // Worn out, with something worth taking off it.
    let tired = give(&mut home, &mut store, &cat, "bicycle");
    store.get_mut(tired).unwrap().condition.wear = 0.75;
    assert_eq!(dispose_of(&home, &store, tired, &market, false), Disposition::Cannibalized);

    // **And what a household sells is where a second-hand market comes
    // from.** A town whose households sell nothing has an empty one.
    let offers = offered_for_sale(&home, &store, &cat, &market, 77);
    assert!(!offers.is_empty(), "a household with a sound machine offered nothing");
    assert!(offers.iter().all(|s| s.used && s.seller == 77));
    let washer_new = market.price_new(cat.must("washing machine")).unwrap();
    let offer = offers.iter().find(|s| s.def == cat.must("washing machine")).unwrap();
    assert!(
        offer.price < washer_new * 0.5,
        "a used machine was offered at {:.0} against {washer_new:.0} new",
        offer.price
    );
}

/// **Gate 13: a hundred detailed households and one aggregate agree.**
///
/// The design doc's rule arriving at demand. If promoting somebody to
/// detail changes what the town asks the market for, the economy depends on
/// who happens to be being watched.
#[test]
fn watching_them_closely_does_not_change_what_they_want() {
    let homes: Vec<Household> = (0..100).map(|_| home_of(Roster::of(2).with(0, 1))).collect();
    let detailed = aggregate(&homes);
    let coarse = aggregate_from(&homes[0], 100.0);

    assert_eq!(detailed.households, coarse.households);
    for need in ALL_NEEDS {
        let a = detailed.level(need);
        let b = coarse.level(need);
        assert!(
            (a - b).abs() < 1e-6,
            "{}: a hundred households want {a:.3} and the aggregate says {b:.3}",
            need.name()
        );
    }

    // And a mixed town is the sum of its parts, not the typical one times
    // the count — which is why the aggregate is only right where the
    // households really are alike.
    let mut mixed: Vec<Household> = (0..50).map(|_| home_of(Roster::of(1))).collect();
    mixed.extend((0..50).map(|_| home_of(Roster::of(4))));
    let real = aggregate(&mixed);
    assert!(
        real.level(Need::Nutrition) > aggregate_from(&mixed[0], 100.0).level(Need::Nutrition),
        "a town of singles and families ate like a town of singles"
    );
}

// =====================================================================
// 14-15: the shop, and the last house standing
// =====================================================================

/// **Gate 14: retail headcount follows customers, not tonnes.**
///
/// The error the census work found and could not fix from inside the
/// commodity model: a supermarket runs about 300 staff on 75 tonnes a day —
/// four people per daily tonne against a flour mill's 0.2 person-hours per
/// tonne. What a shop employs follows the transactions it serves.
#[test]
fn a_shop_employs_people_for_its_customers_not_its_tonnage() {
    // Double the customers through the same floor and the staff rise.
    let quiet = shop_staff_for(400.0, 900.0);
    let busy = shop_staff_for(2_400.0, 900.0);
    assert!(busy > quiet * 1.4, "a shop six times as busy needed {busy:.0} against {quiet:.0}");

    // Same customers over more floor also costs staff — somebody has to
    // keep the place.
    let cramped = shop_staff_for(1_200.0, 500.0);
    let sprawling = shop_staff_for(1_200.0, 3_700.0);
    assert!(sprawling > cramped * 1.5, "a supermarket ran on a corner shop's staff");

    // **Against the real anchor, and in the right unit.** The median US
    // supermarket is about 3,700 m2, takes 15-25,000 customers a week, and
    // employs about 89 people — but most of them part-time, so the hours
    // are far fewer than the payroll. Getting those two mixed up is the
    // denominator error `census.rs` exists to stop, and "200-300 staff" is
    // a Walmart Supercenter at 17,000 m2 rather than a supermarket.
    let fte = shop_staff_for(20_000.0 / 7.0, 3_700.0);
    let heads = heads_from_fte(fte);
    assert!(
        (55.0..85.0).contains(&fte),
        "a real supermarket came out at {fte:.0} full-time equivalents"
    );
    assert!(
        (75.0..105.0).contains(&heads),
        "a real supermarket came out at {heads:.0} people against about 89"
    );
    assert!(heads > fte, "part-time work made the payroll shorter than the hours");

    // And the superstore end lands where a Supercenter does.
    let super_fte = shop_staff_for(7_000.0, 17_000.0);
    assert!(
        (200.0..400.0).contains(&heads_from_fte(super_fte)),
        "a supercentre came out at {:.0} people against about 300",
        heads_from_fte(super_fte)
    );

    // A household is a customer several times a week, and a bigger one more
    // often.
    assert!(visits_a_week(Roster::of(4)) > visits_a_week(Roster::of(1)));
    assert!(visits_a_week(Roster::of(1)) > 1.0);
}

/// **Gate 15: the last household standing mends, makes and scavenges.**
///
/// With no shops at all, every route through the market is shut and the
/// three that are left are the ones people actually fall back on. A model
/// that can only buy things has nothing to say about this at all.
#[test]
fn with_no_shops_left_they_mend_make_and_scavenge() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let mut nothing = Market::default();
    let mut home = home_of(Roster::of(2));
    home.free_hours = 30.0;

    // **Making**: somebody here knows how to knock a bed together.
    home.can_make = vec![cat.must("bed")];
    // **Scavenging**: there is a stove in the empty house up the road.
    nothing.abandoned = vec![cat.must("cooking stove")];
    // **Mending**: their bicycle is broken and one of them can mend it,
    // which in a place with no repairer is the only way it gets mended.
    let bike = give(&mut home, &mut store, &cat, "bicycle");
    store.get_mut(bike).unwrap().condition.damage = 0.5;

    let out = a_period(&mut home, &mut store, &cat, &mut nothing, poor(), 7, 30);

    assert!(out.spent < 1e-9, "it spent money in a country with no shops");
    assert!(
        out.to_make.iter().any(|(_, d)| *d == cat.must("bed")),
        "nobody made the bed they knew how to make"
    );
    assert!(
        out.scavenged.iter().any(|(_, d)| *d == cat.must("cooking stove")),
        "the stove up the road was left there"
    );
    assert!(!out.by_hand.is_empty(), "nothing at all was done by hand");
    // What it scavenged is worn out, which is why this is the last resort.
    let stove = home
        .owns
        .iter()
        .find(|&&id| store.get(id).map(|i| i.definition) == Some(cat.must("cooking stove")))
        .copied()
        .expect("no stove");
    assert!(store.get(stove).unwrap().condition.wear > 0.6, "a scavenged stove came out new");

    // **And with a shop it does none of that.** Same household, same
    // knowledge, a market in the town.
    let mut town = a_town(&cat);
    let mut same = home_of(Roster::of(2));
    same.can_make = vec![cat.must("bed")];
    let with_shops = a_period(&mut same, &mut store, &cat, &mut town, comfortable(), 7, 31);
    assert!(with_shops.scavenged.is_empty(), "a household with money went scavenging");
    assert!(
        with_shops.to_make.is_empty(),
        "somebody who could afford a bed made one instead"
    );
}

// =====================================================================
// running it, and getting rid of it
// =====================================================================

/// **A second radiator does not double the heating bill. A second fridge
/// does.**
///
/// Two kinds of running cost, and collapsing them is what put a man with
/// three heaters on a power bill larger than his food. A demand-following
/// load runs to the weather and shares itself over whatever is installed; a
/// standing load runs day and night whether anybody wants it or not.
#[test]
fn two_radiators_in_a_mild_house_each_run_at_part_load() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let mut home = home_of(Roster::of(2));
    let wanted = home.wants();

    give(&mut home, &mut store, &cat, "space heater");
    let one = running_kwh(&home.owns, &store, &cat, &wanted);
    give(&mut home, &mut store, &cat, "space heater");
    let two = running_kwh(&home.owns, &store, &cat, &wanted);
    give(&mut home, &mut store, &cat, "space heater");
    let three = running_kwh(&home.owns, &store, &cat, &wanted);

    assert!(two > one, "a second heater made no difference to a house that was short of one");
    assert!(
        two < one * 1.6,
        "two heaters cost {two:.0} kWh against one at {one:.0} — they are running flat out"
    );
    assert!(
        (three - two).abs() < 1e-6,
        "a third heater in the same house cost another {:.0} kWh",
        three - two
    );

    // **A fridge is the other kind**, and a second one really does double
    // the bill, because it runs whether or not anybody opens it.
    let mut cold = home_of(Roster::of(2));
    give(&mut cold, &mut store, &cat, "refrigerator");
    let one_fridge = running_kwh(&cold.owns, &store, &cat, &wanted);
    give(&mut cold, &mut store, &cat, "refrigerator");
    let two_fridges = running_kwh(&cold.owns, &store, &cat, &wanted);
    assert!(
        (two_fridges - one_fridge * 2.0).abs() < 1e-6,
        "a second fridge cost {:.1} kWh against the first at {one_fridge:.1}",
        two_fridges - one_fridge
    );

    // **And the weather decides.** The same two heaters in Minnesota and in
    // Miami are not the same bill.
    let cold_house = Household::new(Roster::of(2), Climate::cold(), home.home);
    let hot_house = Household::new(Roster::of(2), Climate::hot(), home.home);
    let in_cold = running_kwh(&home.owns, &store, &cat, &cold_house.wants());
    let in_hot = running_kwh(&home.owns, &store, &cat, &hot_house.wants());
    assert!(
        in_cold > in_hot * 1.5,
        "Minneapolis {in_cold:.0} kWh against Miami {in_hot:.0} — the weather did nothing"
    );

    // A broken one draws nothing at all.
    let dead = home.owns[0];
    store.get_mut(dead).unwrap().condition.damage = 0.9;
    let with_a_dead_one = running_kwh(&home.owns, &store, &cat, &wanted);
    assert!(with_a_dead_one <= two + 1e-9, "a broken heater was still on the bill");
}

/// **What is past mending goes, and the household stops paying to run it.**
///
/// Without this a household accumulates dead appliances for ever — which is
/// exactly how one came to own three heaters, one of them broken, and to be
/// charged for all of them.
#[test]
fn a_dead_machine_does_not_sit_in_the_corner_for_ever() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let mut market = a_town(&cat);
    let mut home = home_of(Roster::of(2));

    let dead = give(&mut home, &mut store, &cat, "refrigerator");
    store.get_mut(dead).unwrap().condition.damage = 0.9;
    store.get_mut(dead).unwrap().condition.wear = 0.97;
    let sound = give(&mut home, &mut store, &cat, "radio set");

    let out = a_period(&mut home, &mut store, &cat, &mut market, comfortable(), 7, 40);

    assert!(
        out.discarded.iter().any(|(id, _)| *id == dead),
        "a scrap fridge stayed on the books"
    );
    assert!(!home.owns.contains(&dead), "it still owns the fridge it got rid of");
    assert!(home.owns.contains(&sound), "it threw out a working radio");
    // **And it is a disposition, not destruction.** Something became of it.
    let fate = out.discarded.iter().find(|(id, _)| *id == dead).unwrap().1;
    assert_ne!(fate, scale_sim::wip::Disposition::InUse, "a scrap fridge was still in use");
    // A dead fridge waiting for the scrap man is still a whole fridge —
    // which is the point of `Disposition` — but it is no longer theirs and
    // it no longer costs them anything to run.
    let still_paying = running_kwh(&home.owns, &store, &cat, &home.wants());
    let with_it_back = {
        let mut owns = home.owns.clone();
        owns.push(dead);
        running_kwh(&owns, &store, &cat, &home.wants())
    };
    assert!(
        (still_paying - with_it_back).abs() < 1e-9,
        "the fridge was drawing power after it was scrapped"
    );

    // **A sound one somebody has finished with reaches the second-hand
    // trade**, which is where used stock comes from.
    let mut seller = home_of(Roster::of(2));
    let spare = give(&mut seller, &mut store, &cat, "washing machine");
    store.get_mut(spare).unwrap().condition.wear = 0.4;
    let offers = offered_for_sale(&seller, &store, &cat, &market, 55);
    assert!(offers.iter().any(|s| s.def == cat.must("washing machine") && s.used));
}

// =====================================================================
// borrowing
// =====================================================================

/// **Gate: without credit nobody owns a car, and with it they do.**
///
/// The single biggest hole in the household shares, and it was not a
/// tuning problem: real US transport is 17% of expenditure and almost all
/// of it is borrowed, so a model where households can only buy what they
/// can pay for outright reads transport as nearly zero. Which it did.
#[test]
fn a_car_is_bought_on_credit_or_not_at_all() {
    let cat = standard_catalogue();
    let mut store = Store::new();
    let car = cat.must("motor car");

    // A comfortable household, and a car at a real used-car price.
    let purse = Means { income: 1_400.0, savings: 4_000.0, credit: 0.0 };
    let mut cash_only = a_town(&cat);
    cash_only.finance = None;

    let mut home = home_of(Roster::of(2));
    let level = requirement_for(Need::Mobility, home.who, home.where_);
    let outright = what_to_do(Need::Mobility, level, &home, &store, &cat, &cash_only, purse);
    assert!(
        !matches!(outright, Intent::BuyOnCredit { .. }),
        "a town with no bank in it offered finance"
    );
    assert_ne!(outright, Intent::BuyNew(car), "somebody paid 9,000 cash out of 4,000 of savings");

    // **The same household, the same money, a bank in the town.**
    let mut with_a_bank = a_town(&cat);
    with_a_bank.finance =
        Some(Finance { rates: scale_sim::bank::Rates::ordinary(), lending: true });
    let financed = what_to_do(Need::Mobility, level, &home, &store, &cat, &with_a_bank, purse);
    match financed {
        Intent::BuyOnCredit { def, monthly, months, rate } => {
            assert_eq!(def, car, "it financed something other than the car");
            assert!(months >= 48, "a car loan over {months} months");
            assert!((0.05..0.20).contains(&rate), "quoted at {:.1}%", rate * 100.0);
            // Real US new-car payments average around $700 a month.
            assert!((150.0..900.0).contains(&monthly), "{monthly:.0} a month for a car");
        }
        other => panic!("a household with a bank and a wage did not finance a car: {other:?}"),
    }

    // **And it turns up in the driveway, with an obligation attached.**
    let out = a_period(&mut home, &mut store, &cat, &mut with_a_bank, purse, 7, 90);
    assert!(!out.borrowed.is_empty(), "nothing was actually borrowed for");
    assert!(home.committed_monthly > 100.0, "the payment did not attach to the household");
    assert!(
        home.owns.iter().any(|&id| store.get(id).map(|i| i.definition) == Some(car)),
        "the loan was written and no car arrived"
    );

    // **The payment comes off the top for years afterwards**, which is the
    // whole reason borrowing is a decision rather than free money.
    let next = a_period(&mut home, &mut store, &cat, &mut with_a_bank, purse, 30, 91);
    assert!(next.debt_service > 100.0, "the loan cost nothing the following month");
}

/// **Gate: a credit crunch is one field going false.**
///
/// Nothing about the household changes — the same wage, the same savings,
/// the same want. The bank simply stops lending, and the car does not
/// happen. Which is what a credit crunch is, and why it bites so fast.
#[test]
fn when_the_bank_stops_lending_the_car_stops_happening() {
    let cat = standard_catalogue();
    let store = Store::new();
    let home = home_of(Roster::of(2));
    let purse = Means { income: 1_400.0, savings: 4_000.0, credit: 0.0 };
    let level = requirement_for(Need::Mobility, home.who, home.where_);

    let mut open = a_town(&cat);
    open.finance = Some(Finance { rates: scale_sim::bank::Rates::ordinary(), lending: true });
    let mut shut = open.clone();
    shut.finance = Some(Finance { rates: scale_sim::bank::Rates::ordinary(), lending: false });

    assert!(matches!(
        what_to_do(Need::Mobility, level, &home, &store, &cat, &open, purse),
        Intent::BuyOnCredit { .. }
    ));
    assert!(
        !matches!(
            what_to_do(Need::Mobility, level, &home, &store, &cat, &shut, purse),
            Intent::BuyOnCredit { .. }
        ),
        "the bank stopped lending and somebody borrowed anyway"
    );

    // **And dear money is not the same as no money.** A high policy rate
    // makes the payment bigger; it does not make the loan impossible.
    let mut dear = open.clone();
    dear.finance = Some(Finance {
        rates: scale_sim::bank::Rates { policy: 0.14, on_deposits: 0.09 },
        lending: true,
    });
    let cheap_deal = what_to_do(Need::Mobility, level, &home, &store, &cat, &open, purse);
    let dear_deal = what_to_do(Need::Mobility, level, &home, &store, &cat, &dear, purse);
    match (cheap_deal, dear_deal) {
        (
            Intent::BuyOnCredit { monthly: a, .. },
            Intent::BuyOnCredit { monthly: b, .. },
        ) => assert!(b > a * 1.1, "nine points on the policy rate cost {:.0} against {:.0}", b, a),
        (_, other) => panic!("dear money made the loan impossible rather than dear: {other:?}"),
    }
}

/// **Gate: what is already owed stops somebody borrowing again.**
///
/// Debt to income is the first number an underwriter looks at, and real
/// lenders stop at 43%. It is also why a household that has borrowed once
/// is not simply a household with more things.
#[test]
fn a_household_already_paying_for_a_car_cannot_borrow_for_another() {
    let cat = standard_catalogue();
    let store = Store::new();
    let purse = Means { income: 1_400.0, savings: 4_000.0, credit: 0.0 };
    let level = requirement_for(Need::Mobility, Roster::of(2), Climate::temperate());
    let mut market = a_town(&cat);
    market.finance = Some(Finance { rates: scale_sim::bank::Rates::ordinary(), lending: true });

    let clear = home_of(Roster::of(2));
    assert!(matches!(
        what_to_do(Need::Mobility, level, &clear, &store, &cat, &market, purse),
        Intent::BuyOnCredit { .. }
    ));

    let mut stretched = home_of(Roster::of(2));
    // A mortgage and a car already, on a wage of about 6,000 a month.
    stretched.committed_monthly = 2_500.0;
    assert!(
        !matches!(
            what_to_do(Need::Mobility, level, &stretched, &store, &cat, &market, purse),
            Intent::BuyOnCredit { .. }
        ),
        "somebody at 43% of income already was written another loan"
    );

    // **And a poor record makes it dearer rather than impossible**, which
    // is the subprime market in one assertion.
    let mut poor_record = home_of(Roster::of(2));
    poor_record.standing = 0.38;
    let mut good_record = home_of(Roster::of(2));
    good_record.standing = 0.85;
    match (
        what_to_do(Need::Mobility, level, &good_record, &store, &cat, &market, purse),
        what_to_do(Need::Mobility, level, &poor_record, &store, &cat, &market, purse),
    ) {
        (Intent::BuyOnCredit { rate: good, .. }, Intent::BuyOnCredit { rate: bad, .. }) => {
            assert!(bad > good * 1.3, "prime {:.1}% against subprime {:.1}%", good * 100.0, bad * 100.0)
        }
        (_, other) => panic!("a subprime borrower was refused outright: {other:?}"),
    }
}

// =====================================================================
// and the shape of the whole thing
// =====================================================================

/// **Engel's law is an output, not an input.**
///
/// Nothing in this module writes down a share of spending. What it writes
/// down is how badly each need is wanted, and the money runs out where it
/// runs out — so a poor household comes out spending a far larger share of
/// its money on food than a comfortable one does.
///
/// Real: the poorest fifth of US households spend about 15.5% of their
/// expenditure on food, the richest about 11%, and the gap is much wider
/// still measured against income.
#[test]
fn the_poor_spend_a_larger_share_of_it_on_food() {
    let cat = standard_catalogue();
    let mut store = Store::new();

    let mut share_for = |means: Means, event: u64| {
        let mut market = a_town(&cat);
        let mut home = home_of(Roster::of(2));
        let mut food = 0.0;
        let mut all = 0.0;
        for week in 0..26 {
            let out = a_period(&mut home, &mut store, &cat, &mut market, means, 7, event + week);
            for (o, paid) in &out.bought {
                all += paid;
                if o.need.category() == Category::Food {
                    food += paid;
                }
            }
        }
        if all > 0.0 { food / all } else { 0.0 }
    };

    let poor_share = share_for(poor(), 100);
    let rich_share = share_for(comfortable(), 200);
    assert!(
        poor_share > rich_share,
        "the poor household spent {:.0}% on food and the comfortable one {:.0}%",
        poor_share * 100.0,
        rich_share * 100.0
    );
    assert!(poor_share > 0.5, "a household on 210 a week spent {:.0}% on food", poor_share * 100.0);
}

/// The categories are the published ones, and they add up.
#[test]
fn the_budget_categories_are_a_real_budget() {
    let total: f64 = ALL_CATEGORIES.iter().map(|c| c.real_share()).sum();
    assert!(
        (total - 0.783).abs() < 0.01,
        "the named categories come to {total:.3} of a real budget"
    );
    // Shelter is the biggest single line in a real budget and is not this
    // module's to spend.
    assert!(Category::Shelter.real_share() > Category::Food.real_share());
    // Every need lands in exactly one of them.
    for need in ALL_NEEDS {
        assert!(ALL_CATEGORIES.contains(&need.category()), "{} has no category", need.name());
    }
}
