//! **What a town has room to build on, against how many people want to
//! live there.**
//!
//! `cargo run --release --bin housing`
//!
//! Written to answer one question before anything was built on it: is
//! there a real spread between towns, or does every town come out alike?
//! If they come out alike there is no shortage anywhere to price, and a
//! housing market would be a constant wearing a mechanism's name.
//!
//! **The first thing it measured killed the first design.** `townplan`'s
//! `Plan::housed()` looked like the stock — it counts the dwellings on the
//! plots — and at the size the economy lays a plan out it holds about
//! fourteen thousand people against populations of nine to thirty-seven
//! million. That is 0.1%, and it is not a bug: `size` is how much ground
//! is generated, not how big the place is, which this project already
//! records. A 1.28 km patch is not a city.
//!
//! So the supply side is measured the way the published work measures it.
//! **Saiz (2010)** estimates developable land from satellite terrain and
//! water, finding that residential development is effectively curtailed by
//! steep ground, and that housing supply elasticity moves from about
//! **2.45 to 1.25** across the interquartile range of land availability in
//! an average-regulated metro of a million people. Here the same question
//! is asked of the cells a town actually draws on.
//!
//! Real anchors *(US Census, Housing Vacancies and Homeownership, Q2
//! 2026)*: the **rental vacancy rate is 7.3%** and the **homeowner vacancy
//! rate 1.2%** — a loose market and a tight one side by side. Over seventy
//! years the rental rate has run about 4-11.5% and the homeowner rate
//! 0.5-3.0%, so those are bands a model should be able to produce rather
//! than one number it should hit.

use scale_sim::econ::Doctrine;
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::region::Nations;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

fn main() {
    let mut seed = 7u64;
    let mut nations = 4usize;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => seed = it.next().and_then(|v| v.parse().ok()).unwrap_or(seed),
            "--nations" => nations = it.next().and_then(|v| v.parse().ok()).unwrap_or(nations),
            _ => {}
        }
    }

    println!("generating a world (seed {seed}, {nations} nations)...");
    let world = World::generate(384, 216, seed);
    let polities = Polities::partition(&world, 24);
    let settlements = Settlements::place(&world, &polities, 3000);
    let network = Network::build(&world, &settlements, 500);
    let n = Nations::build(
        &world,
        &polities,
        &settlements,
        &network,
        nations,
        4,
        Doctrine::Prudent,
    );
    let e = &n.economy;

    println!(
        "\n  {:<18} {:>12} {:>9} {:>10} {:>12} {:>10} {:>9} {:>8} {:>9} {:>6} {:>7}",
        "town",
        "people",
        "relief",
        "buildable",
        "people/km2",
        "price now",
        "rent/day",
        "years",
        "pressure",
        "land",
        "height"
    );
    let mut density: Vec<(String, f64)> = Vec::new();
    let mut years: Vec<f64> = Vec::new();
    for m in 0..e.markets.len() {
        let mk = &e.markets[m];
        let Some(cell) = mk.cell else { continue };
        let relief = world.relief_m_per_km(cell);
        let km2 = mk.buildable_km2.unwrap_or(0.0);
        let r = scale_sim::world::HOUSING_REACH_CELLS as f64 * scale_sim::region::KM_PER_CELL;
        let share = km2 / (std::f64::consts::PI * r * r);
        // A town with no gentle ground has no density to print: it is read
        // at the pressure ceiling, and the column says so rather than
        // printing people divided by nothing.
        let per_km2 = if km2 > 0.0 {
            mk.population / km2
        } else {
            f64::INFINITY
        };
        density.push((mk.name.clone(), per_km2));
        // **The independent check.** The two exponents are fitted to two
        // observed spreads, which leaves no degrees of freedom and so no
        // validation. What nothing in that fitting touches is the
        // *level* — what a house costs against what somebody earns — so
        // that is the figure to read against the real 7.1 years.
        let pay =
            scale_sim::person::day_rate(e, m, scale_sim::occupation::Occupation::ProductionWorker)
                * 250.0;
        years.push(e.house_price(m) / pay.max(1e-9));
        // **Where the town stands against the switch to building up**: its
        // pressure, land's share of a dwelling's value, and how much dearer
        // its buildings are for being stacked.
        let pressure = e.housing_pressure(m);
        let land = e.land_value_ratio(m);
        let height = scale_sim::econ::Economy::height_premium(pressure);
        println!(
            "  {:<18} {:>12.3e} {:>8.0} {:>9.1}% {:>12.0} {:>10.3e} {:>9.3e} {:>8.1} {:>9.2} {:>5.1}% {:>7.2}",
            mk.name,
            mk.population,
            relief,
            share * 100.0,
            per_km2,
            e.house_price(m),
            scale_sim::person::rent_per_day(e, m),
            e.house_price(m) / pay.max(1e-9),
            pressure,
            100.0 * land / (land + height),
            height,
        );
    }
    println!(
        "\n  past pressure {:.2}, where land would pass {:.1}% of a dwelling's value, the land \
         curve is no longer followed and a town is valued as building up",
        scale_sim::econ::Economy::pressure_where_towns_build_up(),
        100.0 * scale_sim::econ::Economy::LAND_SHARE_AT_MOST,
    );

    density.sort_by(|a, b| a.1.total_cmp(&b.1));
    let (lo_n, lo) = density.first().cloned().unwrap_or_default();
    let (hi_n, hi) = density.last().cloned().unwrap_or_default();
    println!(
        "\n  loosest {lo_n} at {lo:.0} people per buildable km2, tightest {hi_n} at {hi:.0}\n  \
         spread {:.2}x",
        hi / lo.max(1e-9)
    );
    println!(
        "\n  real: Saiz (2010) puts housing supply elasticity at 2.45 to 1.25 across the\n  \
         interquartile range of land availability; the US rental vacancy rate is 7.3%\n  \
         and the homeowner rate 1.2% (Census HVS, Q2 2026)."
    );
}
