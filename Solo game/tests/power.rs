//! **Where the electricity comes from, and what that makes it cost.**
//!
//! The economy had one way to make power: burn coal. Every gate here is
//! something that could not be said at all until there were others.

use scale_sim::power::*;

// =====================================================================
// merit order
// =====================================================================

/// **Gate: the last unit needed sets the price for everybody.**
///
/// The mechanism that makes a generation mix mean anything, and the least
/// intuitive fact in an electricity market: a wind farm with no fuel bill
/// is paid exactly what the gas turbine that happened to be last is paid.
#[test]
fn the_last_plant_called_sets_the_price_for_all_of_them() {
    let fleet = [
        Plant::new(Source::Wind, 400.0),
        Plant::new(Source::Hydro, 300.0),
        Plant::new(Source::Coal, 500.0),
        Plant::new(Source::Gas, 600.0),
    ];

    // A quiet night: the wind and the river cover it and the price is what
    // the river costs, which is almost nothing.
    let quiet = dispatch(&fleet, 500.0);
    assert!((quiet.clearing_price - Source::Hydro.marginal_cost()).abs() < 1e-9);
    assert!(
        quiet.clearing_price < 10.0,
        "a windy night cleared at {:.0}",
        quiet.clearing_price
    );
    assert!(quiet.unserved_mw < 1e-9);

    // A cold evening: everything runs and the gas turbine sets it.
    let peak = dispatch(&fleet, 1_600.0);
    assert!((peak.clearing_price - Source::Gas.marginal_cost()).abs() < 1e-9);
    assert!(peak.clearing_price > quiet.clearing_price * 5.0);

    // **And the wind farm is paid the peak price too**, on the same
    // megawatt-hours it would have sold for nothing an hour earlier.
    let wind_ran = peak
        .running
        .iter()
        .find(|(s, _)| *s == Source::Wind)
        .unwrap()
        .1;
    assert!(wind_ran > 0.0);
    assert!(
        peak.revenue() > peak.production_cost * 1.5,
        "revenue {:.0} against a production cost of {:.0}",
        peak.revenue(),
        peak.production_cost
    );

    // Cheapest first, always: the fuel-burners are last on.
    let order: Vec<Source> = quiet.running.iter().map(|r| r.0).collect();
    assert!(
        !order.contains(&Source::Coal),
        "coal ran while the wind was blowing"
    );
}

/// **Gate: a shortage is not a high price, it is a different thing.**
///
/// Real markets set an administrative cap and it is enormous — ERCOT's was
/// $9,000/MWh in the February 2021 Texas freeze and it sat there for four
/// days, which bankrupted several retailers outright.
#[test]
fn when_there_is_nothing_left_to_call_on() {
    let small = [Plant::new(Source::Gas, 100.0)];
    let short = dispatch(&small, 400.0);
    assert!(
        short.unserved_mw > 290.0,
        "{:.0} MW went unserved",
        short.unserved_mw
    );
    assert_eq!(short.clearing_price, SHORTAGE_PRICE);
    assert!(
        short.served_mw() <= 100.0 + 1e-9,
        "it served more than it had"
    );

    // **And a plant that is not available is not capacity.** A dam in a
    // drought and a wind farm on a still day are the same problem.
    let mut becalmed = [
        Plant::new(Source::Wind, 900.0),
        Plant::new(Source::Gas, 200.0),
    ];
    let blowing = dispatch(&becalmed, 600.0);
    assert!(blowing.unserved_mw < 1e-9 && blowing.clearing_price < 1e-9);
    becalmed[0].available = 0.05;
    let still = dispatch(&becalmed, 600.0);
    assert!(
        still.unserved_mw > 300.0,
        "900 MW of becalmed wind kept the lights on"
    );
    assert_eq!(still.clearing_price, SHORTAGE_PRICE);
}

/// **Gate: what it costs a country over a year depends on what was built,
/// not on what anything is worth per hour.**
#[test]
fn a_country_with_a_river_pays_for_its_dam_once() {
    let demand = 1_000.0;
    let with_a_river = [
        Plant::new(Source::Hydro, 900.0),
        Plant::new(Source::Gas, 700.0),
    ];
    let without = [
        Plant::new(Source::Coal, 900.0),
        Plant::new(Source::Gas, 700.0),
    ];

    let river = annual_cost(&with_a_river, demand);
    let burning = annual_cost(&without, demand);
    assert!(
        river < burning * 0.4,
        "a hydro country spent {river:.0} a year against {burning:.0} burning things"
    );

    // **And its power is mostly not burning anything**, which is what
    // decides whether a hard winter costs money or merely costs water.
    let d = dispatch(&with_a_river, demand);
    assert!(
        d.share_from_free_fuel() > 0.8,
        "{:.0}%",
        d.share_from_free_fuel() * 100.0
    );
    let d2 = dispatch(&without, demand);
    assert!(d2.share_from_free_fuel() < 0.05);
}

/// **Gate: the marginal cost and the levelised cost are different numbers
/// and they rank differently.**
///
/// Which is the whole of the difference between a wind farm and a gas
/// turbine: they may cost the same over thirty years and they behave
/// completely differently on a Tuesday.
#[test]
fn what_it_costs_to_build_and_what_it_costs_to_run_are_not_the_same_ranking() {
    // Nuclear is the dearest thing to build and nearly the cheapest to run.
    assert!(Source::Nuclear.capital_per_kw() > Source::Gas.capital_per_kw() * 5.0);
    assert!(Source::Nuclear.marginal_cost() < Source::Gas.marginal_cost() * 0.4);

    // Wind costs nothing to run and cannot be told when to do it.
    assert_eq!(Source::Wind.marginal_cost(), 0.0);
    assert!(!Source::Wind.dispatchable() && !Source::Solar.dispatchable());
    assert!(Source::Hydro.dispatchable(), "a dam cannot be turned on");

    // And a megawatt is not a megawatt: real capacity factors run from 23%
    // for solar to 93% for nuclear.
    assert!(Source::Nuclear.capacity_factor() > Source::Solar.capacity_factor() * 3.5);
    for s in ALL_SOURCES {
        assert!((0.15..=0.95).contains(&s.capacity_factor()), "{}", s.name());
        assert!(s.marginal_cost() >= 0.0);
        assert!(!s.name().is_empty());
    }

    // Only three of them eat something every hour.
    let burners = ALL_SOURCES.iter().filter(|s| s.burns_fuel()).count();
    assert_eq!(burners, 3);
}

// =====================================================================
// what the ground offers
// =====================================================================

/// **Gate: the hydro equation is the real one.**
///
/// `P = ρ g Q H η` — a hundred metres of head with a hundred cubic metres a
/// second is 88 megawatts, which is a serious power station.
#[test]
fn a_hundred_metres_of_head_and_a_hundred_cumecs_is_eighty_eight_megawatts() {
    let mw = hydro_megawatts(100.0, 100.0);
    assert!((mw - 88.3).abs() < 0.5, "came out at {mw:.1} MW");

    // It is linear in both, which is why a big slow river and a small steep
    // one can be worth the same.
    assert!((hydro_megawatts(200.0, 50.0) - mw).abs() < 0.5);
    assert_eq!(hydro_megawatts(0.0, 500.0), 0.0);
    assert_eq!(hydro_megawatts(500.0, 0.0), 0.0);

    // A trickle down a mountain is not a power station.
    assert!(hydro_megawatts(0.4, 300.0) < 2.0);
}

/// **Gate: the wind resource follows latitude, exposure and height.**
///
/// Derived rather than simulated — the world generator has a prevailing
/// direction and no wind speed — but derived from the things that really do
/// govern it, and recorded as an inference.
#[test]
fn the_windiest_places_are_exposed_coasts_in_the_westerlies() {
    // The westerly belt, about 35 to 60 degrees, is the windiest band on
    // earth; the horse latitudes near 30 are the calmest.
    let scotland = wind_resource(56.0, true, 200.0, 0.2);
    let sahara = wind_resource(28.0, false, 300.0, 0.0);
    let equator = wind_resource(2.0, false, 100.0, 0.8);
    assert!(
        scotland > sahara,
        "56N coast {scotland:.2} against 28N inland {sahara:.2}"
    );
    assert!(scotland > equator);
    assert!(
        scotland > 0.7,
        "an exposed Atlantic coast came out at {scotland:.2}"
    );

    // Exposure matters: nothing upwind to slow it.
    let coast = wind_resource(52.0, true, 50.0, 0.1);
    let inland = wind_resource(52.0, false, 50.0, 0.1);
    assert!(coast > inland * 1.2, "coast {coast:.2} inland {inland:.2}");

    // And roughness takes it out again — a forest is not a wind farm.
    let bare = wind_resource(52.0, false, 400.0, 0.0);
    let wooded = wind_resource(52.0, false, 400.0, 1.0);
    assert!(bare > wooded * 1.4);

    // The trade winds are real and gentler than the westerlies.
    assert!(wind_resource(15.0, true, 10.0, 0.0) > wind_resource(30.0, true, 10.0, 0.0));
}

/// **Gate: sunshine is latitude and cloud, and the spread is a factor of
/// two.**
///
/// Real annual irradiation: the US Southwest about 2,000 kWh/m², Germany
/// about 1,000 — which is why the same panel is worth twice as much in
/// Arizona.
#[test]
fn the_same_panel_is_worth_twice_as_much_in_arizona() {
    let arizona = solar_resource(34.0, 0.05);
    let germany = solar_resource(51.0, 0.65);
    assert!(
        arizona > germany * 1.7,
        "Arizona {arizona:.2} Germany {germany:.2}"
    );
    assert!(solar_resource(70.0, 0.5) < solar_resource(20.0, 0.5));
    // Cloud alone makes a real difference at the same latitude.
    assert!(solar_resource(40.0, 0.0) > solar_resource(40.0, 0.9) * 1.6);
}

/// **Gate: a nation builds what the ground offers, and fills the gap with
/// whatever burns.**
#[test]
fn the_endowment_picks_the_mix() {
    let demand = 1_000.0;

    // Norway: a great river, and it needs almost nothing else.
    let norway = Potential {
        hydro_mw: 1_400.0,
        wind: 0.7,
        solar: 0.2,
        geothermal: 0.1,
        coal: 0.0,
    };
    let built = what_they_would_build(&norway, demand, false);
    assert!(built.iter().any(|p| p.source == Source::Hydro));
    let hydro_mw: f64 = built
        .iter()
        .filter(|p| p.source == Source::Hydro)
        .map(|p| p.mw)
        .sum();
    assert!(
        hydro_mw > demand,
        "only {hydro_mw:.0} MW of hydro on a river worth 1,400"
    );

    // Poland: coal under it and not much else.
    let poland = Potential {
        hydro_mw: 20.0,
        wind: 0.35,
        solar: 0.3,
        geothermal: 0.02,
        coal: 0.8,
    };
    let built = what_they_would_build(&poland, demand, false);
    assert!(
        built.iter().any(|p| p.source == Source::Coal),
        "a coalfield built no coal station"
    );
    assert!(
        !built.iter().any(|p| p.source == Source::Solar),
        "solar in a cloudy coal country"
    );

    // Iceland: hot rock, and it works at night, which is the whole point.
    let iceland = Potential {
        hydro_mw: 600.0,
        wind: 0.8,
        solar: 0.15,
        geothermal: 0.9,
        coal: 0.0,
    };
    let built = what_they_would_build(&iceland, demand, false);
    assert!(built.iter().any(|p| p.source == Source::Geothermal));

    // And the mixes really do cost different amounts to run.
    let n = annual_cost(&what_they_would_build(&norway, demand, false), demand);
    let p = annual_cost(&what_they_would_build(&poland, demand, false), demand);
    assert!(n < p * 0.5, "Norway {n:.0} against Poland {p:.0}");
}

// =====================================================================
// and what it changes
// =====================================================================

/// **Gate: cheap power does not make cheap steel. It changes which route
/// is worth building.**
///
/// The correction that mattered. A blast furnace uses coal as a *reductant*
/// and only 250 kWh of electricity a tonne, so halving the power price
/// barely touches it. An aluminium potline is 14 MWh a tonne and about 40%
/// of its cost is the bill.
#[test]
fn the_power_price_matters_enormously_to_some_things_and_not_at_all_to_others() {
    let dear = 90.0;
    let cheap = 20.0;

    // A blast furnace hardly notices.
    let blast_dear = Route::BlastFurnace.cost_a_tonne(dear, 500.0);
    let blast_cheap = Route::BlastFurnace.cost_a_tonne(cheap, 500.0);
    assert!(
        blast_dear < blast_cheap * 1.05,
        "cheap power changed blast-furnace steel from {blast_cheap:.0} to {blast_dear:.0}"
    );
    assert!(
        Route::BlastFurnace.power_share_of_cost(dear) < 0.06,
        "electricity was {:.0}% of blast-furnace steel",
        Route::BlastFurnace.power_share_of_cost(dear) * 100.0
    );

    // **An aluminium potline is a machine for turning electricity into
    // metal**, and real figures put power at about 40% of the cost.
    let share = Route::PrimaryAluminium.power_share_of_cost(45.0);
    assert!(
        (0.28..0.52).contains(&share),
        "electricity was {:.0}% of primary aluminium",
        share * 100.0
    );
    assert!(Route::PrimaryAluminium.mwh_a_tonne() > Route::BlastFurnace.mwh_a_tonne() * 40.0);

    // **And remelting is 5% of primary**, which is exactly why scrap
    // aluminium is worth $1,400 a tonne.
    let ratio = Route::SecondaryAluminium.mwh_a_tonne() / Route::PrimaryAluminium.mwh_a_tonne();
    assert!(
        (0.03..0.08).contains(&ratio),
        "remelt was {:.0}% of primary",
        ratio * 100.0
    );
}

/// **Gate: the endowment picks the steelmaking route, and it is not the
/// power price alone.**
///
/// The US runs 70% electric arc because it has an enormous scrap pool; the
/// world runs 70% blast furnace because it has ore and coal; Brazil still
/// makes pig iron on charcoal because it has forest. **No scrap, no arc
/// furnace**, whatever the electricity costs.
#[test]
fn a_country_makes_its_steel_the_way_its_endowment_allows() {
    // An old industrial country: cheap power, a deep scrap pool.
    assert_eq!(steel_route(25.0, 0.5, 0.9, 0.2), Route::ElectricArc);

    // **The same cheap power with nothing to melt.** An arc furnace is not
    // an option at any price, which is why a developing country cannot
    // simply choose the modern route.
    assert_eq!(steel_route(25.0, 0.6, 0.05, 0.2), Route::BlastFurnace);

    // Forest, no coal, no scrap: charcoal, which is how it was done for
    // eight hundred years and still is in Brazil.
    assert_eq!(steel_route(60.0, 0.05, 0.05, 0.95), Route::CharcoalIron);

    // **And the power price does not flip it**, which is the module's own
    // headline arriving as a result rather than as a claim. The two routes
    // differ by 0.2 MWh a tonne, so even a punishing tariff is $24 against
    // $460 of ore, coal, scrap and conversion. Germany has dear power, coal
    // and a scrap pool and runs blast furnaces; Italy has dear power, no
    // coal and a scrap pool and runs arc furnaces. It is the scrap.
    for power in [15.0, 45.0, 120.0, 250.0] {
        assert_eq!(
            steel_route(power, 0.5, 0.9, 0.2),
            Route::ElectricArc,
            "power at {power} changed how a scrap-rich country makes steel"
        );
        assert_eq!(
            steel_route(power, 0.6, 0.05, 0.2),
            Route::BlastFurnace,
            "power at {power} conjured scrap out of nothing"
        );
    }

    // **Whereas aluminium is decided by nothing else.**
    assert!(worth_a_potline(22.0) && !worth_a_potline(70.0));
}

/// **Gate: a smelter is sited by contract, not by proximity to the dam.**
///
/// Grid losses over a few hundred kilometres are about 5%, so what matters
/// is a forty-year power purchase agreement at a price nobody else gets —
/// which a 500 MW continuous load can negotiate and nobody else can.
#[test]
fn a_potline_needs_a_contract_rather_than_a_dam_next_door() {
    // A big, long, unvarying load gets a real discount, and it is not
    // charity: it is the cheapest load a generator can serve.
    let market = 55.0;
    let small_and_short = industrial_contract(market, 20.0, 2);
    let big_and_long = industrial_contract(market, 600.0, 40);
    assert!(
        big_and_long < small_and_short * 0.75,
        "{big_and_long:.0} against {small_and_short:.0}"
    );
    assert!(big_and_long < market * 0.7);
    // And there is a floor: nobody sells below what it costs them.
    assert!(industrial_contract(market, 100_000.0, 400) >= market * 0.35 - 1e-9);

    // **Almost nowhere is worth a potline**, which is why there are so few.
    assert!(
        !worth_a_potline(55.0),
        "a potline at the ordinary market price"
    );
    assert!(worth_a_potline(industrial_contract(45.0, 600.0, 40)));
    assert!(
        !worth_a_potline(industrial_contract(120.0, 600.0, 40)),
        "a potline in a country with expensive power"
    );
}
