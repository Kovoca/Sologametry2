//! **A world with no reason for anything to differ.**
//!
//! In a real country a spread in days of cover between two towns can be
//! perfectly correct — a mountain port 1,200 km from the grain belt is
//! *supposed* to hold less and pay more — so "cover is level" is a claim
//! about the world rather than about the model, and asserting it in an
//! asymmetric fixture tests something nobody has established. That is why
//! the food-cover question went round in circles for two commits.
//!
//! `slice::symmetric` removes the ambiguity. Three towns identical in
//! every respect, a triangle of identical roads, one nation so they share
//! a season. Under rotation the world maps onto itself, so **any mechanism
//! that respects the economics must give each town the same answer.** A
//! spread is then not a signal. It is a bug.

use scale_sim::econ::{Commodity, Doctrine, Economy};
use scale_sim::logistics::Logistics;
use scale_sim::slice;

fn cover(e: &Economy, m: usize, c: Commodity) -> f64 {
    let held: f64 = (0..e.ledger.sites.len())
        .filter(|&s| e.ledger.sites[s].market == m)
        .map(|s| e.ledger.stock(s, c))
        .sum();
    let d = e.daily_draw(m, c);
    if d > 1e-9 {
        held / d
    } else {
        0.0
    }
}

fn covers(e: &Economy, c: Commodity) -> Vec<f64> {
    (0..e.markets.len()).map(|m| cover(e, m, c)).collect()
}

fn run(days: usize, hauliers: bool, reversed: bool) -> Economy {
    let mut e = slice::symmetric(Doctrine::Prudent);
    if reversed {
        // **The same world, scanned the other way round.** Site indices
        // are referenced by nothing else in a freshly built economy —
        // every site carries its own market — so reversing the vector
        // changes nothing whatever about the economics. If the answer
        // moves, the answer was never about the economics.
        e.ledger.sites.reverse();
    }
    if hauliers {
        e.logistics = Some(Logistics::found(&e));
    }
    for _ in 0..days {
        e.step();
    }
    e
}

/// **Three identical towns hold identical stock.**
///
/// The gate that found the bug. Before pro-rata distribution this came out
/// at **57, 33 and 8 days of food** — a monotone gradient by market index,
/// which is the signature of a scan in vector order and cannot be anything
/// else in a world where the three towns are interchangeable.
#[test]
fn three_identical_towns_end_up_in_the_same_place() {
    let e = run(400, false, false);
    for &c in Commodity::ALL.iter() {
        if !c.storable() {
            continue;
        }
        let v = covers(&e, c);
        let lo = v.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = v.iter().cloned().fold(0.0f64, f64::max);
        assert!(
            hi - lo < 0.01,
            "{c} cover differs between towns that are the same in every \
             respect: {v:?} (spread {:.3})",
            hi - lo
        );
    }
    e.ledger.assert_conserved();
}

/// **And the answer does not depend on the order of a `Vec`.**
///
/// The decisive experiment, because it varies exactly one thing that has
/// no economic content at all.
#[test]
fn reversing_the_site_vector_changes_nothing() {
    let forward = run(400, false, false);
    let backward = run(400, false, true);
    for &c in Commodity::ALL.iter() {
        if !c.storable() {
            continue;
        }
        let (a, b) = (covers(&forward, c), covers(&backward, c));
        for m in 0..a.len() {
            assert!(
                (a[m] - b[m]).abs() < 0.01,
                "{c} in town {m}: {} forward against {} reversed — the answer \
                 depends on where the sites sit in a list",
                a[m],
                b[m]
            );
        }
    }
}

/// **Where nobody outbids anybody, a shortage is shared.**
///
/// The gate for the tie-breaking half of the allocation rule, and getting
/// its fixture right took two attempts — both times by making the same
/// mistake this whole file exists to correct.
///
/// **Shutting two mills of the three does not test this.** It leaves one
/// town with a mill and two without, which is not a symmetric world any
/// more, and the answer — the surviving mill's own town takes every sack,
/// because carriage comes out of the seller's netback and the other two
/// are 800 km away — is then perfectly correct. Asserting an even outcome
/// there is asserting symmetry in a world I had just made asymmetric.
///
/// So the shortage is applied symmetrically: **every mill at 30%**. The
/// three towns stay interchangeable, every buyer's netback is identical,
/// the whole component is one band, and there is no fact about the world
/// that could make one of them go short before the others.
#[test]
fn a_shortage_is_shared_out_rather_than_taken_by_whoever_asks_first() {
    let mut e = slice::symmetric(Doctrine::Prudent);
    let mut throttled = 0;
    for s in 0..e.ledger.sites.len() {
        if e.ledger.sites[s].kind == scale_sim::econ::SiteKind::Mill {
            e.ledger.sites[s].throughput *= 0.30;
            throttled += 1;
        }
    }
    assert_eq!(throttled, 3, "the fixture no longer has three mills");

    for _ in 0..200 {
        e.step();
        e.ledger.assert_conserved();
    }

    for &c in &[Commodity::Flour, Commodity::ProcessedFood] {
        let v = covers(&e, c);
        let lo = v.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = v.iter().cloned().fold(0.0f64, f64::max);
        assert!(
            hi - lo < 0.05 * hi.max(1.0),
            "{c}: a shortage fell on {v:?} — somebody was served first"
        );
        // **And it fell on somebody.** A gate where everyone ends at zero
        // passes on an absence, which is the same mistake as a test that
        // never enters its branch.
        assert!(
            hi > 0.01,
            "{c} came out at nothing anywhere, so the gate proved nothing: {v:?}"
        );
    }

    // The shortage has to be real, or the sharing rule never binds and
    // this is measuring an adequate supply.
    let short = covers(&e, Commodity::ProcessedFood);
    let plenty = covers(&run(200, false, false), Commodity::ProcessedFood);
    assert!(
        short[0] < plenty[0] * 0.9,
        "milling at 30% left food at {:.2} against {:.2} — there was no          shortage to share out",
        short[0],
        plenty[0]
    );
}

/// **Hauliers do not disturb a world that is already level.**
///
/// A freight industry exists to move goods to where they are wanted. In a
/// country where every town already has what it needs there is nothing for
/// it to do, and a model whose carriers shuffle stock about to no purpose
/// is inventing work — which shows up as an oscillation rather than as
/// commerce.
///
/// **The tolerance is relative and the reason is worth recording rather
/// than hiding**: carriers rank markets by cover and break exact ties by
/// index, so in a world where three markets are *precisely* equal the
/// tiebreak has to pick one of them. The resulting wobble is a fraction of
/// a per cent and self-correcting, because tomorrow the town that was
/// served is no longer the neediest. What would not be acceptable is a
/// drift that grows, so the bar is a small share of the level rather than
/// an absolute figure that a bigger stockpile would sail through.
#[test]
fn carriers_have_nothing_to_do_in_a_country_that_is_already_even() {
    let e = run(400, true, false);
    for &c in Commodity::ALL.iter() {
        if !c.storable() {
            continue;
        }
        let v = covers(&e, c);
        let lo = v.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = v.iter().cloned().fold(0.0f64, f64::max);
        let level = v.iter().sum::<f64>() / v.len() as f64;
        assert!(
            hi - lo < (0.03 * level).max(0.01),
            "{c}: carriers moved a level country to {v:?} (spread {:.3})",
            hi - lo
        );
    }
    e.ledger.assert_conserved();
}

/// **No unexploited arbitrage in grain, within a country.**
///
/// The claim that is actually true of an *asymmetric* world, and the one
/// that should have been asserted instead of level cover all along. Two
/// towns may legitimately differ in price; what may not survive is a gap
/// wider than the cost of closing it, where the cheap end has stock to
/// spare. That is money left on the table.
///
/// **Grain is clean and the others are not**, and the boundary is
/// informative rather than embarrassing: grain is made in every town and
/// wanted in every town, so `distribute` reaches it. Medical grade is made
/// in one town and wanted in all of them, and the only mechanism that can
/// move it end to end is a haulier — who decides on **days of cover**
/// while `trade` decides on **price**, and only between adjacent towns.
/// So a price gap between two towns that are not neighbours has nothing
/// looking at it. That is a named gap, not a mystery.
#[test]
fn a_country_does_not_leave_grain_money_on_the_table() {
    use scale_sim::network::Network;
    use scale_sim::polity::Polities;
    use scale_sim::region::Nations;
    use scale_sim::settlement::Settlements;
    use scale_sim::world::World;

    let world = World::generate(384, 216, 7);
    let polities = Polities::partition(&world, 24);
    let settlements = Settlements::place(&world, &polities, 3000);
    let network = Network::build(&world, &settlements, 500);
    let mut n = Nations::build(
        &world,
        &polities,
        &settlements,
        &network,
        4,
        4,
        Doctrine::Prudent,
    );
    for _ in 0..400 {
        n.economy.step();
    }
    let e = &n.economy;
    let c = Commodity::Grain;

    let mut worst = 0.0f64;
    let mut pair = (0usize, 0usize);
    for dear in 0..e.markets.len() {
        for cheap in 0..e.markets.len() {
            if dear == cheap {
                continue;
            }
            // Same country only. Across a border the lanes are thin and
            // this file already records that as a known gap.
            if !n
                .markets_of
                .iter()
                .any(|ms| ms.contains(&dear) && ms.contains(&cheap))
            {
                continue;
            }
            if e.surplus(cheap, c) <= 1e-6 {
                continue;
            }
            let excess = e.markets[dear].price[c as usize]
                - e.markets[cheap].price[c as usize]
                - e.freight_between(cheap, dear);
            if excess > worst {
                worst = excess;
                pair = (cheap, dear);
            }
        }
    }
    let reference = e.markets[0].price[c as usize].max(1e-9);
    assert!(
        worst / reference < 0.05,
        "{} to {}: {worst:.2} a tonne of grain, {:.0}% of its price, going \
         begging inside one country",
        pair.0,
        pair.1,
        100.0 * worst / reference
    );
}

// =====================================================================
// storage order is not an economic fact
// =====================================================================

/// What the world came to, keyed by something that survives a shuffle.
///
/// **Names, not indices.** Comparing two permuted runs slot by slot would
/// compare a farm against a cannery and call the difference a defect;
/// comparing them by index after a permutation is not a comparison at all.
fn by_name(e: &Economy) -> std::collections::BTreeMap<String, (f64, f64)> {
    let mut out = std::collections::BTreeMap::new();
    for s in 0..e.ledger.sites.len() {
        let site = &e.ledger.sites[s];
        let stock: f64 = Commodity::ALL
            .iter()
            .filter(|c| c.storable())
            .map(|&c| e.ledger.stock(s, c))
            .sum();
        out.insert(site.name.clone(), (stock, site.ran));
    }
    for m in 0..e.markets.len() {
        let price: f64 = Commodity::ALL
            .iter()
            .map(|&c| e.markets[m].price[c as usize])
            .sum();
        let cover = cover(e, m, Commodity::ProcessedFood);
        out.insert(format!("market:{}", e.markets[m].name), (price, cover));
    }
    out
}

/// Deterministic orderings of `n` things. Not random: a gate that shuffles
/// differently every run cannot be reproduced when it fails.
fn shufflings(n: usize) -> Vec<Vec<usize>> {
    let ident: Vec<usize> = (0..n).collect();
    let mut out = vec![ident.clone()];
    out.push(ident.iter().rev().copied().collect());
    out.push((0..n).map(|i| (i + 1) % n).collect());
    out.push((0..n).map(|i| (i + n / 2) % n).collect());
    // Evens then odds, which separates neighbours that were adjacent.
    out.push(
        (0..n)
            .filter(|i| i % 2 == 0)
            .chain((0..n).filter(|i| i % 2 == 1))
            .collect(),
    );
    // A fixed hash, so the ordering has no relationship to anything the
    // economy cares about and is the same every run.
    let mut hashed: Vec<usize> = ident.clone();
    hashed.sort_by_key(|&i| {
        let mut z = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        z ^= z >> 29;
        z = z.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z ^ (z >> 32)
    });
    out.push(hashed);
    out
}

/// **The order things are stored in is not an economic fact.**
///
/// Reversing the site vector caught the original allocation bug, and one
/// reversal is one sample: a rule that happens to be symmetric under
/// reversal and biased under everything else would sail through it. Six
/// deterministic orderings, including one with no relationship to anything
/// the model cares about.
#[test]
fn no_ordering_of_the_sites_changes_the_answer() {
    let mut expected: Option<std::collections::BTreeMap<String, (f64, f64)>> = None;
    for (which, order) in shufflings(21).into_iter().enumerate() {
        let mut e = slice::symmetric(Doctrine::Prudent);
        assert_eq!(
            e.ledger.sites.len(),
            order.len(),
            "the fixture changed size"
        );
        // Permute the storage. Nothing else in a freshly built economy
        // holds a site index — `staff_today` and `payroll_met` are filled
        // by the first day's work, and every site carries its own market.
        let sites: Vec<_> = order.iter().map(|&i| e.ledger.sites[i].clone()).collect();
        e.ledger.sites = sites;
        for _ in 0..200 {
            e.step();
        }
        e.ledger.assert_conserved();

        let got = by_name(&e);
        match &expected {
            None => expected = Some(got),
            Some(want) => {
                for (name, &(a, b)) in want {
                    let &(x, y) = got
                        .get(name)
                        .unwrap_or_else(|| panic!("ordering {which} lost {name} altogether"));
                    let scale = a.abs().max(1.0);
                    assert!(
                        (a - x).abs() / scale < 1e-6,
                        "ordering {which}: {name} holds {x} against {a} — the \
                         answer depends on where things sit in a list"
                    );
                    let scale = b.abs().max(1.0);
                    assert!(
                        (b - y).abs() / scale < 1e-6,
                        "ordering {which}: {name} ran {y} against {b}"
                    );
                }
            }
        }
    }
}

/// **And every ordering of the markets themselves.**
///
/// Harder than shuffling sites, because a market index is referenced from
/// three places — every site's `market`, both ends of every road, and the
/// per-market vectors that run alongside. All six orderings of the three,
/// each fully remapped, which is what makes the fixture's symmetry a claim
/// about the model rather than about the order the towns were declared in.
#[test]
fn no_ordering_of_the_markets_changes_the_answer() {
    // All six permutations of three, written out: a gate whose own
    // ordering is generated is a gate with a second thing to get wrong.
    const ORDERS: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let mut expected: Option<std::collections::BTreeMap<String, (f64, f64)>> = None;
    for (which, order) in ORDERS.iter().enumerate() {
        let mut e = slice::symmetric(Doctrine::Prudent);
        assert_eq!(e.markets.len(), 3);
        // `order[new] = old`, so `to_new[old] = new`.
        let mut to_new = [0usize; 3];
        for (new, &old) in order.iter().enumerate() {
            to_new[old] = new;
        }
        e.markets = order.iter().map(|&old| e.markets[old].clone()).collect();
        e.workforce = order.iter().map(|&old| e.workforce[old].clone()).collect();
        for site in e.ledger.sites.iter_mut() {
            site.market = to_new[site.market];
        }
        for r in e.routes.iter_mut() {
            r.a = to_new[r.a];
            r.b = to_new[r.b];
        }
        e.resurvey();

        for _ in 0..200 {
            e.step();
        }
        e.ledger.assert_conserved();

        let got = by_name(&e);
        match &expected {
            None => expected = Some(got),
            Some(want) => {
                for (name, &(a, b)) in want {
                    let &(x, y) = got
                        .get(name)
                        .unwrap_or_else(|| panic!("ordering {which} lost {name}"));
                    let scale = a.abs().max(1.0);
                    assert!(
                        (a - x).abs() / scale < 1e-6,
                        "ordering {which:?}: {name} is {x} against {a} — which \
                         town is which depends on the order they were declared"
                    );
                    let scale = b.abs().max(1.0);
                    assert!((b - y).abs() / scale < 1e-6, "ordering {which}: {name}");
                }
            }
        }
    }
}

/// **The cheap plant runs and the dear one waits.**
///
/// The other half of dispatch, and it needs saying separately because the
/// symmetric fixture cannot test it: three identical stations tie, the
/// whole fleet is one band, and the cost comparison never discriminates.
/// Sorting by cost was therefore a mechanism no gate exercised — which
/// this project has now caught in itself three times — so here is a world
/// where the plants are not alike.
///
/// This is the same rule `power.rs` holds for generation and the reason a
/// windy night clears at almost nothing: cheapest first, and the last unit
/// needed sets the price. Nothing about it is a preference for tidiness —
/// a grid that dispatched its most expensive plant first would burn a
/// country's money for no reason.
#[test]
fn dispatch_runs_the_cheap_station_first() {
    let mut e = slice::symmetric(Doctrine::Prudent);
    // One station on a rich, cheap seam; one on a poor one; one ordinary.
    let mut plants: Vec<usize> = (0..e.ledger.sites.len())
        .filter(|&s| e.ledger.sites[s].kind == scale_sim::econ::SiteKind::PowerPlant)
        .collect();
    assert_eq!(plants.len(), 3, "the fixture no longer has three stations");
    plants.sort();
    // **The cheapest is deliberately last in the vector.** Making the
    // first plant the cheapest lets index order and cost order agree, so
    // the gate passes with the cost comparison deleted -- which is exactly
    // what happened on the first attempt at writing this.
    e.ledger.sites[plants[0]].cost_factor = 2.0;
    e.ledger.sites[plants[1]].cost_factor = 1.0;
    e.ledger.sites[plants[2]].cost_factor = 0.5;

    for _ in 0..30 {
        e.step();
    }

    let ran: Vec<f64> = plants.iter().map(|&s| e.ledger.sites[s].ran).collect();
    assert!(
        ran[2] > ran[1] && ran[1] >= ran[0],
        "dispatch ran {ran:?} for plants costing 2.0, 1.0 and 0.5 — the \
         grid is not choosing on cost"
    );
    assert!(
        ran[2] > 1e-9,
        "the cheapest station on the system did not run at all"
    );
    e.ledger.assert_conserved();
}

// =====================================================================
// the last unit dispatched sets the price
// =====================================================================

/// **A cheap plant does not make cheap electricity if a dear one is still
/// needed.**
///
/// The least intuitive fact in a real wholesale market, and the reason it
/// has to be gated rather than assumed: everybody on the system is paid
/// what it cost to meet the *last* megawatt-hour of the call. A wind farm
/// with no fuel bill earns exactly what the gas turbine that happened to
/// be last earns.
///
/// Electricity was priced on days of cover, which is a category error for
/// something that is never stored — it declares zero target cover
/// precisely because none of it is ever held. `power.rs` has held the
/// merit-order model since it was written and the ledger had never used
/// it.
#[test]
fn the_last_plant_dispatched_sets_the_price() {
    let mut e = slice::symmetric(Doctrine::Prudent);
    let plants: Vec<usize> = (0..e.ledger.sites.len())
        .filter(|&s| e.ledger.sites[s].kind == scale_sim::econ::SiteKind::PowerPlant)
        .collect();
    assert_eq!(plants.len(), 3, "the fixture no longer has three stations");

    // **A cheap one, an ordinary one and a dear one, none of them big
    // enough alone.** That last part is the whole test: if any station can
    // carry the national load by itself, the cheapest always covers the
    // call and no dearer plant is ever on the margin — which is not a
    // merit order, it is a single supplier.
    let call = e.power_demand();
    for (i, factor) in [0.25f64, 1.00, 3.00].iter().enumerate() {
        e.ledger.sites[plants[i]].cost_factor = *factor;
        e.ledger.sites[plants[i]].throughput = call * 0.5;
    }
    for _ in 0..30 {
        e.step();
    }

    // **The discriminating assertion**, and the first version of this gate
    // did not have it: a weighted average of the running plants tracks the
    // margin closely enough that "the price did not go down" passes with
    // the mechanism deleted. What separates a clearing price from an
    // average is that it is set by the *worst* plant on the system, so it
    // must sit strictly above the average.
    let price = e.price(0, Commodity::Electricity);
    let average = e.markets[0].cost[Commodity::Electricity as usize];
    assert!(
        price > average * 1.10,
        "electricity cleared at {price:.2} against an average production          cost of {average:.2} — that is an average, not a market. The last          plant dispatched is supposed to set the price for everybody."
    );

    // And every plant is paid it, including the cheap one — the least
    // intuitive fact in a real wholesale market and the reason a wind farm
    // with no fuel bill earns what the gas turbine earns.
    assert!(
        e.power_clearing.is_some(),
        "nothing was dispatched, so nothing set a price"
    );
    assert!(
        e.ledger.sites[plants[0]].ran > 1e-9,
        "the cheapest station did not run"
    );
    e.ledger.assert_conserved();
}

/// **A shortage is a different thing from a high price.**
///
/// Real markets cap it administratively rather than letting it run away —
/// ERCOT's was $9,000/MWh in the February 2021 Texas freeze, around two
/// hundred times an ordinary wholesale price, and it sat there for four
/// days and bankrupted several retailers.
#[test]
fn unserved_load_prices_at_the_cap_and_not_beyond_it() {
    let mut e = slice::symmetric(Doctrine::Prudent);
    for _ in 0..20 {
        e.step();
    }
    let ordinary = e.price(0, Commodity::Electricity);

    // **Take the coal away**, through the journal like every other change
    // to a stockpile. The plants stand, the load does not go away, and
    // nothing can be generated.
    //
    // Zeroing `throughput` does not work and it is worth saying why: on a
    // power station that field is a sentinel meaning "whatever the grid
    // can carry", so dispatch is limited by fuel and never reads it. That
    // sentinel has now bitten four separate times.
    for site in 0..e.ledger.sites.len() {
        if e.ledger.sites[site].kind != scale_sim::econ::SiteKind::PowerPlant {
            continue;
        }
        let qty = e.ledger.stock(site, Commodity::Coal);
        if qty > 0.0 {
            e.ledger.apply(
                &mut e.journal,
                scale_sim::econ::Event::Consumed {
                    site,
                    commodity: Commodity::Coal,
                    qty,
                    reason: scale_sim::econ::Use::Input,
                },
            );
        }
    }
    for _ in 0..10 {
        e.step();
    }
    let short = e.price(0, Commodity::Electricity);

    assert!(
        short > ordinary * 5.0,
        "a grid that can generate nothing priced at {short:.1} against an \
         ordinary {ordinary:.1} — unserved load is not reaching the price"
    );
    assert!(
        short <= Commodity::Electricity.base_cost() * 201.0,
        "the price ran to {short:.1}, past the administrative cap that \
         every real market has"
    );
    e.ledger.assert_conserved();
}
