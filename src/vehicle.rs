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
//! ## A vehicle is ground, not a mode
//!
//! **You do not "enter" a vehicle; you stand on one of its tiles.** That
//! is how CDDA does it and it is the reason the part model is worth
//! having: a lorry is seventeen metres of occupied road with a seat at
//! one end, and where you are standing on it decides what you can reach.
//! A vehicle with no footprint cannot be stood on, blocked by, crashed
//! into, or slept in.
//!
//! One tile is a metre, which pins the bottom of the scale ladder: a
//! 25 m town plot is 25 x 25 tiles, and a 16.4 km region cell is the
//! world map. The same three numbers all the way down.
//!
//! **This is a modern world.** It has coal-fired power stations, canneries
//! and forty-four-tonne artics on its trunk roads, so the ladder a poor
//! man climbs runs bicycle, second-hand van, box truck, artic — not
//! handcart and pack mule. Those belonged to a different century and were
//! simply the wrong furniture.

/// One component of a vehicle.
///
/// ## What CDDA has, and what is here
///
/// CDDA's list runs to a couple of hundred parts, and the organising idea
/// in it is worth more than the list: **a vehicle is a mobile building**.
/// It has structure, power, storage, workstations, protection and
/// controls, which is the same set a shop or a mill has — and is exactly
/// why the design doc asks for the part system to be generalised to
/// buildings rather than reinvented there.
///
/// Represented here, because each does work in this simulation:
/// structure, wheels, engines, fuel, seating, cargo, **controls** (a part
/// you can lose, after which it does not move), **electrics** (battery,
/// alternator, solar — the grid already models power, so a vehicle can be
/// one), **refrigeration** (this economy has spoilage and a reefer is the
/// difference between hauling food and hauling grain), a **workshop rig**
/// (the fault-response crews already exist and a rig is what they carry),
/// and **land gear** (a plough against eight person-hours a tonne of
/// grain is the whole story of agricultural labour).
///
/// Deliberately not here yet, and each for a reason rather than an
/// oversight: doors, roofs, boards and windows (they matter once weather
/// and the walkable interior exist), armour and turret mounts (they
/// matter once C1's conflict does), lights (once there is night), and
/// kitchens, forges and labs (once crafting does).
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
    /// **The controls.** Wheel, pedals, the rest of it.
    ///
    /// A part you can lose, and the vehicle does not move without it —
    /// which is the property that makes a part list worth having rather
    /// than a specification sheet.
    Controls,
    /// Stores electricity, in kilowatt-hours.
    ///
    /// A lorry's starter battery is 12 V and 220 Ah, so about 2.6 kWh; a
    /// refrigerated trailer carries far more.
    Battery(u32),
    /// Makes electricity while the engine turns, in watts *(real: a truck
    /// alternator is 1.5-3 kW)*.
    Alternator(u32),
    /// Makes electricity from daylight, in watts *(real: about 200 W to
    /// the square metre at peak)*.
    SolarPanel(u32),
    /// **Refrigeration**, in watts of cooling.
    ///
    /// A transport refrigeration unit holding a trailer at -20 C draws
    /// 5-15 kW and burns two to four litres an hour doing it. It is the
    /// difference between hauling food and hauling grain, and in an
    /// economy with spoilage in the ledger that is a real distinction.
    Refrigeration(u32),
    /// A workshop in the back: tools, a bench, a welder.
    ///
    /// The fault-response crews already drive to breakdowns; this is what
    /// they are carrying when they get there.
    WorkshopRig,
    /// Ploughs, drills, harvesters — gear that works the ground rather
    /// than carrying anything.
    ///
    /// Eight person-hours to bring in a tonne of grain is the figure the
    /// whole agricultural labour model rests on, and mechanisation is what
    /// moves it.
    LandGear { working_width_m: u32 },
}

impl Part {
    /// Kerb mass in kilograms *(all real, near enough)*.
    ///
    /// A truck diesel runs about 1.5 kg per kilowatt; a lorry wheel and
    /// tyre is 60-80 kg against a bicycle's two; steel bodywork is roughly
    /// a tenth of the payload it is rated to carry.
    pub fn mass_kg(self) -> f64 {
        match self {
            // **One tile of structure**, not a whole chassis. An artic is
            // 51 tiles of frame and 13-15 t of vehicle before anything
            // goes in it, so a tile of it is about 120 kg. Sizing a frame
            // as if it were the entire ladder chassis gave a 54-tonne
            // empty lorry.
            Part::Frame { heavy } => {
                if heavy {
                    120.0
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
            Part::Controls => 25.0,
            // Lead-acid runs about 25 kg the kilowatt-hour; lithium is a
            // third of that and several times the price.
            Part::Battery(kwh) => 6.0 + 25.0 * kwh as f64,
            Part::Alternator(w) => 4.0 + 0.008 * w as f64,
            // A panel is about 11 kg to the square metre and 200 W to it.
            Part::SolarPanel(w) => 2.0 + 0.055 * w as f64,
            Part::Refrigeration(w) => 120.0 + 0.030 * w as f64,
            Part::WorkshopRig => 350.0,
            Part::LandGear { working_width_m } => 200.0 * working_width_m as f64,
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
                    1.1
                } else {
                    0.3
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
            Part::Controls => 12.0,
            Part::Battery(kwh) => 2.0 + 3.0 * kwh as f64,
            Part::Alternator(w) => 1.0 + 0.004 * w as f64,
            Part::SolarPanel(w) => 0.5 + 0.010 * w as f64,
            Part::Refrigeration(w) => 40.0 + 0.012 * w as f64,
            Part::WorkshopRig => 90.0,
            Part::LandGear { working_width_m } => 25.0 * working_width_m as f64,
        }
    }

    /// Kilograms of payload this part contributes.
    pub fn capacity_kg(self) -> f64 {
        match self {
            Part::CargoBay(kg) => kg as f64,
            _ => 0.0,
        }
    }

    /// Which part of a stacked tile is the one you would name.
    ///
    /// You say "the driver's seat", not "the frame under the driver's
    /// seat", and a wheel is more worth knowing about than the rail it is
    /// hung from.
    pub fn prominence(self) -> u8 {
        match self {
            Part::Controls => 7,
            Part::Seat => 6,
            Part::Refrigeration(_) => 6,
            Part::WorkshopRig => 6,
            Part::LandGear { .. } => 6,
            Part::Engine(_) => 5,
            Part::Battery(_) => 4,
            Part::Alternator(_) => 3,
            Part::SolarPanel(_) => 3,
            Part::CargoBay(_) => 4,
            Part::Tank(_) => 3,
            Part::Wheel { .. } => 2,
            Part::Frame { .. } => 1,
        }
    }

    /// One character, so the thing can be drawn.
    pub fn glyph(self) -> char {
        match self {
            Part::Frame { .. } => '#',
            Part::Engine(_) => 'E',
            Part::Wheel { .. } => 'o',
            Part::CargoBay(_) => '=',
            Part::Tank(_) => 'T',
            Part::Seat => '@',
            Part::Controls => '!',
            Part::Battery(_) => 'b',
            Part::Alternator(_) => 'a',
            Part::SolarPanel(_) => 'p',
            Part::Refrigeration(_) => '*',
            Part::WorkshopRig => 'w',
            Part::LandGear { .. } => 'Y',
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
    /// Every part, and **where on the vehicle it is** — one metre to the
    /// tile, x along the length and y across the width.
    ///
    /// Position is not decoration. It is what makes a vehicle a piece of
    /// ground somebody can stand on, which is the whole point of the CDDA
    /// model: the seat is at one end, the cargo is behind it, and a part
    /// that is damaged is damaged *somewhere*.
    pub parts: Vec<(Part, i32, i32)>,
    /// 1.0 as it left the works, falling as it wears.
    pub condition: f64,
}

impl Vehicle {
    fn of(name: &'static str, parts: Vec<(Part, i32, i32)>) -> Self {
        Vehicle {
            name,
            parts,
            condition: 1.0,
        }
    }

    /// Just the parts, for the sums that do not care where anything is.
    pub fn kinds(&self) -> impl Iterator<Item = Part> + '_ {
        self.parts.iter().map(|&(p, _, _)| p)
    }

    /// How much road it takes up, in tiles: length by width.
    ///
    /// Real dimensions, which is why an artic and a bicycle are different
    /// problems on the same street: a 44-tonne artic is 16.5 m long and
    /// 2.55 m wide, a Transit is 5.5 by 2, a bicycle with a trailer under
    /// 2 by 1.
    pub fn footprint(&self) -> (i32, i32) {
        if self.parts.is_empty() {
            return (0, 0);
        }
        let xs: Vec<i32> = self.parts.iter().map(|&(_, x, _)| x).collect();
        let ys: Vec<i32> = self.parts.iter().map(|&(_, _, y)| y).collect();
        (
            xs.iter().max().unwrap() - xs.iter().min().unwrap() + 1,
            ys.iter().max().unwrap() - ys.iter().min().unwrap() + 1,
        )
    }

    /// What is on one tile of it — **the part that matters there**.
    ///
    /// Tiles stack: a wheel is bolted to a frame and a seat sits on one,
    /// so a tile usually holds several parts. Returning whichever happened
    /// to be added first drew a lorry as a featureless slab with its
    /// wheels and its load hidden underneath the chassis.
    pub fn at(&self, x: i32, y: i32) -> Option<Part> {
        self.parts
            .iter()
            .filter(|&&(_, px, py)| px == x && py == y)
            .map(|&(p, _, _)| p)
            .max_by_key(|p| p.prominence())
    }

    /// Where somebody would sit to drive it.
    pub fn driver_seat(&self) -> Option<(i32, i32)> {
        self.parts
            .iter()
            .find(|(p, _, _)| matches!(p, Part::Seat))
            .map(|&(_, x, y)| (x, y))
    }

    /// The thing drawn out, one character to the metre.
    pub fn render(&self) -> String {
        let xs: Vec<i32> = self.parts.iter().map(|&(_, x, _)| x).collect();
        let ys: Vec<i32> = self.parts.iter().map(|&(_, _, y)| y).collect();
        let (x0, x1) = (*xs.iter().min().unwrap(), *xs.iter().max().unwrap());
        let (y0, y1) = (*ys.iter().min().unwrap(), *ys.iter().max().unwrap());
        let mut out = String::new();
        for y in y0..=y1 {
            for x in x0..=x1 {
                out.push(self.at(x, y).map_or('.', |p| p.glyph()));
            }
            out.push('\n');
        }
        out
    }

    /// **A bicycle with a trailer.** 2 m by 1: what somebody with a
    /// month's savings buys, and it beats walking by a factor of five.
    pub fn bicycle() -> Self {
        Vehicle::of(
            "a bicycle and trailer",
            vec![
                (Part::Frame { heavy: false }, 0, 0),
                (Part::Seat, 0, 0),
                (Part::Controls, 0, 0),
                (Part::Wheel { heavy: false }, 0, 0),
                (Part::CargoBay(80), 1, 0),
                (Part::Wheel { heavy: false }, 1, 0),
                (Part::Wheel { heavy: false }, 1, 0),
            ],
        )
    }

    /// **A second-hand van** — 5.5 m by 2, so 6 tiles by 2. The first rung
    /// that is a living rather than an errand, and the first that needs
    /// fuel.
    pub fn van() -> Self {
        let mut p = vec![
            (Part::Engine(90), 0, 0),
            (Part::Engine(0), 0, 1),
            (Part::Seat, 1, 0),
            (Part::Controls, 1, 0),
            (Part::Alternator(1500), 0, 0),
            (Part::Battery(3), 0, 1),
            (Part::Frame { heavy: true }, 1, 1),
            (Part::Tank(70), 2, 1),
        ];
        p.retain(|&(part, _, _)| part.power_kw() != 0.0 || !matches!(part, Part::Engine(_)));
        for y in 0..2 {
            p.push((Part::Wheel { heavy: false }, 0, y));
            p.push((Part::Wheel { heavy: false }, 4, y));
        }
        for x in 2..6 {
            for y in 0..2 {
                p.push((Part::Frame { heavy: true }, x, y));
            }
        }
        p.push((Part::CargoBay(1200), 4, 0));
        Vehicle::of("a second-hand van", p)
    }

    /// **A rigid box truck** — 8 m by 2.5, so 8 tiles by 3.
    pub fn box_truck() -> Self {
        let mut p = vec![
            (Part::Engine(160), 0, 1),
            (Part::Seat, 1, 1),
            (Part::Controls, 1, 1),
            (Part::Alternator(2000), 0, 0),
            (Part::Battery(3), 0, 2),
            (Part::Tank(150), 1, 2),
        ];
        for x in 0..8 {
            for y in 0..3 {
                p.push((Part::Frame { heavy: true }, x, y));
            }
        }
        for &x in &[0i32, 6] {
            p.push((Part::Wheel { heavy: true }, x, 0));
            p.push((Part::Wheel { heavy: true }, x, 2));
        }
        p.push((Part::CargoBay(1800), 4, 1));
        p.push((Part::CargoBay(1800), 6, 1));
        Vehicle::of("a box truck", p)
    }

    /// **An artic** — 16.5 m by 2.55, so 17 tiles by 3. Forty-four tonnes
    /// gross on European roads, which leaves the twenty-four tonnes of
    /// payload hauliers quote.
    pub fn artic() -> Self {
        let mut p = vec![
            (Part::Engine(330), 0, 1),
            (Part::Seat, 1, 1),
            (Part::Controls, 1, 1),
            (Part::Alternator(3000), 0, 0),
            (Part::Battery(5), 0, 2),
            (Part::Tank(500), 2, 0),
        ];
        // Tractor unit, then the trailer behind it.
        for x in 0..17 {
            for y in 0..3 {
                p.push((Part::Frame { heavy: true }, x, y));
            }
        }
        // Steer axle, drive axles, and the trailer bogie.
        for &x in &[0i32, 2, 3, 13, 14, 15] {
            p.push((Part::Wheel { heavy: true }, x, 0));
            p.push((Part::Wheel { heavy: true }, x, 2));
        }
        // Eight pallet bays down the trailer.
        for i in 0..8 {
            p.push((Part::CargoBay(3000), 5 + i, 1));
        }
        Vehicle::of("an artic", p)
    }

    /// **A refrigerated artic.** The same lorry with a fridge on the front
    /// of the trailer and the generation to run it, which is what lets it
    /// carry food rather than grain.
    pub fn reefer() -> Self {
        let mut v = Vehicle::artic();
        v.name = "a refrigerated artic";
        v.parts.push((Part::Refrigeration(12_000), 4, 1));
        // A reefer carries its own generator set; the tractor's alternator
        // comes nowhere near 12 kW around the clock.
        v.parts.push((Part::Alternator(42_000), 4, 0));
        v.parts.push((Part::Battery(20), 4, 2));
        v
    }

    /// Empty weight, in tonnes.
    pub fn kerb_t(&self) -> f64 {
        self.kinds().map(|p| p.mass_kg()).sum::<f64>() / 1000.0
    }

    /// **What it will carry**, in tonnes — the sum of its cargo bays.
    ///
    /// Not a figure from a table: bolt another bay on and it carries more,
    /// take one off and it carries less.
    pub fn payload_t(&self) -> f64 {
        self.kinds().map(|p| p.capacity_kg()).sum::<f64>() / 1000.0 * self.condition
    }

    pub fn power_kw(&self) -> f64 {
        self.kinds().map(|p| p.power_kw()).sum::<f64>() * self.condition
    }

    /// Loaded weight, in tonnes.
    pub fn gross_t(&self) -> f64 {
        self.kerb_t() + self.payload_t()
    }

    /// **Whether it will move at all.**
    ///
    /// Needs something to drive it, something to steer with, and wheels
    /// under it. This is the property that makes a part list worth having
    /// rather than a specification sheet: a lorry with its controls
    /// stripped is a shed, and everything that follows from that —
    /// salvage, sabotage, a wreck you can rob for parts — needs the
    /// question to be askable.
    pub fn drivable(&self) -> bool {
        let mut controls = false;
        let mut wheels = 0;
        let mut drive = false;
        for p in self.kinds() {
            match p {
                Part::Controls => controls = true,
                Part::Wheel { .. } => wheels += 1,
                Part::Engine(kw) if kw > 0 => drive = true,
                Part::Seat => {}
                _ => {}
            }
        }
        // Muscle counts: a bicycle has no engine and goes perfectly well.
        controls && wheels >= 2 && (drive || self.power_kw() == 0.0)
    }

    /// Electricity it can make in a day, in kilowatt-hours.
    ///
    /// The alternator turns while the engine does; the panel works in
    /// daylight, which averages about five useful hours a day across a
    /// year in temperate country.
    pub fn generation_kwh_per_day(&self) -> f64 {
        let mut out = 0.0;
        for p in self.kinds() {
            match p {
                Part::Alternator(w) => out += w as f64 / 1000.0 * 7.0,
                Part::SolarPanel(w) => out += w as f64 / 1000.0 * 5.0,
                _ => {}
            }
        }
        out * self.condition
    }

    /// Electricity it needs in a day, in kilowatt-hours.
    pub fn draw_kwh_per_day(&self) -> f64 {
        self.kinds()
            .map(|p| match p {
                // A reefer runs around the clock; that is the point of it.
                Part::Refrigeration(w) => w as f64 / 1000.0 * 24.0,
                Part::WorkshopRig => 2.0,
                _ => 0.0,
            })
            .sum()
    }

    /// **Whether the electrics balance**, which decides whether a reefer
    /// holds its temperature or the load is spoiled by the time it lands.
    pub fn powered(&self) -> bool {
        self.draw_kwh_per_day() <= self.generation_kwh_per_day() + 1e-9
    }

    /// Whether it can carry food without spoiling it.
    pub fn refrigerated(&self) -> bool {
        self.kinds().any(|p| matches!(p, Part::Refrigeration(_))) && self.powered()
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
        self.kinds().map(|p| p.price_in_wage_days()).sum::<f64>()
    }
}
