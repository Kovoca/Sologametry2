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

### The land decides what a nation grows

Farms used to be sized by apportioning a nation's own grain requirement
between its towns by fertility. The shares always summed to one, so **every
nation on every planet grew exactly 125% of what it ate** — a country of
153M on the worst ground per head fed itself as comfortably as one with
fifteen times the soil per person. Fertility decided where the farms sat
and nothing about whether the nation was rich or poor in food, which is
why nobody was ever short of anything and trade had nothing to do.

Now the potential is absolute, from real figures:
- **8 t/ha** on prime ground, **3.5** world average, **under 1** on
  marginal. That eightfold spread is the whole point.
- **35% of prime ground under crops**, which puts a world of average
  fertility near Earth's 10% cropland.
- A nation builds to **3x its own need if it can export by sea**, 1.25x if
  landlocked — Argentina runs ~3x, Canada ~2.5x, France ~1.5x, and export
  agriculture has always followed the ports because grain is a bulk cargo.

A town short of grain buys it (`recipe::GRAIN_IMPORTS`), landed against
**its own mills' draw, not the nation's balance** — farms sit by fertility
and mills by population, so a town on poor ground with many mouths is
permanently short even in a country with plenty. That is what a city is.

Watch for: **the grid is sized off the farms actually built.** Pricing it
off the old figure left a station unable to carry a country's fields, and
a town sat on 3,000 t of grain with its mill shut — which reads as a
famine and is a blackout.

**Known gap, named in `seasons_do_not_starve_anyone`:** the smallest town
of one seed's nation is drained. Its fields and terminal made 10,324 t in
a day, both ended holding none, and its mill got 3,122 and stopped. A
producer hands over everything it makes without first covering the works
next door. It never showed while every town's farms matched its own mills.
The fix belongs in `econ::distribute`, not in this seam.

### A vehicle you cannot use is a way to go broke slowly

Four separate bugs, all from the same blind spot: **a wage haul uses the
firm's lorry**, so owning one adds nothing to it. A vehicle only earns on
a venture, and ventures are occasional.

- **Judge it on the load you can afford to fill.** He saved ten years,
  bought a lorry for 3,625, had 71 left to buy cargo with, and his income
  did not change at all — he ended the decade poorer than year eight.
- **Charge the keep on the days it earns nothing.** A horse eats standing
  in a field. Costing only travelling upkeep let him own a wagon for free
  on the days he drove somebody else's.
- **Weight the earning by how often he actually trades.** With the keep
  charged daily and the benefit assumed daily, he bought a wagon and spent
  seven years feeding it: ten years' work, twenty-four days of food left.
  `days_trading / days_lived` is the honest factor.
- **Never pay to downgrade.** Scoring on affordable load made a barrow look
  better than the wagon he had just spent his savings on, so he bought one.
  And then another.

### Haulage work is local, because the towns are self-sufficient

A driver in a city of sixteen million found seventeen days of work in three
years once inter-town freight had to be real. That is not a bug in the
person — **every town in `region.rs` has its own farm, mill and cannery**,
so almost nothing needs to move between them. Real economies specialise;
this one replicates the chain everywhere.

Most freight is local anyway — grain to the mill, flour to the cannery,
tins to the shops — and that is now where a haulier's living comes from.
Inter-town hauls exist only where the economy genuinely shipped something,
recorded on `Route::moved`.

Two ways of inventing loads instead, both wrong: "whatever this town has
most spare" shuttled the same grain back and forth for a decade; gating on
a stock gap starved him, because a city of 16M always holds more tonnes
than a town of 2M so freight flowed one way and never back. **Compare days
of cover, never tonnes** — but the real answer was to stop inventing.

## Buildings are built from fixtures (`src/building.rs`)

The design doc's *"tile-layered data model, generalizing CDDA's
vehicle-part system to buildings"*, one step in. A vehicle is frames and
engines and cargo bays; a shop is tills and shelving and a loading dock.

### What CDDA has, and what is here

CDDA's part list runs to a couple of hundred. The organising idea is worth
more than the list: **a vehicle is a mobile building** — structure, power,
storage, workstations, protection, controls — which is the same set a shop
has, and exactly why the design doc wants one part system for both.

Here, because each does work in this sim: structure, wheels, engines,
fuel, seating, cargo, **controls** (lose them and it does not move),
**electrics** (battery/alternator/solar — the grid already models power),
**refrigeration** (this economy has spoilage; a reefer is the difference
between hauling food and hauling grain), a **workshop rig** (fault crews
already drive to breakdowns — this is what they carry), and **land gear**
(8 person-hours a tonne of grain is what mechanisation moves).

Deliberately absent, each for a reason: doors/roofs/windows (need weather
and the walkable interior), armour and turrets (need C1's conflict),
lights (need night), kitchens and forges (need crafting).

**A vehicle is ground, not a mode.** You do not enter one, you stand on a
tile of it: an artic is 17 m of occupied road with the seat at (1,1), and
where you stand decides what you can reach. One tile is a metre, which
pins the bottom of the ladder — a 25 m town plot is 25x25 tiles, and a
16.4 km region cell is the world map.

**The point is the staff.** A shop does not employ people because a table
says retail is a tenth of the workforce — it employs them because somebody
works each till, fills each bay and unloads each lorry, and the counts
come off the trade it does. Take the tills out and the cashiers go too.

| fixture | staff (FTE) | does |
|---|---|---|
| checkout | 1.4 | 2.5 t/day *(25 customers/h x 7 kg basket)* |
| shelving | 0.25 | holds 0.4 t |
| stockroom racking | 0.04 | holds 3 t |
| loading bay | 1.2 | 40 t/day |
| served counter | 1.6 | 0.4 t/day |

Checked against a real large supermarket — ~75 t/day, ~300 staff — these
give about 190; the shortfall is management, cleaning, security and online
picking, none of which are fixtures here.

Why it had to exist: the economy modelled farms, mills, canneries and
mines, which is roughly 1.5% of real employment, and gave a city of
sixteen million shops that employed **nobody at all**. Retail is around
10%. The missing half was the big one.

**A rota, not a flex.** A shop does not adjust staffing by the hour — it
writes a rota: somebody decides on Wednesday how many are wanted on
Saturday, and that is how many turn up. Thirty checkouts and eight open on
a wet Tuesday, because eight people were put on the morning shift.

**This is where retail's precarity lives.** A shop worker is rostered a
day at a time, so he is employed and still does not know if he is on next
week — ~60% of UK retail is part-time and part-timers average 2.5-3 days.
He works about a third of the days in a year: enough to live on, never
enough to count on. Rostering a guaranteed six-day week gave him 85% of
the year, which is a salary with a different name.

**Somebody has to see the work is done.** Span of control is 8-15 in
retail and light manufacturing *(real)*; at ten, applied twice, management
comes to a tenth to a seventh of employment, which is what real
organisations run at. Works get chargehands too — leaving them out
understated industrial employment and left nowhere to be promoted to.

**Below about six hands the owner works the till.** A corner shop has a
proprietor who serves, orders, sweeps up and does the books; invent a
separate manager for him and you get three staff of whom two supervise.
The hats only come apart once there are enough hands to need it.

Consequences worth having: a shop worker works ~85% of days, which is what
steady retail looks like; a town's unemployment stops swinging 20% with
the harvest because retail has no season; and **supervising is the one
promotion the economy contains**, gated at ~500 days on the floor *(real
promotion runs two to three years in)* and paying a third more.

Not built yet: the walkable tile interior, the wall/floor layers, and the
electrical and water nodes hung off them. Fixtures first, the same way
parts came before vehicle interiors.

## Somewhere to sleep (`Housing` in `src/person.rs`)

**Housing is the largest thing a household buys** — 25-35% of a low income
against a tenth to a seventh on food *(real; "housing stressed" is the
term for anything over 30%)* — and it was not modelled at all, so everybody
lived rent-free and the poorest man in the world could still save for a
lorry.

Lodging, a tenancy, ownership, or nowhere. Rent is due **whether or not he
was on the rota**, which is the whole difficulty: food can be gone without
for a day and rent cannot, so a bad fortnight puts somebody out that a bad
fortnight of hunger would not have killed.

- **No address, no job.** Hiring chance halves on the street, which is what
  makes homelessness self-sustaining rather than a bad month.
- **Getting back in costs more than staying in** — a deposit plus a month
  up front, which is the real barrier.
- Sleeping out costs condition. **Exposure plus hunger kills faster than
  hunger alone**, which is what actually happens to people on the street.

Two bugs it flushed out, both the same shape — *reckoning against one cost
when there are two*:
- He bought a bicycle keeping thirty days of **food** in reserve, could not
  make rent three weeks later, and was out for over a year. A month's
  reserve means a month of everything.
- `labour::update` ran **before** the shops sold, so every shop read as shut
  and rostered a third of its people. A cashier could not keep a room. Who
  worked today has to be counted after the day's trade, not before.

## The ground you stand on (`src/ground.rs`)

The bottom of the ladder, at a metre to the tile. Everything above it was
furniture for a place nobody could be in: a shop had tills and shelving
but no floor to put them on, a lorry was 17 m of parts with no road under
it, and a man had money, hunger and a trade but no position finer than
which market he was in.

**Generated, never stored** (A1.5, rule R5) — a pure function of seed,
plan and tile coordinates, so no chunk needs its neighbour to exist and
returning gives the same ground. **Only near the person** (A1.6): 160
tiles on foot, 288 in a vehicle.

- **A street is not 32 m of tarmac**, and a street runs *one way* —
  orientation comes from its neighbours, because taking the nearer
  centreline regardless put a crossroads in every single street plot and
  paved three quarters of the town.
- **A road is as big as what uses it.** Four classes, all real
  cross-sections, all two-way:

  **One tile is one metre, so these are tile counts.** Measured off the
  generated ground, not asserted in a doc:

  | | carriageway | reserve | shoulder | footway | made |
  |---|---|---|---|---|---|
  | lane | 5 | - | - | 2+2 | 9 |
  | road | 7 | - | - | 3+3 | 13 |
  | dual | 8+8 | 3 | - | 3+3 | 25 |
  | motorway | 11+11 | 3 | 4+3 | **none** | **32** |

  2.75 m is the narrowest lane anybody lays and 3.65 m the standard, which
  is why a lane's two directions share 5 m unmarked (you do not paint a
  centre line that narrow) and a motorway needs 11 m a side. **A motorway
  fills its whole 32 m plot and has no footway** — that is severance, and
  it is why a trunk route through a town cuts it in two.
- **A reality bubble is a radius, not a viewport.** `Ground::around` used
  to halve the height so it fitted a terminal, so somebody could see twice
  as far east as north — and anything measured across an east-west street
  was silently cut off at 24 m. A motorway's shoulders fell outside the
  window and the test that should have caught it passed. `around` is now
  square; `window` is the thing you look through.
- **Lines are dashed, and solid means something.** A carriageway edge is
  solid, the line between lanes is not; the dash lengthens with speed
  (2 m mark / 7 m gap on a centre line). Painting them all solid turns a
  road into a set of rails. Nothing is painted through a junction.
- **A lorry fills its lane, and that is not a rounding artefact.** A
  2.55 m artic — the European legal maximum — in a 3.65 m lane has 55 cm
  either side, which is finer than a metre grid can express. Vehicle width
  is therefore the one measurement *not* read off the tiles: 3.0 m would
  put an ordinary lorry over the 2.9 m line where the police want notice.
  A test holds `width_m` and the tile footprint together.
- **What you must arrange is a property of the load; whether anything can
  get past you is a property of the road.** Folding them into one scale
  gave every class of street the same verdict, which is the tell that the
  road had stopped mattering. Real bands *(UK Special Types order)*:
  **2.9 m** two days' notice to the police, **3.5 m** escort at walking
  pace on a surveyed route, **4.3 m** an order that takes weeks. A tank at
  3.5-3.9 m is escorted everywhere — but it stops a village lane dead and
  a motorway not at all. A grid transformer at 3.5-4.5 m needs the order,
  which is a real reason a substation stays dark on top of the twelve to
  eighteen months to build one.
- **Size comes from rank among the through-routes, not from distance to
  the middle.** The street grid is deliberately irregular, so "within a
  plot of centre" matched nothing on most towns and silently gave every
  road the same width again. Ranking always yields a hierarchy.
- **A vehicle is walkable.** You stand on the seat to drive and the bed to
  load; that is the whole reason parts are tiles rather than a mode.
- **Furniture mostly is not.** You stand at a till and in front of
  shelving, never in a shelf bay, or a shop is one open room with pictures
  of shelves on the floor.
- A shop's interior is `building.rs`'s fixture list given somewhere to
  stand: tills across the front by the door because that is where you pay
  on the way out, aisles through the middle, racking at the back where the
  lorries come.
- **Urban density is a shape, not a number.** Clark's law was already in
  the plan — flats in the middle, houses outward — and nothing at the tile
  layer read it, so a city of 46M had grass and trees between every
  building. A centre has a **street wall**: buildings on the back of the
  footway sharing party walls, with what open ground there is *behind*
  them rather than around them. Real site coverage *(footprint over plot)*
  is 60-80% in a dense core, 40-50% inner terraces, 15-25% detached
  suburbs, and what produces that spread is **setback and party walls**,
  not plot size. A terrace is therefore not a type here but a consequence:
  a house whose neighbours along the street are built shares walls with
  them; one whose neighbours are fields does not.
- **The bigger road runs through; the lesser one stops at it.** Severance
  was asserted in a comment and enforced nowhere. Reading a junction off
  neighbouring plots could not tell a lane joining a trunk road from two
  lanes meeting, so two motorways crossed at grade in the middle of a
  city. Junctions now read the plan's own through-routes (`col_class` /
  `row_class`), and a street meeting a motorway dead-ends against it.
- **A shopping street has no verges; a street of houses does.** Both are
  `Lot::Street` and before this they rendered identically. Where the
  frontage is dense the whole corridor is made ground, kerb to building
  line; where it is houses there are verges, gardens and a kerb. Paving
  both made a residential lane 100% made surface, which is a runway.
- **Known gap: a street plot is 32 m where a real urban corridor is
  12-20 m** building line to building line. Streets take whole plots, so a
  town is less dense than its population implies and the buildings stand
  further back than they should. Fixing it means letting a street occupy
  part of a plot, which is a `townplan` change, not a `ground` one.
- Front and depth are different axes. They were the same while every
  building was a square inset in its plot; once a shop ran the full width
  of a terrace they came apart, and passing the width where the depth was
  wanted put the back wall halfway up the shop.

`cargo run --release --bin walk -- --where shop`

### Height is a Z level (`ground.rs`)

Dwarf Fortress's answer, and the right one: **a building is a stack of
floors, not a floorplate with a number asserted about it.** `tile_at` takes
a `gz`; above the ground there is only what somebody built, and everything
else at that height is `Tile::Sky`.

A **Z level is a storey, not a metre** — floor-to-floor is 2.5-3 m — so
height is measured in a coarser unit than the ground is. That asymmetry is
what makes one tile of stairwell join two levels.

- **The arithmetic did not close before this.** `HOUSEHOLDS_PER_BLOCK = 8`
  on an 800 m² plate over four storeys is 400 m² a flat, which is a
  mansion. Storeys are now real: a terrace is 2, a high-street shop 2
  *(which is why town centres have people living over the shops)*, a
  tenement 4-6, a shed 1.
- **Four storeys is the limit of a walk-up** — nobody carries shopping
  higher — which is exactly where lifts start.
- **High-density housing is a core with dwellings off it**, not a big room
  with partitions: stairwell, lift, landing, flats opening onto it.
  Subdividing the floorplate the way a house is subdivided gave one
  enormous dwelling per block.
- **Down is a direction like up.** `-1` is a level the same way `+1` is:
  a cellar under a building, a **sewer under a made-up street** *(Victorian
  brick sewers run 3-10 m, so one level is about right)*, then soil, then
  the bedrock `geology.rs` chose when the planet was made — which it had
  known since it was written and nothing at this layer had ever asked.
- **Cellars follow the frost line, and that is not taste.** A footing must
  go below the frost or it heaves, so where frost is deep the hole is dug
  anyway and a basement is nearly free. Real frost depths: Minnesota
  1.5 m, New York 1.2 m, Georgia 0.13 m — and US basement prevalence
  follows almost exactly, ~80% across the Midwest and Northeast, under 10%
  in the South. The opposite constraint is water: **New Orleans has no
  basements because the water table is a metre down**, and nor does
  anywhere built on a marsh.
- **A door to the street is on the ground floor only.** Above it the same
  wall carries a window; you get in by the stair.

### A room has a door and something in it

A bare floor inside four walls is an area, not a place. Interiors
subdivide recursively across the long way until the pieces are real rooms
*(UK space standard: double bedroom 12-14 m², living room 16-20, kitchen
8-12; a flat averages 61 m², a house 76 over about five rooms)*, each
partition gets one doorway, and furniture goes **against the walls** the
way furniture does, leaving the middle to walk in. Room use comes from
position, not from a dice roll: the room off the front door is the one you
live in, the kitchen backs onto the yard where the drains and bins are.

Computed per tile, never stored, like the rest of this layer.

## The scale ladder

DF nests a world tile inside mid-level tiles and you pick a block of those
to play on. This had no such middle — 16.4 km region cell straight to a
25 m plot, a factor of 655 with nothing between, so there was no scale at
which you could look at a valley.

| | metres | nests | built |
|---|---|---|---|
| region cell | 16,384 | the world map | yes |
| **locality** | **1,024** | 16 x 16 per cell | **yes** |
| plot | 32 | 32 x 32 per locality | yes (`townplan`) |
| tile | 1 | 32 x 32 per plot | **yes** (`ground`) |

16 x 32 x 32 = 16,384 — the same number all the way down, which is why the
region cell is 16.384 km. A test asserts the multiplication, because if the
rungs drift apart nothing above or below can be trusted; the plot was 25 m
until something hung off it and the arithmetic had to close.
`cargo run --release --bin zoom` walks the top three,
`--bin walk` stands on the bottom one.

### One cell, zoomed (`src/locality.rs`)

Spec A1.3d: *interpolate the coarse fields, add higher-frequency
variation*, store nothing. Both halves matter, and both were got wrong
first:

- **Do not re-derive the biome.** The coarse pass classifies by percentile
  rank over land (CLAUDE.md already records why absolute cuts are
  fragile), so re-classifying a zoomed patch against fixed thresholds
  turned a mountain cell into flat desert. What you see at a kilometre is
  the *transition* between two 16 km cells — forest thinning into
  grassland over a couple of miles — so a patch takes its parent's country
  or its neighbour's near that edge, dithered so the line is ragged.
- **Detail must be smaller than the gap between bands**, or it stops
  refining the coarse pass and starts overruling it.
- **A river is a channel, not a lowland.** Marking every patch below a
  threshold put water on a tenth of the cell in a scatter. It comes in
  from one neighbour and leaves toward another, wanders, and is joined up
  the whole way — an unjoined wander arrives in pieces.

A town then stands **on** its locality: settle in a green zone and there
are trees between the streets.

## A town's ground plan (`src/townplan.rs`)

The rung between the world map and a building's interior — the CDDA
overmap scale. A market used to be a point with a population hung off it,
and the shop `building.rs` fitted out stood nowhere in particular.

**Generated, never stored** (spec A1.5): a pure function of world seed and
the settlement's cell, so walking away and back rebuilds it identically.
One plot is 25 m — a house and its garden, a shop front, or a lane.

**A place is laid out or it grew, and that decides the shape.** A grid is
what an authority surveys before anybody builds — Roman colonies, the Laws
of the Indies, the US Land Ordinance, Manhattan's Commissioners' Plan,
Barcelona's Eixample. An irregular town is what you get when the routes
came first and the buildings followed. Size correlates because a large
city has almost certainly been planned or replanned: you cannot run water,
sewers, trams and freight through a medieval tangle.

- **Linear** under 2,500 — one street, buildings fronting it, fields
  behind. The commonest village form there is.
- **Organic** to 100,000 — what makes a place look grown is not irregular
  *spacing* (jittering a grid still reads as a grid) but that **the lanes
  do not run through**: they come off the main road, serve a few houses
  and stop, and do not line up with the lane opposite.
- **Grid** above that, and **anisotropic** — Manhattan's blocks are 80 m
  by 274, Chicago's ~100 by 200. Square blocks are the giveaway of a grid
  nobody measured.

Real figures that make a town the size and shape it is:
- Households of **2.4**; blocks **80-200 m** between streets.
- **Clark's law** — density decays exponentially from the centre, which
  holds across cities and centuries and is why a town edge is a gradient.
- **Density comes from building upward**, not from smaller plots. A
  European tenement is four storeys of two dwellings; houses alone gave a
  capital of 16M the density of an American suburb.
- **A bigger place is denser, not just wider.** A fixed 4,000/km² meant
  radius was the only thing population changed, so a village and a
  megacity had identical central density — the same error as every nation
  growing 125% of what it ate. Mean density now scales as
  `1000 x (pop/1000)^0.21`: LA 3,200/km², London 5,700, NYC 11,000,
  Paris 20,000 are not the same number. Results: a village 370, a town of
  18k 3,600, a city of 400k 6,900, one of 8M 8,300.
- **Nobody builds upward where land is cheap.** Judging flats on
  centrality alone put 21 blocks of them in a village of 900. Flats need a
  central site *and* a settlement big enough for land to be worth
  something — real apartment blocks are all but absent below ~20,000 and
  dominant over half a million.
- **A 32 m frontage is five houses, not one.** Real frontages: terrace
  4.5-6 m, semi 8-9, detached 10-15, so the same plot holds five terraces
  or one detached house — and *that* is what makes a terraced street four
  times denser than a suburb of identical plots. One dwelling per plot
  housed a village of 900 with 274 people. Gross densities that fall out
  are real: a terrace ~11,700/km² *(Islington runs 10-13,000)*, a detached
  plot ~2,300 *(suburbs 1,500-3,000)*.
- **Every building has to be got at**, and that is a *reach*, not a decay.
  As a decay it thinned every block from the edge inward and left a city
  of 400,000 at a fifth of its density. A plot over ~90 m from a road
  cannot be reached; one within it is ordinary building land.
- **A town's street classes come from traffic, not rank.** Ranking alone
  gave an 18,000 town a dual carriageway — the identical mistake
  `network.rs` unlearned. The busiest street scales as ~`60 x sqrt(pop)`
  and caps at 120,000 *(a village on a B-road sees a couple of thousand
  vehicles a day, a town of 20k about eight, a city of 500k about forty)*;
  each rank down carries about a third of the one above, because traffic
  is concentrated *(UK motorways: 1% of length, 21% of traffic)*.
- **Shops face the street and crowd the middle** — retail density falls
  off inside a few hundred metres. Sprinkled evenly they gave one shop per
  five houses everywhere, which is a bazaar and not a town.
- Works want cheap land and lorry access, so they are out past the
  housing, which is why you cannot see a cannery from a town square.

`cargo run --release --bin town` draws one.

Where the scale ladder stands: world map **built**; cell detail within a
region, not built; **town plan built**; walkable tiles with furniture, not
built (fixtures exist, tiles do not); a person's position finer than which
market they are in, not built.

## Vehicles are built from parts (`src/vehicle.rs`)

Per the design doc: *"Vehicles (and later, spaceships) have interiors and
individual parts, CDDA-style."* First step of that — not the walkable
interior yet, but the principle underneath it: **a vehicle's capabilities
are computed from what it is made of, never typed in.**

A van carries what its cargo bays hold; it cruises as fast as its engine
pushes its gross weight; it drinks fuel in proportion to that weight; it
costs the sum of its parts. Bolt a bigger engine on and it climbs better
*and* drinks more, because both read the same number. Once parts are
individual they can be damaged, removed, salvaged and improvised, which is
the point and the thing a tier list can never do.

What the parts add up to, against real figures:

| | kerb t | payload | km/h | l/100km | km/day |
|---|---|---|---|---|---|
| bicycle + trailer | 0.13 | 80 kg | 15 | - | 99 |
| second-hand van | 2.45 | 1.2 t | 90 | 8.4 | 693 |
| box truck | 4.4 | 3.6 t | 90 | 11.2 | 693 |
| artic | 13.1 | 24 t | 90 | 30.1 | 693 |

- **This is a modern world** — coal-fired stations, canneries, 44-tonne
  artics — so the ladder is bicycle/van/truck/artic. It used to be handcart,
  pack mule, wagon: furniture from a different century.
- **Nine hours is the legal maximum; seven is a working day.** The rest
  goes on loading, queueing, town speeds and mandatory breaks, which is why
  a real artic covers 550-700 km rather than the 900 its cruising speed
  suggests.
- **Judge a vehicle on when work is *available*, not on what he has done.**
  Using his own record is circular — he cannot trade without a vehicle, so
  his record says never, so he never buys one. A man sat on seven thousand
  days of food doing casual work for five a time because of it.

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

## Reading the output

Three views, three symbol sets. **A glyph must not mean two things in the
same picture**, which took a pass to get right: a town in Desert drew open
ground as `.` — a lane — and one in Tundra as `-`, a road, so its streets
were invisible; and a parked lorry came out as `a#To#########ooo#` because
frame was `#` like a wall, cargo `=` like road, tank `T` like a tree and
wheels `o` like windows. Colour is how a roguelike normally separates
these and there is none here, so the characters have to do it.

One legend function per view (`townplan::plan_legend`, `ground::
ground_legend`), so every binary says the same thing.

**Town plan** (`town`, `zoom` step 3) — 32 m to the character:

| | |
|---|---|
| streets | `.` lane `-` road `=` dual `#` motorway |
| built | `h` houses `H` flats `S` shop `W` works `,` park |
| country | `~` water `b` beach `d` desert `;` savanna `"` grass `*` scrub `f` forest `s` swamp `t` taiga `u` tundra `^` mountain `A` snowcap |

**Ground** (`walk`) — 1 m to the character:

| | |
|---|---|
| you | `@` |
| made | `=` carriageway `:` lane marking `;` hard shoulder `-` footway |
| building | `#` wall `/` door `o` window `.` floor |
| fittings | `$` till `S` shelving `R` racking `L` loading bay `C` counter |
| country | `"` grass `T` tree `*` scrub `,` sand `^` rock `A` snow `~` water |
| vehicle | `+` frame `E` engine `O` wheel `B` cargo bay `F` fuel tank `%` seat `!` controls `b` battery `a` alternator `p` solar `x` refrigeration `w` workshop rig `Y` land gear |

## Conventions

- Scalar grids are flat `Vec<f32>` indexed `y * width + x`. Never
  `Vec<Vec<f32>>`.
- Own the RNG; do not add the `rand` crate to generation code.
- Minimal dependencies. `image` is in only for PNG output.
