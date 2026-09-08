//! **Every variant, by name.**
//!
//! `save.rs` already records why: *"an enum variant no test happens to
//! exercise is exactly the one that silently does not come back"*, which is
//! why the WIP codec is gated by a table naming nine kinds of progress,
//! five placements, four work statuses and all thirty-nine materials.
//!
//! The economy's codec has seven enums with forty-odd variants between
//! them. A round-trip test over a generated world exercises whichever ones
//! that world happens to contain, which is not the same thing at all — a
//! country with no tunnel never encodes a tunnel, and a country with no
//! chemical works never encodes one.
//!
//! Each value is also written **part way through** rather than at either
//! end of its range, because a zero survives a codec that drops the field.

use scale_sim::building::{Building, Fixture};
use scale_sim::econ::{
    Basket, Cause, Commodity, Crossing, Level, Market, Opening, Route, Site, SiteKind, Surface,
    N_COMMODITIES,
};
use scale_sim::econ_codec::{load_basket, store_basket};
use scale_sim::quote::RouteId;
use scale_sim::save::{Reader, SaveError, Store, Writer};
use scale_sim::state::Capacity;

fn there_and_back<T: Store + PartialEq + std::fmt::Debug>(v: T) -> T {
    let mut w = Writer::new();
    v.store(&mut w);
    let mut r = Reader::new(&w.bytes);
    let back = T::load(&mut r).expect("it would not load back");
    assert_eq!(back, v, "a value changed on the way through the bytes");
    back
}

/// **A basket is a reading per commodity, not eighteen numbers in a row.**
///
/// Written positionally it would break every save the day a nineteenth
/// commodity is added — which the resource work is going to do, and which
/// is the same defect the shipment's commodity had. The pairs mean a save
/// made before limestone existed loads into a world that has it with every
/// figure on the commodity it was measured for.
#[test]
fn a_basket_survives_a_commodity_being_added() {
    let mut b: Basket = [0.0; N_COMMODITIES];
    for (i, &c) in Commodity::ALL.iter().enumerate() {
        // Distinct, and none of them zero — a zero survives a codec that
        // drops the field.
        b[c as usize] = 1.5 + i as f64 * 7.25;
    }
    let mut w = Writer::new();
    store_basket(&mut w, &b);
    let back = load_basket(&mut Reader::new(&w.bytes)).expect("a basket would not load");
    assert_eq!(back, b);

    // **A column this build does not know about is dropped, not fatal.**
    // Refusing a whole world over one unknown commodity would make every
    // future commodity a breaking change; what must survive is every
    // figure that *can* be represented.
    let mut w = Writer::new();
    w.len(2);
    w.u16(9_999); // something a later build added
    w.f64(1_234.5);
    w.u16(Commodity::Grain.wire_code());
    w.f64(88.0);
    let back = load_basket(&mut Reader::new(&w.bytes))
        .expect("an unknown commodity column refused the whole basket");
    assert_eq!(back[Commodity::Grain as usize], 88.0);
    assert_eq!(
        back.iter().filter(|v| **v != 0.0).count(),
        1,
        "an unknown column landed on some other commodity"
    );
}

/// **Every economic enum, named one at a time.**
#[test]
fn every_variant_comes_back_as_itself() {
    // Twenty-one kinds of works, and the point of listing them is that
    // adding a twenty-second fails to compile in the codec and then fails
    // here until somebody has said what it is called on disk.
    for k in [
        SiteKind::Farm,
        SiteKind::Pasture,
        SiteKind::Butcher,
        SiteKind::Mill,
        SiteKind::Factory,
        SiteKind::Mine,
        SiteKind::IronMine,
        SiteKind::OilField,
        SiteKind::Forestry,
        SiteKind::Steelworks,
        SiteKind::Works,
        SiteKind::Cracker,
        SiteKind::MachineWorks,
        SiteKind::CementWorks,
        SiteKind::ChemicalWorks,
        SiteKind::Pharma,
        SiteKind::Builders,
        SiteKind::Hospital,
        SiteKind::PowerPlant,
        SiteKind::Shop,
        SiteKind::Depot,
    ] {
        there_and_back(k);
    }

    for s in [
        Surface::Water,
        Surface::Highway,
        Surface::Road,
        Surface::Track,
        Surface::Open,
    ] {
        there_and_back(s);
    }

    for l in [
        Level::Service,
        Level::Transformer,
        Level::Feeder,
        Level::Substation,
        Level::Transmission,
    ] {
        there_and_back(l);
    }

    for c in [Cause::Conductor, Cause::Transformer] {
        there_and_back(c);
    }

    for c in [Capacity::Developed, Capacity::Middling, Capacity::Weak] {
        there_and_back(c);
    }

    // **A crossing carries figures, and they are written part way
    // through.** A pass at zero relief in weather that never closes it
    // would survive a codec that dropped both fields.
    there_and_back(Crossing::Level);
    there_and_back(Crossing::Pass {
        summit: 0.62,
        cold: 0.41,
    });
    there_and_back(Crossing::Tunnel { capital: 1_308.0 });

    // A fault names the thing that failed, and the name is the whole
    // content — an empty one would pass a codec that wrote no string.
    there_and_back(scale_sim::econ::Fault::Line("the main line".into()));
    there_and_back(scale_sim::econ::Fault::Transformer("Ashford T2".into()));

    for f in [
        Fixture::Till,
        Fixture::Shelving,
        Fixture::StockRack,
        Fixture::LoadingBay,
        Fixture::Counter,
        Fixture::ChillCabinet,
        Fixture::ColdStore,
    ] {
        there_and_back(f);
    }

    there_and_back(RouteId(4_294_967_296));
}

/// **No two things share a code inside one table.**
///
/// A collision is not a load error — it is two different things loading as
/// the same thing, which is the quietest failure a save format has.
#[test]
fn no_two_variants_share_a_code() {
    fn codes<T: Store>(all: &[T]) -> Vec<Vec<u8>> {
        all.iter()
            .map(|v| {
                let mut w = Writer::new();
                v.store(&mut w);
                w.bytes
            })
            .collect()
    }
    let mut all = codes(&[
        SiteKind::Farm,
        SiteKind::Pasture,
        SiteKind::Butcher,
        SiteKind::Mill,
        SiteKind::Factory,
        SiteKind::Mine,
        SiteKind::IronMine,
        SiteKind::OilField,
        SiteKind::Forestry,
        SiteKind::Steelworks,
        SiteKind::Works,
        SiteKind::Cracker,
        SiteKind::MachineWorks,
        SiteKind::CementWorks,
        SiteKind::ChemicalWorks,
        SiteKind::Pharma,
        SiteKind::Builders,
        SiteKind::Hospital,
        SiteKind::PowerPlant,
        SiteKind::Shop,
        SiteKind::Depot,
    ]);
    let n = all.len();
    all.sort();
    all.dedup();
    assert_eq!(all.len(), n, "two kinds of works share a code on disk");

    let mut lvl = codes(&[
        Level::Service,
        Level::Transformer,
        Level::Feeder,
        Level::Substation,
        Level::Transmission,
    ]);
    let n = lvl.len();
    lvl.sort();
    lvl.dedup();
    assert_eq!(lvl.len(), n, "two grid levels share a code on disk");
}

/// **An unknown code is refused and says which table it came from.**
#[test]
fn an_unknown_code_names_its_table() {
    let mut w = Writer::new();
    w.u16(9_999);
    assert!(
        matches!(
            SiteKind::load(&mut Reader::new(&w.bytes)),
            Err(SaveError::UnknownCode("site kind", 9_999))
        ),
        "an unknown works loaded as something"
    );

    let mut w = Writer::new();
    w.u8(99);
    assert!(
        matches!(
            Surface::load(&mut Reader::new(&w.bytes)),
            Err(SaveError::UnknownCode("road surface", 99))
        ),
        "an unknown road surface loaded as something"
    );
}

fn a_full_basket(seed: f64) -> Basket {
    let mut b: Basket = [0.0; N_COMMODITIES];
    for (i, &c) in Commodity::ALL.iter().enumerate() {
        b[c as usize] = seed + i as f64;
    }
    b
}

/// **The structures, with every field set to something distinguishable.**
#[test]
fn a_road_a_market_and_a_works_come_back_whole() {
    there_and_back(Route {
        id: RouteId(7),
        name: "Ashford to Bexley".into(),
        a: 3,
        b: 5,
        freight_cost: 45.5,
        sound_cost: 41.25,
        km: 173.0,
        surface: Surface::Highway,
        crossing: Crossing::Pass {
            summit: 0.58,
            cold: 0.3,
        },
        snowed_in: true,
        capacity: 500.0,
        open: true,
        moved: Some((Commodity::Steel, 4, 88.5)),
    });

    there_and_back(Market {
        name: "Bexley".into(),
        nation: 2,
        population: 1_250_000.0,
        southern: true,
        harvest_quality: 0.91,
        price: a_full_basket(10.0),
        cost: a_full_basket(200.0),
        landed: a_full_basket(300.0),
        cover: a_full_basket(4.0),
        expected_cover: a_full_basket(5.0),
    });

    let mut capacity = a_full_basket(10_000.0);
    let mut stock = a_full_basket(100.0);
    // Stock must not exceed the store, which the codec now refuses.
    for &c in Commodity::ALL.iter() {
        capacity[c as usize] = 50_000.0;
        stock[c as usize] = 100.0 + c as usize as f64;
    }
    there_and_back(Site {
        name: "Bexley cannery".into(),
        kind: SiteKind::Factory,
        market: 1,
        stock,
        capacity,
        recipe: Some(0),
        throughput: 420.0,
        powered: true,
        ran: 380.0,
        cost_factor: 1.15,
        fitted: Some(Building {
            fixtures: vec![(Fixture::Till, 12.0), (Fixture::ColdStore, 2.0)],
        }),
    });

    there_and_back(Opening {
        day: 4_242,
        price: vec![a_full_basket(1.0), a_full_basket(2.0)],
        cover: vec![a_full_basket(3.0)],
        landed: vec![a_full_basket(4.0), a_full_basket(5.0)],
        stock: vec![a_full_basket(6.0)],
    });
}

/// **What cannot be true is refused here too**, and each rejection is
/// provoked, because a validator that has only ever seen clean data is
/// untested.
#[test]
fn a_world_that_cannot_be_true_is_refused() {
    let sound_route = || Route {
        id: RouteId(1),
        name: "a road".into(),
        a: 0,
        b: 1,
        freight_cost: 45.0,
        sound_cost: 45.0,
        km: 173.0,
        surface: Surface::Road,
        crossing: Crossing::Level,
        snowed_in: false,
        capacity: 500.0,
        open: true,
        moved: None,
    };
    let refuse = |r: Route, what: &str| {
        let mut w = Writer::new();
        r.store(&mut w);
        match Route::load(&mut Reader::new(&w.bytes)) {
            Err(SaveError::Impossible(_)) => {}
            other => panic!("{what} was accepted: {other:?}"),
        }
    };
    let mut r = sound_route();
    r.id = RouteId(0);
    refuse(r, "a road with no name");
    let mut r = sound_route();
    r.b = r.a;
    refuse(r, "a road from a town to itself");
    let mut r = sound_route();
    r.km = -1.0;
    refuse(r, "a road of negative length");

    // A works holding more than its own store is a contradiction the
    // allocation code would then try to reconcile for ever.
    let mut capacity: Basket = [10.0; N_COMMODITIES];
    let stock: Basket = [5.0; N_COMMODITIES];
    capacity[Commodity::Grain as usize] = 1.0;
    let site = Site {
        name: "an impossible silo".into(),
        kind: SiteKind::Farm,
        market: 0,
        stock,
        capacity,
        recipe: None,
        throughput: 1.0,
        powered: true,
        ran: 0.0,
        cost_factor: 1.0,
        fitted: None,
    };
    let mut w = Writer::new();
    site.store(&mut w);
    match Site::load(&mut Reader::new(&w.bytes)) {
        Err(SaveError::Impossible(_)) => {}
        other => panic!("a works holding more than its store was accepted: {other:?}"),
    }

    // And a recipe that does not exist in this build.
    let mut site = site;
    site.capacity = [10.0; N_COMMODITIES];
    site.recipe = Some(9_999);
    let mut w = Writer::new();
    site.store(&mut w);
    match Site::load(&mut Reader::new(&w.bytes)) {
        Err(SaveError::Impossible(_)) => {}
        other => panic!("a works with no such recipe was accepted: {other:?}"),
    }
}
