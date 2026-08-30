//! The bottom of the ladder: ground at a metre to the tile.
//!
//! Everything above this was furniture for a place nobody could be in.

use scale_sim::building::Fixture;
use scale_sim::ground::{Ground, Tile};
use scale_sim::townplan::{Lot, Plan, StreetClass, TILES_PER_PLOT};
use scale_sim::vehicle::Vehicle;
use scale_sim::world::Biome;

fn a_town() -> Plan {
    Plan::lay_out_on(20260828, 4242, 400_000.0, 32, Biome::Grassland)
}

fn a_city() -> Plan {
    // Big enough to have a trunk route driven through it.
    Plan::lay_out_on(20260828, 4242, 2_500_000.0, 32, Biome::Grassland)
}

/// **Walk across a street and count what is under your feet.** Returns the
/// running surface and the footway, in metres, for a plain stretch of the
/// given class — not a junction, which legitimately carries both roads.
fn walk_across(plan: &Plan, class: StreetClass) -> (usize, usize) {
    let t = TILES_PER_PLOT as i64;
    for y in 1..plan.height - 1 {
        for x in 1..plan.width - 1 {
            if plan.at(x, y) != Lot::Street || plan.street_class(x, y) != Some(class) {
                continue;
            }
            let ns = plan.at(x, y - 1) == Lot::Street || plan.at(x, y + 1) == Lot::Street;
            let ew = plan.at(x - 1, y) == Lot::Street || plan.at(x + 1, y) == Lot::Street;
            if ns == ew {
                continue; // a crossroads, or a stub going nowhere
            }
            let at = (x as i64 * t + t / 2, y as i64 * t + t / 2);
            let g = Ground::around(1, plan, at, 48);
            let (mut surface, mut foot) = (0, 0);
            for k in -20..=20i64 {
                let (gx, gy) = if ns { (at.0 + k, at.1) } else { (at.0, at.1 + k) };
                let (vx, vy) = (gx - g.origin.0, gy - g.origin.1);
                if vx < 0 || vy < 0 || vx as usize >= g.w || vy as usize >= g.h {
                    continue;
                }
                match g.at(vx as usize, vy as usize) {
                    Tile::Road | Tile::Marking | Tile::Shoulder => surface += 1,
                    Tile::Pavement => foot += 1,
                    _ => {}
                }
            }
            return (surface, foot);
        }
    }
    panic!("no plain stretch of {class:?} in this town");
}

fn stand_on(plan: &Plan, want: Lot) -> (i64, i64) {
    let t = TILES_PER_PLOT as i64;
    for y in 0..plan.height {
        for x in 0..plan.width {
            if plan.at(x, y) == want {
                return (x as i64 * t + t / 2, y as i64 * t + t / 2);
            }
        }
    }
    panic!("no {want:?} in this town");
}

#[test]
fn the_same_ground_every_time() {
    // Spec A1.5 and rule R5: generated on demand, discarded on leaving,
    // identical on return. This is what keeps a save bounded — without it
    // every field you ever crossed accumulates state.
    let plan = a_town();
    let at = stand_on(&plan, Lot::Street);
    let a = Ground::around(1, &plan, at, 48);
    let b = Ground::around(1, &plan, at, 48);
    assert_eq!(a.render(None), b.render(None));
}

#[test]
fn a_street_is_not_thirty_two_metres_of_tarmac() {
    // **A residential carriageway is five or six metres** with pavements
    // either side; the rest of the plot is verge and frontage. Paving the
    // whole plot gave every lane the footprint of a dual carriageway.
    let plan = a_town();
    // **Not a crossroads.** One of those legitimately carries both roads
    // and is more than half made surface; a plain lane is not.
    let t = TILES_PER_PLOT as i64;
    let mut at = stand_on(&plan, Lot::Street);
    'find: for y in 1..plan.height - 1 {
        for x in 1..plan.width - 1 {
            if plan.at(x, y) != Lot::Street {
                continue;
            }
            if plan.street_class(x, y) != Some(StreetClass::Lane) {
                continue;
            }
            let ns = plan.at(x, y - 1) == Lot::Street || plan.at(x, y + 1) == Lot::Street;
            let ew = plan.at(x - 1, y) == Lot::Street || plan.at(x + 1, y) == Lot::Street;
            if ns != ew {
                at = (x as i64 * t + t / 2, y as i64 * t + t / 2);
                break 'find;
            }
        }
    }
    let g = Ground::around(1, &plan, at, 64);

    let road = g.tiles.iter().filter(|&&t| t == Tile::Road).count();
    let paved = g.tiles.iter().filter(|&&t| t == Tile::Pavement).count();
    assert!(road > 0 && paved > 0, "a street with no carriageway or no path");

    // **Measured over the plot, not along a line.** Scanning across an
    // east-west street finds road the whole way, quite correctly — what
    // must be small is the share of the *ground* that is paved.
    let t = TILES_PER_PLOT as i64;
    let (px, py) = (at.0.div_euclid(t), at.1.div_euclid(t));
    let mut made = 0;
    let mut total = 0;
    for iy in 0..t {
        for ix in 0..t {
            let gx = px * t + ix;
            let gy = py * t + iy;
            let (vx, vy) = (gx - g.origin.0, gy - g.origin.1);
            if vx < 0 || vy < 0 || vx as usize >= g.w || vy as usize >= g.h {
                continue;
            }
            total += 1;
            if matches!(g.at(vx as usize, vy as usize), Tile::Road | Tile::Pavement) {
                made += 1;
            }
        }
    }
    let share = made as f64 / total.max(1) as f64;
    assert!(
        share < 0.42,
        "{:.0}% of a street plot is made surface — that is not a lane, it is          a runway",
        share * 100.0
    );
}

#[test]
fn a_shop_has_walls_a_door_tills_and_aisles() {
    // The fixture list from `building.rs` given somewhere to stand: tills
    // across the front by the door because that is where you pay on the
    // way out, aisles through the middle, racking at the back where the
    // lorries come.
    let plan = a_town();
    let at = stand_on(&plan, Lot::Shop);
    let g = Ground::around(1, &plan, at, 72);

    let count = |t: Tile| g.tiles.iter().filter(|&&x| x == t).count();
    assert!(count(Tile::Wall) > 20, "a shop with no walls");
    assert!(count(Tile::Door) > 0, "a shop nobody can get into");
    assert!(
        count(Tile::Fitting(Fixture::Till)) > 0,
        "a shop with no checkouts"
    );
    assert!(
        count(Tile::Fitting(Fixture::Shelving)) > 20,
        "a shop with nothing on sale"
    );

    // The tills are at the front, nearer the door than the racking is.
    let mean_y = |t: Tile| {
        let ys: Vec<usize> = (0..g.h)
            .flat_map(|y| (0..g.w).map(move |x| (x, y)))
            .filter(|&(x, y)| g.at(x, y) == t)
            .map(|(_, y)| y)
            .collect();
        ys.iter().sum::<usize>() as f64 / ys.len().max(1) as f64
    };
    assert!(
        mean_y(Tile::Fitting(Fixture::Till))
            < mean_y(Tile::Fitting(Fixture::StockRack)),
        "the checkouts are behind the stockroom"
    );
}

#[test]
fn you_can_stand_on_a_lorry_but_not_in_a_shelf() {
    // **A vehicle is ground, not a mode.** You stand on the seat to drive
    // and on the bed to load, which is the whole reason parts are tiles.
    // Furniture mostly is not: you stand at a till and in front of
    // shelving, never inside a shelf bay.
    let plan = a_town();
    let at = stand_on(&plan, Lot::Street);
    let mut g = Ground::around(1, &plan, at, 64);
    let artic = Vehicle::artic();
    g.park(&artic, (at.0 - 8, at.1));

    let on_rig = g.tiles.iter().filter(|t| matches!(t, Tile::Vehicle(_))).count();
    let (l, w) = artic.footprint();
    assert!(
        on_rig > (l * w / 2) as usize,
        "an artic of {l} by {w} put only {on_rig} tiles on the road"
    );
    assert!(
        g.tiles
            .iter()
            .filter(|t| matches!(t, Tile::Vehicle(_)))
            .all(|t| t.walkable()),
        "a lorry you cannot climb onto"
    );

    assert!(!Tile::Wall.walkable());
    assert!(!Tile::Fitting(Fixture::Shelving).walkable());
    assert!(Tile::Fitting(Fixture::Till).walkable());
}

#[test]
fn a_road_is_as_big_as_what_uses_it() {
    // **The whole point.** A town whose streets are all one width is a
    // housing estate drawn by somebody who has never seen one. By length
    // the UK is about 1% motorway, 12% A-road and 87% minor, and the
    // widths are not close: a residential lane is 5 m of shared surface, a
    // motorway is 33 m of corridor.
    let town = a_town();
    let (lane, lane_foot) = walk_across(&town, StreetClass::Lane);
    let (road, road_foot) = walk_across(&town, StreetClass::Road);
    let (dual, _) = walk_across(&town, StreetClass::Dual);
    let (motorway, motorway_foot) = walk_across(&a_city(), StreetClass::Motorway);

    assert!(
        lane < road && road < dual && dual < motorway,
        "not a hierarchy: lane {lane} m, road {road} m, dual {dual} m, motorway {motorway} m"
    );

    // **Every one of them is two-way.** The narrowest carriageway anybody
    // builds is two lanes of about 2.5 m; a lane that could not fit two
    // cars passing would be a driveway.
    assert!(lane >= 5, "a {lane} m lane is single-track");
    // Anything above a lane has to let two lorries meet: an artic is
    // 2.55 m wide, so 7 m of two 3.65 m lanes is the standard.
    for (name, w) in [("road", road), ("dual", dual), ("motorway", motorway)] {
        assert!(w >= 7, "two artics cannot pass on {w} m of {name}");
    }

    // Footways exist on streets people live on...
    assert!(lane_foot >= 2 && road_foot >= 2, "a street with nowhere to walk");
    // ...and not on a motorway. That is what severance means: the town
    // either side of it is joined by bridges or not at all.
    assert_eq!(
        motorway_foot, 0,
        "a footway down the side of a motorway"
    );
}

#[test]
fn the_lines_down_a_road_are_dashed() {
    // A solid line means something specific — do not cross — so the edge
    // of a carriageway is solid and the line between lanes is not. Painting
    // every line solid turns a road into a set of rails.
    let plan = a_town();
    let t = TILES_PER_PLOT as i64;
    let mut found = false;
    'find: for y in 1..plan.height - 1 {
        for x in 1..plan.width - 1 {
            if plan.at(x, y) != Lot::Street
                || plan.street_class(x, y) != Some(StreetClass::Road)
            {
                continue;
            }
            let ns = plan.at(x, y - 1) == Lot::Street || plan.at(x, y + 1) == Lot::Street;
            let ew = plan.at(x - 1, y) == Lot::Street || plan.at(x + 1, y) == Lot::Street;
            if ns == ew {
                continue;
            }
            let at = (x as i64 * t + t / 2, y as i64 * t + t / 2);
            let g = Ground::around(1, &plan, at, 40);
            // Walk *along* the centre line and count mark against gap.
            let (mut mark, mut gap) = (0, 0);
            for k in -14..=14i64 {
                let (gx, gy) = if ns { (at.0, at.1 + k) } else { (at.0 + k, at.1) };
                let (vx, vy) = (gx - g.origin.0, gy - g.origin.1);
                if vx < 0 || vy < 0 || vx as usize >= g.w || vy as usize >= g.h {
                    continue;
                }
                match g.at(vx as usize, vy as usize) {
                    Tile::Marking => mark += 1,
                    Tile::Road => gap += 1,
                    _ => {}
                }
            }
            assert!(
                mark > 0 && gap > mark,
                "a centre line of {mark} m painted and {gap} m clear is not a dashed line"
            );
            found = true;
            break 'find;
        }
    }
    assert!(found, "no two-way road in this town to look at");
}
