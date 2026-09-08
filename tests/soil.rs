use scale_sim::field::Field;
use scale_sim::world::World;

/// Mean over cells matching a predicate.
fn mean_of(w: &World, f: &Field, pick: &dyn Fn(usize) -> bool) -> f64 {
    let c: Vec<usize> = (0..f.data.len())
        .filter(|&i| w.elevation.data[i] >= w.sea_level && pick(i))
        .collect();
    c.iter().map(|&i| f.data[i] as f64).sum::<f64>() / c.len().max(1) as f64
}

#[test]
fn soil_collects_where_the_ground_collects_it() {
    // **Slope and curvature are the first-order predictors.** Soil thins
    // on convex ground where it is shed and thickens in concave ground
    // where it gathers: bare rock on a sharp ridge, 0.2-0.5 m on steep
    // ground, 1-2 m on a planar slope, several metres of alluvium on a
    // floodplain.
    let w = World::generate(256, 144, 20260828);
    let (ww, hh) = (w.width, w.height);

    let curvature = |i: usize| -> f32 {
        let (x, y) = (i % ww, i / ww);
        if y == 0 || y + 1 >= hh {
            return 0.0;
        }
        let e = w.elevation.data[i];
        let m = (w.elevation.data[y * ww + (x + ww - 1) % ww]
            + w.elevation.data[y * ww + (x + 1) % ww]
            + w.elevation.data[(y - 1) * ww + x]
            + w.elevation.data[(y + 1) * ww + x])
            * 0.25;
        m - e
    };

    let hollow = mean_of(&w, &w.soil_depth, &|i| curvature(i) > 0.0005);
    let ridge = mean_of(&w, &w.soil_depth, &|i| curvature(i) < -0.0005);
    assert!(
        hollow > ridge * 1.3,
        "hollows carry {hollow:.2} m of soil against {ridge:.2} m on ridges — \
         curvature is not moving anything"
    );

    // Floodplains are deposited rather than weathered, so they are deeper
    // than the hillsides that fed them.
    let flood = mean_of(&w, &w.soil_depth, &|i| w.river[i] || w.lake[i]);
    let upland = mean_of(&w, &w.soil_depth, &|i| !w.river[i] && !w.lake[i]);
    assert!(
        flood > upland * 1.4,
        "floodplains at {flood:.2} m against {upland:.2} m of upland soil"
    );

    // Real depths, not just an ordering.
    let all = mean_of(&w, &w.soil_depth, &|_| true);
    assert!(
        (0.4..3.0).contains(&all),
        "the planet averages {all:.2} m of soil"
    );
    let deepest = (0..w.soil_depth.data.len())
        .map(|i| w.soil_depth.data[i])
        .fold(0.0f32, f32::max);
    assert!(deepest <= 8.01, "{deepest:.1} m of soil somewhere");
}

#[test]
fn deep_soil_carries_a_dry_season_and_thin_soil_does_not() {
    // **This is what soil depth is for**, and it has to be tested with
    // everything else held still. In the world at large deep soil
    // correlates with floodplains, so a quartile comparison across cells
    // says the opposite of the truth — it is comparing climates, not soils.
    //
    // Same forage, same temperature, same season, same rain. Only the
    // depth differs.
    use scale_sim::biota::settle;
    let climate = |soil_m: f32| {
        settle(
            600_000.0, // kg/km²/yr of forage
            18.0,      // mean °C
            18.0,      // summer-to-winter range
            0.45,      // climatic moisture: semi-arid
            soil_m, 500.0, // mm of rain a year
            0.22,  // winter-wet: a Mediterranean dry season
            8.0,   // water table well below the root zone
        )
    };
    let thin = climate(0.25);
    let deep = climate(1.5);

    assert!(
        deep.plant_winter > thin.plant_winter * 1.15,
        "thin soil ends the year on {:.0} kg/km2 and deep soil on {:.0} - depth is buying nothing",
        thin.plant_winter,
        deep.plant_winter
    );
    // And it carries more stock, because the herd eats through the year
    // and not only in spring.
    assert!(
        deep.herbivore >= thin.herbivore,
        "deep soil carries less game than thin"
    );

    // **Water below the root zone is present and inaccessible.** Past
    // about a metre and a half of rootable depth, more soil buys nothing,
    // which is why a floodplain is not infinitely productive.
    let deeper = climate(6.0);
    assert!(
        (deeper.plant_winter - deep.plant_winter).abs() < deep.plant_winter * 0.05,
        "four more metres of subsoil changed what the grass could reach"
    );
}

#[test]
fn snow_waters_the_thaw_and_not_january() {
    // **Precipitation below freezing is snow and feeds nothing until it
    // melts.** Without a store, a boreal cell waters its plants in
    // January and goes dry in June, which is backwards. The tell is that
    // cold seasonal country still grows something: if winter rain were
    // being spent in winter there would be nothing left for the summer.
    let w = World::generate(256, 144, 20260828);
    let cold: Vec<usize> = (0..w.biomes.len())
        .filter(|&i| {
            w.elevation.data[i] >= w.sea_level
                && (-5.0..5.0).contains(&w.temperature_c(i))
                && w.seasonality.data[i] > 10.0
        })
        .collect();
    assert!(!cold.is_empty(), "no cold seasonal country on this world");
    let summer = cold
        .iter()
        .map(|&i| w.biota.standing_summer.data[i] as f64)
        .sum::<f64>()
        / cold.len() as f64;
    assert!(
        summer > 5_000.0,
        "cold seasonal country grows {summer:.0} kg/km² in summer — \
         its winter precipitation is being spent in winter"
    );
}

#[test]
fn heat_takes_the_water_back() {
    // Equal rainfall is not equal water. Evaporative demand rises with
    // temperature, so the hotter of two equally rained-on cells retains
    // less — which is the whole reason the moisture index exists rather
    // than using rainfall directly.
    let w = World::generate(256, 144, 20260828);
    let land: Vec<usize> = (0..w.biomes.len())
        .filter(|&i| w.elevation.data[i] >= w.sea_level)
        .collect();
    // Matched on rainfall, split on temperature.
    let band: Vec<usize> = land
        .iter()
        .copied()
        .filter(|&i| (600.0..900.0).contains(&w.rainfall_mm(i)))
        .collect();
    assert!(band.len() > 100, "not enough cells at a matched rainfall");
    let mut by_temp = band.clone();
    by_temp.sort_by(|&a, &b| {
        w.temperature_c(a)
            .total_cmp(&w.temperature_c(b))
            .then(a.cmp(&b))
    });
    let q = by_temp.len() / 4;
    let m = |c: &[usize]| {
        c.iter()
            .map(|&i| w.climatic_moisture.data[i] as f64)
            .sum::<f64>()
            / c.len() as f64
    };
    let cool = m(&by_temp[..q]);
    let hot = m(&by_temp[by_temp.len() - q..]);
    assert!(
        cool > hot,
        "at the same rainfall, hot ground holds {hot:.2} against cool ground's {cool:.2}"
    );
}

#[test]
fn a_waterlogged_floodplain_reads_wet_and_roots_shallow() {
    // **Roots need air as much as water**, so where the water table
    // stands inside the root zone the soil is waterlogged and roots
    // suffocate in it. Field drainage exists for exactly this reason.
    //
    // Note what it does and does not do: the *water* is still there — more
    // of it, because capillary rise carries it up — and what suffers is
    // the plant's ability to use it. Which is why the penalty belongs to
    // the crop and not to the water figure. Rice does not care.
    use scale_sim::biota::{settle, Crop};
    let at_water_table =
        |depth_m: f32| settle(600_000.0, 18.0, 18.0, 0.45, 3.0, 500.0, 0.22, depth_m);
    let drained = at_water_table(8.0);
    let waterlogged = at_water_table(0.4);

    // Natural vegetation suffers: it cannot choose to be rice.
    assert!(
        waterlogged.plant_winter < drained.plant_winter * 0.9,
        "a water table 40 cm down costs nothing: {:.0} against {:.0} kg/km2 drained",
        waterlogged.plant_winter,
        drained.plant_winter
    );
    // And there is *more* water, not less — the capillary fringe is
    // feeding the root zone from below.
    assert!(
        waterlogged.crop_water_mm > drained.crop_water_mm,
        "a shallow water table is not reaching the root zone at all"
    );
    assert!(waterlogged.aeration < 0.3 && drained.aeration > 0.9);

    // Wheat drowns; the land is not therefore worthless.
    assert!(
        Crop::Wheat.yield_t_per_ha(18.0, waterlogged.crop_water_mm, waterlogged.aeration) < 1.0
    );
}

#[test]
fn groundwater_is_a_resource_in_dry_country_and_a_liability_in_wet() {
    // **The water table is not only a hazard.** Capillary rise carries
    // water up into the root zone, which is why a floodplain or an oasis
    // grows anything in country that has no business growing anything —
    // the Nile, in one sentence. It was doing nothing here, so a shallow
    // table could only ever hurt.
    //
    // For a crop that drowns, the relationship is humped: too shallow and
    // the roots suffocate, too deep and it is out of reach, with an
    // optimum a metre or two down — which is where field drainage aims to
    // hold it.
    use scale_sim::biota::{settle, Crop};
    let dry_wheat = |table_m: f32| {
        let s = settle(300_000.0, 20.0, 16.0, 0.25, 1.5, 250.0, 0.8, table_m);
        Crop::Wheat.yield_t_per_ha(s.season_c, s.crop_water_mm, s.aeration)
    };
    let (drowned, optimum, deep) = (dry_wheat(0.3), dry_wheat(1.5), dry_wheat(20.0));

    assert!(
        optimum > deep * 3.0,
        "groundwater at 1.5 m yields {optimum:.1} t/ha against {deep:.1} where it is out of \
         reach - capillary rise is feeding nothing"
    );
    assert!(
        drowned < deep,
        "wheat on a table 30 cm down yields {drowned:.1} t/ha, more than on dry ground"
    );
    assert!(
        (0.5..3.5).contains(&deep),
        "250 mm of rain alone yields {deep:.1} t/ha of wheat"
    );

    // **But somebody else farms that ground.** The land wheat cannot use
    // is the land that feeds the most people anywhere on Earth.
    let marsh = settle(300_000.0, 26.0, 16.0, 0.25, 1.5, 250.0, 0.8, 0.3);
    let (crop, y) =
        scale_sim::biota::best_crop(marsh.season_c, marsh.crop_water_mm, marsh.aeration);
    assert_eq!(crop, Crop::Rice, "a warm marsh grows nothing worth having");
    assert!(y > 3.0, "a rice paddy yields {y:.1} t/ha");
}
