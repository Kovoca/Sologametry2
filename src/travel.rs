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

/// What somebody has to move goods with.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Conveyance {
    /// Your own back. What everybody starts with.
    OnFoot,
    /// A barrow. The cheapest thing that multiplies a man.
    Handcart,
    /// A donkey or mule. Eats whether it works or not.
    PackAnimal,
    /// A cart and something to pull it.
    Wagon,
    /// Diesel, and the first conveyance that is a business rather than a
    /// possession.
    Lorry,
}

impl Conveyance {
    pub const ALL: [Conveyance; 5] = [
        Conveyance::OnFoot,
        Conveyance::Handcart,
        Conveyance::PackAnimal,
        Conveyance::Wagon,
        Conveyance::Lorry,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Conveyance::OnFoot => "on foot",
            Conveyance::Handcart => "a handcart",
            Conveyance::PackAnimal => "a pack mule",
            Conveyance::Wagon => "a wagon",
            Conveyance::Lorry => "a lorry",
        }
    }

    /// What it will carry, in tonnes *(all real)*.
    ///
    /// - **On foot**: an infantryman's marching load is about 30 kg and
    ///   that is considered heavy. Call it 35 kg for a man who has chosen
    ///   the load himself and will put it down when he likes.
    /// - **Handcart**: 150 kg is an ordinary barrow load.
    /// - **Pack animal**: a mule carries about 20% of its body weight, so
    ///   70-90 kg.
    /// - **Wagon**: a one-horse cart takes half a tonne to a tonne; a pair
    ///   of oxen rather more but slower.
    /// - **Lorry**: 44 t gross on European roads leaves ~26 t of payload;
    ///   24 is the figure hauliers quote.
    pub fn payload(self) -> f64 {
        match self {
            Conveyance::OnFoot => 0.035,
            Conveyance::Handcart => 0.15,
            Conveyance::PackAnimal => 0.09,
            Conveyance::Wagon => 0.80,
            Conveyance::Lorry => 24.0,
        }
    }

    /// Kilometres covered in a day over a given surface *(all real)*.
    ///
    /// - **On foot**: infantry march 25-32 km a day and can keep it up.
    ///   Laden and over broken ground, less.
    /// - **Wheels without an engine** are barely faster than a man and are
    ///   much more hurt by a bad surface — an ox cart makes 15-20 km.
    /// - **A lorry** does 500-600 km in a driver's legal day on a good
    ///   road, and a fraction of that on a track. This spread is the whole
    ///   argument for building roads.
    ///
    /// Returns `None` where the thing simply cannot go: a lorry does not
    /// cross open country, and nothing but a boat crosses water.
    pub fn km_per_day(self, surface: Surface) -> Option<f64> {
        use Conveyance::*;
        use Surface::*;
        // Nobody walks or drives across open water. A person travelling a
        // water route is a passenger on somebody's boat, which is fast and
        // is priced as freight rather than as effort.
        if surface == Water {
            return Some(180.0); // a coastal steamer at 8-10 knots
        }
        let base = match self {
            OnFoot => 28.0,
            Handcart => 22.0,
            PackAnimal => 28.0,
            Wagon => 32.0,
            Lorry => 550.0,
        };
        let factor = match (self, surface) {
            // An engine gains enormously from a made road and loses
            // enormously without one. This is why the state builds them.
            (Lorry, Highway) => 1.10,
            (Lorry, Road) => 1.00,
            (Lorry, Track) => 0.30,
            (Lorry, Open) => return None,
            // Legs care much less. A road helps; it does not transform.
            (_, Highway) => 1.05,
            (_, Road) => 1.00,
            (_, Track) => 0.85,
            (OnFoot, Open) => 0.55,
            (PackAnimal, Open) => 0.55,
            // Wheels off a road are close to useless.
            (_, Open) => 0.30,
            (_, Water) => 1.0,
        };
        Some(base * factor)
    }

    /// What it costs to buy, in days of an unskilled wage *(all real, as
    /// ratios to earnings — the absolute prices are meaningless across
    /// centuries, the ratios are not)*.
    ///
    /// - A **barrow** is a few weeks' wages anywhere in history.
    /// - A **donkey** runs to two to six months of unskilled earnings in
    ///   the economies that still buy them.
    /// - A **cart and draught animal** is most of a year.
    /// - A **used lorry** is £20-30k against a UK median wage of ~£35k, so
    ///   roughly eight months of *median* earnings — considerably more of
    ///   an unskilled one, and it is the jump that most people never make.
    pub fn price_in_wage_days(self) -> f64 {
        match self {
            Conveyance::OnFoot => 0.0,
            Conveyance::Handcart => 25.0,
            Conveyance::PackAnimal => 120.0,
            Conveyance::Wagon => 300.0,
            Conveyance::Lorry => 700.0,
        }
    }

    /// What it costs to keep and run for a day on the road, again in days
    /// of an unskilled wage.
    ///
    /// **An animal eats whether it works or not**, which is the thing that
    /// ruins people. A lorry does not, but a 44-tonne artic costs about
    /// £1.20 a kilometre all-in against a driver's £150 a day, so a full
    /// day's running is several times the wage of the man steering it.
    pub fn upkeep_in_wage_days(self, travelling: bool) -> f64 {
        match self {
            Conveyance::OnFoot => 0.0,
            Conveyance::Handcart => 0.0,
            // Fodder every day of its life.
            Conveyance::PackAnimal => 0.15,
            Conveyance::Wagon => 0.35,
            Conveyance::Lorry => {
                if travelling {
                    4.0
                } else {
                    0.2
                }
            }
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
