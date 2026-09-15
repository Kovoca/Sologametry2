# Scale Sim — status and plan

**The authoritative record of what is built, what is not, and what order
the rest goes in.** Where this file and the specifications disagree, this
file describes the repository and the specs describe the target.

It exists so that neither the owner nor an assistant has to hold the state
of the project in their head. `CLAUDE.md` records the *reasoning* and the
calibration figures; `docs/session-log.md` records how it got here; this
file records **where it is and where it is going**.

---

## Refresh the numbers before trusting them

A number somebody types goes out of date silently, in whichever direction
flatters. Every figure in the header below is produced by a command, and the
command is given so it can be re-run rather than believed.

```bash
git rev-parse --short HEAD
ls src/*.rs | wc -l ; ls src/bin/*.rs | wc -l
cat src/*.rs src/bin/*.rs tests/*.rs | wc -l
cargo test --release 2>&1 | grep -E "^test result" \
  | awk '{gsub(/;/,""); p+=$4; f+=$6; i+=$8; n++} \
         END {print "binaries:", n, "passed:", p, "failed:", f, "ignored:", i}'
```

Do not pipe the suite through `tail` to see the summary — it truncates away
the per-binary results the total is built from. That mistake was made while
writing this file.

| | | measured |
|---|---|---|
| Commit | `7f82ef7` + skill | 2026-09-15 |
| Source | 66 modules, 17 diagnostic binaries | 2026-09-15 |
| Lines | ~109,000 including tests | 2026-09-15 |
| Tests | 85 binaries, **880 passed, 0 failed, 0 ignored** | 2026-09-15 |
| Build | clean | 2026-09-15 |

**The full release suite takes over ten minutes**, which is worth knowing
before planning a change that touches every module — and the travel-time
work below had to run it three times.

The total is measured across every binary including doctests. Counting
`#[test]` attributes statically instead gives a smaller number and misses
the doctests — which is where this project keeps its `compile_fail` proofs,
so it misses precisely the ones that matter most.

---

## The goal

From `docs/scale-sim-design-doc.md`: a single-player, solo-built Rust
simulation where the player **drops into a procedurally generated universe
as a single individual and shapes it to the degree they are able** — which
means: if they have the means they can shape as they please, and if they do
not, they cannot. Nothing is scaled to them.

- **Interaction is Dwarf Fortress adventure mode / CDDA.** You stand
  somewhere, you see and hear what is near you, you talk to people, you pick
  things up, take them apart and make things out of them.
- **Scale target** is individual to intergalactic; **build order starts
  planetary** and expands outward. Space is deferred on purpose.
- **A modern technological baseline**, not a medieval growth arc.
- Tarn Adams' four principles: do not overplan; model independent underlying
  elements and let results emerge; nothing exists unless it has
  player-perceptible impact; base the model on real-world analogs.
- This project's own additions: **calibrate against real published figures
  rather than taste**, and **delete a mechanism and require its gate to go
  red** before believing the gate tests anything.

---

## The scope, as stated by the owner

Recorded here because it is the target the tables below are scored against,
and because it existed only in conversation before this.

| | wanted |
|---|---|
| **Interaction** | DF adventure mode / CDDA: walk, look, talk, take, make. "To the degree they're able" means means, not permission |
| **Economy** | realistic micro **and** macro. Finance, banks, **stock markets** |
| **Industry** | factories producing goods through **labour and machinery**; refining, so oil and chemical; **assembly lines and machinery as multiple tiles working in tandem** |
| **Networks** | electrical grids, **water systems**, **telecoms** |
| **Vehicles** | individual to cargo to **mining equipment** |
| **Marine** | boats from individual to **industrial cargo and fishing**; **oil rigs** |

Bold is what does not exist yet. The rest is built or substantially built.

---

## Where each area actually stands

**Built** means it exists and is gated by tests. **Partial** means the
mechanism exists but not the scope wanted. **Absent** means not started.

### World

| area | state | what exists | what is missing |
|---|---|---|---|
| Terrain, climate, hydrology | **built** | elevation, erosion, rivers, lakes, rainfall, temperature, water table | — |
| Geology | **built** | rock provinces, ore/coal/petroleum grades, soil fertility and depth | — |
| Biota | **built** | Miami-model NPP, biomass, game, timber, six crops, seven livestock | `biota.game` has no consumer; nothing hunts |
| Polities and settlement | **built** | emergent borders, siting on reachable water, real rank-size, villages | no history sim; polities never consolidate |
| Roads and networks | **built** | traffic-derived classes, passes and tunnels, maintenance deficit | rail, ports as places, airfields |
| Scale ladder | **built** | region cell to locality to plot to tile, arithmetic asserted | — |

### Economy

| area | state | what exists | what is missing |
|---|---|---|---|
| Commodities and production | **built** | 18 commodities, 32 recipes, 21 site kinds, conserved ledger | more of the tree: limestone, aggregate, copper |
| Prices and trade | **built** | cost separated from price, nine typed value kinds, arbitrage bounds | — |
| Freight and logistics | **built** | Dijkstra routing, cargo in transit, road capacity booked, **transit time reads the road surface leg by leg** | multi-drop rounds; the quay-to-town leg is priced, not ridden; road *condition* raises cost but not yet time |
| Money | **built** | one write path, conserved, four account kinds, unpaid tracked by reason | firm-to-firm unpaid is the largest open category |
| Banking and credit | **built** | loans create deposits, capital adequacy binds, real credit tiers | central bank, policy rate, bonds |
| **Stock markets** | **absent** | firm *form* is known — sole trader, partnership, company | shares, valuation, issuance, an exchange, ownership registers |
| The state | **built** | one per nation, real tax capacity, public payroll, services | provinces — no rung between town and nation |
| The border | **built** | import and export parity, duty hook, exchange rate, capital account | a currency per nation; reserves |
| Labour and services | **built** | recipe-derived headcount, contracts, rotas, promotion, sectors, **skill reaching what a works makes** | retail headcount about 4.9 points short of real; skill does not yet reach scrap |
| Households | **built** | basket by need, repair/reuse/second-hand, utility arrears, scrap | no "eating cheaply"; buys on credit nobody extended |

### Things

| area | state | what exists | what is missing |
|---|---|---|---|
| Materials | **built** | 39 materials, properties, hazard, recycling routes | — |
| Items | **built** | definition/instance split, seven content kinds, full BOMs, quality apart from condition | — |
| Crafting | **built** | capability-with-a-figure tools, three kinds of time, 13 outcomes, batch faults | — |
| Work in progress | **built** | real intermediate objects, identity rules, interruption per process | — |
| Teardown and scrap | **built** | nine intentions, joint table, recovery grades, real scrap prices | — |
| Scheduling | **built** | benches booked by the **minute**, capacity, outage per process | **nothing drives its diary** — durations exist, a tick to spend them in does not |
| **Machines as tiles** | **absent** | seven fixtures, all retail: till, shelving, rack, dock, counter, chiller | footprint, input and output ends, conveyors, a line in tandem |
| **Weapons** | **absent** | a rifle exists as an item with a real bill of materials | combat, projectiles, wounds |
| Fields and hazards | **absent** | — | fire, smoke, gas, flooding |

### Infrastructure

| area | state | what exists | what is missing |
|---|---|---|---|
| Electrical grid | **built** | five levels, N-1 transmission, faults sized by level, merit order, spares, mutual aid | routing to every meter; sub-day clearing |
| **Water and sewer** | **absent** | the water *table* is real ground truth; "sewer" is a Z level under a street | mains, treatment, pressure, drainage as graphs |
| **Telecoms** | **absent** | one `bool`, `comms_up`, gating fault reporting | nodes, backhaul, coverage, capacity |

### Vehicles and craft

| area | state | what exists | what is missing |
|---|---|---|---|
| Road vehicles | **built** | 13 part kinds, capability from parts, structure severs, centre of mass, real drag | ordinary cars, doors and windows, driving, collision with the world |
| **Mining equipment** | **absent** | `LandGear` is the only working attachment | excavators, loaders, draglines, haul trucks |
| **Boats** | **absent** | sea freight is a share of cargo value; berth depth reads the sea floor | hulls, holds, nets, engines — any vessel as an object |
| **Oil rigs** | **absent** | oil fields are land sites; `offshore` means wind turbines | platforms, derricks, offshore logistics |

### Mind and people

| area | state | what exists | what is missing |
|---|---|---|---|
| Personality and emotion | **built** | measured Big Five substrate, appraisal, concerns apart from episodes | — |
| Memory and witness | **built** | provenance, reconstruction, line of sight and acoustics | — |
| Needs, relations, speech | **built** | semantic satisfaction, eight-dimension relations, no telepathy (compile-proofed) | — |
| Growth, coping, scaling | **built** | doubt before change, strain ladder, fidelity-invariant advance | — |
| Conversation | **built** | assembled from memory, witness, relations, coping; no privileged channel for the player | — |
| Population | **built** | sampled cohorts, ageing, births, education, inheritance | a sampled pocket is not the household pool |

### Persistence and spine

| area | state | what exists | what is missing |
|---|---|---|---|
| Durable identity | **built** | `Id<T>` arenas, `Key<T>` registry, seven rules each sabotage-gated | — |
| Save format | **built** | hand-rolled, explicit wire codes, floats as bits, four stores, crash points tested | — |
| Ground overlay | **built** | per-layer changes, base hash, rebase refusal | — |
| Economy codec | **built** | every enum variant named, impossible worlds refused | — |
| The root | **partial** | one clock, one day order, **four of eight parts saveable**: clock, seed, ground, economy | the item store, the people, buildings and utilities, vehicles and work orders |
| **Play** | **absent** | generators and 16 inspection binaries | **no player, no commands, no turn loop, no save loop** |

---

## One clock, a tick nothing has chosen, and a diary nothing drives

**This section said "there are two clocks" and that was the wrong name for
it.** A 24-hour and a 12-hour clock are one scheme presented two ways, and
minutes are not a rival unit to seconds — in CDDA a turn is a second *and*
every action carries a duration measured in those turns, so a craft, a
butchering or a long read runs over many turns and can be interrupted.
Durations attached to actions are the same clock at a different magnitude,
which is what this project already has:

```text
game.rs      day: u64      private, no setter, `advance` the only mover
schedule.rs  minutes       setup, paid and owner minutes, fatigue over a
                           540-minute day, tool wear per minute, spoilage
                           after N minutes, outages split at the exact minute

35 minutes of oven      = 2,100 ticks at a second
a 540-minute working day = 32,400 ticks
```

So `schedule.rs` is not a competing clock. It is a set of **CDDA-shaped
activity durations already written** — and nothing advances its diary. What
is missing is a conversion and a driver, not a reconciliation.

The economy meanwhile has **no sub-day time at all**: every "minute" in
`econ.rs` is prose in a doc comment, and the only sub-day structure in the
whole world model is `day % 7` for the weekday.

What actually has to be decided is narrower than it first looked:

- **The base tick.** A second, or near it — a conversation, a fight, a door,
  a tile of walking all live there. The 24h/12h clock is display.
- **Durations on actions**, which `craft.rs` and `schedule.rs` already carry
  and which only need expressing in ticks.
- **What the rest of the world does meanwhile** — the one genuinely open
  question. The player spends 2,100 ticks at an oven and the economy cannot
  step 2,100 times. CDDA answers this with the reality bubble: a small region
  live, everything outside caught up on return. **This project has already
  built the proof that catching up is safe** — `scaling.rs` and
  `coping::advance` assert that an analytic advance over an interval equals
  that many daily steps, split at every discontinuity, with the one
  irremovable divergence named and its direction gated. Built for minds,
  never asked of the economy.
- **The day order becomes a time-of-day order**, and this is the piece that
  stays real work. `game.rs` encodes two rules learned the hard way: shops
  trade before anybody counts who worked, and a service is paid before wages
  fall due. Once the player can act at 14:30, "has the shop restocked yet?"
  has an answer and those become clock rules. A modelling decision, not a
  refactor.
- **The grid wants an intermediate rate on its own merits.** Merit order
  clears every half hour in life, and the whole point of a merit order is
  that the marginal plant moves through the day. Clearing once a day means
  the evening peak does not exist.
- **Most of the model must not move to the tick rate**, and not for
  performance — because it would be meaningless. There is no wheat price at
  14:30:07.

**And the cost of a day is measured, and roughly cubic in towns** — 16
markets 1.4 s per 100 days, 40 gives 14.1 s, 80 gives 116.7 s, 160 gives
1,489.5 s. That is what blocks the regional rung, and it constrains how much
of the world can be live at once under any clock.

---

## Tracked gaps

Real defects, each visible in a test or measurable in a binary.

1. **Firm-to-firm supply goes unpaid**, about 2.6e11 over 700 days — now the
   largest unpaid category, ahead of households at the till.
2. **A household buys the basket whatever its balance.** `consume_households`
   records the shortfall as unpaid, so a poor town eats like a rich one on
   credit nobody extended. Fixing it moves hunger.
3. **A sampled person's pocket is not the household pool.** Promoting
   somebody to detail creates their savings. The reification problem, not an
   accounting one.
4. **Profit is still about half of household income** against a real ~40%,
   because firms pay no rent, interest or depreciation — all three fall into
   the residual.
5. **Retail headcount about 4.9 points short**, because shop fixtures are
   sized on commodity tonnage and real retail follows customers and floor.
6. **The water table is too deep in the typical cell** — median 85 m against
   a far shallower reality. It cuts about 60% of candidate settlement sites.
7. **`biota.game` has no consumer.** Generated, and nothing hunts it.
8. **No history sim.** Polities are partitioned geographically and never
   consolidate into great powers.
9. **The trade multiples are the floor of the observed band**, not its
   middle; recentring at 8 days of food is a change to be measured.
10. **`GameState` saves four of eight parts**, measured by serialising rather
    than typed in.

---

## The plan

Ordered by what unblocks the most, and by what would have to be built twice
if taken out of order.

### 1. The clock and the player — one job, not two

You cannot have a player who acts without deciding what a tick is, and the
tick decides what everything above it is allowed to assume.

Deliver: a base tick of about a second; the durations `craft.rs` and
`schedule.rs` already hold expressed in it; a reality bubble with catch-up
outside it, built on `scaling.rs`'s invariance contract; the day order
re-expressed as a time of day; and a player who can stand on a tile, move,
look, talk and pick things up.

**Measure first** — what actually has to run per second, per minute, per day.
A guess about a hot loop is not evidence: memoising `share_out` looked
obvious and took 80 markets from 116.7 s to 206 s.

### 2. Machines as tile-placed objects

Unlocks factories, refineries and assembly lines in one move, and makes four
already-built modules pay off — `craft`, `schedule`, `wip` and `ground`.
Needs the clock first, because a line is stations passing work along in
minutes. It also gives water and telecom nodes somewhere to hang.

### 2b. Compliance — the missing half of every rule in the model

The thread running through the owner's scope statements, and it is **one
concept rather than three**. In each case the physical half is built and
the half where somebody decides whether to do it properly is absent:

| | the model has | the model lacks |
|---|---|---|
| production and efficiency | durations, throughput, **skill reaching output** | skill reaching *scrap* — `craft.rs` has the yields, nothing consumes them |
| travel, and rules that keep it safer | transit days, four real regulations | the choice to break them, and anyone watching |
| officials doing their job | `Capacity` — what a state can reach | an office, discretion, and its abuse |

**Every road rule in this model is obeyed automatically, and a rule obeyed
automatically is not a rule — it is a constant.** Four are already in and
each costs throughput, which is the point of them: an artic limited to
90 km/h when it has the engine for 126; nine hours' driving as the legal
maximum against seven as a working day; 2.55 m of legal width; and the
oversize ladder that makes a borrowed transformer take three weeks rather
than three days. Nobody can run over hours, nobody can be overweight, and
there is no enforcement to evade.

`custom.rs::will_bend(regard_for_law, dutifulness, gain, chance_seen)` is
already that primitive, with certainty deterring and severity deliberately
not a term. It was written for fly-tipping and is wired to nothing with
money in it.

**And the shape of it is a distribution, not a type** *(the owner's
correction, and it matters)*:

- **Rules are generally followed and bent sometimes.** The ordinary case is
  compliance; breaking is the exception, and `will_bend`'s `gain` gate
  already says nobody bends a rule for nothing.
- **A criminal is a derived label, not a stored flag** — somebody whose
  rate of breaking is high and whose offences are serious. That is exactly
  `relations.rs`'s existing discipline, where `friend = true` is stored
  nowhere and friend, rival, creditor and employer can all be true at once.
- **Or somebody who bends enough to avoid jail by keeping it hidden**,
  which is a high-gain low-visibility offence and is the same arithmetic as
  an official with discretion. One mechanism, both cases.
- **Severity decides the response, not the deterrence.** The existing rule
  — certainty deters and severity mostly does not — is about whether
  somebody does it. What severity settles is what happens when they are
  caught: a ticket, a court, a cell. Three different consequences with
  three different costs to a life, and none of them a deterrent term.
- **The motive is ordinary.** Somebody speeding is late, or impatient — not
  wicked, and not a criminal. The `gain` has to come from the situation
  rather than from character, or the model has only villains.

So: enforcement density scaled by `Capacity`, `will_bend` where there is
money, and a severity table for consequences. **This is what makes offices
and regime change possible**, and it is why it sits before them.

### 3. The save loop

The root saves four of eight parts. The item store and the people are the two
that make a session resumable at all.

### 4. Boats, rigs and mining plant

Extensions of `vehicle.rs` rather than a new system: capability from parts
already works, and `fitted.rs` already makes parts individual objects that
wear out and get replaced. A trawler is frames, engine, hull, hold and nets.

### 5. Water and telecom networks

`power.rs`'s five-level hierarchy with faults sized by where they happen is
the template, and it is a good one. Both are graphs with condition and decay,
which `state-and-economy-spec.md` already asks for.

### 6. Stock markets, and a currency per nation

Both need what precedes them. A tradeable share needs firms to be valued
entities. A currency needs the per-nation state — now built — plus a second
money, a conversion on every cross-border payment, and a conservation rule
spanning both. **The tonnage ledger must not change**: tonnage is physics,
not an institution, and one ledger for the planet is what makes a cargo
leaving one country the same tonnes arriving in another.

### 7. Making a day cheap enough for a region

Roughly cubic in towns. Until it comes down, `Nations::build` holds 16
markets out of 3,000 generated settlements, and a regional rung would be a
field nothing reads.

---

## Calibration anchors

Every figure in the model is anchored to a published or measured real value.
The full list is in `CLAUDE.md`; these are the ones most of the rest hangs
off.

| | real value | used for |
|---|---|---|
| Earth land mean temperature | 8.5 °C | climate calibration |
| Earth land mean precipitation | 715 mm | rainfall scale |
| Miami model NPP | land mean ~700 g/m²/yr | all vegetation |
| French–Schultz | 22 kg/ha/mm, 80 mm loss | crop yield |
| BF-BOF steel | 1.4 t ore + 0.8 t coal, 200–300 kWh | the whole metal chain |
| World crude steel | ~230 kg/head/yr | goods composition |
| Labour share of value added | 51.8% of US GDP, 55–60% OECD | the wage–price link |
| Wage curve elasticity | −0.1 | pay against local unemployment |
| US current account | in deficit every year since 1976 | the exchange-rate band |
| BIS FX turnover | $7.5tn/day against ~$66bn/day of trade | why only the *real* rate is modelled |
| UK road freight | 1.6 bn t, 150 bn t-km, ~94 km mean haul | freight sizing |
| Dock turnaround | 45–60 min, one at each end | the fixed half of carriage |
| Building Bulletin 103 | 350 m² + 4.1 m²/pupil | school size |
| Hospital | 47.5 m²/bed | hospital size |
| Government employment | UK 17%, US 14%, FR 21% | state size |
| Public staffing ratios | health, education, admin 1 in 45 each | the public payroll |
| Graduate ability | +0.67 SD | education gate |
| Modified OECD scale | 1.0 / 0.5 / 0.3 | household costs |
| Nursery cost | 65% of a median wage | childcare |
| Customer minutes lost | UK 35/yr, DE 12, US ~90 | grid reliability |
| Big Five heritability | h² 0.40, parent–offspring ≈0.20 | personality inheritance |
| Personality stability | r ≈0.68 observed at reliability 0.80 | the twenty-year calibration |
