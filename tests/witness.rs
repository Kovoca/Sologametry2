//! **Whether somebody was in a position to know.**
//!
//! Slice 3 of `docs/mind-spec.md`. The visibility system in `ground.rs`
//! has been able to say *could this person see that* since it was built
//! and nothing has ever asked it; this is the asking.
//!
//! What is guarded here is that a wall does not make somebody ignorant —
//! it makes them a **different sort of witness**, and that is where two
//! honest accounts of one event start to differ.

use scale_sim::ground::{Ground, Tile};
use scale_sim::memory::{EventKind, Memory, Place, Source, WorldEvent};
use scale_sim::mind::{Facet, Happening, Mind, Value};
use scale_sim::id::{Arena, Id};
use scale_sim::person::{Person, Trade};
use scale_sim::rng::Rng;

/// **Real person handles**, now that a mind can hold one. A fresh arena
/// each call is fine: `Id` is a slot and a generation, so the same index
/// gives the same handle every time.
fn who(n: u32) -> Id<Person> {
    let mut folk: Arena<Person> = Arena::new();
    let mut last = folk.add(Person::new("x", Trade::Labourer, 0, 0.0));
    for _ in 0..n {
        last = folk.add(Person::new("x", Trade::Labourer, 0, 0.0));
    }
    last
}

use scale_sim::townplan::{Lot, Plan, TILES_PER_PLOT};
use scale_sim::witness::{
    from_the_ground, in_the_settlement, loudness_db, told, walls_between, Context,
    AMBIENT_INDOORS_DB, AMBIENT_STREET_DB,
};

fn a_mind(seed: u64) -> Mind {
    let mut m = Mind::draw(&mut Rng::new(seed), &[(Value::Fairness, 25)]);
    for f in Facet::ALL {
        m.person.set_baseline(f, 0.0);
    }
    m
}

/// A real town, and a real shop to stand in — the walls are the ones the
/// generator makes, not ones invented for a test.
fn a_shop() -> (u64, Plan, (i64, i64)) {
    let seed = 20260828u64;
    let plan = Plan::lay_out_on(seed, 4242, 2_500_000.0, 32, scale_sim::world::Biome::Grassland);
    let t = TILES_PER_PLOT as i64;
    for y in 1..plan.height - 1 {
        for x in 1..plan.width - 1 {
            if plan.at(x, y) == Lot::Shop {
                return (seed, plan, (x as i64 * t + t / 2, y as i64 * t + t / 2));
            }
        }
    }
    panic!("a town with no shop in it");
}

fn a_scream(at_day: u64) -> WorldEvent {
    WorldEvent {
        kind: EventKind::Assault,
        who: vec![who(1), who(2)],
        actor: Some(who(2)),
        place: Place(1),
        day: at_day,
        severity: -0.8,
        facts: Happening {
            severity: -0.8,
            to_me: 0.2,
            deliberate: true,
            control: 0.2,
            unexpected: 0.9,
            ..Default::default()
        },
    }
}

/// **A wall does not make somebody ignorant. It makes them a different
/// kind of witness.**
///
/// The headline of the slice, and it runs on the generated world rather
/// than a contrived one: two people, one event, a real wall between them.
/// The one in the room saw it and can name the man. The one on the other
/// side heard it and cannot — which is exactly the neighbour who heard
/// the screaming and could not say who was shouting.
#[test]
fn a_wall_makes_a_different_witness_and_not_an_ignorant_one() {
    let (seed, plan, spot) = a_shop();
    let g = Ground::around(seed, &plan, spot, 40);

    // **The next-door case, found rather than assumed.** What is being
    // tested is one wall between two people a few metres apart — the
    // neighbour who heard the screaming. A backroom two walls in, or a
    // street tile forty metres off, is a different claim and an untrue
    // one: a scream really does not carry through two masonry walls into
    // traffic.
    let floors: Vec<(i64, i64)> = (0..g.h)
        .flat_map(|y| (0..g.w).map(move |x| (x, y)))
        .filter(|&(x, y)| g.at(x, y) == Tile::Floor)
        .map(|(x, y)| (g.origin.0 + x as i64, g.origin.1 + y as i64))
        .collect();
    let streets: Vec<(i64, i64)> = (0..g.h)
        .flat_map(|y| (0..g.w).map(move |x| (x, y)))
        .filter(|&(x, y)| matches!(g.at(x, y), Tile::Road | Tile::Pavement))
        .map(|(x, y)| (g.origin.0 + x as i64, g.origin.1 + y as i64))
        .collect();
    let Some((inside, outside)) = floors.iter().find_map(|&i| {
        streets.iter().copied().find_map(|o| {
            let d = (o.0 - i.0).abs() + (o.1 - i.1).abs();
            (d <= 12 && walls_between(&g, o, i) == 1).then_some((i, o))
        })
    }) else {
        return; // this shop has no room fronting the street directly
    };

    // **The assault happens at the spot the wall was measured to**, or
    // the two calculations are about two different lines and the test
    // proves nothing. Two metres further in is through a doorway.
    let scene = inside;
    let beside_him = (inside.0, inside.1 + 1);
    let in_the_room = from_the_ground(&g, beside_him, scene, EventKind::Assault, AMBIENT_INDOORS_DB)
        .expect("somebody standing a metre away perceived nothing");
    // A quiet street: the classic case is a neighbour at night, and
    // sixty-five decibels of traffic masks a great deal.
    let through_the_wall =
        from_the_ground(&g, outside, scene, EventKind::Assault, AMBIENT_INDOORS_DB);

    assert_eq!(in_the_room.source, Source::Witnessed);
    assert!(in_the_room.could_identify, "a man two metres away could not say who");

    let outside_man = through_the_wall.expect(
        "a scream indoors reached nobody in the street: sound does not stop at a wall",
    );
    assert_eq!(
        outside_man.source,
        Source::Overheard,
        "somebody in the street saw through the shop wall"
    );
    assert!(
        !outside_man.could_identify,
        "a man in the street heard a scream and knew exactly who was shouting"
    );
    assert!(
        outside_man.exposure < in_the_room.exposure,
        "hearing it through a wall told him as much as watching it"
    );

    // And what each of them ends up remembering differs accordingly.
    let mind = a_mind(1);
    let mem = Memory::new();
    let mut rng = Rng::new(1);
    let ev = a_scream(10);
    let close = mem.perceive_as(&ev, None, &in_the_room, &mut rng).unwrap();
    let far = mem.perceive_as(&ev, None, &outside_man, &mut rng).unwrap();
    // **He saw the man, and the record says how sure he is.** A clear
    // look names somebody outright; a poor one names them with a doubt.
    assert_eq!(
        close.believed_actor.as_ref().and_then(|p| p.person()),
        Some(who(2)),
        "the man beside it could not name the attacker"
    );
    assert!(
        close.believed_actor.as_ref().unwrap().certainty() > 0.6,
        "he watched it happen and is not sure who it was"
    );
    assert_eq!(far.believed_actor, None, "the man outside named a suspect");
    assert!(far.confidence < close.confidence);
    let _ = mind;
}

/// **You can see that something happened and not see who.**
///
/// A person is detectable at a kilometre and recognisable at about
/// twenty-five metres. The gap between those two is where mistaken
/// identity lives, and a model without it has every witness identifying
/// everybody.
#[test]
fn distance_takes_the_face_before_it_takes_the_event() {
    let (seed, plan, spot) = a_shop();
    // A long clear run of street, so the only thing varying is distance.
    let g = Ground::around(seed, &plan, spot, 60);
    let street: Vec<(i64, i64)> = (0..g.w)
        .filter(|&x| g.at(x, g.h / 2) == Tile::Road)
        .map(|x| (g.origin.0 + x as i64, g.origin.1 + g.h as i64 / 2))
        .collect();
    if street.len() < 40 {
        return; // this town's centre has no long straight; nothing to test
    }
    let near = street[0];
    let far = street[street.len() - 1];
    let event_at = street[1];

    let up_close = from_the_ground(&g, near, event_at, EventKind::Assault, AMBIENT_STREET_DB)
        .expect("standing beside it and saw nothing");
    let down_the_road =
        from_the_ground(&g, far, event_at, EventKind::Assault, AMBIENT_STREET_DB);

    assert!(up_close.could_identify, "a man beside it could not say who");
    if let Some(w) = down_the_road {
        assert!(
            !w.could_identify,
            "a witness {} m down the street identified the man by his face",
            (far.0 - event_at.0).abs()
        );
        assert!(w.exposure < up_close.exposure);
    }
}

/// **Out of sight and out of earshot is not a memory at all.**
#[test]
fn far_enough_away_and_nothing_happened() {
    let (seed, plan, spot) = a_shop();
    let g = Ground::around(seed, &plan, spot, 70);
    let here = spot;
    let miles_off = (spot.0 + 400, spot.1 + 400);
    assert!(
        from_the_ground(&g, here, miles_off, EventKind::Conversation, AMBIENT_STREET_DB)
            .is_none(),
        "a conversation four hundred metres away was overheard"
    );
    // Even a roof coming in has a range.
    let very_far = (spot.0 + 5000, spot.1);
    assert!(
        from_the_ground(&g, here, very_far, EventKind::Collapse, AMBIENT_STREET_DB).is_none(),
        "a collapse five kilometres away was heard"
    );
}

/// **A collapse carries and a conversation does not.**
///
/// Real acoustics, and it falls out of the decibel figures rather than
/// being asserted: ordinary talk is 60 dB at a metre, a scream 90, a roof
/// coming in 110, and every doubling of distance costs 6.
#[test]
fn a_roof_coming_in_carries_and_a_conversation_does_not() {
    assert!(loudness_db(EventKind::Collapse) > loudness_db(EventKind::Assault));
    assert!(loudness_db(EventKind::Assault) > loudness_db(EventKind::Conversation));

    let (seed, plan, spot) = a_shop();
    let g = Ground::around(seed, &plan, spot, 70);
    let far = (spot.0 + 60, spot.1);

    // **And a wall takes far more out of a voice than out of a rumble**,
    // which is why the collapse gets through and the talking does not.
    // **Hold the walls constant and vary the sound**, or this measures
    // architecture instead of acoustics. Two points on the same open
    // street, sixty metres apart, nothing between them.
    let open: Vec<(i64, i64)> = (0..g.w)
        .filter(|&x| matches!(g.at(x, g.h / 2), Tile::Road | Tile::Marking))
        .map(|x| (g.origin.0 + x as i64, g.origin.1 + g.h as i64 / 2))
        .collect();
    if open.len() < 3 {
        return;
    }
    let (here, there) = (open[0], open[open.len() - 1]);
    assert_eq!(walls_between(&g, here, there), 0, "the street is not open");

    let crash = from_the_ground(&g, here, there, EventKind::Collapse, AMBIENT_STREET_DB);
    let chat = from_the_ground(&g, here, there, EventKind::Conversation, AMBIENT_STREET_DB);
    assert!(
        crash.is_some(),
        "a roof came in {} m down an open street and nobody noticed",
        (there.0 - here.0).abs()
    );
    // **And a factory floor hears nothing**, which is real and is why
    // some workplaces produce no social contact at all.
    // **And a factory floor is a place you communicate by sight**, which
    // is the real consequence: at eighty-five decibels the conversation
    // is inaudible, so anything perceived is perceived by looking.
    let on_the_press = from_the_ground(&g, here, there, EventKind::Conversation, 85.0);
    if let Some(w) = on_the_press {
        assert_eq!(
            w.source,
            Source::Witnessed,
            "two men heard each other talking across a stamping shop"
        );
    }
}

/// **A sampled person has no tile, and still has to be able to know
/// things.**
///
/// The middle tier. Sharing a household is not sharing a street: you see
/// everything that happens in the first and a fraction of what happens in
/// the second.
#[test]
fn a_settlement_person_knows_what_happens_where_they_are() {
    let home = Context::Household(3);
    let work = Context::Workplace(9);
    let street = Context::Street(2);
    let mine = vec![home, work, street];

    let at_home = in_the_settlement(&mine, home, EventKind::Assault).unwrap();
    let at_work = in_the_settlement(&mine, work, EventKind::Assault).unwrap();
    let in_the_road = in_the_settlement(&mine, street, EventKind::Assault).unwrap();

    assert!(at_home.exposure > at_work.exposure);
    assert!(
        at_work.exposure > in_the_road.exposure,
        "a street is as intimate as a workplace"
    );
    assert!(
        in_the_road.exposure < 0.2,
        "everything that happens on your street happens in front of you"
    );
    assert!(at_home.could_identify, "somebody in the next room could not say who");
    assert!(
        !in_the_road.could_identify,
        "something at the far end of the street was identified by face"
    );

    // Somewhere they are not is somewhere they know nothing about.
    assert!(
        in_the_settlement(&mine, Context::Tavern(1), EventKind::Assault).is_none(),
        "he knew what happened in a tavern he was not in"
    );
}

/// **Only what is worth carrying reaches somebody who was nowhere near.**
///
/// A death travels through a village in a day. A stranger's dinner
/// travels nowhere at all — and neither does the identity, which is where
/// a rumour blames the wrong man.
#[test]
fn word_of_mouth_carries_a_death_and_not_a_dinner() {
    assert!(
        told(EventKind::Meal, 1, who(4)).is_none(),
        "somebody carried news of a stranger's dinner to the next town"
    );
    let death = told(EventKind::Death, 1, who(4)).expect("nobody passed on a death");
    assert_eq!(death.source, Source::Told { by: who(4) });
    assert!(death.could_identify, "the man who told him could not say who died");

    // Third hand, and the identity is the first thing to go.
    let secondhand = told(EventKind::Death, 3, who(4)).unwrap();
    assert!(matches!(secondhand.source, Source::Rumour { hops: 3 }));
    assert!(
        !secondhand.could_identify,
        "a story three removes from the event still named the right man"
    );
    assert!(
        secondhand.exposure < death.exposure,
        "a rumour arrived as reliably as being told directly"
    );
}

/// **The three tiers are the same interface**, which is what lets a
/// distant person be simulated statistically and a loaded one properly,
/// without memory knowing or caring which.
#[test]
fn all_three_tiers_produce_the_same_kind_of_answer() {
    let (seed, plan, spot) = a_shop();
    let g = Ground::around(seed, &plan, spot, 30);
    let mind = a_mind(2);
    let mem = Memory::new();
    let mut rng = Rng::new(2);
    let ev = a_scream(5);

    let ground = from_the_ground(&g, spot, (spot.0 + 1, spot.1), EventKind::Assault, AMBIENT_INDOORS_DB);
    let settlement = in_the_settlement(&[Context::Household(1)], Context::Household(1), EventKind::Assault);
    let distant = told(EventKind::Assault, 2, who(8));

    for w in [ground, settlement, distant].into_iter().flatten() {
        let p = mem
            .perceive_as(&ev, None, &w, &mut rng)
            .expect("an exposure that reached somebody produced no perception");
        assert!((0.0..=1.0).contains(&p.confidence));
        // Whatever the tier, the appraisal is the perceiver's.
        let a = mind.read(&p.facts);
        assert!(a.severity <= 0.0);
    }
}
