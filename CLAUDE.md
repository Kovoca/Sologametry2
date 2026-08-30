# CLAUDE.md

## What this is

A solo-built procedural simulation game in Rust. Long-term scope is
individual-to-intergalactic; the build starts at planetary terrain
generation and expands outward.

**Design docs (`docs/`):**
- `scale-sim-design-doc.md` — the vision. A direction, not a spec (Tarn
  Adams, Principle 1: get something running, then iterate).
- `design-review-triage.md` — an external review of the vision and how its
  gaps were triaged.
- `implementation-spec.md` — **the settled decisions.** Sections A1 (space/
  time), A2 (state model: ledger/journal/realization), A3 (belief/values/
  preferences), A4 (planner), B1–B7, C1 are resolved and written to be
  implemented one section at a time. Built code should conform to this.
- `state-and-economy-spec.md` — commodities, recipes, stockpiles, supply
  and demand, price formation and arbitrage, labour, freight;
  infrastructure as graphs (power/water/comms) with condition and decay;
  and how a government allocates its budget. Every figure is anchored to a
  real-world value so output can be checked rather than eyeballed. Ends
  with the vertical slice's acceptance test.

Only world-gen (elevation → climate → biomes) is implemented so far; the
spec's A1–A4 architecture and everything else is design, not code yet.

The owner drives by testing, not by coding. Keep every commit building and
`cargo test` green. Prefer small, visible increments that can be run and
judged.

**Terminology note:** the current generator's `width × height` grid is the
spec's *coarse pass at region resolution* — one cell of the grid ≈ one
16.4 km region (spec A1.1). The tile/chunk/cell/region coordinate hierarchy
and lazy cell refinement (A1.1, A1.3d) are not built yet.

## Build / run / test

```
cargo run --release              # generate a world, write PNGs to out/
cargo run --release -- --seed N   # specific planet
cargo run --release -- --help     # all flags (--size, --land, --wind, --out)
cargo test                        # regression guards
cargo build                       # debug build (10-30x slower generation)
```

Tunable generation parameters live in `world::Params`; add new knobs there
rather than to function signatures. `World::generate` uses the defaults;
`World::generate_with` takes an explicit `Params`.

Always benchmark generation with `--release`.

## Pipeline (current state)

`src/world.rs` runs the coarse pass (spec A1.3b); `src/hydrology.rs` holds
the flow routing:

1. **Base elevation** — continental fBm mask (X-wrapping, polar falloff
   sinks the poles) + a carve field for multiple continents + ridged noise
   for mountain ranges.
2. **Smooth** — `diffuse_land` makes plains.
3. **Provisional climate** — RNG-free latitude temp + moisture advection,
   to weight erosion.
4. **Erosion & rivers** — priority-flood depression fill, D8 flow routing,
   flow accumulation, gentle stream-power incision into the elevation
   field. Natural endorheic basins recorded here.
5. **Re-smooth**, then re-resolve sea level by percentile on carved terrain.
6. **Rainfall, final** — advection against the *carved* elevation + jitter.
7. **Temperature, final** — latitude + altitude lapse, carved elevation.
8. **Drainage / runoff** — permeability + slope.
9. **Rivers & lakes** — final flow routing weighted by final rainfall;
   `extract_water` picks river cells (top ~5% of land by accumulation, with
   a real downhill neighbour) and lake cells (basins, sinks, pooled flow);
   `despeckle` drops noise-scale fragments.
10. **Biomes** — emergent, classified from percentile-ranked fields.
11. **Geology** (`src/geology.rs`) — rock provinces (igneous / metamorphic
    / sedimentary) from elevation, slope and crustal noise; metal ore, coal
    and petroleum concentrations from rock type + climate history + vein
    noise; soil fertility from rock parent material, rain, temperature,
    slope, drainage and river proximity.

`World` carries `elevation/temperature/rainfall/drainage` fields plus
`flow_accum`, `river: Vec<bool>`, `lake: Vec<bool>`, and `geology`.

`src/polity.rs` runs **after** `World::generate`, reading it — `World`
stays pure geography. It scores habitability (fertility, fresh water,
coast, minerals), seeds cores at the best spaced-out sites, and grows them
by multi-source Dijkstra over a terrain expansion cost, with a reach budget
so remote/hostile ground stays unclaimed. Borders emerge where two
expansions cost the same: ridges, deserts, straits. `--nations N` sets the
seed count; the real count and all borders emerge.

This is *not* the history sim. Consolidation of these into great powers is
the history sim's job.

`src/settlement.rs` runs after both, placing cities and towns inside each
polity. Sites are chosen for food/water/harbour/minerals; every land cell
is then assigned to its cheapest-to-reach settlement, and **population
follows the size of that hinterland** — no rank-size rule is imposed, the
distribution falls out. Tuned against Earth: largest city ~11-46M, median
~300k, largest/median ~150x.

`src/network.rs` builds the trade infrastructure (spec A4.9). Every hub
routes to the *nearest larger* hub, so traffic flows up a hierarchy the way
real road networks grew; existing routes are cheaper to widen than to
duplicate, so traffic converges onto trunk lines. Nothing is designated a
highway — a highway is a road that ended up carrying a lot. Also marks
navigable rivers (big flow, connected to the sea) and chokepoints (trunk
cells with no parallel route: bridges, passes). Routing is 8-connected;
4-connected comes out visibly axis-aligned.

Not built yet: water table, named-region detection, rail/ports/airfields,
flora/fauna, history sim.

## The economy (`src/econ.rs`, `src/slice.rs`)

First of the connective systems, per `state-and-economy-spec.md`.

**The journal is the only write path.** `Ledger::apply` takes `&mut
Journal` so there is no route that changes a stockpile without recording
why. `assert_conserved` runs every tick in debug builds and is asserted
explicitly in tests. This is spec A2.3 and it is not negotiable — it exists
to make the "state quietly evaporates at a handoff" bug class impossible.

Day order: generate power → allocate (shed by priority) → produce →
households buy → shops restock → inter-market trade → prices. Shops sell
*before* restocking, so the day's cover figure means days-in-hand.

`src/region.rs` is the seam between the two halves: it reads a generated
planet and produces an `Economy` for one of its nations. Farms are sized by
the soil those cities actually draw on, the colliery exists only where the
geology put coal (a nation without it imports fuel and gains a dependency
that can be cut), and freight costs come from real distances and whether
there is navigable water between the towns. Nothing here invents capacity.

**Infrastructure costs money, and terrain decides how much.** `src/
infrastructure.rs` prices roads with real figures: ~$2M/km across
grassland, ~$7M through swamp, ~$15M through mountains, plus a bridge
premium at watercourses; maintenance 2-4% of capital a year, worst in
freeze-thaw climates. Terrain multiplies cost far more than distance does.

The consequence worth having is a trap: serving marginal country costs
more per kilometre *and* holds fewer people to pay for it, which is why
remote regions stay poorly connected and why C1.3's "difficult terrain"
is an opportunity for anyone who does not want to be governed.

**Over, through, or around** — the engineer's actual choice, not just a
cost per kilometre. Where a route's summit clears 55% of the land's
relief it becomes a crossing, and the state picks:

- **Pass** — cheap, because it follows the ground. Steep, so heavy freight
  crawls (+50% at a high col), and **snow shuts it every winter**. An
  economy that depends on one has a seasonal hole in it.
- **Tunnel** — ~90M a kilometre, so a few kilometres is a national
  project. Flat, 20% cheaper to haul, and open in February.

Bought when the traffic justifies it and the state can find the money,
which is why real mountain country has both side by side. On one test
nation: over the pass 310/t and closed each winter, or tunnelled for
1,308M and 187/t all year.

**The maintenance deficit runs end to end.** Doctrine sets what share of
upkeep is funded (prudent 100%, negligent 55%); the shortfall decays road
condition by ~6%/yr of the gap; freight cost scales with condition. A
neglected network goes 1.00 -> 0.46 over twenty years and freight rises
43% — slow enough that whoever cut the budget is long gone before it
shows, which is exactly why real governments cut it.

**A nation's routes follow the roads it actually built.** Freight costs
come from a Dijkstra over the generated network priced by the road class
under each step, and the towns are joined by a minimum spanning tree over
those costs. No settlement sits off its own country's network. Straight
lines to the capital were wrong twice over: distance ignored terrain, and
the star topology made two neighbouring cities trade through a capital a
thousand kilometres away. Typical result — 1,196 km of road for a 596 km
gap where a range is in the way.

`region::Nations` folds several nations into **one** economy with lanes
between them — one ledger for the whole planet, because that is what makes
conservation mean anything across a border. Nations are maritime if their
*territory* reaches the sea, not if their biggest cities are ports (most
great cities are inland; the country still ships through whatever harbour
it has). Sea lanes cost about a sixth of land.

`cargo run --release --bin slice` runs the hand-built two-town scenario;
`--bin region` runs one real nation off a generated planet; `--bin world`
runs several of them trading. `tests/economy.rs` is the spec's acceptance
test; `tests/region.rs` guards the seam; `tests/nations.rs` guards trade.

**Known gap, documented in a test rather than hidden:** trade volumes are
too small to equalise prices between nations. Lanes join capitals, so a
cargo from a glut market in one country to a dear market in another must
clear three separate price-versus-freight tests in a row. Real trade is
agents choosing a route end to end, not a pairwise test at every hop.

**Nothing is repaired on a schedule.** A fault must be noticed, reported
over working comms, assigned to a crew, and travelled to before any work
starts. Cut comms and it is never fixed at all.

Real restoration times, and they matter:
- A downed line is **2-4 days**. Travel is almost never the reason — a
  crew lorry covers ~600 km in a working day, and any region with towns in
  it has a depot far closer than that. Distance is not what makes a bad
  utility slow; competence and stores are.
- A destroyed **transformer** is about a week *if a spare is in store* and
  twelve to eighteen months if not, because they are built to order. That
  gap is why holding spares is the highest-leverage prudence decision on a
  grid and why transformers are what grid attacks target.

**A routine fault must not produce a famine.** A working region absorbs an
ordinary failure inside its stock buffers; if every fault cascades, the
model is worthless. Catastrophe should require a real cause — a part that
must be manufactured, or a region that cannot report its own emergency.

`Doctrine` (prudent/negligent) sets redundancy, crew count, depot distance
and spares together — one trait, four concrete purchases.

**Seasons.** A calendar, a harvest curve (most of the year's grain lands in
about six weeks, peaking in early autumn), and a weather multiplier redrawn
each year. Southern-hemisphere regions run six months out of step. Farms
follow the curve; nothing else does.

The important thing learned building it: **in a well-provisioned economy,
seasons do not move prices, and that is correct.** Granaries exist to turn
a burst harvest into steady eating, and real bread prices do not swing.
The seasonal signal lives in *grain*, which is why intermediate commodities
must be priced off industrial demand rather than household demand — price
only what people buy directly and grain has no price at all and the whole
cycle is invisible.

There is a genuine tension: enough farm slack to survive a bad year is
enough buffer to flatten the seasonal price signal. Both are real. Marginal
nations show a visible grain cycle; comfortable ones do not.

Things that were wrong first time, and would be again:
- Price as `scarcity^(1/elasticity)` compounds to absurdity — it priced
  food at 4000x cost. Elasticity relates a *proportional* shortfall to a
  proportional price move: `1 + shortfall/|elasticity|`.
- A town with no works of its own must be able to restock down the road,
  not only through price-gap arbitrage, or it starves in the baseline.
- Traders must not ship a market below its own target cover, or the two
  towns just slosh stock back and forth forever.
- Distribution must prefer local suppliers. Scanning sites in index order
  meant every mill in the country drained the capital's granary before
  touching its own, so the capital read as famine-struck while the
  provinces sat on full silos.
- Price a stored staple off a *slow average* of cover, not today's silo
  reading. Grain stock legitimately halves between harvest and midsummer;
  tracking that directly gave a 10x annual price swing, which no stored
  staple has, because merchants buying at harvest damp the very swing they
  are betting on.
- Size farms against what is actually eaten, not against the mills' rated
  capacity. Multiplying by the mills' headroom too gave a permanent 14%
  surplus and a floored price.

## People (`src/person.rs`)

The first human in the simulation, and the smallest thing that makes this
a game rather than a world simulator. One person with money, hunger, a
trade and a market to be in. `cargo run --release --bin life`.

Three rules from spec A4.1 carry it, and breaking any of them turns it
back into a quest game:
- **Nothing is pushed.** Work exists as a fact in the world and is found by
  being where it is. There are no quests, so there are no quest markers.
- **Nothing scales to the player.** Contracts come from the economy's own
  arithmetic; most are out of reach or not worth the trip.
- **The world does not wait.** A contract heard of last week is gone.

**A wage is not a margin.** The first version paid a haulier the whole
arbitrage, which made one lorry-load worth two years of a man's food and
turned the job into a money printer. The margin belongs to whoever owns
the cargo; a driver is paid by the day. Somebody who wants the margin must
buy the goods first and carry the risk — the trader's path, and a later
thing.

**Work must actually happen.** A haul that pays but shifts nothing leaves
the price gap open, so the same job is offered for ever. Deliveries go
through the journal like every other change, so conservation covers a
person's work too.

Calibration: a day's work buys 6-10 days of food, which is the real figure
for low-wage work; starvation takes about 45 days, but hunger stops someone taking
heavy work long before that, which is what makes poverty a trap rather
than a timer.

Not built: bodies, position finer than which market they are in, skills
beyond a trade tag, relationships, beliefs, ageing, or more than one
person at a time. All specced (A3, B1, B2, B6), none built.

## Calibrate against reality, not against taste

Several passes were tuned by eye toward a "feel" that turned out to be
wrong. Look up the real numbers first — it has repeatedly saved rounds of
pointless tuning:
- Earth's largest state holds 12.7% of land; top 3 hold 27%, top 5 41%.
- Earth's largest city is ~37M; median city over 100k is ~250k.
- Earth is ~29% land, ~91% of it claimed, ~10% arable.

## Rules that already cost time to learn

- Continental interiors collapse to desert without evapotranspiration
  recycling in the rainfall model.
- Advecting rows independently streaks the map vertically — mix adjacent
  rows each step.
- Radial falloff on *elevation* makes a central dome; it belongs on the land
  *mask*. Ridged noise gives ranges.
- Absolute biome thresholds are fragile. Rank fields over land tiles first
  (`rank_over_land`).
- Same lesson in geology: classify rock by stretching scores over land then
  cutting, and anchor deposit concentrations on a high percentile, never on
  the raw max — one outlier cell otherwise squashes every real deposit.
  Percentile *cuts* would force identical rock ratios on every world; use
  stretch-then-fixed-cut so worlds genuinely differ.
- A purely global deposit anchor leaves whole regions with nothing to mine.
  Fix is a per-region *lift* applied after global normalisation (raise each
  region's best ground over the workable line, capped), not a per-region
  anchor — dividing by a local mean is self-defeating, because a deposit
  raises the very bar it must clear. If you try it anyway, the averaging
  window must be far larger than a deposit.
- The map wraps, so apparently separate continents are usually one
  connected landmass. Check `geology.landmasses` before assuming otherwise.
- Keep RNG and sorts deterministic — a seed must rebuild the same planet
  forever. Use `f32::total_cmp` + index tiebreak, never `sort_unstable` on
  bare floats.

### Agents in the economy

Every money printer so far has had the same shape: an agent gets paid for
something that did not happen.

- **A rule that binds firms must bind people.** `Economy::surplus` — what a
  market holds above its own working reserve — is the single definition of
  what is for sale, and bulk trade and a man with a lorry both go through
  it. Applying it to firms only made the most profitable trade in the
  country "strip whichever town is shortest of something".
- **Settle on what was delivered, not what was intended.** A market quotes a
  price for goods it has none of. `deliver` returns the tonnage that
  actually moved, and pay, profit and the refund of an unfilled outlay all
  key off that number.
- **Haulage is mostly routine distribution, not arbitrage.** Offering only
  price-gap hauls left a driver in a city of 16M idle for four months and
  starved him. Freight exists because firms move their own stock.
- Route capacity is population × ~24 tonnes per head per year *(real:* UK
  moves ~1.6bn t across 67M people*)*, not a multiple of food demand —
  internal freight is dominated by bulk, which dwarfs what people eat.
- A persistent price gap between two towns is not automatically a bug. A
  mountain port 1,200 km of road from the grain belt, at $0.26/t-km, is
  *supposed* to pay three times as much for grain.

## Who does the work (`src/labour.rs`)

The join between the infrastructure sim and the human one. It exists for
one causal chain: **a transformer fails, so the mill has no power, so the
mill does not run, so the people who work at the mill are not working, so
they cannot buy food.** Before it, a blackout was an inconvenience to a
stockpile; it did not happen to anybody.

Headcount comes straight from the spec's recipe labour-hours — 8 h to
bring in a tonne of grain *(US agriculture runs ~9)*, 0.2 h to mill a
tonne of flour, 1.2 to can it — over an 1,800-hour working year *(OECD
~1,750)*. Nothing is invented.

Scope note: this is the labour market **for the trades the economy has**,
not a city's whole employment. Six commodities is a thin slice, and
reporting the other 99% as unemployed would be nonsense.

Three things that were wrong first time:
- **A power station's `throughput` is a sentinel** meaning "whatever the
  grid can carry" (1e9). Reading headcount off it staffed one coal station
  with 4.1M people. Rate plants off `grid.capacity()`, and note their
  staffing does not follow load — nobody sends half the shift home because
  demand dipped. It is all of them, or when the plant is down, none.
- **A farm is not idle out of harvest.** Output collapses between harvests;
  employment does not, because the ground still needs ploughing, sowing
  and tending. Tying headcount to tonnage put a farming town at 89%
  unemployment for ten months. Real agricultural labour swings by about 2x
  season to season, so use a floor, not a collapse.
- **Nobody is hired and fired by the day.** Firms hoard labour through a
  short stoppage. Without ~21-day stickiness a town flickered between full
  employment and half idle overnight.

Wages sag where hands are idle but **do not collapse** — nominal wages are
sticky. What actually gives is whether anybody is hiring, so work is
rationed by availability (`chance_of_work`) rather than by price. **A
venture is exempt**: nobody hires you to trade on your own account, so
self-employment is the way out of a town with no work in it, which is
exactly why a vehicle is worth saving for.

Result worth having: with a spare transformer in store, a man survives the
failure and buys a handcart. Without one, unemployment climbs from day
274, he goes hungry on 279 and is dead on 323. **The fault alone did not
kill him — the fault plus winter did.**

### What a blackout actually does to a man

The first version starved him, and that was wrong in three separate ways.
All three came from the same habit — making a fault bite by breaking
something it does not really break.

- **A blackout stops the factory, not the farm.** Spec A.2 says so and it
  is right: a grid failure shuts a mill within the hour while the tractors
  run on diesel. `produce()` gated every site on power, so a dead
  transformer idled a nation's agriculture. `needs_water` had been sitting
  unused since it was written; this is what it was for. The *shift offer*
  had the same two gates and had to be fixed with it.
- **Wages must lag prices.** Pay was anchored to today's food price, so
  bread quintupled in a blackout and wages quintupled the same week and
  nobody felt a thing. Nominal wages are renegotiated once a year
  *(studies of nominal rigidity: 9-18 months)*; that lag *is* how a supply
  shock makes people poorer. `Workforce::food_anchor` is a ~120-day
  average of the cost of living, and pay is set against that.
- **Compare in days of food, never in money.** He finished a blackout year
  holding half again as much cash and much worse off. Nominal balances are
  not comparable across a price shock.

The real result, on one nation: with a spare transformer the run is
byte-identical to no fault at all. Without one, his wage stands still
while bread goes 0.99 -> 4.93, and a year later he is still walking
because the handcart never became affordable. **A supply shock does not
kill a working man, it keeps him poor.**

### A wage buys 6-10 days of food, and ours bought 2.6

This note ("real low-wage work is 6-10x") sat in this file while the code
paid 2.6 and 3.2. It is not cosmetic: at 2.6 a labourer who gets work
three days in five cannot feed himself working flat out, so nobody ever
saved for anything and every life ended poorer than it began. Being *at*
subsistence is the historical condition; being permanently below it is
not, or there would be nobody left.

Same shape as the farm floor — 0.45 of peak headcount meant a farm shed
more than half its people out of harvest, which reads as a permanent
agricultural depression. Real farm employment swings about a quarter
season to season, so the floor is 0.80.

**Known gap this made visible: firms do not pay wages.** Money is not
conserved, so there is no counterweight to a wage. Over 400 days with the
fault live the squeeze is right and he is clearly worse off; run it to 730
and the repair lets prices fall faster than the wage anchor, and he comes
out ahead. Real disinflation with sticky wages causes *unemployment* for
exactly this reason — firms cannot afford the real wage — and we cannot
model that until a wage is somebody's cost. That is the money-ledger job.

### Getting there is not free (`src/travel.rs`)

Freight cost answers what a *tonne* costs. It says nothing about how one
man crosses a pass with something to sell, and skipping that let a trader
with 60 in hand quietly shift 24-tonne lorry-loads.

**What you can carry is a thing you own.** Real payloads and speeds, and
the ladder falls out of them rather than being asserted:

| | payload | speed | price |
|---|---|---|---|
| on foot | 35 kg | 28 km/day | - |
| handcart | 150 kg | 22 | 25 wage-days |
| pack mule | 90 kg | 28 | 120 |
| wagon | 800 kg | 32 | 300 |
| lorry | 24 t | 550 | 700 |

- **There is no fixed ladder, and coding one was wrong.** A mule carries
  less than a barrow, so on a made road it is four months' wages *spent to
  get worse*. Its niche is where wheels stop working — pick the best by
  `payload x speed` on the surface actually being worked, net of upkeep,
  and the mule correctly never gets bought on a paved country.
- An animal eats whether it works or not; a lorry's running costs several
  times its driver's wage. Both must leave the purse. Charging running
  cost against profit while never deducting it is free diesel.
- A wage haul is driving the *firm's* lorry, so it is not limited by what
  the driver owns. Only a venture is.

### Road class comes from real traffic, not from percentiles

Ranking a map's own stretches and cutting at percentiles gives every
world the same 8% highway and 65% track however rich or empty it is —
the identical mistake geology had to unlearn. Use absolute figures:

- **Paving pays at ~300 vehicles/day** *(World Bank rule of thumb: 200-400)*.
  Below it, grading gravel beats laying pavement, which is why most road
  length on Earth is unpaved.
- **Dualling pays at ~13,000 vehicles/day** on a single carriageway.
- Traffic here is population served; ~2% of it makes an inter-urban trip
  on a given day.

Consequence worth having: a 2.3M nation came out with a town 1,092 km
away over **open country — 71 days on foot, and no lorry gets through at
all**. That is C1.3's "difficult terrain" arriving on its own.

**Known gap this exposed: there are no villages.** The 2,000th settlement
still holds 260k people, so every link between any two of them earns its
pavement honestly and `Road::Track` never appears. That is a hole in
`settlement.rs`, not in the rule.

### A person's books must close

`money == start + earned - spent - staked`, asserted. `spent` is food and
vehicles; `staked` is cargo still on the road, which is not a leak. Every
money printer so far was caught by this identity failing.

## Conventions

- Scalar grids are flat `Vec<f32>` indexed `y * width + x`. Never
  `Vec<Vec<f32>>`.
- Own the RNG; do not add the `rand` crate to generation code.
- Minimal dependencies. `image` is in only for PNG output.
