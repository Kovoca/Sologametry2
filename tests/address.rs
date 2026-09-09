//! **Where something is, which is not what it is.**
//!
//! A firm that moves premises has a new address and is the same firm; a
//! building outlives whoever occupies it. So an address never identifies
//! anybody — it says where to take the pallet, and the party who owes for
//! the load is named separately. P.O. boxes are the proof that they must
//! be two things: a delivery address with no premises behind it, belonging
//! to somebody who is physically elsewhere.

use scale_sim::townplan::{Address, Axis, Lot, Pattern, Plan, Street};
use std::collections::{BTreeMap, BTreeSet};

fn a_town() -> Plan {
    // Big enough to have several streets each way and a real grid.
    Plan::lay_out(20260909, 41, 400_000.0, 40)
}

/// **Generated, never stored**, like the ground it stands on. Walking away
/// and coming back finds the same street with the same name, because the
/// name is a function of the plan rather than a thing somebody wrote down.
#[test]
fn a_street_has_the_same_name_when_you_come_back() {
    let a = a_town();
    let b = a_town();
    let mut named = 0;
    for line in 0..a.width {
        for axis in [Axis::NorthSouth, Axis::EastWest] {
            let s = Street { axis, line };
            if a.class_of(s).is_some() {
                assert_eq!(
                    a.street_name(s),
                    b.street_name(s),
                    "the same street is called two things on two visits"
                );
                named += 1;
            }
        }
    }
    assert!(named > 4, "only {named} streets in a town of 400,000");
}

/// **A surveyed town numbers one way and names the other**, which is why
/// you can navigate Manhattan without a map. A town nobody laid out has no
/// numbers anywhere, because numbering is something an authority does.
#[test]
fn the_naming_follows_whether_anybody_surveyed_it() {
    let grid = a_town();
    assert_eq!(
        grid.pattern,
        Pattern::Grid,
        "a city of 400,000 was not laid out on a grid, so this gate is testing the wrong town"
    );

    let mut numbered = 0;
    let mut named = 0;
    for line in 0..grid.width.max(grid.height) {
        for axis in [Axis::NorthSouth, Axis::EastWest] {
            let s = Street { axis, line };
            if grid.class_of(s).is_none() {
                continue;
            }
            let name = grid.street_name(s);
            let starts_with_a_digit = name.chars().next().is_some_and(|c| c.is_ascii_digit());
            match axis {
                Axis::NorthSouth => {
                    assert!(
                        starts_with_a_digit,
                        "a surveyed town should number one axis, and this one is called {name:?}"
                    );
                    numbered += 1;
                }
                Axis::EastWest => {
                    assert!(
                        !starts_with_a_digit,
                        "both axes are numbered, so an address cannot say which way it runs: \
                         {name:?}"
                    );
                    named += 1;
                }
            }
        }
    }
    assert!(
        numbered > 1 && named > 1,
        "{numbered} numbered, {named} named"
    );

    // And a place that grew has no numbers at all.
    let village = Plan::lay_out(20260909, 41, 1_200.0, 40);
    assert_ne!(village.pattern, Pattern::Grid);
    for line in 0..village.width.max(village.height) {
        for axis in [Axis::NorthSouth, Axis::EastWest] {
            let s = Street { axis, line };
            if village.class_of(s).is_none() {
                continue;
            }
            let name = village.street_name(s);
            assert!(
                !name.chars().next().is_some_and(|c| c.is_ascii_digit()),
                "nobody surveyed this place and it has a {name:?}"
            );
        }
    }
}

/// **The number alone tells you where to go**, which is the whole point of
/// a hundred-block. The 400 block of a street is between the fourth and
/// fifth crossings, so a stranger with a number and no map can find it —
/// and odd against even says which side to cross to.
#[test]
fn the_number_says_which_block_and_which_side() {
    let plan = a_town();
    let mut addresses: Vec<(usize, usize, Address)> = Vec::new();
    for y in 0..plan.height {
        for x in 0..plan.width {
            if let Some(a) = plan.address_at(0, x, y) {
                addresses.push((x, y, a));
            }
        }
    }
    assert!(
        addresses.len() > 200,
        "only {} addressed plots in a city",
        addresses.len()
    );

    // **Odd on one side, even on the other.** Not a decoration: it is how
    // you know whether to cross before you set off.
    for (x, y, a) in addresses.iter() {
        let on_the_low_side = match a.street.axis {
            Axis::NorthSouth => *x < a.street.line,
            Axis::EastWest => *y < a.street.line,
        };
        assert_eq!(
            a.number % 2 == 1,
            on_the_low_side,
            "{} at ({x}, {y}) is on the wrong side of its own street",
            plan.write_address(a, "town")
        );
    }

    // **The hundreds digit is the block.** Two buildings with numbers in
    // the same hundred are between the same pair of crossings.
    let mut by_block: BTreeMap<(Street, u32), Vec<usize>> = BTreeMap::new();
    for (_, y, a) in addresses.iter() {
        if a.street.axis == Axis::NorthSouth {
            by_block
                .entry((a.street, a.number / 100))
                .or_default()
                .push(*y);
        }
    }
    let mut checked = 0;
    for ((street, block), mut alongs) in by_block {
        alongs.sort_unstable();
        // Nobody in this block may be separated from the rest by a
        // crossing, or the number lies about where it is.
        let crossings: Vec<usize> = (0..plan.height)
            .filter(|&r| {
                plan.class_of(Street {
                    axis: Axis::EastWest,
                    line: r,
                })
                .is_some()
            })
            .collect();
        for w in alongs.windows(2) {
            let between = crossings.iter().filter(|&&c| c > w[0] && c < w[1]).count();
            assert_eq!(
                between,
                0,
                "the {}00 block of {} is split by {between} crossing(s), so its number does \
                 not say where it is",
                block,
                plan.street_name(street)
            );
        }
        checked += 1;
    }
    assert!(checked > 2, "only {checked} blocks were checked");
}

/// **No two buildings share an address**, or a delivery is a guess.
#[test]
fn an_address_belongs_to_one_place() {
    let plan = a_town();
    let mut seen: BTreeMap<Address, (usize, usize)> = BTreeMap::new();
    let mut clashes = Vec::new();
    for y in 0..plan.height {
        for x in 0..plan.width {
            let Some(a) = plan.address_at(0, x, y) else {
                continue;
            };
            if let Some(other) = seen.insert(a.clone(), (x, y)) {
                clashes.push((plan.write_address(&a, "town"), other, (x, y)));
            }
        }
    }
    assert!(
        clashes.is_empty(),
        "{} addresses belong to more than one plot, first {:?}",
        clashes.len(),
        clashes.first()
    );
}

/// **A street is not an address**, and neither is open country. Both are
/// real answers: you cannot deliver to the middle of the road, and a field
/// has no number.
#[test]
fn you_cannot_deliver_to_the_middle_of_the_road() {
    let plan = a_town();
    let mut streets = 0;
    for y in 0..plan.height {
        for x in 0..plan.width {
            if plan.at(x, y) == Lot::Street {
                assert!(
                    plan.address_at(0, x, y).is_none(),
                    "the carriageway at ({x}, {y}) has a house number"
                );
                streets += 1;
            }
        }
    }
    assert!(streets > 50, "only {streets} street plots in a city");

    // And every built plot that a street reaches does have one, because a
    // building nobody can deliver to is a building nobody can supply.
    let mut built = 0;
    let mut addressed = 0;
    for y in 0..plan.height {
        for x in 0..plan.width {
            if matches!(
                plan.at(x, y),
                Lot::House | Lot::Flats | Lot::Shop | Lot::Works
            ) {
                built += 1;
                if plan.address_at(0, x, y).is_some() {
                    addressed += 1;
                }
            }
        }
    }
    assert!(built > 100, "only {built} buildings");
    assert!(
        addressed * 100 / built.max(1) > 90,
        "only {addressed} of {built} buildings can be delivered to"
    );
}

/// **A corner takes the bigger street**, which is what a corner building
/// really does: the address is worth more on the busier road.
#[test]
fn a_corner_is_addressed_on_the_better_road() {
    let plan = a_town();
    let mut corners = 0;
    for y in 1..plan.height - 1 {
        for x in 1..plan.width - 1 {
            let Some(chosen) = plan.street_of(x, y) else {
                continue;
            };
            let touching: Vec<Street> = [
                Street {
                    axis: Axis::NorthSouth,
                    line: x - 1,
                },
                Street {
                    axis: Axis::NorthSouth,
                    line: x + 1,
                },
                Street {
                    axis: Axis::EastWest,
                    line: y - 1,
                },
                Street {
                    axis: Axis::EastWest,
                    line: y + 1,
                },
            ]
            .into_iter()
            .filter(|s| plan.class_of(*s).is_some())
            .collect();
            if touching.len() < 2 {
                continue;
            }
            corners += 1;
            let best = touching
                .iter()
                .map(|s| plan.class_of(*s).unwrap().size())
                .max()
                .unwrap();
            assert_eq!(
                plan.class_of(chosen).unwrap().size(),
                best,
                "a corner at ({x}, {y}) took {} when a bigger road was going past",
                plan.street_name(chosen)
            );
        }
    }
    assert!(corners > 10, "only {corners} corners in a city");
    let _: BTreeSet<u8> = BTreeSet::new();
}
