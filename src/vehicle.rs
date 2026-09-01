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

/// **How wide a vehicle really is, given how many tiles it occupies.**
///
/// The grid is not square-on to reality, and CDDA's answer to that is
/// better than the one this file had before. Length in tiles is length in
/// metres, near enough — but **width is deliberately compressed**:
///
/// | tiles | modelled |
/// |---:|---:|
/// | 1 | 0.90 m |
/// | 2 | 1.30 m |
/// | 3 | 1.70 m |
/// | 4 | 2.10 m |
/// | 5 | 2.50 m |
/// | 6 | 2.65 m |
/// | 7 | 2.80 m |
///
/// The point is **interior resolution**. A real car is about 2 m across,
/// which at one metre to the tile is two squares — and two squares cannot
/// hold two seats, two doors and the bodywork round them. Spending four
/// squares on it and then reading the width off this table gets both: a
/// cabin you can lay out and a vehicle that is the right size on the road.
///
/// This replaces a stored `width_m` that had to be typed in per vehicle,
/// with a note in the project file calling it "the one measurement not
/// read off the tiles". It is read off the tiles now; it just is not read
/// off them linearly.
///
/// Above seven tiles the curve has flattened — a bus and an artic are both
/// about 2.8 m over the mirrors, and the law is why. Which is a pleasing
/// fit with `townplan::clearance_for`: **2.9 m is the width above which
/// you must give the police two days' notice**, so the widest ordinary
/// vehicle on the road is the widest one that needs no paperwork.
pub fn modelled_width_m(tiles: i32) -> f64 {
    match tiles.max(1) {
        1 => 0.90,
        2 => 1.30,
        3 => 1.70,
        4 => 2.10,
        5 => 2.50,
        6 => 2.65,
        _ => 2.80,
    }
}

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
            // **One tile of structure**, not a whole chassis — a square
            // metre of chassis rail and bodywork.
            //
            // Recalibrated when the grids widened to CDDA footprints: a
            // vehicle drawn four to seven tiles across has two or three
            // times the frame tiles it had at two or three, so a figure
            // set against the old narrow layouts made every lorry far too
            // heavy. An artic is 85 tiles of frame and 13-15 t empty,
            // which is about 65 kg a tile; car bodywork is 25.
            Part::Frame { heavy } => {
                if heavy {
                    65.0
                } else {
                    25.0
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
    /// **Nothing a vehicle is made of may look like the ground it stands
    /// on.** These used to be `#`, `=`, `T`, `o`, `*` and `@` — which are
    /// wall, road, tree, window, scrub and the player — so a parked lorry
    /// read as `a#To#########ooo#` and was genuinely impossible to pick
    /// out from the street it was on. Colour is how a roguelike normally
    /// solves this and there is none here, so the characters have to do
    /// the work themselves.
    pub fn glyph(self) -> char {
        match self {
            Part::Frame { .. } => '+',
            Part::Engine(_) => 'E',
            Part::Wheel { .. } => 'O',
            Part::CargoBay(_) => 'B',
            Part::Tank(_) => 'F', // fuel
            Part::Seat => '%',
            Part::Controls => '!',
            Part::Battery(_) => 'b',
            Part::Alternator(_) => 'a',
            Part::SolarPanel(_) => 'p',
            Part::Refrigeration(_) => 'x',
            Part::WorkshopRig => 'w',
            Part::LandGear { .. } => 'Y',
        }
    }

    /// What it is, for a legend.
    pub fn label(self) -> &'static str {
        match self {
            Part::Frame { .. } => "frame",
            Part::Engine(_) => "engine",
            Part::Wheel { .. } => "wheel",
            Part::CargoBay(_) => "cargo bay",
            Part::Tank(_) => "fuel tank",
            Part::Seat => "seat",
            Part::Controls => "controls",
            Part::Battery(_) => "battery",
            Part::Alternator(_) => "alternator",
            Part::SolarPanel(_) => "solar panel",
            Part::Refrigeration(_) => "refrigeration",
            Part::WorkshopRig => "workshop rig",
            Part::LandGear { .. } => "land gear",
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
    /// **How wide it really is**, in metres.
    ///
    /// The one measurement that is not read off the tile grid, because a
    /// metre is too coarse for it: an artic is 2.55 m — the European legal
    /// maximum — and rounds up to three tiles, and 3.0 m would put an
    /// ordinary lorry over the 2.9 m line where the police want notice.
    /// The tiles say where it sits on the road; this says whether it fits.
    /// A test holds the two together, so a layout cannot drift from it.
    pub width_m: f64,
}

impl Vehicle {
    fn of(name: &'static str, parts: Vec<(Part, i32, i32)>) -> Self {
        let mut v = Vehicle {
            name,
            parts,
            condition: 1.0,
            width_m: 0.0,
        };
        v.width_m = modelled_width_m(v.footprint().1);
        v
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
                (Part::Wheel { heavy: false }, 0, 0),
                (Part::Frame { heavy: false }, 1, 0),
                (Part::Seat, 1, 0),
                (Part::Controls, 1, 0),
                (Part::Frame { heavy: false }, 2, 0),
                (Part::Wheel { heavy: false }, 2, 0),
                (Part::CargoBay(80), 2, 0),
            ],
        )
    }

    /// **A second-hand van** — 5.5 m by 2, so 6 tiles by 2. The first rung
    /// that is a living rather than an errand, and the first that needs
    /// fuel.
    pub fn van() -> Self {
        // **Seven by four**, which is a cargo van in CDDA's stock list and
        // 7.0 x 2.1 m once the width table is applied. Four across is what
        // buys a cab you can lay out: two seats side by side with the
        // bodywork either side of them, rather than a two-tile slab that
        // has to pretend.
        let mut p = Vec::new();
        // A van has a ladder chassis under it, not a car's monocoque.
        for x in 0..7 {
            for y in 0..4 {
                p.push((Part::Frame { heavy: true }, x, y));
            }
        }
        p.push((Part::Engine(90), 0, 1));
        p.push((Part::Seat, 1, 1));
        p.push((Part::Controls, 1, 1));
        p.push((Part::Seat, 1, 2));
        p.push((Part::Alternator(1500), 0, 2));
        p.push((Part::Battery(3), 0, 0));
        p.push((Part::Tank(70), 2, 3));
        for y in [0, 3] {
            p.push((Part::Wheel { heavy: false }, 0, y));
            p.push((Part::Wheel { heavy: false }, 5, y));
        }
        for x in 3..6 {
            p.push((Part::CargoBay(400), x, 1));
        }
        Vehicle::of("a second-hand van", p)
    }

    /// **A rigid box truck** — 8 m by 2.5, so 8 tiles by 3.
    pub fn box_truck() -> Self {
        // **Eight by seven** — CDDA's cube van. The body is five tiles
        // across and the seven-tile span is the mirrors, which is exactly
        // what matters on a road: mirrors are what clip.
        let mut p = Vec::new();
        for x in 0..8 {
            for y in 1..6 {
                p.push((Part::Frame { heavy: true }, x, y));
            }
        }
        // Mirrors, standing proud of the body at the cab.
        p.push((Part::Frame { heavy: false }, 1, 0));
        p.push((Part::Frame { heavy: false }, 1, 6));
        p.push((Part::Engine(160), 0, 3));
        p.push((Part::Seat, 1, 2));
        p.push((Part::Controls, 1, 2));
        p.push((Part::Seat, 1, 4));
        p.push((Part::Alternator(2000), 0, 1));
        p.push((Part::Battery(3), 0, 5));
        p.push((Part::Tank(150), 2, 5));
        for &x in &[0i32, 6] {
            p.push((Part::Wheel { heavy: true }, x, 1));
            p.push((Part::Wheel { heavy: true }, x, 5));
        }
        for x in 3..8 {
            p.push((Part::CargoBay(720), x, 3));
        }
        Vehicle::of("a box truck", p)
    }

    /// **An artic** — 16.5 m by 2.55, so 17 tiles by 3. Forty-four tonnes
    /// gross on European roads, which leaves the twenty-four tonnes of
    /// payload hauliers quote.
    pub fn artic() -> Self {
        let mut p = Vec::new();
        // **Tractor unit, five tiles across with the mirrors out to
        // seven.** CDDA gives a semi tractor 9x7 and a trailer 10x5, and
        // the mirrors are the reason for the difference.
        for x in 0..7 {
            for y in 1..6 {
                p.push((Part::Frame { heavy: true }, x, y));
            }
        }
        p.push((Part::Frame { heavy: false }, 1, 0));
        p.push((Part::Frame { heavy: false }, 1, 6));
        // The trailer behind it, five across and no wider.
        for x in 7..17 {
            for y in 1..6 {
                p.push((Part::Frame { heavy: true }, x, y));
            }
        }
        p.push((Part::Engine(330), 0, 3));
        p.push((Part::Seat, 1, 2));
        p.push((Part::Controls, 1, 2));
        p.push((Part::Seat, 1, 4));
        p.push((Part::Alternator(3000), 0, 1));
        p.push((Part::Battery(5), 0, 5));
        p.push((Part::Tank(500), 2, 1));
        p.push((Part::Tank(500), 2, 5));
        // Steer axle, drive axles, and the trailer bogie.
        for &x in &[0i32, 4, 5, 13, 14, 15] {
            p.push((Part::Wheel { heavy: true }, x, 1));
            p.push((Part::Wheel { heavy: true }, x, 5));
        }
        // Thirteen pallet bays down the trailer, which is what a standard
        // 13.6 m curtainsider takes.
        for i in 0..13 {
            p.push((Part::CargoBay(1850), 4 + i / 2, 2 + (i % 2) * 2));
        }
        Vehicle::of("an artic", p)
    }

    /// **A refrigerated artic.** The same lorry with a fridge on the front
    /// of the trailer and the generation to run it, which is what lets it
    /// carry food rather than grain.
    pub fn reefer() -> Self {
        let mut v = Vehicle::artic();
        v.name = "a refrigerated artic";
        // On the front bulkhead of the trailer, which is where a
        // transport refrigeration unit actually hangs — and inside the
        // frame, because a part with no frame under it is a part falling
        // off.
        v.parts.push((Part::Refrigeration(12_000), 7, 3));
        // A reefer carries its own generator set; the tractor's alternator
        // comes nowhere near 12 kW around the clock.
        v.parts.push((Part::Alternator(42_000), 7, 2));
        v.parts.push((Part::Battery(20), 7, 4));
        // Insulation costs you width: a refrigerated body is allowed 2.6 m
        // where a dry one is held to 2.55.
        v.width_m = 2.6;
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

    /// **Power to weight**, which is what decides whether a lorry holds
    /// its speed on a hill. The EU requires at least 5 kW per tonne to be
    /// roadworthy; a well-powered artic runs 7-8.
    pub fn kw_per_tonne(&self) -> f64 {
        self.power_kw() / self.gross_t().max(0.01)
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

// ---------------------------------------------------------------------------
// Structure: what holds a vehicle together, and what happens when it does not
// ---------------------------------------------------------------------------

impl Part {
    /// **Does this have to be bolted to something?**
    ///
    /// CDDA's rule, and it is the one that makes a part list a structure
    /// rather than a bag: nearly everything requires a frame or a mounting
    /// point beneath it, and **the frame must exist before anything can be
    /// installed onto it**. A seat with no floor under it is not a seat, it
    /// is a seat falling through the hole where the floor was.
    pub fn needs_a_frame(self) -> bool {
        !matches!(self, Part::Frame { .. })
    }
}

impl Vehicle {
    /// Every coordinate that has a frame on it.
    fn frames(&self) -> Vec<(i32, i32)> {
        let mut v: Vec<(i32, i32)> = self
            .parts
            .iter()
            .filter(|(p, _, _)| matches!(p, Part::Frame { .. }))
            .map(|&(_, x, y)| (x, y))
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// **Is everything actually attached to something?**
    ///
    /// A well-formed vehicle has a frame under every part that needs one.
    /// This is cheap to check and worth checking, because a layout typed
    /// in by hand is exactly the sort of thing that quietly grows a seat
    /// hanging in mid-air.
    pub fn well_formed(&self) -> bool {
        let frames = self.frames();
        self.parts
            .iter()
            .filter(|(p, _, _)| p.needs_a_frame())
            .all(|&(_, x, y)| frames.binary_search(&(x, y)).is_ok())
    }

    /// **The connected sections of the frame**, four-connected.
    ///
    /// A vehicle is one object for as long as its surviving structure
    /// stays joined up. This is the question `destroy_frame` asks
    /// afterwards.
    pub fn sections(&self) -> Vec<Vec<(i32, i32)>> {
        let frames = self.frames();
        let mut unseen: Vec<(i32, i32)> = frames.clone();
        let mut out: Vec<Vec<(i32, i32)>> = Vec::new();

        while let Some(&start) = unseen.first() {
            let mut group = Vec::new();
            let mut stack = vec![start];
            unseen.retain(|&t| t != start);
            while let Some((x, y)) = stack.pop() {
                group.push((x, y));
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let n = (x + dx, y + dy);
                    if let Some(i) = unseen.iter().position(|&t| t == n) {
                        unseen.remove(i);
                        stack.push(n);
                    }
                }
            }
            group.sort_unstable();
            out.push(group);
        }
        // Biggest first, and deterministic on a tie.
        out.sort_by(|a, b| b.len().cmp(&a.len()).then(a[0].cmp(&b[0])));
        out
    }

    /// **Knock the structure out of one tile and see what is left.**
    ///
    /// This is the property the whole part model exists for. A collision
    /// does not subtract from one global health bar: it destroys the frame
    /// it hit, everything installed on that frame goes with it, and then
    /// **whatever is no longer joined to the rest becomes separate
    /// wreckage**. That is how a crash tears the back off a lorry.
    ///
    /// Returns the pieces, largest first. One piece means it held
    /// together; none means there was nothing left.
    pub fn destroy_frame(&self, x: i32, y: i32) -> Vec<Vehicle> {
        self.destroy_frames(&[(x, y)])
    }

    /// **What a real impact does**, which is take out several tiles at
    /// once.
    ///
    /// Losing one square metre out of a five-wide slab disconnects
    /// nothing, and that is correct — a lorry does not come in half
    /// because somebody put a hole in the floor. It comes in half when a
    /// whole cross-section goes, which is what this takes.
    pub fn destroy_frames(&self, gone: &[(i32, i32)]) -> Vec<Vehicle> {
        let mut wreck = Vehicle {
            name: self.name,
            parts: self
                .parts
                .iter()
                .copied()
                .filter(|&(_, px, py)| !gone.contains(&(px, py)))
                .collect(),
            condition: self.condition,
            width_m: 0.0,
        };
        if wreck.parts.is_empty() {
            return Vec::new();
        }

        let sections = wreck.sections();
        if sections.len() <= 1 {
            wreck.width_m = modelled_width_m(wreck.footprint().1);
            return vec![wreck];
        }

        sections
            .into_iter()
            .map(|group| {
                let mut piece = Vehicle {
                    name: self.name,
                    parts: wreck
                        .parts
                        .iter()
                        .copied()
                        .filter(|&(_, px, py)| group.binary_search(&(px, py)).is_ok())
                        .collect(),
                    condition: self.condition * 0.5,
                    width_m: 0.0,
                };
                piece.width_m = modelled_width_m(piece.footprint().1);
                piece
            })
            .filter(|p| !p.parts.is_empty())
            .collect()
    }

    /// **Where the weight actually sits**, in tiles.
    ///
    /// Cargo is not one global number: it is stowed at a coordinate, it
    /// adds its mass there, and it shifts this. Which is why loading a
    /// lorry badly is a real mistake and not a cosmetic one.
    pub fn centre_of_mass(&self) -> (f64, f64) {
        let mut m = 0.0;
        let (mut sx, mut sy) = (0.0, 0.0);
        for &(p, x, y) in self.parts.iter() {
            let kg = p.mass_kg();
            m += kg;
            sx += kg * x as f64;
            sy += kg * y as f64;
        }
        if m <= 0.0 {
            return (0.0, 0.0);
        }
        (sx / m, sy / m)
    }

    /// **Is it standing on its own wheels?**
    ///
    /// A valid arrangement needs enough working wheels, and the centre of
    /// mass has to fall inside the ground they cover. Outside it, the
    /// thing tips — which is the real reason a loaded vehicle can be
    /// undriveable while every individual part still works.
    pub fn supported(&self) -> bool {
        let wheels: Vec<(i32, i32)> = self
            .parts
            .iter()
            .filter(|(p, _, _)| matches!(p, Part::Wheel { .. }))
            .map(|&(_, x, y)| (x, y))
            .collect();
        if wheels.len() < 2 {
            return false;
        }
        let (cx, cy) = self.centre_of_mass();
        let xs: Vec<i32> = wheels.iter().map(|&(x, _)| x).collect();
        let ys: Vec<i32> = wheels.iter().map(|&(_, y)| y).collect();
        let (x0, x1) = (
            *xs.iter().min().unwrap() as f64,
            *xs.iter().max().unwrap() as f64,
        );
        let (y0, y1) = (
            *ys.iter().min().unwrap() as f64,
            *ys.iter().max().unwrap() as f64,
        );
        cx >= x0 && cx <= x1 && cy >= y0 && cy <= y1
    }

    /// **The width the law measures**, which is not the width that clips.
    ///
    /// A vehicle's legal width is taken over the **body**: mirrors are
    /// excluded, which is why a 2.55 m artic — the European legal maximum
    /// — stands 2.9 m over its mirrors and is still ordinary traffic.
    /// `width_m` is the over-mirrors figure, because that is what actually
    /// hits things; this is the one the paperwork is written against.
    ///
    /// The body is found rather than declared: mirrors stick out for one
    /// tile of the length, so a row carrying only a tile or two of parts
    /// is not bodywork.
    pub fn body_width_m(&self) -> f64 {
        let (_, w) = self.footprint();
        if self.parts.is_empty() {
            return 0.0;
        }
        let y0 = self.parts.iter().map(|&(_, _, y)| y).min().unwrap();
        let mut along: Vec<usize> = vec![0; w as usize];
        for row in 0..w {
            let mut xs: Vec<i32> = self
                .parts
                .iter()
                .filter(|&&(_, _, y)| y == y0 + row)
                .map(|&(_, x, _)| x)
                .collect();
            xs.sort_unstable();
            xs.dedup();
            along[row as usize] = xs.len();
        }
        let longest = along.iter().copied().max().unwrap_or(0);
        let body = along
            .iter()
            .filter(|&&n| n * 4 > longest)
            .count() as i32;
        modelled_width_m(body.max(1))
    }

    /// **How tall it stands**, in metres — the other half of frontal area.
    ///
    /// Not stored, because it follows from what the thing is: a car sits
    /// low, a rigid box is about the height of a doorway and a half, and a
    /// trailer is four metres because that is what the bridges allow.
    pub fn height_m(&self) -> f64 {
        let heavy = self
            .kinds()
            .any(|p| matches!(p, Part::Frame { heavy: true }));
        let (len, _) = self.footprint();
        if !heavy {
            1.5
        } else if len > 8 {
            4.0
        } else {
            2.9
        }
    }

    /// **The speed at which the engine runs out of push.**
    ///
    /// Maximum velocity is where available power equals aerodynamic drag
    /// plus rolling resistance — which is the actual physics and is what
    /// makes the width table matter, because frontal area is width times
    /// height.
    ///
    /// ```text
    /// P = Â½Â·ÏÂ·CdÂ·AÂ·vÂ³  +  CrrÂ·mÂ·gÂ·v
    /// ```
    ///
    /// Real constants: air at **1.225 kg/mÂ³**; rolling resistance
    /// **0.007** for truck tyres on asphalt and 0.009 for car tyres; a
    /// drivetrain gives about **85%** of engine power to the road.
    pub fn top_speed_kmh(&self) -> f64 {
        const AIR: f64 = 1.225;
        const G: f64 = 9.81;
        const DRIVETRAIN: f64 = 0.85;

        let power = self.power_kw() * 1000.0 * DRIVETRAIN;
        if power <= 0.0 {
            // Muscle. A cyclist with a loaded trailer does about 15 km/h
            // and is not much slower up a hill than down it.
            return 15.0;
        }
        let heavy = self
            .kinds()
            .any(|p| matches!(p, Part::Frame { heavy: true }));
        let area = self.width_m * self.height_m();
        // A bluff box against something shaped to go through the air.
        let cd = if heavy { 0.60 } else { 0.32 };
        let crr = if heavy { 0.007 } else { 0.009 };
        let mass = self.gross_t() * 1000.0;

        // Bisection: the left-hand side rises monotonically with v.
        let needed = |v: f64| 0.5 * AIR * cd * area * v * v * v + crr * mass * G * v;
        let (mut lo, mut hi) = (0.0f64, 120.0f64);
        for _ in 0..48 {
            let mid = 0.5 * (lo + hi);
            if needed(mid) > power {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        lo * 3.6
    }

    /// **What it actually travels at**, which is not what it could do.
    ///
    /// A heavy goods vehicle is **limited by law, not by power**: an EU
    /// speed limiter caps one at 90 km/h, and an artic with 330 kW would
    /// otherwise sit well above that. Everything else is held by the
    /// motorway limit and by the fact that nobody cruises flat out.
    pub fn cruise_kmh(&self) -> f64 {
        let top = self.top_speed_kmh();
        if self.power_kw() <= 0.0 {
            return top;
        }
        // Real: 3.5 t is the line between a van and a lorry, and 12 t the
        // line above which the limiter is compulsory.
        let governed = if self.gross_t() > 3.5 { 90.0 } else { 112.0 };
        (top * 0.85).min(governed)
    }
}

