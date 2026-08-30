//! Buildings built out of fixtures, and the jobs that fall out of them.
//!
//! The same idea as `vehicle.rs`, applied where the design doc says it
//! should be: *"a tile-layered data model, generalizing CDDA's vehicle-part
//! system to buildings."* A vehicle is frames and engines and cargo bays; a
//! shop is shelves and tills and a loading dock. Neither has its
//! capabilities typed in.
//!
//! **The point here is the staff.** A shop does not employ people because
//! a table says retail is a tenth of the workforce. It employs them
//! because somebody has to work each till, fill each shelf and unload each
//! lorry, and how many of each it has depends on how much it sells. Take
//! the tills out and the cashiers go with them.
//!
//! That is why this exists now: the economy modelled farms, mills,
//! canneries and mines — the small half of employment — and gave a nation
//! of sixteen million people shops that employed **nobody at all**. Retail
//! is around a tenth of all jobs in a developed economy against about 1.5%
//! in food manufacturing, so the half that was missing was the big one.
//!
//! Not yet built: the walkable tile-by-tile interior, the wall and floor
//! layers, and the electrical and water nodes hung off them. Fixtures
//! first, the same way parts came before vehicle interiors.

/// A piece of fitted-out furniture that does a job.
///
/// Deliberately coarse — a bank of shelving, not each shelf. It gets finer
/// when there is a reason for it to.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Fixture {
    /// A checkout. Somebody has to ring the goods through.
    Till,
    /// A bay of shelving on the shop floor. Somebody has to keep it full.
    Shelving,
    /// Racking in the stockroom out the back.
    StockRack,
    /// A dock for the lorries. Somebody has to unload them.
    LoadingBay,
    /// A served counter — bakery, deli, fishmonger.
    Counter,
}

impl Fixture {
    /// **People it takes to work it**, as full-time equivalents covering
    /// the hours the shop is open *(real, near enough)*.
    ///
    /// A checkout needs somebody on it whenever the doors are open, which
    /// is more than one person's week. Shelf-filling is the biggest single
    /// job in a supermarket and is done by fewer people spread over more
    /// stock. A loading dock is a couple of hands.
    ///
    /// Sanity check against a real large supermarket: about 75 t of goods
    /// a day and roughly 300 staff. The fixtures below give something near
    /// 190, the shortfall being management, cleaning, security and the
    /// online picking that none of these fixtures represent.
    pub fn staff(self) -> f64 {
        match self {
            Fixture::Till => 1.4,
            Fixture::Shelving => 0.25,
            Fixture::StockRack => 0.04,
            Fixture::LoadingBay => 1.2,
            Fixture::Counter => 1.6,
        }
    }

    /// Tonnes a day it can put through, where that is what limits it.
    pub fn throughput_t(self) -> f64 {
        match self {
            // A till serves ~25 customers an hour and a basket is about
            // 7 kg, so 175 kg an hour and something over 2 t across a
            // trading day.
            Fixture::Till => 2.5,
            Fixture::LoadingBay => 40.0,
            Fixture::Counter => 0.4,
            _ => 0.0,
        }
    }

    /// Tonnes it will hold, where holding is what it is for.
    pub fn holds_t(self) -> f64 {
        match self {
            Fixture::Shelving => 0.4,
            Fixture::StockRack => 3.0,
            _ => 0.0,
        }
    }

    /// Floor it takes up, in square metres.
    pub fn floor_m2(self) -> f64 {
        match self {
            Fixture::Till => 6.0,
            Fixture::Shelving => 4.0,
            Fixture::StockRack => 3.0,
            Fixture::LoadingBay => 40.0,
            Fixture::Counter => 12.0,
        }
    }

    /// What fitting it out costs, in days of an unskilled wage.
    pub fn price_in_wage_days(self) -> f64 {
        match self {
            Fixture::Till => 20.0,
            Fixture::Shelving => 4.0,
            Fixture::StockRack => 3.0,
            Fixture::LoadingBay => 60.0,
            Fixture::Counter => 45.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Fixture::Till => "checkout",
            Fixture::Shelving => "shelving",
            Fixture::StockRack => "stockroom racking",
            Fixture::LoadingBay => "loading bay",
            Fixture::Counter => "served counter",
        }
    }
}

/// A building: a count of each fixture in it.
#[derive(Clone, Debug, Default)]
pub struct Building {
    pub fixtures: Vec<(Fixture, f64)>,
}

impl Building {
    /// Fit out a shop to sell `tonnes_per_day`, holding `cover_days` of it.
    ///
    /// **Everything is sized off the trade it does.** A village shop and a
    /// city supermarket are the same arrangement of the same furniture at
    /// different counts, which is why one employs four people and the
    /// other three hundred without either number being written down
    /// anywhere.
    pub fn shop(tonnes_per_day: f64, cover_days: f64) -> Self {
        let t = tonnes_per_day.max(0.0);
        let held = t * cover_days.max(0.0);
        // A third of the stock sits out on the shop floor and the rest is
        // out the back, which is roughly how a supermarket splits.
        Building {
            fixtures: vec![
                (Fixture::Till, (t / Fixture::Till.throughput_t()).ceil()),
                (
                    Fixture::Shelving,
                    (held * 0.33 / Fixture::Shelving.holds_t()).ceil(),
                ),
                (
                    Fixture::StockRack,
                    (held * 0.67 / Fixture::StockRack.holds_t()).ceil(),
                ),
                (
                    Fixture::LoadingBay,
                    (t / Fixture::LoadingBay.throughput_t()).ceil().max(1.0),
                ),
                // One served counter per few hundred tonnes a day: the
                // bakery, the deli, the fish slab.
                (Fixture::Counter, (t / 250.0).ceil().max(1.0)),
            ],
        }
    }

    fn sum(&self, f: impl Fn(Fixture) -> f64) -> f64 {
        self.fixtures.iter().map(|&(fx, n)| f(fx) * n).sum()
    }

    /// **How many people work here.** The whole reason this module exists.
    pub fn staff(&self) -> f64 {
        self.sum(|f| f.staff())
    }

    /// Staff doing one particular job, so a person can look for that work.
    pub fn staff_at(&self, fixture: Fixture) -> f64 {
        self.fixtures
            .iter()
            .filter(|&&(fx, _)| fx == fixture)
            .map(|&(fx, n)| fx.staff() * n)
            .sum()
    }

    pub fn floor_m2(&self) -> f64 {
        self.sum(|f| f.floor_m2())
    }

    pub fn holds_t(&self) -> f64 {
        self.sum(|f| f.holds_t())
    }

    pub fn price_in_wage_days(&self) -> f64 {
        self.sum(|f| f.price_in_wage_days())
    }
}
