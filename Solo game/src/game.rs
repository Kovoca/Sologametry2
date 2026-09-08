//! **One world, one clock, one update.**
//!
//! Every system in this project can demonstrate itself and none of them
//! agrees with the others about what time it is. `econ::Ledger` keeps a
//! day and advances it inside `step`; `scaling` keeps another and advances
//! it inside its own tick; a household keeps a day of the year; and
//! `person::live_a_day` takes the day as an *argument*, so whoever calls it
//! is responsible for passing the same number the economy happens to be on.
//! Nothing checks that they match.
//!
//! That is the shape of the problem the whole integration has: **several
//! good models, each holding its own copy of state the others also hold.**
//! A diagnostic binary composes a few of them by hand and keeps them in
//! step by being careful. Being careful is not a contract.
//!
//! So this is the root. It owns the clock, it owns the subsystems, and it
//! is the only thing that advances any of them. A system may be asked to do
//! something and may report what happened; it may not decide on its own
//! what day it is.
//!
//! **What this is not, yet.** It does not own buildings, vehicles, work
//! orders, minds, relationships or utilities — those still live where they
//! live. The point of starting here is that each of those becomes a
//! mechanical migration rather than a rewrite, and every one of them is
//! independently testable on the way. A root that owns three things and
//! enforces one clock is worth more than a plan for a root that owns
//! everything.

use crate::econ::Economy;
use crate::item::Store as ItemStore;
use crate::patch::Overlay;
use crate::populace::Populace;

// =====================================================================
// the order of a day
// =====================================================================

/// **What happens in a day, in the order it happens.**
///
/// Written down rather than implied by the order somebody happened to call
/// things in a binary. The order is not arbitrary and two of the rules in
/// it were learned the hard way and are recorded elsewhere in this project:
///
/// - **The shops trade before anybody counts who worked.** `labour::update`
///   ran before the day's selling, so every shop read as shut and rostered
///   a third of its people, and a cashier could not keep a room.
/// - **A service is paid before wages fall due**, because a hospital cannot
///   meet today's payroll out of money it will be given this evening.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phase {
    /// Weather, season, and anything else the world does to itself.
    World,
    /// Generation, production, distribution, trade and prices.
    Economy,
    /// Who worked, who was paid, who was let go.
    Labour,
    /// Households buying, people living, minds moving.
    People,
    /// What any of that did to anybody, and the record of it.
    Consequences,
}

pub const PHASES: [Phase; 5] = [
    Phase::World,
    Phase::Economy,
    Phase::Labour,
    Phase::People,
    Phase::Consequences,
];

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Phase::World => "the world",
            Phase::Economy => "the economy",
            Phase::Labour => "work",
            Phase::People => "people",
            Phase::Consequences => "consequences",
        }
    }
}

// =====================================================================
// the root
// =====================================================================

/// **The live world.**
///
/// One clock, deliberately private: `day` has no setter and `advance` is
/// the only thing that moves it. A subsystem that wants to know the date
/// asks; it does not keep its own.
pub struct GameState {
    /// **The clock.** Private on purpose — see above.
    day: u64,
    /// What generated this world. Everything derived is derived from it.
    pub world_seed: u64,

    /// Firms, markets, prices, the grid, freight, the treasury.
    pub economy: Option<Economy>,
    /// Every physical object anybody owns or has made.
    pub items: ItemStore,
    /// The individuated sample of people, and their households.
    pub folk: Option<Populace>,
    /// What has been done to the ground that generation would not produce.
    pub ground: Overlay,

    /// What happened today, cleared at the start of each day. Not a
    /// history: the journal is for that.
    pub today: Vec<Happening>,
}

/// Something a system did that another system may care about.
///
/// **The handoff, and the reason systems do not call each other.** A model
/// that reaches into another to tell it what to think ends up holding a
/// copy of its state; one that reports a fact does not.
#[derive(Clone, Debug, PartialEq)]
pub struct Happening {
    pub day: u64,
    pub phase: Phase,
    pub what: String,
}

impl GameState {
    pub fn new(world_seed: u64) -> Self {
        GameState {
            day: 0,
            world_seed,
            economy: None,
            items: ItemStore::new(),
            folk: None,
            ground: Overlay::default(),
            today: Vec::new(),
        }
    }

    /// The date. There is one.
    pub fn day(&self) -> u64 {
        self.day
    }

    /// Attach an economy. It gives up its own clock in the process — its
    /// ledger is set to the world's date rather than starting again at nought.
    pub fn with_economy(mut self, mut economy: Economy) -> Self {
        economy.ledger.day = self.day;
        economy.told_the_day = None;
        self.economy = Some(economy);
        self
    }

    /// Attach a sample of people.
    pub fn with_folk(mut self, folk: Populace) -> Self {
        self.folk = Some(folk);
        self
    }

    /// **A day, in order.**
    ///
    /// The only thing that moves the clock, and it moves it once at the
    /// end — so every phase within a day sees the same date, which is what
    /// makes "what day is it" answerable at all.
    pub fn a_day(&mut self) {
        // **The clock moves to the day, then the day happens.** Running the
        // phases first and advancing afterwards leaves every system a day
        // behind the world for the whole of the day it is simulating, which
        // is the same disagreement in a smaller costume.
        self.day += 1;
        self.today.clear();
        for phase in PHASES {
            self.run(phase);
        }
        debug_assert!(self.clocks_agree(), "a subsystem is keeping its own time");
    }

    pub fn advance(&mut self, days: u64) {
        for _ in 0..days {
            self.a_day();
        }
    }

    fn run(&mut self, phase: Phase) {
        match phase {
            // Nothing owns the weather yet; the economy still rolls its own
            // inside `step`, and moving it is a later migration.
            Phase::World => {}
            Phase::Economy => {
                if let Some(e) = self.economy.as_mut() {
                    // **Told the day rather than counting it.**
                    //
                    // Correcting the ledger afterwards was the first
                    // attempt and it was worthless: both counters
                    // incremented by one, so they agreed by coincidence and
                    // the gate on "everybody agrees what day it is" passed
                    // with the correction deleted. An invariant that holds
                    // whether or not you enforce it is not being enforced.
                    let day = self.day;
                    e.step_at(day);
                }
            }
            // Labour runs inside the economy's step for now.
            Phase::Labour => {}
            Phase::People => {
                let day = self.day;
                if let (Some(folk), Some(e)) = (self.folk.as_mut(), self.economy.as_mut()) {
                    folk.live_a_day(e, day);
                }
            }
            Phase::Consequences => {}
        }
    }

    /// **Everybody agrees what day it is.**
    ///
    /// The single invariant this whole module exists to make checkable. It
    /// was not checkable before, because there was nothing that could be
    /// asked.
    pub fn clocks_agree(&self) -> bool {
        match self.economy.as_ref() {
            Some(e) => e.ledger.day == self.day,
            None => true,
        }
    }

    pub fn assert_clocks_agree(&self) {
        if let Some(e) = self.economy.as_ref() {
            assert_eq!(
                e.ledger.day, self.day,
                "the economy thinks it is day {} and the world thinks it is day {}",
                e.ledger.day, self.day
            );
        }
    }

    /// Note something worth another system knowing.
    pub fn note(&mut self, phase: Phase, what: impl Into<String>) {
        let day = self.day;
        self.today.push(Happening {
            day,
            phase,
            what: what.into(),
        });
    }

    /// **What can be saved of it today**, which is not yet all of it.
    ///
    /// Stated as a count rather than a boast: the save carries the clock,
    /// the seed and the ground overlay, and does not yet carry the economy,
    /// the item store or the people. Each of those is a migration, and this
    /// number going up is how the migration is measured.
    pub fn saveable_parts(&self) -> (usize, usize) {
        let owned = 4; // clock, seed, ground, items
        let saved = 3; // clock, seed, ground
        (saved, owned)
    }
}
