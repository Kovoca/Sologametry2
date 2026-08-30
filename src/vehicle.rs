//! Vehicles built out of parts, and what falls out of the parts.
//!
//! Per the design doc: *"Vehicles (and later, spaceships) have interiors
//! and individual parts, CDDA-style."* This is the first step of that —
//! not the tile-by-tile interior yet, but the principle that matters
//! underneath it: **a vehicle's capabilities are not typed in, they are
//! computed from what it is made of.**
//!
//! So a van does not carry 1.2 tonnes because a table says so. It carries
//! what its cargo bays hold, up to what its frame and axles will bear; it
//! goes as fast as its engine can push its gross weight; it drinks fuel in
//! proportion to that weight; and it costs what its parts cost. Bolt a
//! bigger engine on and it climbs better and drinks more, because both of
//! those read the same number.
//!
//! That matters beyond tidiness. Once parts are individual they can be
//! damaged, removed, salvaged and improvised, which is the whole point of
//! the CDDA model and the thing a tier list can never do.
//!
//! **This is a modern world.** It has coal-fired power stations, canneries
//! and forty-four-tonne artics on its trunk roads, so the ladder a poor
//! man climbs runs bicycle, second-hand van, box truck, artic — not
//! handcart and pack mule. Those belonged to a different century and were
//! simply the wrong furniture.

/// One component of a vehicle.
///
/// Deliberately coarse for now — a frame section, not every bolt. The
/// finer breakdown (individual panels, lights, seats, the interior you can
/// walk about in) is what this grows into.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Part {
    /// Structural section. Carries load and holds everything else on.
    /// A lorry chassis rail is not a bicycle tube, so it matters which.
    Frame { heavy: bool },
    /// Diesel or petrol engine, rated in kilowatts.
    Engine(u32),
    /// One wheel, sized for the class of vehicle it is under.
    Wheel { heavy: bool },
    /// Somewhere to put cargo, rated in kilograms it will take.
    CargoBay(u32),
    /// Fuel, in litres.
    Tank(u32),
    /// The driver has to sit somewhere.
    Seat,
}

impl Part {
    /// Kerb mass in kilograms *(all real, near enough)*.
    ///
    /// A truck diesel runs about 1.5 kg per kilowatt; a lorry wheel and
    /// tyre is 60-80 kg against a bicycle's two; steel bodywork is roughly
    /// a tenth of the payload it is rated to carry.
    pub fn mass_kg(self) -> f64 {
        match self {
            Part::Frame { heavy } => {
                if heavy {
                    900.0
                } else {
                    15.0
                }
            }
            Part::Engine(kw) => 40.0 + 1.5 * kw as f64,
            Part::Wheel { heavy } => {
                if heavy {
                    70.0
                } else {
                    12.0
                }
            }
            // Trailer bodywork runs to about a quarter of what it is
            // rated to carry: a 44-tonne artic is 13-15 t of vehicle
            // before anything goes in it.
            Part::CargoBay(kg) => 40.0 + 0.28 * kg as f64,
            Part::Tank(l) => 8.0 + 0.35 * l as f64,
            Part::Seat => 18.0,
        }
    }

    /// What it costs, in days of an unskilled wage.
    ///
    /// The absolute prices are meaningless across centuries; the ratios to
    /// earnings are not. A second-hand van is a few months of unskilled
    /// wages, an artic tractor unit the better part of two years.
    pub fn price_in_wage_days(self) -> f64 {
        match self {
            Part::Frame { heavy } => {
                if heavy {
                    8.0
                } else {
                    1.5
                }
            }
            Part::Engine(kw) => 6.0 + 0.55 * kw as f64,
            Part::Wheel { heavy } => {
                if heavy {
                    7.0
                } else {
                    1.0
                }
            }
            Part::CargoBay(kg) => 4.0 + 0.012 * kg as f64,
            Part::Tank(l) => 1.0 + 0.02 * l as f64,
            Part::Seat => 3.0,
        }
    }

    /// Kilograms of payload this part contributes.
    pub fn capacity_kg(self) -> f64 {
        match self {
            Part::CargoBay(kg) => kg as f64,
            _ => 0.0,
        }
    }

    pub fn power_kw(self) -> f64 {
        match self {
            Part::Engine(kw) => kw as f64,
            _ => 0.0,
        }
    }
}

/// A vehicle: some parts bolted together, in a state of repair.
#[derive(Clone, Debug)]
pub struct Vehicle {
    pub name: &'static str,
    pub parts: Vec<Part>,
    /// 1.0 as it left the works, falling as it wears.
    pub condition: f64,
}

impl Vehicle {
    fn of(name: &'static str, parts: Vec<Part>) -> Self {
        Vehicle {
            name,
            parts,
            condition: 1.0,
        }
    }

    /// **A bicycle with a trailer.** What somebody with a month's savings
    /// buys, and it beats walking by a factor of five.
    pub fn bicycle() -> Self {
        Vehicle::of(
            "a bicycle and trailer",
            vec![
                Part::Frame { heavy: false },
                Part::Wheel { heavy: false },
                Part::Wheel { heavy: false },
                Part::Wheel { heavy: false },
                Part::Seat,
                Part::CargoBay(80),
            ],
        )
    }

    /// **A second-hand van.** The first rung that is a living rather than
    /// an errand, and the first that needs fuel.
    pub fn van() -> Self {
        Vehicle::of(
            "a second-hand van",
            vec![
                Part::Frame { heavy: true },
                Part::Frame { heavy: true },
                Part::Engine(90),
                Part::Wheel { heavy: false },
                Part::Wheel { heavy: false },
                Part::Wheel { heavy: false },
                Part::Wheel { heavy: false },
                Part::Seat,
                Part::Tank(70),
                Part::CargoBay(1200),
            ],
        )
    }

    /// **A rigid box truck**, seven and a half tonnes gross.
    pub fn box_truck() -> Self {
        Vehicle::of(
            "a box truck",
            vec![
                Part::Frame { heavy: true },
                Part::Frame { heavy: true },
                Part::Frame { heavy: true },
                Part::Engine(160),
                Part::Wheel { heavy: true },
                Part::Wheel { heavy: true },
                Part::Wheel { heavy: true },
                Part::Wheel { heavy: true },
                Part::Seat,
                Part::Tank(150),
                Part::CargoBay(1800),
                Part::CargoBay(1800),
            ],
        )
    }

    /// **An artic.** Forty-four tonnes gross on European roads, which
    /// leaves the twenty-four tonnes of payload hauliers quote.
    pub fn artic() -> Self {
        let mut parts = vec![
            Part::Frame { heavy: true },
            Part::Frame { heavy: true },
            Part::Frame { heavy: true },
            Part::Frame { heavy: true },
            Part::Frame { heavy: true },
            Part::Engine(330),
            Part::Seat,
            Part::Tank(500),
        ];
        for _ in 0..12 {
            parts.push(Part::Wheel { heavy: true });
        }
        for _ in 0..8 {
            parts.push(Part::CargoBay(3000));
        }
        Vehicle::of("an artic", parts)
    }

    /// Empty weight, in tonnes.
    pub fn kerb_t(&self) -> f64 {
        self.parts.iter().map(|p| p.mass_kg()).sum::<f64>() / 1000.0
    }

    /// **What it will carry**, in tonnes — the sum of its cargo bays.
    ///
    /// Not a figure from a table: bolt another bay on and it carries more,
    /// take one off and it carries less.
    pub fn payload_t(&self) -> f64 {
        self.parts.iter().map(|p| p.capacity_kg()).sum::<f64>() / 1000.0 * self.condition
    }

    pub fn power_kw(&self) -> f64 {
        self.parts.iter().map(|p| p.power_kw()).sum::<f64>() * self.condition
    }

    /// Loaded weight, in tonnes.
    pub fn gross_t(&self) -> f64 {
        self.kerb_t() + self.payload_t()
    }

    /// Cruising speed, in km/h.
    ///
    /// **From power against weight**, which is what actually decides
    /// whether a lorry holds its speed. The EU requires at least 5 kW per
    /// tonne to be roadworthy; a well-powered artic runs 7-8 and cruises
    /// at 85. Below that it crawls on the hills, which is the whole reason
    /// heavy freight and mountains do not get on.
    pub fn cruise_kmh(&self) -> f64 {
        let kw_per_t = self.power_kw() / self.gross_t().max(0.01);
        if kw_per_t <= 0.0 {
            // Muscle. A cyclist with a loaded trailer does 15 km/h and is
            // not much slower up a hill than down it.
            return 15.0;
        }
        (30.0 + 8.0 * kw_per_t).min(90.0)
    }

    /// Litres of fuel per 100 km *(real)*.
    ///
    /// A 44-tonne artic burns about 35 l/100 km, a 3.5-tonne van about 10.
    /// Weight is most of it.
    pub fn litres_per_100km(&self) -> f64 {
        if self.power_kw() <= 0.0 {
            return 0.0;
        }
        6.0 + 0.65 * self.gross_t()
    }

    /// What it costs to buy, in days of an unskilled wage.
    pub fn price_in_wage_days(&self) -> f64 {
        self.parts
            .iter()
            .map(|p| p.price_in_wage_days())
            .sum::<f64>()
    }
}
