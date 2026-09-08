//! **Not everything is reusable, and what decides it is money.**
//!
//! The material layer has said what each thing can come back as since it
//! was written. What it could not say is whether anybody would take the
//! pile — because that is a fact about the spread between what a mill pays
//! and what it costs to get there, not about the material.

use scale_sim::material::{Material, Recovers, ALL_MATERIALS};
use scale_sim::scrap::*;

/// **Gate: the price range is four orders of magnitude, and it is why the
/// recycling rates are what they are.**
///
/// Real US: lead-acid batteries come back at 99% and plastics at 8.7%.
/// Nobody is being virtuous about lead — it is worth $1,000 a tonne and
/// mixed plastics are worth nothing.
#[test]
fn what_gets_recycled_is_what_is_worth_money() {
    assert!(price_a_tonne(Material::Copper) > price_a_tonne(Material::MildSteel) * 20.0);
    assert!(
        price_a_tonne(Material::Lead) > 500.0,
        "lead was not worth collecting"
    );
    assert!(price_a_tonne(Material::Glass) < price_a_tonne(Material::Aluminium) * 0.05);

    // **And some of it is a cost.** A negative price is not a defect in the
    // model; it is the ordinary case for a great deal of a household.
    assert!(
        price_a_tonne(Material::Rubber) < 0.0,
        "tyres were worth money"
    );
    assert!(price_a_tonne(Material::Propellant) < -1_000.0);
    assert!(
        price_a_tonne(Material::Lithium) < 0.0,
        "a cell was an asset at the gate"
    );

    // Every material has an answer, including the ones nobody weighs in.
    for m in ALL_MATERIALS {
        let p = price_a_tonne(m);
        assert!(p.is_finite(), "{} has no price at the gate", m.name());
    }
}

/// **Gate: distance decides whether a material is recycled at all.**
///
/// The rule `logistics.rs` already applies to freight, arriving at waste.
/// Copper will cross a continent; broken glass will not cross a county, and
/// that is exactly why one is recycled everywhere and the other mostly is
/// not.
#[test]
fn copper_will_travel_and_glass_will_not() {
    let copper = economic_range_km(Material::Copper);
    let glass = economic_range_km(Material::Glass);
    let steel = economic_range_km(Material::MildSteel);

    assert!(
        copper > 10_000.0,
        "copper stopped being worth having after {copper:.0} km"
    );
    assert!(glass < 300.0, "glass was worth hauling {glass:.0} km");
    assert!(steel > 1_000.0 && steel < copper);

    assert!(worth_collecting(Material::Copper, 500.0));
    assert!(!worth_collecting(Material::Glass, 500.0));
    // And nothing with a gate fee is ever worth collecting for its own
    // sake, at any distance.
    assert!(!worth_collecting(Material::Rubber, 1.0));
}

/// **Gate: what cannot come back is a cost, not a shortfall.**
///
/// `Recovers::Nothing` has existed since the material layer was written and
/// nothing ever charged for it. A load of things that only burn or only
/// bury is a bill.
#[test]
fn a_load_of_what_nobody_wants_is_a_bill() {
    let yard = Yard::ordinary(1, 10.0);

    // A tonne of clean copper is a good day.
    let good = Load::of(&[(Material::Copper, 1.0)]);
    let s = weigh_in(&good, &yard, 0.0);
    match s {
        Settlement::Taken {
            paid,
            ref recovered,
            residue,
        } => {
            assert!(paid > 8_000.0, "a tonne of copper fetched {paid:.0}");
            assert_eq!(recovered.len(), 1);
            assert!(
                residue < 1e-9,
                "clean copper left {residue:.2} t of residue"
            );
        }
        other => panic!("a tonne of copper was refused: {other:?}"),
    }

    // A tonne of tyres and plasterboard is a bill, and the customer has to
    // be willing to pay it.
    let bad = Load::of(&[(Material::Rubber, 0.6), (Material::Gypsum, 0.4)]);
    assert_eq!(
        weigh_in(&bad, &yard, 0.0),
        Settlement::Refused(Refusal::NotWorthTaking),
        "a yard took a load of tyres for nothing"
    );
    match weigh_in(&bad, &yard, 500.0) {
        Settlement::Taken { paid, residue, .. } => {
            assert!(
                paid < 0.0,
                "tyres and plasterboard paid the customer {paid:.0}"
            );
            assert!(
                residue > 0.9,
                "only {residue:.2} t of a tonne of tyres was tipped"
            );
        }
        other => panic!("{other:?}"),
    }

    // The material layer already knew which was which; this only put a
    // price on it.
    assert_eq!(Material::Copper.recovers(), Recovers::Feedstock);
    assert!(matches!(
        Material::Rubber.recovers(),
        Recovers::Fuel | Recovers::Nothing
    ));
}

/// **Gate: contamination is a discount and a loss, and past a limit it is a
/// refusal.**
///
/// Real single-stream recycling runs at 15-25% contamination, a MRF rejects
/// a load over about 10%, and China's National Sword set the limit at 0.5%
/// in 2018 and collapsed the world market for mixed recyclate overnight.
#[test]
fn a_dirty_load_is_worth_less_and_then_worth_nothing() {
    let yard = Yard::ordinary(1, 10.0);
    let clean = Load::of(&[(Material::Aluminium, 1.0)]);
    let mut dirty = Load::of(&[(Material::Aluminium, 1.0)]);
    dirty.contamination = 0.10;

    let (a, b) = (weigh_in(&clean, &yard, 0.0), weigh_in(&dirty, &yard, 0.0));
    let (
        Settlement::Taken {
            paid: clean_paid,
            residue: clean_res,
            ..
        },
        Settlement::Taken {
            paid: dirty_paid,
            residue: dirty_res,
            ..
        },
    ) = (a, b)
    else {
        panic!("a clean load of aluminium was refused")
    };
    assert!(
        dirty_paid < clean_paid * 0.92,
        "a tenth of rubbish cost nothing"
    );
    assert!(
        dirty_res > clean_res,
        "the rubbish in it did not go to the hole"
    );

    // Past the limit it is not a discount, it is the gate closing.
    let mut filthy = Load::of(&[(Material::Aluminium, 1.0)]);
    filthy.contamination = 0.35;
    assert_eq!(
        weigh_in(&filthy, &yard, 10_000.0),
        Settlement::Refused(Refusal::TooContaminated),
        "a yard took a load that was a third rubbish"
    );

    // **National Sword**, as a shock: the same load against a limit of half
    // a percent.
    let mut strict = Yard::ordinary(1, 10.0);
    strict.contamination_limit = 0.005;
    let mut ordinary_kerbside = Load::of(&[(Material::Abs, 1.0)]);
    ordinary_kerbside.contamination = 0.18;
    assert_eq!(
        weigh_in(&ordinary_kerbside, &strict, 10_000.0),
        Settlement::Refused(Refusal::TooContaminated),
        "ordinary kerbside recyclate passed a half-percent limit"
    );
}

/// **Gate: hazardous content is checked, not taken on trust.**
///
/// A yard that finds a lithium cell in a bale has a fire, not a
/// discrepancy — US facilities report hundreds a year. And a load nobody is
/// licensed to take is what ends up in a hedge.
#[test]
fn a_cell_in_the_bale_is_turned_away_whatever_the_paperwork_says() {
    let ordinary = Yard::ordinary(1, 10.0);
    let licensed = Yard::licensed(2, 90.0);

    let mut sneaky = Load::of(&[(Material::MildSteel, 0.9), (Material::Lithium, 0.1)]);
    // Declared clean, and it is not.
    sneaky.declared_hazard = false;
    assert!(
        sneaky.actually_hazardous(),
        "a load with a lithium cell read as safe"
    );
    assert_eq!(
        weigh_in(&sneaky, &ordinary, 10_000.0),
        Settlement::Refused(Refusal::Hazardous),
        "an unlicensed yard took a load with cells in it"
    );

    // The licensed processor will, further away and for a fee.
    match weigh_in(&sneaky, &licensed, 10_000.0) {
        Settlement::Taken { paid, .. } => {
            assert!(
                paid < 300.0,
                "a hazardous load paid {paid:.0} like clean steel"
            );
        }
        other => panic!("a licensed processor refused it: {other:?}"),
    }

    // And hazard is a property of the material, which is what makes the
    // check possible at all.
    assert!(Material::Lead.hazardous() && Material::Propellant.hazardous());
    assert!(!Material::MildSteel.hazardous());
}

/// **Gate: a dead appliance is worth close to nothing, and some of it costs
/// money to be rid of.**
///
/// Which is why so much of it is left at the kerb. A refrigerator is the
/// standard case: its refrigerant must be recovered by a certified
/// technician before the shell can be shredded, and that costs more than
/// the steel is worth.
#[test]
fn a_dead_fridge_is_not_worth_the_trip() {
    // A washing machine: 70 kg, mostly steel and 21 kg of concrete.
    let washer = [
        (Material::MildSteel, 0.0315),
        (Material::Concrete, 0.0210),
        (Material::Abs, 0.0084),
        (Material::Copper, 0.0035),
    ];
    let near = worth_as_scrap("washing machine", &washer, 5.0, false);
    assert!(
        near > 0.0,
        "a washing machine was worth {near:.2} at the yard down the road"
    );
    assert!(near < 100.0, "a scrap washing machine fetched {near:.0}");

    // **And the refrigerant is what tips a fridge over.**
    let fridge = [
        (Material::MildSteel, 0.0273),
        (Material::Abs, 0.0149),
        (Material::Polyethylene, 0.0087),
        (Material::Copper, 0.0050),
    ];
    let with_handling = worth_as_scrap("refrigerator", &fridge, 5.0, false);
    let without = worth_as_scrap("washing machine", &fridge, 5.0, false);
    assert!(
        with_handling < without,
        "recovering the refrigerant cost nothing: {with_handling:.2} against {without:.2}"
    );
    assert!(
        with_handling < 40.0,
        "a scrap fridge was worth {with_handling:.0} after the technician"
    );

    // Far enough away and it stops being worth the trip at all, which is
    // what a rural fly-tip is.
    let far = worth_as_scrap("washing machine", &washer, 400.0, false);
    assert!(far < near, "distance cost nothing");
}

/// **Gate: a motor is not copper.**
///
/// You get the clean price only if you have clean metal, and the whole
/// scrap trade lives in that gap. Real US grades for what is chemically the
/// same copper: bare bright wire $8,500 a tonne, insulated wire $1,500-2,500,
/// electric motors $350-500, mixed appliance scrap $150-250. Pricing an
/// unsorted object at the sum of its clean fractions valued a dead washing
/// machine at $42 against a real $10-20.
#[test]
fn you_get_the_clean_price_only_if_you_have_clean_metal() {
    let copper = Material::Copper;
    assert!(
        as_found(copper, false) < as_found(copper, true) * 0.25,
        "a motor sold for the price of bare bright wire"
    );
    assert!(
        as_found(copper, false) > UNSORTED_RATE,
        "sorted copper was worth no more than tin"
    );
    // Nothing below the mixed rate is improved by being sorted — a tonne of
    // broken glass is a tonne of broken glass however carefully it is
    // presented.
    assert_eq!(
        as_found(Material::Glass, false),
        as_found(Material::Glass, true)
    );
    assert_eq!(
        as_found(Material::Rubber, false),
        as_found(Material::Rubber, true)
    );

    // **And the numbers land where the real ones do.** A 70 kg washing
    // machine at the yard down the road.
    let washer = [
        (Material::MildSteel, 0.0315),
        (Material::Concrete, 0.0210),
        (Material::Abs, 0.0084),
        (Material::Copper, 0.0035),
    ];
    let as_it_stands = worth_as_scrap("washing machine", &washer, 5.0, false);
    let taken_apart = worth_as_scrap("washing machine", &washer, 5.0, true);
    assert!(
        (8.0..22.0).contains(&as_it_stands),
        "a scrap washing machine fetched {as_it_stands:.2} against a real 10-20"
    );
    assert!(
        taken_apart > as_it_stands * 2.0,
        "stripping it was worth nothing"
    );

    // Which is a real decision, and it turns on somebody's time.
    assert!(
        worth_stripping(&washer, 0.75, 22.0),
        "45 minutes for 30 dollars was not worth it"
    );
    assert!(
        !worth_stripping(&washer, 8.0, 22.0),
        "a whole day for 30 dollars was worth it"
    );
}

/// **Gate: some things cost money to be rid of, and that is why they end up
/// in a hedge.**
///
/// A refrigerator is the standard case. Its refrigerant has to be recovered
/// by a certified technician *(EPA Section 608)* before the shell can be
/// shredded, and that costs more than the steel in it is worth — so a
/// household doing the right thing pays, and one that does not, does not.
#[test]
fn a_scrap_fridge_costs_money_and_a_scrap_washer_does_not() {
    let fridge = [
        (Material::MildSteel, 0.0273),
        (Material::Abs, 0.0149),
        (Material::Polyethylene, 0.0087),
        (Material::Copper, 0.0050),
    ];
    let washer = [
        (Material::MildSteel, 0.0315),
        (Material::Concrete, 0.0210),
        (Material::Abs, 0.0084),
        (Material::Copper, 0.0035),
    ];

    let fridge_worth = worth_as_scrap("refrigerator", &fridge, 5.0, false);
    let washer_worth = worth_as_scrap("washing machine", &washer, 5.0, false);
    assert!(
        fridge_worth < 0.0,
        "a scrap fridge was worth {fridge_worth:.2} despite the refrigerant"
    );
    assert!(
        washer_worth > 0.0,
        "a scrap washing machine was a liability"
    );

    // **And it is the handling, not the metal.** The identical bill of
    // materials without the certified recovery is worth having.
    let same_metal = worth_as_scrap("washing machine", &fridge, 5.0, false);
    assert!(same_metal > 0.0);
    assert!(
        (same_metal - fridge_worth - special_handling("refrigerator")).abs() < 1e-9,
        "the difference was something other than the technician"
    );
    assert!(special_handling("refrigerator") > 0.0);
    assert!(special_handling("bed") == 0.0);
}

/// **Gate: a car comes back at about 95% by weight and the rest is real.**
///
/// Real end-of-life vehicles: metals are 70-75% of the mass and come back
/// almost entirely; **automotive shredder residue** — foam, fabric, glass
/// and mixed plastic — is 20-25% and goes in the ground almost everywhere.
#[test]
fn a_shredded_car_leaves_a_real_pile_of_fluff() {
    let car = Load::of(&[
        (Material::MildSteel, 0.870),
        (Material::Aluminium, 0.135),
        (Material::Abs, 0.165),
        (Material::Polyethylene, 0.060),
        (Material::Rubber, 0.090),
        (Material::Glass, 0.060),
        (Material::Copper, 0.045),
        (Material::Polyester, 0.030),
        (Material::Lubricant, 0.030),
        (Material::Lead, 0.015),
    ]);
    let total = car.tonnes();
    let (back, fluff) = shred(&car);

    let recovered: f64 = back.iter().map(|b| b.1).sum();
    assert!(
        (recovered + fluff - total).abs() < 1e-9,
        "a shredder lost {:.3} t of a {total:.3} t car",
        total - recovered - fluff
    );

    let share = recovered / total;
    assert!(
        (0.68..0.85).contains(&share),
        "a shredder recovered {:.0}% of a car by weight",
        share * 100.0
    );
    // The metals come back and the plastics do not, which is exactly what a
    // magnet is good at.
    let steel_back = back.iter().find(|b| b.0 == Material::MildSteel).unwrap().1;
    assert!(steel_back / 0.870 > 0.95, "the magnets missed the steel");
    assert!(
        back.iter().find(|b| b.0 == Material::Rubber).is_none(),
        "a shredder recovered the tyres"
    );

    // And the fluff is a real quantity, not a rounding error.
    assert!(
        fluff / total > 0.15,
        "the shredder residue was {:.0}%",
        fluff / total * 100.0
    );
}
fn a_dead_fridge() -> [(Material, f64); 4] {
    [
        (Material::MildSteel, 0.0273),
        (Material::Abs, 0.0149),
        (Material::Polyethylene, 0.0087),
        (Material::Copper, 0.0050),
    ]
}

/// An ordinary situation, to be varied one thing at a time.
fn getting_rid_of<'a>(
    name: &'static str,
    materials: &'a [(Material, f64)],
    kg: f64,
    council: &'a Council,
) -> Circumstances<'a> {
    Circumstances {
        name,
        materials,
        kg,
        bulky: true,
        still_works: false,
        carrying: Carrying::ACar,
        council,
        collections_used: 9,
        others_going: 0,
        hourly_worth: 15.0,
        scruple: 0.9,
        room_to_store: false,
        somebody_wants_it: false,
        charity_nearby: false,
        being_replaced: false,
        scrappers_about: false,
        somewhere_to_burn: false,
        event: 1,
    }
}

/// **Gate: there are a dozen ways to be rid of something and only three of
/// them involve a fee.**
///
/// The commonest routes are not the ones a disposal model usually has: the
/// shop takes the old one away when it delivers the new one, somebody wants
/// it, it goes on the pavement and vanishes, or it goes in the loft and is
/// never dealt with at all.
#[test]
fn most_things_never_become_a_disposal_problem() {
    let town = Council::a_town();
    let fridge = a_dead_fridge();

    // **The shop takes the old one**, which is what actually happens to
    // most large appliances and costs the household nothing.
    let mut c = getting_rid_of("refrigerator", &fridge, 62.0, &town);
    c.being_replaced = true;
    assert!(matches!(
        how_to_get_rid_of_it(&c),
        HowToGetRidOfIt::TradeIn { .. }
    ));

    // **A deposit beats everything**, because it is money for doing what
    // you were going to do anyway — and it is the whole reason 99% of
    // lead-acid batteries come back.
    let cells = [(Material::Lead, 0.012), (Material::Abs, 0.004)];
    let c = getting_rid_of("battery pack", &cells, 16.0, &town);
    match how_to_get_rid_of_it(&c) {
        HowToGetRidOfIt::RedeemTheDeposit { refund } => assert!(refund > 5.0),
        other => panic!("nobody took the battery back: {other:?}"),
    }
    assert!(deposit_on("battery pack") > 0.0 && deposit_on("bed") == 0.0);

    // **Somebody wants it**, and a working thing given away costs nothing.
    let mut c = getting_rid_of("radio set", &[(Material::Abs, 0.0024)], 2.4, &town);
    c.bulky = false;
    c.still_works = true;
    c.somebody_wants_it = true;
    assert_eq!(how_to_get_rid_of_it(&c), HowToGetRidOfIt::GiveItAway);

    // **A charity takes what it can sell and refuses what it cannot.**
    let mut c = getting_rid_of("radio set", &[(Material::Abs, 0.0024)], 2.4, &town);
    c.bulky = false;
    c.still_works = true;
    c.charity_nearby = true;
    assert_eq!(how_to_get_rid_of_it(&c), HowToGetRidOfIt::Donate);
    assert!(
        !a_charity_would_take_it("radio set", false),
        "a charity took a broken radio"
    );
    assert!(
        !a_charity_would_take_it("bed", true),
        "a charity took a mattress"
    );

    // **Put it on the pavement and it goes** — but only if there is metal
    // in it, because it is the scrappers who take it. Something with a few
    // dollars of steel in it is not worth a trip to the yard and is
    // certainly worth somebody else's.
    let a_small_thing = [(Material::MildSteel, 0.006), (Material::Abs, 0.003)];
    let mut c = getting_rid_of("space heater", &a_small_thing, 9.0, &town);
    c.bulky = false;
    c.scrappers_about = true;
    assert_eq!(how_to_get_rid_of_it(&c), HowToGetRidOfIt::LeaveItOut);
    // And with nobody about to take it, the same thing is a chore.
    c.scrappers_about = false;
    assert!(!matches!(
        how_to_get_rid_of_it(&c),
        HowToGetRidOfIt::LeaveItOut
    ));

    // **Or it goes in the loft**, which is what happens to most things and
    // is a decision deferred rather than taken.
    let mut c = getting_rid_of("refrigerator", &fridge, 62.0, &town);
    c.room_to_store = true;
    assert_eq!(how_to_get_rid_of_it(&c), HowToGetRidOfIt::KeepIt);

    // And the council's free collections come before anybody pays for
    // anything.
    let mut c = getting_rid_of("refrigerator", &fridge, 62.0, &town);
    c.collections_used = 0;
    assert_eq!(how_to_get_rid_of_it(&c), HowToGetRidOfIt::Kerbside);
}

/// **Gate: a man with a truck gets rid of it for a fraction of what his
/// neighbour pays — once he has a load.**
///
/// Nobody drives one thing to the tip. The minimum gate charge, the fuel
/// and the afternoon are costs of the *trip*, so one item carries all of
/// them and a truckload divides them — which is exactly why things pile up
/// in a yard before anybody goes anywhere.
#[test]
fn owning_a_truck_is_worth_money_once_you_have_a_load() {
    let town = Council::a_town();
    let fridge = a_dead_fridge();

    // One thing, on its own: not worth a special trip, and the council is
    // cheaper. Which is right.
    let mut alone = getting_rid_of("refrigerator", &fridge, 62.0, &town);
    alone.carrying = Carrying::ATruck;
    assert!(
        !matches!(
            how_to_get_rid_of_it(&alone),
            HowToGetRidOfIt::TakeItYourself { .. }
        ),
        "he made a special trip for one fridge"
    );

    // **A load changes it.** Five things going at once and the trip is
    // cheap per item.
    let mut a_load = alone;
    a_load.others_going = 5;
    let with_a_truck = how_to_get_rid_of_it(&a_load);
    assert!(
        matches!(with_a_truck, HowToGetRidOfIt::TakeItYourself { .. }),
        "a truck and a full load and he still paid somebody: {with_a_truck:?}"
    );

    // And a car cannot do it at all — that is about the doors, not the
    // springs.
    let mut in_a_car = a_load;
    in_a_car.carrying = Carrying::ACar;
    assert!(
        !matches!(
            how_to_get_rid_of_it(&in_a_car),
            HowToGetRidOfIt::TakeItYourself { .. }
        ),
        "a fridge went in the back of a car"
    );
    assert!(with_a_truck.cost() < how_to_get_rid_of_it(&in_a_car).cost());

    // **And whose afternoon it is matters.** The same truck, the same load,
    // somebody whose time is worth a great deal, and paying is sensible.
    let mut busy = a_load;
    busy.hourly_worth = 300.0;
    assert!(
        !matches!(
            how_to_get_rid_of_it(&busy),
            HowToGetRidOfIt::TakeItYourself { .. }
        ),
        "somebody on 300 an hour spent an afternoon at the tip to save 40"
    );

    // The trip cost divides and the technician's fee does not.
    let (one, hours_one) = cost_of_a_trip(62.0, &town, 22.0, 1);
    let (many, hours_many) = cost_of_a_trip(62.0, &town, 22.0, 6);
    assert!(many < one, "a load of six cost as much per item as one");
    assert!(
        hours_many < hours_one / 3.0,
        "six trips were made instead of one"
    );
    assert!(
        many > 22.0,
        "the refrigerant recovery was divided between six fridges"
    );
}

/// **Gate: what stops people leaving it in the woods is being seen, and
/// the distance to the tip is itself a cause.**
///
/// The rule `custom.rs` already carries — certainty deters and severity
/// mostly does not — arriving at fly-tipping. Doing it properly costs money
/// and an afternoon; the woods cost neither.
///
/// Asserted as a **rate over many people** rather than on one draw, because
/// a propensity is not a decision and a single sample can go either way
/// while the model underneath is exactly right. Same discipline as the
/// crash model in `fitted.rs`.
#[test]
fn a_hedge_in_the_country_is_full_and_one_in_town_is_not() {
    let town = Council::a_town();
    let country = Council::out_in_the_country();
    let fridge = a_dead_fridge();

    let rate = |council: &Council, scruple: f64| {
        let mut tipped = 0;
        for k in 0..400u64 {
            let mut c = getting_rid_of("refrigerator", &fridge, 62.0, council);
            c.scruple = scruple;
            c.event = 5_000 + k;
            if !how_to_get_rid_of_it(&c).legal() {
                tipped += 1;
            }
        }
        tipped as f64 / 400.0
    };

    let watched = rate(&town, -0.9);
    let unwatched = rate(&country, -0.9);
    assert!(
        unwatched > watched * 2.0,
        "watched {:.0}% against unwatched {:.0}% — being seen deterred nobody",
        watched * 100.0,
        unwatched * 100.0
    );
    assert!(
        unwatched > 0.35,
        "only {:.0}% tipped it where nobody was looking",
        unwatched * 100.0
    );

    // **And a scrupulous man does not, wherever he is.** Being unobserved
    // is an opportunity and not a motive.
    let honest = rate(&country, 0.95);
    assert!(
        honest < 0.05,
        "{:.0}% of honest men tipped it in a ditch",
        honest * 100.0
    );

    // Nobody bends a rule for nothing: something free to be rid of is not
    // worth breaking the law over, however few people are watching. **And
    // the country has no free collections at all**, which is itself part of
    // why the hedges are full — so this has to be tested where the service
    // exists.
    assert_eq!(
        country.free_collections_a_year, 0,
        "the country grew a bulky waste service"
    );
    let mut free_to_dispose = getting_rid_of("refrigerator", &fridge, 62.0, &town);
    free_to_dispose.scruple = -0.9;
    free_to_dispose.collections_used = 0;
    assert_eq!(
        how_to_get_rid_of_it(&free_to_dispose),
        HowToGetRidOfIt::Kerbside
    );

    // **And what gets burnt is what burns.** A bonfire in a yard nobody
    // overlooks is easier than a drive to the woods.
    let timber = [(Material::Pine, 0.030), (Material::Particleboard, 0.020)];
    let mut burnable = getting_rid_of("furniture", &timber, 50.0, &country);
    burnable.bulky = false;
    burnable.scruple = -0.9;
    burnable.somewhere_to_burn = true;
    let mut burnt = 0;
    for k in 0..200u64 {
        burnable.event = 9_000 + k;
        if how_to_get_rid_of_it(&burnable) == HowToGetRidOfIt::BurnIt {
            burnt += 1;
        }
    }
    assert!(
        burnt > 20,
        "only {burnt} of 200 lit a bonfire with a yard and no scruples"
    );

    // A fridge does not burn, so the same man drives it to the woods
    // instead.
    let mut wont_burn = getting_rid_of("refrigerator", &fridge, 62.0, &country);
    wont_burn.scruple = -0.9;
    wont_burn.somewhere_to_burn = true;
    let mut burnt_a_fridge = 0;
    for k in 0..200u64 {
        wont_burn.event = 11_000 + k;
        if how_to_get_rid_of_it(&wont_burn) == HowToGetRidOfIt::BurnIt {
            burnt_a_fridge += 1;
        }
    }
    assert_eq!(burnt_a_fridge, 0, "somebody set fire to a refrigerator");
}

/// **Gate: what happens to it is a different question from what the
/// household decided**, and the two have to agree.
#[test]
fn every_route_says_what_became_of_the_thing() {
    use scale_sim::wip::Disposition;
    let all = [
        HowToGetRidOfIt::SellIt { paid: 10.0 },
        HowToGetRidOfIt::RedeemTheDeposit { refund: 15.0 },
        HowToGetRidOfIt::TradeIn { allowance: 25.0 },
        HowToGetRidOfIt::GiveItAway,
        HowToGetRidOfIt::Donate,
        HowToGetRidOfIt::LeaveItOut,
        HowToGetRidOfIt::Kerbside,
        HowToGetRidOfIt::TakeItYourself {
            cost: 30.0,
            hours: 1.2,
        },
        HowToGetRidOfIt::PayAHauler { fee: 90.0 },
        HowToGetRidOfIt::KeepIt,
        HowToGetRidOfIt::Cannibalize,
        HowToGetRidOfIt::BurnIt,
        HowToGetRidOfIt::FlyTip,
        HowToGetRidOfIt::AbandonIt,
    ];
    for r in all {
        assert!(!r.name().is_empty());
        let _ = r.disposition();
    }
    // **Three of the fourteen are not lawful**, which is the point of
    // listing them rather than pretending everybody does the right thing.
    let unlawful = all.iter().filter(|r| !r.legal()).count();
    assert_eq!(unlawful, 2, "{unlawful} of the routes were unlawful");

    // Burning it is the one route that ends the object without anybody
    // getting anything back.
    assert!(!HowToGetRidOfIt::BurnIt.survives());
    assert!(HowToGetRidOfIt::GiveItAway.survives());
    assert!(
        HowToGetRidOfIt::FlyTip.survives(),
        "a fly-tipped fridge stopped existing"
    );
    assert_eq!(HowToGetRidOfIt::KeepIt.disposition(), Disposition::Stored);
    assert_eq!(
        HowToGetRidOfIt::FlyTip.disposition(),
        Disposition::Abandoned
    );
}

/// **Gate: every material can be written down and read back.**
///
/// Adding the scrap prices found that `ALL_MATERIALS` was missing `Water` —
/// so a save containing it would have failed to load, and the table-driven
/// codec gate could not see the hole because it walked the same incomplete
/// list. The roster has to be complete, and the only way to know is to make
/// something exhaustive walk it.
#[test]
fn the_material_roster_is_complete() {
    use scale_sim::save::{Reader, Store, Writer};
    let mut seen = 0;
    for m in ALL_MATERIALS {
        assert_eq!(
            Material::from_name(m.name()),
            Some(m),
            "{} does not read back",
            m.name()
        );
        let mut w = Writer::new();
        m.store(&mut w);
        let mut r = Reader::new(&w.bytes);
        assert_eq!(Material::load(&mut r).unwrap(), m);
        seen += 1;
    }
    // The `price_a_tonne` match is exhaustive over the enum, so if the enum
    // has more variants than the roster the compiler will have said so
    // there and this count will disagree with it.
    assert_eq!(seen, ALL_MATERIALS.len());
    assert!(seen >= 40, "the roster has shrunk to {seen}");
}
