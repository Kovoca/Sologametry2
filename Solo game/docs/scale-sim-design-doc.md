# Individual-to-Intergalactic Sim — Design Document

*A living document. Per Principle 1 below, this is a direction to iterate toward, not a finished spec to complete before building anything.*

## Core Concept

- Player drops into a procedurally generated universe as a single individual and shapes it to the degree they're able — Mount & Blade-style progression (learn a trade/skill, earn money, learn trade routes and watch goods prices, join the military, travel) layered on Dwarf Fortress/CDDA-level simulation depth.
- Scale target: individual → intergalactic, but **build order starts at the planetary level and expands outward later.** Space travel, other planets, asteroids, and alien races are explicitly deferred to future discussion.
- Single-player only — no multiplayer. This relaxes determinism requirements (see Simulation Fidelity below): consistency only needs to hold for one player's save/load, not for verifiable replay across multiple clients.
- Built solo in Rust. (Read "GameMaker Studio 2 / GML" until 2026-08-31; the project has been Rust throughout.)

## Guiding Design Principles

From Tarn Adams, *"Simulation Principles from Dwarf Fortress"* (Game AI Pro 2):

1. **Don't overplan your model.** Simulation-based systems can't be fully predicted in advance and often shouldn't be. Get something running quickly, then iterate — don't try to finish designing before building.
2. **Break down and understand the system.** Model independent underlying elements (e.g., temperature, rainfall, drainage as separate fields) and let higher-level results (biomes) emerge from their interplay, rather than authoring results directly.
3. **Don't overcomplicate.** No variable or system should exist unless it has a meaningful, player-perceptible impact. Operate at the level of what the player sees, or one layer below.
4. **Base your model on real-world analogs.** Grounding systems in real-world mechanics (rain shadows, drainage, actual infrastructure dependency chains) helps validate and debug the simulation.

## Player Character & Movement

- Player starts as an individual with nothing; chooses a trade/skill, earns money, learns trade routes/goods pricing (economics), or joins the military; can travel between locations.
- **Movement: real-time-with-pause** (CDDA-style), not strictly turn-based DF adventure mode — chosen specifically because it supports live battles the player can walk into mid-fight, and variable movement cost (terrain, encumbrance, injury) that makes inventory/loadout choices consequential.
- **Z-levels via explicit vertical connectors** (stairs, ramps, elevators) rather than free-form 3D movement — keeps pathfinding and raised-position tactics tractable, DF-style.
- Vehicles (and later, spaceships) have interiors and individual parts, CDDA-style.

## World Generation Pipeline (Initial Planet)

Following DF's layered approach, each step reading from the ones before it:

1. **Elevation** — heightmap from randomized fractal/noise.
2. **Climate fields, generated independently** — temperature, rainfall, drainage (not derived from each other or elevation directly).
3. **Biome classification** — emerges from the interplay of 1+2, not authored directly (this is what naturally produces effects like rain shadows).
4. **Hydrology** — erosion pass, then permanent rivers, then a water table. Prioritized early since infrastructure/contamination systems later need real ground truth to attach to.
5. **Flora/fauna placement**, seeded by biome — gives the later ecology/cascading-failure systems (disease, contamination) a foundation.
6. **Civilization placement** — settlements form where resources, water access, and defensibility intersect.
7. **History simulation** — abstracted "zero-player game" pass: nations grow, factions form with doctrine traits, trade routes emerge, wars happen, all logged as events, run forward for a chosen span before the player starts.

**Civilization starting point (decided):** all nations on the initial planet begin the history sim at a modern technological baseline — Information/Internet Age — not DF's classic primitive-to-medieval growth arc. History sim runs forward from that point until FTL is discovered. Alien races and other planets are deferred.

**FTL discovery mechanism (decided):** randomized and tied to an individual, not a pure faction-wide research race. Each relevant individual carries an X% chance of being "born" with the circumstances/ability to develop the breakthrough — reflecting how most real technological progress traces back to a specific person, not an abstract institution. Once discovered by one faction, other factions acquire it through two channels:
  - **Trade/relations** — diffusion over time, probability increasing with the strength of trade ties or diplomatic relations between factions (an extension of the existing faction-relationship system).
  - **Espionage** — a faster, riskier channel: a faction can attempt to steal the tech, succeeding or getting caught based on its own espionage investment vs. the source faction's counter-intelligence, with a caught attempt carrying real diplomatic/conflict-state consequences.

  Worth noting: this makes the breakthrough discoverer a natural third trigger for NPC individuation, alongside "player curiosity" and "casualty event" already defined above — a major historical achievement should generate a named individual (with the event logged in history), the same way DF makes its legendary/historical figures real. The individual's chance of appearing is a good place to let faction doctrine matter too — a faction that invests heavily in research could plausibly raise the effective odds of producing its own breakthrough individual, tying this back into the same doctrine-trait system governing infrastructure and civil defense elsewhere.

**Suggested first buildable slice** (per Principle 1): steps 1–3 only — heightmap, temperature/rainfall/drainage fields, emergent biome classification, rendered as a plain colored 2D map. No civilization or history sim yet. Small, fast, and immediately tells you whether the terrain gen actually looks/feels like a real planet.

## Simulation Fidelity & Scale Management

- **Tiered fidelity by distance/relevance**, not binary loaded/unloaded:
  - *Active* (player present): full simulation, every system running.
  - *Nearby/relevant*: periodic cheap ticks — aggregate stats only (population, wealth, faction standing), no individual NPCs simulated.
  - *Distant*: frozen, just a seed + timestamp, until interaction triggers catch-up.
- **Catch-up on interaction** should run the same cheap periodic-tick formula in a tight loop rather than literally simulating elapsed time step-by-step at full fidelity — the trap to avoid is simulating individual NPC behavior during catch-up, which is what causes load-time spikes.
- Being single-player-only means catch-up doesn't need to be strictly deterministic — it needs to be *plausible* and *consistent with what the player already knows*, and save/load only needs per-region persisted state (seed, last-tick timestamp, aggregate stats), not full replay-from-seed determinism.
- **General principle (confirmed):** assume anything happening in the world is actually simulated — existing and updating at the appropriate tier — so the player can physically run into it. Nothing is faked into existence only when the player looks; detail is what's lazy, not existence.

## Reification System (Physical World)

- CDDA "reality bubble" pattern: only the space immediately around the player is a physically simulated place at any given time.
- Distant/unvisited locations exist only as an **event log** — narrative facts (what happened, where, casualties, structural damage) — rich enough to regenerate a consistent physical scene on demand.
- On player approach, a reification pass consumes the event log and procedurally generates the actual map, seeded by those narrative facts rather than pure noise.
- **Decay stages**, as a function of elapsed time since the event, applied as different reification templates over the same underlying log:
  - *Fresh* (minutes–hours): bodies where they fell, blood not dried, fires still burning.
  - *Recent* (hours–a day): bodies moved/evacuated, fires out, scorch marks remain.
  - *Settling* (days): bodies gone, rubble untouched, opportunists may have moved in.
  - *Aftermath* (weeks+): cleanup/rebuilding if the region is functional, or abandonment/overgrowth if not.
- A town evacuated ahead of fighting reifies differently (empty homes, left-behind possessions, no bodies) than one caught by surprise (casualties where people stood, chaos) — same system, different event log content.

## NPC / Troop Individuation — "Generate on Inspection"

- Large forces/populations remain pure statistics by default (e.g., an army of 40,000 is headcount, readiness %, morale average, equipment level — nothing more).
- **Generation triggers:** player curiosity (drilling into a specific unit) *or* a consequential event (that unit taking casualties) promotes it from statistic to named individuals.
- Once generated, a unit's individuals are **permanent and persistent** — even if never viewed again, the data stays intact (never re-abstracted or discarded), so revisiting it later stays consistent.
- This is the same underlying "promote to detail on attention or consequence" mechanism as the physical reification system above — one general pattern, applied to both space and population.

## NPC Interiority

Tiered by individuation status, to keep cost bounded:

- **Abstract/statistical NPCs:** CDDA-style flat needs model only (hunger, thirst, pain, fatigue, morale). Cheap, scalable to any population size.
- **Individuated NPCs:** DF-style personality trait vector + a running thought/memory log — specific things that happened to them color mood and produce genuinely emergent likes/dislikes/ambitions, rather than hand-authored flavor.
- **NPCs with an ongoing relationship to the player** (recurring troops, contacts): richer relationship-web tracking (opinions of specific other NPCs) — reserved for a small circle, since it's the most expensive tier.
- **Individuated NPCs keep living their own lives in the background** — career changes, retirement, conscription — via the same background-tick system, and this persistence must survive scale transitions (e.g., planet to station), meaning location needs to be tracked in a scale-agnostic (hierarchical) way so a player can re-encounter someone whose life has moved on since they last met.

## Combat & Elevation

- Awareness/detection state machine (unaware → suspicious → alert), CDDA-style, drives ambushes: the first-strike advantage isn't a hardcoded bonus, it falls out of however many turns it takes an unaware target to detect a threat and transition to alert.
- Elevation matters for two separable reasons, both modeled rather than collapsed into a flat bonus:
  1. Extended/earlier line of sight — height lets you spot threats sooner, feeding the awareness system.
  2. Different cover geometry — a wall covering a ground-level target may not cover a rooftop shooter, and vice versa; requires real elevation-aware line-of-sight checks.
- Fortification quality can emerge from the same faction doctrine trait (prudent/negligent) that governs infrastructure investment, rather than being hand-authored per fortification.
- **Open question:** full ballistic trajectory simulation vs. an abstracted to-hit roll modified by range/cover/elevation (CDDA/DF precedent favors the abstracted approach for cost reasons) — not yet decided.
- Use cases discussed: convoy ambushes, one thug group ambushing another unaware, and defending fortifications.

## Buildings, Urban Density & Multi-Z Simulation

### Buildings are persistent instances, not giant tiles

- A **BuildingDefinition** is a reusable blueprint: footprint rules, floor
  types, structural system, rooms, circulation cores, utility routes,
  fixtures, furniture, likely occupants.
- A **BuildingInstance** is the persistent building at a location:
  ownership, tenants, condition, alterations, damage, utility connections,
  vacancies, event history.
- The blueprint writes an *initial* state into the local 3D map. After
  generation **the current tiles and objects win**; a damaged wall or
  rerouted pipe is never restored merely because the template placed one
  there.
- Structure, construction, utilities, furniture, items, actors and
  room/ownership volumes stay separate layers. A wall can be breached
  without deleting the room, the building identity, the circuit or the
  ownership record attached to that location.
- Cables, pipes, ducts, shafts, panels, valves, pumps and meters occupy
  tiles, tile boundaries or dedicated service spaces. They are physical,
  inspectable, damageable, isolatable and repairable. **The simulation
  stores the component; the renderer chooses its glyph.**

### Z-levels are not capped at three

- A building owns an absolute Z range, e.g. -4 to +34. The data model must
  not assume basement/ground/roof are the only levels.
- **A storey and a Z-level are not necessarily the same thing.** An
  ordinary storey takes one level; a warehouse bay, theatre, atrium,
  concourse or double-height lobby spans two or more. Floors and ceilings
  are *boundaries between volumes*, not implied by the next Z existing.
- Repeated tower floors may be generated from one floor-family template,
  but each becomes persistent once changed or inspected. Repetition saves
  authoring cost; it does not make floors share damage or contents.
- Vertically aligned spaces are explicit stacks: hoistways, stairs, service
  risers, plumbing stacks, ventilation shafts, light wells, atria,
  structural cores. A shaft opening on one level must connect to the
  matching volume above or below.
- Tall-building pathfinding is hierarchical — entrance, vertical core,
  destination floor, then local room path. Never one flat search across
  forty expanded floors.

### Vertical regimes

Three, matching the horizontal ladder, because the ratio from a 3 m tile
to orbit is 10^8 and one unit cannot span it.

1. **Tile Z — a local generated window, not a planetary range.** Local Z 0
   is the ground you are standing on, and absolute height is derived:
   `absolute elevation = local surface elevation + local Z x 3 m`. A
   settlement on a 4,000 m plateau therefore has ordinary surface tiles
   rather than a coordinate 1,300 levels up, and there is no planetary Z
   range to bound. For scale at 3 m a level: the deepest mine on Earth is
   −1,333 and the tallest building +276.
2. **Air — altitude in metres, position at locality or region scale.**
   Aircraft, artillery, drones, migrating flocks. Ceiling ~20 km, the
   Armstrong limit, where a body needs a pressure suit regardless.
3. **Orbit — orbital elements, no map position.** Something in low orbit
   crosses the planet in 90 minutes; "which cell is it over" is derived,
   not where it lives.

**An entity enters tile space when it needs tile-resolution movement or
collision** — not merely because it *could* affect tiles. Migrating geese
are one Air-layer flock; they reify into individual birds when they land.
The same rule covers a helicopter at 2,000 m against one on the ground.

### Density is more than building height

Enough values to tell a tall isolated tower from a genuinely dense
district: parcel area and site coverage; total floor area and floor-area
ratio; residential units, beds, jobs, visitor capacity; **day, night and
event occupancy** (an office district is dense at noon and empty at
night); vacancy and usable-floor percentage (a forty-storey shell is not a
functioning forty-storey tower); street, transit, parking, lift and
utility capacity; public/private/service space.

Illustrative urban-form bands — *operating problems, not height caps*:

| Urban form | Above grade | Below grade | Simulation character |
|---|---|---|---|
| Rural/edge | 1-2 | cellar or none | long service runs, wells/septic, vehicle dependence |
| Suburban | 1-3 | basement/garage | low FAR, horizontal travel, many separate connections |
| Low urban | 2-6 | utilities/storage | walkable blocks, mixed frontage |
| Mid-rise | 5-12 | parking/loading/plant | shared cores; lifts matter but stairs remain practical |
| High-rise | 13-40 | multi-level parking/plant | pressure zones, lift banks, refuge floors, concentrated failure |
| Very tall | 40+ | deep foundations/transit | internally zoned like several linked districts |

### Building style follows concentration — not yet built

*Parked deliberately: the density **gradient** exists (site coverage,
party walls, terraces against detached, flats only where land is dear),
but building **style** does not. The two are different things and only the
first is done.*

A small town is not a city with fewer people in it; it is a different
**form**. What should vary with concentration:

- **Suburban** (small town, town edge) — detached and semi-detached, deep
  front and back gardens, a driveway and a garage, pitched roofs, cul-de-
  sacs and crescents rather than a grid, low site coverage.
- **Low urban / inner** (market town, city fringe) — terraces and
  tenements on the back of the footway, rear yards and back lanes, shops
  with dwellings over them, a party wall every 5-6 m.
- **High density** (city core) — podium and tower, mixed use stacked
  vertically, service cores, loading docks, no gardens at all, structured
  parking below grade rather than a driveway.

Era and wealth should cut across this: the same density band looks
different built in different decades, and a poor district and a rich one
at identical density differ in plot size, frontage and condition rather
than in height.

The tell that it is missing: at present a house in a village and a house
on the edge of a city are the same object with a different setback.

### Physical utility-network model

Power, water, sanitary sewer, storm drainage and communications share one
spatial graph:

source -> trunk -> district distribution -> parcel service -> building
distribution -> end use

- **Nodes**: sources, junctions, transformers, valves, hydrants, manholes,
  pumps, tanks, meters, panels, cleanouts, fixtures.
- **Edges**: feeders, mains, laterals, risers, stacks, ducts, conduits —
  each with a real route through tiles or boundaries.
- **Common state**: capacity, load/flow, condition, ownership,
  accessibility, isolation state, upstream/downstream connections.
- **Network-specific state stays separate**: voltage/load; pressure/head,
  volume, contamination; slope, fill, blockage, backflow.
- **Service is derived, never a stored yes/no.** A sink works only if an
  intact route reaches a live source at adequate pressure; a lift works
  only if its circuit and controls are live and the car and shaft usable.
- Breakers, switches, valves and gates isolate only the damaged branch.
  Redundancy is an alternate *physical route*, not a reliability bonus.
- Recalculate on change (connection, source, load band, valve, component
  state) or by zone — never solve every pipe and circuit every frame.

Per-network paths, each with its own failure character:

- **Power** — generation, substation, feeder, transformer, service and
  meter, switchgear, riser and panel, circuit, load. Buildings draw
  average *and* peak; overload trips protection rather than silently
  supplying infinite power. Critical and non-critical circuits are
  explicit, so backup can hold alarms, emergency lighting, pumps, comms
  and selected lifts while shedding retail and comfort load.
- **Water** — source, treatment and storage, transmission, district main,
  service/meter/backflow, tank or pump or pressure zone, riser, fixture or
  sprinkler. **A connected pipe at insufficient pressure does not supply an
  upper floor.** Tall buildings divide into pressure zones on booster
  pumps or elevated tanks, which makes water depend on power. Firefighting
  can drain storage or starve domestic supply.
- **Sanitary sewer** — fixture, branch, stack, building drain and lateral,
  street gravity main, interceptor, lift station, treatment. Gravity needs
  a valid downhill route; fixtures below the main's invert need an
  ejector, and losing its power floods the lowest level.
- **Storm** — inlet, drain, storm main, detention, outfall or pump.
  Separate from sanitary in newer districts, combined in older ones, so
  heavy rain can produce sewage overflow.

### Streets are infrastructure corridors

District generation reserves rights-of-way beneath streets, pavements,
alleys and rear-lot easements *before* placing buildings. Visible
serviceable assets — poles, cabinets, pad transformers, hydrants, valve
boxes, manholes, drains — anchor the abstract graph to the physical map.
An excavation or explosion can hit several colocated services at once;
congested old rights-of-way are cheaper to generate and slower to repair.

### Urban generation, growth and decline

1. Terrain, hydrology, flood risk, existing routes.
2. Streets and transit; **reserve utility corridors**.
3. Size district power/water/sewer/storm/comms/transport from era, wealth,
   doctrine and expected demand.
4. Divide blocks into parcels; assign land use from access, land value,
   zoning, industry, population pressure.
5. Select massing — footprint, setbacks, floor count, basement depth —
   constrained by terrain, access, construction capability and **available
   service capacity**.
6. Place cores, shafts, mechanical and refuge floors, service entrances,
   loading and public circulation *before* room layouts and furniture.
7. Connect every building to real upstream networks; reject, downgrade or
   mark illegal/overloaded development where capacity is insufficient.
8. Run occupancy and maintenance history, so districts acquire vacancies,
   renovations, obsolete systems, informal connections, abandoned pipes
   and unequal reliability instead of spawning pristine.

Growth is therefore physical: denser development raises utility, transit,
delivery, waste and emergency demand, so a settlement must extend its
networks, accept degraded service, or stop densifying. War, disaster,
depopulation and neglect reverse it through vacancy, scavenging, leaks,
illegal taps and selective abandonment.

### Player-visible effects of density

Movement (horizontal in sprawl, vertical and queued in towers);
population (occupancy profiles say who is plausibly present without
instantiating everyone); economy (floor area creates capacity, but output
is capped by labour, goods movement, utilities and comms); emergency
response; combat geometry (alleys, atria, rooftops, cores, tunnels,
parking decks, service routes); evacuation (a tower does not instantly
become a street crowd — discharge is limited by stairs, lifts and exits,
then by pavement and road capacity); atmosphere (scheduled crowds,
traffic, deliveries, refuse, lighting, noise, water pressure and visible
service failures make a city feel occupied before any dialogue exists).

## Infrastructure Failure & Cascading Consequences

- Knocking out a region's main power generation causes blackouts; military falls back on backup power with a limited duration unless leadership was prudent enough to build redundancy.
- Each system downstream of power (life support, weapons/turrets, comms, medical, manufacturing, morale) should have its own "how long can it run on backup" clock, producing a multi-system countdown rather than a flat debuff.
- **Faction doctrine trait** (prudent/frugal vs. negligent) governs investment in redundancy, fortification quality, and civil-defense/evacuation planning — one trait expressed across multiple domains, letting a scouting player "read" a faction's competence before attacking.
- Cascading failure chains should be **hand-authored for a small number of dramatic, legible chains** (e.g., power → backup exhaustion → life support failure; power → water treatment failure → contamination → sickness) rather than a generic "everything affects everything" propagation engine, to keep dev cost bounded.
- Backup power management is a standalone resource-management mechanic worth prototyping in isolation: fuel/battery capacity, load-shedding choices, whether reinforcement arrives before reserves deplete.
- Vertical circulation ties directly in: an elevator stranding people mid-use during a blackout is a civilian-relevant emergent disaster, no combat required.

## Incident Response System

- Response is **not automatic** — requires an in-world trigger: a witness with a working communication device actually calling for help, matching real cause-and-effect.
- **Communications is a third attackable infrastructure layer**, alongside power and water/sewer — cutting comms prevents both civilian reporting and military coordination in that area.
- Dispatch involves real travel time, creating a window where response crews can themselves be ambushed (a real-world tactic: hit the repair/rescue team, not just the infrastructure).
- **Civilian responders** (electricians, emergency responders): skill/time-check based, generally unarmed, may need an area secured by military before entering a warzone.
- **Military responders** (combat engineers): faster/more robust repair under fire; dual-use for both defensive repair and offensive sabotage of enemy infrastructure.
- Electrician/emergency responder are legitimate **player trade/career paths**, tying back to the original trade-choice system — not just NPC background roles.

## Core Decision Primitive (Foundational System)

**One decision-making mechanism underlies nearly every autonomous behavior in the game:**

> *An entity with traits evaluates an opportunity against its own risk tolerance and values, then acts.*

Rather than authoring bespoke logic per scenario, the same formula runs with different actors, stakes, and consequences. Places it already applies:

- A gang deciding whether to ambush a refugee column (value of loot vs. risk of an armed escort).
- A faction's doctrine determining investment in infrastructure redundancy, fortification quality, and civil-defense planning.
- A faction deciding whether to attempt espionage to steal FTL tech (value of the tech vs. risk of getting caught).
- An employee deciding whether to sabotage a rival for a promotion (value of the position vs. risk of exposure).
- A soldier under fire deciding whether to follow the squad leader's order or break and flee (survival odds of compliance vs. flight, weighted by training/discipline/morale).

This is what makes the "endless avenues" goal achievable within Principle 3 — combinatorial variety comes from one formula run against many trait combinations, not from hand-authoring many scenarios. **Discipline required:** when a new scenario arises, add new *consequences*, not new *decision logic*. A bespoke decision tree for a specific scenario is a signal to route it back through this system instead.

## Organizations & Positions (Civilian and Military)

**Position and ownership are separate axes.** A player can hold real authority in an organization without owning it, and must answer to whoever does.

Every organization shares the same structure, reused across corporations, gangs, military units, and government office:

- **Ownership/oversight layer** — board/shareholders, family owner, founder, the state, or a military chain of command. Sets priorities and can remove the position-holder.
- **Position-holder** — where the player sits when they hold a role. Real decision-making authority, but accountable upward.
- **Staff hierarchy below** — mostly abstract/statistical, individuating via the standard triggers.

**Accountability relationship:** oversight has its own priorities (profit margin, risk tolerance, growth vs. stability, faction alignment). Player decisions are evaluated against these — drift too far and be fired or demoted; align well or build enough personal influence and gain latitude, potentially enough to reshape what the oversight layer itself wants.

**Advancement requires a real vacancy, not a level-up timer.** Because individuated NPCs live their own careers in the background (job changes, retirement, death, being poached), the chain above the player is populated by real people with real trajectories. The player's climb is "be the best-positioned candidate when an opening genuinely occurs" — which produces real variance and anticipation about specific people's fates.

**Positioning for advancement runs through the core decision primitive**, and both legitimate and illegitimate routes are viable:

- Skill/performance, seniority, and reputation/relationship with the oversight layer (the same tracking that prevents firing also drives promotion).
- Politics: courting specific people, positioning, undermining rivals — up to and including bribery, blackmail, and sabotage, resolved with career/reputation/legal consequences rather than combat consequences.

**People directly above the player individuate early**, via the standard attention trigger — the moment a player works toward a position, its current holder and rival candidates become real, named individuals, well before any vacancy occurs.

## Civilian Economy — Sector Pipeline

Four linked tiers, each consuming the output of the one before it. These are the economic backdrop organizations operate within; players work into positions within those organizations rather than climbing a fixed ladder.

**1. Farming / raw extraction (primary)**
- Inputs: land, water, seasonal/day-night cycle, labor. Outputs: raw goods.
- Infrastructure tie: **water/irrigation matters more than power** — a distinct failure mode from the factory tier.
- Yield affected by weather (from world-gen climate fields) and infrastructure integrity.

**2. Factory / manufacturing (secondary)**
- Inputs: raw goods, labor, **steady high power demand**. Outputs: finished goods.
- This is where blackouts have *economic* teeth: lost production propagates downstream as retail scarcity.
- Machine upkeep interfaces with the engineer/electrician trade; shift work interacts with the day/night cycle.

**3. Commercial / retail (tertiary)**
- Inputs: finished goods (local or imported via trade routes). Outputs: consumer availability at supply-driven prices.
- This is where the original "watch prices, learn trade routes" loop mechanically lives.
- Direct interface with the protection-racket system: a shopkeeper is a retail-tier actor, and extortion draining their capital should visibly propagate into worse stock and higher prices.

**4. Office / corporate / services (quaternary)**
- Inputs: labor and **comms infrastructure**, not physical goods. Produces coordination, finance, administration, information.
- Functions as an **efficiency multiplier** on the tiers below rather than producing goods — a well-run corporate/logistics layer improves trade route function; a degraded one (comms cut, corruption, mismanagement) damages the whole chain without a farm or factory being touched.
- **Open design risk:** the least physically active tier and the hardest to make feel like gameplay rather than a spreadsheet — needs concrete thought rather than assuming it works like the others.

## Military Organization & Tactics

**Rank hierarchy reuses the organizations/positions system above** — squad member → squad leader → platoon leader → company commander → higher, with the same vacancy-plus-merit-or-politics advancement and the same accountability-to-oversight dynamic.

**Hierarchical intent, not hierarchical micromanagement.** Each command layer issues *intent* to the layer below, which translates it into more concrete action:
- Platoon leader issues an objective (take that building, hold this line, flank left).
- Squad leaders translate into squad maneuver (which team suppresses, which advances).
- Individual soldiers translate into personal behavior (find cover, watch a sector, advance to a point).

Each layer only needs to understand the layer above's intent, not the whole battle — keeping the AI tractable and mirroring real command structure (Principle 4).

**Individual soldiers still run the core decision primitive underneath the tactical layer** — this is where discipline and morale matter mechanically. A disciplined veteran executes fire-and-movement as ordered; a green or shaken soldier may hesitate, freeze, or break, using the same evaluate-against-traits logic as everything else. Veteran vs. conscript units feel different without separately authored behavior trees.

**Training raises those trait values over time** — drills and exercises improve discipline and tactical proficiency. Runs as cheap background ticks for NPC garrisons (readiness stat rises) or as played content when the player is a trainee or is running the training.

**Fidelity tiers apply as everywhere else:** off-screen battles resolve via formulas factoring average training, leadership quality, and equipment — no individual cover-seeking pathfinding. Actual tactical movement AI only runs when the player is present or a unit is individuated.

**Scope warning:** coordinated small-unit tactics (cover-to-cover movement, fire-and-movement, formation cohesion under leader intent) is genuinely one of the harder AI problems in game development, and is a *group* coordination system unlike anything else in this design. Treat it as its own future prototype, separate from the world-gen and economy work.

## Conflict State as a Global Modifier

- Conflict/governance state (peacetime / contested / active warzone / lawless frontier) should be a **spatial gradient** — distance/exposure to an actual moving front line — not a flat per-region binary flag.
- This state modifies behavior across many systems uniformly: dispatch priority/availability, who's allowed to respond, law enforcement/court function, civilian evacuation behavior, and whether gang "frontage" ambush signals are even plausible in that area.
- The same architecture applies recursively at smaller scale — gang turf disputes escalating to open gang war use the same conflict-state-ladder logic as national war, just at neighborhood scope.

## Economy & Crime Layer

- Core loop: player learns trade routes, watches/exploits goods prices.
- **Gangs modeled as lightweight factions:** block-level territory, a doctrine trait (aggressive-expansionist vs. content-with-turf), and inter-gang relationships that can escalate via the same conflict-state ladder as national war.
- **Protection money:** periodic extortion collection tied to territory control, with a target compliance choice (pay / refuse / report) carrying real downstream consequences — refusal triggers an incident via the comms/dispatch chain; paying drains a shopkeeper's capital, visibly affecting local goods availability and prices.
- **Open question:** can the player run a protection racket / join a gang as a playable path, or is this purely an NPC/AI-driven backdrop the player reacts to?

## Civilian Evacuation & Refugee Systems

- Evacuation/warning behavior before fighting is an expression of the same prudent/negligent doctrine trait, applied to civil defense.
- Individual civilian flee/relocate decisions are mostly **emergent** from the needs-based NPC system (rising safety need as regional threat climbs); top-down evacuation orders from competent governments accelerate and organize that same behavior rather than being a separate system.
- Evacuation "tells" (families loading vehicles, etc.) are meant to be **player-observable signals** of impending violence — but only in genuinely threatened, frontage-relevant areas, not uniformly; requires the conflict-state spatial gradient above.
- **Refugee columns are actually simulated** as real traveling entities on the road/trade-route network — abstract-tier when the player isn't nearby, reifying into physical detail on encounter (same generate-on-inspection pattern as troops/terrain).
- Refugee flow causes real logistics consequences: destination settlements strain under absorbed population (housing, food/goods supply, price spikes/shortages even far from fighting); roads become contested resources shared with military logistics.
- Whether a given gang attacks a refugee column falls out of that gang's own traits via a general **opportunity-evaluation check** (perceived value vs. perceived risk) — not a scripted rule; same decision-logic shape as faction doctrine, running more frequently at smaller scale.

## Deferred to Later Discussion

- Space travel, other planets, asteroids.
- Alien races / other civilizations.
- Full ballistic simulation vs. abstracted combat resolution.
- Playable protection-racket/gang-leader path.
- Exact espionage mechanics (success/detection odds, diplomatic consequences of a caught attempt) — resolved at the level of "this exists and works roughly like this," not yet fully specified.

## Next Step

First buildable slice: planetary terrain generation — elevation, independent climate fields (temperature, rainfall, drainage), emergent biome classification — rendered as a plain 2D colored map. Everything past that (hydrology, civilization placement, history sim) waits until this piece is running and feels right.
