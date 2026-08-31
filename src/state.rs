//! What the state raises, what it spends it on, and who that employs.
//!
//! Spec C.1 and C.2. The labour market had a hole in it the size of the
//! public sector: **real government employment is 14-21% of the
//! workforce** — UK 17%, US 14%, France 21% — and none of it existed
//! here. A nation's people could work a farm, a mill, a cannery, a mine or
//! a shop, which is about a tenth of what people actually do, and there
//! was nowhere at all for the other nine tenths to go.
//!
//! Nothing here is a subsidy or a modifier. **A teacher is a job somebody
//! holds**, paid out of a budget line that comes out of a tax take that
//! comes out of the economy, and every one of those numbers is real.

use crate::econ::Economy;

/// **What a state can actually take, as a share of its economy** *(spec
/// C.1, real figures)*.
///
/// | state | take |
/// |---|---|
/// | high-capacity developed | 35-50% |
/// | middle-income | 20-30% |
/// | low-capacity, weak control | 10-18% |
///
/// A state with weak control cannot tax what it cannot reach, which is a
/// real feedback loop and the reason weak states stay weak.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Capacity {
    Developed,
    Middling,
    Weak,
}

impl Capacity {
    pub fn tax_take(self) -> f64 {
        match self {
            Capacity::Developed => 0.40,
            Capacity::Middling => 0.25,
            Capacity::Weak => 0.14,
        }
    }

    /// What it manages to collect of what it is owed. This is the control
    /// axis showing up as money.
    pub fn collection(self) -> f64 {
        match self {
            Capacity::Developed => 0.95,
            Capacity::Middling => 0.80,
            Capacity::Weak => 0.55,
        }
    }
}

/// A line in the budget, and what it buys in people.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Service {
    /// Schools. Real: 4-6% of the economy.
    Education,
    /// Hospitals, clinics, care. Real: 5-10%, the largest single line in
    /// most developed states.
    Health,
    /// Civil service, courts, tax collection, local government. Real:
    /// 2-5%, and it is what makes the *other* lines collectable —
    /// spending on administration is what buys the control that raises
    /// the revenue.
    Administration,
    /// Police and fire. Real: about 1%.
    Safety,
    /// Real: 1-4% in peacetime, 10-40% at war, and it ratchets.
    Defence,
    /// **Family support** — subsidised childcare, allowances, parental
    /// leave. Real spending runs from **0.6% of GDP in the United States
    /// to about 4% in France**, with an OECD average of 2%; Denmark,
    /// France, Hungary, Sweden and the UK are all above 3.5%, while Japan,
    /// Korea, Spain and the US are under 1.5%.
    ///
    /// **This is the line that decides whether a below-replacement
    /// fertility rate is destiny.** A state that leaves a nursery place at
    /// 65% of a wage gets the maternal employment and the birth rate that
    /// implies; one that caps it — Sweden holds parents to about 3% of
    /// income — gets both back.
    ///
    /// The honest caveat, and it matters: **spending does not simply buy
    /// births.** OECD fertility fell from 1.8 to 1.7 between 2009 and 2017
    /// across countries that were spending heavily, and Korea has cheap
    /// childcare and the lowest fertility on earth. Housing, hours and
    /// what is expected of a parent all bear on it. What family spending
    /// reliably buys is that **a parent can work**; the birth rate
    /// responds, but weakly.
    Family,
}

impl Service {
    pub const ALL: [Service; 6] = [
        Service::Education,
        Service::Health,
        Service::Administration,
        Service::Safety,
        Service::Defence,
        Service::Family,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Service::Education => "education",
            Service::Health => "health",
            Service::Administration => "administration",
            Service::Safety => "safety",
            Service::Defence => "defence",
            Service::Family => "family support",
        }
    }

    /// Share of the economy a peacetime state spends here *(spec C.2)*.
    pub fn peacetime_share(self) -> f64 {
        match self {
            Service::Education => 0.05,
            Service::Health => 0.075,
            Service::Administration => 0.035,
            Service::Safety => 0.010,
            Service::Defence => 0.020,
            // OECD average 2% of the economy; France ~4%, the US 0.6%.
            Service::Family => 0.025,
        }
    }

    /// **How many people it employs per head of population** *(real)*.
    ///
    /// These are the figures that make the public sector a sixth of the
    /// workforce rather than a line in an accounts sheet:
    ///
    /// | | staff per head | source |
    /// |---|---|---|
    /// | health | 1 in 45 | NHS 1.5M of 67M, plus social care |
    /// | education | 1 in 45 | ~1.5M school staff of 67M |
    /// | administration | 1 in 45 | civil service and local government |
    /// | safety | 1 in 350 | ~150k police of 67M, plus fire |
    /// | defence | 1 in 450 | ~150k regulars of 67M |
    ///
    /// They sum to about 8.7% of the population, which against a workforce
    /// of roughly half the population is the **17%** the UK actually runs.
    pub fn staff_per_head(self) -> f64 {
        match self {
            Service::Education => 1.0 / 45.0,
            Service::Health => 1.0 / 45.0,
            Service::Administration => 1.0 / 45.0,
            Service::Safety => 1.0 / 350.0,
            Service::Defence => 1.0 / 450.0,
            // Nursery and childcare staff. Ratios of one adult to three
            // under-twos are why this employs so many for so few children.
            Service::Family => 1.0 / 200.0,
        }
    }
}

/// What a nation's state is doing.
pub struct Government {
    pub capacity: Capacity,
    /// Revenue a day, in the economy's currency.
    pub revenue: f64,
    /// What each line is funded at, 0 to 1 of what it wants. **Under-fund
    /// a line and it employs fewer people**, which is the whole point of
    /// the budget competing with itself.
    pub funded: [f64; 6],
    /// Public posts in each market, summed over the services.
    pub posts: Vec<f64>,
}

impl Government {
    /// **Raise the revenue, then spend it.**
    ///
    /// The budget is a fixed sum allocated across lines that compete: guns
    /// against butter is not a slogan, it is the constraint. Where revenue
    /// falls short of what the lines want, every line is cut in
    /// proportion — which is not what real states do, but it is honest
    /// until C.3's priorities are built.
    pub fn govern(econ: &Economy, capacity: Capacity) -> Government {
        // Taxable activity: what the economy is worth in a day. Household
        // spending stands in for it, which understates an economy with a
        // lot of intermediate trade and is the right order of magnitude.
        let daily_economy: f64 = econ
            .markets
            .iter()
            .map(|m| {
                crate::econ::Commodity::ALL
                    .iter()
                    .map(|&c| m.daily_household_demand(c) * c.base_cost())
                    .sum::<f64>()
            })
            .sum();

        let revenue = daily_economy * capacity.tax_take() * capacity.collection();
        let wanted: f64 = Service::ALL
            .iter()
            .map(|s| daily_economy * s.peacetime_share())
            .sum();
        let cover = if wanted > 1e-9 {
            (revenue / wanted).min(1.0)
        } else {
            1.0
        };

        let mut funded = [0.0f64; 6];
        for (i, _s) in Service::ALL.iter().enumerate() {
            funded[i] = cover;
        }

        // Posts follow the population and the funding together: a service
        // funded at four fifths employs four fifths of the staff.
        let posts = econ
            .markets
            .iter()
            .map(|m| {
                Service::ALL
                    .iter()
                    .enumerate()
                    .map(|(i, s)| m.population * s.staff_per_head() * funded[i])
                    .sum::<f64>()
            })
            .collect();

        Government {
            capacity,
            revenue,
            funded,
            posts,
        }
    }

    /// **What a family actually pays for childcare**, after what the state
    /// carries.
    ///
    /// Real: an unsupported nursery place is 65% of a median take-home
    /// wage in England; Sweden caps what a parent pays at about 3% of
    /// income. A fully funded family policy therefore takes most of it
    /// away, and that is the difference between a parent working and not.
    pub fn childcare_borne_by_parents(&self) -> f64 {
        let i = Service::ALL
            .iter()
            .position(|&s| s == Service::Family)
            .unwrap_or(0);
        // A fully funded policy leaves the parent about a sixth of it,
        // which is roughly where the Nordic countries actually land.
        1.0 - 0.84 * self.funded[i]
    }

    /// Public posts in one market.
    pub fn posts_in(&self, market: usize) -> f64 {
        self.posts.get(market).copied().unwrap_or(0.0)
    }

    /// Public posts in one market for one service.
    pub fn posts_for(&self, econ: &Economy, market: usize, service: Service) -> f64 {
        let i = Service::ALL.iter().position(|&s| s == service).unwrap_or(0);
        econ.markets
            .get(market)
            .map(|m| m.population * service.staff_per_head() * self.funded[i])
            .unwrap_or(0.0)
    }

    /// **What share of the working-age population this employs.**
    ///
    /// Real: 17% in Britain, 14% in the United States, 21% in France. If
    /// this comes out at a couple of percent the staffing ratios are
    /// wrong; if it comes out at half the country, the state is not a
    /// state, it is the economy.
    pub fn share_of_workforce(&self, econ: &Economy) -> f64 {
        /// Roughly half a population is in work at any time — real
        /// participation runs 45-55% of the whole population once the
        /// young, the old and the unwaged are taken out.
        const IN_WORK: f64 = 0.5;
        let people: f64 = econ.markets.iter().map(|m| m.population).sum();
        let posts: f64 = self.posts.iter().sum();
        if people <= 0.0 {
            return 0.0;
        }
        posts / (people * IN_WORK)
    }
}
