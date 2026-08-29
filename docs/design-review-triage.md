# Design Review — Triage & Proposed Resolutions

An external review (2026-08-29) found that the design doc is a strong
*simulation manifesto* but not yet a coherent *system specification*: the
individual features are fine, the connective rules that make them one
simulation are missing.

This document triages every point raised, tags it, and — for the things
that need deciding before we build further — proposes a resolution for
Peter to ratify, adjust, or reject. Nothing here is settled until the
Decision Log says so.

**Status tags**
| Tag | Meaning |
|---|---|
| `DECIDE-NOW` | Architectural or directional; expensive to retrofit. Needs a call before the next build phase. |
| `RULE` | A standing principle to adopt now; cheap, shapes everything downstream. |
| `DONE` | Already handled in code; the design doc text is just stale. |
| `DOC-FIX` | Design doc wording is now inaccurate; listed for merging. |
| `SPEC-LATER` | Real work, but correctly deferred. Spec it when we build that system. Has a trigger. |

---

## Decision Log

**Worked through 2026-08-29. All `DECIDE-NOW` and `RULE` items in Parts 1–3
are now resolved in [`implementation-spec.md`](implementation-spec.md)** —
sections A1, A2, A3, A4, B1–B7, C1 are marked SETTLED there. This triage
doc is kept as the record of *how* the review was categorised; the spec is
the source of truth for *what was decided*.

Still open (tracked in the spec):
- B3.6 — starting region rolled vs chosen (undecided).
- A1.10 — `K` (the "relevant" region radius), z-level extent: pending
  measurement.
- B4.8 — per-region live individuation cap: pending profiling.
- Economy and infrastructure-network specs: still `SPEC-LATER`, needed
  before the vertical slice.

---

## Part 1 — Architectural decisions (`DECIDE-NOW`)

### A1. Spatial & temporal hierarchy

**Proposed hierarchy:**
`universe → star system → planet → region → locale → chunk → tile → z-level`

| Level | Scale | Role | Simulation tier |
|---|---|---|---|
| tile | ~1 m² | atomic physical unit; the character stands on one | physical |
| chunk | 32×32 tiles | unit of load/unload and local procedural gen | physical |
| locale | a town / farm / fort / mine / road segment | what civ & economy attach to; has a persistent gen seed + delta set | physical when visited, ledger otherwise |
| region | one pixel of the current planetary map (~tens of km) | holds climate, biome, terrain, list of locales | ledger / aggregate |
| planet | the 768×432 region grid, cylinder-wrapped | — | — |
| star system, universe | deferred — slots reserved only | — | — |

**Two map modes:**
- **Strategic map** (region level): travel is abstracted — pick a
  destination, time advances, cost = distance × terrain × transport ×
  route risk. This is how long journeys happen without real-world hours.
- **Local map** (chunk/tile level): the reified reality bubble,
  real-time-with-pause. Entered on arrival or when something forces
  reification. Transition between modes is explicit (M&B style).
- **Reality bubble size:** a radius of chunks around the player (start
  ~3×3 chunks ≈ 96×96 tiles) plus the locale you're in. Tunable.

**Time:**
- **tick** — base step inside the reality bubble (a fraction of a
  game-second).
- **region tick** — cheap aggregate update for *relevant* regions, once
  per game-hour or game-day.
- **catch-up** — on interacting with a frozen region, run its region-tick
  formula in a tight loop for the elapsed span. Never simulate
  individuals during catch-up.
- **calendar** — fixed year length, 4 seasons (drive farming, weather,
  temperature swing), 24-hour days (drive shift work, visibility).

**"Nearby / relevant" definition:**
- *active* — player is in the region.
- *relevant* — within K regions of the player, **or** contains an
  individuated NPC in the player's relationship web, **or** part of an
  event chain the player is entangled in.
- *frozen* — everything else.

**Cross-region events** (armies, refugee columns, caravans, epidemics):
owned by a **mobile entity** on the strategic map that ticks at region
cadence and reifies on encounter. Not "processed by both regions" — it is
its own thing that moves between them.

### A2. Single source-of-truth model

*The review calls this the most important missing section. Agreed.*

Three layers, strict ownership — a quantity is owned by **exactly one**
layer at a time:

1. **Macro ledger** — authoritative state for anything not in the reality
   bubble. Conserved quantities: population by cohort, goods stockpiles by
   commodity, money/accounts, property ownership, org rosters (as counts +
   seeded roster spec), infrastructure capacity/damage, ongoing
   activities. One row per region and per locale. **This is what saves.**
2. **Event journal** — append-only, timestamped log of *changes* to the
   ledger and of narrative facts (battle, fire, casualties, who did what).
   Explains how the ledger reached its state; seeds reification. Source of
   truth for *history/story*, **not** for quantities.
3. **Physical realization** — the reified local map. Built from **(a)** a
   persistent per-locale generation seed that deterministically rebuilds
   the baseline geography + buildings, plus **(b)** a delta set of every
   meaningful change since (wall breached, building burned, crater, corpse,
   dropped item, ownership change). Realize = regenerate baseline → apply
   deltas → apply a decay template keyed to time since the driving events.

**Transfer rules:**
- **Reify (ledger → physical):** local map draws population/goods/
  ownership from the ledger; named people materialize from the seeded
  roster spec; ledger row flagged "reified" so it is not also ticked
  abstractly.
- **De-reify (physical → ledger):** on player exit, diff the local map
  against what was reified; write the differences back as a ledger delta
  and journal events; resume abstract ticking.
- **Never double-count:** the handoff is atomic; one owner at a time.

*Refugee-column example:* a mobile entity with a ledger row (500 people by
cohort, 30 vehicles, 4 days food, injury stats, roster spec naming ~8
people plus anyone already individuated). Reifies to 500 bodies + named
people + vehicles. Player intervenes. On exit: diff → "473 people, 26
vehicles, 2 days food, +12 individuated" written back. Nothing invented,
nothing lost.

### A3. Information & belief model

Five levels:

| Level | What it is |
|---|---|
| ground truth | what the ledger / physical layer says actually happened |
| observation | what an entity perceived — gated by senses, line-of-sight, presence |
| report | an observation transmitted over working comms; has source, timestamp; can be lossy or delayed |
| belief | an entity's / faction's current world-model, built from its observations + received reports. **The decision primitive reads this**, never ground truth. Goes stale; can be wrong. |
| deception | a deliberately false report — spy, propaganda, feint. A report with a false payload and hidden true source. |

- **Player knowledge = the player's belief layer.** The map you see is
  your belief, not ground truth. Same mechanism as NPCs.
- **Cheap implementation:** abstract NPCs share one belief store per
  faction per region; only individuated NPCs get personal divergence.
- Prices, ambushes, diplomacy, military orders, investigations all read
  belief.

### A4. Decisions → actions (the planner layer)

The decision primitive evaluates value-vs-risk but does not generate
opportunities or plans. Between it and the world:

- **Opportunity sources** — opportunities are *pushed* by systems, not
  scanned: a job board posts contracts, a faction AI proposes strategic
  options, a detected convoy raises "raid it" for a nearby gang, a vacancy
  opens a promotion. Each entity holds a small **opportunity queue**,
  refreshed on events.
- **Goal selection** — run the primitive over the queue + current goal;
  keep the current goal unless something clearly beats it (hysteresis —
  switching has a cost).
- **Planning** — a goal expands into a plan via a library of
  **hierarchical task templates** (goal → subtasks → primitive actions),
  generalizing the military hierarchical-intent idea to everything. Plans
  persist across ticks; entities execute plans, they don't re-decide every
  tick.
- **Reservations** — committing to a resource (job slot, tool, vehicle,
  stockpile) places an expiring reservation so planners don't collide.
- **Reconsideration triggers** — re-run goal selection only on: plan step
  done, plan failed, new high-salience event in belief, or a slow boredom
  timer.
- **Think budget** — each region gets a per-tick budget of "agent
  thinks." Bubble individuals think often; nearby cohorts think as a
  group; frozen regions don't think (formulas only).

---

## Part 2 — Directional decisions (`DECIDE-NOW`)

### B1. The player loop

**Repeatable loop (Mount & Blade shape):** you are an individual with
needs (hunger/thirst/rest/health/money), skills, an inventory, and a
location. You take **work** — a contract from an org or person (haul
cargo, guard a caravan, work a factory shift, do a repair job, run a
training). Work pays, consumes time, exercises skills, exposes you to
events. Wages buy equipment, supplies, training, property, shares.

- **Skills** rise with use and deliberate training; some roles need
  **certification** (skill threshold + a credential from a guild / org /
  state).
- **Advancement** — worker → supervisor → manager → director →
  owner/commander → office-holder, by being the best-positioned candidate
  when a **real vacancy** opens (org system already specced). Each rung =
  more authority, more delegation.
- **Delegation** — past a scale threshold you stop issuing primitive
  actions and issue **intent** to subordinates (reuse the military
  hierarchical-intent mechanism for civilian orgs too). Commanding
  millions = setting policy/priorities for a hierarchy that runs on the
  A4 planner layer, with reports and exceptions surfaced to you.

### B2. Succession / aging / the "one lifetime" problem

*"Individual to intergalactic" can't fit one ordinary lifetime.*

**Proposed: all levers, layered.**
1. **Variable, player-controlled time compression.** Strategic travel and
   waiting fast-forward; the reality bubble runs near real-time. Decades
   of career are playable because most of it runs compressed.
2. **Real aging and death.** You age, decline, die. Family, heirs, and
   organizations persist.
3. **Play continues as a successor** you positioned — a child, protégé,
   lieutenant, the next office-holder. Position / reputation / property
   pass per the relevant institution's succession rules, *not*
   automatically. Grooming a successor and institutionalizing your
   influence *before* death is how continuity is earned.
4. **Longevity tech** is a late-game investment, not the baseline —
   life-extension becomes available as tech advances, letting one
   character persist into the interstellar era if the player pays for it.

### B3. Sandbox goals & failure states

**Proposed:** open sandbox, **no imposed win condition**. Real failure
states: death with no successor → new random character (soft restart);
bankruptcy / imprisonment / exile → setbacks, not game-overs. **Emergent
milestones** the game recognizes and logs (first company owned, first
office held, a war won by your faction, FTL discovered by your faction,
first interstellar colony) — acknowledgement, not victory.

### B4. Individuation bounding

*Permanent individuation is unbounded — casualties in a 40,000-person army
could create tens of thousands of permanent records.*

**Proposed:**
- Forces/populations are **counts + a seeded roster spec** (seed + gen
  rules), never instantiated wholesale.
- Individuation materializes **only the people the trigger touches** — the
  commander you inspect; the N casualties in *this* engagement; the rival
  candidates for *this* vacancy.
- Materialized individuals are persistent but **archivable**: one long
  uninteracted-with and not in an active relationship web compresses to a
  compact record (identity, key traits, status, location, memory digest) —
  enough to rehydrate consistently, not a live thought-log. LRU eviction
  from the live set; never outright deletion.
- Hard cap on the live individuated set per region; overflow compresses
  coldest-first.

### B5. FTL timeline

*Unclear whether gameplay begins before / at / after the breakthrough;
"run history until FTL" conflicts with space being deferred.*

**Proposed:**
- Gameplay **begins pre-FTL**, at the Information Age baseline.
- The initial history sim runs a **bounded span** (decades to ~2
  centuries — a knob) and **stops before FTL by construction**.
- FTL is a **late-game emergent milestone**, discovered during play via
  the individual-breakthrough mechanism — probability arising from
  **career prerequisites** (education, access to prior tech, institutional
  backing, funding, traits), not a pure birth lottery (adopts the
  reviewer's fix).
- "Space is deferred" = we don't *build* star systems / aliens / ship
  interiors yet. Not that FTL is unreachable. It slots into the reserved
  `star system → universe` levels when built.

---

## Part 3 — Standing rules to adopt (`RULE`)

| # | Rule |
|---|---|
| R1 | **Conservation of goods & people.** Quantities enter and leave the macro ledger via journalled deltas. Nothing appears or vanishes through statistical adjustment. |
| R2 | **Infrastructure is a graph, not a flag.** Every utility is source → transport → distribution → consumer, with capacity, load, and rerouting. You must be able to ask *which* source feeds a tile and whether the surviving network has spare capacity. |
| R3 | **Perception before decision.** An actor decides on its *belief*, never on ground truth. |
| R4 | **Existence is persistent; detail is lazy.** Distant places and entities always exist and their state advances (lazily, by formula); only their physical detail is generated on demand. |
| R5 | **Baseline seed + deltas.** Any regenerable thing (a town's buildings, a region's terrain) is stored as a generation seed plus persistent changes — never a full snapshot, never narrative log alone. |

---

## Part 4 — Already handled / doc fixes

### `DONE` — Climate is not independent noise

The review says rain shadows won't emerge from independent rainfall noise,
and temperature needs latitude + elevation. **The code already does this:**
`generate_rainfall` runs prevailing-wind moisture advection with
orographic lift and evapotranspiration recycling; `generate_temperature`
is a latitude band minus an altitude lapse rate. Rain shadows emerge from
the physics, as intended.

### `DOC-FIX` — design doc lines to merge

| Location | Current text | Change to |
|---|---|---|
| Pipeline step 2 | "Climate fields, generated independently ... not derived from each other or elevation directly" | "Climate fields — temperature from latitude + altitude; rainfall from prevailing-wind moisture advection over the elevation field with orographic lift; drainage from soil permeability + terrain runoff. Generated as *separate systems*, not as one field derived from another, but each may read elevation." |
| Pipeline step 2 | (add) | Split **drainage** into *soil drainage* (permeability, a real independent field) and *runoff* (derived from slope/flow). Currently merged. `SPEC-LATER`, trigger: hydrology. |
| Simulation Fidelity, *Distant* bullet | "frozen, just a seed + timestamp, until interaction triggers catch-up" | "existence is persistent; development is lazily resolved — the region keeps a ledger row + last-tick timestamp and is caught up by formula on interaction (rule R4)" |
| FTL discovery | "born with the circumstances/ability" | add: breakthrough probability accrues over a career from education, prior tech, institutional support, funding, and traits — birth sets potential, not outcome (B5) |

---

## Part 5 — Spec when we build it (`SPEC-LATER`)

| Area | What's needed | Build trigger |
|---|---|---|
| Economic accounting | commodity defs (unit, quality), production recipes + time, stockpiles + storage limits, labor/wages/operating costs/unemployment, ownership/accounts/budgets/loans/bankruptcy, price formation, cargo capacity/fuel/travel-time/route-risk, business formation/expansion/closure, taxes + government purchasing. Sits on the A2 ledger; obeys R1. | vertical slice (economy half) |
| Population & demographics | birth/death/aging/migration, households + dependants, housing allocation + homelessness, education → skill supply, class/culture/political affiliation. Cohorts live in the A2 ledger. | first settlement sim |
| Law & governance | property rights + jurisdiction, crime detection/evidence/arrest/courts/punishment, government formation/elections/coups/policy. | playable crime or politics |
| Infrastructure networks | generation→transmission→distribution→consumer; water source→treatment→storage→pressure; sewage collection + treatment; comms towers/range/backhaul/congestion; capacity/load/damage/isolation/rerouting; maintenance/spares/deterioration. Obeys R2. | vertical slice (power graph for one region) |
| Weather & seasons | seasonal climate cycle + ordinary weather; feeds farm yield, campaigning, civilian behavior. | after hydrology |
| Natural hazards | flood plains, quake zones, storm tracks, volcanism. | after weather |

---

## Part 6 — World-gen pipeline additions

Insert **after hydrology, before civilization placement** (civ placement
and the economy depend on these):

- **Soil type & fertility** — from geology + climate + slope + drainage.
- **Geology / mineral & fossil deposits** — ore bodies, coal, oil, stone,
  precious; drives mining locales and industry/trade.
- **Navigable waterways & road suitability** — from rivers + terrain +
  slope; drives trade routes and strategic-map travel cost.

Later, not blocking civ placement: seasons & weather, natural hazards.

---

## Part 7 — Build-order recommendation

**After terrain feels right, do NOT continue straight down the world-gen
pipeline.** Instead:

1. Ratify the decisions in this document.
2. Build the **vertical slice** (the review's suggestion): one hand-placed
   region — a town, a farm, a factory, a shop, a connecting road.
   Implement the minimum of: A1 spatial/temporal model + reality bubble,
   A2 ledger + journal, A4 planner layer, a few commodities + one
   production recipe, R2 power graph for that region, comms + incident
   dispatch, and the B1 player loop (take a haul contract → earn a wage →
   buy supplies → travel town ↔ farm ↔ factory ↔ shop).
3. Prove the cascade: cut the factory's power → production halts → shop
   stock falls and prices rise → a witness with comms reports it → a
   repair crew dispatches with real travel time → the player can protect
   or intercept it.
4. *Then* return to world-gen (hydrology, soil, geology, flora/fauna,
   civs, history sim) with the connective systems proven.

**Rationale (Principle 1):** a walkable town with a working failure
cascade tells us far more about whether this game works than a bigger map
does. The connective systems are the risk. Terrain is not.

---

## Open questions for Peter

1. **Build order** — accept Path B (vertical slice next) or keep going
   down the world-gen pipeline (hydrology next)?
2. **Succession (B2)** — comfortable with "you die, you continue as a
   successor you positioned"? Or do you want the player character
   effectively permanent via early/cheap longevity tech?
3. **Sandbox (B3)** — pure open sandbox with no win condition, milestones
   as acknowledgement only — right call?
4. Anything in Parts 1–3 you want to change before it becomes foundation.
