//! Who actually moves the goods.
//!
//! The economy had two ways for a tonne to travel and neither was a
//! haulier. `distribute` is a **pull**: every site looks round for a
//! supplier, prefers a local one, and takes what it can reach.  `trade` is
//! a **price test**: on each route separately, if the gap between two
//! adjacent markets beats the freight, something moves.
//!
//! Both are pairwise, and that is the flaw. A cargo going three towns
//! down the road has to clear a separate test at every hop, so it usually
//! never sets off — which is why two hospitals in five ran at 72% of their
//! supplies while the national stock sat in the town with the works, and
//! why `nations.rs` records that trade volumes never equalise prices
//! across a border.
//!
//! A real freight operator does neither. It is paid to move somebody
//! else's stock from where it is to where it is wanted, it plans the
//! whole journey before the lorry leaves, and it does not care what the
//! goods are worth except insofar as the customer can afford the haul.
//! **That is a different algorithm, not a better-tuned version of the
//! same one**, and it is what this is.
//!
//! Real shape of the industry, which the model follows:
//!
//! - UK road freight moves ~**1.6 bn tonnes** and ~**150 bn tonne-km** a
//!   year, so the average haul is about **94 km** — most freight is
//!   local, and the long-distance trunk work is the visible minority.
//! - **Most hauliers are tiny.** Around 60% of UK operators run one or
//!   two vehicles; a handful run thousands. The same shape `building.rs`
//!   already records for firms in general: almost every *business* is one
//!   person, almost every *job* is at a big one.
//! - Third-party logistics — carrying other people's goods for a fee — is
//!   an industry of about **£30 bn** in Britain alone.
//! - An artic carries **24 t** and covers **550-700 km** in a legal day,
//!   both of which `vehicle.rs` already knows.

use crate::econ::{Commodity, Economy, Event};

/// **What a lorry costs to run for a day**, as a share of what it earns.
/// Fuel, tyres, wear, the driver, the operator's licence and the depot.
const OPERATING_MARGIN: f64 = 0.88;

/// Payload of the standard vehicle, in tonnes. A 44 t artic at the
/// European legal maximum.
const PAYLOAD_T: f64 = 24.0;

/// **A legal day, not a theoretical one.** Nine hours is the driving
/// maximum and seven is a working day once loading, queueing, town speeds
/// and mandatory breaks are taken out.
const KM_PER_DAY: f64 = 620.0;

/// A firm that carries other people's goods.
pub struct Carrier {
    pub name: String,
    /// The depot. A carrier works outward from somewhere.
    pub home: usize,
    /// Artics, or their equivalent in smaller vehicles.
    pub vehicles: f64,
    /// **Tonne-kilometres a day**, which is the unit the industry
    /// actually sells: a lorry's day is a distance, and what it is worth
    /// depends on how much was in it.
    pub capacity_t_km: f64,
    /// Charged per tonne-kilometre. Real paved road haulage runs
    /// $0.05-0.10, which is the figure `infrastructure.rs` already uses.
    pub rate: f64,
    // --- the day's work ---
    pub hauled_t: f64,
    pub worked_t_km: f64,
    pub revenue: f64,
    /// Cumulative, for reporting.
    pub lifetime_t: f64,
}

impl Carrier {
    fn spare(&self) -> f64 {
        (self.capacity_t_km - self.worked_t_km).max(0.0)
    }

    /// How busy it was today, 0 to 1 — which is what decides whether it
    /// is hiring drivers.
    pub fn utilisation(&self) -> f64 {
        if self.capacity_t_km <= 0.0 {
            return 0.0;
        }
        (self.worked_t_km / self.capacity_t_km).clamp(0.0, 1.0)
    }

    /// Drivers on the books. One per vehicle, near enough: real haulage
    /// runs slightly over one driver per truck once shifts and holidays
    /// are counted, and slightly under it when there is a driver shortage.
    pub fn drivers(&self) -> f64 {
        self.vehicles
    }
}

/// The country's freight industry, and the network it works over.
pub struct Logistics {
    pub carriers: Vec<Carrier>,
    /// All-pairs end-to-end freight cost per tonne. `None` where no route
    /// exists — which is the honest answer for an island or a town the
    /// roads never reached.
    cost: Vec<Vec<Option<f64>>>,
    /// All-pairs distance along the roads actually taken.
    km: Vec<Vec<f64>>,
}

impl Logistics {
    /// **Set up the industry**, sized on what the country actually has to
    /// move rather than on a number somebody chose.
    ///
    /// Every town gets a carrier, because freight is a local business
    /// before it is a national one, and the fleet follows population —
    /// which is the same reasoning `services.rs` uses and for the same
    /// reason: haulage is consumed where the goods are.
    pub fn found(econ: &Economy) -> Logistics {
        /// **Real internal freight is ~24 tonnes a head a year** (the UK
        /// moves ~1.6 bn tonnes across 67M people), and it is dominated by
        /// bulk. Most of that never leaves the town; this is the share
        /// that goes between them.
        const INTERTOWN_SHARE: f64 = 0.18;
        const TONNES_PER_HEAD_YEAR: f64 = 24.0;

        let carriers = (0..econ.markets.len())
            .map(|m| {
                let people = econ.markets[m].population;
                let tonnes_a_day = people * TONNES_PER_HEAD_YEAR / 365.0 * INTERTOWN_SHARE;
                // A vehicle shifts a payload over a day's distance, so
                // the fleet a town needs falls out of the tonnage and the
                // typical haul rather than being asserted.
                const TYPICAL_HAUL_KM: f64 = 94.0;
                let vehicles = (tonnes_a_day * TYPICAL_HAUL_KM) / (PAYLOAD_T * KM_PER_DAY);
                Carrier {
                    name: format!("{} carriers", econ.markets[m].name),
                    home: m,
                    vehicles: vehicles.max(1.0),
                    capacity_t_km: vehicles.max(1.0) * PAYLOAD_T * KM_PER_DAY,
                    rate: 0.08,
                    hauled_t: 0.0,
                    worked_t_km: 0.0,
                    revenue: 0.0,
                    lifetime_t: 0.0,
                }
            })
            .collect();

        let mut l = Logistics {
            carriers,
            cost: Vec::new(),
            km: Vec::new(),
        };
        l.survey(econ);
        l
    }

    /// **Plan the whole journey before the lorry leaves.**
    ///
    /// Dijkstra from every town over the routes that are open today, so a
    /// haul three towns down the road is one decision costed end to end
    /// rather than three separate tests it has to pass in a row. The
    /// network is small — a handful of towns — so all-pairs is the honest
    /// and cheap answer.
    ///
    /// Re-surveyed daily, because a pass shuts in winter and a road that
    /// has been let go costs more than it did last year.
    pub fn survey(&mut self, econ: &Economy) {
        let n = econ.markets.len();
        let mut adj: Vec<Vec<(usize, f64, f64)>> = vec![Vec::new(); n];
        for r in econ.routes.iter().filter(|r| r.usable()) {
            adj[r.a].push((r.b, r.freight_cost, r.km));
            adj[r.b].push((r.a, r.freight_cost, r.km));
        }

        self.cost = vec![vec![None; n]; n];
        self.km = vec![vec![0.0; n]; n];

        for start in 0..n {
            let mut best = vec![f64::INFINITY; n];
            let mut dist = vec![0.0f64; n];
            let mut done = vec![false; n];
            best[start] = 0.0;
            for _ in 0..n {
                // Deterministic: cheapest first, index breaking ties.
                let mut at = usize::MAX;
                let mut cheapest = f64::INFINITY;
                for i in 0..n {
                    if !done[i] && best[i] < cheapest {
                        cheapest = best[i];
                        at = i;
                    }
                }
                if at == usize::MAX {
                    break;
                }
                done[at] = true;
                for &(to, c, d) in adj[at].iter() {
                    let through = best[at] + c;
                    if through < best[to] {
                        best[to] = through;
                        dist[to] = dist[at] + d;
                    }
                }
            }
            for i in 0..n {
                self.cost[start][i] = if best[i].is_finite() {
                    Some(best[i])
                } else {
                    None
                };
                self.km[start][i] = dist[i];
            }
        }
    }

    /// End-to-end freight cost per tonne, or `None` if the roads do not
    /// join the two.
    pub fn freight(&self, from: usize, to: usize) -> Option<f64> {
        self.cost.get(from).and_then(|r| r.get(to).copied()).flatten()
    }

    pub fn distance_km(&self, from: usize, to: usize) -> f64 {
        self.km
            .get(from)
            .and_then(|r| r.get(to).copied())
            .unwrap_or(0.0)
    }

    /// **A day's haulage.**
    ///
    /// Runs on **days of cover, never on tonnes** — a city of sixteen
    /// million always holds more tonnes than a town of two, so comparing
    /// stock levels sends freight one way for ever and never back. That
    /// mistake is already recorded in this project's notes; it is made
    /// once.
    pub fn haul(&mut self, econ: &mut Economy, _day: u64) {
        self.survey(econ);
        for c in self.carriers.iter_mut() {
            c.hauled_t = 0.0;
            c.worked_t_km = 0.0;
            c.revenue = 0.0;
        }

        let n = econ.markets.len();
        for &commodity in Commodity::ALL.iter() {
            if !commodity.storable() {
                continue;
            }

            // Cover in days, market by market.
            let mut cover: Vec<f64> = Vec::with_capacity(n);
            let mut held: Vec<f64> = Vec::with_capacity(n);
            let mut draw: Vec<f64> = Vec::with_capacity(n);
            for m in 0..n {
                let d = econ.daily_draw(m, commodity);
                let h: f64 = (0..econ.ledger.sites.len())
                    .filter(|&s| econ.ledger.sites[s].market == m)
                    .map(|s| econ.ledger.stock(s, commodity))
                    .sum();
                held.push(h);
                draw.push(d);
                cover.push(if d > 1e-9 { h / d } else { f64::INFINITY });
            }

            // Who is short, worst first.
            let target = commodity.target_cover_days();
            let mut short: Vec<usize> = (0..n)
                .filter(|&m| draw[m] > 1e-9 && cover[m] < target)
                .collect();
            short.sort_by(|&a, &b| cover[a].total_cmp(&cover[b]).then(a.cmp(&b)));

            for dst in short {
                // How much would put this market right.
                let want = (target * draw[dst] - held[dst]).max(0.0);
                if want <= 1e-6 {
                    continue;
                }

                // Who can spare it, best-covered first.
                let mut sources: Vec<usize> = (0..n)
                    .filter(|&m| m != dst && econ.surplus(m, commodity) > 1e-6)
                    .collect();
                sources.sort_by(|&a, &b| cover[b].total_cmp(&cover[a]).then(a.cmp(&b)));

                let mut still_wanted = want;
                for src in sources {
                    if still_wanted <= 1e-6 {
                        break;
                    }
                    let Some(per_tonne) = self.freight(src, dst) else {
                        continue;
                    };
                    let km = self.distance_km(src, dst);

                    // **Value density decides how far a thing travels**,
                    // and it is not a rule anybody wrote: cement and
                    // aggregate barely move because the haul costs more
                    // than the goods, which is exactly why there is a
                    // cement works in every region and a pharmaceutical
                    // plant in hardly any country at all.
                    if per_tonne > commodity.base_cost() * 0.5 {
                        continue;
                    }

                    // **Never strip the source.** A market must keep its
                    // own working reserve or the two towns just slosh
                    // stock back and forth for ever.
                    let spare = econ.surplus(src, commodity);
                    let mut qty = still_wanted.min(spare);
                    if qty <= 1e-6 {
                        continue;
                    }

                    // Whichever carrier has room. A haul is offered to the
                    // depot at either end first — a lorry would rather
                    // start or finish at home than run empty both ways.
                    let mut order: Vec<usize> = (0..self.carriers.len()).collect();
                    order.sort_by_key(|&i| {
                        let h = self.carriers[i].home;
                        let near = if h == src || h == dst { 0 } else { 1 };
                        (near, i)
                    });

                    for ci in order {
                        if qty <= 1e-6 {
                            break;
                        }
                        let room_t = if km > 1e-6 {
                            self.carriers[ci].spare() / km
                        } else {
                            qty
                        };
                        let take = qty.min(room_t);
                        if take <= 1e-6 {
                            continue;
                        }

                        let moved = ship(econ, src, dst, commodity, take);
                        if moved <= 1e-9 {
                            continue;
                        }
                        let c = &mut self.carriers[ci];
                        c.hauled_t += moved;
                        c.lifetime_t += moved;
                        c.worked_t_km += moved * km;
                        c.revenue += moved * km * c.rate * OPERATING_MARGIN;
                        qty -= moved;
                        still_wanted -= moved;
                        let _ = per_tonne;
                    }
                }
            }
        }
    }

    /// Tonnes moved today by everybody.
    pub fn hauled_today(&self) -> f64 {
        self.carriers.iter().map(|c| c.hauled_t).sum()
    }

    /// Tonne-kilometres today.
    pub fn t_km_today(&self) -> f64 {
        self.carriers.iter().map(|c| c.worked_t_km).sum()
    }

    /// **The average length of haul**, which is the single number that
    /// says what kind of freight industry this is. Real UK road freight
    /// comes out at about 94 km.
    pub fn mean_haul_km(&self) -> f64 {
        let t = self.hauled_today();
        if t <= 1e-9 {
            return 0.0;
        }
        self.t_km_today() / t
    }

    /// Driving jobs the industry is holding open.
    pub fn drivers(&self) -> f64 {
        self.carriers.iter().map(|c| c.drivers()).sum()
    }
}

/// Move goods between two markets, through the journal like everything
/// else — so conservation covers a haulier's work the way it covers a
/// farm's.
fn ship(econ: &mut Economy, from: usize, to: usize, c: Commodity, qty: f64) -> f64 {
    let mut left = qty;
    let mut moved = 0.0;

    // **Collect from a store, not off a shop's shelves.**
    //
    // A market's surplus is the whole town's stock above its reserve, but
    // that reserve can perfectly well be sitting in a works while the
    // shop is nearly empty. Taking from whoever happened to hold some
    // cleared the shelves of a town whose books said it had four days of
    // food, and people went hungry with the ledger balanced.
    //
    // So: producers and stores first, and a shop only for what it holds
    // above its own counter cover.
    let mut sources: Vec<usize> = (0..econ.ledger.sites.len())
        .filter(|&s| {
            let site = &econ.ledger.sites[s];
            if site.market != from || econ.ledger.stock(s, c) <= 0.0 {
                return false;
            }
            // **Never collect from a site that consumes the stuff.**
            //
            // A market's surplus is the whole town's holding above its
            // reserve, and that reserve can perfectly well be a
            // merchant's yard while a works is running on what is in its
            // own hopper. Taking from whoever held some backed a lorry up
            // to a cannery and carried off its tinplate: the town still
            // had its four days of steel on paper, the cannery had none,
            // it stopped, and the shortage came out as a famine two
            // commodities downstream.
            //
            // A haulier collects from an output bay or a merchant. It
            // does not empty its customer's raw material store.
            let consumes = site
                .recipe
                .map(|r| {
                    let rec = &crate::econ::RECIPES[r];
                    rec.inputs.iter().any(|&(ic, _)| ic == c)
                        && !rec.outputs.iter().any(|&(oc, _)| oc == c)
                })
                .unwrap_or(false);
            !consumes
        })
        .collect();
    sources.sort_by_key(|&s| {
        let shop = econ.ledger.sites[s].kind == crate::econ::SiteKind::Shop;
        (shop as u8, s)
    });
    // **Deliver where the goods are wanted, not merely where there is
    // room.**
    //
    // Dropping a load wherever a shelf had space put a town's food into a
    // cannery's output store, where households cannot buy it — the stock
    // was in the town, the ledger balanced, and people went hungry
    // anyway. A haulier delivers to a consignee: a shop that sells the
    // stuff, or a works that consumes it.
    let sinks: Vec<usize> = (0..econ.ledger.sites.len())
        .filter(|&s| {
            let site = &econ.ledger.sites[s];
            if site.market != to || site.capacity[c as usize] <= 0.0 {
                return false;
            }
            let sells = site.kind == crate::econ::SiteKind::Shop
                && econ.markets[to].daily_household_demand(c) > 0.0;
            let consumes = site
                .recipe
                .map(|r| {
                    crate::econ::RECIPES[r]
                        .inputs
                        .iter()
                        .any(|&(ic, _)| ic == c)
                })
                .unwrap_or(false);
            sells || consumes
        })
        .collect();
    if sources.is_empty() || sinks.is_empty() {
        return 0.0;
    }

    for &src in sources.iter() {
        if left <= 1e-9 {
            break;
        }
        for &dst in sinks.iter() {
            if left <= 1e-9 {
                break;
            }
            let mut have = econ.ledger.stock(src, c);
            if econ.ledger.sites[src].kind == crate::econ::SiteKind::Shop {
                // What the counter must keep to trade tomorrow.
                let keep = econ.markets[from].daily_household_demand(c) * c.target_cover_days();
                have = (have - keep).max(0.0);
            }
            let room =
                (econ.ledger.sites[dst].capacity[c as usize] - econ.ledger.stock(dst, c)).max(0.0);
            let take = left.min(have).min(room);
            if take <= 1e-9 {
                continue;
            }
            econ.ledger.apply(
                &mut econ.journal,
                Event::Shipped {
                    from: src,
                    to: dst,
                    commodity: c,
                    qty: take,
                },
            );
            left -= take;
            moved += take;
        }
    }
    moved
}
