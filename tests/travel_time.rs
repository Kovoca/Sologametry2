//! **How long a haul takes, and what decides it.**
//!
//! `Route::surface`'s own doc comment has always said the surface is "the
//! number that decides both what can travel **and how fast**". Only the
//! first half was true: a haul's *cost* came from a Dijkstra priced by the
//! road class under every step of the path, while its *time* was
//! `kilometres / 620` and nothing else. Six hundred kilometres of track
//! arrived the same day as six hundred of motorway.
//!
//! These gates hold the distance still and vary the road.

use scale_sim::econ::{Commodity, Doctrine, Surface};
use scale_sim::network::Network;
use scale_sim::polity::Polities;
use scale_sim::quote::{RouteId, Routing};
use scale_sim::region::Region;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

/// Two towns, one road, and nothing different about them but the surface.
fn two_towns(km: f64, surface: Surface) -> Routing {
    Routing::build(
        2,
        &[(
            RouteId(1),
            0,
            1,
            // Freight cost held constant on purpose: this gate is about
            // time, and letting the cost move too would let a cheaper path
            // explain the result.
            1.0,
            km,
            1e9,
            surface.pace(),
        )],
    )
}

/// **The same distance over worse ground takes longer.**
///
/// Sabotage: make `Surface::pace` answer 1.0 for everything and the three
/// readings collapse to one.
#[test]
fn the_road_under_a_haul_decides_how_long_it_takes() {
    let km = 600.0;
    let highway = two_towns(km, Surface::Highway).travel_days(0, 1);
    let road = two_towns(km, Surface::Road).travel_days(0, 1);
    let track = two_towns(km, Surface::Track).travel_days(0, 1);

    // A motorway beats an ordinary road beats a track, strictly.
    assert!(
        highway < road && road < track,
        "600 km: highway {highway:.2} d, road {road:.2} d, track {track:.2} d \
         — the surface is not reaching the answer"
    );

    // And the size of it is the real figure rather than merely an ordering:
    // a track is 0.30 of a made road, so it is between three and four
    // times as long a journey.
    let ratio = track / road;
    assert!(
        (3.0..=3.7).contains(&ratio),
        "a track should take about 3.3x a made road, took {ratio:.2}x"
    );

    // The distance has not moved. If it had, this gate would be measuring
    // a different route rather than the same one over worse ground.
    for s in [Surface::Highway, Surface::Road, Surface::Track] {
        assert!(
            (two_towns(km, s).km(0, 1) - km).abs() < 1e-9,
            "the distance changed with the surface"
        );
    }
}

/// **Time accumulates leg by leg, at each leg's own pace.**
///
/// This is what separates the model from the obvious wrong one. Taking the
/// *worst* surface anywhere on the path and applying it to the whole
/// distance is a defensible-sounding rule and is badly wrong: a journey of
/// four hundred kilometres of motorway and forty of track is not four
/// hundred and forty kilometres of track.
///
/// Sabotage: accumulate `km` and divide once by the minimum pace on the
/// path, and the mixed route comes out at the all-track figure.
#[test]
fn one_bad_stretch_costs_that_stretch_and_no_more() {
    // a --400 km highway--> b --40 km track--> c
    let mixed = Routing::build(
        3,
        &[
            (RouteId(1), 0, 1, 1.0, 400.0, 1e9, Surface::Highway.pace()),
            (RouteId(2), 1, 2, 1.0, 40.0, 1e9, Surface::Track.pace()),
        ],
    );
    let got = mixed.travel_days(0, 2);

    // What it should be, from the two legs.
    let want = 400.0 / (620.0 * Surface::Highway.pace()) + 40.0 / (620.0 * Surface::Track.pace());
    assert!(
        (got - want).abs() < 1e-9,
        "expected {want:.4} days from the two legs, got {got:.4}"
    );

    // The wrong rule, named so the gate says what it is ruling out: 440 km
    // all at a track's pace.
    let worst_surface_everywhere = 440.0 / (620.0 * Surface::Track.pace());
    assert!(
        got < worst_surface_everywhere * 0.6,
        "{got:.2} days is close to the all-track figure of \
         {worst_surface_everywhere:.2} — the bad stretch is being applied \
         to the whole journey"
    );

    // **And the bad stretch costs far more than its share of the
    // distance**, which is the claim worth asserting and is what makes one
    // unmade mile matter. Forty kilometres is 9% of the journey's length
    // and should be about a quarter of its time.
    //
    // The first version of this gate asserted the mixed route took half as
    // long again as an all-motorway one, which is arithmetically
    // impossible — 9% of the distance cannot add 50% to the trip — and the
    // gate correctly went red on my own bad bar rather than on the model.
    let all_highway = 440.0 / (620.0 * Surface::Highway.pace());
    assert!(
        got > all_highway,
        "{got:.4} days against {all_highway:.4} all-motorway — the track is \
         costing nothing at all"
    );

    let track_share_of_distance = 40.0 / 440.0;
    let track_share_of_time = (40.0 / (620.0 * Surface::Track.pace())) / got;
    assert!(
        track_share_of_time > track_share_of_distance * 2.5,
        "the track is {:.1}% of the distance and only {:.1}% of the time — \
         a bad stretch is supposed to cost more than its length",
        track_share_of_distance * 100.0,
        track_share_of_time * 100.0
    );
}

/// **A journey nobody can make is not a slow journey.**
///
/// Distinct from the above and easy to conflate: an unreachable pair has
/// no travel time at all, and answering a finite number would let a caller
/// quote a haul that cannot happen.
#[test]
fn a_town_nothing_reaches_has_no_travel_time() {
    let islanded = Routing::build(
        3,
        &[(RouteId(1), 0, 1, 1.0, 100.0, 1e9, Surface::Road.pace())],
    );
    assert!(islanded.travel_days(0, 1).is_finite());
    assert!(
        !islanded.travel_days(0, 2).is_finite(),
        "a market off the network answered a finite travel time"
    );
    assert_eq!(islanded.travel_days(0, 0), 0.0, "nowhere takes no time");
}

/// **And it reaches a real country**, where the roads are whatever the
/// ground made them.
///
/// The unit gates above prove the arithmetic; this proves it is wired to
/// the world. On a generated planet some links are made road and some are
/// open country — the fallback where the network never reached, which this
/// project already prices at open-country rates and which is why such a
/// place stays poor. It should be slow as well as dear.
#[test]
fn a_haul_over_open_country_is_slow_as_well_as_dear() {
    let world = World::generate(384, 216, 7);
    let pol = Polities::partition(&world, 24);
    let set = Settlements::place(&world, &pol, 3000);
    let net = Network::build(&world, &set, 500);
    let id = pol.ranked()[0].0;
    let e = Region::extract(&world, &pol, &set, &net, id, 5, Doctrine::Prudent)
        .expect("a nation")
        .economy;

    // Per kilometre, so the comparison is about the road and not about how
    // far apart two particular towns happen to be.
    let mut made = Vec::new();
    let mut rough = Vec::new();
    for r in e.routes.iter().filter(|r| r.usable() && r.km > 1.0) {
        let per_km = r.km / (620.0 * r.surface.pace()) / r.km;
        match r.surface {
            Surface::Highway | Surface::Road => made.push(per_km),
            Surface::Track | Surface::Open => rough.push(per_km),
            Surface::Water => {}
        }
    }
    assert!(!made.is_empty(), "this nation has no made roads at all");

    // Where the nation has both, the rough ground must be slower. Where it
    // has only made road the claim is not testable here and is asserted by
    // the unit gates above rather than faked.
    if let (Some(&worst_made), Some(&best_rough)) = (
        made.iter().max_by(|a, b| a.total_cmp(b)),
        rough.iter().min_by(|a, b| a.total_cmp(b)),
    ) {
        assert!(
            best_rough > worst_made,
            "a day per km: rough ground {best_rough:.5} against made road \
             {worst_made:.5} — the surface is not reaching the route table"
        );
    }

    // And the whole point of the change: somewhere in this country a haul
    // takes longer than its distance alone would say.
    let mut differs = 0;
    for a in 0..e.markets.len() {
        for b in 0..e.markets.len() {
            if a == b {
                continue;
            }
            let km = e.routing.km(a, b);
            if !km.is_finite() {
                continue;
            }
            let flat = scale_sim::shipment::days_on_the_road(km);
            let real = e.routing.travel_days(a, b).floor() as u64;
            if real != flat {
                differs += 1;
            }
        }
    }
    assert!(
        differs > 0,
        "every haul in this country takes exactly what distance alone said, \
         so the route table is still dividing kilometres by one constant"
    );
}

/// **A perishable now rots on the slow road**, which is the consequence
/// that reaches the rest of the model.
///
/// `Quote::loss` is spoilage over the days the load is actually
/// travelling. With time read off distance alone, a cargo of food over
/// rough ground lost exactly what the same distance of motorway lost.
#[test]
fn what_rots_on_the_way_follows_the_road_and_not_only_the_distance() {
    let km = 900.0;
    let by_road = two_towns(km, Surface::Road)
        .quote(0, 1, Commodity::ProcessedFood, 0.0)
        .expect("a quote");
    let by_track = two_towns(km, Surface::Track)
        .quote(0, 1, Commodity::ProcessedFood, 0.0)
        .expect("a quote");

    assert!(
        by_track.days > by_road.days,
        "same 900 km: {} nights by road against {} by track",
        by_road.days,
        by_track.days
    );
    assert!(
        by_track.loss > by_road.loss,
        "a perishable lost {:.4} over a track and {:.4} over a made road \
         across the same distance",
        by_track.loss,
        by_road.loss
    );
}
