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
            if plain_frontage && [(0i64, -1i64), (0, 1), (-1, 0), (1, 0)].iter().any(|&(dx, dy)| {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                nx >= 0
                    && ny >= 0
                    && (nx as usize) < plan.width
                    && (ny as usize) < plan.height
                    && matches!(plan.at(nx as usize, ny as usize), Lot::Flats | Lot::Shop)
            }) {
                continue;
            }
            let at = (x as i64 * t + t / 2, y as i64 * t + t / 2);
            // Wide enough to hold the whole plot: a motorway corridor is
            // 33 m, and a window that clipped it made this test pass by
            // never seeing the hard shoulders.
            let g = Ground::around(1, plan, at, TILES_PER_PLOT);
            let (mut surface, mut foot) = (0, 0);
            for k in -(TILES_PER_PLOT as i64 / 2)..TILES_PER_PLOT as i64 / 2 {
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
    assert_eq!((lane, lane_foot), (5, 4), "a lane is 5 m shared and 2 m of path each side");
    assert_eq!((road, road_foot), (7, 6), "a road is 7.3 m of two lanes and 2.5 m of path");
    assert_eq!(dual, 16, "a dual is two carriageways of 7.3 m either side of a reserve");
    assert_eq!(motorway, 29, "a motorway is 11 m of three lanes and 3.3 m of shoulder, twice");

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
            if !matches!(
                g.at(vx as usize, vy as usize),
                Tile::Grass | Tile::Scrub | Tile::Sand | Tile::Rock
                    | Tile::Snow | Tile::Water | Tile::Tree
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
    best.map(|(p, _)| p).unwrap_or_else(|| panic!("no {want:?} in this town"))
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
    let core = site_coverage(&plan, find_lot(&plan, Lot::Flats, true).0, find_lot(&plan, Lot::Flats, true).1);
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
    assert!(inside(&g3) > 100, "the third floor of a block of flats is empty");

    // ...and there is more open air up there, because the two-storey
    // shops either side of it have run out.
    let sky = |g: &Ground| g.tiles.iter().filter(|t| **t == Tile::Sky).count();
    assert_eq!(sky(&g0), 0, "open air at ground level");
    assert!(sky(&g3) > sky(&g0), "nothing thins out with height");

    // **Four storeys is the limit of a walk-up**, which is exactly where
    // lifts start, and a shed is one storey however big it is.
    assert!(storeys_of(Lot::Flats, 800) > 4, "a tenement of four storeys or fewer");
    assert_eq!(storeys_of(Lot::House, 60), 2, "a terraced house is two storeys");
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
        g.tiles.iter().filter(|t| **t == Tile::Furnishing(f)).count()
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
        g.tiles.iter().any(|t| *t == Tile::Stairs),
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
        cellar.tiles.iter().any(|t| *t == Tile::Water),
        "a city street with nothing running under it"
    );
    assert!(
        cellar.tiles.iter().filter(|t| **t == Tile::Earth).count() > 100,
        "the ground either side of a sewer is not solid"
    );
    assert!(!cellar.tiles.iter().any(|t| *t == Tile::Sky), "sky below ground");

    // Below the dug level it is rock all the way.
    let deep = Ground::around_on(1, &plan, at, TILES_PER_PLOT, -2);
    assert!(
        deep.tiles.iter().all(|t| *t == Tile::Rock),
        "something hollow at 6 m down that nobody dug"
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
