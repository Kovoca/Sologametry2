# Implementation Spec

Resolved design decisions, written to be implemented directly. Built one
section at a time; each is settled before the next begins.

Companion documents: `scale-sim-design-doc.md` (vision),
`design-review-triage.md` (review), `decision-log.md` (positions taken).

---

## A1. Spatial & Temporal Hierarchy — SETTLED

### A1.1 Coordinate system

**Global tile coordinates.** A tile is addressed by signed 64-bit `x`, `y`
across the entire planet. Higher levels are derived by **bit shift**, never
division:

```
chunk_x  = tile_x >> 5       // 32 tiles per chunk
region_x = tile_x >> 14      // 512 chunks per region
```

All divisors are powers of two. This is not cosmetic — at planetary
distances float coordinates drift, and division/modulo on signed values
rounds toward zero, which breaks in the negative hemisphere. Shifts are
exact, branchless, and correct for negative coordinates.

| Level | Size | In tiles | Metres | Role |
|---|---|---|---|---|
| tile | atomic | 1 | 1 m | physical unit |
| chunk | 32 × 32 tiles | 32 | 32 m | load/unload, local gen |
| **cell** | 64 × 64 chunks | 2,048 | 2.048 km | **natural data**: climate, biome, soil, hydrology |
| region | 8 × 8 cells | 16,384 | 16.384 km | **administrative**: ledger row, sim tier, ownership, locales |

```
chunk_x  = tile_x >> 5
cell_x   = tile_x >> 11
region_x = tile_x >> 14
```

**Why the cell level exists.** Dwarf Fortress carries climate and biome data
at ~1.87 km resolution (its world/region tile). Our region is 16.384 km —
76× the area. One biome value covering 268 km² produces uniform slabs of
terrain; DF's variety comes from its *fine climate grid*, not from its local
generator. The cell restores comparable resolution (2.048 km ≈ DF's 1.87 km)
without changing region size.

**The split is physical vs. administrative.** Cells hold what nature
decides. Regions hold what people and the simulation decide. One level was
doing both jobs badly.

### A1.1b Named geographic regions are detected, not gridded

DF does not subdivide the world into named geographic areas. After
generation settles, it detects the limits of the final biome regions and
gives them names and identity — "The Forest of Sorcery" is an emergent
contiguous area, found by flood-fill over finished data.

Adopt this. Two distinct concepts, do not conflate them:

- **Region** (grid, 16.384 km): administrative and simulation unit. Fixed.
- **Named geographic region**: a contiguous run of cells sharing biome
  character, discovered by flood-fill *after* generation completes, then
  named. Arbitrary size and shape. What the player and NPCs actually refer
  to in conversation, history, and travel.

A named region may span many administrative regions, and vice versa. This
is correct and intended — real geography does not respect borders.

### A1.2 World size (generation parameter)

Planet dimensions are chosen at world generation, DF-style. Not hardcoded.

| Preset | Regions | Planet size | Purpose |
|---|---|---|---|
| Pocket | 64 × 36 | 1,048 × 590 km | Fast iteration; generates in ~1 s |
| Small | 192 × 108 | 3,146 × 1,769 km | Development default |
| Medium | 384 × 216 | 6,291 × 3,539 km | Normal play |
| Large | 768 × 432 | 12,583 × 7,078 km | Comparable to Mars |

Aspect ratio is fixed at 16:9 so latitude behaviour is consistent across
presets. Cell counts are 8x these figures per axis (Large = 6,144 x 3,456
cells). Use **Pocket** while building — a generator you can rerun in a
second gets tuned; one that takes a minute does not.

### A1.3 Planet topology

**East–west: cylinder wrap.** Region x wraps modulo world width. Travel
west far enough and you arrive back from the east. All noise used in world
generation must **tile horizontally**, or the seam is visible.

**North–south: poles, no wrap.** Y is clamped, not wrapped. The top and
bottom rows are polar. Latitude is derived from y:

```
latitude = (y / (height - 1)) * 2 - 1     // -1 south pole, +1 north pole
```

**FIXED — implemented in both `worldgen.py` and `worldgen.rs`.**

The generator previously used a radial edge falloff (producing an island with
ocean on all four sides) and wrapped rainfall smoothing vertically (`% h`,
making the world a torus with no poles). Five changes were applied:

1. **Radial falloff removed.** Land crosses the east/west edges continuously.
2. **Ocean placement comes from the continental field alone**, not from
   distance to map centre. A radial falloff also forces mountains into a
   central dome.
3. **Moisture advection wraps east–west**, sweeping `REVOLUTIONS = 2` full
   passes so the field converges rather than depending on an arbitrary
   starting column. Air leaving the east edge arrives at the west already
   dry — which is what makes wrapped rain shadows differ structurally from
   island ones.
4. **All vertical smoothing clamps at the poles** instead of wrapping (both
   the per-column moisture row-mixing and the final lateral smoothing).
5. **Value noise tiles on the x axis** — the sample grid wraps in x, with
   `fx` mapping onto `[0, gw)` rather than `[0, gw-1]`. Y clamps.

**Verified:** mean seam discontinuity between the east and west edge columns
measured at 0.20x the typical adjacent-column difference — i.e. seamless.
Land now reaches the poles (polar continents, as Antarctica does).

### A1.3b Generation is multi-pass, with feedback

**The current generator is single-pass and this is wrong.** It computes
climate once against raw elevation and never revisits it. Once hydrology
carves valleys, the rainfall map will describe terrain that no longer
exists.

DF's order — elevation, smooth, rivers *carve the elevation field*, smooth
again, **then** rain shadows and orographic precipitation, **then**
temperature reset against the modified elevation plus rainfall plus forest
damping, **then** vegetation finalised — computes climate against finished
terrain, not raw terrain.

**Adopted pipeline (coarse pass, at cell resolution):**

| # | Pass | Notes |
|---|---|---|
| 1 | Base elevation | continental mask + ridged noise |
| 2 | Smooth mid-elevations | creates plains; DF does this deliberately |
| 3 | Provisional climate | rough temperature and moisture; needed to seed erosion |
| 4 | **Erosion and rivers** | rivers carve channels into the elevation field |
| 5 | Re-smooth | mountains down to sea |
| 6 | **Rainfall, final** | moisture advection with orographic lift, against *carved* elevation |
| 7 | **Temperature, final** | latitude, altitude lapse, rainfall, forest damping |
| 8 | Drainage / runoff | soil permeability (independent) + slope runoff (derived) |
| 9 | Biome classification | from finished fields |
| 10 | Named region detection | flood-fill contiguous biome areas, name them |
| 11 | Soil, geology, minerals | per triage Part 6 |
| 12 | Navigable water, road suitability | feeds civ placement and travel cost |

Passes 6 and 7 must run after 4. That is the entire point.

### A1.3c Rejection sampling

DF generates a world, verifies it against the requested parameters, and
**rejects and retries** if it fails — for example, too few mountain peaks,
because the number cannot be known ahead of time.

Adopt this. Generation parameters specify acceptable ranges (land fraction,
minimum mountain area, biome diversity, river count). After the coarse
pass, verify; on failure, increment the seed and retry, with a retry cap.

This is far cheaper than trying to guarantee properties analytically, and it
is why the Pocket preset matters — a coarse pass fast enough to run twenty
times is one you can afford to reject.

### A1.3d Memory: the coarse pass is global, cell detail is lazy

A Large planet at cell resolution is 6,144 × 3,456 = **21.2 million cells**.
Six `f32` fields is ~510 MB before intermediates. That is not affordable, and
it would destroy fast iteration.

Therefore:

1. **Coarse global pass runs at region resolution** (768 × 432 = 331k —
   trivial). This *must* be global: moisture advection sweeps the whole
   cylinder east-west and cannot be computed per-region.
2. **Cell detail is generated per region on demand**, deterministically from
   the world seed: interpolate the coarse fields, add higher-frequency
   variation.

Rain shadows come from the coarse pass — 16 km resolution is ample to model
a mountain range blocking weather. Local variety comes from refinement.
Nothing large is ever stored.

This is rules R4 and R5 applied to world generation rather than to saves:
existence persistent, detail lazy; baseline seed plus deltas.

### A1.4 Map modes

**Strategic map** — region level. Travel is abstracted: choose a
destination, time advances, cost = distance × terrain × transport × route
risk. This is how long journeys happen without real-world hours.

**Local map** — chunk/tile level, real-time-with-pause. The reified
reality bubble. Entered on arrival, or when something forces reification
(ambush, encounter, incident).

Transition between modes is explicit and player-visible (Mount & Blade
shape).

### A1.5 Walkable space

**All terrain between locales is physically real and enterable** — fields,
rivers, forests, mountains, roads. Not just locales connected by abstract
lines. This is required by the existing design: convoy ambushes on open
road, intercepting refugee columns mid-journey, walking onto a battlefield.

**Generated on demand, not stored.** A chunk is generated deterministically
from the world seed plus its coordinates whenever entered. Leave, and it is
discarded. Re-enter, and it regenerates identically.

**Persistence is triggered by journalled events only.** A chunk becomes
persistent — carrying a delta set forever — the first time something
journalled happens in it: a firefight, a wreck, a grave, construction, an
ownership change. Otherwise it is never written to disk.

This is rule R5 applied unchanged, and it is what keeps save size bounded.
Without it, every field ever crossed accumulates state.

**Seam requirement:** hand-authored locales and generated wilderness must
be visually and mechanically indistinguishable. Both go through the same
chunk generator; a locale supplies stronger constraints to it, it does not
bypass it. If the seam is visible, the world reads as fake.

### A1.6 Reality bubble

Elastic by travel mode — a fixed 96 × 96 tiles is under 100 m, far too
short for a road ambush to be legible.

| Mode | Radius | Tiles | Metres |
|---|---|---|---|
| On foot | 5 × 5 chunks | 160 × 160 | 160 m |
| In vehicle | 9 × 9 chunks | 288 × 288 | 288 m |

Plus the entirety of the locale currently occupied, regardless of radius.
Tunable; these are starting values, to be revised once movement is playable.

### A1.7 Time

| Unit | Scope | Cadence |
|---|---|---|
| tick | inside the reality bubble | fraction of a game-second |
| region tick | *relevant* regions | once per game-hour or game-day |
| catch-up | frozen region on interaction | region-tick formula in a tight loop over the elapsed span |
| calendar | global | fixed year, 4 seasons, 24-hour days |

**Catch-up never simulates individuals.** It runs aggregate formulas only.
The moment catch-up touches individual behaviour, load times spike
unboundedly with elapsed time.

Seasons drive farming, weather, and temperature swing. The day cycle drives
shift work and visibility.

### A1.8 Simulation tiers

| Tier | Definition |
|---|---|
| **active** | Player is in the region. Full simulation. |
| **relevant** | Within K regions of the player, **or** contains an individuated NPC in the player's relationship web, **or** part of an event chain the player is entangled in. Region ticks only. |
| **frozen** | Everything else. Ledger row plus last-tick timestamp; caught up by formula on interaction. |

Per rule R4: existence is persistent, development is lazily resolved.
Frozen regions are not paused — their state advances by formula whenever
someone looks.

### A1.9 Cross-region events

Armies, refugee columns, caravans, and epidemics are **mobile entities**
owning their own ledger row. They tick at region cadence on the strategic
map and reify on encounter.

They are explicitly **not** processed by the regions they pass through. A
mobile entity has exactly one owner — itself — which avoids an entire class
of double-processing and double-counting bug.

### A1.10 Open / deferred

- `star system` and `universe` levels: slots reserved, not specified.
- Z-levels: vertical extent per chunk not yet fixed. Deferred until
  buildings are built. Note DF's quirk: a single tall peak forces every
  embark in that region to carry all the empty air above it. Bound z-extent
  per chunk, not per region, to avoid inheriting that.
- K (the "relevant" radius): needs a real number once region ticks are
  measurable.

---

## A2. State Model — Ledger, Journal, Realization — SETTLED

### A2.1 The governing failure to avoid

Dwarf Fortress offloads units and re-initializes them from historical figure
data whenever the local map unloads. That record does not carry wounds,
hunger, or intoxication — so fast-travelling or sleeping heals all
non-permanent wounds (some convert to scars) and resets stomach fullness and
intoxication. Players use it as an exploit.

**The cause is not lossy storage. It is a ledger schema narrower than the
physical state it must absorb.** Wounds could exist physically but had
nowhere to live abstractly, so they evaporated at the handoff.

Every rule below exists to prevent that class of leak.

### A2.2 Persistent vs. ephemeral — declared, not assumed

Every mutable property is explicitly classified. This table is written
*before* the entity type is implemented, not after.

- **Persistent** — must be representable in the ledger. If the physical
  layer can change it, the ledger must be able to hold it. Wounds,
  inventory, skills, relationships, goods, ownership, money, position at
  locale granularity.
- **Ephemeral** — exists only while reified; safe to discard. Exact tile
  position within a locale, pathfinding scratch, animation state, current
  step within a plan.

DF's bug is a single miscategorisation: wounds treated as ephemeral. Getting
this table right eliminates the whole class; getting it wrong anywhere
reproduces the exploit.

### A2.3 The journal is the write path

Persistent state **cannot be written directly**. It changes only by emitting
a journal event. The ledger is a materialised view of the journal.

Consequences, all of them wanted:

- **No exit diff.** The triage proposed diffing the local map against
  reify-time state on exit — requiring a full snapshot, double memory, and
  an O(map) walk. Unnecessary: if mutations are journalled as they occur,
  the delta list already exists when the player leaves.
- **Every change has a cause.** An unattributed delta is a bug, not data.
- **Conservation assertions are trivial** — totals in must equal totals out
  plus journalled deltas. Run unconditionally in debug builds.

To avoid replaying from origin: periodic **ledger snapshot + journal since
snapshot**.

### A2.4 Terminal state is regenerated; ongoing state is stored

**Rule: lossy regeneration is legitimate for anything that has stopped
changing, never for anything still in play.**

A corpse is frozen. Store **cause of death + seed**; generate the wound
pattern deterministically when someone inspects it. Shrapnel at close range
produces one pattern, a rifle round another — identical every time it is
viewed. This is R5 applied to bodies: cause is the seed, wounds are the
generated detail.

Generalises directly: a ruin stores *burned* and regenerates char and
collapse; a wreck stores *destroyed by artillery* and regenerates damage.

**Wound state tiers:**

| Subject | Storage |
|---|---|
| Dead | cause + seed; regenerate on inspection |
| Living, individuated | real wound state, stored and ticked |
| Living, abstract | aggregate only (casualty counts, readiness penalty) |

**Wounds keep evolving while abstracted.** An enemy wounded and then fled
must carry the wound out of the bubble — bleeding out, becoming infected, or
healing over days on the region tick. Not full physiology: severity, type,
treatment status, timestamp. Small enough to tick cheaply.

If leaving the reality bubble healed enemies, every fight would be
unwinnable in the same way DF's fast-travel healing is exploitable, inverted
against the player.

### A2.5 Individuation trigger — mechanical effect

Added to the existing triggers (attention, consequence, achievement):

> **Any entity the player mechanically affects is individuated at that
> instant.**

Wound someone, rob someone, hire someone — they become permanently real,
because specific state now exists about them that only a real entity can
carry. A statistic cannot hold a specific injury.

Self-limiting in the right way: a player can only personally affect so many
people, and everyone promoted is someone there is a reason to remember. It
also yields the revenge loop for free — the raider who escaped with your
wound is a persistent individual carrying a memory of you.

### A2.6 Roster identity

Populations are counts plus a **seeded roster spec**, never instantiated
wholesale.

- Identity is deterministic: `person = hash(roster_seed, index)`.
- On individuation, the index is **pinned** and never reused.
- Casualty rolls draw indices **uniformly across the whole roster**.

If a pinned index is hit, that person dies, and the death is journalled as a
named event. **Individuation confers persistence, not protection.** Biasing
casualties toward unindividuated people would make the player's
acquaintances quietly immortal, and players notice that quickly.

Improvement over DF: anyone the adventurer encounters becomes a historical
figure, but anyone not already one is a complete blank slate with almost no
knowledge of anything. A seeded roster spec generates a consistent backstory
instead, so a promoted individual has a past rather than being born empty.

### A2.7 Journal compaction

DF does not solve this: a large world with 1,000 years of history can
produce an XML dump up to a gigabyte. It caps historical figure count at
world-gen and culls unimportant ones — a hard cap plus deletion.

Worse for us: DF's history is generated once and then largely static; ours
runs continuously during play. Compaction is mandatory.

**Compact by reference, not by age alone.** An event survives at full detail
if it references a still-live individuated entity, or is a flagged
milestone. Otherwise it degrades:

1. Full event → 2. aggregate delta → 3. summary fact ("the town burned, 200
died"), per-person detail dropped.

This is why 200 anonymous casualties compress to a number while the death of
someone the player met stays a named event indefinitely.

**Archive, never delete** (per B4). Compression is in storage, never in
identity or traits. Rehydration is verified by test.

### A2.8 Recorded fact vs. known fact

Worth stealing from DF: its "Reveal All Historical Events" setting means
events exist in history but may not be known to any civilization.

That is the A3 belief model applied to history itself — what happened versus
what anyone knows happened. It should fall out of A3 naturally rather than
needing separate machinery, and it is what makes investigation, rumour, and
lost history possible.

---

## C1. Armed Group Formation — SETTLED

### C1.1 No encounter spawning, ever

Hostile entities are never generated because the player is on a road and it
is time for a fight. Every armed group exists as an actor in the ledger with
a home, a reason for existing, and a territory. What the player meets is
that entity being somewhere it plausibly is.

This is R4 applied to threats, and it is the only way regional character
holds automatically: raiders cannot appear in a lawful region because no
raider band exists within range of it.

### C1.2 Governance has two axes, not one

A single lawful↔lawless scale cannot express a state with firm control that
preys on its own people. Two independent regional axes:

- **Control** — how much effective authority exists: none / contested / firm
- **Disposition** — whose benefit it serves: protective / extractive /
  predatory

| | Protective | Predatory |
|---|---|---|
| **Firm control** | Lawful. Petty and organised crime hiding from enforcement. No raiders. | Checkpoint extortion, soldiers shaking down civilians, disappearances. The state *is* the threat. |
| **Weak control** | Well-meaning but absent. Banditry in the gaps, gangs running neighbourhoods. | Warlordism. Open raiding, armed groups competing. |

Both are **per-region** — a capital may be firmly held while a frontier
province is not — and they move independently. A state's disposition can rot
without its control weakening at all.

### C1.3 Formation requires three conditions simultaneously

Not one grievance. **Motive + capability + opportunity, or nothing forms.**

1. **Motive** — a grievance with a target. Multiple stack and *compound*
   rather than add: economic desperation, foreign occupation, national or
   ethnic identity suppressed, ideological conviction, political exclusion,
   revenge for specific violence.
2. **Capability** — armed-capable people, weapons, funding. Without it,
   grievance produces protest, emigration, or nothing.
3. **Opportunity** — weak control, difficult terrain, cross-border
   sanctuary, external sponsor.

This is why brutal repression holds deeply resentful populations for decades
(motive high, opportunity zero), and why prosperous stable regions produce
no insurgency despite available weapons. The interesting cases are
conjunctions, which is historically correct.

### C1.4 One mechanism, four outcomes

| Group | Grievance | Organisation | Target |
|---|---|---|---|
| **Bandit** | economic desperation | none — opportunistic | whoever passes |
| **Mercenary** | none — a business | commercial, hierarchical | whoever pays |
| **Insurgent** | political, against a specific authority | ideological, recruits | that authority |
| **Rebel** | insurgency grown enough to hold territory | proto-state | governance |

Same formation code, different input weights. A mercenary company is a
bandit group with a business model and no grievance — it needs only
capability plus a market, forms in stable regions as readily as unstable
ones, and is evaluated through the ordinary opportunity primitive rather
than the grievance system.

### C1.5 Manpower comes from displaced population

Groups draw from populations that already exist and have lost their
livelihood. Never from nowhere.

- **Demobilised or unpaid soldiers** — historically the most reliable
  source; wires directly into the military system. Makes unpaid armies
  genuinely dangerous, which gives the economy real teeth on the military
  side.
- Deserters from a losing side
- Refugees no destination will absorb
- Workers from a collapsed industry
- Displaced landholders

**The trigger is a ledger condition, not a random roll:** armed-capable
population exceeding available legitimate employment, under weak control.
Checkable on the region tick from numbers already held.

### C1.6 Baseline behaviour is endurance

**Most people, under most grievances, adapt.** They keep working, avoid the
checkpoint, stay quiet, and get on with their lives. This is not passivity —
the expected cost of fighting is enormous and the benefit remote and
uncertain.

Armed resistance is the **rare tail**. The three-condition test should
almost always return no.

- **Participation rates are tiny.** Even in severe insurgencies, active
  fighters are a fraction of a percent of the affected population. A region
  of 200,000 under occupation produces dozens of fighters, not thousands. If
  output looks like a popular uprising by default, the model is wrong.
- **Exit competes with resistance and usually wins.** Responses to grievance
  rank roughly: endure, comply, emigrate, resist non-violently, turn to
  crime, take up arms — in descending order of frequency. **Blocking exit is
  therefore itself a driver:** sealing a border to contain a problem removes
  the cheaper option and makes it worse.
- **Recruitment draws from a narrow demographic slice**, not the general
  population: young, unattached, few dependents, personally victimised, high
  risk tolerance, or already armed and organised. Dependents strongly
  suppress — people with children to feed overwhelmingly endure.
- **Soldier and police defection is disproportionately important.** Already
  armed, trained, organised, and past the threshold once. Small numbers,
  outsized effect.

An insurgency forming should feel like a notable event, not weather.

### C1.7 Cultural values as a regional multiplier

Participation rates vary by orders of magnitude between populations facing
identical grievance and opportunity. Culture is a **per-region multiplier on
the formation threshold**, not a separate system. Axes, all secular and
mechanical:

- **Martial tradition** — is fighting a normal expectation, or an aberration?
- **Kinship obligation** — does harm to a relative compel response?
- **Collective vs. individual identity** — is an attack on the group an
  attack on me?
- **In-group loyalty radius** — family, clan, tribe, region, nation?
- **State legitimacy** — is authority owed obedience, or is it merely
  whoever currently holds power?
- **Honour cost of submission** — what does enduring cost socially?

A region high on these produces fighters at rates that look absurd if
calibrated on a low-obligation, high-legitimacy population.

**Kinship obligation is the mechanic to build carefully**, because it is
self-sustaining: casualty → obligated kin → recruitment pool. Counter-
insurgency operations then *generate* recruits, each casualty creating
several newly obligated fighters. This is how movements survive heavy
attrition and why aggressive suppression can grow the insurgency it intends
to crush. It is a specific checkable rule, not an abstraction.

It also feeds A2.5: someone whose relative the player killed has a specific
personal reason to become individuated.

Populations low on kinship obligation absorb losses and demobilise instead —
which is where suppression does work.

### C1.8 Dissolution

Groups are not permanent once formed. The same conditions reversed dissolve
them: pay restored, amnesty offered, employment available, or sufficient
force applied. Members return to the population.

Counter-insurgency can therefore attack **motive** (address grievance),
**capability** (interdict weapons and funding), or **opportunity** (control
terrain, close borders). Attacking only capability while grievance grows is
the classic failure mode, and the model reproduces it.

---

## A3. Belief, Values, Preferences — SETTLED

Three distinct systems that share loose language and must not share
implementation. Separating them is the point of this section.

| System | Answers | Changes over time? | Read by |
|---|---|---|---|
| **Belief** | what an actor thinks is *true* | yes — decays, updated by reports | the planner, before acting |
| **Values** | what an actor thinks is *right* | no — stable disposition | the decision primitive, as weighting |
| **Preferences** | what an actor *likes* | rarely | mood, satisfaction, social leverage |

They compose: **belief supplies the facts, values supply the weighting.**
Two people with identical information about an unguarded shipment reach
opposite conclusions — one weights law heavily, the other weights gain. Same
belief, different values, different act.

### A3.1 Belief record

Minimum viable record — four fields, sufficient to drive prices, ambush
decisions, military planning, and investigation:

| Field | Meaning |
|---|---|
| claim | the asserted fact |
| timestamp | when it was true, or believed true |
| source | who or what supplied it |
| confidence | how strongly it is held |

Example: *Faction X believes region Y holds a garrison of ~400, as of day
212, per a trader's report, low confidence.*

### A3.2 Belief decays toward uncertainty, not toward error

As the timestamp ages, confidence falls. Below a threshold **the actor knows
that it does not know** — and that is what drives scouting, sending
messengers, hiring informants, and paying for intelligence.

An actor who is *confidently wrong* is a special case — deception, or a
report that was accurate when sent and has since become false — never the
default. Stale belief must not silently become confident delusion.

### A3.3 Belief attaches to subjects, and only where there is reason

Holding beliefs about every region is unaffordable. Beliefs attach to
**subjects**: a region, a faction, a person, a route, a commodity. An actor
holds a belief only about subjects it has reason to care about. No reason,
no record — and the actor simply does not know.

This is R4 applied to knowledge: existence persistent, detail lazy.

**Cost tiers**, matching the existing individuation tiers:
- Abstract NPCs: share one belief store per faction per region.
- Individuated NPCs: personal divergence from the faction store.

### A3.4 Player knowledge is a belief layer

The player's map is their belief, not ground truth — same mechanism as
NPCs, no exceptions.

**This is a significant UI commitment, not only a simulation one.** The
interface must display information that is stale, sourced, or contradicted,
and make it legible *as belief* so it does not read as a bug: map data with
timestamps and provenance, reports that prove false, prices that were
accurate last week. Budget for it as UI work.

### A3.5 Recorded fact vs. known fact

Per A2.8 — events exist in history whether or not anyone knows them. This
falls out of the belief system rather than needing separate machinery, and
is what makes investigation, rumour, and lost history possible.

### A3.6 Values

Stable moral and dispositional weightings: attitude to law, justice, greed,
loyalty, honour, cruelty, ambition, risk. They do not decay and are not
updated by reports.

**Generated from regional culture plus personal variation.** The cultural
axes already defined in C1.7 — martial tradition, kinship obligation,
collective vs. individual identity, in-group loyalty radius, state
legitimacy, honour cost of submission — supply the distribution; the
individual roll picks a point within it.

An NPC generated in a high-kinship-obligation, low-state-legitimacy region
skews accordingly, but individuals still vary — producing the law-abiding
person in a lawless region and the opportunist in a strict one.

**One cultural profile per region, read by two systems:** C1 insurgency
formation and A3 value generation. Regional character for free.

**Values derive from where a person was raised, not where they now are.** A
migrant carries their origin's values into a new region. Otherwise
relocation would silently rewrite who someone is — unacceptable given that
migration and displacement are central to C1.

### A3.7 Preferences

Small, arbitrary, specific likes — a food, a material, a gem, a colour, a
craft form. Dwarf Fortress's mechanism, and worth having for the same
reason: they generate specific mood effects and give each individual texture
without authored content.

Two payoffs:
- **Mood and satisfaction** — a soldier who received their preferred meal.
- **Social leverage** — knowing what someone likes enables gifts, bribes,
  and recruitment.

**Preferences roll against local availability, not culture.** A person
cannot prefer a food that did not exist where they grew up. Regional
cuisine, local materials, local crafts.

**Consequence worth keeping:** preferences become a *tell*. Someone whose
preferences do not match their claimed origin is worth a second look —
giving imposter and infiltration mechanics a foundation at no extra cost.

### A3.8 Generation timing

Values and preferences are rolled **lazily at individuation**, derived from
the person's seed and their origin region's cultural and availability
profile. Consistent forever, never precomputed.

Definitions live in external data files (raws-style), not hardcoded — so
adding a value axis, a cuisine, or a craft is a data change.

---

## A4. Decisions to Actions — the Planner Layer — SETTLED

The decision primitive (see the design doc) evaluates value against risk. It
does **not** generate opportunities or plans. This section supplies the
layer between it and the world.

### A4.1 Framing: the player lives in the world

This is a roguelike, not a quest game. The planner is the machinery of other
people's lives; the player is one more inhabitant who may or may not be
standing somewhere useful when something happens.

**The player gets no privileged channel.** The same rules apply as to
everyone:

- **Nothing is pushed to the player.** Opportunities exist as facts in the
  world — a vacancy, a posted contract, a freight job. The player learns of
  them as anyone would: by being present, asking, knowing someone, or
  reading a notice. If nobody tells you, you do not know.
- **Nothing scales to the player.** Opportunities do not adjust to level or
  resources. Most of what is happening is irrelevant, out of reach, or
  already taken by someone better placed. This is not a difficulty setting;
  it is what being one person in a populated world means.
- **The world does not wait.** A vacancy heard about last week was filled by
  someone closer. Contracts expire. This is what makes information and
  position valuable rather than decorative.
- **No quest markers, because there are no quests.** There is work, and
  there are people who want things.

**The run is the unit, and most runs end badly.** A character will usually
die poor, having never run a company or held office. That is the genre, not
a failure to design out. What persists is the world — the same planet,
still holding the consequences of what previous characters did.

This reframes B2 succession: succession is not a safety net, it is the rare
good outcome, available only to a player who built something worth
inheriting. Dying with nothing positioned is the normal case.

### A4.2 Opportunities are side-effects of other lives

**Opportunities are never manufactured for anyone's benefit.** A vacancy
exists because someone was promoted, died, or was dismissed — decisions
those people made for their own reasons. This is C1.1's "no encounter
spawning" applied to opportunity: an opportunity is a real change in the
world that happens to be exploitable by an actor positioned to notice it.

**Consequence: opportunities are not distributed fairly, and most go
unnoticed.** An opportunity reaches only actors with belief coverage of it
(A3.3) — you must know the position exists and that it opened. Well-
connected actors hear early; isolated ones hear late or never. This is what
makes information networks mechanically valuable.

**Pushed, never scanned.** Systems post opportunities to the queues of
actors with coverage. Actors do not scan the world for possibilities — that
is what makes every NPC consider every action every tick, and it is the
primary performance failure to avoid.

Each entity holds a small **opportunity queue**, refreshed on events, with
entries that expire.

### A4.3 Goal selection with hysteresis

Real people do not re-evaluate their lives hourly. Switching goals carries
cost — sunk investment, reputation, relationships — so a new option must
**clearly** beat the current goal, not merely edge it.

This also prevents actors thrashing between goals, which is the usual
failure mode of naive utility AI.

### A4.4 Plans persist and are executed

A goal expands into a plan via a library of **hierarchical task templates**
(goal → subtasks → primitive actions). This generalises the military
hierarchical-intent mechanism to every actor.

Plans persist across ticks. **Entities execute plans; they do not re-decide
every tick** — expensive and unrealistic in equal measure.

### A4.5 Reconsideration triggers

An actor re-runs goal selection **only** on:

1. Plan step completed
2. Plan failed
3. High-salience new belief
4. A slow drift timer

The drift timer is what allows someone to quietly become dissatisfied with a
life that is going fine.

### A4.6 Plan failure is normal, not an error state

Per A3, actors plan on **belief**, and belief can be wrong or stale. The
freight you meant to intercept took a different route. The job was already
filled. The bridge was down.

Failure must be a **handled, common outcome** that feeds back into belief
("my information was stale") and triggers replanning. It is not an
exception path.

### A4.7 Reservations

Committing to a resource — job slot, tool, vehicle, dock, stockpile — places
an **expiring reservation**, so two planners do not claim the same thing.
Expiry matters: an actor that dies or abandons a plan must not hold a
resource forever.

### A4.8 Think budget

**The performance ceiling of the entire game.** Each region receives a
per-tick budget of "agent thinks", enforced as a hard cap, not a target.

| Tier | Thinking |
|---|---|
| Inside the reality bubble | individuals think often |
| Relevant regions | cohorts think as a group |
| Frozen regions | no thinking; formulas only |

### A4.9 Logistics is modern, and freight is not caravans

The setting begins at an Information Age baseline (B5). There are no
caravans. Freight is **scheduled, routed infrastructure**: trucks on roads,
rail, container shipping, air freight — each with capacity, speed, cost, and
an infrastructure dependency (roads, rail, ports, airfields, fuel supply).

**This makes logistics attackable through infrastructure rather than
interception.** You do not ambush the shipment; you drop the bridge, hit the
fuel depot, disable the port crane. This connects directly to the
infrastructure cascade system.

**Escort is a signal, not a baseline.** Nobody hires guards for a truck on a
maintained highway in a firmly-controlled region (C1.2). Armed escort exists
only where control is weak or contested — so the presence of escort tells
the player something about the route.

**Entry-level work is freight-economy work**: driver, dockhand, warehouse
labour, loader, dispatcher, mechanic. Running goods through contested
territory is the *dangerous, well-paid* option, not the starting one.

**World-gen consequence:** A1's "navigable waterways and road suitability"
pass must produce an actual routed network — highways, rail, ports,
airfields, with capacity and chokepoints. Trade routes are physical
infrastructure with specific vulnerable points, not abstract lines between
settlements.

### A4.10 Three kinds of movement, not one

Military convoys, commercial freight, and individual movement are
mechanically distinct. Conflating them produces wrong behaviour in all
three.

| | Military convoy | Commercial freight | Individual movement |
|---|---|---|---|
| **Purpose** | orders, scheduled by command | goods moved for a business, routed on cost and time | a person or family relocating, or a small trader carrying their own goods |
| **Defence** | armed, in formation, with a security posture | unarmed by default; escort only where control has degraded | unarmed, or lightly armed and untrained |
| **Cargo** | materiel | commodities | everything they own |
| **Loss means** | an operational problem | an insurance claim and supply disruption | personal and permanent ruin |
| **Attacking it is** | an act of war, with an attribution problem for the attacker | ordinary crime | atrocity against civilians |
| **Response** | military reprisal | investigation, maybe | often none at all |

**The response row is the important one.** Attacking military freight brings
a military answer. Robbing a truck brings an investigation, perhaps. Robbing
a family in a weakly-controlled region brings nothing — which is precisely
why C1's predatory groups target them, and why doing so generates the
grievance and kinship obligation that C1.7 makes self-sustaining.

Target selection runs through the ordinary opportunity primitive: **value
against defence against consequence.** No special-case logic per target
type.

---

## B1. Skills, Certification, Work — SETTLED

### B1.1 Model: DF specialists, not CDDA generalists

A **fine-grained skill list** (DF-style, ~100 skills) rather than a coarse
one (CDDA-style, ~30). Consequences, all intended:

- **Specialists, not polymaths.** Nobody is competent at everything. An
  individual masters a handful of skills in a lifetime.
- **Interdependence is forced.** You cannot personally do everything, so you
  must hire, partner, or go without. This is what makes organisations
  (and the position system) mechanically necessary rather than optional.
- **Identity comes from specialisation.** "I am a welder" is a real
  statement about a character, not a build choice.
- Slower progression per skill, and more UI surface. Accepted.

Skills are defined in **external data files** (raws-style), not hardcoded.
Adding a skill is a data change.

### B1.2 Skill list by reading system

Skills exist because a system reads them. Grouped accordingly:

**Trades** — read by construction, repair, and the incident-response system.
Electrical, plumbing, HVAC, welding, machining, carpentry, masonry, heavy
equipment operation. These fix infrastructure under time pressure and are
directly load-bearing for the blackout cascade.

**Logistics** — read by A4.9's freight economy. Driving (by vehicle class),
heavy vehicle operation, cargo handling, dispatch and routing, vehicle
mechanics, marine operation, aviation. Entry-level work lives here.

**Production** — read by the four-sector economy. Agriculture, food
processing, industrial machine operation, quality control, chemistry,
materials processing.

**Commerce and administration** — read by the organisation and position
system. Negotiation, accounting, market analysis, contract law, management,
personnel assessment. The route to a directorship rather than the floor.

**Technical and information** — read by A3's belief and comms layers.
Computing, networking, electronics repair, radio operation, data analysis,
cryptography. Makes information networks something a player can *build*
rather than merely possess.

**Medical** — first aid, trauma care, surgery, pharmacology, diagnosis.
Reads into A2.4's wound tiers.

**Combat and security** — small arms, marksmanship, close quarters,
explosives, fortification, tactics, unit command. **Tactics and command are
separate skills from shooting**, per the hierarchical-intent model — a
superb marksman may be an incompetent squad leader.

**Social and covert** — persuasion, deception, intimidation, observation,
tailing, infiltration, forgery. These read belief (A3) and values (A3.6);
A3.7 already gives infiltration a foundation via preference tells.

### B1.3 Competence and certification are separate

**Competence** is the actual skill value. **Certification** is a credential
issued by a guild, employer, or state.

The two can diverge in both directions: a skilled electrician with no
licence, or a licensed incompetent. This gap is modelled deliberately.

**Certification gates legitimate work, competence determines outcomes.**

| | Certified | Uncertified |
|---|---|---|
| **Competent** | full access to legitimate work | capable but locked out — drives informal and criminal economies |
| **Incompetent** | hired, then fails at the task | neither access nor ability |

In a firmly-controlled region (C1.2) the licence is what gets you hired at
all — so the competent-but-uncertified are pushed toward informal work,
cash jobs, and eventually crime. In weakly-controlled regions certification
matters little and competence is judged directly.

This is a significant driver of the informal economy and connects C1.2
governance to individual career outcomes.

### B1.4 Progression

Skills rise through **use** and through **deliberate training**. Training is
faster but costs time and usually money or an instructor — so it is a real
investment decision, evaluated through the ordinary opportunity primitive.

Instruction requires a more skilled instructor, which makes skilled
individuals valuable beyond their own output and gives organisations a
reason to retain and develop people.

### B1.5 Work and advancement

Work is a **contract** from an organisation or person: haul a load, work a
shift, do a repair, run a training. Work pays, consumes time, exercises
skills, and exposes the character to events.

Advancement follows the organisation and position system: worker →
supervisor → manager → director → owner or commander → office-holder, by
being the best-positioned candidate when a **real vacancy** opens (A4.2 —
vacancies are side-effects of other lives, never manufactured).

**Delegation:** past a scale threshold the player stops issuing primitive
actions and issues **intent** to subordinates, reusing the military
hierarchical-intent mechanism for civilian organisations. Commanding at
scale means setting policy and priorities for a hierarchy running on the A4
planner, with reports and exceptions surfaced upward.

---

## B3. Direction, Competition, Institutions — SETTLED

### B3.1 Direction comes from reading situations, not from offers

There is no win condition and nothing is pushed to the player (A4.1). The
gap this appears to create — *how does a player find anything to do?* — is
closed not by a direction system but by making situations legible.

**The world presents situations; the player forms intentions.** Available
paths include, without prompting or markers:

- Look for work
- Steal, scam, run a racket
- Start or join an insurgency
- Carve territory from a warlord's control
- Work a job, buy a house, raise a family, accumulate capital, start a
  business
- Enter a trade that is underserved in the area

**These are not tiers.** Working a job and raising a family is a complete
life, not the tutorial for warlordism.

**Starting region determines the option space, not the difficulty.** In a
firmly-controlled protective region (C1.2) warlordism is unavailable —
there is no vacuum. In a contested predatory one, the quiet life is
unavailable. Same rules everywhere; different subsets reachable. No new
machinery: C1.2's two axes already produce this.

**The gap is the opportunity.** A region with no competent welder pays well
for welding; one with six does not. Choosing a trade is therefore an
economic read of the region — which requires knowing the region, which is
A3 belief. Nobody announces a shortage. The player notices, or does not.

### B3.2 Competition has three axes

Competition is against **specific people**, not against a market rate — the
DF-specialist model (B1.1) means a rival is an individual with a skill
value, a reputation, and a contract history.

**1. Skill.** Being better at the work. Winning a contract from an incumbent
means persuading the client to switch: the client holds beliefs about both
parties (quality, reliability, price, personal relationship) which are stale
and imperfect (A3). Negotiation skill plus the rival's actual track record —
journalled events the client may or may not know about — determine whether
those beliefs update. Undercutting someone means either being genuinely
better, or being better at making yourself known. Both are playable.

**2. Cost structure and capacity.** A different axis entirely. A larger firm
wins not on quality but on doing six jobs at once at lower unit cost.
Organisations hold what individuals cannot:

- standing supply contracts with volume pricing
- equipment
- employees
- working capital — including the ability to **bid below cost briefly**,
  which is how firms actually kill competitors: undercut until the small
  operator's cash runs out, then raise prices

No special-case code — the opportunity primitive with a longer horizon and
deeper reserves.

**Scale advantages are earned and losable.** Bulk pricing comes from a real
supply contract with a real supplier, who can be outbid, disrupted, or cut
off when a road goes out (A4.9). Equipment breaks and needs the mechanic
skill. Employees leave if underpaid. A large firm is a structure that must
be maintained, and it is attackable through systems already specced.

**Small operators must retain real counters**, or the sandbox collapses into
"largest firm wins": work the big firm will not take (too small, too remote,
too contested), personal client relationships that pricing does not
override, physical proximity, and — in weakly-controlled regions — things a
legitimate company cannot be seen doing.

This also gives the protection racket its economic teeth: a firm bleeding
capital to extortion loses precisely the reserve needed to survive a price
war.

**3. Relationships.** Covered by reputation and belief; clients do not
switch on price alone.

### B3.3 Competition generates consequences

**A displaced competitor is a person with a future.** Take someone's
contract and they are a skilled person without work — which is exactly
C1.5's manpower condition. They may leave, retrain, resent you, undercut you
back, or in a weakly-controlled region do something worse.

They are individuated automatically, because the player mechanically
affected them (A2.5).

**Or the same evaluation produces partnership.** Two specialists cover more
work than one. The opportunity primitive weighs cooperation against
competition using values (A3.6): a high-greed, low-loyalty character
undercuts; a cooperative one partners. Same inputs, opposite action, no
special-case code.

### B3.4 Education pipelines — the supply of skilled people

Skilled people must come from somewhere. Three institutions, differing in
cost, duration, and output:

| Pipeline | Produces | Duration | Cost to learner | Certifies? |
|---|---|---|---|---|
| **Apprenticeship** | trades — welders, electricians, machinists | long | low or negative (apprentices do real work) | via sponsorship |
| **Vocational school** | trades, in cohorts | medium | moderate | **yes** — source of the licence gating legitimate work (B1.3) |
| **University** | scholars, engineers, physicians, administrators | very long | high | yes |

Apprenticeship requires a master with both skill and willingness to teach
(B1.4's instructor requirement).

**Universities are where research capability lives** — which matters for B5:
FTL breakthrough probability accrues from education, prior technology
access, and institutional backing. No universities, no breakthrough.

**Consequences worth the simulation cost:**

- **Skilled labour has a lead time.** Losing a region's only surgeon means
  years to replace, not a hiring round. Wars, epidemics, and emigration have
  long tails.
- **Institutions are infrastructure and can be destroyed.** Destroying a
  university does not merely kill people; it cuts the region's future supply
  of physicians and engineers. A strategic target with delayed effect — far
  more interesting than a warehouse.
- **Education is a class mechanism.** University costs money and years, so
  it is reachable from some starting positions and not others. An honest
  constraint on the player's path.
- **Brain drain is emergent.** Regions that train people but cannot employ
  or protect them lose them — feeding C1's displacement conditions.

For a player starting from nothing, apprenticeship is the realistic route.
University is what you fund for a child, or reach after capital.

### B3.5 Taking on an apprentice

The player can take an apprentice: labour at a fraction of certified rates,
in exchange for training and eventual certification sponsorship.

**For the master:** cheap labour now, paid for in supervision time and the
risk of botched work. An apprentice on an unsupervised job is a liability —
so there is a real limit on how many can be run, and exceeding it produces
failures that damage reputation.

**For the apprentice:** underpaid for years in exchange for skill and
certification. **Exploitable** — a master can string someone along,
extracting cheap labour while teaching little and never sponsoring
certification. This is a grievance generator inside an ordinary economic
relationship, and a real choice on both sides of it.

**As a competitive weapon:** a firm running apprentices undercuts one paying
certified rates — the cost-structure axis of B3.2. This is why guilds and
licensing bodies restrict apprentice ratios in reality; that restriction is
a policy lever a governing faction can set, with enforcement depending on
C1.2's control axis. In a weakly-controlled region, nobody is checking.

**The apprentice eventually becomes a competitor.** They know your clients,
rates, and methods. Whether they stay, partner, or set up across town runs
through their values and the opportunity primitive. Train someone well and
treat them badly, and you have built your own rival.

### B3.6 Open

- **Starting region: rolled or chosen?** Rolled fits the roguelike framing
  and makes each run's option space a discovery. Chosen lets a player
  deliberately pursue a path. Undecided.
- **Reputation** is load-bearing for all of B3.2 and B3.3 and is not yet
  specified anywhere. Needs its own section.

---

## B6. Reputation — SETTLED

### B6.1 Reputation is belief, not a score

There is **no global reputation number**. A global score would make every
actor omniscient about the player, contradicting A3.

What exists is **beliefs held by specific actors, with a person as the
subject**. Consequences, all intended:

- You can be trusted in one town and notorious in the next.
- Outrunning a bad reputation by relocating is legitimate.
- Reputation spreads by the ordinary report mechanism (A3) — witnesses,
  gossip, records — travelling along social and trade connections, not
  instantly.

**No new machinery is required.** Reputation *is* A3 belief with a person as
the subject: timestamps, sources, confidence, and decay already apply.

### B6.2 Dimensions

Reputation is **multidimensional**. These come apart constantly, and the
characters worth remembering are those whose dimensions disagree — a feared
enforcer has poor honesty and excellent dangerousness, and both are assets
in his trade; a drunk who does beautiful work has high competence and low
reliability. Collapsing them into one score destroys exactly that texture.

**Core five** — build first:

| Dimension | Question | Read by |
|---|---|---|
| Competence | are you good at the work? | hiring, contracts (B3.2) |
| Reliability | do you deliver? | contracts, employment |
| Honesty | do you cheat? | trade, negotiation |
| Loyalty | do you sell people out? | organisations, factions |
| Dangerousness | will you hurt someone? | target selection, intimidation |

**Additional five** — add as the systems reading them are built:

| Dimension | Question | Read by |
|---|---|---|
| Solvency | do you pay, and can you? | credit, supply on account, deferred contracts. Distinct from honesty — an honest man who is broke is still a bad risk. Becomes essential once B3.2 price wars exist, since those run on reserves. |
| Discretion | do you talk? | information networks (A3), criminal and covert work. Distinct from loyalty — one can be wholly loyal and still a gossip. |
| Connectedness | who would answer if you called? | whether harming you is safe. Not a virtue. A well-connected mediocre person is a worse target than an isolated dangerous one. |
| Legitimacy | are you seen as belonging here? | C1.7's in-group loyalty radius. Outsider / migrant / local / established family. **Migrants cannot improve this quickly** regardless of other qualities. Gives A3.7's preference tells their weight. |
| Ideological standing | are you sound on what this population cares about? | insurgency recruitment, political office, trust in a divided region. Generalises beyond religion — it is standing relative to whatever the region's dominant commitments are. |

**Deliberately excluded:** generosity (folds into personal feeling toward
you) and temperament (better modelled as observable behaviour feeding
dangerousness and reliability).

### B6.3 Habits and conditions are causes, not reputation entries

Alcoholism, addiction, and similar conditions are **not** reputation
dimensions. They are conditions affecting performance and behaviour, which
*produce* missed work and botched jobs — and those observable events are
what actors form beliefs about.

Nobody observes "alcoholic". They observe a man who did not show up on
Tuesday.

**This means a condition can be concealed for a period**, until its
consequences accumulate into a reputation for unreliability — which is more
interesting than a visible flag, and gives the player something to hide or
to detect in others.

---

## B2. Death, Succession, Longevity — SETTLED

### B2.1 Permadeath is the default

Death is permanent. There is no respawn and no automatic continuation.

**Succession comes from family only** — a child. There is no protégé,
lieutenant, or institutional-successor mechanism. If there is no child, the
run ends and the world persists (A4.1: the run is the unit; the world
remembers).

This means the normal outcome is permadeath, and continuity is something a
player must have deliberately built.

### B2.2 What survives you is what you built

Because succession runs on inheritance rather than position, **the
ownership/position split (A2, organisations) determines continuity, not just
authority.**

| Inherits | Does not inherit | Partially inherits |
|---|---|---|
| Property | Skills and competence | Connectedness — contacts and enemies both know the family |
| Business ownership | Competence reputation | Legitimacy — an heir starts known, for better and worse |
| Debts | Certifications | |
| Family legitimacy in the region | Office | |
| **Grievances and kinship obligations** (C1.7) | Military rank | |

A player who built a business leaves something. A player who reached CEO of
someone else's company leaves a name — the board selects the next holder,
not the deceased.

**Grievance inheritance is the important one.** Blood feuds outlive the
people who began them, which is what makes conflicts recur across
generations rather than resetting. This is the mechanism C1.7's kinship
obligation depends on.

### B2.3 You play the heir from whatever age they are

If the heir is six, play resumes at six. No skipping to majority.

This is viable because the education pipelines (B3.4) already exist —
childhood is not dead time. It is the one life stage in which schooling and
apprenticeship are *experienced* rather than observed, and per A3.6 values
are formed by where a character is raised, so a child character is having
their values set during play.

**Constraints while a minor:**
- No legal capacity — cannot own, contract, or hold position
- Cannot take adult employment
- Physically weak; poor combat outcomes
- Education is the primary available activity

**Time compression (A1.7, strategic mode) carries the years.** Childhood is
playable, not endured in real time.

**The guardian is a genuine vulnerability.** Someone holds the estate until
majority, and that guardian is an ordinary NPC running the opportunity
primitive (A4) with an inheritance within reach. They may steward it
faithfully, mismanage it, or strip it. Coming of age means recovering
whatever remains.

This is emergent drama requiring no bespoke system — it is the position
system, values, and the opportunity primitive meeting an unusual
circumstance.

### B2.4 Longevity technology

A **late-game commodity**, not an unlock. It is an alternative to succession
for those who reach the capital and technology to afford it — most runs will
never see it. It postpones permadeath for a wealthy few, which is an honest
outcome rather than a designed escape hatch.

Per B5.2, the technology itself must be *discovered* through the same
career-and-institution mechanism as FTL. It does not exist at the
Information Age baseline; a faction must develop it, requiring universities
and funding (B3.4). It therefore emerges mid-game.

#### Physical age is a separate stat from chronological age

| Stat | Drives |
|---|---|
| **Physical age** | strength, endurance, injury recovery (A2.4), fertility, remaining lifespan |
| **Chronological age** | accumulated skills, memory, reputation history, how others regard you |

Everything mechanical reads **physical age**. Chronological age is only how
long the character has existed.

**The gap between them is itself information.** A physically-30 individual
holding 80 years of accumulated skill and reputation is visibly anomalous to
anyone who knows their history — a wealth signal, a target marker, and, for
someone arriving in a new region without history, an identity puzzle. Nobody
in a new town knows you are eighty.

**Fertility reads physical age**, which interacts with B2.1's
succession-by-children-only rule: a reverted individual can start a new
family at chronological 80.

#### The drug: doses stack, and 50 is a threshold

The treatment restores telomeres. It is bought in doses at different price
points (e.g. 10 / 20 / 30 years) which **accumulate into a stack**.

**Below 50 stacked — ordinary extension.** Doses add years of lifespan, and
they **deplete as they are lived through**. Buy 20, live two years, hold 18.
Maintaining extension therefore means buying faster than you consume — an
ongoing financial commitment, not a purchase.

**At 50 stacked — reversion triggers.** Physical age drops to a healthy
twenties. The stack is consumed. Aging then proceeds normally from the new
physical age.

#### Yield scales with age at trigger

The 50 is **reversion capacity, not years purchased**. How much life is
actually gained depends on how old the body was when it fired:

| Age at trigger | Physical age after | Life recovered |
|---|---|---|
| 80 | ~20 | ~60 years |
| 70 | ~20 | ~50 years |
| 60 | ~20 | ~40 years |

Same product, same price, **more value the later it is taken** — because it
recovers years already spent.

#### The resulting tension

Three pressures pulling against each other, which is the point:

1. **Waiting increases yield.** Triggering early wastes much of the stack's
   value.
2. **Waiting risks everything.** Death by violence, accident, or disease
   before the stack completes wastes all of it. **Age-related illness is a
   specific and rising risk** — the longer a character holds out for better
   yield, the more likely age itself produces a condition that kills or
   disables them first.
3. **The stack depletes while accumulating.** Reaching 50 requires buying
   faster than living, which is only possible during a stretch of real
   surplus wealth. Holding at 50 without triggering is not possible; the
   stack drains underneath.

A character at 48 stacked and age 79 occupies a very specific and very tense
position.

**Class dimension:** the wealthy do not merely live longer — they *time it
well*, because they can hold reserves and wait. Someone forced to treat
early by a health crisis gets poor value.

#### It is a commodity, with a commodity's fragility

The drug obeys ordinary supply and demand (economic accounting,
SPEC-LATER). It is not abstracted:

- **Production is a facility** requiring power, inputs, and skilled staff —
  fully exposed to the infrastructure cascade.
- **Distribution is freight** (A4.9) — a cut road or lost port interrupts it.
- **The facility is a physical place**, and a war can be fought around it.
- **Price responds to scarcity**, pricing out all but the wealthiest during
  wartime disruption.

**Consequence:** an extended character is dependent on the stability of a
world they may be actively destabilising. Start a war and you may sever your
own supply. No special-case code — the ordinary commodity system with a
lifespan attached.

**It is also an outstanding target.** Not only the facility but the *supply*
to specific individuals. Learning which powerful figures are dependent tells
an adversary exactly what to interdict — killing them slowly without an
assassination. An espionage and warfare objective that emerges rather than
being designed.

#### Open

- Floor on physical age after reversion (proposed: ~20).
- Whether dependency is observable to others — if discoverable, it becomes
  leverage, which fits the belief (A3) and reputation (B6) systems well.

---

## B5. FTL Discovery — SETTLED

### B5.1 Timeline

- Gameplay **begins pre-FTL**, at the Information Age baseline.
- The initial history simulation runs a **bounded span** (decades to roughly
  two centuries — a generation parameter) and **stops before FTL by
  construction**.
- FTL is a **late-game emergent milestone**, discovered during play.

"Space is deferred" means star systems, alien civilisations, and ship
interiors are not being *built* yet. It does not mean FTL is unreachable —
it slots into the reserved `star system → universe` levels (A1) when those
are built.

### B5.2 Breakthrough probability accrues over a career

**Not a birth lottery.** Birth sets potential; the breakthrough probability
accumulates during an individual's career from:

- Education level and institution quality (B3.4 — no universities, no
  breakthrough)
- Access to prior technology
- Institutional backing
- Funding
- Personal traits and values (A3.6)

This makes the breakthrough something factional investment can genuinely
influence, and gives universities strategic weight beyond training
physicians.

### B5.3 Diffusion after discovery

Once one faction holds FTL, others acquire it by two channels:

- **Trade and relations** — diffusion over time, probability rising with the
  strength of trade ties and diplomatic relations. Largely free: the
  inter-faction relationship data already exists.
- **Espionage** — faster and riskier. Success or detection resolves against
  the attacking faction's espionage investment versus the target's
  counter-intelligence. A detected attempt carries real diplomatic and
  conflict-state consequences (C1.2).

### B5.4 The discoverer individuates

A world-changing breakthrough is a **third individuation trigger** alongside
attention and consequence (and A2.5's mechanical effect): the individual who
achieves it is generated as a named person and the event is journalled into
history, as DF does with its legendary figures.

---

## B4. Individuation Bounding & Archival — SETTLED

### B4.1 The problem

Individuation triggers only ever run one direction. A statistical person
becomes a permanent individual on **attention** (inspection),
**consequence** (dying in an engagement the player was near), **achievement**
(B5.4), or **mechanical effect** (A2.5 — wounded, robbed, hired).

Each is reasonable alone. Together, over a long run, they grow the live set
without limit: every casualty, every rival for every position, every person
the player ever injured — all carrying values (A3.6), preferences (A3.7),
beliefs (A3), relationships, and a memory log.

**Bounding caps how many stay fully live at once, without breaking the
promise that they persist.**

### B4.2 Populations are never instantiated wholesale

Per A2.6: forces and populations are **counts plus a seeded roster spec**.
Individuation materialises only the people a trigger actually touches — the
commander inspected, the casualties in *this* engagement, the rival
candidates for *this* vacancy. Never the whole roster.

This is the first and largest bound. Everything below concerns those already
materialised.

### B4.3 Archive, never delete

When the live set exceeds its cap, the **coldest** individuals compress to
an archived record. They are never deleted.

DF caps historical figure count at world generation and culls unimportant
ones — a hard cap plus deletion. We archive instead, because deletion breaks
the persistence promise precisely for people the player may return to.

### B4.4 What compresses, and what must not

The distinction below is where DF's fast-travel healing exploit lived (see
A2.1). Getting it wrong reproduces that bug.

**Never lossy — compresses in storage, never in content:**
- Identity
- Traits and values (A3.6)
- Preferences (A3.7)
- Relationships
- **Anything the player mechanically caused** — wounds (A2.4), debts owed,
  property, grudges

These are stored packed rather than as live objects, and **rehydration must
reproduce them exactly**.

**Digestible:**
- The memory and thought log. A live individual accumulates hundreds of
  small events; an archived one retains a digest preserving the
  emotionally significant and collapsing the rest.

This is A2.7's journal compaction applied per person. The digest must be
**deterministic** — the same input always producing the same digest.

### B4.5 The rehydration contract

Stated as a test, not a principle:

> **archive → rehydrate → compare.** Every field in the never-lossy set must
> be exactly equal.

Run against randomly selected archived individuals in debug builds,
unconditionally. This is what prevents the exploit reappearing quietly
months into development.

**Specific case to assert:** evicting someone the player wounded and
rehydrating them without the wound is DF's exploit rebuilt. A2.5 (the
mechanical-effect trigger) and B4 (the cap) pull in opposite directions —
one grows the set, the other shrinks it. This contract is what resolves that
tension safely.

### B4.6 Archived individuals still live

The persistence promise requires that archived people continue to age and
change status — they can die, retire, relocate, be conscripted, change
career.

They do so on the **cheap region-tick formula, never by thinking** (A4.8:
frozen tiers run formulas only). An archived record therefore carries a
**last-updated timestamp** and is caught up on rehydration, exactly as
frozen regions are (A1.7).

Same machinery, third application.

### B4.7 Eviction policy

Evict the genuinely cold — longest since interaction. **Never evict:**

- anyone in the player's active relationship web
- anyone in an unresolved event chain
- anyone with unfinished business against the player

These are precisely the individuals whose absence or inconsistency would be
noticed.

LRU ordering among the remainder.

### B4.8 The cap is measured, not guessed

The per-region live cap depends on what an individuated NPC actually costs
once belief, values, preferences, relationships, and a memory log are
attached. **Placeholder pending measurement.**

Set it only after profiling a realistic individual record. Document the
measured per-individual cost alongside the chosen cap, so the number can be
revisited when the record grows.

---

## B7. Health, Genetics, Lifestyle — SETTLED

Health outcomes are **caused**, not rolled. Three inputs interacting; no
single one determines the result. Good genetics with poor lifestyle in a bad
region still ends badly.

| Input | Source |
|---|---|
| **Genetics** | inherited, rolled at generation, partially heritable through family lines — children carry some of a parent's predispositions (B2) |
| **Lifestyle** | the individual's own accumulated behaviour: diet, work type, substance use (B6.3's conditions live here), physical activity, stress |
| **Environment** | what the region supplies: food quality, air and water, occupational hazard, medical access |

### B7.1 Public health as a societal axis

Whether a society prioritises population health or extracts from it is a
**regional characteristic with measurable results**. A region may have
plentiful food that is nutritionally poor, high employment in hazardous
industry, and no preventive medicine — while a poorer region produces
longer-lived people.

**This is governed by C1.2's existing disposition axis** (protective /
extractive / predatory). An extractive state does not regulate food quality
or workplace safety, and its population's health reflects that. No new axis
required.

### B7.2 Delayed feedback is the point

Poor health policy manifests as a shorter-lived, less capable workforce **a
generation later** — long after the decision that caused it, and long after
whoever made it has moved on.

This is one of the few systems in the design with a multi-decade feedback
loop, and it is worth preserving as such.

### B7.3 Interaction with longevity treatment (B2.4)

Age-related illness risk is the pressure that makes B2.4's "wait for better
yield" strategy genuinely uncertain — and that risk is **individual**,
derived from genetics, lifestyle, and environment rather than a flat curve.

**Regional consequence:** unhealthy populations need longevity treatment
more and can afford it less.
