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

/// Small enough that its streets have houses on them rather than an
/// unbroken wall of flats, which is what the footway figures need.
fn a_small_town() -> Plan {
    Plan::lay_out_on(20260828, 4242, 18_000.0, 32, Biome::Grassland)
}

fn a_city() -> Plan {
    // Big enough to have a trunk route driven through it.
    Plan::lay_out_on(20260828, 4242, 2_500_000.0, 32, Biome::Grassland)
}

/// **Walk across a street and count what is under your feet.** Returns the
/// running surface and the footway, in metres, for a plain stretch of the
/// given class — not a junction, which legitimately carries both roads.
fn walk_across(plan: &Plan, class: StreetClass, plain_frontage: bool) -> (usize, usize) {
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
            // Away from dense frontage — where the footway runs to the
            // building line and the cross-section is not the whole story —
            // but only when the footway is what is being measured. A dual
            // or a motorway exists only in a city, where every street has
            // frontage, so insisting on it there finds nothing at all.
            if plain_frontage
                && [(0i64, -1i64), (0, 1), (-1, 0), (1, 0)]
                    .iter()
                    .any(|&(dx, dy)| {
                        let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                        nx >= 0
                            && ny >= 0
                            && (nx as usize) < plan.width
                            && (ny as usize) < plan.height
                            && matches!(plan.at(nx as usize, ny as usize), Lot::Flats | Lot::Shop)
                    })
            {
                continue;
            }
            let at = (x as i64 * t + t / 2, y as i64 * t + t / 2);
            // Wide enough to hold the whole plot: a motorway corridor is
            // 33 m, and a window that clipped it made this test pass by
            // never seeing the hard shoulders.
            let g = Ground::around(1, plan, at, TILES_PER_PLOT);
            let (mut surface, mut foot) = (0, 0);
            for k in -(TILES_PER_PLOT as i64 / 2)..TILES_PER_PLOT as i64 / 2 {
                let (gx, gy) = if ns {
                    (at.0 + k, at.1)
                } else {
                    (at.0, at.1 + k)
                };
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
    let plan = a_small_town();
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
            // **A street of houses**, which has verges and front gardens.
            // A city-centre street is legitimately paved kerb to building
            // line and would fail this quite correctly.
            if [(0i64, -1i64), (0, 1), (-1, 0), (1, 0), (-1, -1), (1, 1)]
                .iter()
                .any(|&(dx, dy)| {
                    let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                    nx >= 0
                        && ny >= 0
                        && (nx as usize) < plan.width
                        && (ny as usize) < plan.height
                        && matches!(plan.at(nx as usize, ny as usize), Lot::Flats | Lot::Shop)
                })
            {
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
    assert!(
        road > 0 && paved > 0,
        "a street with no carriageway or no path"
    );

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
        mean_y(Tile::Fitting(Fixture::Till)) < mean_y(Tile::Fitting(Fixture::StockRack)),
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

    // **The lorry stands on the road; it does not replace it.** Parking
    // used to overwrite the terrain, so a street with a vehicle on it had
    // no surface left underneath and driving away would leave a hole.
    let on_rig = g.over.iter().filter(|p| p.is_some()).count();
    assert!(
        g.tiles
            .iter()
            .filter(|t| matches!(t, Tile::Road | Tile::Marking))
            .count()
            > 0,
        "the road under the lorry has been deleted"
    );
    let (l, w) = artic.footprint();
    assert!(
        on_rig > (l * w / 2) as usize,
        "an artic of {l} by {w} put only {on_rig} tiles on the road"
    );
    assert!(
        g.over
            .iter()
            .flatten()
            .all(|&p| Tile::Vehicle(p).walkable()),
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
    // **Where each class is measured matters.** In a city of 400,000
    // every street has flats or shops on it, so every stretch is paved
    // kerb to building line and the footway figure is not the
    // cross-section's — the small town is where a footway is a footway.
    // Duals and motorways only exist somewhere big.
    let town = a_small_town();
    let city = a_city();
    let (lane, lane_foot) = walk_across(&town, StreetClass::Lane, true);
    let (road, road_foot) = walk_across(&town, StreetClass::Road, true);
    let (dual, _) = walk_across(&city, StreetClass::Dual, false);
    let (motorway, motorway_foot) = walk_across(&city, StreetClass::Motorway, false);

    assert!(
        lane < road && road < dual && dual < motorway,
        "not a hierarchy: lane {lane} m, road {road} m, dual {dual} m, motorway {motorway} m"
    );

    // **The exact cross-sections, in metres, because one tile is a metre.**
    // Pinned rather than merely ordered: these are real figures, and
    // changing one should cost an argument.
    assert_eq!(
        (lane, lane_foot),
        (5, 4),
        "a lane is 5 m shared and 2 m of path each side"
    );
    assert_eq!(
        (road, road_foot),
        (7, 6),
        "a road is 7.3 m of two lanes and 2.5 m of path"
    );
    assert_eq!(
        dual, 16,
        "a dual is two carriageways of 7.3 m either side of a reserve"
    );
    assert_eq!(
        motorway, 29,
        "a motorway is 11 m of three lanes and 3.3 m of shoulder, twice"
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
    assert!(
        lane_foot >= 2 && road_foot >= 2,
        "a street with nowhere to walk"
    );
    // ...and not on a motorway. That is what severance means: the town
    // either side of it is joined by bridges or not at all.
    assert_eq!(motorway_foot, 0, "a footway down the side of a motorway");

    // And the corridor fills the plot. **That is what severance is**: the
    // 32 m of town this route runs through is entirely road, so the halves
    // either side of it are joined by bridges or not at all.
    assert_eq!(
        motorway + motorway_foot + 3, // + the central reserve
        TILES_PER_PLOT,
        "a motorway that leaves room either side of it is not a motorway"
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
            if plan.at(x, y) != Lot::Street || plan.street_class(x, y) != Some(StreetClass::Road) {
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
                let (gx, gy) = if ns {
                    (at.0, at.1 + k)
                } else {
                    (at.0 + k, at.1)
                };
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

/// What share of a plot is actually built on.
fn site_coverage(plan: &Plan, px: usize, py: usize) -> f64 {
    let t = TILES_PER_PLOT as i64;
    let at = (px as i64 * t + t / 2, py as i64 * t + t / 2);
    let g = Ground::around(1, plan, at, TILES_PER_PLOT);
    let (mut built, mut total) = (0, 0);
    for iy in 0..t {
        for ix in 0..t {
            let (gx, gy) = (px as i64 * t + ix, py as i64 * t + iy);
            let (vx, vy) = (gx - g.origin.0, gy - g.origin.1);
            if vx < 0 || vy < 0 || vx as usize >= g.w || vy as usize >= g.h {
                continue;
            }
            total += 1;
            // **Site coverage is footprint over plot**, so what counts is
            // the building and not the ground around it. A commercial
            // plot's spare land is a service yard — hardstanding, bins,
            // somewhere to turn a van — which is made ground and is still
            // not a building. Counting it put a block of flats at 98% of
            // its plot, which is not a city centre, it is a monolith.
            if !matches!(
                g.at(vx as usize, vy as usize),
                Tile::Grass
                    | Tile::Scrub
                    | Tile::Sand
                    | Tile::Rock
                    | Tile::Snow
                    | Tile::Water
                    | Tile::Tree
                    | Tile::Parking
            ) {
                built += 1;
            }
        }
    }
    built as f64 / total.max(1) as f64
}

fn find_lot(plan: &Plan, want: Lot, from_centre: bool) -> (usize, usize) {
    let c = (plan.width as i64 / 2, plan.height as i64 / 2);
    let mut best: Option<((usize, usize), i64)> = None;
    for y in 0..plan.height {
        for x in 0..plan.width {
            if plan.at(x, y) != want {
                continue;
            }
            let d = (x as i64 - c.0).abs().max(y as i64 - c.1);
            let score = if from_centre { -d } else { d };
            if best.is_none_or(|(_, b)| score > b) {
                best = Some(((x, y), score));
            }
        }
    }
    best.map(|(p, _)| p)
        .unwrap_or_else(|| panic!("no {want:?} in this town"))
}

#[test]
fn a_city_centre_is_a_street_wall_and_a_suburb_is_not() {
    // **Urban density is a shape, not a number.** Clark's law was already
    // in the plan — flats in the middle, houses outward — and nothing at
    // the tile layer read it, so a city of forty-six million had grass and
    // trees between every building. What a centre actually has is a street
    // wall: buildings on the back of the footway, sharing party walls.
    //
    // Real site coverage *(footprint over plot)*: a dense urban core is
    // 60-80%, inner terraces 40-50%, detached suburbs 15-25%.
    let plan = a_city();
    let core = site_coverage(
        &plan,
        find_lot(&plan, Lot::Flats, true).0,
        find_lot(&plan, Lot::Flats, true).1,
    );
    let (hx, hy) = find_lot(&plan, Lot::House, false);
    let suburb = site_coverage(&plan, hx, hy);

    assert!(
        (0.60..=0.85).contains(&core),
        "a block of flats covering {:.0}% of its plot is not a city centre",
        core * 100.0
    );
    assert!(
        suburb < 0.45,
        "a house covering {:.0}% of its plot has no garden at all",
        suburb * 100.0
    );
    assert!(
        core > suburb * 1.7,
        "centre {:.0}% against suburb {:.0}% is not a density gradient",
        core * 100.0,
        suburb * 100.0
    );
}

#[test]
fn a_street_meeting_a_motorway_stops_at_it() {
    // **Severance, which was asserted in a doc comment and enforced
    // nowhere.** Reading a junction off the neighbouring plots could not
    // tell a lane joining a trunk road from two lanes meeting, so two
    // motorways crossed at grade in the middle of a city. The bigger road
    // runs through; the lesser one dead-ends against its corridor.
    use scale_sim::townplan::StreetClass;
    let plan = a_city();
    let t = TILES_PER_PLOT as i64;

    // A plot where a motorway column crosses a lesser row.
    let mut found = None;
    for y in 0..plan.height {
        for x in 0..plan.width {
            if plan.at(x, y) != Lot::Street {
                continue;
            }
            let (c, r) = (plan.col_class(x), plan.row_class(y));
            let crossing = matches!(
                (c, r),
                (Some(StreetClass::Motorway), Some(o)) if o != StreetClass::Motorway
            ) || matches!(
                (r, c),
                (Some(StreetClass::Motorway), Some(o)) if o != StreetClass::Motorway
            );
            if crossing {
                found = Some((x, y));
            }
        }
    }
    let Some((x, y)) = found else {
        return; // this town has no trunk route crossing a lesser street
    };

    let at = (x as i64 * t + t / 2, y as i64 * t + t / 2);
    let g = Ground::around(1, &plan, at, TILES_PER_PLOT);
    let mut paved = 0;
    for iy in 0..t {
        for ix in 0..t {
            let (gx, gy) = (x as i64 * t + ix, y as i64 * t + iy);
            let (vx, vy) = (gx - g.origin.0, gy - g.origin.1);
            if vx < 0 || vy < 0 || vx as usize >= g.w || vy as usize >= g.h {
                continue;
            }
            if g.at(vx as usize, vy as usize) == Tile::Pavement {
                paved += 1;
            }
        }
    }
    assert_eq!(
        paved, 0,
        "a footway across a motorway: the lesser street punched through"
    );
}

#[test]
fn a_city_centre_street_is_paved_to_the_building_line() {
    // The complement of `a_street_is_not_thirty_two_metres_of_tarmac`, and
    // the reason that one has to say *which* street it means. **A shopping
    // street has no verges** — the frontage comes out to the back of the
    // footway and the whole corridor is made ground. A street of houses
    // has grass, gardens and a kerb. Both are streets; they do not look
    // remotely alike, and before this they looked identical.
    let plan = a_city();
    let t = TILES_PER_PLOT as i64;
    let mut best = 0.0f64;
    for y in 1..plan.height - 1 {
        for x in 1..plan.width - 1 {
            if plan.at(x, y) != Lot::Street {
                continue;
            }
            // A stretch with dense frontage on both sides of it.
            let dense = |dx: i64, dy: i64| {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                matches!(plan.at(nx as usize, ny as usize), Lot::Flats | Lot::Shop)
            };
            if !((dense(-1, 0) && dense(1, 0)) || (dense(0, -1) && dense(0, 1))) {
                continue;
            }
            let at = (x as i64 * t + t / 2, y as i64 * t + t / 2);
            let g = Ground::around(1, &plan, at, TILES_PER_PLOT);
            let (mut made, mut total) = (0, 0);
            for iy in 0..t {
                for ix in 0..t {
                    let (gx, gy) = (x as i64 * t + ix, y as i64 * t + iy);
                    let (vx, vy) = (gx - g.origin.0, gy - g.origin.1);
                    if vx < 0 || vy < 0 || vx as usize >= g.w || vy as usize >= g.h {
                        continue;
                    }
                    total += 1;
                    if matches!(
                        g.at(vx as usize, vy as usize),
                        Tile::Road | Tile::Pavement | Tile::Marking | Tile::Shoulder
                    ) {
                        made += 1;
                    }
                }
            }
            best = best.max(made as f64 / total.max(1) as f64);
        }
    }
    assert!(
        best > 0.9,
        "the most built-up street in a city of 2.5M is only {:.0}% made surface —          it still has verges down a shopping street",
        best * 100.0
    );
}

#[test]
fn a_building_is_a_stack_of_floors() {
    // **Height is managed with Z levels**, the way Dwarf Fortress does it.
    // Before this a building was a floorplate with a number of storeys
    // asserted about it, which is how a block of eight dwellings on an
    // 800 m² plate came to 400 m² a flat — a mansion, not a tenement.
    //
    // A Z level is a storey and not a metre: floor-to-floor is 2.5-3 m.
    use scale_sim::ground::{storeys_of, METRES_PER_LEVEL};
    assert!((2.5..=3.2).contains(&METRES_PER_LEVEL));

    let plan = a_city();
    let at = stand_on(&plan, Lot::Flats);

    // Ground floor and third floor are both inside the building...
    let g0 = Ground::around_on(1, &plan, at, TILES_PER_PLOT, 0);
    let g3 = Ground::around_on(1, &plan, at, TILES_PER_PLOT, 3);
    let inside = |g: &Ground| {
        g.tiles
            .iter()
            .filter(|t| matches!(t, Tile::Floor | Tile::Furnishing(_) | Tile::Wall))
            .count()
    };
    assert!(
        inside(&g3) > 100,
        "the third floor of a block of flats is empty"
    );

    // ...and there is more open air up there, because the two-storey
    // shops either side of it have run out.
    //
    // A level asked for by name is that level, so the third floor is still
    // a horizontal slice and open air still reads as open air. It is the
    // level somebody is *standing* on that goes looking for the ground.
    let sky = |g: &Ground| g.tiles.iter().filter(|t| **t == Tile::Sky).count();
    assert_eq!(sky(&g0), 0, "open air at ground level");
    assert!(sky(&g3) > sky(&g0), "nothing thins out with height");

    // **Four storeys is the limit of a walk-up**, which is exactly where
    // lifts start, and a shed is one storey however big it is.
    assert!(
        storeys_of(Lot::Flats, 800) > 4,
        "a tenement of four storeys or fewer"
    );
    assert_eq!(
        storeys_of(Lot::House, 60),
        2,
        "a terraced house is two storeys"
    );
    assert_eq!(storeys_of(Lot::Works, 900), 1, "a shed with an upstairs");

    // Above the roof there is nothing at all.
    let high = Ground::around_on(1, &plan, at, TILES_PER_PLOT, 40);
    assert!(
        high.tiles.iter().all(|t| *t == Tile::Sky),
        "something is standing 120 m up"
    );
}

#[test]
fn a_room_has_a_door_and_something_in_it() {
    // A bare floor inside four walls is an area, not a place. What makes
    // an interior legible is partitions, doorways through them, and
    // furniture that says what each room is for — which is the thing CDDA
    // has and a single open room does not.
    let plan = a_city();
    let at = stand_on(&plan, Lot::Flats);
    let g = Ground::around(1, &plan, at, TILES_PER_PLOT);

    let count = |f: scale_sim::ground::Furnishing| {
        g.tiles
            .iter()
            .filter(|t| **t == Tile::Furnishing(f))
            .count()
    };
    use scale_sim::ground::Furnishing::*;
    assert!(count(Bed) > 0, "a block of flats with nowhere to sleep");
    assert!(count(Stove) > 0, "nowhere to cook");
    assert!(
        g.tiles.iter().filter(|t| **t == Tile::Door).count() > 4,
        "more than one room means more than one door"
    );
    // A stair, because you cannot get to the floors above without one.
    assert!(
        g.tiles.contains(&Tile::Stairs),
        "a block of flats with no stairwell"
    );
}

#[test]
fn down_is_a_direction_like_up() {
    // **+1 and -1 are both a new level.** Returning open air below ground
    // was simply wrong: under a town there is a cellar, then what a spade
    // goes through, then the rock the planet put there — which
    // `geology.rs` has known since it was written and nothing at this
    // layer had ever asked about.
    let plan = a_city();
    let at = stand_on(&plan, Lot::Street);

    // A made-up street has a sewer under it. Victorian brick sewers run
    // 3-10 m down, so one level is about right.
    let cellar = Ground::around_on(1, &plan, at, TILES_PER_PLOT, -1);
    assert!(
        cellar.tiles.contains(&Tile::Water),
        "a city street with nothing running under it"
    );
    assert!(
        cellar.tiles.iter().filter(|t| **t == Tile::Earth).count() > 100,
        "the ground either side of a sewer is not solid"
    );
    assert!(!cellar.tiles.contains(&Tile::Sky), "sky below ground");

    // Below the dug level nothing is hollow — but it is not bedrock
    // either. **Six metres down is still weathered rock**, which a spade
    // goes through; regolith runs to about ten metres and the real rock
    // is under that. Asserting stone at 6 m was asserting a quarry face.
    let shallow = Ground::around_on(1, &plan, at, TILES_PER_PLOT, -2);
    assert!(
        shallow.tiles.iter().all(|t| *t == Tile::Earth),
        "six metres down is stone, or hollow"
    );
    let deep = Ground::around_on(1, &plan, at, TILES_PER_PLOT, -10);
    assert!(
        deep.tiles.iter().all(|t| *t == Tile::Rock),
        "thirty metres down and still digging soil"
    );
    assert!(!Tile::Rock.walkable() && !Tile::Earth.walkable());
}

#[test]
fn cellars_follow_the_frost_line() {
    // **Not a matter of taste.** A footing must go below the frost line or
    // it heaves, so where frost is deep the hole is dug anyway and a
    // basement is nearly free: real frost depths are 1.5 m in Minnesota,
    // 1.2 m in New York, 0.13 m in Georgia, and US basement prevalence
    // follows almost exactly — ~80% in the Midwest and Northeast, under
    // 10% in the South. The opposite constraint is water: New Orleans has
    // no basements because the water table is a metre down.
    use scale_sim::world::Biome;
    let under = |b: Biome| {
        let plan = Plan::lay_out_on(20260828, 4242, 400_000.0, 32, b);
        let at = stand_on(&plan, Lot::Flats);
        let g = Ground::around_on(1, &plan, at, TILES_PER_PLOT, -1);
        // **Count the racking, not the floor.** A sewer has a ledge you
        // walk on, which is also `Floor`, and the window takes in the
        // street outside — so counting floor found a cellar under a marsh
        // that was actually a drain.
        g.tiles
            .iter()
            .filter(|t| **t == Tile::Fitting(Fixture::StockRack))
            .count()
    };
    assert!(under(Biome::Taiga) > 20, "hard winters and no cellars");
    assert_eq!(under(Biome::Swamp), 0, "a cellar dug into a marsh");
    assert_eq!(under(Biome::Desert), 0, "a cellar nobody needed to dig");
}

#[test]
fn a_geological_layer_spans_many_levels() {
    // **Terrain and material are separate**, and a layer is not a level.
    // Real thicknesses: topsoil 0.1-0.3 m, subsoil to 1-2, weathered rock
    // to ~10, sedimentary cover 0 on a shield and 1-2 km on a continent,
    // crystalline basement under all of it.
    use scale_sim::geology::Rock;
    use scale_sim::ground::{stratum_at, Stratum};

    let basin = a_city().on_rock(Rock::Sedimentary);
    assert_eq!(stratum_at(&basin, 0.1), Stratum::Topsoil);
    assert_eq!(stratum_at(&basin, 1.0), Stratum::Subsoil);
    assert_eq!(stratum_at(&basin, 5.0), Stratum::Regolith);
    // The cover runs for a kilometre and more — hundreds of Z levels of
    // the same layer, which is the whole point.
    assert!(matches!(stratum_at(&basin, 50.0), Stratum::Cover(_)));
    assert!(matches!(stratum_at(&basin, 1_000.0), Stratum::Cover(_)));
    assert!(matches!(stratum_at(&basin, 2_000.0), Stratum::Basement(_)));

    // **A shield has no cover**: exposed igneous or metamorphic rock at
    // the surface is precisely what that means.
    let shield = a_city().on_rock(Rock::Igneous);
    assert!(matches!(
        stratum_at(&shield, 50.0),
        Stratum::Basement(Rock::Igneous)
    ));
    assert_eq!(
        stratum_at(&shield, 0.1),
        Stratum::Topsoil,
        "a shield with no soil on it"
    );

    // Soil is soil whatever is underneath.
    assert_eq!(stratum_at(&basin, 1.0).rock(), None);
    assert!(!stratum_at(&basin, 1.0).is_rock());
}

#[test]
fn a_stair_is_a_connection_both_ends_agree_about() {
    // A stair down at (x,y,z) is only real if there is a stair up at
    // (x,y,z-1). Held as one tile's property the two can drift; asked as a
    // question about both ends they cannot. And a ramp is a *direction* —
    // the simulation must never read a glyph to decide what is walkable.
    use scale_sim::ground::{connections_at, surface_z};
    let plan = a_city();
    let at = stand_on(&plan, Lot::Flats);
    let sz = surface_z(1, &plan, at.0, at.1);

    // Find the stairwell.
    let g = Ground::around(1, &plan, at, TILES_PER_PLOT);
    let mut found = None;
    for y in 0..g.h {
        for x in 0..g.w {
            if g.at(x, y) == Tile::Stairs {
                found = Some((g.origin.0 + x as i64, g.origin.1 + y as i64));
            }
        }
    }
    let (sx, sy) = found.expect("a block of flats with no stairwell");

    let ground = connections_at(1, &plan, sx, sy, sz);
    assert!(
        ground.up,
        "a stairwell that goes nowhere from the ground floor"
    );
    let upper = connections_at(1, &plan, sx, sy, sz + 1);
    assert!(
        upper.down,
        "the floor above does not agree that the stair comes up to it"
    );

    // Away from the stairwell there is no connection at all.
    let none = connections_at(1, &plan, sx + 9, sy + 9, sz);
    assert!(!none.up && !none.down);
}

#[test]
fn what_happened_beats_what_was_generated() {
    // **The generator gives the initial state; the tile wins.**
    //
    // Everything below the world map is a pure function of seed and
    // coordinates, which is what keeps a save bounded — and it also meant
    // nothing could ever change. Knock a hole in a wall and the wall came
    // back the moment you looked away, because the generator has no memory
    // and it is the generator that answers.
    use scale_sim::ground::Changes;
    let plan = a_city();
    let at = stand_on(&plan, Lot::Flats);
    let z = scale_sim::ground::surface_z(1, &plan, at.0, at.1);

    // Find a wall to knock through.
    let before = Ground::around(1, &plan, at, TILES_PER_PLOT);
    let mut wall = None;
    for y in 0..before.h {
        for x in 0..before.w {
            if before.at(x, y) == Tile::Wall {
                wall = Some((before.origin.0 + x as i64, before.origin.1 + y as i64));
            }
        }
    }
    let (wx, wy) = wall.expect("a block of flats with no walls");

    let mut changes = Changes::new();
    assert!(changes.is_empty());
    changes.set((wx, wy, z), Tile::Floor);

    // Look away and look back: the hole is still there.
    let after = Ground::around_with(1, &plan, at, TILES_PER_PLOT, 0, &changes);
    let (vx, vy) = (
        (wx - after.origin.0) as usize,
        (wy - after.origin.1) as usize,
    );
    assert_eq!(
        after.at(vx, vy),
        Tile::Floor,
        "the wall grew back the moment nobody was looking"
    );

    // **A save costs what was done to the world, not what was seen of
    // it.** One knocked-through wall is one stored tile, however far the
    // player walked to reach it.
    assert_eq!(changes.len(), 1);

    // And nothing else moved: the rest is still the generator's answer.
    let plain = Ground::around(1, &plan, at, TILES_PER_PLOT);
    let differing = (0..plain.h)
        .flat_map(|y| (0..plain.w).map(move |x| (x, y)))
        .filter(|&(x, y)| plain.at(x, y) != after.at(x, y))
        .count();
    assert_eq!(differing, 1, "changing one tile changed {differing}");

    // Reverting hands it back to the generator.
    changes.revert((wx, wy, z));
    let restored = Ground::around_with(1, &plan, at, TILES_PER_PLOT, 0, &changes);
    assert_eq!(restored.at(vx, vy), Tile::Wall);
    assert!(changes.is_empty());
}

#[test]
fn a_floor_is_a_boundary_not_a_property_of_a_level() {
    // **"Floors and ceilings are boundaries between volumes, not implied
    // merely because the next Z coordinate exists."** A floor was implied
    // wherever the next level existed, which makes a shaft, an atrium, a
    // double-height bay and a breach four special cases instead of one
    // missing boundary.
    use scale_sim::ground::{floor_below, levels_of, levels_per_storey, storeys_of, surface_z};

    let plan = a_city();

    // At the surface the ground is the boundary, and it holds you up.
    let at = stand_on(&plan, Lot::Flats);
    let sz = surface_z(1, &plan, at.0, at.1);
    let ground = floor_below(1, &plan, at.0, at.1, sz).expect("no ground to stand on");
    assert!(ground.supports_weight() && !ground.liquid_permeable());

    // Between the storeys of a block of flats there is a floor.
    assert!(
        floor_below(1, &plan, at.0, at.1, sz + 1).is_some(),
        "the first floor of a block of flats has nothing under it"
    );

    // **Above the roof there is no boundary at all** — that is what open
    // air is, and it needs no separate representation.
    assert!(floor_below(1, &plan, at.0, at.1, sz + 40).is_none());

    // **A stairwell is a hole through every floor it passes.** Not a
    // special object: a floor that is open.
    let g = Ground::around(1, &plan, at, TILES_PER_PLOT);
    let mut stair = None;
    for y in 0..g.h {
        for x in 0..g.w {
            if g.at(x, y) == Tile::Stairs {
                stair = Some((g.origin.0 + x as i64, g.origin.1 + y as i64));
            }
        }
    }
    let (sx, sy) = stair.expect("a block of flats with no stairwell");
    let shaft = floor_below(1, &plan, sx, sy, sz + 1).expect("the shaft left the building");
    assert!(!shaft.supports_weight(), "a stairwell you cannot fall down");
    assert!(
        shaft.liquid_permeable(),
        "water that will not run down a stairwell"
    );

    // **A storey and a level are not the same thing.** A shed's clear
    // height is 6-12 m against a dwelling's 2.5-3, so a works is one
    // storey and several levels — with no floor part way up it.
    assert_eq!(levels_per_storey(Lot::Works), 3);
    assert_eq!(levels_per_storey(Lot::House), 1);
    assert_eq!(storeys_of(Lot::Works, 900), 1, "a shed with an upstairs");
    assert_eq!(
        levels_of(Lot::Works, 900),
        3,
        "a shed one storey tall and flat"
    );

    // Works sit out past the housing, so a 1 km square has none in it.
    let wide = Plan::lay_out_on(20260828, 4242, 2_500_000.0, 72, Biome::Grassland);
    let works = stand_on(&wide, Lot::Works);
    let wz = surface_z(1, &wide, works.0, works.1);
    assert!(
        floor_below(1, &wide, works.0, works.1, wz + 1).is_none(),
        "a floor half way up a warehouse bay"
    );
    assert!(
        floor_below(1, &wide, works.0, works.1, wz + 3).is_none(),
        "a shed with a floor above its roof"
    );
}

/// **A shop floor is mostly the space between the shelves.**
///
/// The fittings were right and the circulation was not: aisles one tile
/// wide, shelving hard against the walls, and the checkouts standing
/// immediately inside the door with nowhere to queue. You cannot pass a
/// trolley in a metre, a line of six people had nowhere to stand, and
/// getting to the far side of the shop meant walking through the shelving.
///
/// Real dimensions, which is what the layout is built from:
///
/// | | real |
/// |---|---|
/// | aisle, two trolleys passing | **1.8-2.4 m** |
/// | decompression zone inside the door | 1.5-4.5 m *(5-15 ft)* |
/// | queuing space at a checkout | 2-3 m |
/// | perimeter racetrack | the main circulation, wider than an aisle |
#[test]
fn a_shop_floor_can_be_walked_round() {
    let plan = a_city();
    let at = stand_on(&plan, Lot::Shop);
    // **Wide enough to hold the whole shop.** A superstore runs across
    // three plots — 96 m, because 2,800-4,650 m² does not fit on one — so
    // a window of a single plot cuts it in half.
    let g = Ground::around(1, &plan, at, TILES_PER_PLOT * 2);

    let idx = |x: usize, y: usize| y * g.w + x;
    let walkable: Vec<bool> = g.tiles.iter().map(|t| t.walkable()).collect();

    // --- everything you can stand on is one connected space ---
    //
    // Not an aesthetic point: an unreachable pocket of floor is somewhere
    // the shop has built shelving around, and nobody would.
    // **Start where the shopper is standing**, not at the first floor
    // tile in the window — that is inside whatever building the corner of
    // the view happens to clip, and flooding *its* interior proves
    // nothing about this one.
    let (sx, sy) = ((at.0 - g.origin.0), (at.1 - g.origin.1));
    let start = (0..g.w * g.h)
        .filter(|i| walkable[*i] && g.tiles[*i] == Tile::Floor)
        .min_by_key(|i| ((*i % g.w) as i64 - sx).abs() + ((*i / g.w) as i64 - sy).abs())
        .expect("a shop with no floor in it");
    let mut seen = vec![false; g.w * g.h];
    let mut stack = vec![start];
    seen[start] = true;
    while let Some(i) = stack.pop() {
        let (x, y) = (i % g.w, i / g.w);
        for (dx, dy) in [(0i64, -1i64), (1, 0), (0, 1), (-1, 0)] {
            let (nx, ny) = (x as i64 + dx, y as i64 + dy);
            if nx < 0 || ny < 0 || nx >= g.w as i64 || ny >= g.h as i64 {
                continue;
            }
            let n = idx(nx as usize, ny as usize);
            if walkable[n] && !seen[n] {
                seen[n] = true;
                stack.push(n);
            }
        }
    }

    // --- no gangway is one tile wide ---
    //
    // A floor tile with shelving on both sides of it is a corridor a
    // metre across: one trolley, and nobody coming the other way. Both
    // axes, because the gondolas run one way and the cross aisles the
    // other.
    let shelf = |x: i64, y: i64| {
        x >= 0
            && y >= 0
            && x < g.w as i64
            && y < g.h as i64
            && matches!(
                g.tiles[idx(x as usize, y as usize)],
                Tile::Fitting(Fixture::Shelving)
            )
    };
    let mut pinched = 0;
    for y in 0..g.h as i64 {
        for x in 0..g.w as i64 {
            if g.tiles[idx(x as usize, y as usize)] != Tile::Floor {
                continue;
            }
            if (shelf(x, y - 1) && shelf(x, y + 1)) || (shelf(x - 1, y) && shelf(x + 1, y)) {
                pinched += 1;
            }
        }
    }
    assert_eq!(
        pinched, 0,
        "{pinched} gangways in the shop are one tile wide, so two trolleys \
         cannot pass"
    );

    // --- you can reach every shelf, and get out past the tills ---
    let mut shelves = 0;
    let mut reachable_shelves = 0;
    for y in 0..g.h as i64 {
        for x in 0..g.w as i64 {
            if !shelf(x, y) {
                continue;
            }
            shelves += 1;
            if [(0i64, -1i64), (1, 0), (0, 1), (-1, 0)]
                .iter()
                .any(|(dx, dy)| {
                    let (nx, ny) = (x + dx, y + dy);
                    nx >= 0
                        && ny >= 0
                        && nx < g.w as i64
                        && ny < g.h as i64
                        && seen[idx(nx as usize, ny as usize)]
                })
            {
                reachable_shelves += 1;
            }
        }
    }
    assert!(shelves > 40, "only {shelves} shelf tiles in a supermarket");
    assert_eq!(
        shelves,
        reachable_shelves,
        "{} shelf tiles cannot be reached from the shop floor",
        shelves - reachable_shelves
    );

    // **A way in that is not through a checkout.** The entrance lane runs
    // past the end of the line, which is where the trolleys stand.
    let tills: Vec<usize> = (0..g.w * g.h)
        .filter(|i| g.tiles[*i] == Tile::Fitting(Fixture::Till))
        .collect();
    assert!(
        tills.len() > 4,
        "a supermarket with {} checkouts",
        tills.len()
    );
    let till_row = tills[0] / g.w;
    let gap = (0..g.w)
        .filter(|x| g.tiles[idx(*x, till_row)] == Tile::Floor)
        .count();
    assert!(
        gap >= 5,
        "only {gap} tiles of the checkout row are walkable, so there is no \
         way in that is not between two tills"
    );
}

/// **The back of house is a warehouse, and a warehouse is worked by
/// machine.**
///
/// Racking stood on every other column with a *one-metre* gap between the
/// runs — the same fault the sales floor had, except that back here the
/// thing that has to get down the aisle is a forklift with a pallet on it.
/// Real aisle widths: a counterbalance truck wants **3.0-3.6 m**, a reach
/// truck 2.5-2.8, and only a wire-guided very-narrow-aisle machine will go
/// below two. A pallet is 1.2 x 1.0 m and racking back to back is 2.4.
///
/// And a dock door is **3.0-3.5 m** wide with one per **10-12 m** of wall,
/// lined up with the bay in front of it — a bay a lorry reverses onto is
/// no use if the wall behind it is solid. Two metres every eight was a
/// door a pallet would not fit through, twice as often as anybody builds
/// them.
#[test]
fn a_stockroom_is_worked_by_forklift() {
    let plan = a_city();
    let at = stand_on(&plan, Lot::Shop);
    let g = Ground::around(1, &plan, at, TILES_PER_PLOT * 2);
    let idx = |x: usize, y: usize| y * g.w + x;

    // **One building, not everything the window clips.** A view two plots
    // across catches the back of the shop next door, and the racking in
    // *that* is laid out on its own building's coordinates — so measuring
    // over the whole window reads a gap between two separate stockrooms as
    // a gangway a metre wide. Scope to the dock nearest the shopper and
    // the walls either side of it.
    let (px, py) = ((at.0 - g.origin.0), (at.1 - g.origin.1));
    let dock_row = (0..g.h)
        .filter(|y| (0..g.w).any(|x| g.tiles[idx(x, *y)] == Tile::Fitting(Fixture::LoadingBay)))
        .min_by_key(|y| (*y as i64 - py).abs())
        .expect("a shop with no dock");
    let bay_x = (0..g.w)
        .filter(|x| g.tiles[idx(*x, dock_row)] == Tile::Fitting(Fixture::LoadingBay))
        .min_by_key(|x| (*x as i64 - px).abs())
        .expect("a dock row with no bays");
    let mut lo = bay_x;
    while lo > 0 && g.tiles[idx(lo - 1, dock_row)] != Tile::Wall {
        lo -= 1;
    }
    let mut hi = bay_x;
    while hi + 1 < g.w && g.tiles[idx(hi + 1, dock_row)] != Tile::Wall {
        hi += 1;
    }
    assert!(hi - lo > 20, "a dock only {} m wide", hi - lo);

    // And bounded above by the partition, or the scan runs on into the
    // back of the building behind this one.
    // Walk up the bay's own column: it is certainly inside this building,
    // and the first wall above it is the partition.
    let mut top = dock_row;
    while top > 0 && g.tiles[idx(bay_x, top - 1)] != Tile::Wall {
        top -= 1;
    }
    assert!(
        dock_row - top >= 3,
        "a stockroom only {} m deep",
        dock_row - top
    );

    // A cold room is racking that happens to be refrigerated: it stands
    // in the same runs and wants the same gangway.
    let rack = |x: usize, y: usize| {
        matches!(
            g.tiles[idx(x, y)],
            Tile::Fitting(Fixture::StockRack) | Tile::Fitting(Fixture::ColdStore)
        )
    };

    // --- every gangway takes a forklift ---
    //
    // Measured, not inferred: walk each row and take the length of every
    // run of floor with racking at both ends. That is the aisle, in
    // metres.
    let mut racks = 0;
    let mut narrowest = usize::MAX;
    for y in top..=dock_row {
        let (mut run, mut after_rack) = (0usize, false);
        for x in lo..=hi {
            if rack(x, y) {
                racks += 1;
                if after_rack && run > 0 {
                    narrowest = narrowest.min(run);
                }
                run = 0;
                after_rack = true;
            } else if g.tiles[idx(x, y)] == Tile::Floor {
                run += 1;
            } else {
                run = 0;
                after_rack = false;
            }
        }
    }
    // A single-plot shop, so a couple of dozen bays of racking. A
    // superstore across three plots carries three times this.
    assert!(racks > 20, "only {racks} tiles of racking in a stockroom");
    assert!(
        narrowest >= 3,
        "the tightest gangway between two rack runs is {narrowest} m, and a \
         counterbalance forklift needs three"
    );

    // --- the doors line up with the bays ---
    let mut bays = 0;
    let mut backed_by_a_door = 0;
    for x in lo..=hi {
        if g.tiles[idx(x, dock_row)] != Tile::Fitting(Fixture::LoadingBay) {
            continue;
        }
        bays += 1;
        if (1..=3).any(|d| dock_row + d < g.h && g.tiles[idx(x, dock_row + d)] == Tile::Door) {
            backed_by_a_door += 1;
        }
    }
    // **A shop has one to four docks, not one per ten metres of wall.**
    // That is a distribution centre's rule and it gave a supermarket
    // eight. Real: 5-15 HGV deliveries a day and 45-60 minutes to turn
    // one round, so a door handles 8-10 and a shop needs a couple.
    let docks = bays / 3;
    assert!(
        (1..=4).contains(&docks),
        "{docks} loading docks on one shop, which is a depot and not a shop"
    );
    assert_eq!(
        bays,
        backed_by_a_door,
        "{} loading bays of {bays} have no door behind them, so the goods \
         cannot come off the lorry",
        bays - backed_by_a_door
    );

    // --- a service elevation is blank ---
    //
    // The glazing belongs on the shopfront, where it sells something.
    // **Walked along the wall itself**, not over a band of rows: a couple
    // of rows past the dock is the yard, and beyond that the frontage of
    // whatever building stands on the far side of it, which is glazed
    // quite correctly.
    let wall_row = dock_row + 1;
    assert!(wall_row < g.h, "a dock with no wall behind it");
    let mut wx = bay_x;
    while wx > 0 && matches!(g.tiles[idx(wx - 1, wall_row)], Tile::Wall | Tile::Door) {
        wx -= 1;
    }
    let mut span = 0;
    while wx < g.w
        && matches!(
            g.tiles[idx(wx, wall_row)],
            Tile::Wall | Tile::Door | Tile::Window
        )
    {
        assert_ne!(
            g.tiles[idx(wx, wall_row)],
            Tile::Window,
            "a window at {wx},{wall_row} in the dock wall"
        );
        wx += 1;
        span += 1;
    }
    assert!(span > 20, "a rear elevation only {span} m long");
}
