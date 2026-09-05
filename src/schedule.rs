//! **The scheduler does not model crafting. It allocates the three kinds
//! of time crafting already has.**
//!
//! `craft.rs` knows that a loaf is 35 minutes of labour, 35 minutes of
//! oven and 125 minutes on the clock. What it cannot say is whether the
//! baker can shape the next batch while the first one proves, whether the
//! oven is free, or what happens to a tray that is in it when the power
//! goes off. That is this module, and it is a **calendar** rather than a
//! second crafting model:
//!
//! ```text
//! recipe → work order → reservations → execution and interruption
//!        → outputs, rework and scrap
//! ```
//!
//! **Deterministic earliest-feasible**, and deliberately nothing cleverer.
//! Factory-wide optimisation is a different problem and a much later one;
//! what is wanted first is that two jobs cannot have the same bench at the
//! same moment, and that the answer does not depend on when anybody
//! happened to look.

use crate::craft::{
    roll_outcome, Effort, Grade, Maker, Need, RecipeBook, Step, WorkOrder, Workplace, Yields,
};
use crate::item::{Capability, Catalogue, Provides};

// =====================================================================
// what a shop has
// =====================================================================

/// **A place at which one job can be under way.**
///
/// A bench, a saw, an oven, a proving rack. Capacity is what separates
/// them: a bandsaw does one thing at a time and an oven takes four trays,
/// and a model with only "in use" cannot say so.
#[derive(Clone, Debug)]
pub struct Station {
    pub name: &'static str,
    pub provides: Vec<Provides>,
    /// How many jobs at once. One for a hand tool, four for an oven, and
    /// a proving rack is as many as it has shelves.
    pub capacity: u32,
    /// Continuous draw while it is working.
    pub kw: f64,
}

impl Station {
    pub fn new(name: &'static str, provides: Vec<Provides>) -> Self {
        let kw = provides.iter().map(|p| p.kw).fold(0.0f64, f64::max);
        Station { name, provides, capacity: 1, kw }
    }

    pub fn holding(mut self, capacity: u32) -> Self {
        self.capacity = capacity;
        self
    }

    pub fn can(&self, n: Need, powered: bool) -> bool {
        self.provides
            .iter()
            .any(|p| p.satisfies(n.capability, n.capacity, n.precision_mm))
            && (powered || self.kw <= 0.0)
    }

    fn speed_for(&self, n: Need) -> f64 {
        self.provides
            .iter()
            .filter(|p| p.satisfies(n.capability, n.capacity, n.precision_mm))
            .map(|p| p.speed)
            .fold(1.0f64, f64::max)
    }
}

/// Somebody who can be booked.
#[derive(Clone, Copy, Debug)]
pub struct Worker {
    pub person: u64,
    pub maker: Maker,
    /// **An owner-operator is still an hour of somebody's day.** Whether
    /// a wage is paid is an accounting question and does not change the
    /// calendar.
    pub paid: bool,
}

impl Worker {
    pub fn new(person: u64, maker: Maker) -> Self {
        Worker { person, maker, paid: true }
    }

    pub fn owner(person: u64, maker: Maker) -> Self {
        Worker { person, maker, paid: false }
    }
}

/// What is booked, and when.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Booked {
    Worker(usize),
    Station(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Booking {
    pub what: Booked,
    pub from: f64,
    pub to: f64,
    pub order: u64,
    pub step: usize,
    pub attempt: u32,
}

/// **Who has what, when.** Nothing more: the calendar refuses a clash and
/// has no opinion about anything else.
#[derive(Clone, Debug, Default)]
pub struct Calendar {
    bookings: Vec<Booking>,
}

impl Calendar {
    /// How many of a thing are booked across an interval. Overlap is
    /// half-open, so a job ending at 60 and one starting at 60 do not
    /// clash.
    pub fn load(&self, what: Booked, from: f64, to: f64) -> u32 {
        self.bookings
            .iter()
            .filter(|b| b.what == what && b.from < to - 1e-9 && from < b.to - 1e-9)
            .count() as u32
    }

    pub fn free(&self, what: Booked, from: f64, to: f64, capacity: u32) -> bool {
        self.load(what, from, to) < capacity
    }

    pub fn book(&mut self, b: Booking) {
        self.bookings.push(b);
    }

    /// Every moment at which something is given up, from `after` onward.
    /// The earliest-feasible search only ever needs to try these.
    fn release_times(&self, after: f64) -> Vec<f64> {
        let mut t: Vec<f64> = self
            .bookings
            .iter()
            .map(|b| b.to)
            .filter(|&x| x > after + 1e-9)
            .collect();
        t.push(after);
        t.sort_by(f64::total_cmp);
        t.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        t
    }

    pub fn bookings(&self) -> &[Booking] {
        &self.bookings
    }

    /// Drop everything booked for an order from a time onward — what a
    /// replan does, and it must not touch what has already happened.
    pub fn release_from(&mut self, order: u64, at: f64) {
        self.bookings.retain(|b| !(b.order == order && b.from >= at - 1e-9));
    }
}

/// A shop: the people, the equipment, the supply, and the diary.
#[derive(Clone, Debug)]
pub struct Shop {
    pub id: u32,
    pub workers: Vec<Worker>,
    pub stations: Vec<Station>,
    /// What the incoming supply will carry. A shop can have more machines
    /// than it can run at once, which is a real constraint and a
    /// different one from not having the machine.
    pub power_kw: f64,
    pub powered: bool,
    /// Jigs and fixtures, which `craft.rs` reads.
    pub jigs: f64,
    pub calendar: Calendar,
}

impl Shop {
    pub fn new(id: u32, workers: Vec<Worker>, stations: Vec<Station>) -> Self {
        Shop {
            id,
            workers,
            stations,
            power_kw: 50.0,
            powered: true,
            jigs: 0.1,
            calendar: Calendar::default(),
        }
    }

    /// The `Workplace` that `craft.rs` wants, derived from the shop rather
    /// than described twice. **One physics, two views of it.**
    pub fn as_workplace(&self) -> Workplace {
        Workplace {
            power: self.powered,
            water: true,
            celsius: 18.0,
            tools: self.stations.iter().flat_map(|s| s.provides.iter().copied()).collect(),
            workers: self.workers.len() as f64,
            stations: self.stations.iter().map(|s| s.capacity).max().unwrap_or(1),
            jigs: self.jigs,
        }
    }

    fn station_for(&self, n: Need) -> Option<usize> {
        self.stations.iter().position(|s| s.can(n, self.powered))
    }

    /// **Doing the work costs the person doing it.**
    ///
    /// An hour at a bench is an hour of somebody's day whether or not
    /// anybody is paid for it, and it leaves them more tired than they
    /// were. Real: a working day is seven to nine hours and people are
    /// measurably worse at the end of one.
    pub fn worked(&mut self, worker: usize, minutes: f64) {
        if let Some(w) = self.workers.get_mut(worker) {
            // A nine-hour day takes somebody from fresh to spent.
            w.maker.fatigue = (w.maker.fatigue + minutes / 540.0).clamp(0.0, 1.0);
        }
    }

    /// And it wears the tool. A blade is duller at the end of the run.
    pub fn tool_wear(&mut self, station: usize, minutes: f64) {
        if let Some(s) = self.stations.get_mut(station) {
            for p in &mut s.provides {
                // A thousand hours takes a tool from new to needing
                // attention, which is about a working year of one machine.
                p.precision_mm *= 1.0 + minutes / 60_000.0;
            }
        }
    }

    /// A rest puts some of it back.
    pub fn rested(&mut self, worker: usize, minutes: f64) {
        if let Some(w) = self.workers.get_mut(worker) {
            w.maker.fatigue = (w.maker.fatigue - minutes / 720.0).clamp(0.0, 1.0);
        }
    }
}

// =====================================================================
// what an interruption does
// =====================================================================

/// **A blackout does not have one universal result.**
///
/// An electric saw stops and picks up where it left off. A kiln cools and
/// has to be brought back. Food in an oven goes on changing whether or not
/// the element is on. A weld interrupted half way is a weld that has to be
/// done again. Glue does not care.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OnInterruption {
    /// Stops, and resumes from where it stopped.
    Resume,
    /// Carries on regardless — curing, proving, cooling.
    Unaffected,
    /// Has to be started again from the beginning of the operation.
    Restart,
    /// The equipment loses its state and needs bringing back before the
    /// work can go on.
    LosesState { recovery_min: f64 },
    /// The workpiece goes on changing while nobody is doing anything to
    /// it, and after a while it is spoiled.
    ContinuesChanging { spoils_after_min: f64 },
}

impl OnInterruption {
    /// What a step's kind implies, before anybody says otherwise. A
    /// resting step is unaffected; a joining step has to be redone; an
    /// oven loses its heat.
    pub fn of(step: &Step) -> Self {
        use crate::craft::Operation::*;
        match step.operation {
            Rest => OnInterruption::Unaffected,
            Bake => OnInterruption::ContinuesChanging { spoils_after_min: 45.0 },
            Weld | Solder | Cast | Forge => OnInterruption::Restart,
            HeatTreat => OnInterruption::LosesState { recovery_min: 40.0 },
            _ => OnInterruption::Resume,
        }
    }
}

// =====================================================================
// the plan
// =====================================================================

/// One operation, booked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub order: u64,
    pub step: usize,
    pub attempt: u32,
    pub start: f64,
    pub end: f64,
    /// Which worker is on it, if anybody has to be.
    pub worker: Option<usize>,
    pub station: Option<usize>,
    /// Minutes of somebody's day this actually costs — which is not the
    /// same as `end - start`.
    pub labour_min: f64,
    pub machine_min: f64,
    pub policy: OnInterruption,
    /// Set once it has actually been worked through.
    pub done: bool,
}

impl Slot {
    pub fn elapsed(&self) -> f64 {
        self.end - self.start
    }
}

/// Why an operation could not be booked at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unschedulable {
    NoSuchOrder,
    NoWorker,
    /// The shop does not have anything that can do it — as opposed to
    /// having it and being busy.
    NoStation(Capability),
    NoPower,
}

/// **The diary for one work order.**
#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub slots: Vec<Slot>,
}

impl Plan {
    pub fn finish(&self) -> f64 {
        self.slots.iter().map(|s| s.end).fold(0.0f64, f64::max)
    }

    pub fn labour_min(&self) -> f64 {
        self.slots.iter().map(|s| s.labour_min).sum()
    }

    pub fn machine_min(&self) -> f64 {
        self.slots.iter().map(|s| s.machine_min).sum()
    }

    /// Wall clock from the first thing starting to the last thing ending.
    pub fn elapsed_min(&self) -> f64 {
        let start = self.slots.iter().map(|s| s.start).fold(f64::MAX, f64::min);
        if self.slots.is_empty() {
            0.0
        } else {
            self.finish() - start
        }
    }

    /// What one person is actually on, at a moment.
    pub fn who_is_busy(&self, at: f64) -> Vec<usize> {
        self.slots
            .iter()
            .filter(|s| s.start <= at + 1e-9 && at < s.end - 1e-9)
            .filter_map(|s| s.worker)
            .collect()
    }
}

/// **Book a work order into a shop, earliest feasible.**
///
/// Walks the operations in order — they are a sequence, and a plan whose
/// steps could be shuffled is a different kind of plan — and for each
/// finds the first moment at which everything it needs is free at once.
pub fn book(
    shop: &mut Shop,
    order: &WorkOrder,
    book_of_recipes: &RecipeBook,
    not_before: f64,
    units: u32,
) -> Result<Plan, Unschedulable> {
    // **A plan that cannot be finished books nothing.**
    //
    // Booking the setup and then discovering there is no saw would leave
    // the shop with a reservation for work that will never happen — and
    // the next order would queue behind a ghost.
    match try_book(shop, order, book_of_recipes, not_before, units) {
        Ok(plan) => Ok(plan),
        Err(e) => {
            shop.calendar.release_from(order.id, not_before);
            Err(e)
        }
    }
}

fn try_book(
    shop: &mut Shop,
    order: &WorkOrder,
    book_of_recipes: &RecipeBook,
    not_before: f64,
    units: u32,
) -> Result<Plan, Unschedulable> {
    let recipe = book_of_recipes.get(order.recipe).ok_or(Unschedulable::NoSuchOrder)?;
    let mut plan = Plan::default();
    let mut t = not_before;

    // **Setup happens once**, whatever the batch size. It is somebody's
    // time and it needs a person.
    if recipe.setup_minutes > 0.0 {
        let slot = fit(shop, order.id, usize::MAX, 0, recipe.setup_minutes, 0.0, &[], t, true,
                       OnInterruption::Resume)?;
        t = slot.end;
        plan.slots.push(slot);
    }

    let runs = (units.max(1) as f64 / recipe.yields.max(1) as f64).ceil() as u32;
    for run in 0..runs {
        for (i, step) in recipe.steps.iter().enumerate() {
            let policy = OnInterruption::of(step);
            let (labour, machine, span, needs_body) = match step.effort {
                Effort::Hands { minutes } => (minutes, 0.0, minutes, true),
                Effort::Machine { minutes, tending } => (tending, minutes, minutes, false),
                Effort::Unattended { minutes } => (0.0, 0.0, minutes, false),
            };
            let slot = fit(
                shop,
                order.id,
                i,
                run,
                span,
                machine,
                &step.needs,
                t,
                needs_body || labour > 0.0,
                policy,
            )?;
            // **A sequence.** The next operation cannot start before this
            // one has finished, whoever is free.
            t = slot.end;
            let mut slot = slot;
            slot.labour_min = labour;
            slot.machine_min = machine;
            plan.slots.push(slot);
        }
    }
    Ok(plan)
}

/// Find the first window in which everything an operation needs is free,
/// and take it.
#[allow(clippy::too_many_arguments)]
fn fit(
    shop: &mut Shop,
    order: u64,
    step: usize,
    attempt: u32,
    span: f64,
    machine_min: f64,
    needs: &[Need],
    not_before: f64,
    needs_worker: bool,
    policy: OnInterruption,
) -> Result<Slot, Unschedulable> {
    // Which station could do it at all — a shop that has no bandsaw is a
    // different problem from a shop whose bandsaw is busy.
    let mut station = None;
    for n in needs {
        match shop.station_for(*n) {
            Some(k) => station = Some(k),
            None => {
                return Err(if !shop.powered {
                    Unschedulable::NoPower
                } else {
                    Unschedulable::NoStation(n.capability)
                })
            }
        }
    }
    if needs_worker && shop.workers.is_empty() {
        return Err(Unschedulable::NoWorker);
    }

    // The work goes faster with a better tool and a better hand, which is
    // what makes the *duration* a property of who does it.
    let speed = match (station, needs.first()) {
        (Some(k), Some(n)) => shop.stations[k].speed_for(*n),
        _ => 1.0,
    };
    let pace = shop.workers.first().map(|w| w.maker.pace()).unwrap_or(1.0);
    let duration = if needs_worker {
        (span / (speed * pace).max(0.05)).max(1e-6)
    } else {
        span
    };

    for t in shop.calendar.release_times(not_before) {
        let end = t + duration;
        let worker = if needs_worker {
            (0..shop.workers.len())
                .find(|&w| shop.calendar.free(Booked::Worker(w), t, end, 1))
        } else {
            None
        };
        if needs_worker && worker.is_none() {
            continue;
        }
        if let Some(k) = station {
            if !shop.calendar.free(Booked::Station(k), t, end, shop.stations[k].capacity) {
                continue;
            }
        }
        if let Some(w) = worker {
            shop.calendar.book(Booking { what: Booked::Worker(w), from: t, to: end, order, step, attempt });
        }
        if let Some(k) = station {
            shop.calendar.book(Booking { what: Booked::Station(k), from: t, to: end, order, step, attempt });
        }
        return Ok(Slot {
            order,
            step,
            attempt,
            start: t,
            end,
            worker,
            station,
            labour_min: if needs_worker { duration } else { 0.0 },
            machine_min,
            policy,
            done: false,
        });
    }
    Err(Unschedulable::NoWorker)
}

// =====================================================================
// running it, and being interrupted
// =====================================================================

/// What happened to an operation that was under way when the supply
/// failed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interrupted {
    pub order: u64,
    pub step: usize,
    /// How far through it was, in minutes of its own span.
    pub progress_min: f64,
    pub policy: OnInterruption,
    /// What has to be redone: zero for a resume, all of it for a restart.
    pub lost_min: f64,
    pub spoiled: bool,
}

/// **Cut the power at an instant and see what it costs.**
///
/// The split happens at the exact time, not at the end of the step and
/// not at the end of the day — and what it costs depends on the
/// operation's own policy rather than on one universal rule.
pub fn interrupt(plan: &mut Plan, at: f64, until: f64) -> Vec<Interrupted> {
    let mut hit = Vec::new();
    for s in plan.slots.iter_mut() {
        if s.done || s.start >= at - 1e-9 || s.end <= at + 1e-9 {
            continue;
        }
        let progress = at - s.start;
        let outage = until - at;
        let (lost, spoiled) = match s.policy {
            OnInterruption::Resume => (0.0, false),
            OnInterruption::Unaffected => (0.0, false),
            OnInterruption::Restart => (progress, false),
            OnInterruption::LosesState { recovery_min } => (recovery_min, false),
            OnInterruption::ContinuesChanging { spoils_after_min } => {
                (0.0, outage > spoils_after_min)
            }
        };
        hit.push(Interrupted {
            order: s.order,
            step: s.step,
            progress_min: progress,
            policy: s.policy,
            lost_min: lost,
            spoiled,
        });
        // **Unaffected work keeps its slot; everything else is pushed
        // out by the outage plus whatever it lost.**
        if s.policy != OnInterruption::Unaffected {
            s.end += outage + lost;
        }
    }
    // Everything downstream moves with it.
    let push: f64 = hit
        .iter()
        .map(|h| (until - at) + h.lost_min)
        .fold(0.0f64, f64::max);
    if push > 0.0 {
        for s in plan.slots.iter_mut() {
            if !s.done && s.start >= at - 1e-9 {
                s.start += push;
                s.end += push;
            }
        }
    }
    hit
}

/// **Work the plan up to a moment.** Anything that has finished by then is
/// marked done and stays done: resuming must not redo it.
pub fn advance_to(plan: &mut Plan, now: f64) {
    for s in plan.slots.iter_mut() {
        if s.end <= now + 1e-9 {
            s.done = true;
        }
    }
}

/// How much of somebody's day the plan has actually cost by a moment.
pub fn labour_spent_by(plan: &Plan, now: f64) -> f64 {
    plan.slots
        .iter()
        .filter(|s| s.labour_min > 0.0)
        .map(|s| {
            if s.end <= now {
                s.labour_min
            } else if s.start >= now {
                0.0
            } else {
                s.labour_min * ((now - s.start) / s.elapsed().max(1e-9))
            }
        })
        .sum()
}

/// **A machine that fails leaves work in progress**, not a lost order and
/// certainly not two outputs. What it takes is a replan from the moment it
/// broke, with everything already finished left alone.
pub fn machine_failed(
    shop: &mut Shop,
    plan: &mut Plan,
    station: usize,
    at: f64,
) -> Vec<Slot> {
    advance_to(plan, at);
    let order = plan.slots.first().map(|s| s.order).unwrap_or(0);
    shop.calendar.release_from(order, at);
    // The station is out; anything booked on it after now has to go
    // somewhere else or wait.
    let stranded: Vec<Slot> =
        plan.slots.iter().copied().filter(|s| !s.done && s.station == Some(station)).collect();
    plan.slots.retain(|s| s.done || s.station != Some(station));
    stranded
}

/// A worker walks out. **Active personal work stops; autonomous
/// processing carries on**, which is the whole reason the three kinds of
/// time are separate.
pub fn worker_leaves(plan: &mut Plan, worker: usize, at: f64) -> (usize, usize) {
    let mut stopped = 0;
    let mut carried_on = 0;
    for s in plan.slots.iter_mut() {
        if s.done || s.end <= at + 1e-9 {
            continue;
        }
        if s.worker == Some(worker) && s.start <= at + 1e-9 {
            stopped += 1;
        } else if s.worker.is_none() && s.start <= at + 1e-9 {
            carried_on += 1;
        }
    }
    (stopped, carried_on)
}

/// **What a batch costs, and what it does not save.**
///
/// Setup once; per-unit labour and per-unit material untouched. The
/// scheduler's version of the rule `craft.rs` already holds, and stated in
/// units that cannot be misread: 500 chairs are 950 hours in total and
/// 1.9 hours each — never "500 chairs take 1.9 hours".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BatchCost {
    pub units: u32,
    pub total_labour_min: f64,
    pub per_unit_labour_min: f64,
    pub setup_min: f64,
}

pub fn batch_cost(book_of_recipes: &RecipeBook, recipe: usize, units: u32) -> Option<BatchCost> {
    let r = book_of_recipes.get(recipe)?;
    let total = r.labour_for_batch(units);
    Some(BatchCost {
        units,
        total_labour_min: total,
        per_unit_labour_min: total / units.max(1) as f64,
        setup_min: r.setup_minutes,
    })
}

/// **A batch has two kinds of fault, and they are not the same kind.**
///
/// Setting the jig up wrong spoils the whole run; a slip on the ninetieth
/// unit spoils the ninetieth unit. A model with one roll per batch cannot
/// have the first, and a model with only per-unit rolls cannot have the
/// second.
#[derive(Clone, Debug, PartialEq)]
pub struct BatchOutcome {
    /// How the setting-up went. If this is not `Accepted`, every unit in
    /// the run carries it.
    pub setup: Grade,
    pub units: Vec<Grade>,
}

impl BatchOutcome {
    pub fn accepted(&self) -> usize {
        if self.setup != Grade::Accepted {
            return 0;
        }
        self.units.iter().filter(|g| **g == Grade::Accepted).count()
    }

    pub fn scrapped(&self) -> usize {
        if self.setup == Grade::Scrapped {
            return self.units.len();
        }
        self.units.iter().filter(|g| **g == Grade::Scrapped).count()
    }
}

/// Roll a whole run: the setup once, then each unit on its own. Keyed the
/// same way every other outcome in this project is, so a reload cannot
/// change it.
pub fn batch_outcome(order: u64, units: u32, yields: Yields, hazard: f64) -> BatchOutcome {
    let (setup, _) = roll_outcome(order, usize::MAX, 0, yields, hazard);
    let unit_grades = (0..units)
        .map(|u| roll_outcome(order, u as usize, 0, yields, hazard).0)
        .collect();
    BatchOutcome { setup, units: unit_grades }
}

/// A station built from a named tool in the catalogue, so a shop is
/// described in the same terms as everything else.
pub fn station(cat: &Catalogue, name: &'static str) -> Option<Station> {
    let d = cat.get(cat.named(name)?)?;
    Some(Station::new(name, d.provides.clone()))
}
