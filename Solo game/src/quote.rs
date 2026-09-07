//! **One answer to "what would it cost to get it there?"**
//!
//! Four things in this model had to decide whether a haul was worth
//! making, and they were asking different questions and getting different
//! numbers. `trade` priced a haul at `Route::freight_cost` — the direct
//! link — while a delivery was charged `freight_between`, the cheapest
//! *path*. Wherever going round was cheaper than going straight the two
//! disagreed permanently, a price gap opened that nothing could close, and
//! goods chased it for ever.
//!
//! So there is one quotation function and everybody uses it: pairwise
//! trade, the hauliers, a consignment being raised, and the price model.
//!
//! ### And an accounting cost is not a trade signal
//!
//! The deeper error, and the reason this module exists rather than a
//! tidied-up `freight_between`. **Six different numbers had been collapsed
//! into two**, and the ones that matter most were the ones missing:
//!
//! | | what it is for |
//! |---|---|
//! | inventory cost basis | what the stock on hand cost — historical |
//! | contract price | what was paid to the supplier |
//! | inbound freight | what was paid to the carrier |
//! | landed inventory cost | purchase + freight + losses, per tonne held |
//! | **marginal replacement quote** | **what the next tonne would cost, now** |
//! | market price | what it actually clears at |
//! | scarcity premium | what shortage adds on top |
//!
//! A works consumes the **inventory cost basis** of what is in its yard. A
//! haulier deciding where to send a lorry, and a market forming a price,
//! must use the **marginal replacement quote** — what it would cost to get
//! another tonne here today — and never the weighted-average historical
//! cost of what happens to be in the destination's warehouse. Using the
//! second for the first is what let a town be made to look dear by the
//! carriage that had already got its goods there.

use crate::econ::Commodity;

/// **What it costs to get the next tonne from one market to another, now.**
#[derive(Clone, Debug, PartialEq)]
pub struct Quote {
    pub from: usize,
    pub to: usize,
    /// Road distance along the route that would actually be taken.
    pub km: f64,
    /// Nights the load spends on the road.
    pub days: u64,
    /// Tonnes a day the tightest link on that path will take. **A quote
    /// against a saturated route is not a quote**, which is why a spread
    /// wider than the carriage is legitimate when this is nearly used up.
    pub capacity: f64,
    /// Carriage, per tonne.
    pub freight: f64,
    /// The share of the load expected not to arrive — spoilage over the
    /// days it is travelling. Real for anything perishable and zero for
    /// steel.
    pub loss: f64,
    /// Duty per tonne, ad valorem at the border. Zero within a nation.
    pub tariff: f64,
}

impl Quote {
    /// **The carriage on a tonne**: everything that is paid to somebody
    /// other than the seller.
    pub fn carriage(&self) -> f64 {
        self.freight + self.tariff
    }

    /// **What a tonne costs delivered**, given what it is worth where it
    /// stands.
    ///
    /// The loss allowance is grossed up rather than added: to land one
    /// tonne through a route that loses a tenth you have to despatch one
    /// and a ninth, and pay carriage on all of it.
    pub fn delivered(&self, ex_works: f64) -> f64 {
        let survives = (1.0 - self.loss).max(0.05);
        (ex_works + self.carriage()) / survives
    }

    /// A quote for goods that are already where they are wanted.
    pub fn here(m: usize) -> Quote {
        Quote {
            from: m,
            to: m,
            km: 0.0,
            days: 0,
            capacity: f64::INFINITY,
            freight: 0.0,
            loss: 0.0,
            tariff: 0.0,
        }
    }
}

/// **All-pairs carriage, worked out once and read many times.**
///
/// A Dijkstra per market rather than per pair, and the whole table is
/// rebuilt when the network changes rather than on every enquiry — the
/// price pass alone asks for it thousands of times a day.
#[derive(Clone, Debug, Default)]
pub struct Routing {
    pub n: usize,
    /// Cheapest carriage, `n x n`, `INFINITY` where nothing connects.
    freight: Vec<f64>,
    /// Road distance along that same cheapest-carriage path — **not the
    /// shortest distance**, which is a different route and a different
    /// answer. A haulier goes the cheap way and it takes as long as it
    /// takes.
    km: Vec<f64>,
    /// The tightest link along it, in tonnes a day.
    capacity: Vec<f64>,
}

impl Routing {
    pub fn empty(n: usize) -> Routing {
        Routing {
            n,
            freight: vec![f64::INFINITY; n * n],
            km: vec![f64::INFINITY; n * n],
            capacity: vec![0.0; n * n],
        }
    }

    /// Build from the open routes. `edges` yields `(a, b, freight, km,
    /// capacity)` for every usable link.
    pub fn build(n: usize, edges: &[(usize, usize, f64, f64, f64)]) -> Routing {
        let mut r = Routing::empty(n);
        // Adjacency, both ways: a road is a road in both directions.
        let mut adj: Vec<Vec<(usize, f64, f64, f64)>> = vec![Vec::new(); n];
        for &(a, b, f, km, cap) in edges {
            if a < n && b < n {
                adj[a].push((b, f, km, cap));
                adj[b].push((a, f, km, cap));
            }
        }
        for src in 0..n {
            let mut best = vec![f64::INFINITY; n];
            let mut dist = vec![f64::INFINITY; n];
            let mut tight = vec![0.0f64; n];
            let mut done = vec![false; n];
            best[src] = 0.0;
            dist[src] = 0.0;
            tight[src] = f64::INFINITY;
            loop {
                let mut here = None;
                let mut lowest = f64::INFINITY;
                for m in 0..n {
                    if !done[m] && best[m] < lowest {
                        lowest = best[m];
                        here = Some(m);
                    }
                }
                let Some(here) = here else { break };
                done[here] = true;
                for &(next, f, km, cap) in &adj[here] {
                    let through = best[here] + f;
                    if through < best[next] {
                        best[next] = through;
                        dist[next] = dist[here] + km;
                        tight[next] = tight[here].min(cap);
                    }
                }
            }
            for m in 0..n {
                r.freight[src * n + m] = best[m];
                r.km[src * n + m] = dist[m];
                r.capacity[src * n + m] = tight[m];
            }
        }
        r
    }

    pub fn freight(&self, from: usize, to: usize) -> f64 {
        if from == to {
            return 0.0;
        }
        self.freight
            .get(from * self.n + to)
            .copied()
            .unwrap_or(f64::INFINITY)
    }

    pub fn km(&self, from: usize, to: usize) -> f64 {
        if from == to {
            return 0.0;
        }
        self.km.get(from * self.n + to).copied().unwrap_or(f64::INFINITY)
    }

    pub fn capacity(&self, from: usize, to: usize) -> f64 {
        if from == to {
            return f64::INFINITY;
        }
        self.capacity
            .get(from * self.n + to)
            .copied()
            .unwrap_or(0.0)
    }

    pub fn reaches(&self, from: usize, to: usize) -> bool {
        self.freight(from, to).is_finite()
    }

    /// **The one quote.** `None` when nothing connects the two, which is
    /// the honest answer for an island or a town the roads never reached —
    /// and is why cutting a road starves a place rather than making it
    /// expensive.
    pub fn quote(&self, from: usize, to: usize, c: Commodity, duty: f64) -> Option<Quote> {
        if from == to {
            return Some(Quote::here(from));
        }
        let freight = self.freight(from, to);
        if !freight.is_finite() {
            return None;
        }
        let km = self.km(from, to);
        let days = crate::shipment::days_on_the_road(km);
        // **What rots on the way is a property of the goods and the
        // journey**, and nothing here invents a rate: it is the same
        // spoilage the model already applies to a store, over the days the
        // load is actually travelling.
        let per_day = c.spoilage_per_day(c.needs_cold());
        let loss = 1.0 - (1.0 - per_day).powi(days as i32);
        Some(Quote {
            from,
            to,
            km,
            days,
            capacity: self.capacity(from, to),
            freight,
            loss: loss.clamp(0.0, 0.95),
            tariff: duty,
        })
    }
}
