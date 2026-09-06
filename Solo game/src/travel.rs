//! How a person and their goods actually get from one town to the next.
//!
//! Freight costs answer what a *tonne* costs to move. They say nothing
//! about how a particular man gets forty-nine kilometres over a pass with
//! something to sell at the other end, and that turned out to matter: a
//! trader was quietly shifting twenty-four tonnes because that is a
//! lorry-load, while owning nothing but the clothes he stood in.
//!
//! **What you can carry is a thing you own.** That is the whole idea here.
//! A man on foot carries what a man can carry; a handcart is a season's
//! wages and multiplies it fivefold; a lorry is years of saving and lifts
//! it by three orders of magnitude. The ladder out of poverty is not a
//! bigger purse, it is a better vehicle — and the purse is how you buy one.
//!
//! Every figure here is real, because guessing them is how you end up with
//! a man dragging twenty-four tonnes over a mountain on foot.

use crate::econ::Surface;
use crate::vehicle::Vehicle;

/// What somebody has to move goods with.
///
/// **A modern ladder**, because this is a modern world: it has coal-fired
/// power stations, canneries and forty-four-tonne artics on its trunk
/// roads. It used to run handcart, pack mule, wagon — furniture from a
/// different century that had no business in it.
///
/// Each rung past the first is a real assembly of parts (`vehicle.rs`),
/// and every figure below is computed from those parts rather than typed
/// in here.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Conveyance {
    /// Your own back. What everybody starts with.
    OnFoot,
    /// A bicycle and trailer. A month of savings, and five times what a
    /// man can carry.
    Bicycle,
    /// A second-hand van: the first rung that is a living rather than an
    /// errand, and the first that needs fuel.
    Van,
    /// A rigid box truck at seven and a half tonnes gross.
    BoxTruck,
    /// An artic. Years of saving, and a business rather than a possession.
    Artic,
}

impl Conveyance {
    pub const ALL: [Conveyance; 5] = [
        Conveyance::OnFoot,
        Conveyance::Bicycle,
        Conveyance::Van,
        Conveyance::BoxTruck,
        Conveyance::Artic,
    ];

    /// The machine itself, if there is one.
    pub fn vehicle(self) -> Option<Vehicle> {
        match self {
            Conveyance::OnFoot => None,
            Conveyance::Bicycle => Some(Vehicle::bicycle()),
            Conveyance::Van => Some(Vehicle::van()),
            Conveyance::BoxTruck => Some(Vehicle::box_truck()),
            Conveyance::Artic => Some(Vehicle::artic()),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Conveyance::OnFoot => "on foot",
            Conveyance::Bicycle => "a bicycle and trailer",
            Conveyance::Van => "a second-hand van",
            Conveyance::BoxTruck => "a box truck",
            Conveyance::Artic => "an artic",
        }
    }

    /// What it will carry, in tonnes.
    ///
    /// **Computed from the cargo bays**, not typed in. A man on foot
    /// carries an infantryman's marching load, which is about 35 kg and
    /// is considered heavy.
    pub fn payload(self) -> f64 {
        match self.vehicle() {
            Some(v) => v.payload_t(),
            None => 0.035,
        }
    }

    /// Kilometres covered in a day over a given surface.
    ///
    /// A driver's legal day is about nine hours at the wheel, so distance
    /// is cruising speed times that — and cruising speed comes off the
    /// engine against the loaded weight. On foot it is a march: infantry
    /// make 25-32 km a day and can keep it up.
    ///
    /// Returns `None` where the thing cannot go at all: nothing on wheels
    /// crosses open country, and only a boat crosses water.
    pub fn km_per_day(self, surface: Surface) -> Option<f64> {
        use Surface::*;
        // A person on a water route is a passenger on somebody's boat,
        // which is fast and priced as freight rather than as effort.
        if surface == Water {
            return Some(180.0); // a coastal steamer at 8-10 knots
        }
        let Some(v) = self.vehicle() else {
            // Legs. A road helps; it does not transform. Infantry make
            // 25-32 km a day and can keep it up.
            let base = 28.0;
            return Some(match surface {
                Highway => base * 1.05,
                Road => base,
                Track => base * 0.85,
                Open => base * 0.55,
                Water => base,
            });
        };
        // **An engine gains enormously from a made road and loses
        // enormously without one.** This is why the state builds them.
        let factor = match surface {
            Highway => 1.10,
            Road => 1.00,
            Track => 0.30,
            Open => return None,
            Water => 1.0,
        };
        // **Nine hours is the legal maximum; seven is a working day.**
        // The rest goes on loading, queueing, town speeds and the breaks
        // the law also requires, which is why a real artic covers 550-700
        // km rather than the 900 its cruising speed would suggest. Muscle
        // gets less again: a cyclist with a loaded trailer does about six
        // hours before it stops being worth it.
        let hours = if v.power_kw() > 0.0 { 7.0 } else { 6.0 };
        Some(v.cruise_kmh() * hours * factor)
    }

    /// What it costs to buy, in days of an unskilled wage.
    ///
    /// The sum of its parts, literally. The absolute prices are
    /// meaningless across centuries; the ratios to earnings are not.
    pub fn price_in_wage_days(self) -> f64 {
        self.vehicle().map_or(0.0, |v| v.price_in_wage_days())
    }

    /// What it costs to keep and run for a day, in days of an unskilled
    /// wage.
    ///
    /// **Fuel is the running cost and it comes off the parts**: litres per
    /// hundred kilometres against the distance a day covers. Standing
    /// costs — tax, insurance, the yard it sits in, the maintenance it
    /// needs whether or not it turns a wheel — run at roughly a fifth of
    /// that, and a bicycle has none worth counting.
    pub fn upkeep_in_wage_days(self, travelling: bool) -> f64 {
        let Some(v) = self.vehicle() else { return 0.0 };
        if v.power_kw() <= 0.0 {
            return 0.0; // muscle costs nothing but the food already eaten
        }
        // A litre of fuel is about a fiftieth of an unskilled day's pay in
        // a developed economy (roughly £1.50 against £120 take-home).
        const LITRE_IN_WAGE_DAYS: f64 = 0.02;
        let km = v.cruise_kmh() * 7.0;
        let fuel = v.litres_per_100km() / 100.0 * km * LITRE_IN_WAGE_DAYS;
        if travelling {
            fuel
        } else {
            fuel * 0.2
        }
    }

    /// What this is worth to somebody working `surface`, per day, net of
    /// what it costs to keep.
    ///
    /// **There is no ladder.** A pack mule carries 90 kg and a handcart
    /// carries 150, so on a made road the barrow is simply the better
    /// machine and the mule is a waste of four months' wages. The mule
    /// earns its keep exactly where wheels stop earning theirs — a track,
    /// a hillside, open country — and that falls out of the real payloads
    /// and speeds rather than being asserted.
    ///
    /// Capability is tonne-kilometres a day, valued at what freight
    /// fetches; upkeep is subtracted because an animal eats whether it
    /// works or not.
    /// What this is worth per day to somebody with `cargo_money` to fill
    /// it, working `surface`.
    ///
    /// **An empty lorry earns nothing.** Capacity you cannot afford to
    /// load is not capacity, it is a standing cost — so the payload that
    /// counts is the lesser of what the vehicle holds and what the purse
    /// will buy.
    pub fn worth_per_day(
        self,
        surface: Surface,
        wage: f64,
        freight_rate: f64,
        cargo_money: f64,
        goods_price: f64,
        // Share of days actually spent carrying your own cargo. A vehicle
        // earns only on those; it eats on all of them.
        usage: f64,
    ) -> f64 {
        let Some(speed) = self.km_per_day(surface) else {
            return f64::NEG_INFINITY; // cannot go at all
        };
        let can_fill = (cargo_money / goods_price.max(1e-9)).min(self.payload());
        // **The keep is paid every day; the earning is not.**
        //
        // A horse eats on the days it stands in the field, and most of a
        // haulier's days are spent driving somebody else's lorry for a
        // wage that owes nothing to what he owns. Costing only the
        // travelling upkeep had him buy a wagon in year three and spend
        // the next seven years feeding it: ten years' work, and twenty-four
        // days of food at the end of it.
        let usage = usage.clamp(0.0, 1.0);
        let keep = self.upkeep_in_wage_days(false)
            + self.upkeep_in_wage_days(true) * usage;
        can_fill * speed * freight_rate * usage - keep * wage
    }

    /// The best thing `budget` will buy for work over `surface`, if it
    /// beats what is already owned.
    ///
    /// **Judged on what is left after buying it.** A man who spends every
    /// penny on a lorry owns a lorry and no cargo, and a haulier's wage
    /// does not depend on what he owns — so he earns exactly what he did
    /// before, minus the upkeep. Ten years of saving went that way once,
    /// and the year after buying it he was poorer than the year before.
    /// It is the classic way an owner-driver goes bust and it should be
    /// possible, but not compulsory.
    pub fn best_upgrade(
        current: Conveyance,
        budget: f64,
        wage: f64,
        surface: Surface,
        freight_rate: f64,
        goods_price: f64,
        usage: f64,
    ) -> Option<(Conveyance, f64)> {
        let have =
            current.worth_per_day(surface, wage, freight_rate, budget, goods_price, usage);
        let mut best: Option<(Conveyance, f64, f64)> = None;
        for &c in Conveyance::ALL.iter() {
            // **Never pay to downgrade.** Scoring on the load he can
            // afford to fill made a cheaper vehicle look better once his
            // savings had gone into a dearer one, so he bought a wagon in
            // year four and a handcart in year five, and again, and again.
            // Nobody sells the wagon to buy a barrow.
            if c.payload() <= current.payload() {
                continue;
            }
            let price = c.price_in_wage_days() * wage;
            if price > budget {
                continue;
            }
            let left = budget - price;
            let worth = c.worth_per_day(surface, wage, freight_rate, left, goods_price, usage);
            if worth <= have {
                continue;
            }
            if best.is_none_or(|(_, w, _)| worth > w) {
                best = Some((c, worth, price));
            }
        }
        best.map(|(c, _, price)| (c, price))
    }
}
