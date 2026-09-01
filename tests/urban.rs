//! A city centre continues into a city.
//!
//! A junction in the middle of a large town should look out onto blocks,
//! frontage, pavements and service alleys — not straight back into
//! wilderness. Land that is open in a city centre has to be open *for a
//! reason*: a park, a yard, a corridor. Otherwise the settlement's
//! density field is not reaching the plots around it.

use scale_sim::townplan::{Lot, Plan};
use scale_sim::world::World;

/// **Every city on a seed, not just the biggest one.**
///
/// Testing only the largest town missed the case that was actually broken:
/// a city of 3.6M elsewhere on the same planet with open country at its
/// most central junction. One town passing says nothing about the field
/// reaching the others.
fn cities(seed: u64, how_many: usize) -> Vec<(String, Plan, f64)> {
    let world = World::generate(384, 216, seed);
    let pol = scale_sim::polity::Polities::partition(&world, 30);
    let set = scale_sim::settlement::Settlements::place(&world, &pol, 5000);
    let mut by_size: Vec<usize> = (0..set.list.len()).collect();
    by_size.sort_by(|&a, &b| {
        set.list[b]
            .population
            .cmp(&set.list[a].population)
            .then(a.cmp(&b))
    });
    by_size
        .into_iter()
        .take(how_many)
        .map(|i| {
            let s = &set.list[i];
            let plan =
                Plan::lay_out_on(seed, s.cell, s.population as f64, 40, world.biomes[s.cell])
                    .on_rock(world.geology.rock[s.cell]);
            (format!("#{i}"), plan, s.population as f64)
        })
        .collect()
}

/// Build the largest town on a seed and hand back its plan.
fn biggest_town(seed: u64) -> (Plan, f64) {
    let mut c = cities(seed, 1);
    let (_, plan, pop) = c.remove(0);
    (plan, pop)
}

/// **The centre of a city is built up in every direction.**
#[test]
fn city_centre_context_reaches_all_visible_plots() {
    for seed in [4242u64, 20260828, 1] {
      for (which, plan, pop) in cities(seed, 6) {
        if pop < 250_000.0 {
            continue;
        }
        let (cx, cy) = (plan.width / 2, plan.height / 2);

        // Everything within four plots of the middle — which is the 160 m
        // a person can see on foot, and more.
        let r = 4usize;
        let mut built = 0;
        let mut street = 0;
        let mut open = 0;
        for y in cy.saturating_sub(r)..(cy + r).min(plan.height) {
            for x in cx.saturating_sub(r)..(cx + r).min(plan.width) {
                match plan.at(x, y) {
                    Lot::Street => street += 1,
                    Lot::Open => open += 1,
                    _ => built += 1,
                }
            }
        }
        let total = (built + street + open) as f64;
        assert!(total > 0.0);

        // **Open ground in a city centre is the exception.** Real central
        // districts run 60-80% site coverage; a park is deliberate and a
        // field is not.
        assert!(
            open as f64 / total < 0.30,
            "seed {seed} {which}: {:.0}% of the middle of a town of {pop:.0} \
             is open country ({open} plots of {total:.0}), so the centre \
             looks out onto wilderness",
            open as f64 / total * 100.0
        );
        // And there is more building than road, which is what a district
        // is: blocks with streets between them, not streets with the
        // occasional block.
        assert!(
            built > street,
            "seed {seed} {which}: {street} street plots against {built} built \
             ones in the middle of a city of {pop:.0}"
        );
      }
    }
}

/// **What you actually look at from a central junction.**
///
/// Averaging over a district hides the case that matters: a crossroads
/// with blocks on one side and open reserve filling the whole of the
/// other. Somebody standing on that corner sees a city in front of them
/// and wilderness behind, which is not a city centre — it is the edge of
/// town with a dual carriageway through it.
#[test]
fn every_quadrant_of_a_central_junction_is_developed() {
    for seed in [4242u64, 20260828, 1] {
      for (which, plan, pop) in cities(seed, 6) {
        if pop < 250_000.0 {
            continue;
        }
        // The junction `walk --where corner` would find: the one nearest
        // the middle with streets on both axes.
        let (cx, cy) = (plan.width as i64 / 2, plan.height as i64 / 2);
        let mut best: Option<(usize, usize, i64)> = None;
        for y in 1..plan.height - 1 {
            for x in 1..plan.width - 1 {
                let junction = plan.at(x, y) == Lot::Street
                    && (plan.at(x, y - 1) == Lot::Street || plan.at(x, y + 1) == Lot::Street)
                    && (plan.at(x - 1, y) == Lot::Street || plan.at(x + 1, y) == Lot::Street);
                if !junction {
                    continue;
                }
                let d = (x as i64 - cx).abs().max(y as i64 - cy).abs();
                if best.is_none_or(|(_, _, bd)| d < bd) {
                    best = Some((x, y, d));
                }
            }
        }
        let Some((jx, jy, _)) = best else { continue };

        // The four corners of the crossroads. Each is what somebody
        // standing on the junction is looking at.
        let mut open_corners = Vec::new();
        for (dx, dy) in [(-1i64, -1i64), (1, -1), (-1, 1), (1, 1)] {
            let (x, y) = (jx as i64 + dx, jy as i64 + dy);
            if x < 0 || y < 0 || x >= plan.width as i64 || y >= plan.height as i64 {
                continue;
            }
            if plan.at(x as usize, y as usize) == Lot::Open {
                open_corners.push((dx, dy));
            }
        }
        assert!(
            open_corners.len() <= 1,
            "seed {seed} {which}: {} of the four corners of the most central \
             junction in a town of {pop:.0} are open country \
             ({open_corners:?}), so standing there you look at a city on one \
             side and reserve on the other",
            open_corners.len()
        );
      }
    }
}

/// **A town thins toward its edge and does not stop dead.** Clark's law:
/// density decays exponentially from the centre, which is why a town edge
/// is a gradient rather than a wall.
#[test]
fn density_falls_off_with_distance_from_the_middle() {
    let (plan, pop) = biggest_town(4242);
    assert!(pop > 10_000.0, "not a big enough town to say anything");
    let (cx, cy) = (plan.width as i64 / 2, plan.height as i64 / 2);

    let built_within = |lo: i64, hi: i64| -> f64 {
        let (mut built, mut all) = (0.0, 0.0);
        for y in 0..plan.height as i64 {
            for x in 0..plan.width as i64 {
                let d = (x - cx).abs().max((y - cy).abs());
                if d < lo || d >= hi {
                    continue;
                }
                all += 1.0;
                if !matches!(plan.at(x as usize, y as usize), Lot::Open) {
                    built += 1.0;
                }
            }
        }
        if all > 0.0 {
            built / all
        } else {
            0.0
        }
    };

    let core = built_within(0, 4);
    let middle = built_within(4, 9);
    let edge = built_within(9, 16);
    assert!(
        core > middle && middle > edge,
        "density should fall outward: core {core:.2}, middle {middle:.2}, \
         edge {edge:.2}"
    );
    assert!(core > 0.7, "a city core only {core:.0}% built up");
}
